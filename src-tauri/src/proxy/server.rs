use crate::proxy::TokenManager;
use axum::{
    extract::DefaultBodyLimit,
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::{any, get, post},
    Router,
};
use std::sync::Arc;
use tokio::sync::oneshot;
use tower_http::trace::TraceLayer;
use tracing::{debug, error};
use tokio::sync::RwLock;
use std::sync::atomic::AtomicUsize;

/// Axum applicationStatus
#[derive(Clone)]
pub struct AppState {
    pub token_manager: Arc<TokenManager>,
    pub custom_mapping: Arc<tokio::sync::RwLock<std::collections::HashMap<String, String>>>,
    #[allow(dead_code)]
    pub request_timeout: u64, // API RequestTimeout(秒)
    #[allow(dead_code)]
    pub thought_signature_map: Arc<tokio::sync::Mutex<std::collections::HashMap<String, String>>>, // Thought chainSignMapping (ID -> Signature)
    #[allow(dead_code)]
    pub upstream_proxy: Arc<tokio::sync::RwLock<crate::proxy::config::UpstreamProxyConfig>>,
    pub upstream: Arc<crate::proxy::upstream::client::UpstreamClient>,
    pub zai: Arc<RwLock<crate::proxy::ZaiConfig>>,
    pub provider_rr: Arc<AtomicUsize>,
    pub zai_vision_mcp: Arc<crate::proxy::zai_vision_mcp::ZaiVisionMcpState>,
    pub monitor: Arc<crate::proxy::monitor::ProxyMonitor>,
    pub experimental: Arc<RwLock<crate::proxy::config::ExperimentalConfig>>,
}

/// Axum ServerInstance
pub struct AxumServer {
    shutdown_tx: Option<oneshot::Sender<()>>,
    custom_mapping: Arc<tokio::sync::RwLock<std::collections::HashMap<String, String>>>,
    proxy_state: Arc<tokio::sync::RwLock<crate::proxy::config::UpstreamProxyConfig>>,
    security_state: Arc<RwLock<crate::proxy::ProxySecurityConfig>>,
    zai_state: Arc<RwLock<crate::proxy::ZaiConfig>>,
    experimental: Arc<RwLock<crate::proxy::config::ExperimentalConfig>>,
}

impl AxumServer {
    pub async fn update_mapping(&self, config: &crate::proxy::config::ProxyConfig) {
        {
            let mut m = self.custom_mapping.write().await;
            *m = config.custom_mapping.clone();
        }
        tracing::debug!("Model Mapping (Custom) Fully calibratedUpdate");
    }

    /// UpdateProxyConfig
    pub async fn update_proxy(&self, new_config: crate::proxy::config::UpstreamProxyConfig) {
        let mut proxy = self.proxy_state.write().await;
        *proxy = new_config;
        tracing::info!("upstreamProxyConfigAlready hotUpdate");
    }

    pub async fn update_security(&self, config: &crate::proxy::config::ProxyConfig) {
        let mut sec = self.security_state.write().await;
        *sec = crate::proxy::ProxySecurityConfig::from_proxy_config(config);
        tracing::info!("Anti-generationalServiceSafetyConfigAlready hotUpdate");
    }

    pub async fn update_zai(&self, config: &crate::proxy::config::ProxyConfig) {
        let mut zai = self.zai_state.write().await;
        *zai = config.zai.clone();
        tracing::info!("z.ai ConfigAlready hotUpdate");
    }

