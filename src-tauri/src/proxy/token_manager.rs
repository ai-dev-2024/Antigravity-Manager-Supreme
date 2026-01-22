// Removeredundant top layerImport，BecauseThese are included in the code by full path 或LocalImportHandle
use dashmap::DashMap;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::proxy::rate_limit::RateLimitTracker;
use crate::proxy::sticky_config::StickySessionConfig;

#[derive(Debug, Clone)]
pub struct ProxyToken {
    pub account_id: String,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
    pub timestamp: i64,
    pub email: String,
    pub account_path: PathBuf,  // AccountFilePath，used forUpdate
    pub project_id: Option<String>,
    pub subscription_tier: Option<String>, // "FREE" | "PRO" | "ULTRA"
    pub remaining_quota: Option<i32>, // [FIX #563] Remaining quota for priority sorting
    pub protected_models: HashSet<String>, // [NEW #621]
}


pub struct TokenManager {
    tokens: Arc<DashMap<String, ProxyToken>>,  // account_id -> ProxyToken
    current_index: Arc<AtomicUsize>,
    last_used_account: Arc<tokio::sync::Mutex<Option<(String, std::time::Instant)>>>,
    data_dir: PathBuf,
    rate_limit_tracker: Arc<RateLimitTracker>,  // New: Rate LimitTrace器
    sticky_config: Arc<tokio::sync::RwLock<StickySessionConfig>>, // New：SchedulingConfig
    session_accounts: Arc<DashMap<String, String>>, // New：Session与AccountMapping (SessionID -> AccountID)
    preferred_account_id: Arc<tokio::sync::RwLock<Option<String>>>, // [FIX #820] priorityUsing的AccountID（fixedAccountMode）
}

