use tauri::State;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use crate::proxy::{ProxyConfig, TokenManager};
use tokio::time::Duration;
use crate::proxy::monitor::{ProxyMonitor, ProxyRequestLog, ProxyStats};


/// Anti-generationalServiceStatus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyStatus {
    pub running: bool,
    pub port: u16,
    pub base_url: String,
    pub active_accounts: usize,
}

/// Anti-generationalServiceGlobalStatus
pub struct ProxyServiceState {
    pub instance: Arc<RwLock<Option<ProxyServiceInstance>>>,
    pub monitor: Arc<RwLock<Option<Arc<ProxyMonitor>>>>,
}

/// Anti-generationalServiceInstance
pub struct ProxyServiceInstance {
    pub config: ProxyConfig,
    pub token_manager: Arc<TokenManager>,
    pub axum_server: crate::proxy::AxumServer,
    pub server_handle: tokio::task::JoinHandle<()>,
}

impl ProxyServiceState {
    pub fn new() -> Self {
        Self {
            instance: Arc::new(RwLock::new(None)),
            monitor: Arc::new(RwLock::new(None)),
        }
    }
}

/// StartAnti-generationalService
#[tauri::command]
pub async fn start_proxy_service(
    config: ProxyConfig,
    state: State<'_, ProxyServiceState>,
    app_handle: tauri::AppHandle,
) -> Result<ProxyStatus, String> {
    let mut instance_lock = state.instance.write().await;
    
    // preventDuplicateStart
    if instance_lock.is_some() {
        return Err("ServiceAlready inRun中".to_string());
    }

    // Ensure monitor exists
    {
        let mut monitor_lock = state.monitor.write().await;
        if monitor_lock.is_none() {
            *monitor_lock = Some(Arc::new(ProxyMonitor::new(1000, Some(app_handle.clone()))));
        }
        // Sync enabled state from config
        if let Some(monitor) = monitor_lock.as_ref() {
            monitor.set_enabled(config.enable_logging);
        }
    }
    
    let monitor = state.monitor.read().await.as_ref().unwrap().clone();
    
    // 2. Initialize Token Manager
    let app_data_dir = crate::modules::account::get_data_dir()?;
    // Ensure accounts dir exists even if the user will only use non-Google providers (e.g. z.ai).
    let _ = crate::modules::account::get_accounts_dir()?;
    let accounts_dir = app_data_dir.clone();
    
    let token_manager = Arc::new(TokenManager::new(accounts_dir));
    token_manager.start_auto_cleanup(); // StartRate LimitRecordAutomatically clean the backgroundTask
    // Sync UI Delivery scheduleConfig
    token_manager.update_sticky_config(config.scheduling.clone()).await;
    
    // 3. LoadAccount
    let active_accounts = token_manager.load_accounts().await
        .map_err(|e| format!("LoadAccountFailed: {}", e))?;
    
    if active_accounts == 0 {
        let zai_enabled = config.zai.enabled
            && !matches!(config.zai.dispatch_mode, crate::proxy::ZaiDispatchMode::Off);
        if !zai_enabled {
            return Err("NoneAvailableAccount，please firstAddAccount".to_string());
        }
    }
    
    // Start Axum Server
    let (axum_server, server_handle) =
        match crate::proxy::AxumServer::start(
            config.get_bind_address().to_string(),
            config.port,
            token_manager.clone(),
            config.custom_mapping.clone(),
            config.request_timeout,
            config.upstream_proxy.clone(),
            crate::proxy::ProxySecurityConfig::from_proxy_config(&config),
            config.zai.clone(),
            monitor.clone(),
            config.experimental.clone(),

        ).await {
            Ok((server, handle)) => (server, handle),
            Err(e) => return Err(format!("Start Axum ServerFailed: {}", e)),
        };
    
    // CreateServiceInstance
    let instance = ProxyServiceInstance {
        config: config.clone(),
        token_manager: token_manager.clone(), // Clone for ProxyServiceInstance
        axum_server,
        server_handle,
    };
    
    *instance_lock = Some(instance);
    

    // SaveConfig到Global AppConfig
    let mut app_config = crate::modules::config::load_app_config().map_err(|e| e)?;
    app_config.proxy = config.clone();
    crate::modules::config::save_app_config(&app_config).map_err(|e| e)?;
    
    Ok(ProxyStatus {
        running: true,
        port: config.port,
        base_url: format!("http://127.0.0.1:{}", config.port),
        active_accounts,
    })
}

