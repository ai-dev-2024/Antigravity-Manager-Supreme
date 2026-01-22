use dashmap::DashMap;
use std::time::{SystemTime, Duration};
use regex::Regex;

/// Rate LimitreasonType
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RateLimitReason {
    /// Quota exhausted (QUOTA_EXHAUSTED)
    QuotaExhausted,
    /// rateLimit (RATE_LIMIT_EXCEEDED)
    RateLimitExceeded,
    /// ModelCapacity exhausted (MODEL_CAPACITY_EXHAUSTED)
    ModelCapacityExhausted,
    /// ServerError (5xx)
    ServerError,
    /// unknown reason
    Unknown,
}

/// Rate LimitInfo
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct RateLimitInfo {
    /// Rate LimitResetTime
    pub reset_time: SystemTime,
    /// RetryInterval(秒)
    #[allow(dead_code)]
    pub retry_after_sec: u64,
    /// DetectionTime
    #[allow(dead_code)]
    pub detected_at: SystemTime,
    /// Rate Limitreason
    #[allow(dead_code)] // Used for logging and diagnostics
    pub reason: RateLimitReason,
    /// associatedModel (used forModelLevelRate Limit)
    /// None expressAccountLevelRate Limit,Some(model) express特定ModelRate Limit
    #[allow(dead_code)] // Used for model-level rate limiting
    pub model: Option<String>,
}

/// FailedcountExpiredTime：1Hour（more than thisTime未Failed则Resetcount）
const FAILURE_COUNT_EXPIRY_SECONDS: u64 = 3600;

/// Rate LimitTrace器
pub struct RateLimitTracker {
    limits: DashMap<String, RateLimitInfo>,
    /// continuousFailedcount（for intelligenceExponential Backoff），带Timestampfor automaticExpired
    failure_counts: DashMap<String, (u32, SystemTime)>,
}

impl RateLimitTracker {
    pub fn new() -> Self {
        Self {
            limits: DashMap::new(),
            failure_counts: DashMap::new(),
        }
    }
    
    /// GetAccountRemaining的WaitTime(秒)
    pub fn get_remaining_wait(&self, account_id: &str) -> u64 {
        if let Some(info) = self.limits.get(account_id) {
            let now = SystemTime::now();
            if info.reset_time > now {
                return info.reset_time.duration_since(now).unwrap_or(Duration::from_secs(0)).as_secs();
            }
        }
        0
    }
    
    /// markAccountRequestSuccess，ResetcontinuousFailedcount
    /// 
    /// WhenAccountSuccessCompleteRequestcall this afterMethod，put itFailedcount reset to zero，
    /// Like this next timeFailedwill start from the shortestLock定Time（60秒）Begin。
    pub fn mark_success(&self, account_id: &str) {
        if self.failure_counts.remove(account_id).is_some() {
            tracing::debug!("Account {} RequestSuccess，已ResetFailedcount", account_id);
        }
        // MeanwhileClearRate LimitRecord（If有）
        self.limits.remove(account_id);
    }
    
    /// accurateLock定Accountto designatedTime点
    /// 
    /// UsingAccountQuotain reset_time to be preciseLock定Account,
    /// This is better thanExponential BackoffMore accurate。
    /// 
    /// # Parameter
    /// - `model`: Optional的ModelName,used forModelLevelRate Limit。None expressAccountLevelRate Limit
    pub fn set_lockout_until(&self, account_id: &str, reset_time: SystemTime, reason: RateLimitReason, model: Option<String>) {
        let now = SystemTime::now();
        let retry_sec = reset_time
            .duration_since(now)
            .map(|d| d.as_secs())
            .unwrap_or(60); // IfTimePassed,UsingDefault 60 秒
        
        let info = RateLimitInfo {
            reset_time,
            retry_after_sec: retry_sec,
            detected_at: now,
            reason,
            model: model.clone(),  // 🆕 SupportModelLevelRate Limit
        };
        
        self.limits.insert(account_id.to_string(), info);
        
        if let Some(m) = &model {
            tracing::info!(
                "Account {} 的Model {} AccurateLockArriveQuotaRefreshTime,Remaining {} 秒",
                account_id,
                m,
                retry_sec
            );
        } else {
            tracing::info!(
                "Account {} AccurateLockArriveQuotaRefreshTime,Remaining {} 秒",
                account_id,
                retry_sec
            );
        }
    }
    