impl TokenManager {
    /// CreateNew TokenManager
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            tokens: Arc::new(DashMap::new()),
            current_index: Arc::new(AtomicUsize::new(0)),
            last_used_account: Arc::new(tokio::sync::Mutex::new(None)),
            data_dir,
            rate_limit_tracker: Arc::new(RateLimitTracker::new()),
            sticky_config: Arc::new(tokio::sync::RwLock::new(StickySessionConfig::default())),
            session_accounts: Arc::new(DashMap::new()),
            preferred_account_id: Arc::new(tokio::sync::RwLock::new(None)), // [FIX #820]
        }
    }

    /// StartRate LimitRecordAutomatically clean the backgroundTask（每60秒Check并ClearExpiredRecord）
    pub fn start_auto_cleanup(&self) {
        let tracker = self.rate_limit_tracker.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                let cleaned = tracker.cleanup_expired();
                if cleaned > 0 {
                    tracing::info!("🧹 Auto-cleanup: Removed {} expired rate limit record(s)", cleaned);
                }
            }
        });
        tracing::info!("✅ Rate limit auto-cleanup task started (interval: 60s)");
    }
    
    /// from master applicationAccountDirectoryLoadAllAccount
    pub async fn load_accounts(&self) -> Result<usize, String> {
        let accounts_dir = self.data_dir.join("accounts");
        
        if !accounts_dir.exists() {
            return Err(format!("AccountDirectoryDoes not exist: {:?}", accounts_dir));
        }

        // Reload should reflect current on-disk state (accounts can be added/removed/disabled).
        self.tokens.clear();
        self.current_index.store(0, Ordering::SeqCst);
        {
            let mut last_used = self.last_used_account.lock().await;
            *last_used = None;
        }
        
        let entries = std::fs::read_dir(&accounts_dir)
            .map_err(|e| format!("ReadAccountDirectoryFailed: {}", e))?;
        
        let mut count = 0;
        
        for entry in entries {
            let entry = entry.map_err(|e| format!("ReadDirectory项Failed: {}", e))?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            
            // TryingLoadAccount
            match self.load_single_account(&path).await {
                Ok(Some(token)) => {
                    let account_id = token.account_id.clone();
                    self.tokens.insert(account_id, token);
                    count += 1;
                },
                Ok(None) => {
                    // jump overInvalidAccount
                },
                Err(e) => {
                    tracing::debug!("LoadAccountFailed {:?}: {}", path, e);
                }
            }
        }
        
        Ok(count)
    }

    /// againLoadSpecifyAccount（used forQuotaUpdatereal time afterSync）
    pub async fn reload_account(&self, account_id: &str) -> Result<(), String> {
        let path = self.data_dir.join("accounts").join(format!("{}.json", account_id));
        if !path.exists() {
            return Err(format!("AccountFileDoes not exist: {:?}", path));
        }

        match self.load_single_account(&path).await {
            Ok(Some(token)) => {
                self.tokens.insert(account_id.to_string(), token);
                Ok(())
            }
            Ok(None) => Err("AccountLoadFailed".to_string()),
            Err(e) => Err(format!("SyncAccountFailed: {}", e)),
        }
    }

    /// againLoadAllAccount
    pub async fn reload_all_accounts(&self) -> Result<usize, String> {
        self.load_accounts().await
    }
    
    /// LoadsingleAccount
    async fn load_single_account(&self, path: &PathBuf) -> Result<Option<ProxyToken>, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("ReadFileFailed: {}", e))?;
        
        let mut account: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| format!("Parse JSON Failed: {}", e))?;

        if account
            .get("disabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            tracing::debug!(
                "Skipping disabled account file: {:?} (email={})",
                path,
                account.get("email").and_then(|v| v.as_str()).unwrap_or("<unknown>")
            );
            return Ok(None);
        }

        // 【New】QuotaProtectCheck - 在Check proxy_disabled BeforeExecute
        // soCan在LoadAutomatically restore whenQuotaResumed的Account
        if self.check_and_protect_quota(&mut account, path).await {
            tracing::debug!(
                "Account skipped due to quota protection: {:?} (email={})",
                path,
                account.get("email").and_then(|v| v.as_str()).unwrap_or("<unknown>")
            );
            return Ok(None);
        }

        // CheckinitiativeDisableStatus
        if account
            .get("proxy_disabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            tracing::debug!(
                "Skipping proxy-disabled account file: {:?} (email={})",
                path,
                account.get("email").and_then(|v| v.as_str()).unwrap_or("<unknown>")
            );
            return Ok(None);
        }

        let account_id = account["id"].as_str()
            .ok_or("Lack id Field")?
            .to_string();
        
        let email = account["email"].as_str()
            .ok_or("Lack email Field")?
            .to_string();
        
        let token_obj = account["token"].as_object()
            .ok_or("Lack token Field")?;
        
        let access_token = token_obj["access_token"].as_str()
            .ok_or("Lack access_token")?
            .to_string();
        
        let refresh_token = token_obj["refresh_token"].as_str()
            .ok_or("Lack refresh_token")?
            .to_string();
        
        let expires_in = token_obj["expires_in"].as_i64()
            .ok_or("Lack expires_in")?;
        
        let timestamp = token_obj["expiry_timestamp"].as_i64()
            .ok_or("Lack expiry_timestamp")?;
        
        // project_id YesOptional的
        let project_id = token_obj.get("project_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        
        
        // 【New】Extract subscription level (subscription_tier 为 "FREE" | "PRO" | "ULTRA")
        let subscription_tier = account.get("quota")
            .and_then(|q| q.get("subscription_tier"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        
        // [FIX #563] extractMaximumRemainingQuotaPercentageused forPrioritySort (Option<i32> now)
        let remaining_quota = account.get("quota")
            .and_then(|q| self.calculate_quota_stats(q));
            // .filter(|&r| r > 0); // Remove >0 Filter，Because 0% 也YesValidData，只YesPriority低
        
        // 【New #621】Extraction restrictedModelList
        let protected_models: HashSet<String> = account.get("protected_models")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default();
        
        Ok(Some(ProxyToken {
            account_id,
            access_token,
            refresh_token,
            expires_in,
            timestamp,
            email,
            account_path: path.clone(),
            project_id,
            subscription_tier,
            remaining_quota,
            protected_models,
        }))
    }

    
    /// CheckAccountYesNoShould被QuotaProtect
    /// IfQuotalower thanThreshold，automaticDisableAccount并Return true
    async fn check_and_protect_quota(&self, account_json: &mut serde_json::Value, account_path: &PathBuf) -> bool {
        // 1. LoadQuotaProtectConfig
        let config = match crate::modules::config::load_app_config() {
            Ok(cfg) => cfg.quota_protection,
            Err(_) => return false, // ConfigLoadFailed，Skip protection
        };
        
        if !config.enabled {
            return false; // QuotaProtection is notEnable
        }
        
        // 2. GetQuotaInfo
        // Notice：usNeed clone QuotaInfoto traverse，Avoid borrow conflicts，但ModifiedYesagainst account_json 的
        let quota = match account_json.get("quota") {
            Some(q) => q.clone(),
            None => return false, // 无QuotaInfo，jump over
        };

        // 3. CheckYesNoAlready被Accountlevel orModel级QuotaProtectDisable
        let is_proxy_disabled = account_json.get("proxy_disabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        
        let reason = account_json.get("proxy_disabled_reason")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        
        if is_proxy_disabled {
            if reason == "quota_protection" {
                // [Compatible性 #621] IfYesby old versionAccountlevel protectionDisable的，Tryingrestore and convert toModel级
                return self.check_and_restore_quota(account_json, account_path, &quota, &config).await;
            }
            return true; // other reasonsDisable，jump overLoad
        }
        
        // 4. GetModelList
        let models = match quota.get("models").and_then(|m| m.as_array()) {
            Some(m) => m,
            None => return false,
        };

        // 5. Traverse the monitoredModel，CheckProtect and restore
        let threshold = config.threshold_percentage as i32;


        let mut changed = false;

        for model in models {
            let name = model.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if !config.monitored_models.iter().any(|m| m == name) {
                continue; 
            }

            let percentage = model.get("percentage").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let account_id = account_json.get("id").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();

            if percentage <= threshold {
                // Trigger protection (Issue #621 Change toModel级)
                if self.trigger_quota_protection(account_json, &account_id, account_path, percentage, threshold, name).await.unwrap_or(false) {
                    changed = true;
                }
            } else {
                // Tryingrecover (IfBeforerestricted)
                let protected_models = account_json.get("protected_models").and_then(|v| v.as_array());
                let is_protected = protected_models.map_or(false, |arr| {
                    arr.iter().any(|m| m.as_str() == Some(name))
                });

                if is_protected {
                    if self.restore_quota_protection(account_json, &account_id, account_path, name).await.unwrap_or(false) {
                        changed = true;
                    }
                }
            }
        }
        
        let _ = changed; // avoid unused Warning，IfSubsequent logicNeedCancontinueUsing
        
        // we no longerBecauseQuotareasonReturn true（i.e. no more skippingAccount），
        // 而YesLoadAnd in get_token Shi JinLineFilter。
        false
    }
    
    /// calculateAccount的MaximumRemainingQuotaPercentage（used forSort）
    /// ReturnValue: Option<i32> (max_percentage)
    fn calculate_quota_stats(&self, quota: &serde_json::Value) -> Option<i32> {
        let models = match quota.get("models").and_then(|m| m.as_array()) {
            Some(m) => m,
            None => return None,
        };
        
        let mut max_percentage = 0;
        let mut has_data = false;
        
        for model in models {
            if let Some(pct) = model.get("percentage").and_then(|v| v.as_i64()) {
                let pct_i32 = pct as i32;
                if pct_i32 > max_percentage {
                    max_percentage = pct_i32;
                }
                has_data = true;
            }
        }
        
        if has_data {
            Some(max_percentage)
        } else {
            None
        }
    }
    
    /// triggerQuotaProtect，LimitspecificModel (Issue #621)
    /// Return true Ifchanged
    async fn trigger_quota_protection(
        &self,
        account_json: &mut serde_json::Value,
        account_id: &str,
        account_path: &PathBuf,
        current_val: i32,
        threshold: i32,
        model_name: &str,
    ) -> Result<bool, String> {
        // 1. Initialize protected_models Array（IfDoes not exist）
        if account_json.get("protected_models").is_none() {
            account_json["protected_models"] = serde_json::Value::Array(Vec::new());
        }
        
        let protected_models = account_json["protected_models"].as_array_mut().unwrap();
        
        // 2. CheckYesNoAlready exists
        if !protected_models.iter().any(|m| m.as_str() == Some(model_name)) {
            protected_models.push(serde_json::Value::String(model_name.to_string()));
            
            tracing::info!(
                "Account {} 的Model {} 因Quotarestricted（{}% <= {}%）has been added to the protectionList",
                account_id, model_name, current_val, threshold
            );
            
            // 3. Writedisk
            std::fs::write(account_path, serde_json::to_string_pretty(account_json).unwrap())
                .map_err(|e| format!("WriteFileFailed: {}", e))?;
            
            return Ok(true);
        }
        
        Ok(false)
    }
    
    /// Checkand fromAccountlevel protection recovery（Migrate toModel级，Issue #621）
    async fn check_and_restore_quota(
        &self,
        account_json: &mut serde_json::Value,
        account_path: &PathBuf,
        quota: &serde_json::Value,
        config: &crate::models::QuotaProtectionConfig,
    ) -> bool {
        // [Compatible性] If该AccountCurrentin proxy_disabled=true And the reasonYes quota_protection，
        // we will proxy_disabled set to false，但MeanwhileUpdate其 protected_models List。
        tracing::info!(
            "CurrentlymigrateAccount {} 从GlobalQuotaProtectMode至Model级ProtectMode",
            account_json.get("email").and_then(|v| v.as_str()).unwrap_or("unknown")
        );

        account_json["proxy_disabled"] = serde_json::Value::Bool(false);
        account_json["proxy_disabled_reason"] = serde_json::Value::Null;
        account_json["proxy_disabled_at"] = serde_json::Value::Null;

        let threshold = config.threshold_percentage as i32;
        let mut protected_list = Vec::new();

        if let Some(models) = quota.get("models").and_then(|m| m.as_array()) {
            for model in models {
                let name = model.get("name").and_then(|v| v.as_str()).unwrap_or("");
                if !config.monitored_models.iter().any(|m| m == name) { continue; }
                
                let percentage = model.get("percentage").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                if percentage <= threshold {
                    protected_list.push(serde_json::Value::String(name.to_string()));
                }
            }
        }
        
        account_json["protected_models"] = serde_json::Value::Array(protected_list);
        
        let _ = std::fs::write(account_path, serde_json::to_string_pretty(account_json).unwrap());
        
        false // Return false means it is nowCanTryingLoad该Account（Model级Filterwill be in get_token occurs when）
    }
    
    /// restore specificModel的QuotaProtect (Issue #621)
    /// Return true Ifchanged
    async fn restore_quota_protection(
        &self,
        account_json: &mut serde_json::Value,
        account_id: &str,
        account_path: &PathBuf,
        model_name: &str,
    ) -> Result<bool, String> {
        if let Some(arr) = account_json.get_mut("protected_models").and_then(|v| v.as_array_mut()) {
            let original_len = arr.len();
            arr.retain(|m| m.as_str() != Some(model_name));
            
            if arr.len() < original_len {
                tracing::info!("Account {} 的Model {} QuotaResumed，Remove protectionList", account_id, model_name);
                std::fs::write(account_path, serde_json::to_string_pretty(account_json).unwrap())
                    .map_err(|e| format!("WriteFileFailed: {}", e))?;
                return Ok(true);
            }
        }
        
        Ok(false)
    }

    
    /// GetCurrentAvailable的 Token（SupportviscositySessionand intelligent scheduling）
    /// Parameter `quota_group` used to distinguish "claude" vs "gemini" 组
    /// Parameter `force_rotate` 为 true will be ignored whenLock定，Force switchingAccount
    /// Parameter `session_id` used acrossRequestmaintainSessionviscosity
    /// Parameter `target_model` used forCheckQuotaProtect (Issue #621)
    pub async fn get_token(
        &self, 
        quota_group: &str, 
        force_rotate: bool, 
        session_id: Option<&str>,
        target_model: &str,
    ) -> Result<(String, String, String), String> {
        // 【Optimization Issue #284】Add 5 秒Timeout，prevent deathLock
        let timeout_duration = std::time::Duration::from_secs(5);
        match tokio::time::timeout(timeout_duration, self.get_token_internal(quota_group, force_rotate, session_id, target_model)).await {
            Ok(result) => result,
            Err(_) => Err("Token acquisition timeout (5s) - system too busy or deadlock detected".to_string()),
        }
    }

    /// Insideaccomplish：Get Token 的Corelogic
    async fn get_token_internal(
        &self, 
        quota_group: &str, 
        force_rotate: bool, 
        session_id: Option<&str>,
        target_model: &str,
    ) -> Result<(String, String, String), String> {
        let mut tokens_snapshot: Vec<ProxyToken> = self.tokens.iter().map(|e| e.value().clone()).collect();
        let total = tokens_snapshot.len();
        if total == 0 {
            return Err("Token pool is empty".to_string());
        }

        // ===== 【Optimization】Based on subscription level andRemainingQuotaSort =====
        // [FIX #563] Priority: ULTRA > PRO > FREE, 同tierInternal priority is higherQuotaAccount
        // reason: ULTRA/PRO Reset快，Prioritize consumption；FREE Reset慢，Used for pocketing
        //       high quotaAccountpriorityUsing，avoid lowQuotaAccountused up
        tokens_snapshot.sort_by(|a, b| {
            let tier_priority = |tier: &Option<String>| match tier.as_deref() {
                Some("ULTRA") => 0,
                Some("PRO") => 1,
                Some("FREE") => 2,
                _ => 3,
            };
            
            // First: compare by subscription tier
            let tier_cmp = tier_priority(&a.subscription_tier)
                .cmp(&tier_priority(&b.subscription_tier));
            
            if tier_cmp != std::cmp::Ordering::Equal {
                return tier_cmp;
            }
            
            // [FIX #563] Second: compare by remaining quota percentage (higher is better)
            // Accounts with unknown/zero percentage go last within their tier
            let quota_a = a.remaining_quota.unwrap_or(0);
            let quota_b = b.remaining_quota.unwrap_or(0);
            quota_b.cmp(&quota_a)  // Descending: higher percentage first
        });
        
        // 【DebugLog】PrintSortlaterAccountorder
        tracing::info!(
            "🔄 [Token Rotation] Accounts: {:?}",
            tokens_snapshot.iter().map(|t| format!(
                "{}(protected={:?})", 
                t.email, t.protected_models
            )).collect::<Vec<_>>()
        );

        // 0. ReadCurrentSchedulingConfig
        let scheduling = self.sticky_config.read().await.clone();
        use crate::proxy::sticky_config::SchedulingMode;
        
        // 【New】CheckQuotaProtectYesNoEnable（IfClose，then ignore protected_models Check）
        let quota_protection_enabled = crate::modules::config::load_app_config()
            .map(|cfg| cfg.quota_protection.enabled)
            .unwrap_or(false);

        // ===== [FIX #820] fixedAccountMode：priorityUsingSpecifyAccount =====
        let preferred_id = self.preferred_account_id.read().await.clone();
        if let Some(ref pref_id) = preferred_id {
            // Search firstAccount
            if let Some(preferred_token) = tokens_snapshot.iter().find(|t| &t.account_id == pref_id) {
                // CheckAccountYesNoAvailable（未Rate Limit、beenQuotaProtect）
                let normalized_target = crate::proxy::common::model_mapping::normalize_to_standard_id(target_model)
                    .unwrap_or_else(|| target_model.to_string());

                let is_rate_limited = self.is_rate_limited_by_account_id(&preferred_token.account_id);
                let is_quota_protected = quota_protection_enabled && preferred_token.protected_models.contains(&normalized_target);

                if !is_rate_limited && !is_quota_protected {
                    tracing::info!(
                        "🔒 [FIX #820] Using preferred account: {} (fixed mode)",
                        preferred_token.email
                    );

                    // directUsingpriorityAccount，jump overRound Robinlogic
                    let mut token = preferred_token.clone();

                    // Check token YesNoExpired（in advance5minuteRefresh）
                    let now = chrono::Utc::now().timestamp();
                    if now >= token.timestamp - 300 {
                        tracing::debug!("Account {} 的 token Coming soonExpired，CurrentlyRefresh...", token.email);
                        match crate::modules::oauth::refresh_access_token(&token.refresh_token).await {
                            Ok(token_response) => {
                                token.access_token = token_response.access_token.clone();
                                token.expires_in = token_response.expires_in;
                                token.timestamp = now + token_response.expires_in;

                                if let Some(mut entry) = self.tokens.get_mut(&token.account_id) {
                                    entry.access_token = token.access_token.clone();
                                    entry.expires_in = token.expires_in;
                                    entry.timestamp = token.timestamp;
                                }
                                let _ = self.save_refreshed_token(&token.account_id, &token_response).await;
                            }
                            Err(e) => {
                                tracing::warn!("Preferred account token refresh failed: {}", e);
                                // continueUsing旧 token，Let the subsequent logicHandleFailed
                            }
                        }
                    }

                    // Make sure there is project_id
                    let project_id = if let Some(pid) = &token.project_id {
                        pid.clone()
                    } else {
                        match crate::proxy::project_resolver::fetch_project_id(&token.access_token).await {
                            Ok(pid) => {
                                if let Some(mut entry) = self.tokens.get_mut(&token.account_id) {
                                    entry.project_id = Some(pid.clone());
                                }
                                let _ = self.save_project_id(&token.account_id, &pid).await;
                                pid
                            }
                            Err(_) => "bamboo-precept-lgxtn".to_string() // fallback
                        }
                    };

                    return Ok((token.access_token, project_id, token.email));
                } else {
                    if is_rate_limited {
                        tracing::warn!("🔒 [FIX #820] Preferred account {} is rate-limited, falling back to round-robin", preferred_token.email);
                    } else {
                        tracing::warn!("🔒 [FIX #820] Preferred account {} is quota-protected for {}, falling back to round-robin", preferred_token.email, target_model);
                    }
                }
            } else {
                tracing::warn!("🔒 [FIX #820] Preferred account {} not found in pool, falling back to round-robin", pref_id);
            }
        }
        // ===== [END FIX #820] =====

        // 【Optimization Issue #284】将LockMove operation outside the loop，avoidDuplicateGetLock
        // advanceGet last_used_account snapshot of，Avoid adding multiple times in a loopLock
        let last_used_account_id = if quota_group != "image_gen" {
            let last_used = self.last_used_account.lock().await;
            last_used.clone()
        } else {
            None
        };

        let mut attempted: HashSet<String> = HashSet::new();
        let mut last_error: Option<String> = None;
        let mut need_update_last_used: Option<(String, std::time::Instant)> = None;

        for attempt in 0..total {
            let rotate = force_rotate || attempt > 0;

            // ===== 【Core】viscositySessionwith intelligent scheduling logic =====
            let mut target_token: Option<ProxyToken> = None;
            
            // normalizationTargetModelnamed standard ID，used forQuotaProtectCheck
            let normalized_target = crate::proxy::common::model_mapping::normalize_to_standard_id(target_model)
                .unwrap_or_else(|| target_model.to_string());
            
            // Mode A: viscositySessionHandle (CacheFirst 或 Balance And there is session_id)
            if !rotate && session_id.is_some() && scheduling.mode != SchedulingMode::PerformanceFirst {
                let sid = session_id.unwrap();
                
                // 1. CheckSessionYesNoBoundAccount
                if let Some(bound_id) = self.session_accounts.get(sid).map(|v| v.clone()) {
                    // 【repair】Pass first account_id Find the correspondingAccount，Get其 email
                    // 2. Convert email -> account_id CheckboundAccountYesNoRate Limit
                    if let Some(bound_token) = tokens_snapshot.iter().find(|t| t.account_id == bound_id) {
                        let key = self.email_to_account_id(&bound_token.email).unwrap_or_else(|| bound_token.account_id.clone());
                        let reset_sec = self.rate_limit_tracker.get_remaining_wait(&key);
                        if reset_sec > 0 {
                            // 【repair Issue #284】Unbind and switch nowAccount，No more blockingWait
                            // reason：blockWaitwill lead toConcurrentRequest时Client socket Timeout (UND_ERR_SOCKET)
                            tracing::debug!(
                                "Sticky Session: Bound account {} is rate-limited ({}s), unbinding and switching.",
                                bound_token.email, reset_sec
                            );
                            self.session_accounts.remove(sid);
                        } else if !attempted.contains(&bound_id) && !(quota_protection_enabled && bound_token.protected_models.contains(&normalized_target)) {
                            // 3. AccountAvailableand has not been marked asTryingFailed，Prioritize reuse
                            tracing::debug!("Sticky Session: Successfully reusing bound account {} for session {}", bound_token.email, sid);
                            target_token = Some(bound_token.clone());
                        } else if quota_protection_enabled && bound_token.protected_models.contains(&normalized_target) {
                            tracing::debug!("Sticky Session: Bound account {} is quota-protected for model {} [{}], unbinding and switching.", bound_token.email, normalized_target, target_model);
                            self.session_accounts.remove(sid);
                        }
                    } else {
                        // boundAccount已Does not exist（May被Delete），unbundle
                        tracing::debug!("Sticky Session: Bound account not found for session {}, unbinding", sid);
                        self.session_accounts.remove(sid);
                    }
                }
            }

            // Mode B: Atomic化 60s GlobalLock定 (For none session_id situationalDefaultProtect)
            // 【repair】PerformancepriorityModeshould be skipped 60s Lock定；
            if target_token.is_none() && !rotate && quota_group != "image_gen" && scheduling.mode != SchedulingMode::PerformanceFirst {
                // 【Optimization】UsingadvanceGetsnapshot of，No longer added within the loopLock
                if let Some((account_id, last_time)) = &last_used_account_id {
                    // [FIX #3] 60s Lockdefinite logicCheck `attempted` Set，avoidDuplicateTryingFailed的Account
                    if last_time.elapsed().as_secs() < 60 && !attempted.contains(account_id) {
                        if let Some(found) = tokens_snapshot.iter().find(|t| &t.account_id == account_id) {
                            // 【repair】CheckRate LimitStatus和QuotaProtect，Avoid reuse ofLockDeterminedAccount
                            if !self.is_rate_limited_by_account_id(&found.account_id) && !(quota_protection_enabled && found.protected_models.contains(&normalized_target)) {
                                tracing::debug!("60s Window: Force reusing last account: {}", found.email);
                                target_token = Some(found.clone());
                            } else {
                                if self.is_rate_limited_by_account_id(&found.account_id) {
                                    tracing::debug!("60s Window: Last account {} is rate-limited, skipping", found.email);
                                } else {
                                    tracing::debug!("60s Window: Last account {} is quota-protected for model {} [{}], skipping", found.email, normalized_target, target_model);
                                }
                            }
                        }
                    }
                }
                
                // If notLock定，则Round RobinSelect newAccount
                if target_token.is_none() {
                    let start_idx = self.current_index.fetch_add(1, Ordering::SeqCst) % total;
                    for offset in 0..total {
                        let idx = (start_idx + offset) % total;
                        let candidate = &tokens_snapshot[idx];
                        if attempted.contains(&candidate.account_id) {
                            continue;
                        }

                        // 【New #621】Model级Rate LimitCheck
                        if quota_protection_enabled && candidate.protected_models.contains(&normalized_target) {
                            tracing::debug!("Account {} is quota-protected for model {} [{}], skipping", candidate.email, normalized_target, target_model);
                            continue;
                        }

                        // 【New】Actively avoidRate Limit或 5xx LockDeterminedAccount (高AvailableOptimization)
                        if self.is_rate_limited_by_account_id(&candidate.account_id) { // Changed to account_id
                            continue;
                        }

                        target_token = Some(candidate.clone());
                        // 【Optimization】markNeedUpdate，Will write back later
                        need_update_last_used = Some((candidate.account_id.clone(), std::time::Instant::now()));
                        
                        // IfYesSessionfirst allocated andNeedviscosity，Create binding here
                        if let Some(sid) = session_id {
                            if scheduling.mode != SchedulingMode::PerformanceFirst {
                                self.session_accounts.insert(sid.to_string(), candidate.account_id.clone());
                                tracing::debug!("Sticky Session: Bound new account {} to session {}", candidate.email, sid);
                            }
                        }
                        break;
                    }
                }
            } else if target_token.is_none() {
                // Mode C: 纯Round RobinMode (Round-robin) or forced rotation
                let start_idx = self.current_index.fetch_add(1, Ordering::SeqCst) % total;
                tracing::info!("🔄 [Mode C] Round-robin from idx {}, total: {}", start_idx, total);
                for offset in 0..total {
                    let idx = (start_idx + offset) % total;
                    let candidate = &tokens_snapshot[idx];
                    
                    if attempted.contains(&candidate.account_id) {
                        tracing::debug!("  [{}] {} - SKIP: already attempted", idx, candidate.email);
                        continue;
                    }

                    // 【New #621】Model级Rate LimitCheck
                    if quota_protection_enabled && candidate.protected_models.contains(&normalized_target) {
                        tracing::info!("  ⛔ {} - SKIP: quota-protected for {} [{}]", candidate.email, normalized_target, target_model);
                        continue;
                    }

                    // 【New】Actively avoidRate Limit或 5xx LockDeterminedAccount
                    if self.is_rate_limited_by_account_id(&candidate.account_id) { // Changed to account_id
                        tracing::info!("  ⏳ {} - SKIP: rate-limited", candidate.email);
                        continue;
                    }

                    tracing::debug!("  [{}] {} - SELECTED", idx, candidate.email);
                    target_token = Some(candidate.clone());
                    
                    if rotate {
                        tracing::debug!("Force Rotation: Switched to account: {}", candidate.email);
                    }
                    break;
                }
            }
            
            let mut token = match target_token {
                Some(t) => t,
                None => {
                    // optimismResetStrategy: Double layer protection mechanism
                    // WhenAllAccountWhen neither can be selected,MayYesCaused by timing competitionStatus不Sync
                    
                    // Calculate the shortestWaitTime
                    let min_wait = tokens_snapshot.iter()
                        .filter_map(|t| self.rate_limit_tracker.get_reset_seconds(&t.account_id))
                        .min();
                    
                    // Layer 1: IfshortestWaitTime <= 2秒,ExecuteBufferDelay
                    if let Some(wait_sec) = min_wait {
                        if wait_sec <= 2 {
                            tracing::warn!(
                                "All accounts rate-limited but shortest wait is {}s. Applying 500ms buffer for state sync...",
                                wait_sec
                            );
                            
                            // BufferDelay 500ms
                            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                            
                            // againTryingchooseAccount
                            let retry_token = tokens_snapshot.iter()
                                .find(|t| !attempted.contains(&t.account_id) && !self.is_rate_limited_by_account_id(&t.account_id)); // Changed to account_id
                            
                            if let Some(t) = retry_token {
                                tracing::info!("✅ Buffer delay successful! Found available account: {}", t.email);
                                t.clone()
                            } else {
                                // Layer 2: Bufferstill noAvailableAccount,ExecuteoptimismReset
                                tracing::warn!(
                                    "Buffer delay failed. Executing optimistic reset for all {} accounts...",
                                    tokens_snapshot.len()
                                );
                                
                                // ClearAllRate LimitRecord
                                self.rate_limit_tracker.clear_all();
                                
                                // againTryingchooseAccount
                                let final_token = tokens_snapshot.iter()
                                    .find(|t| !attempted.contains(&t.account_id));
                                
                                if let Some(t) = final_token {
                                    tracing::info!("✅ Optimistic reset successful! Using account: {}", t.email);
                                    t.clone()
                                } else {
                                    // AllStrategies areFailed,ReturnError
                                    return Err(
                                        "All accounts failed after optimistic reset. Please check account health.".to_string()
                                    );
                                }
                            }
                        } else {
                            // WaitTime > 2秒,normalReturnError
                            return Err(format!("All accounts are currently limited. Please wait {}s.", wait_sec));
                        }
                    } else {
                        // 无Rate LimitRecordbut still noAvailableAccount,MayYesOther questions
                        return Err("All accounts failed or unhealthy.".to_string());
                    }
                }
            };

        
            // 3. Check token YesNoExpired（in advance5minuteRefresh）
            let now = chrono::Utc::now().timestamp();
            if now >= token.timestamp - 300 {
                tracing::debug!("Account {} 的 token Coming soonExpired，CurrentlyRefresh...", token.email);

                // call OAuth Refresh token
                match crate::modules::oauth::refresh_access_token(&token.refresh_token).await {
                    Ok(token_response) => {
                        tracing::debug!("Token RefreshSuccess！");

                        // UpdatelocalMemoryObjectFor follow-upUsing
                        token.access_token = token_response.access_token.clone();
                        token.expires_in = token_response.expires_in;
                        token.timestamp = now + token_response.expires_in;

                        // SyncUpdate跨Threadshared DashMap
                        if let Some(mut entry) = self.tokens.get_mut(&token.account_id) {
                            entry.access_token = token.access_token.clone();
                            entry.expires_in = token.expires_in;
                            entry.timestamp = token.timestamp;
                        }

                        // SyncPlace the order（Avoid continuing after rebootUsingExpired timestamp resulting in frequentRefresh）
                        if let Err(e) = self.save_refreshed_token(&token.account_id, &token_response).await {
                            tracing::debug!("SaveRefreshlater token Failed ({}): {}", token.email, e);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Token RefreshFailed ({}): {}，TryingNextAccount", token.email, e);
                        if e.contains("\"invalid_grant\"") || e.contains("invalid_grant") {
                            tracing::error!(
                                "Disabling account due to invalid_grant ({}): refresh_token likely revoked/expired",
                                token.email
                            );
                            let _ = self
                                .disable_account(&token.account_id, &format!("invalid_grant: {}", e))
                                .await;
                            self.tokens.remove(&token.account_id);
                        }
                        // Avoid leaking account emails to API clients; details are still in logs.
                        last_error = Some(format!("Token refresh failed: {}", e));
                        attempted.insert(token.account_id.clone());

                        // 【Optimization】markNeedClearLock定，Avoid addingLock
                        if quota_group != "image_gen" {
                            if matches!(&last_used_account_id, Some((id, _)) if id == &token.account_id) {
                                need_update_last_used = Some((String::new(), std::time::Instant::now())); // EmptyString representationNeedClear
                            }
                        }
                        continue;
                    }
                }
            }

            // 4. Make sure there is project_id
            let project_id = if let Some(pid) = &token.project_id {
                pid.clone()
            } else {
                tracing::debug!("Account {} Lack project_id，TryingGet...", token.email);
                match crate::proxy::project_resolver::fetch_project_id(&token.access_token).await {
                    Ok(pid) => {
                        if let Some(mut entry) = self.tokens.get_mut(&token.account_id) {
                            entry.project_id = Some(pid.clone());
                        }
                        let _ = self.save_project_id(&token.account_id, &pid).await;
                        pid
                    }
                    Err(e) => {
                        tracing::error!("Failed to fetch project_id for {}: {}", token.email, e);
                        last_error = Some(format!("Failed to fetch project_id for {}: {}", token.email, e));
                        attempted.insert(token.account_id.clone());

                        // 【Optimization】markNeedClearLock定，Avoid addingLock
                        if quota_group != "image_gen" {
                            if matches!(&last_used_account_id, Some((id, _)) if id == &token.account_id) {
                                need_update_last_used = Some((String::new(), std::time::Instant::now())); // EmptyString representationNeedClear
                            }
                        }
                        continue;
                    }
                }
            };

            // 【Optimization】在SuccessReturn前，unifiedUpdate last_used_account（IfNeed）
            if let Some((new_account_id, new_time)) = need_update_last_used {
                if quota_group != "image_gen" {
                    let mut last_used = self.last_used_account.lock().await;
                    if new_account_id.is_empty() {
                        // EmptyString representationNeedClearLock定
                        *last_used = None;
                    } else {
                        *last_used = Some((new_account_id, new_time));
                    }
                }
            }

            return Ok((token.access_token, project_id, token.email));
        }

        Err(last_error.unwrap_or_else(|| "All accounts failed".to_string()))
    }

    async fn disable_account(&self, account_id: &str, reason: &str) -> Result<(), String> {
        let path = if let Some(entry) = self.tokens.get(account_id) {
            entry.account_path.clone()
        } else {
            self.data_dir
                .join("accounts")
                .join(format!("{}.json", account_id))
        };

        let mut content: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&path).map_err(|e| format!("ReadFileFailed: {}", e))?,
        )
        .map_err(|e| format!("Parse JSON Failed: {}", e))?;

        let now = chrono::Utc::now().timestamp();
        content["disabled"] = serde_json::Value::Bool(true);
        content["disabled_at"] = serde_json::Value::Number(now.into());
        content["disabled_reason"] = serde_json::Value::String(truncate_reason(reason, 800));

        std::fs::write(&path, serde_json::to_string_pretty(&content).unwrap())
            .map_err(|e| format!("WriteFileFailed: {}", e))?;
        
        // 【repair Issue #3】从Memory中RemoveDisable的Account，prevent being60sLockDefinite logic continuesUsing
        self.tokens.remove(account_id);

        tracing::warn!("Account disabled: {} ({:?})", account_id, path);
        Ok(())
    }

    /// Save project_id 到AccountFile
    async fn save_project_id(&self, account_id: &str, project_id: &str) -> Result<(), String> {
        let entry = self.tokens.get(account_id)
            .ok_or("AccountDoes not exist")?;
        
        let path = &entry.account_path;
        
        let mut content: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(path).map_err(|e| format!("ReadFileFailed: {}", e))?
        ).map_err(|e| format!("Parse JSON Failed: {}", e))?;
        
        content["token"]["project_id"] = serde_json::Value::String(project_id.to_string());
        
        std::fs::write(path, serde_json::to_string_pretty(&content).unwrap())
            .map_err(|e| format!("WriteFileFailed: {}", e))?;
        
        tracing::debug!("已Save project_id 到Account {}", account_id);
        Ok(())
    }
    
    /// SaveRefreshlater token 到AccountFile
    async fn save_refreshed_token(&self, account_id: &str, token_response: &crate::modules::oauth::TokenResponse) -> Result<(), String> {
        let entry = self.tokens.get(account_id)
            .ok_or("AccountDoes not exist")?;
        
        let path = &entry.account_path;
        
        let mut content: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(path).map_err(|e| format!("ReadFileFailed: {}", e))?
        ).map_err(|e| format!("Parse JSON Failed: {}", e))?;
        
        let now = chrono::Utc::now().timestamp();
        
        content["token"]["access_token"] = serde_json::Value::String(token_response.access_token.clone());
        content["token"]["expires_in"] = serde_json::Value::Number(token_response.expires_in.into());
        content["token"]["expiry_timestamp"] = serde_json::Value::Number((now + token_response.expires_in).into());
        
        std::fs::write(path, serde_json::to_string_pretty(&content).unwrap())
            .map_err(|e| format!("WriteFileFailed: {}", e))?;
        
        tracing::debug!("已SaveRefreshlater token 到Account {}", account_id);
        Ok(())
    }
    
    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    /// pass email GetSpecifyAccount的 Token（Used for preheating, etc.NeedSpecifyAccountscene）
    /// 此MethodWill automaticallyRefreshExpired的 token
    pub async fn get_token_by_email(&self, email: &str) -> Result<(String, String, String), String> {
        // FindAccountInfo
        let token_info = {
            let mut found = None;
            for entry in self.tokens.iter() {
                let token = entry.value();
                if token.email == email {
                    found = Some((
                        token.account_id.clone(),
                        token.access_token.clone(),
                        token.refresh_token.clone(),
                        token.timestamp,
                        token.expires_in,
                        chrono::Utc::now().timestamp(),
                        token.project_id.clone(),
                    ));
                    break;
                }
            }
            found
        };

        let (
            account_id,
            current_access_token,
            refresh_token,
            timestamp,
            expires_in,
            now,
            project_id_opt,
        ) = match token_info {
            Some(info) => info,
            None => return Err(format!("not foundAccount: {}", email)),
        };

        let project_id = project_id_opt.unwrap_or_else(|| "bamboo-precept-lgxtn".to_string());
        
        // CheckYesNoExpired (in advance5minute)
        if now < timestamp + expires_in - 300 {
            return Ok((current_access_token, project_id, email.to_string()));
        }

        tracing::info!("[Warmup] Token for {} is expiring, refreshing...", email);

        // call OAuth Refresh token
        match crate::modules::oauth::refresh_access_token(&refresh_token).await {
            Ok(token_response) => {
                tracing::info!("[Warmup] Token refresh successful for {}", email);
                let new_now = chrono::Utc::now().timestamp();
                
                // UpdateCache
                if let Some(mut entry) = self.tokens.get_mut(&account_id) {
                    entry.access_token = token_response.access_token.clone();
                    entry.expires_in = token_response.expires_in;
                    entry.timestamp = new_now;
                }

                // Saveto disk
                let _ = self.save_refreshed_token(&account_id, &token_response).await;

                Ok((token_response.access_token, project_id, email.to_string()))
            }
            Err(e) => Err(format!("[Warmup] Token refresh failed for {}: {}", email, e)),
        }
    }
    
    // ===== Rate LimitmanageMethod =====
    
    /// markAccountRate Limit(从Outsidecall,usually in handler 中)
    /// Parameter为 email，InsideWill automaticallyConvert为 account_id
    pub fn mark_rate_limited(
        &self,
        email: &str,
        status: u16,
        retry_after_header: Option<&str>,
        error_body: &str,
    ) {
        // 【alternative】Convert email -> account_id
        let key = self.email_to_account_id(email).unwrap_or_else(|| email.to_string());
        self.rate_limit_tracker.parse_from_error(
            &key,
            status,
            retry_after_header,
            error_body,
            None,
        );
    }
    

    /// CheckAccountYesNo在Rate Limit中 (directUsing account_id)
    pub fn is_rate_limited_by_account_id(&self, account_id: &str) -> bool {
        self.rate_limit_tracker.is_rate_limited(account_id)
    }
    
    /// GetdistanceRate LimitResetHow many seconds are left?
    #[allow(dead_code)]
    pub fn get_rate_limit_reset_seconds(&self, account_id: &str) -> Option<u64> {
        self.rate_limit_tracker.get_reset_seconds(account_id)
    }
    
    /// ClearExpired的Rate LimitRecord
    #[allow(dead_code)]
    pub fn clean_expired_rate_limits(&self) {
        self.rate_limit_tracker.cleanup_expired();
    }
    
    /// 【alternative】pass email Find the corresponding account_id
    /// used to handlers incoming email Convert为 tracker Using的 account_id
    fn email_to_account_id(&self, email: &str) -> Option<String> {
        self.tokens.iter()
            .find(|entry| entry.value().email == email)
            .map(|entry| entry.value().account_id.clone())
    }
    
    /// ClearSpecifyAccount的Rate LimitRecord
    #[allow(dead_code)]
    pub fn clear_rate_limit(&self, account_id: &str) -> bool {
        self.rate_limit_tracker.clear(account_id)
    }
    
    /// markAccountRequestSuccess，ResetcontinuousFailedcount
    /// 
    /// 在RequestSuccessCompletecall after，will thisAccount的Failedcount reset to zero，
    /// next timeFailedtime from the shortestLock定TimeBegin（intelligentRate Limit）。
    pub fn mark_account_success(&self, account_id: &str) {
        self.rate_limit_tracker.mark_success(account_id);
    }
    
    /// CheckYesNo有Available的 Google Account
    /// 
    /// used for"Only the bottom line"Modeintelligent judgment:WhenAll Google Account不AvailableOnly thenUsingOutsideprovider。
    /// 
    /// # Parameter
    /// - `quota_group`: Quota组("claude" 或 "gemini"),Not yetUsingbut reserved for future useExtension
    /// - `target_model`: TargetModelName(Normalized),used forQuotaProtectCheck
    /// 
    /// # ReturnValue
    /// - `true`: There is at least oneAvailableAccount(未Rate Limitand has not beenQuotaProtect)
    /// - `false`: AllAccountNeitherAvailable(被Rate Limitor beQuotaProtect)
    /// 
    /// # Example
    /// ```ignore
    /// // CheckYesNo有AvailableAccountHandle claude-sonnet Request
    /// let has_available = token_manager.has_available_account("claude", "claude-sonnet-4-20250514").await;
    /// if !has_available {
    ///     // switch toOutsideprovider
    /// }
    /// ```
    pub async fn has_available_account(&self, _quota_group: &str, target_model: &str) -> bool {
        // CheckQuotaProtectYesNoEnable
        let quota_protection_enabled = crate::modules::config::load_app_config()
            .map(|cfg| cfg.quota_protection.enabled)
            .unwrap_or(false);
        
        // TraverseAllAccount,CheckYesNo有Available的
        for entry in self.tokens.iter() {
            let token = entry.value();
            
            // 1. CheckYesNo被Rate Limit
            if self.is_rate_limited_by_account_id(&token.account_id) {
                tracing::debug!(
                    "[Fallback Check] Account {} is rate-limited, skipping",
                    token.email
                );
                continue;
            }
            
            // 2. CheckYesNo被QuotaProtect(IfEnable)
            if quota_protection_enabled && token.protected_models.contains(target_model) {
                tracing::debug!(
                    "[Fallback Check] Account {} is quota-protected for model {}, skipping",
                    token.email,
                    target_model
                );
                continue;
            }
            
            // find at least oneAvailableAccount
            tracing::debug!(
                "[Fallback Check] Found available account: {} for model {}",
                token.email,
                target_model
            );
            return true;
        }
        
        // AllAccountNeitherAvailable
        tracing::info!(
            "[Fallback Check] No available Google accounts for model {}, fallback should be triggered",
            target_model
        );
        false
    }
    
    /// 从AccountFileGetQuotaRefreshTime
    /// 
    /// Return该AccountlatestQuotaRefreshTimestring（ISO 8601 Format）
    pub fn get_quota_reset_time(&self, email: &str) -> Option<String> {
        // Trying从AccountFileReadQuotaInfo
        let accounts_dir = self.data_dir.join("accounts");
        
        // TraverseAccountFileFind the corresponding email
        if let Ok(entries) = std::fs::read_dir(&accounts_dir) {
            for entry in entries.flatten() {
                if let Ok(content) = std::fs::read_to_string(entry.path()) {
                    if let Ok(account) = serde_json::from_str::<serde_json::Value>(&content) {
                        // Check email YesNomatch
                        if account.get("email").and_then(|e| e.as_str()) == Some(email) {
                            // Get quota.models earliest of reset_time
                            if let Some(models) = account
                                .get("quota")
                                .and_then(|q| q.get("models"))
                                .and_then(|m| m.as_array()) 
                            {
                                // find the earliest reset_time（the most conservativeLockSet strategy）
                                let mut earliest_reset: Option<&str> = None;
                                for model in models {
                                    if let Some(reset_time) = model.get("reset_time").and_then(|r| r.as_str()) {
                                        if !reset_time.is_empty() {
                                            if earliest_reset.is_none() || reset_time < earliest_reset.unwrap() {
                                                earliest_reset = Some(reset_time);
                                            }
                                        }
                                    }
                                }
                                if let Some(reset) = earliest_reset {
                                    return Some(reset.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }
    
    /// UsingQuotaRefreshTimeaccurateLock定Account
    /// 
    /// When API Return 429 但None quotaResetDelay 时,TryingUsingAccount的QuotaRefreshTime
    /// 
    /// # Parameter
    /// - `model`: Optional的ModelName,used forModelLevelRate Limit
    pub fn set_precise_lockout(&self, email: &str, reason: crate::proxy::rate_limit::RateLimitReason, model: Option<String>) -> bool {
        if let Some(reset_time_str) = self.get_quota_reset_time(email) {
            tracing::info!("turn upAccount {} 的QuotaRefreshTime: {}", email, reset_time_str);
            self.rate_limit_tracker.set_lockout_until_iso(email, &reset_time_str, reason, model)
        } else {
            tracing::debug!("not foundAccount {} 的QuotaRefreshTime,将UsingDefaultbackoff strategy", email);
            false
        }
    }
    
    /// real timeRefreshQuotaand preciseLock定Account
    /// 
    /// When 429 This is called whenMethod:
    /// 1. real time callQuotaRefresh API Get最New reset_time
    /// 2. Using最New reset_time accurateLock定Account
    /// 3. IfGetFailed,Return false Let the callerUsingfallback strategy
    /// 
    /// # Parameter
    /// - `model`: Optional的ModelName,used forModelLevelRate Limit
    pub async fn fetch_and_lock_with_realtime_quota(
        &self,
        email: &str,
        reason: crate::proxy::rate_limit::RateLimitReason,
        model: Option<String>,
    ) -> bool {
        // 1. 从 tokens 中Get该Account的 access_token
        let access_token = {
            let mut found_token: Option<String> = None;
            for entry in self.tokens.iter() {
                if entry.value().email == email {
                    found_token = Some(entry.value().access_token.clone());
                    break;
                }
            }
            found_token
        };
        
        let access_token = match access_token {
            Some(t) => t,
            None => {
                tracing::warn!("cannot be foundAccount {} 的 access_token,Unable to real-timeRefreshQuota", email);
                return false;
            }
        };
        
        // 2. callQuotaRefresh API
        tracing::info!("Account {} Currentlyreal timeRefreshQuota...", email);
        match crate::modules::quota::fetch_quota(&access_token, email).await {
            Ok((quota_data, _project_id)) => {
                // 3. From the latestQuotaextracted from reset_time
                let earliest_reset = quota_data.models.iter()
                    .filter_map(|m| {
                        if !m.reset_time.is_empty() {
                            Some(m.reset_time.as_str())
                        } else {
                            None
                        }
                    })
                    .min();
                
                if let Some(reset_time_str) = earliest_reset {
                    tracing::info!(
                        "Account {} real timeQuotaRefreshSuccess,reset_time: {}",
                        email, reset_time_str
                    );
                    self.rate_limit_tracker.set_lockout_until_iso(email, reset_time_str, reason, model)
                } else {
                    tracing::warn!("Account {} QuotaRefreshSuccessbut not found reset_time", email);
                    false
                }
            },
            Err(e) => {
                tracing::warn!("Account {} real timeQuotaRefreshFailed: {:?}", email, e);
                false
            }
        }
    }
    
    /// markAccountRate Limit(AsyncVersion,Supportreal timeQuotaRefresh)
    /// 
    /// Level threeFallbackStrategy:
    /// 1. priority: API Return quotaResetDelay → directUsing
    /// 2. suboptimal: real timeRefreshQuota → Getup to date reset_time
    /// 3. Guaranteed: UsinglocalCacheQuota → ReadAccountFile
    /// 4. reveal all the details: Exponential BackoffStrategy → DefaultLock定Time
    /// 
    /// # Parameter
    /// - `model`: Optional的ModelName,used forModelLevelRate Limit。Pass in actualUsing的ModelCanavoidDifferentModelQuotainfluence each other
    pub async fn mark_rate_limited_async(
        &self,
        account_id: &str,
        status: u16,
        retry_after_header: Option<&str>,
        error_body: &str,
        model: Option<&str>,  // 🆕 NewModelParameter
    ) {
        // Check API YesNoReturnpreciseRetryTime
        let has_explicit_retry_time = retry_after_header.is_some() || 
            error_body.contains("quotaResetDelay");
        
        if has_explicit_retry_time {
            // API ReturnaccurateTime(quotaResetDelay),directUsing,No need to be real-timeRefresh
            if let Some(m) = model {
                tracing::debug!("Account {} 的Model {} 的 429 ResponsePacket含 quotaResetDelay,directUsing API Return的Time", account_id, m);
            } else {
                tracing::debug!("Account {} 的 429 ResponsePacket含 quotaResetDelay,directUsing API Return的Time", account_id);
            }
            self.rate_limit_tracker.parse_from_error(
                account_id,
                status,
                retry_after_header,
                error_body,
                model.map(|s| s.to_string()),
            );
            return;
        }
        
        // SureRate Limitreason
        let reason = if error_body.to_lowercase().contains("model_capacity") {
            crate::proxy::rate_limit::RateLimitReason::ModelCapacityExhausted
        } else if error_body.to_lowercase().contains("exhausted") || error_body.to_lowercase().contains("quota") {
            crate::proxy::rate_limit::RateLimitReason::QuotaExhausted
        } else {
            crate::proxy::rate_limit::RateLimitReason::Unknown
        };
        
        // API 未Return quotaResetDelay,Needreal timeRefreshQuotaGetaccurateLock定Time
        if let Some(m) = model {
            tracing::info!("Account {} 的Model {} 的 429 Response未Packet含 quotaResetDelay,Tryingreal timeRefreshQuota...", account_id, m);
        } else {
            tracing::info!("Account {} 的 429 Response未Packet含 quotaResetDelay,Tryingreal timeRefreshQuota...", account_id);
        }
        
        if self.fetch_and_lock_with_realtime_quota(account_id, reason, model.map(|s| s.to_string())).await {
            tracing::info!("Account {} Usedreal timeQuotaaccurateLock定", account_id);
            return;
        }
        
        // real timeRefreshFailed,TryingUsinglocalCache的QuotaRefreshTime
        if self.set_precise_lockout(account_id, reason, model.map(|s| s.to_string())) {
            tracing::info!("Account {} UsedlocalCacheQuotaLock定", account_id);
            return;
        }
        
        // 都Failed了,Fallback toExponential BackoffStrategy
        tracing::warn!("Account {} Unable to getQuotaRefreshTime,UsingExponential BackoffStrategy", account_id);
        self.rate_limit_tracker.parse_from_error(
            account_id,
            status,
            retry_after_header,
            error_body,
            model.map(|s| s.to_string()),
        );
    }

    // ===== SchedulingConfigRelatedMethod =====

    /// GetCurrentSchedulingConfig
    pub async fn get_sticky_config(&self) -> StickySessionConfig {
        self.sticky_config.read().await.clone()
    }

    /// UpdateSchedulingConfig
    pub async fn update_sticky_config(&self, new_config: StickySessionConfig) {
        let mut config = self.sticky_config.write().await;
        *config = new_config;
        tracing::debug!("Scheduling configuration updated: {:?}", *config);
    }

    /// ClearspecificSessionThe stickinessMapping
    #[allow(dead_code)]
    pub fn clear_session_binding(&self, session_id: &str) {
        self.session_accounts.remove(session_id);
    }

    /// ClearAllSessionThe stickinessMapping
    pub fn clear_all_sessions(&self) {
        self.session_accounts.clear();
    }

    // ===== [FIX #820] fixedAccountModeRelatedMethod =====

    /// SetpriorityUsing的AccountID（fixedAccountMode）
    /// incoming Some(account_id) EnablefixedAccountMode，incoming None recoverRound RobinMode
    pub async fn set_preferred_account(&self, account_id: Option<String>) {
        let mut preferred = self.preferred_account_id.write().await;
        if let Some(ref id) = account_id {
            tracing::info!("🔒 [FIX #820] Fixed account mode enabled: {}", id);
        } else {
            tracing::info!("🔄 [FIX #820] Round-robin mode enabled (no preferred account)");
        }
        *preferred = account_id;
    }

    /// GetCurrentpriorityUsing的AccountID
    pub async fn get_preferred_account(&self) -> Option<String> {
        self.preferred_account_id.read().await.clone()
    }
}

fn truncate_reason(reason: &str, max_len: usize) -> String {
    if reason.chars().count() <= max_len {
        return reason.to_string();
    }
    let mut s: String = reason.chars().take(max_len).collect();
    s.push('…');
    s
}
