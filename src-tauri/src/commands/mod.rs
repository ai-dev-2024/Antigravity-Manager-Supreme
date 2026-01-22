use crate::models::{Account, AppConfig, QuotaData, TokenData};
use crate::modules;
use tauri_plugin_opener::OpenerExt;
use tauri::{Emitter, Manager};

// Export proxy Order
pub mod proxy;
// Export autostart Order
pub mod autostart;
// Export environment Order (Claude/OpenCode integration)
pub mod environment;

/// Column出AllAccount
#[tauri::command]
pub async fn list_accounts() -> Result<Vec<Account>, String> {
    modules::list_accounts()
}

/// AddAccount
#[tauri::command]
pub async fn add_account(
    app: tauri::AppHandle,
    _email: String,
    refresh_token: String,
) -> Result<Account, String> {
    // 1. Using refresh_token Get access_token
    // Notice：HereWe ignore the incoming _email，而YesGo directly Google GetTruereal mailbox
    let token_res = modules::oauth::refresh_access_token(&refresh_token).await?;

    // 2. GetUserInfo
    let user_info = modules::oauth::get_user_info(&token_res.access_token).await?;

    // 3. structure TokenData
    let token = TokenData::new(
        token_res.access_token,
        refresh_token, // continueUsingUserincoming refresh_token
        token_res.expires_in,
        Some(user_info.email.clone()),
        None, // project_id will be inNeed时Get
        None, // session_id
    );

    // 4. UsingTruereal email Add或UpdateAccount
    let account =
        modules::upsert_account(user_info.email.clone(), user_info.get_display_name(), token)?;

    modules::logger::log_info(&format!("AddAccountSuccess: {}", account.email));

    // 5. automatic triggerRefreshQuota
    let mut account = account;
    let _ = internal_refresh_account_quota(&app, &mut account).await;

    // 6. If proxy is running, reload token pool so changes take effect immediately.
    let _ = crate::commands::proxy::reload_proxy_accounts(
        app.state::<crate::commands::proxy::ProxyServiceState>(),
    )
    .await;

    Ok(account)
}

/// DeleteAccount
#[tauri::command]
pub async fn delete_account(app: tauri::AppHandle, account_id: String) -> Result<(), String> {
    modules::logger::log_info(&format!("receiveDeleteAccountRequest: {}", account_id));
    modules::delete_account(&account_id).map_err(|e| {
        modules::logger::log_error(&format!("DeleteAccountFailed: {}", e));
        e
    })?;
    modules::logger::log_info(&format!("AccountDeleteSuccess: {}", account_id));

    // forceSynctray
    crate::modules::tray::update_tray_menus(&app);
    Ok(())
}

/// batchDeleteAccount
#[tauri::command]
pub async fn delete_accounts(
    app: tauri::AppHandle,
    account_ids: Vec<String>,
) -> Result<(), String> {
    modules::logger::log_info(&format!(
        "received batchDeleteRequest，共 {} 个Account",
        account_ids.len()
    ));
    modules::account::delete_accounts(&account_ids).map_err(|e| {
        modules::logger::log_error(&format!("batchDeleteFailed: {}", e));
        e
    })?;

    // forceSynctray
    crate::modules::tray::update_tray_menus(&app);
    Ok(())
}

/// againSortAccountList
/// According to the incomingAccountIDArrayorderUpdateAccount排Column
#[tauri::command]
pub async fn reorder_accounts(account_ids: Vec<String>) -> Result<(), String> {
    modules::logger::log_info(&format!("receiveAccount重SortRequest，共 {} 个Account", account_ids.len()));
    modules::account::reorder_accounts(&account_ids).map_err(|e| {
        modules::logger::log_error(&format!("Account重SortFailed: {}", e));
        e
    })
}

/// switchAccount
#[tauri::command]
pub async fn switch_account(app: tauri::AppHandle, account_id: String) -> Result<(), String> {
    let res = modules::switch_account(&account_id).await;
    if res.is_ok() {
        crate::modules::tray::update_tray_menus(&app);
    }
    res
}

/// GetCurrentAccount
#[tauri::command]
pub async fn get_current_account() -> Result<Option<Account>, String> {
    // println!("🚀 Backend Command: get_current_account called"); // Commented out to reduce noise for frequent calls, relies on frontend log for frequency
    // Actually user WANTS to see it.
    modules::logger::log_info("Backend Command: get_current_account called");

    let account_id = modules::get_current_account_id()?;

    if let Some(id) = account_id {
        // modules::logger::log_info(&format!("   Found current account ID: {}", id));
        modules::load_account(&id).map(Some)
    } else {
        modules::logger::log_info("   No current account set");
        Ok(None)
    }
}