/// StopAnti-generationalService
#[tauri::command]
pub async fn stop_proxy_service(
    state: State<'_, ProxyServiceState>,
) -> Result<(), String> {
    let mut instance_lock = state.instance.write().await;
    
    if instance_lock.is_none() {
        return Err("Service未Run".to_string());
    }
    
    // Stop Axum Server
    if let Some(instance) = instance_lock.take() {
        instance.axum_server.stop();
        // WaitServerTaskComplete
        instance.server_handle.await.ok();
    }
    
    Ok(())
}

/// GetAnti-generationalServiceStatus
#[tauri::command]
pub async fn get_proxy_status(
    state: State<'_, ProxyServiceState>,
) -> Result<ProxyStatus, String> {
    let instance_lock = state.instance.read().await;
    
    match instance_lock.as_ref() {
        Some(instance) => Ok(ProxyStatus {
            running: true,
            port: instance.config.port,
            base_url: format!("http://127.0.0.1:{}", instance.config.port),
            active_accounts: instance.token_manager.len(),
        }),
        None => Ok(ProxyStatus {
            running: false,
            port: 0,
            base_url: String::new(),
            active_accounts: 0,
        }),
    }
}

/// GetAnti-generationalServicestatistics
#[tauri::command]
pub async fn get_proxy_stats(
    state: State<'_, ProxyServiceState>,
) -> Result<ProxyStats, String> {
    let monitor_lock = state.monitor.read().await;
    if let Some(monitor) = monitor_lock.as_ref() {
        Ok(monitor.get_stats().await)
    } else {
        Ok(ProxyStats::default())
    }
}

/// GetAnti-generationalRequestLog
#[tauri::command]
pub async fn get_proxy_logs(
    state: State<'_, ProxyServiceState>,
    limit: Option<usize>,
) -> Result<Vec<ProxyRequestLog>, String> {
    let monitor_lock = state.monitor.read().await;
    if let Some(monitor) = monitor_lock.as_ref() {
        Ok(monitor.get_logs(limit.unwrap_or(100)).await)
    } else {
        Ok(Vec::new())
    }
}

/// SetMonitoring is onStatus
#[tauri::command]
pub async fn set_proxy_monitor_enabled(
    state: State<'_, ProxyServiceState>,
    enabled: bool,
) -> Result<(), String> {
    let monitor_lock = state.monitor.read().await;
    if let Some(monitor) = monitor_lock.as_ref() {
        monitor.set_enabled(enabled);
    }
    Ok(())
}

/// ClearAnti-generationalRequestLog
#[tauri::command]
pub async fn clear_proxy_logs(
    state: State<'_, ProxyServiceState>,
) -> Result<(), String> {
    let monitor_lock = state.monitor.read().await;
    if let Some(monitor) = monitor_lock.as_ref() {
        monitor.clear().await;
    }
    Ok(())
}