    pub async fn update_experimental(&self, config: &crate::proxy::config::ProxyConfig) {
        let mut exp = self.experimental.write().await;
        *exp = config.experimental.clone();
        tracing::info!("ExperimentalConfigAlready hotUpdate");
    }
    /// Start Axum Server
    pub async fn start(
        host: String,
        port: u16,
        token_manager: Arc<TokenManager>,
        custom_mapping: std::collections::HashMap<String, String>,
        _request_timeout: u64,
        upstream_proxy: crate::proxy::config::UpstreamProxyConfig,
        security_config: crate::proxy::ProxySecurityConfig,
        zai_config: crate::proxy::ZaiConfig,
        monitor: Arc<crate::proxy::monitor::ProxyMonitor>,
        experimental_config: crate::proxy::config::ExperimentalConfig,

    ) -> Result<(Self, tokio::task::JoinHandle<()>), String> {
        let custom_mapping_state = Arc::new(tokio::sync::RwLock::new(custom_mapping));
	        let proxy_state = Arc::new(tokio::sync::RwLock::new(upstream_proxy.clone()));
	        let security_state = Arc::new(RwLock::new(security_config));
	        let zai_state = Arc::new(RwLock::new(zai_config));
	        let provider_rr = Arc::new(AtomicUsize::new(0));
	        let zai_vision_mcp_state =
	            Arc::new(crate::proxy::zai_vision_mcp::ZaiVisionMcpState::new());
	        let experimental_state = Arc::new(RwLock::new(experimental_config));

	        let state = AppState {
	            token_manager: token_manager.clone(),
	            custom_mapping: custom_mapping_state.clone(),
	            request_timeout: 300, // 5minuteTimeout
            thought_signature_map: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            upstream_proxy: proxy_state.clone(),
            upstream: Arc::new(crate::proxy::upstream::client::UpstreamClient::new(Some(
                upstream_proxy.clone(),
            ))),
            zai: zai_state.clone(),
            provider_rr: provider_rr.clone(),
            zai_vision_mcp: zai_vision_mcp_state,
            monitor: monitor.clone(),
            experimental: experimental_state.clone(),
        };


        // buildRoute - Usingnew architecture handlers！
        use crate::proxy::handlers;
        // buildRoute
        let app = Router::new()
            // OpenAI Protocol
            .route("/v1/models", get(handlers::openai::handle_list_models))
            .route(
                "/v1/chat/completions",
                post(handlers::openai::handle_chat_completions),
            )
            .route(
                "/v1/completions",
                post(handlers::openai::handle_completions),
            )
            .route("/v1/responses", post(handlers::openai::handle_completions)) // Compatible Codex CLI
            .route(
                "/v1/images/generations",
                post(handlers::openai::handle_images_generations),
            ) // Picturegenerate API
            .route(
                "/v1/images/edits",
                post(handlers::openai::handle_images_edits),
            ) // Pictureedit API
            .route(
                "/v1/audio/transcriptions",
                post(handlers::audio::handle_audio_transcription),
            ) // AudioTranscribe API
            // Claude Protocol
            .route("/v1/messages", post(handlers::claude::handle_messages))
            .route(
                "/v1/messages/count_tokens",
                post(handlers::claude::handle_count_tokens),
            )
            .route(
                "/v1/models/claude",
                get(handlers::claude::handle_list_models),
            )
            // z.ai MCP (optional reverse-proxy)
            .route(
                "/mcp/web_search_prime/mcp",
                any(handlers::mcp::handle_web_search_prime),
            )
	            .route(
	                "/mcp/web_reader/mcp",
	                any(handlers::mcp::handle_web_reader),
	            )
	            .route(
	                "/mcp/zai-mcp-server/mcp",
	                any(handlers::mcp::handle_zai_mcp_server),
	            )
	            // Gemini Protocol (Native)
	            .route("/v1beta/models", get(handlers::gemini::handle_list_models))
            // Handle both GET (get info) and POST (generateContent with colon) at the same route
            .route(
                "/v1beta/models/:model",
                get(handlers::gemini::handle_get_model).post(handlers::gemini::handle_generate),
            )
            .route(
                "/v1beta/models/:model/countTokens",
                post(handlers::gemini::handle_count_tokens),
            ) // Specific route priority
            .route("/v1/models/detect", post(handlers::common::handle_detect_model))
            .route("/internal/warmup", post(handlers::warmup::handle_warmup)) // InsideWarm up endpoints
            .route("/v1/api/event_logging/batch", post(silent_ok_handler))
            .route("/v1/api/event_logging", post(silent_ok_handler))
            .route("/healthz", get(health_check_handler))
            .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
            .layer(axum::middleware::from_fn_with_state(state.clone(), crate::proxy::middleware::monitor::monitor_middleware))
            .layer(TraceLayer::new_for_http())
            .layer(axum::middleware::from_fn_with_state(
                security_state.clone(),
                crate::proxy::middleware::auth_middleware,
            ))
            .layer(crate::proxy::middleware::cors_layer())
            .with_state(state);

        // bindingAddress
        let addr = format!("{}:{}", host, port);
        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .map_err(|e| format!("Address {} bindingFailed: {}", addr, e))?;

        tracing::info!("Anti-generationalServerStart在 http://{}", addr);

        // CreateCloseChannel
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();

        let server_instance = Self {
            shutdown_tx: Some(shutdown_tx),
            custom_mapping: custom_mapping_state.clone(),
            proxy_state,
            security_state,
            zai_state,
            experimental: experimental_state.clone(),
        };

        // in newTask中StartServer
        let handle = tokio::spawn(async move {
            use hyper::server::conn::http1;
            use hyper_util::rt::TokioIo;
            use hyper_util::service::TowerToHyperService;

            loop {
                tokio::select! {
                    res = listener.accept() => {
                        match res {
                            Ok((stream, _)) => {
                                let io = TokioIo::new(stream);
                                let service = TowerToHyperService::new(app.clone());

                                tokio::task::spawn(async move {
                                    if let Err(err) = http1::Builder::new()
                                        .serve_connection(io, service)
                                        .with_upgrades() // Support WebSocket (IfafterNeed)
                                        .await
                                    {
                                        debug!("ConnectHandleEndor error: {:?}", err);
                                    }
                                });
                            }
                            Err(e) => {
                                error!("ReceiveConnectFailed: {:?}", e);
                            }
                        }
                    }
                    _ = &mut shutdown_rx => {
                        tracing::info!("Anti-generationalServerStopListen");
                        break;
                    }
                }
            }
        });

        Ok((server_instance, handle))
    }

    /// StopServer
    pub fn stop(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

// ===== API Handler (The old code hasRemove，由 src/proxy/handlers/* take over) =====

/// Health CheckHandler
async fn health_check_handler() -> Response {
    Json(serde_json::json!({
        "status": "ok"
    }))
    .into_response()
}

/// silenceSuccessHandler (Used to intercept telemetryLog等)
async fn silent_ok_handler() -> Response {
    StatusCode::OK.into_response()
}