/// InsideAuxiliaryFunction：在Add或ImportAccountAfter automaticRefreshOne-time quota
async fn internal_refresh_account_quota(
    app: &tauri::AppHandle,
    account: &mut Account,
) -> Result<QuotaData, String> {
    modules::logger::log_info(&format!("automatic triggerRefreshQuota: {}", account.email));

    // Using带Retry的Query (Shared logic)
    match modules::account::fetch_quota_with_retry(account).await {
        Ok(quota) => {
            // UpdateAccountQuota
            let _ = modules::update_account_quota(&account.id, quota.clone());
            // UpdatetrayMenu
            crate::modules::tray::update_tray_menus(app);
            Ok(quota)
        }
        Err(e) => {
            modules::logger::log_warn(&format!("automaticRefreshQuotaFailed ({}): {}", account.email, e));
            Err(e.to_string())
        }
    }
}

/// QueryAccountQuota
#[tauri::command]
pub async fn fetch_account_quota(
    app: tauri::AppHandle,
    proxy_state: tauri::State<'_, crate::commands::proxy::ProxyServiceState>,
    account_id: String,
) -> crate::error::AppResult<QuotaData> {
    modules::logger::log_info(&format!("ManualRefreshQuotaRequest: {}", account_id));
    let mut account =
        modules::load_account(&account_id).map_err(crate::error::AppError::Account)?;

    // Using带Retry的Query (Shared logic)
    let quota = modules::account::fetch_quota_with_retry(&mut account).await?;

    // 4. UpdateAccountQuota
    modules::update_account_quota(&account_id, quota.clone())
        .map_err(crate::error::AppError::Account)?;

    crate::modules::tray::update_tray_menus(&app);

    // 5. Sync到Runcounter-generationService（If已Start）
    let instance_lock = proxy_state.instance.read().await;
    if let Some(instance) = instance_lock.as_ref() {
        let _ = instance.token_manager.reload_account(&account_id).await;
    }

    Ok(quota)
}

pub use modules::account::RefreshStats;

/// RefreshAllAccountQuota
#[tauri::command]
pub async fn refresh_all_quotas(
    proxy_state: tauri::State<'_, crate::commands::proxy::ProxyServiceState>,
) -> Result<RefreshStats, String> {
    let stats = modules::account::refresh_all_quotas_logic().await?;

    // Sync到Runcounter-generationService（If已Start）
    let instance_lock = proxy_state.instance.read().await;
    if let Some(instance) = instance_lock.as_ref() {
        let _ = instance.token_manager.reload_all_accounts().await;
    }

    Ok(stats)
}
/// GetDevice fingerprint（Current storage.json + Accountbinding）
#[tauri::command]
pub async fn get_device_profiles(
    account_id: String,
) -> Result<modules::account::DeviceProfiles, String> {
    modules::get_device_profiles(&account_id)
}

/// Bind device fingerprint（capture: collectionCurrent；generate: Generate new fingerprint），并Write storage.json
#[tauri::command]
pub async fn bind_device_profile(
    account_id: String,
    mode: String,
) -> Result<crate::models::DeviceProfile, String> {
    modules::bind_device_profile(&account_id, &mode)
}

/// Preview generates a fingerprint（Not placing the order）
#[tauri::command]
pub async fn preview_generate_profile() -> Result<crate::models::DeviceProfile, String> {
    Ok(crate::modules::device::generate_profile())
}

/// UsingBind the given fingerprint directly
#[tauri::command]
pub async fn bind_device_profile_with_profile(
    account_id: String,
    profile: crate::models::DeviceProfile,
) -> Result<crate::models::DeviceProfile, String> {
    modules::bind_device_profile_with_profile(&account_id, profile, Some("generated".to_string()))
}

/// 将AccountThe bound fingerprint is applied to storage.json
#[tauri::command]
pub async fn apply_device_profile(
    account_id: String,
) -> Result<crate::models::DeviceProfile, String> {
    modules::apply_device_profile(&account_id)
}

/// earliest restored storage.json backup（approximate“Raw”Status）
#[tauri::command]
pub async fn restore_original_device() -> Result<String, String> {
    modules::restore_original_device()
}