/// GetAnti-generationalRequestLog (Pagination)
#[tauri::command]
pub async fn get_proxy_logs_paginated(
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<ProxyRequestLog>, String> {
    crate::modules::proxy_db::get_logs_summary(
        limit.unwrap_or(20),
        offset.unwrap_or(0)
    )
}

/// GetSingleLogFull details of
#[tauri::command]
pub async fn get_proxy_log_detail(
    log_id: String,
) -> Result<ProxyRequestLog, String> {
    crate::modules::proxy_db::get_log_detail(&log_id)
}

/// GetLogtotal
#[tauri::command]
pub async fn get_proxy_logs_count() -> Result<u64, String> {
    crate::modules::proxy_db::get_logs_count()
}

/// ExportAllLogto designatedFile
#[tauri::command]
pub async fn export_proxy_logs(
    file_path: String,
) -> Result<usize, String> {
    let logs = crate::modules::proxy_db::get_all_logs_for_export()?;
    let count = logs.len();
    
    let json = serde_json::to_string_pretty(&logs)
        .map_err(|e| format!("Failed to serialize logs: {}", e))?;
    
    std::fs::write(&file_path, json)
        .map_err(|e| format!("Failed to write file: {}", e))?;
    
    Ok(count)
}

/// ExportdesignatedLogJSON到File
#[tauri::command]
pub async fn export_proxy_logs_json(
    file_path: String,
    json_data: String,
) -> Result<usize, String> {
    // Parse to count items
    let logs: Vec<serde_json::Value> = serde_json::from_str(&json_data)
        .map_err(|e| format!("Failed to parse JSON: {}", e))?;
    let count = logs.len();
    
    // Pretty print
    let pretty_json = serde_json::to_string_pretty(&logs)
        .map_err(|e| format!("Failed to serialize: {}", e))?;
    
    std::fs::write(&file_path, pretty_json)
        .map_err(|e| format!("Failed to write file: {}", e))?;
    
    Ok(count)
}

/// Get带SearchCondition的LogAmount
#[tauri::command]
pub async fn get_proxy_logs_count_filtered(
    filter: String,
    errors_only: bool,
) -> Result<u64, String> {
    crate::modules::proxy_db::get_logs_count_filtered(&filter, errors_only)
}

/// Get带SearchConditionpagingLog
#[tauri::command]
pub async fn get_proxy_logs_filtered(
    filter: String,
    errors_only: bool,
    limit: usize,
    offset: usize,
) -> Result<Vec<crate::proxy::monitor::ProxyRequestLog>, String> {
    crate::modules::proxy_db::get_logs_filtered(&filter, errors_only, limit, offset)
}

/// generate API Key
#[tauri::command]
pub fn generate_api_key() -> String {
    format!("sk-{}", uuid::Uuid::new_v4().simple())
}

/// againLoadAccount（Whenmain applicationAdd/DeleteAccountCalled when）
#[tauri::command]
pub async fn reload_proxy_accounts(
    state: State<'_, ProxyServiceState>,
) -> Result<usize, String> {
    let instance_lock = state.instance.read().await;
    
    if let Some(instance) = instance_lock.as_ref() {
        // againLoadAccount
        let count = instance.token_manager.load_accounts().await
            .map_err(|e| format!("againLoadAccountFailed: {}", e))?;
        Ok(count)
    } else {
        Err("Service未Run".to_string())
    }
}

/// UpdateModel Mapping表 (热Update)
#[tauri::command]
pub async fn update_model_mapping(
    config: ProxyConfig,
    state: State<'_, ProxyServiceState>,
) -> Result<(), String> {
    let instance_lock = state.instance.read().await;
    
    // 1. IfServiceCurrentlyRun，immediatelyUpdateMemoryinMapping (HereCurrently onlyUpdate了 anthropic_mapping 的 RwLock, 
    // Follow-upCanaccording toNeed让 resolve_model_route directReadfull amount config)
    if let Some(instance) = instance_lock.as_ref() {
        instance.axum_server.update_mapping(&config).await;
        tracing::debug!("rear endService已Receivefull amountModel MappingConfig");
    }
    
    // 2. regardlessYesNoRun，都Save到GlobalConfigPersistent化
    let mut app_config = crate::modules::config::load_app_config().map_err(|e| e)?;
    app_config.proxy.custom_mapping = config.custom_mapping;
    crate::modules::config::save_app_config(&app_config).map_err(|e| e)?;
    
    Ok(())
}

fn join_base_url(base: &str, path: &str) -> String {
    let base = base.trim_end_matches('/');
    let path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{}", path)
    };
    format!("{}{}", base, path)
}