    /// Using ISO 8601 Timestring exactLock定Account
    /// 
    /// ParseSimilar "2026-01-08T17:00:00Z" Format的Timestring
    /// 
    /// # Parameter
    /// - `model`: Optional的ModelName,used forModelLevelRate Limit
    pub fn set_lockout_until_iso(&self, account_id: &str, reset_time_str: &str, reason: RateLimitReason, model: Option<String>) -> bool {
        // TryingParse ISO 8601 Format
        match chrono::DateTime::parse_from_rfc3339(reset_time_str) {
            Ok(dt) => {
                let reset_time = SystemTime::UNIX_EPOCH + 
                    std::time::Duration::from_secs(dt.timestamp() as u64);
                self.set_lockout_until(account_id, reset_time, reason, model);
                true
            },
            Err(e) => {
                tracing::warn!(
                    "Unable to parseQuotaRefreshTime '{}': {},将UsingDefaultbackoff strategy",
                    reset_time_str, e
                );
                false
            }
        }
    }
    
    /// 从ErrorResponseParseRate LimitInfo
    /// 
    /// # Arguments
    /// * `account_id` - Account ID
    /// * `status` - HTTP Status码
    /// * `retry_after_header` - Retry-After header Value
    /// * `body` - ErrorResponse body
    pub fn parse_from_error(
        &self,
        account_id: &str,
        status: u16,
        retry_after_header: Option<&str>,
        body: &str,
        model: Option<String>,
    ) -> Option<RateLimitInfo> {
        // Support 429 (Rate Limit) as well as 500/503/529 (Backend fault soft avoidance)
        if status != 429 && status != 500 && status != 503 && status != 529 {
            return None;
        }
        
        // 1. ParseRate LimitreasonType
        let reason = if status == 429 {
            tracing::warn!("Google 429 Error Body: {}", body);
            self.parse_rate_limit_reason(body)
        } else {
            RateLimitReason::ServerError
        };
        
        let mut retry_after_sec = None;
        
        // 2. 从 Retry-After header extract
        if let Some(retry_after) = retry_after_header {
            if let Ok(seconds) = retry_after.parse::<u64>() {
                retry_after_sec = Some(seconds);
            }
        }
        
        // 3. 从ErrorMessageextract (priorityTrying JSON Parse，Try the regex again)
        if retry_after_sec.is_none() {
            retry_after_sec = self.parse_retry_time_from_body(body);
        }
        
        // 4. HandleDefaultValuewith soft avoidance logic（according toRate LimitTypeSetDifferentDefaultValue）
        let retry_sec = match retry_after_sec {
            Some(s) => {
                // SetSafetyBuffer：Minimum 2 秒，Protect against extremely high frequenciesInvalidRetry
                if s < 2 { 2 } else { s }
            },
            None => {
                // GetcontinuousFailedCount，used forExponential Backoff（With automaticExpiredlogic）
                let failure_count = {
                    let now = SystemTime::now();
                    let mut entry = self.failure_counts.entry(account_id.to_string()).or_insert((0, now));
                    // CheckYesNoExceedExpiredTime，IfYes则Resetcount
                    let elapsed = now.duration_since(entry.1).unwrap_or(Duration::from_secs(0)).as_secs();
                    if elapsed > FAILURE_COUNT_EXPIRY_SECONDS {
                        tracing::debug!("Account {} Failedcount hasExpired（{}秒），Reset为 0", account_id, elapsed);
                        *entry = (0, now);
                    }
                    entry.0 += 1;
                    entry.1 = now;
                    entry.0
                };
                
                match reason {
                    RateLimitReason::QuotaExhausted => {
                        // [intelligentRate Limit] According to continuousFailedCountDynamicAdjustmentLock定Time
                        // 第1次: 60s, 第2次: 5min, 第3次: 30min, 第4次+: 2h
                        let lockout = match failure_count {
                            1 => {
                                tracing::warn!("detectedQuota exhausted (QUOTA_EXHAUSTED)，第1次Failed，Lock定 60秒");
                                60
                            },
                            2 => {
                                tracing::warn!("detectedQuota exhausted (QUOTA_EXHAUSTED)，第2consecutive timesFailed，Lock定 5minute");
                                300
                            },
                            3 => {
                                tracing::warn!("detectedQuota exhausted (QUOTA_EXHAUSTED)，第3consecutive timesFailed，Lock定 30minute");
                                1800
                            },
                            _ => {
                                tracing::warn!("detectedQuota exhausted (QUOTA_EXHAUSTED)，第{}consecutive timesFailed，Lock定 2Hour", failure_count);
                                7200
                            }
                        };
                        lockout
                    },
                    RateLimitReason::RateLimitExceeded => {
                        // rateLimit：generallyYesshort-lived，UsingshorterDefaultValue（30秒）
                        tracing::debug!("Detected rateLimit (RATE_LIMIT_EXCEEDED)，UsingDefaultValue 30秒");
                        30
                    },
                    RateLimitReason::ModelCapacityExhausted => {
                        // ModelCapacity exhausted：ServerNone at the momentAvailable GPU Instance
                        // This isTemporarysexual problems，UsingshorterRetryTime（15秒）
                        tracing::warn!("detectedModelInsufficient capacity (MODEL_CAPACITY_EXHAUSTED)，ServerNoneAvailableInstance，15seconds laterRetry");
                        15
                    },
                    RateLimitReason::ServerError => {
                        // ServerError：Execute"soft avoidance"，DefaultLock定 20 秒
                        tracing::warn!("detected 5xx Error ({}), Execute 20s soft avoidance...", status);
                        20
                    },
                    RateLimitReason::Unknown => {
                        // unknown reason：UsingmediumDefaultValue（60秒）
                        tracing::debug!("Unable to parse 429 Rate Limitreason, UsingDefaultValue 60秒");
                        60
                    }
                }
            }
        };
        
        let info = RateLimitInfo {
            reset_time: SystemTime::now() + Duration::from_secs(retry_sec),
            retry_after_sec: retry_sec,
            detected_at: SystemTime::now(),
            reason,
            model,
        };
        
        // storage
        self.limits.insert(account_id.to_string(), info.clone());
        
        tracing::warn!(
            "Account {} [{}] Rate LimitType: {:?}, Resetdelay: {}秒",
            account_id,
            status,
            reason,
            retry_sec
        );
        
        Some(info)
    }
    