/// ColumnTake out fingerprintsVersion
#[tauri::command]
pub async fn list_device_versions(
    account_id: String,
) -> Result<modules::account::DeviceProfiles, String> {
    modules::list_device_versions(&account_id)
}

/// 按VersionRestore fingerprint
#[tauri::command]
pub async fn restore_device_version(
    account_id: String,
    version_id: String,
) -> Result<crate::models::DeviceProfile, String> {
    modules::restore_device_version(&account_id, &version_id)
}

/// Deletehistorical fingerprint（baseline Cannot be deleted）
#[tauri::command]
pub async fn delete_device_version(account_id: String, version_id: String) -> Result<(), String> {
    modules::delete_device_version(&account_id, &version_id)
}

/// OpenDevice storageDirectory
#[tauri::command]
pub async fn open_device_folder(app: tauri::AppHandle) -> Result<(), String> {
    let dir = modules::device::get_storage_dir()?;
    let dir_str = dir
        .to_str()
        .ok_or("Unable to parsestorageDirectoryPathas string")?
        .to_string();
    app.opener()
        .open_path(dir_str, None::<&str>)
        .map_err(|e| format!("OpenDirectoryFailed: {}", e))
}


/// LoadConfig
#[tauri::command]
pub async fn load_config() -> Result<AppConfig, String> {
    modules::load_app_config()
}

/// SaveConfig
#[tauri::command]
pub async fn save_config(
    app: tauri::AppHandle,
    proxy_state: tauri::State<'_, crate::commands::proxy::ProxyServiceState>,
    config: AppConfig,
) -> Result<(), String> {
    modules::save_app_config(&config)?;

    // NotificationtrayConfig已Update
    let _ = app.emit("config://updated", ());

    // 热UpdateCurrentlyRun的Service
    let instance_lock = proxy_state.instance.read().await;
    if let Some(instance) = instance_lock.as_ref() {
        // UpdateModel Mapping
        instance.axum_server.update_mapping(&config.proxy).await;
        // UpdateupstreamProxy
        instance
            .axum_server
            .update_proxy(config.proxy.upstream_proxy.clone())
            .await;
        // Updatesecurity policy (auth)
        instance.axum_server.update_security(&config.proxy).await;
        // Update z.ai Config
        instance.axum_server.update_zai(&config.proxy).await;
        // UpdateExperimentalConfig
        instance.axum_server.update_experimental(&config.proxy).await;
        tracing::debug!("已Sync热UpdateAnti-generationalServiceConfig");
    }

    Ok(())
}

// --- OAuth Order ---

#[tauri::command]
pub async fn start_oauth_login(app_handle: tauri::AppHandle) -> Result<Account, String> {
    modules::logger::log_info("Begin OAuth AuthorizeStream程...");

    // 1. Start OAuth Stream程Get Token
    let token_res = modules::oauth_server::start_oauth_flow(app_handle.clone()).await?;

    // 2. Check refresh_token
    let refresh_token = token_res.refresh_token.ok_or_else(|| {
        "未Get到 Refresh Token。\n\n\
         Mayreason:\n\
         1. 您Before已AuthorizePass this app,Google not againReturn refresh_token\n\n\
         solution:\n\
         1. access https://myaccount.google.com/permissions\n\
         2. Undo 'Antigravity Tools' visitPermission\n\
         3. Re-enterLine OAuth Authorize\n\n\
         OrUsing 'Refresh Token' Tabpage manualAddAccount"
            .to_string()
    })?;

    // 3. GetUserInfo
    let user_info = modules::oauth::get_user_info(&token_res.access_token).await?;
    modules::logger::log_info(&format!("GetUserInfoSuccess: {}", user_info.email));

    // 4. TryingGetprojectID
    let project_id = crate::proxy::project_resolver::fetch_project_id(&token_res.access_token)
        .await
        .ok();

    if let Some(ref pid) = project_id {
        modules::logger::log_info(&format!("GetprojectIDSuccess: {}", pid));
    } else {
        modules::logger::log_warn("failedGetprojectID,Will be lazy laterLoad");
    }

    // 5. structure TokenData
    let token_data = TokenData::new(
        token_res.access_token,
        refresh_token,
        token_res.expires_in,
        Some(user_info.email.clone()),
        project_id,
        None,
    );

    // 6. Add或Update到AccountList
    modules::logger::log_info("CurrentlySaveAccountInfo...");
    let mut account = modules::upsert_account(
        user_info.email.clone(),
        user_info.get_display_name(),
        token_data,
    )?;

    // 7. automatic triggerRefreshQuota
    let _ = internal_refresh_account_quota(&app_handle, &mut account).await;

    // 8. If proxy is running, reload token pool so changes take effect immediately.
    let _ = crate::commands::proxy::reload_proxy_accounts(
        app_handle.state::<crate::commands::proxy::ProxyServiceState>(),
    )
    .await;

    Ok(account)
}

