// preheatHandler - Insidepreheat API
//
// supply /internal/warmup endpoint，Support：
// - SpecifyAccount（pass email）
// - SpecifyModel（Don't do itMapping，directUsingRawModelName）
// - ReuseProxy的Allinfrastructure（UpstreamClient、TokenManager）

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{info, warn};

use crate::proxy::mappers::gemini::wrapper::wrap_request;
use crate::proxy::server::AppState;

/// preheatRequest体
#[derive(Debug, Deserialize)]
pub struct WarmupRequest {
    /// AccountMail
    pub email: String,
    /// ModelName（RawName，Don't do itMapping）
    pub model: String,
    /// Optional：Provide directly Access Token（used for absence TokenManager inAccount）
    pub access_token: Option<String>,
    /// Optional：Provide directly Project ID
    pub project_id: Option<String>,
}

/// preheatResponse
#[derive(Debug, Serialize)]
pub struct WarmupResponse {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// HandlepreheatRequest
pub async fn handle_warmup(
    State(state): State<AppState>,
    Json(req): Json<WarmupRequest>,
) -> Response {
    info!(
        "[Warmup-API] ========== START: email={}, model={} ==========",
        req.email, req.model
    );

    // ===== step 1: Get Token =====
    let (access_token, project_id) = if let (Some(at), Some(pid)) = (&req.access_token, &req.project_id) {
        (at.clone(), pid.clone())
    } else {
        match state.token_manager.get_token_by_email(&req.email).await {
            Ok((at, pid, _)) => (at, pid),
            Err(e) => {
                warn!(
                    "[Warmup-API] Step 1 FAILED: Token error for {}: {}",
                    req.email, e
                );
                return (
                    StatusCode::BAD_REQUEST,
                    Json(WarmupResponse {
                        success: false,
                        message: format!("Failed to get token for {}", req.email),
                        error: Some(e),
                    }),
                )
                    .into_response();
            }
        }
    };

    // ===== step 2: according toModelTypebuildRequest体 =====
    let is_claude = req.model.to_lowercase().contains("claude");
    let is_image = req.model.to_lowercase().contains("image");

    let body: Value = if is_claude {
        // Claude Model：Using transform_claude_request_in Convert
        let session_id = format!("warmup_{}_{}", 
            chrono::Utc::now().timestamp_millis(),
            &uuid::Uuid::new_v4().to_string()[..8]
        );
        let claude_request = crate::proxy::mappers::claude::models::ClaudeRequest {
            model: req.model.clone(),
            messages: vec![crate::proxy::mappers::claude::models::Message {
                role: "user".to_string(),
                content: crate::proxy::mappers::claude::models::MessageContent::String(
                    "ping".to_string(),
                ),
            }],
            max_tokens: Some(1),
            stream: false,
            system: None,
            temperature: None,
            top_p: None,
            top_k: None,
            tools: None,
            metadata: Some(crate::proxy::mappers::claude::models::Metadata {
                user_id: Some(session_id),
            }),
            thinking: None,
            output_config: None,
            size: None,
            quality: None,
        };

        match crate::proxy::mappers::claude::transform_claude_request_in(
            &claude_request,
            &project_id,
            false,
        ) {
            Ok(transformed) => transformed,
            Err(e) => {
                warn!("[Warmup-API] Step 2 FAILED: Claude transform error: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(WarmupResponse {
                        success: false,
                        message: format!("Transform error: {}", e),
                        error: Some(e),
                    }),
                )
                    .into_response();
            }
        }
    } else {
        // Gemini Model：Using wrap_request
        let session_id = format!("warmup_{}_{}", 
            chrono::Utc::now().timestamp_millis(),
            &uuid::Uuid::new_v4().to_string()[..8]
        );

        let base_request = if is_image {
            json!({
                "model": req.model,
                "contents": [{"role": "user", "parts": [{"text": "Say hi"}]}],
                "generationConfig": {
                    "maxOutputTokens": 10,
                    "temperature": 0,
                    "responseModalities": ["TEXT"]
                },
                "session_id": session_id
            })
        } else {
            json!({
                "model": req.model,
                "contents": [{"role": "user", "parts": [{"text": "Say hi"}]}],
                "generationConfig": {
                    "temperature": 0
                },
                "session_id": session_id
            })
        };

        wrap_request(&base_request, &project_id, &req.model, Some(&session_id))
    };

    // ===== step 3: call UpstreamClient =====
    let model_lower = req.model.to_lowercase();
    let prefer_non_stream = model_lower.contains("flash-lite") || model_lower.contains("2.5-pro");

    let (method, query) = if prefer_non_stream {
        ("generateContent", None)
    } else {
        ("streamGenerateContent", Some("alt=sse"))
    };

    let mut result = state
        .upstream
        .call_v1_internal(method, &access_token, body.clone(), query)
        .await;

    // IfStream式RequestFailed，Trying非Stream式Request
    if result.is_err() && !prefer_non_stream {
        result = state
            .upstream
            .call_v1_internal("generateContent", &access_token, body, None)
            .await;
    }

    // ===== step 4: HandleResponse =====
    match result {
        Ok(response) => {
            let status = response.status();
            let mut response = if status.is_success() {
                info!(
                    "[Warmup-API] ========== SUCCESS: {} / {} ==========",
                    req.email, req.model
                );
                (
                    StatusCode::OK,
                    Json(WarmupResponse {
                        success: true,
                        message: format!("Warmup triggered for {}", req.model),
                        error: None,
                    }),
                )
                    .into_response()
            } else {
                let status_code = status.as_u16();
                let error_text = response.text().await.unwrap_or_default();
                (
                    StatusCode::from_u16(status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                    Json(WarmupResponse {
                        success: false,
                        message: format!("Warmup failed: HTTP {}", status_code),
                        error: Some(error_text),
                    }),
                )
                    .into_response()
            };

            // AddResponse头，let monitorMiddlewarecaptureAccountInfo
            if let Ok(email_val) = axum::http::HeaderValue::from_str(&req.email) {
                response.headers_mut().insert("X-Account-Email", email_val);
            }
            if let Ok(model_val) = axum::http::HeaderValue::from_str(&req.model) {
                response.headers_mut().insert("X-Mapped-Model", model_val);
            }
            
            response
        }
        Err(e) => {
            warn!(
                "[Warmup-API] ========== ERROR: {} / {} - {} ==========",
                req.email, req.model, e
            );
            
            let mut response = (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(WarmupResponse {
                    success: false,
                    message: "Warmup request failed".to_string(),
                    error: Some(e),
                }),
            ).into_response();

            // even thoughFailed也AddResponse头，for monitoring
            if let Ok(email_val) = axum::http::HeaderValue::from_str(&req.email) {
                response.headers_mut().insert("X-Account-Email", email_val);
            }
            if let Ok(model_val) = axum::http::HeaderValue::from_str(&req.model) {
                response.headers_mut().insert("X-Mapped-Model", model_val);
            }
            
            response
        }
    }
}