fn extract_model_ids(value: &serde_json::Value) -> Vec<String> {
    let mut out = Vec::new();

    fn push_from_item(out: &mut Vec<String>, item: &serde_json::Value) {
        match item {
            serde_json::Value::String(s) => out.push(s.to_string()),
            serde_json::Value::Object(map) => {
                if let Some(id) = map.get("id").and_then(|v| v.as_str()) {
                    out.push(id.to_string());
                } else if let Some(name) = map.get("name").and_then(|v| v.as_str()) {
                    out.push(name.to_string());
                }
            }
            _ => {}
        }
    }

    match value {
        serde_json::Value::Array(arr) => {
            for item in arr {
                push_from_item(&mut out, item);
            }
        }
        serde_json::Value::Object(map) => {
            if let Some(data) = map.get("data") {
                if let serde_json::Value::Array(arr) = data {
                    for item in arr {
                        push_from_item(&mut out, item);
                    }
                }
            }
            if let Some(models) = map.get("models") {
                match models {
                    serde_json::Value::Array(arr) => {
                        for item in arr {
                            push_from_item(&mut out, item);
                        }
                    }
                    other => push_from_item(&mut out, other),
                }
            }
        }
        _ => {}
    }

    out
}

/// Fetch available models from the configured z.ai Anthropic-compatible API (`/v1/models`).
#[tauri::command]
pub async fn fetch_zai_models(
    zai: crate::proxy::ZaiConfig,
    upstream_proxy: crate::proxy::config::UpstreamProxyConfig,
    request_timeout: u64,
) -> Result<Vec<String>, String> {
    if zai.base_url.trim().is_empty() {
        return Err("z.ai base_url is empty".to_string());
    }
    if zai.api_key.trim().is_empty() {
        return Err("z.ai api_key is not set".to_string());
    }

    let url = join_base_url(&zai.base_url, "/v1/models");

    let mut builder = reqwest::Client::builder().timeout(Duration::from_secs(request_timeout.max(5)));
    if upstream_proxy.enabled && !upstream_proxy.url.is_empty() {
        let proxy = reqwest::Proxy::all(&upstream_proxy.url)
            .map_err(|e| format!("Invalid upstream proxy url: {}", e))?;
        builder = builder.proxy(proxy);
    }
    let client = builder
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", zai.api_key))
        .header("x-api-key", zai.api_key)
        .header("anthropic-version", "2023-06-01")
        .header("accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("Upstream request failed: {}", e))?;

    let status = resp.status();
    let text = resp.text().await.map_err(|e| format!("Failed to read response: {}", e))?;

    if !status.is_success() {
        let preview = if text.len() > 4000 { &text[..4000] } else { &text };
        return Err(format!("Upstream returned {}: {}", status, preview));
    }

    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("Invalid JSON response: {}", e))?;
    let mut models = extract_model_ids(&json);
    models.retain(|s| !s.trim().is_empty());
    models.sort();
    models.dedup();
    Ok(models)
}

/// GetCurrentSchedulingConfig
#[tauri::command]
pub async fn get_proxy_scheduling_config(
    state: State<'_, ProxyServiceState>,
) -> Result<crate::proxy::sticky_config::StickySessionConfig, String> {
    let instance_lock = state.instance.read().await;
    if let Some(instance) = instance_lock.as_ref() {
        Ok(instance.token_manager.get_sticky_config().await)
    } else {
        Ok(crate::proxy::sticky_config::StickySessionConfig::default())
    }
}

/// UpdateSchedulingConfig
#[tauri::command]
pub async fn update_proxy_scheduling_config(
    state: State<'_, ProxyServiceState>,
    config: crate::proxy::sticky_config::StickySessionConfig,
) -> Result<(), String> {
    let instance_lock = state.instance.read().await;
    if let Some(instance) = instance_lock.as_ref() {
        instance.token_manager.update_sticky_config(config).await;
        Ok(())
    } else {
        Err("Service未Run，UnableUpdatereal timeConfig".to_string())
    }
}

/// ClearAllSessionsticky binding
#[tauri::command]
pub async fn clear_proxy_session_bindings(
    state: State<'_, ProxyServiceState>,
) -> Result<(), String> {
    let instance_lock = state.instance.read().await;
    if let Some(instance) = instance_lock.as_ref() {
        instance.token_manager.clear_all_sessions();
        Ok(())
    } else {
        Err("Service未Run".to_string())
    }
}