/// Complete OAuth Authorize（Not automaticOpenBrowser）
#[tauri::command]
pub async fn complete_oauth_login(app_handle: tauri::AppHandle) -> Result<Account, String> {
    modules::logger::log_info("Complete OAuth AuthorizeStream程 (manual)...");

    // 1. WaitCallbackand exchange Token（不 open browser）
    let token_res = modules::oauth_server::complete_oauth_flow(app_handle.clone()).await?;

    // 2. Check refresh_token
    let refresh_token = token_res.refresh_token.ok_or_else(|| {
        "未Get到 Refresh Token。\n\n\
         Mayreason:\n\
         1. 您Before已AuthorizePass this app,Google not againReturn refresh_token\n\n\
         solution:\n\
         1. access https://myaccount.google.com/permissions\n\
         2. Undo 'Antigravity Tools' visitPermission\n\
         3. Re-enterLine OAuth Authorize\n\n\
         OrUsing 'Refresh Token' Tabpage manualAddAccount"
            .to_string()
    })?;

    // 3. GetUserInfo
    let user_info = modules::oauth::get_user_info(&token_res.access_token).await?;
    modules::logger::log_info(&format!("GetUserInfoSuccess: {}", user_info.email));

    // 4. TryingGetprojectID
    let project_id = crate::proxy::project_resolver::fetch_project_id(&token_res.access_token)
        .await
        .ok();

    if let Some(ref pid) = project_id {
        modules::logger::log_info(&format!("GetprojectIDSuccess: {}", pid));
    } else {
        modules::logger::log_warn("failedGetprojectID,Will be lazy laterLoad");
    }

    // 5. structure TokenData
    let token_data = TokenData::new(
        token_res.access_token,
        refresh_token,
        token_res.expires_in,
        Some(user_info.email.clone()),
        project_id,
        None,
    );

    // 6. Add或Update到AccountList
    modules::logger::log_info("CurrentlySaveAccountInfo...");
    let mut account = modules::upsert_account(
        user_info.email.clone(),
        user_info.get_display_name(),
        token_data,
    )?;

    // 7. automatic triggerRefreshQuota
    let _ = internal_refresh_account_quota(&app_handle, &mut account).await;

    // 8. If proxy is running, reload token pool so changes take effect immediately.
    let _ = crate::commands::proxy::reload_proxy_accounts(
        app_handle.state::<crate::commands::proxy::ProxyServiceState>(),
    )
    .await;

    Ok(account)
}

/// pregenerated OAuth AuthorizeLink (不OpenBrowser)
#[tauri::command]
pub async fn prepare_oauth_url(app_handle: tauri::AppHandle) -> Result<String, String> {
    crate::modules::oauth_server::prepare_oauth_url(app_handle).await
}

#[tauri::command]
pub async fn cancel_oauth_login() -> Result<(), String> {
    modules::oauth_server::cancel_oauth_flow();
    Ok(())
}

// --- ImportOrder ---

#[tauri::command]
pub async fn import_v1_accounts(app: tauri::AppHandle) -> Result<Vec<Account>, String> {
    let accounts = modules::migration::import_from_v1().await?;

    // 对Import的AccountTryingRefresha wave
    for mut account in accounts.clone() {
        let _ = internal_refresh_account_quota(&app, &mut account).await;
    }

    Ok(accounts)
}

#[tauri::command]
pub async fn import_from_db(app: tauri::AppHandle) -> Result<Account, String> {
    // SyncFunctionPacketPretend to be async
    let mut account = modules::migration::import_from_db().await?;

    // now thatYes从DataLibraryImport（即 IDE CurrentAccount），automatically set it to Manager 的CurrentAccount
    let account_id = account.id.clone();
    modules::account::set_current_account_id(&account_id)?;

    // automatic triggerRefreshQuota
    let _ = internal_refresh_account_quota(&app, &mut account).await;

    // RefreshtrayIconexhibit
    crate::modules::tray::update_tray_menus(&app);

    Ok(account)
}