    /// ParseRate LimitreasonType
    fn parse_rate_limit_reason(&self, body: &str) -> RateLimitReason {
        // Trying从 JSON extracted from reason Field
        let trimmed = body.trim();
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(trimmed) {
                if let Some(reason_str) = json.get("error")
                    .and_then(|e| e.get("details"))
                    .and_then(|d| d.as_array())
                    .and_then(|a| a.get(0))
                    .and_then(|o| o.get("reason"))
                    .and_then(|v| v.as_str()) {
                    
                    return match reason_str {
                        "QUOTA_EXHAUSTED" => RateLimitReason::QuotaExhausted,
                        "RATE_LIMIT_EXCEEDED" => RateLimitReason::RateLimitExceeded,
                        "MODEL_CAPACITY_EXHAUSTED" => RateLimitReason::ModelCapacityExhausted,
                        _ => RateLimitReason::Unknown,
                    };
                }
                // [NEW] Trying从 message Field进Linetext matching（prevent missed reason）
                 if let Some(msg) = json.get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|v| v.as_str()) {
                    let msg_lower = msg.to_lowercase();
                    if msg_lower.contains("per minute") || msg_lower.contains("rate limit") {
                        return RateLimitReason::RateLimitExceeded;
                    }
                 }
            }
        }
        
        // IfUnable to access from JSON Parse，Trying从Messagetext judgment
        let body_lower = body.to_lowercase();
        // [FIX] Prioritize judgment at minute levelLimit，avoid TPM Misjudged as Quota
        if body_lower.contains("per minute") || body_lower.contains("rate limit") || body_lower.contains("too many requests") {
             RateLimitReason::RateLimitExceeded
        } else if body_lower.contains("exhausted") || body_lower.contains("quota") {
            RateLimitReason::QuotaExhausted
        } else {
            RateLimitReason::Unknown
        }
    }
    
    /// GeneralTimeParseFunction：Support "2h1m1s" 等AllFormatcombination
    fn parse_duration_string(&self, s: &str) -> Option<u64> {
        tracing::debug!("[TimeParse] TryingParse: '{}'", s);
        
        // UsingRegular expression to extract hours、minute、秒、millisecond
        // SupportFormat："2h1m1s", "1h30m", "5m", "30s", "500ms" 等
        let re = Regex::new(r"(?:(\d+)h)?(?:(\d+)m)?(?:(\d+(?:\.\d+)?)s)?(?:(\d+)ms)?").ok()?;
        let caps = match re.captures(s) {
            Some(c) => c,
            None => {
                tracing::warn!("[TimeParse] Regex not matched: '{}'", s);
                return None;
            }
        };
        
        let hours = caps.get(1)
            .and_then(|m| m.as_str().parse::<u64>().ok())
            .unwrap_or(0);
        let minutes = caps.get(2)
            .and_then(|m| m.as_str().parse::<u64>().ok())
            .unwrap_or(0);
        let seconds = caps.get(3)
            .and_then(|m| m.as_str().parse::<f64>().ok())
            .unwrap_or(0.0);
        let milliseconds = caps.get(4)
            .and_then(|m| m.as_str().parse::<u64>().ok())
            .unwrap_or(0);
        
        tracing::debug!("[TimeParse] extractResult: {}h {}m {:.3}s {}ms", hours, minutes, seconds, milliseconds);
        
        // Calculate total seconds
        let total_seconds = hours * 3600 + minutes * 60 + seconds.ceil() as u64 + (milliseconds + 999) / 1000;
        
        // IfThe total number of seconds is 0，DescriptionParseFailed
        if total_seconds == 0 {
            tracing::warn!("[TimeParse] Failed: '{}' (The total number of seconds is0)", s);
            None
        } else {
            tracing::info!("[TimeParse] ✓ Success: '{}' => {}秒 ({}h {}m {:.1}s)", 
                s, total_seconds, hours, minutes, seconds);
            Some(total_seconds)
        }
    }
    
    /// 从ErrorMessage body 中ParseResetTime
    fn parse_retry_time_from_body(&self, body: &str) -> Option<u64> {
        // A. priorityTrying JSON AccurateParse
        let trimmed = body.trim();
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(trimmed) {
                // 1. Google common quotaResetDelay Format (SupportAllFormat："2h1m1s", "1h30m", "42s", "500ms" 等)
                // Path: error.details[0].metadata.quotaResetDelay
                if let Some(delay_str) = json.get("error")
                    .and_then(|e| e.get("details"))
                    .and_then(|d| d.as_array())
                    .and_then(|a| a.get(0))
                    .and_then(|o| o.get("metadata"))  // Add metadata Hierarchy
                    .and_then(|m| m.get("quotaResetDelay"))
                    .and_then(|v| v.as_str()) {
                    
                    tracing::debug!("[JSONParse] turn up quotaResetDelay: '{}'", delay_str);
                    
                    // UsingGeneralTimeParseFunction
                    if let Some(seconds) = self.parse_duration_string(delay_str) {
                        return Some(seconds);
                    }
                }
                
                // 2. OpenAI common retry_after Field (number)
                if let Some(retry) = json.get("error")
                    .and_then(|e| e.get("retry_after"))
                    .and_then(|v| v.as_u64()) {
                    return Some(retry);
                }
            }
        }

        // B. Regular matchMode (reveal all the details)
        // Mode 1: "Try again in 2m 30s"
        if let Ok(re) = Regex::new(r"(?i)try again in (\d+)m\s*(\d+)s") {
            if let Some(caps) = re.captures(body) {
                if let (Ok(m), Ok(s)) = (caps[1].parse::<u64>(), caps[2].parse::<u64>()) {
                    return Some(m * 60 + s);
                }
            }
        }
        
        // Mode 2: "Try again in 30s" 或 "backoff for 42s"
        if let Ok(re) = Regex::new(r"(?i)(?:try again in|backoff for|wait)\s*(\d+)s") {
            if let Some(caps) = re.captures(body) {
                if let Ok(s) = caps[1].parse::<u64>() {
                    return Some(s);
                }
            }
        }
        
        // Mode 3: "quota will reset in X seconds"
        if let Ok(re) = Regex::new(r"(?i)quota will reset in (\d+) second") {
            if let Some(caps) = re.captures(body) {
                if let Ok(s) = caps[1].parse::<u64>() {
                    return Some(s);
                }
            }
        }
        
        // Mode 4: OpenAI Stylish "Retry after (\d+) seconds"
        if let Ok(re) = Regex::new(r"(?i)retry after (\d+) second") {
            if let Some(caps) = re.captures(body) {
                if let Ok(s) = caps[1].parse::<u64>() {
                    return Some(s);
                }
            }
        }

        // Mode 5: bracket form "(wait (\d+)s)"
        if let Ok(re) = Regex::new(r"\(wait (\d+)s\)") {
            if let Some(caps) = re.captures(body) {
                if let Ok(s) = caps[1].parse::<u64>() {
                    return Some(s);
                }
            }
        }
        
        None
    }
    
    /// GetAccount的Rate LimitInfo
    pub fn get(&self, account_id: &str) -> Option<RateLimitInfo> {
        self.limits.get(account_id).map(|r| r.clone())
    }
    
    /// CheckAccountYesNostillRate Limit中
    pub fn is_rate_limited(&self, account_id: &str) -> bool {
        if let Some(info) = self.get(account_id) {
            info.reset_time > SystemTime::now()
        } else {
            false
        }
    }
    
    /// GetdistanceRate LimitResetHow many seconds are left?
    pub fn get_reset_seconds(&self, account_id: &str) -> Option<u64> {
        if let Some(info) = self.get(account_id) {
            info.reset_time
                .duration_since(SystemTime::now())
                .ok()
                .map(|d| d.as_secs())
        } else {
            None
        }
    }
    
    /// ClearExpired的Rate LimitRecord
    #[allow(dead_code)]
    pub fn cleanup_expired(&self) -> usize {
        let now = SystemTime::now();
        let mut count = 0;
        
        self.limits.retain(|_k, v| {
            if v.reset_time <= now {
                count += 1;
                false
            } else {
                true
            }
        });
        
        if count > 0 {
            tracing::debug!("Clear了 {} 个Expired的Rate LimitRecord", count);
        }
        
        count
    }
    
    /// ClearSpecifyAccount的Rate LimitRecord
    #[allow(dead_code)]
    pub fn clear(&self, account_id: &str) -> bool {
        self.limits.remove(account_id).is_some()
    }
    
    /// ClearAllRate LimitRecord (optimismResetStrategy)
    /// 
    /// used for optimismResetmechanism,WhenAllAccountAll wereRate Limit但WaitTimevery short time,
    /// ClearAllRate LimitRecordto resolve timing contentionCondition
    pub fn clear_all(&self) {
        let count = self.limits.len();
        self.limits.clear();
        tracing::warn!("🔄 Optimistic reset: Cleared all {} rate limit record(s)", count);
    }
}