#[tauri::command]
#[allow(dead_code)]
pub async fn import_custom_db(app: tauri::AppHandle, path: String) -> Result<Account, String> {
    // After calling the refactoredCustomImportFunction
    let mut account = modules::migration::import_from_custom_db_path(path).await?;

    // automatically set toCurrentAccount
    let account_id = account.id.clone();
    modules::account::set_current_account_id(&account_id)?;

    // automatic triggerRefreshQuota
    let _ = internal_refresh_account_quota(&app, &mut account).await;

    // RefreshtrayIconexhibit
    crate::modules::tray::update_tray_menus(&app);

    Ok(account)
}

#[tauri::command]
pub async fn sync_account_from_db(app: tauri::AppHandle) -> Result<Option<Account>, String> {
    // 1. Get DB in Refresh Token
    let db_refresh_token = match modules::migration::get_refresh_token_from_db() {
        Ok(token) => token,
        Err(e) => {
            modules::logger::log_info(&format!("automaticSyncjump over: {}", e));
            return Ok(None);
        }
    };

    // 2. Get Manager CurrentAccount
    let curr_account = modules::account::get_current_account()?;

    // 3. contrast：If Refresh Token Same，DescriptionAccountNo change，No needImport
    if let Some(acc) = curr_account {
        if acc.token.refresh_token == db_refresh_token {
            // Accountunchanged，becauseAlreadyYesPeriod性Task，usCanSelectivityRefreshone timeQuota，OrdirectReturn
            // Hereto save API Stream量，directReturn
            return Ok(None);
        }
        modules::logger::log_info(&format!(
            "detectedAccountswitch ({} -> DB新Account)，CurrentlySync...",
            acc.email
        ));
    } else {
        modules::logger::log_info("new detectedLoginAccount，CurrentlyautomaticSync...");
    }

    // 4. ExecutewholeImport
    let account = import_from_db(app).await?;
    Ok(Some(account))
}

/// SavetextFile (Bypass the front end Scope Limit)
#[tauri::command]
pub async fn save_text_file(path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, content).map_err(|e| format!("WriteFileFailed: {}", e))
}

/// ReadtextFile (Bypass the front end Scope Limit)
#[tauri::command]
pub async fn read_text_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| format!("ReadFileFailed: {}", e))
}

/// clean upLogCache
#[tauri::command]
pub async fn clear_log_cache() -> Result<(), String> {
    modules::logger::clear_logs()
}

/// OpenDataDirectory
#[tauri::command]
pub async fn open_data_folder() -> Result<(), String> {
    let path = modules::account::get_data_dir()?;

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("OpenFolderFailed: {}", e))?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(path)
            .spawn()
            .map_err(|e| format!("OpenFolderFailed: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("OpenFolderFailed: {}", e))?;
    }

    Ok(())
}

/// GetDataDirectoryabsolutePath
#[tauri::command]
pub async fn get_data_dir_path() -> Result<String, String> {
    let path = modules::account::get_data_dir()?;
    Ok(path.to_string_lossy().to_string())
}

/// Show主Window
#[tauri::command]
pub async fn show_main_window(window: tauri::Window) -> Result<(), String> {
    window.show().map_err(|e| e.to_string())
}

/// Get Antigravity 可ExecuteFilePath
#[tauri::command]
pub async fn get_antigravity_path(bypass_config: Option<bool>) -> Result<String, String> {
    // 1. Prioritize fromConfigQuery (UnlessExplicitly request bypass)
    if bypass_config != Some(true) {
        if let Ok(config) = crate::modules::config::load_app_config() {
            if let Some(path) = config.antigravity_executable {
                if std::path::Path::new(&path).exists() {
                    return Ok(path);
                }
            }
        }
    }

    // 2. ExecuteReal-time detection
    match crate::modules::process::get_antigravity_executable_path() {
        Some(path) => Ok(path.to_string_lossy().to_string()),
        None => Err("not found Antigravity InstallPath".to_string()),
    }
}

/// Get Antigravity StartParameter
#[tauri::command]
pub async fn get_antigravity_args() -> Result<Vec<String>, String> {
    match crate::modules::process::get_args_from_running_process() {
        Some(args) => Ok(args),
        None => Err("not foundCurrentlyRun的 Antigravity Process".to_string()),
    }
}

/// DetectionUpdateResponseStruct
pub use crate::modules::update_checker::UpdateInfo;

/// Detection GitHub releases Update
#[tauri::command]
pub async fn check_for_updates() -> Result<UpdateInfo, String> {
    modules::logger::log_info("Received front-end triggeredUpdateCheckRequest");
    crate::modules::update_checker::check_for_updates().await
}

#[tauri::command]
pub async fn should_check_updates() -> Result<bool, String> {
    let settings = crate::modules::update_checker::load_update_settings()?;
    Ok(crate::modules::update_checker::should_check_for_updates(&settings))
}

#[tauri::command]
pub async fn update_last_check_time() -> Result<(), String> {
    crate::modules::update_checker::update_last_check_time()
}


/// GetUpdateSet
#[tauri::command]
pub async fn get_update_settings() -> Result<crate::modules::update_checker::UpdateSettings, String> {
    crate::modules::update_checker::load_update_settings()
}

/// SaveUpdateSet
#[tauri::command]
pub async fn save_update_settings(
    settings: crate::modules::update_checker::UpdateSettings,
) -> Result<(), String> {
    crate::modules::update_checker::save_update_settings(&settings)
}



/// switchAccountanti-generationDisableStatus
#[tauri::command]
pub async fn toggle_proxy_status(
    app: tauri::AppHandle,
    proxy_state: tauri::State<'_, crate::commands::proxy::ProxyServiceState>,
    account_id: String,
    enable: bool,
    reason: Option<String>,
) -> Result<(), String> {
    modules::logger::log_info(&format!(
        "switchAccountAnti-generationalStatus: {} -> {}",
        account_id,
        if enable { "Enable" } else { "Disable" }
    ));

    // 1. ReadAccountFile
    let data_dir = modules::account::get_data_dir()?;
    let account_path = data_dir.join("accounts").join(format!("{}.json", account_id));

    if !account_path.exists() {
        return Err(format!("AccountFileDoes not exist: {}", account_id));
    }

    let content = std::fs::read_to_string(&account_path)
        .map_err(|e| format!("ReadAccountFileFailed: {}", e))?;

    let mut account_json: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("ParseAccountFileFailed: {}", e))?;

    // 2. Update proxy_disabled Field
    if enable {
        // EnableAnti-generational
        account_json["proxy_disabled"] = serde_json::Value::Bool(false);
        account_json["proxy_disabled_reason"] = serde_json::Value::Null;
        account_json["proxy_disabled_at"] = serde_json::Value::Null;
    } else {
        // DisableAnti-generational
        let now = chrono::Utc::now().timestamp();
        account_json["proxy_disabled"] = serde_json::Value::Bool(true);
        account_json["proxy_disabled_at"] = serde_json::Value::Number(now.into());
        account_json["proxy_disabled_reason"] = serde_json::Value::String(
            reason.unwrap_or_else(|| "UserManualDisable".to_string())
        );
    }

    // 3. Saveto disk
    std::fs::write(&account_path, serde_json::to_string_pretty(&account_json).unwrap())
        .map_err(|e| format!("WriteAccountFileFailed: {}", e))?;

    modules::logger::log_info(&format!(
        "AccountAnti-generationalStatus已Update: {} ({})",
        account_id,
        if enable { "已Enable" } else { "已Disable" }
    ));

    // 4. IfAnti-generationalServiceCurrentlyRun,againLoadAccountPool
    let _ = crate::commands::proxy::reload_proxy_accounts(proxy_state).await;

    // 5. UpdatetrayMenu
    crate::modules::tray::update_tray_menus(&app);

    Ok(())
}

/// preheatAllAvailableAccount
#[tauri::command]
pub async fn warm_up_all_accounts() -> Result<String, String> {
    modules::quota::warm_up_all_accounts().await
}

/// Preheat designationAccount
#[tauri::command]
pub async fn warm_up_account(account_id: String) -> Result<String, String> {
    modules::quota::warm_up_account(&account_id).await
}

// ============================================================================
// HTTP API SetOrder
// ============================================================================

/// Get HTTP API Set
#[tauri::command]
pub async fn get_http_api_settings() -> Result<crate::modules::http_api::HttpApiSettings, String> {
    crate::modules::http_api::load_settings()
}

/// Save HTTP API Set
#[tauri::command]
pub async fn save_http_api_settings(
    settings: crate::modules::http_api::HttpApiSettings,
) -> Result<(), String> {
    crate::modules::http_api::save_settings(&settings)
}