impl Default for RateLimitTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_retry_time_minutes_seconds() {
        let tracker = RateLimitTracker::new();
        let body = "Rate limit exceeded. Try again in 2m 30s";
        let time = tracker.parse_retry_time_from_body(body);
        assert_eq!(time, Some(150)); 
    }
    
    #[test]
    fn test_parse_google_json_delay() {
        let tracker = RateLimitTracker::new();
        let body = r#"{
            "error": {
                "details": [
                    { 
                        "metadata": {
                            "quotaResetDelay": "42s" 
                        }
                    }
                ]
            }
        }"#;
        let time = tracker.parse_retry_time_from_body(body);
        assert_eq!(time, Some(42));
    }

    #[test]
    fn test_parse_retry_after_ignore_case() {
        let tracker = RateLimitTracker::new();
        let body = "Quota limit hit. Retry After 99 Seconds";
        let time = tracker.parse_retry_time_from_body(body);
        assert_eq!(time, Some(99));
    }

    #[test]
    fn test_get_remaining_wait() {
        let tracker = RateLimitTracker::new();
        tracker.parse_from_error("acc1", 429, Some("30"), "", None);
        let wait = tracker.get_remaining_wait("acc1");
        assert!(wait > 25 && wait <= 30);
    }

    #[test]
    fn test_safety_buffer() {
        let tracker = RateLimitTracker::new();
        // If API Return 1s，We force it to be 2s
        tracker.parse_from_error("acc1", 429, Some("1"), "", None);
        let wait = tracker.get_remaining_wait("acc1");
        // Due to time passing, it might be 1 or 2
        assert!(wait >= 1 && wait <= 2);
    }

    #[test]
    fn test_tpm_exhausted_is_rate_limit_exceeded() {
        let tracker = RateLimitTracker::new();
        // simulationTruereal world TPM Error，MeanwhilePacket含 "Resource exhausted" 和 "per minute"
        let body = "Resource has been exhausted (e.g. check quota). Quota limit 'Tokens per minute' exceeded.";
        let reason = tracker.parse_rate_limit_reason(body);
        // Shouldidentified as RateLimitExceeded，Instead ofYes QuotaExhausted
        assert_eq!(reason, RateLimitReason::RateLimitExceeded);
    }
}
