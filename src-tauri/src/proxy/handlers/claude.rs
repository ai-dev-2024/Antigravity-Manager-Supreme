// Claude Protocol handler

use axum::{
    body::Body,
    extract::{Json, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use futures::StreamExt;
use serde_json::{json, Value};
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info};

use crate::proxy::mappers::claude::{
    transform_claude_request_in, transform_response, create_claude_sse_stream, ClaudeRequest,
    filter_invalid_thinking_blocks_with_family, close_tool_loop_for_thinking,
    clean_cache_control_from_messages, merge_consecutive_messages,
    models::{Message, MessageContent},
};
use crate::proxy::server::AppState;
use crate::proxy::mappers::context_manager::ContextManager;
use crate::proxy::mappers::estimation_calibrator::get_calibrator;
use axum::http::HeaderMap;
use std::sync::{atomic::Ordering, Arc};

const MAX_RETRY_ATTEMPTS: usize = 3;

// ===== Model Constants for Background Tasks =====
// These can be adjusted for performance/cost optimization or overridden by custom_mapping
const INTERNAL_BACKGROUND_TASK: &str = "internal-background-task";  // Unified virtual ID for all background tasks

// ===== Layer 3: XML Summary Prompt Template =====
// Borrowed from Practical-Guide-to-Context-Engineering + Claude Code official practice
// This prompt generates a structured 8-section XML summary for context compression
const CONTEXT_SUMMARY_PROMPT: &str = r#"You are a context compression specialist. Your task is to create a structured XML snapshot of the conversation history.

This snapshot will become the Agent's ONLY memory of the past. All key details, plans, errors, and user instructions MUST be preserved.

First, think through the entire history in a private <scratchpad>. Review the user's overall goal, the agent's actions, tool outputs, file modifications, and any unresolved issues. Identify every piece of information critical for future actions.

After reasoning, generate the final <state_snapshot> XML object. Information must be extremely dense. Omit any irrelevant conversational filler.

The structure MUST be as follows:

<state_snapshot>
  <overall_goal>
    <!-- Describe the user's high-level goal in one concise sentence -->
  </overall_goal>
  
  <technical_context>
    <!-- Tech stack: frameworks, languages, toolchain, dependency versions -->
  </technical_context>
  
  <file_system_state>
    <!-- List files that were created, read, modified, or deleted. Note their status -->
  </file_system_state>
  
  <code_changes>
    <!-- Key code snippets (preserve function signatures and important logic) -->
  </code_changes>
  
  <debugging_history>
    <!-- List all errors encountered, with stack traces, and how they were fixed -->
  </debugging_history>
  
  <current_plan>
    <!-- Step-by-step plan. Mark completed steps -->
  </current_plan>
  
  <user_preferences>
    <!-- User's work preferences for this project (test commands, code style, etc.) -->
  </user_preferences>
  
  <key_decisions>
    <!-- Critical architectural decisions and design choices -->
  </key_decisions>
  
  <latest_thinking_signature>
    <!-- [CRITICAL] Preserve the last valid thinking signature -->
    <!-- Format: base64-encoded signature string -->
    <!-- This MUST be copied exactly as-is, no modifications -->
  </latest_thinking_signature>
</state_snapshot>

**IMPORTANT**:
1. Code snippets must be complete, including function signatures and key logic
2. Error messages must be preserved verbatim, including line numbers and stacks
3. File paths must use absolute paths
4. The thinking signature must be copied exactly, no modifications
"#;

// ===== Jitter Configuration (REMOVED) =====
// Jitter was causing connection instability, reverted to fixed delays
// const JITTER_FACTOR: f64 = 0.2;


// ===== Unified backoff strategyModule =====

// [REMOVED] apply_jitter function
// Jitter logic removed to restore stability (v3.3.16 fix)

/// RetryStrategyEnum
#[derive(Debug, Clone)]
enum RetryStrategy {
    /// 不Retry，directReturnError
    NoRetry,
    /// fixedDelay
    FixedDelay(Duration),
    /// linear backoff：base_ms * (attempt + 1)
    LinearBackoff { base_ms: u64 },
    /// Exponential Backoff：base_ms * 2^attempt，upper limit max_ms
    ExponentialBackoff { base_ms: u64, max_ms: u64 },
}

/// according toErrorStatusMaheErrorInfoSureRetryStrategy
fn determine_retry_strategy(
    status_code: u16,
    error_text: &str,
    retried_without_thinking: bool,
) -> RetryStrategy {
    match status_code {
        // 400 Error：Thinking SignFailed
        400 if !retried_without_thinking
            && (error_text.contains("Invalid `signature`")
                || error_text.contains("thinking.signature")
                || error_text.contains("thinking.thinking")) =>
        {
            // fixed 200ms Delay后Retry
            RetryStrategy::FixedDelay(Duration::from_millis(200))
        }

        // 429 Rate LimitError
        429 => {
            // priorityUsingServerReturn的 Retry-After
            if let Some(delay_ms) = crate::proxy::upstream::retry::parse_retry_delay(error_text) {
                let actual_delay = delay_ms.saturating_add(200).min(10_000);
                RetryStrategy::FixedDelay(Duration::from_millis(actual_delay))
            } else {
                // ElseUsinglinear backoff：1s, 2s, 3s
                RetryStrategy::LinearBackoff { base_ms: 1000 }
            }
        }

        // 503 Service unavailable / 529 Serveroverload
        503 | 529 => {
            // Exponential Backoff：1s, 2s, 4s, 8s
            RetryStrategy::ExponentialBackoff {
                base_ms: 1000,
                max_ms: 8000,
            }
        }

        // 500 ServerInternal error
        500 => {
            // linear backoff：500ms, 1s, 1.5s
            RetryStrategy::LinearBackoff { base_ms: 500 }
        }

        // 401/403 Authenticate/PermissionError：可Retry（rotationAccount）
        401 | 403 => RetryStrategy::FixedDelay(Duration::from_millis(100)),

        // otherError：不Retry
        _ => RetryStrategy::NoRetry,
    }
}

/// Executeretreat strategy andReturnYesNoShouldcontinueRetry
async fn apply_retry_strategy(
    strategy: RetryStrategy,
    attempt: usize,
    status_code: u16,
    trace_id: &str,
) -> bool {
    match strategy {
        RetryStrategy::NoRetry => {
            debug!("[{}] Non-retryable error {}, stopping", trace_id, status_code);
            false
        }

        RetryStrategy::FixedDelay(duration) => {
            let base_ms = duration.as_millis() as u64;
            info!(
                "[{}] ⏱️  Retry with fixed delay: status={}, attempt={}/{}, base={}ms",
                trace_id,
                status_code,
                attempt + 1,
                MAX_RETRY_ATTEMPTS,
                base_ms
            );
            sleep(duration).await;
            true
        }

        RetryStrategy::LinearBackoff { base_ms } => {
            let calculated_ms = base_ms * (attempt as u64 + 1);
            info!(
                "[{}] ⏱️  Retry with linear backoff: status={}, attempt={}/{}, base={}ms",
                trace_id,
                status_code,
                attempt + 1,
                MAX_RETRY_ATTEMPTS,
                calculated_ms
            );
            sleep(Duration::from_millis(calculated_ms)).await;
            true
        }

        RetryStrategy::ExponentialBackoff { base_ms, max_ms } => {
            let calculated_ms = (base_ms * 2_u64.pow(attempt as u32)).min(max_ms);
            info!(
                "[{}] ⏱️  Retry with exponential backoff: status={}, attempt={}/{}, base={}ms",
                trace_id,
                status_code,
                attempt + 1,
                MAX_RETRY_ATTEMPTS,
                calculated_ms
            );
            sleep(Duration::from_millis(calculated_ms)).await;
            true
        }
    }
}

/// judgeYesNoShouldrotationAccount
fn should_rotate_account(status_code: u16) -> bool {
    match status_code {
        // TheseErrorYesAccountLevel的，Needrotation
        429 | 401 | 403 | 500 => true,
        // TheseErrorYesServerLevel的，rotationAccountmeaningless
        400 | 503 | 529 => false,
        // otherErrorDefaultNo rotation
        _ => false,
    }
}

// ===== backoff strategyModuleEnd =====

/// Handle Claude messages Request
/// 
/// Handle Chat MessageRequestStream程
pub async fn handle_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    tracing::debug!("handle_messages called. Body JSON len: {}", body.to_string().len());
    
    // generateRandom Trace ID Usertrack
    let trace_id: String = rand::Rng::sample_iter(rand::thread_rng(), &rand::distributions::Alphanumeric)
        .take(6)
        .map(char::from)
        .collect::<String>().to_lowercase();
        
    // Decide whether this request should be handled by z.ai (Anthropic passthrough) or the existing Google flow.
    let zai = state.zai.read().await.clone();
    let zai_enabled = zai.enabled && !matches!(zai.dispatch_mode, crate::proxy::ZaiDispatchMode::Off);
    let google_accounts = state.token_manager.len();

    // [CRITICAL REFACTOR] priorityParseRequest以GetModelInfo(Used for intelligent bottom-up judgment)
    let mut request: crate::proxy::mappers::claude::models::ClaudeRequest = match serde_json::from_value(body) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "type": "error",
                    "error": {
                        "type": "invalid_request_error",
                        "message": format!("Invalid request body: {}", e)
                    }
                }))
            ).into_response();
        }
    };

    // [Issue #703 Fix] Intelligent bottom-up judgment:NeednormalizationModelname used forQuotaProtectCheck
    let normalized_model = crate::proxy::common::model_mapping::normalize_to_standard_id(&request.model)
        .unwrap_or_else(|| request.model.clone());

    let use_zai = if !zai_enabled {
        false
    } else {
        match zai.dispatch_mode {
            crate::proxy::ZaiDispatchMode::Off => false,
            crate::proxy::ZaiDispatchMode::Exclusive => true,
            crate::proxy::ZaiDispatchMode::Fallback => {
                if google_accounts == 0 {
                    // None Google Account,Usingreveal all the details
                    tracing::info!("[{}] No Google accounts available, using fallback provider", trace_id);
                    true
                } else {
                    // [Issue #703 Fix] Intelligent judgment:CheckYesNo有Available的 Google Account
                    let has_available = state.token_manager.has_available_account("claude", &normalized_model).await;
                    if !has_available {
                        tracing::info!(
                            "[{}] All Google accounts unavailable (rate-limited or quota-protected for {}), using fallback provider",
                            trace_id,
                            request.model
                        );
                    }
                    !has_available
                }
            }
            crate::proxy::ZaiDispatchMode::Pooled => {
                // Treat z.ai as exactly one extra slot in the pool.
                // No strict guarantees: it may get 0 requests if selection never hits.
                let total = google_accounts.saturating_add(1).max(1);
                let slot = state.provider_rr.fetch_add(1, Ordering::Relaxed) % total;
                slot == 0
            }
        }
    };

    // [CRITICAL FIX] Clean up beforehandAllMessagein cache_control Field (Issue #744)
    // Must在Sequence化BeforeHandle，to ensure z.ai 和 Google Flow are not affected by historyMessageCachemark interference
    clean_cache_control_from_messages(&mut request.messages);

    // [FIX #813] Mergeconsecutive sameRoleMessage (Consecutive User Messages)
    // This is useful for z.ai (Anthropic directForward) PathcrucialImportant，BecauseRawStructMustconform toProtocol
    merge_consecutive_messages(&mut request.messages);

    // Get model family for signature validation
    let target_family = if use_zai {
        Some("claude")
    } else {
        let mapped_model = crate::proxy::common::model_mapping::map_claude_model_to_gemini(&request.model);
        if mapped_model.contains("gemini") {
            Some("gemini")
        } else {
            Some("claude")
        }
    };

    // [CRITICAL FIX] Filterand fix Thinking BlockSign (Enhanced with family check)
    filter_invalid_thinking_blocks_with_family(&mut request.messages, target_family);

    // [New] Recover from broken tool loops (where signatures were stripped)
    // This prevents "Assistant message must start with thinking" errors by closing the loop with synthetic messages
    if state.experimental.read().await.enable_tool_loop_recovery {
        close_tool_loop_for_thinking(&mut request.messages);
    }

    // ===== [Issue #467 Fix] intercept Claude Code Warmup Request =====
    // Claude Code Will every 10 秒Sendonce warmup Requestto keepConnectwarm up，
    // TheseRequestWill consume a lot ofQuota。detected warmup Requestdirectly afterReturnsimulationResponse。
    if is_warmup_request(&request) {
        tracing::info!(
            "[{}] 🔥 intercept Warmup Request，ReturnsimulationResponse（saveQuota）",
            trace_id
        );
        return create_warmup_response(&request, request.stream);
    }

    if use_zai {
        // againSequenceafter chemical repairRequest体
        let new_body = match serde_json::to_value(&request) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("Failed to serialize fixed request for z.ai: {}", e);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };

        return crate::proxy::providers::zai_anthropic::forward_anthropic_json(
            &state,
            axum::http::Method::POST,
            "/v1/messages",
            &headers,
            new_body,
        )
        .await;
    }
    
    // Google Flow continueUsing request Object
    // (The subsequent code is notNeedagain filter_invalid_thinking_blocks)
    
    // [NEW] GetContextcontrolConfig
    let experimental = state.experimental.read().await;
    let scaling_enabled = experimental.enable_usage_scaling;
    let threshold_l1 = experimental.context_compression_threshold_l1;
    let threshold_l2 = experimental.context_compression_threshold_l2;
    let threshold_l3 = experimental.context_compression_threshold_l3;

    // GetLatest item“Meaningful”的MessageContent（used forLogRecord和Background task detection）
    // Strategy：Reverse traversal，Firstfilter outAllRole为 "user" 的Message，ThenFind the first non- "Warmup" 且NonEmptythe text ofMessage
    // GetLatest item“Meaningful”的MessageContent（used forLogRecord和Background task detection）
    // Strategy：Reverse traversal，Firstfilter outAll和UserrelevantMessage (role="user")
    // Thenextract its textContent，jump over "Warmup" 或SystemDefault reminder
    let meaningful_msg = request.messages.iter().rev()
        .filter(|m| m.role == "user")
        .find_map(|m| {
            let content = match &m.content {
                crate::proxy::mappers::claude::models::MessageContent::String(s) => s.to_string(),
                crate::proxy::mappers::claude::models::MessageContent::Array(arr) => {
                    // forArray，extractAll Text Blockand splice，neglect ToolResult
                    arr.iter()
                        .filter_map(|block| match block {
                            crate::proxy::mappers::claude::models::ContentBlock::Text { text } => Some(text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                }
            };
            
            // Filterrule：
            // 1. neglectEmptyMessage
            // 2. neglect "Warmup" Message
            // 3. neglect <system-reminder> Tab的Message
            if content.trim().is_empty() 
                || content.starts_with("Warmup") 
                || content.contains("<system-reminder>") 
            {
                None 
            } else {
                Some(content)
            }
        });

    // Ifgo throughFilter还YesNot found（For example pureToolcall），then fall back toFinallyone pieceMessage的Rawexhibit
    let latest_msg = meaningful_msg.unwrap_or_else(|| {
        request.messages.last().map(|m| {
            match &m.content {
                crate::proxy::mappers::claude::models::MessageContent::String(s) => s.clone(),
                crate::proxy::mappers::claude::models::MessageContent::Array(_) => "[Complex/Tool Message]".to_string()
            }
        }).unwrap_or_else(|| "[No Messages]".to_string())
    });
    
    
    // INFO Level: concise oneLineDigest
    info!(
        "[{}] Claude Request | Model: {} | Stream: {} | Messages: {} | Tools: {}",
        trace_id,
        request.model,
        request.stream,
        request.messages.len(),
        request.tools.is_some()
    );
    
    // DEBUG Level: DetailedDebugInfo
    debug!("========== [{}] CLAUDE REQUEST DEBUG START ==========", trace_id);
    debug!("[{}] Model: {}", trace_id, request.model);
    debug!("[{}] Stream: {}", trace_id, request.stream);
    debug!("[{}] Max Tokens: {:?}", trace_id, request.max_tokens);
    debug!("[{}] Temperature: {:?}", trace_id, request.temperature);
    debug!("[{}] Message Count: {}", trace_id, request.messages.len());
    debug!("[{}] Has Tools: {}", trace_id, request.tools.is_some());
    debug!("[{}] Has Thinking Config: {}", trace_id, request.thinking.is_some());
    debug!("[{}] Content Preview: {:.100}...", trace_id, latest_msg);
    
    // Outputevery itemMessagedetailsInfo
    for (idx, msg) in request.messages.iter().enumerate() {
        let content_preview = match &msg.content {
            crate::proxy::mappers::claude::models::MessageContent::String(s) => {
                let char_count = s.chars().count();
                if char_count > 200 {
                    // 【repair】Using chars().take() Safe interception，avoid UTF-8 characterEdge界 panic
                    let preview: String = s.chars().take(200).collect();
                    format!("{}... (total {} chars)", preview, char_count)
                } else {
                    s.clone()
                }
            },
            crate::proxy::mappers::claude::models::MessageContent::Array(arr) => {
                format!("[Array with {} blocks]", arr.len())
            }
        };
        debug!("[{}] Message[{}] - Role: {}, Content: {}", 
            trace_id, idx, msg.role, content_preview);
    }
    
    debug!("[{}] Full Claude Request JSON: {}", trace_id, serde_json::to_string_pretty(&request).unwrap_or_default());
    debug!("========== [{}] CLAUDE REQUEST DEBUG END ==========", trace_id);

    // 1. Get Session ID (Deprecated content-based hash，Use instead TokenManager Inside的Time window locking)
    let _session_id: Option<&str> = None;

    // 2. Get UpstreamClient
    let upstream = state.upstream.clone();
    
    // 3. ready to closePacket
    let mut request_for_body = request.clone();
    let token_manager = state.token_manager;
    
    let pool_size = token_manager.len();
    // [FIX] Ensure max_attempts is at least 2 to allow for internal retries (e.g. stripping signatures)
    // even if the user has only 1 account.
    let max_attempts = MAX_RETRY_ATTEMPTS.min(pool_size.saturating_add(1)).max(2);

    let mut last_error = String::new();
    let retried_without_thinking = false;
    let mut last_email: Option<String> = None;
    
    for attempt in 0..max_attempts {
        // 2. ModelRouteParse
        let mut mapped_model = crate::proxy::common::model_mapping::resolve_model_route(
            &request_for_body.model,
            &*state.custom_mapping.read().await,
        );
        
        // 将 Claude Toolconvert to Value Arrayto detect networking
        let tools_val: Option<Vec<Value>> = request_for_body.tools.as_ref().map(|list| {
            list.iter().map(|t| serde_json::to_value(t).unwrap_or(json!({}))).collect()
        });

        let config = crate::proxy::mappers::common_utils::resolve_request_config(
            &request_for_body.model, 
            &mapped_model, 
            &tools_val,
            request.size.as_deref(),      // [NEW] Pass size parameter
            request.quality.as_deref()    // [NEW] Pass quality parameter
        );

        // 0. Tryingextract session_id for sticky scheduling (Phase 2/3)
        // Using SessionManager generateStable的Sessionfingerprint
        let session_id_str = crate::proxy::session_manager::SessionManager::extract_session_id(&request_for_body);
        let session_id = Some(session_id_str.as_str());

        let force_rotate_token = attempt > 0;
        let (access_token, project_id, email) = match token_manager.get_token(&config.request_type, force_rotate_token, session_id, &config.final_model).await {
            Ok(t) => t,
            Err(e) => {
                let safe_message = if e.contains("invalid_grant") {
                    "OAuth refresh failed (invalid_grant): refresh_token likely revoked/expired; reauthorize account(s) to restore service.".to_string()
                } else {
                    e
                };
                 return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(json!({
                        "type": "error",
                        "error": {
                            "type": "overloaded_error",
                            "message": format!("No available accounts: {}", safe_message)
                        }
                    }))
                ).into_response();
            }
        };

        last_email = Some(email.clone());
        info!("✓ Using account: {} (type: {})", email, config.request_type);
        
        
        // ===== 【Optimization】BackstageTaskIntelligent detection andFallback =====
        // UsingNewDetectionSystem，Support 5 Major categoriesKeywords and many Flash ModelStrategy
        let background_task_type = detect_background_task_type(&request_for_body);
        
        // transferMappinglaterModel名
        let mut request_with_mapped = request_for_body.clone();

        if let Some(task_type) = background_task_type {
            // Background detectedTask,forceFallback到 Flash Model
            let virtual_model_id = select_background_model(task_type);
            
            // [FIX] Mustaccording toVirtual ID Re-resolve Route，以SupportUserCustomMapping (如 internal-task -> gemini-3)
            // Elsewill be directUsing generic ID As a result, the downstream cannot recognize or can onlyUsingStaticDefaultValue
            let resolved_model = crate::proxy::common::model_mapping::resolve_model_route(
                virtual_model_id, 
                &*state.custom_mapping.read().await
            );

            info!(
                "[{}][AUTO] Background detectedTask (Type: {:?}), RouteRedirect: {} -> {} (ultimate physicsModel: {})",
                trace_id,
                task_type,
                mapped_model,
                virtual_model_id,
                resolved_model
            );
            
            // coverUserCustomMapping (MeanwhileUpdateVariable和 Request Object)
            mapped_model = resolved_model.clone();
            request_with_mapped.model = resolved_model;
            
            // BackstageTaskpurify：
            // 1. RemoveTooldefinition（BackstageTask不NeedTool）
            request_with_mapped.tools = None;
            
            // 2. Remove Thinking Config（Flash ModelNot supported）
            request_with_mapped.thinking = None;
            
            // 3. clean historyMessagein Thinking Block，prevent Invalid Argument
            // Using ContextManager unified strategy (Aggressive)
            crate::proxy::mappers::context_manager::ContextManager::purify_history(
                &mut request_with_mapped.messages, 
                crate::proxy::mappers::context_manager::PurificationStrategy::Aggressive
            );
        }

        // ===== [3-Layer Progressive Compression + Calibrated Estimation] Context Management =====
        // [ENHANCED] Integrate 3.3.47 The third floorCompressFramework + PR #925 的Dynamiccalibration mechanism
        // Layer 1 (60%): Tool message trimming - Does NOT break cache
        // Layer 2 (75%): Thinking purification - Breaks cache but preserves signatures
        // Layer 3 (90%): Fork conversation + XML summary - Ultimate optimization
        let mut is_purified = false;
        let mut compression_applied = false;
        
        if !retried_without_thinking {
            // 1. Determine context limit (Flash: ~1M, Pro: ~2M)
            let context_limit = if mapped_model.contains("flash") {
                1_000_000
            } else {
                2_000_000
            };

            // 2. [ENHANCED] UsingCalibrator improves estimation accuracy (PR #925)
            let raw_estimated = ContextManager::estimate_token_usage(&request_with_mapped);
            let calibrator = get_calibrator();
            let mut estimated_usage = calibrator.calibrate(raw_estimated);
            let mut usage_ratio = estimated_usage as f32 / context_limit as f32;
            
            info!(
                "[{}] [ContextManager] Context pressure: {:.1}% (raw: {}, calibrated: {} / {}), Calibration factor: {:.2}",
                trace_id, usage_ratio * 100.0, raw_estimated, estimated_usage, context_limit, calibrator.get_factor()
            );

            // ===== Layer 1: Tool Message Trimming (L1 threshold) =====
            // Borrowed from Practical-Guide-to-Context-Engineering
            // Advantage: Completely cache-friendly (only removes messages, doesn't modify content)
            if usage_ratio > threshold_l1 && !compression_applied {
                if ContextManager::trim_tool_messages(&mut request_with_mapped.messages, 5) {
                    info!(
                        "[{}] [Layer-1] Tool trimming triggered (usage: {:.1}%, threshold: {:.1}%)",
                        trace_id, usage_ratio * 100.0, threshold_l1 * 100.0
                    );
                    compression_applied = true;
                    
                    // Re-estimate after trimming (with calibration)
                    let new_raw = ContextManager::estimate_token_usage(&request_with_mapped);
                    let new_usage = calibrator.calibrate(new_raw);
                    let new_ratio = new_usage as f32 / context_limit as f32;
                    
                    info!(
                        "[{}] [Layer-1] Compression result: {:.1}% → {:.1}% (saved {} tokens)",
                        trace_id, usage_ratio * 100.0, new_ratio * 100.0, estimated_usage - new_usage
                    );
                    
                    // If compression is sufficient, skip further layers
                    if new_ratio < 0.7 {
                        estimated_usage = new_usage;
                        usage_ratio = new_ratio;
                        // Success, no need for Layer 2
                    } else {
                        // Still high pressure, update for Layer 2
                        usage_ratio = new_ratio;
                        compression_applied = false; // Allow Layer 2 to run
                    }
                }
            }

            // ===== Layer 2: Thinking Content Compression (L2 threshold) =====
            // NEW: Preserve signatures while compressing thinking text
            // This prevents signature chain breakage (Issue #902)
            if usage_ratio > threshold_l2 && !compression_applied {
                info!(
                    "[{}] [Layer-2] Thinking compression triggered (usage: {:.1}%, threshold: {:.1}%)",
                    trace_id, usage_ratio * 100.0, threshold_l2 * 100.0
                );
                
                // Use new signature-preserving compression
                if ContextManager::compress_thinking_preserve_signature(
                    &mut request_with_mapped.messages, 
                    4 // Protect last 4 messages (~2 turns)
                ) {
                    is_purified = true; // Still breaks cache, but preserves signatures
                    compression_applied = true;
                    
                    let new_raw = ContextManager::estimate_token_usage(&request_with_mapped);
                    let new_usage = calibrator.calibrate(new_raw);
                    let new_ratio = new_usage as f32 / context_limit as f32;
                    
                    info!(
                        "[{}] [Layer-2] Compression result: {:.1}% → {:.1}% (saved {} tokens)",
                        trace_id, usage_ratio * 100.0, new_ratio * 100.0, estimated_usage - new_usage
                    );
                    
                    usage_ratio = new_ratio;
                }
            }

            // ===== Layer 3: Fork Conversation + XML Summary (L3 threshold) =====
            // Ultimate optimization: Generate structured summary and start fresh conversation
            // Advantage: Completely cache-friendly (append-only), extreme compression ratio
            if usage_ratio > threshold_l3 && !compression_applied {
                info!(
                    "[{}] [Layer-3] Context pressure ({:.1}%) exceeded threshold ({:.1}%), attempting Fork+Summary",
                    trace_id, usage_ratio * 100.0, threshold_l3 * 100.0
                );
                
                // Clone token_manager Arc to avoid borrow issues
                let token_manager_clone = token_manager.clone();
                
                match try_compress_with_summary(&request_with_mapped, &trace_id, &token_manager_clone).await {
                    Ok(forked_request) => {
                        info!(
                            "[{}] [Layer-3] Fork successful: {} → {} messages",
                            trace_id,
                            request_with_mapped.messages.len(),
                            forked_request.messages.len()
                        );
                        
                        request_with_mapped = forked_request;
                        is_purified = false; // Fork doesn't break cache!
                        
                        // Re-estimate after fork (with calibration)
                        let new_raw = ContextManager::estimate_token_usage(&request_with_mapped);
                        let new_usage = calibrator.calibrate(new_raw);
                        let new_ratio = new_usage as f32 / context_limit as f32;
                        
                        info!(
                            "[{}] [Layer-3] Compression result: {:.1}% → {:.1}% (saved {} tokens)",
                            trace_id, usage_ratio * 100.0, new_ratio * 100.0, estimated_usage - new_usage
                        );
                    }
                    Err(e) => {
                        error!(
                            "[{}] [Layer-3] Fork+Summary failed: {}, falling back to error response",
                            trace_id, e
                        );
                        
                        // Return friendly error to user
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(json!({
                                "type": "error",
                                "error": {
                                    "type": "invalid_request_error",
                                    "message": format!("Context too long and automatic compression failed: {}", e),
                                    "suggestion": "Please use /compact or /clear command in Claude Code, or switch to a model with larger context window."
                                }
                            }))
                        ).into_response();
                    }
                }
            }
        }

        // [FIX] Estimate AFTER purification to get accurate token count for calibrator learning
        // Only estimate for calibrator when content was not purified, to avoid skewed learning
        let raw_estimated = if !is_purified {
            ContextManager::estimate_token_usage(&request_with_mapped)
        } else {
            0 // Don't record calibration data when content was purified
        };

        request_with_mapped.model = mapped_model;

        // generate Trace ID (Simple用TimestampSuffix)
        // let _trace_id = format!("req_{}", chrono::Utc::now().timestamp_subsec_millis());

        let gemini_body = match transform_claude_request_in(&request_with_mapped, &project_id, retried_without_thinking) {
            Ok(b) => {
                debug!("[{}] Transformed Gemini Body: {}", trace_id, serde_json::to_string_pretty(&b).unwrap_or_default());
                b
            },
            Err(e) => {
                 return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "type": "error",
                        "error": {
                            "type": "api_error",
                            "message": format!("Transform error: {}", e)
                        }
                    }))
                ).into_response();
            }
        };
        
    // 4. upstream call - automaticConvertlogic
    let client_wants_stream = request.stream;
    // [AUTO-CONVERSION] 非 Stream RequestautomaticConvert为 Stream to enjoy a more relaxedQuota
    let force_stream_internally = !client_wants_stream;
    let actual_stream = client_wants_stream || force_stream_internally;
    
    if force_stream_internally {
        info!("[{}] 🔄 Auto-converting non-stream request to stream for better quota", trace_id);
    }
    
    let method = if actual_stream { "streamGenerateContent" } else { "generateContent" };
    let query = if actual_stream { Some("alt=sse") } else { None };
        // [FIX #765] Prepare Beta Headers for Thinking + Tools
        let mut extra_headers = std::collections::HashMap::new();
        if request_with_mapped.thinking.is_some() && request_with_mapped.tools.is_some() {
            extra_headers.insert("anthropic-beta".to_string(), "interleaved-thinking-2025-05-14".to_string());
            tracing::debug!("[{}] Added Beta Header: interleaved-thinking-2025-05-14", trace_id);
        }

        // 5. upstream call
        let response = match upstream
            .call_v1_internal_with_headers(method, &access_token, gemini_body, query, extra_headers.clone())
            .await {
            Ok(r) => r,
            Err(e) => {
                last_error = e.clone();
                debug!("Request failed on attempt {}/{}: {}", attempt + 1, max_attempts, e);
                continue;
            }
        };
        
        let status = response.status();
        
        // Success
        if status.is_success() {
            // [intelligentRate Limit] RequestSuccess，Reset该AccountContinuousFailedcount
            token_manager.mark_account_success(&email);
            
                // Determine context limit based on model
                let context_limit = crate::proxy::mappers::claude::utils::get_context_limit_for_model(&request_with_mapped.model);

            // HandleStreaming response
            if actual_stream {
                let stream = response.bytes_stream();
                let gemini_stream = Box::pin(stream);


                // [FIX #530/#529/#859] Enhanced Peek logic to handle heartbeats and slow start
                // We must pre-read until we find a MEANINGFUL content block (like message_start).
                // If we only get heartbeats (ping) and then the stream dies, we should rotate account.
                let mut claude_stream = create_claude_sse_stream(
                    gemini_stream,
                    trace_id.clone(),
                    email.clone(),
                    Some(session_id_str.clone()),
                    scaling_enabled,
                    context_limit,
                    Some(raw_estimated) // [FIX] Pass estimated tokens for calibrator learning
                );

                let mut first_data_chunk = None;
                let mut retry_this_account = false;

                // Loop to skip heartbeats during peek
                loop {
                    match tokio::time::timeout(std::time::Duration::from_secs(60), claude_stream.next()).await {
                        Ok(Some(Ok(bytes))) => {
                            if bytes.is_empty() {
                                continue;
                            }
                            
                            let text = String::from_utf8_lossy(&bytes);
                            // Skip SSE comments/pings
                            if text.trim().starts_with(":") {
                                debug!("[{}] Skipping peek heartbeat: {}", trace_id, text.trim());
                                continue;
                            }

                            // We found real data!
                            first_data_chunk = Some(bytes);
                            break;
                        }
                        Ok(Some(Err(e))) => {
                            tracing::warn!("[{}] Stream error during peek: {}, retrying...", trace_id, e);
                            last_error = format!("Stream error during peek: {}", e);
                            retry_this_account = true;
                            break;
                        }
                        Ok(None) => {
                            tracing::warn!("[{}] Stream ended during peek (Empty Response), retrying...", trace_id);
                            last_error = "Empty response stream during peek".to_string();
                            retry_this_account = true;
                            break;
                        }
                        Err(_) => {
                            tracing::warn!("[{}] Timeout waiting for first data (60s), retrying...", trace_id);
                            last_error = "Timeout waiting for first data".to_string();
                            retry_this_account = true;
                            break;
                        }
                    }
                }

                if retry_this_account {
                    continue;
                }

                match first_data_chunk {
                    Some(bytes) => {
                        // We have data! Construct the combined stream
                        let stream_rest = claude_stream;
                        let combined_stream = Box::pin(futures::stream::once(async move { Ok(bytes) })
                            .chain(stream_rest.map(|result| -> Result<Bytes, std::io::Error> {
                                match result {
                                    Ok(b) => Ok(b),
                                    Err(e) => Ok(Bytes::from(format!("data: {{\"error\":\"{}\"}}\n\n", e))),
                                }
                            })));

                        // judgeClientexpectedFormat
                        if client_wants_stream {
                            // ClientI want it Stream，directReturn SSE
                            return Response::builder()
                                .status(StatusCode::OK)
                                .header(header::CONTENT_TYPE, "text/event-stream")
                                .header(header::CACHE_CONTROL, "no-cache")
                                .header(header::CONNECTION, "keep-alive")
                                .header("X-Accel-Buffering", "no")
                                .header("X-Account-Email", &email)
                                .header("X-Mapped-Model", &request_with_mapped.model)
                                .header("X-Context-Purified", if is_purified { "true" } else { "false" })
                                .body(Body::from_stream(combined_stream))
                                .unwrap();
                        } else {
                            // ClientOtherwise Stream，NeedComplete collectionResponse并Convert为 JSON
                            use crate::proxy::mappers::claude::collect_stream_to_json;
                            
                            match collect_stream_to_json(combined_stream).await {
                                Ok(full_response) => {
                                    info!("[{}] ✓ Stream collected and converted to JSON", trace_id);
                                    return Response::builder()
                                        .status(StatusCode::OK)
                                        .header(header::CONTENT_TYPE, "application/json")
                                        .header("X-Account-Email", &email)
                                        .header("X-Mapped-Model", &request_with_mapped.model)
                                        .header("X-Context-Purified", if is_purified { "true" } else { "false" })
                                        .body(Body::from(serde_json::to_string(&full_response).unwrap()))
                                        .unwrap();
                                }
                                Err(e) => {
                                    return (StatusCode::INTERNAL_SERVER_ERROR, format!("Stream collection error: {}", e)).into_response();
                                }
                            }
                        }
                    },

                    None => {
                        tracing::warn!("[{}] Stream ended immediately (Empty Response), retrying...", trace_id);
                        last_error = "Empty response stream (None)".to_string();
                        continue;
                    }
                }
            } else {
                // HandleNon-streaming response
                let bytes = match response.bytes().await {
                    Ok(b) => b,
                    Err(e) => return (StatusCode::BAD_GATEWAY, format!("Failed to read body: {}", e)).into_response(),
                };
                
                // Debug print
                if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                    debug!("Upstream Response for Claude request: {}", text);
                }

                let gemini_resp: Value = match serde_json::from_slice(&bytes) {
                    Ok(v) => v,
                    Err(e) => return (StatusCode::BAD_GATEWAY, format!("Parse error: {}", e)).into_response(),
                };

                // 解Packet response Field（v1internal Format）
                let raw = gemini_resp.get("response").unwrap_or(&gemini_resp);

                // Convert为 Gemini Response Struct
                let gemini_response: crate::proxy::mappers::claude::models::GeminiResponse = match serde_json::from_value(raw.clone()) {
                    Ok(r) => r,
                    Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Convert error: {}", e)).into_response(),
                };
                
                // Determine context limit based on model
                let context_limit = crate::proxy::mappers::claude::utils::get_context_limit_for_model(&request_with_mapped.model);

                // Convert
                // [FIX #765] Pass session_id and model_name for signature caching
                let s_id_owned = session_id.map(|s| s.to_string());
                let claude_response = match transform_response(&gemini_response, scaling_enabled, context_limit, s_id_owned, request_with_mapped.model.clone()) {
                    Ok(r) => r,
                    Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Transform error: {}", e)).into_response(),
                };

                // [Optimization] Recordclosed loopLog：Consumption
                let cache_info = if let Some(cached) = claude_response.usage.cache_read_input_tokens {
                    format!(", Cached: {}", cached)
                } else {
                    String::new()
                };
                
                tracing::info!(
                    "[{}] Request finished. Model: {}, Tokens: In {}, Out {}{}", 
                    trace_id, 
                    request_with_mapped.model, 
                    claude_response.usage.input_tokens, 
                    claude_response.usage.output_tokens,
                    cache_info
                );

                return (StatusCode::OK, [("X-Account-Email", email.as_str()), ("X-Mapped-Model", request_with_mapped.model.as_str())], Json(claude_response)).into_response();
            }
        }
        
        // 1. Extract nowStatusMahe headers（prevent response 被 move）
        let status_code = status.as_u16();
        let retry_after = response.headers().get("Retry-After").and_then(|h| h.to_str().ok()).map(|s| s.to_string());
        
        // 2. GetErrortext and transfer Response All权
        let error_text = response.text().await.unwrap_or_else(|_| format!("HTTP {}", status));
        last_error = format!("HTTP {}: {}", status_code, error_text);
        debug!("[{}] Upstream Error Response: {}", trace_id, error_text);
        
        // 3. markRate LimitStatus(used for UI Show) - UsingAsyncVersion以Supportreal timeQuotaRefresh
        // 🆕 Pass in actualUsing的Model,accomplishModelLevelRate Limit,avoidDifferentModelQuotainfluence each other
        if status_code == 429 || status_code == 529 || status_code == 503 || status_code == 500 {
            token_manager.mark_rate_limited_async(&email, status_code, retry_after.as_deref(), &error_text, Some(&request_with_mapped.model)).await;
        }

        // 4. Handle 400 Error (Thinking SignInvalid 或 BlockorderError)
        if status_code == 400
            && !retried_without_thinking
            && (error_text.contains("Invalid `signature`")
                || error_text.contains("thinking.signature: Field required")
                || error_text.contains("thinking.thinking: Field required")
                || error_text.contains("thinking.signature")
                || error_text.contains("thinking.thinking")
                || error_text.contains("INVALID_ARGUMENT")
                || error_text.contains("Corrupted thought signature")
                || error_text.contains("failed to deserialise")
                || error_text.contains("Invalid signature")
                || error_text.contains("thinking block")
                || error_text.contains("Found `text`")
                || error_text.contains("Found 'text'")
                || error_text.contains("must be `thinking`")
                || error_text.contains("must be 'thinking'")
                )
        {
            // Existing logic for thinking signature...\n            retried_without_thinking = true;
            
            // Using WARN Level,BecauseThis is notShouldoften happens(AlreadyinitiativeFilter过)
            tracing::warn!(
                "[{}] Unexpected thinking signature error (should have been filtered). \
                 Retrying with all thinking blocks removed.",
                trace_id
            );

            // [NEW] Additional fixesHintWord arrivesFinallyone pieceUserMessage
            if let Some(last_msg) = request_for_body.messages.last_mut() {
                if last_msg.role == "user" {
                    let repair_prompt = "\n\n[System Recovery] Your previous output contained an invalid signature. Please regenerate the response without the corrupted signature block.";
                    
                    match &mut last_msg.content {
                        crate::proxy::mappers::claude::models::MessageContent::String(s) => {
                            s.push_str(repair_prompt);
                        }
                        crate::proxy::mappers::claude::models::MessageContent::Array(blocks) => {
                            blocks.push(crate::proxy::mappers::claude::models::ContentBlock::Text {
                                text: repair_prompt.to_string(),
                            });
                        }
                    }
                    tracing::debug!("[{}] Appended repair prompt to last user message", trace_id);
                }
            }

            // [IMPROVED] no longerDisable Thinking Mode！
            // Since weAlreadywill history Thinking Block Convert为 Text，SoCurrentRequestCanregarded as oneNew Thinking Session
            // Keep thinking Configturn on，让Modelregenerate thinking，avoid degenerating intoSimple的 "OK" reply
            // request_for_body.thinking = None;
            
            // clean historyMessageinAll Thinking Block，put itConvert为 Text to reserveContext
            for msg in request_for_body.messages.iter_mut() {
                if let crate::proxy::mappers::claude::models::MessageContent::Array(blocks) = &mut msg.content {
                    let mut new_blocks = Vec::with_capacity(blocks.len());
                    for block in blocks.drain(..) {
                        match block {
                            crate::proxy::mappers::claude::models::ContentBlock::Thinking { thinking, .. } => {
                                // Fallback为 text
                                if !thinking.is_empty() {
                                    tracing::debug!("[Fallback] Converting thinking block to text (len={})", thinking.len());
                                    new_blocks.push(crate::proxy::mappers::claude::models::ContentBlock::Text { 
                                        text: thinking 
                                    });
                                }
                            },
                            crate::proxy::mappers::claude::models::ContentBlock::RedactedThinking { .. } => {
                                // Redacted thinking Of no use，Discard directly
                            },
                            _ => new_blocks.push(block),
                        }
                    }
                }
            }
            
            // [NEW] Heal session after stripping thinking blocks to prevent "naked ToolResult" rejection
            // This ensures that any ToolResult in history is properly "closed" with synthetic messages
            // if its preceding Thinking block was just converted to Text.
            crate::proxy::mappers::claude::thinking_utils::close_tool_loop_for_thinking(&mut request_for_body.messages);
            
            // clean upModelin name -thinking Suffix
            if request_for_body.model.contains("claude-") {
                let mut m = request_for_body.model.clone();
                m = m.replace("-thinking", "");
                if m.contains("claude-sonnet-4-5-") {
                    m = "claude-sonnet-4-5".to_string();
                } else if m.contains("claude-opus-4-5-") || m.contains("claude-opus-4-") {
                    m = "claude-opus-4-5".to_string();
                }
                request_for_body.model = m;
            }
            
            // [FIX] forceRetry：BecauseusAlreadyCleaned up thinking block，SoThis isoneNew、CanRetry的Request
            // don't wantUsing determine_retry_strategy，Becauseit willBecause retried_without_thinking=true 而Return NoRetry
            if apply_retry_strategy(
                RetryStrategy::FixedDelay(Duration::from_millis(100)), 
                attempt, 
                status_code, 
                &trace_id
            ).await {
                continue;
            }
        }

        // 5. Unified handling of all retryable errors
        // [REMOVED] No longer special handling QUOTA_EXHAUSTED,Allow account rotation
        // The original logic will be in the firstAccountQuota exhausteddirectReturn,lead to"balance"ModeUnable to switchAccount
        
        
        // SureRetryStrategy
        let strategy = determine_retry_strategy(status_code, &error_text, retried_without_thinking);
        
        // Executeretreat
        if apply_retry_strategy(strategy, attempt, status_code, &trace_id).await {
            // judgeYesNoNeedrotationAccount
            if !should_rotate_account(status_code) {
                debug!("[{}] Keeping same account for status {} (server-side issue)", trace_id, status_code);
            }
            continue;
        } else {
            // 5. enhanced 400 Error handling: Prompt Too Long friendlyHint
            if status_code == 400 && (error_text.contains("too long") || error_text.contains("exceeds") || error_text.contains("limit")) {
                 return (
                    StatusCode::BAD_REQUEST,
                    [("X-Account-Email", email.as_str())],
                    Json(json!({
                        "id": "err_prompt_too_long",
                        "type": "error",
                        "error": {
                            "type": "invalid_request_error",
                            "message": "Prompt is too long (server-side context limit reached).",
                            "suggestion": "Please: 1) Executive '/compact' in Claude Code 2) Reduce conversation history 3) Switch to gemini-1.5-pro (2M context limit)"
                        }
                    }))
                ).into_response();
            }

            // NoRetry的Error，directReturn
            error!("[{}] Non-retryable error {}: {}", trace_id, status_code, error_text);
            return (status, [("X-Account-Email", email.as_str())], error_text).into_response();
        }
    }
    
    if let Some(email) = last_email {
        (StatusCode::TOO_MANY_REQUESTS, [("X-Account-Email", email)], Json(json!({
            "type": "error",
            "error": {
                "type": "overloaded_error",
                "message": format!("All {} attempts failed. Last error: {}", max_attempts, last_error)
            }
        }))).into_response()
    } else {
        (StatusCode::TOO_MANY_REQUESTS, Json(json!({
            "type": "error",
            "error": {
                "type": "overloaded_error",
                "message": format!("All {} attempts failed. Last error: {}", max_attempts, last_error)
            }
        }))).into_response()
    }
}

/// Column出AvailableModel
pub async fn handle_list_models(State(state): State<AppState>) -> impl IntoResponse {
    use crate::proxy::common::model_mapping::get_all_dynamic_models;

    let model_ids = get_all_dynamic_models(
        &state.custom_mapping,
    ).await;

    let data: Vec<_> = model_ids.into_iter().map(|id| {
        json!({
            "id": id,
            "object": "model",
            "created": 1706745600,
            "owned_by": "antigravity"
        })
    }).collect();

    Json(json!({
        "object": "list",
        "data": data
    }))
}

/// calculate tokens (placeholder)
pub async fn handle_count_tokens(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let zai = state.zai.read().await.clone();
    let zai_enabled = zai.enabled && !matches!(zai.dispatch_mode, crate::proxy::ZaiDispatchMode::Off);

    if zai_enabled {
        return crate::proxy::providers::zai_anthropic::forward_anthropic_json(
            &state,
            axum::http::Method::POST,
            "/v1/messages/count_tokens",
            &headers,
            body,
        )
        .await;
    }

    Json(json!({
        "input_tokens": 0,
        "output_tokens": 0
    }))
    .into_response()
}

// RemoveexpiredSimpleCellTest，A complete integration will be completed in the futureTest
/*
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_handle_list_models() {
        // handle_list_models NowNeed AppState，Skip hereOldCellTest
    }
}
*/

// ===== Background task detectionAuxiliaryFunction =====

/// BackstageTaskType
#[derive(Debug, Clone, Copy, PartialEq)]
enum BackgroundTaskType {
    TitleGeneration,      // Title generation
    SimpleSummary,        // SimpleDigest
    ContextCompression,   // ContextCompress
    PromptSuggestion,     // Hintsuggestion
    SystemMessage,        // SystemMessage
    EnvironmentProbe,     // Environmentdetection
}

/// Title generation levelKey词
const TITLE_KEYWORDS: &[&str] = &[
    "write a 5-10 word title",
    "Please write a 5-10 word title",
    "Respond with the title",
    "Generate a title for",
    "Create a brief title",
    "title for the conversation",
    "conversation title",
    "generate title",
    "为ConversationGive a title",
];

/// DigestGenerate levelKey词
const SUMMARY_KEYWORDS: &[&str] = &[
    "Summarize this coding conversation",
    "Summarize the conversation",
    "Concise summary",
    "in under 50 characters",
    "compress the context",
    "Provide a concise summary",
    "condense the previous messages",
    "shorten the conversation history",
    "extract key points from",
];

/// Suggestions for generatingKey词
const SUGGESTION_KEYWORDS: &[&str] = &[
    "prompt suggestion generator",
    "suggest next prompts",
    "what should I ask next",
    "generate follow-up questions",
    "recommend next steps",
    "possible next actions",
];

/// SystemMessage关Key词
const SYSTEM_KEYWORDS: &[&str] = &[
    "Warmup",
    "<system-reminder>",
    // Removed: "Caveat: The messages below were generated" - this is a normal Claude Desktop system prompt
    "This is a system message",
];

/// EnvironmentDetection levelKey词
const PROBE_KEYWORDS: &[&str] = &[
    "check current directory",
    "list available tools",
    "verify environment",
    "test connection",
];

/// Detection backgroundTask并ReturnTaskType
fn detect_background_task_type(request: &ClaudeRequest) -> Option<BackgroundTaskType> {
    let last_user_msg = extract_last_user_message_for_detection(request)?;
    let preview = last_user_msg.chars().take(500).collect::<String>();
    
    // LengthFilter：BackstageTaskUsually no more than 800 character
    if last_user_msg.len() > 800 {
        return None;
    }
    
    // 按Prioritymatch
    if matches_keywords(&preview, SYSTEM_KEYWORDS) {
        return Some(BackgroundTaskType::SystemMessage);
    }
    
    if matches_keywords(&preview, TITLE_KEYWORDS) {
        return Some(BackgroundTaskType::TitleGeneration);
    }
    
    if matches_keywords(&preview, SUMMARY_KEYWORDS) {
        if preview.contains("in under 50 characters") {
            return Some(BackgroundTaskType::SimpleSummary);
        }
        return Some(BackgroundTaskType::ContextCompression);
    }
    
    if matches_keywords(&preview, SUGGESTION_KEYWORDS) {
        return Some(BackgroundTaskType::PromptSuggestion);
    }
    
    if matches_keywords(&preview, PROBE_KEYWORDS) {
        return Some(BackgroundTaskType::EnvironmentProbe);
    }
    
    None
}

/// AuxiliaryFunction：关Keyword match
fn matches_keywords(text: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|kw| text.contains(kw))
}

/// AuxiliaryFunction：extractFinallyone pieceUserMessage（for detection）
fn extract_last_user_message_for_detection(request: &ClaudeRequest) -> Option<String> {
    request.messages.iter().rev()
        .filter(|m| m.role == "user")
        .find_map(|m| {
            let content = match &m.content {
                crate::proxy::mappers::claude::models::MessageContent::String(s) => s.to_string(),
                crate::proxy::mappers::claude::models::MessageContent::Array(arr) => {
                    arr.iter()
                        .filter_map(|block| match block {
                            crate::proxy::mappers::claude::models::ContentBlock::Text { text } => Some(text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                }
            };
            
            if content.trim().is_empty() 
                || content.starts_with("Warmup") 
                || content.contains("<system-reminder>") 
            {
                None 
            } else {
                Some(content)
            }
        })
}

/// According to the backgroundTaskTypeChoose the right oneModel
fn select_background_model(task_type: BackgroundTaskType) -> &'static str {
    match task_type {
        BackgroundTaskType::TitleGeneration => INTERNAL_BACKGROUND_TASK,
        BackgroundTaskType::SimpleSummary => INTERNAL_BACKGROUND_TASK,
        BackgroundTaskType::SystemMessage => INTERNAL_BACKGROUND_TASK,
        BackgroundTaskType::PromptSuggestion => INTERNAL_BACKGROUND_TASK,
        BackgroundTaskType::EnvironmentProbe => INTERNAL_BACKGROUND_TASK,
        BackgroundTaskType::ContextCompression => INTERNAL_BACKGROUND_TASK,
    }
}

// ===== [Issue #467 Fix] Warmup Requestintercept =====

/// DetectionYesNo为 Warmup Request
/// 
/// Claude Code 每 10 秒Sendonce warmup Request，featurePacket括：
/// 1. UserMessageContent以 "Warmup" beginning orPacket含 "Warmup"
/// 2. tool_result Content为 "Warmup" Error
/// 3. MessagecycleMode：AssistantSendToolcall，UserReturn Warmup Error
fn is_warmup_request(request: &ClaudeRequest) -> bool {
    // [FIX] Only check the LATEST message for Warmup characteristics.
    // Scanning history (take(10)) caused a "poisoned session" bug where one historical Warmup
    // message would cause all subsequent user inputs (e.g. "Continue") to be intercepted 
    // and replied with "OK".
    
    if let Some(msg) = request.messages.last() {
        // We only care if the *current* trigger is a Warmup
        match &msg.content {
            crate::proxy::mappers::claude::models::MessageContent::String(s) => {
                // Check if simple text starts with Warmup (and is short)
                if s.trim().starts_with("Warmup") && s.len() < 100 {
                    return true;
                }
            },
            crate::proxy::mappers::claude::models::MessageContent::Array(arr) => {
                for block in arr {
                    match block {
                        crate::proxy::mappers::claude::models::ContentBlock::Text { text } => {
                            let trimmed = text.trim();
                            if trimmed == "Warmup" || trimmed.starts_with("Warmup\n") {
                                return true;
                            }
                        },
                        crate::proxy::mappers::claude::models::ContentBlock::ToolResult { 
                            content, is_error, .. 
                        } => {
                            // Check tool result errors
                            let content_str = if let Some(s) = content.as_str() {
                                s.to_string()
                            } else {
                                content.to_string()
                            };
                            
                            // If it's an error and starts with Warmup, it's a warmup signal
                            if *is_error == Some(true) && content_str.trim().starts_with("Warmup") {
                                return true;
                            }
                        },
                        _ => {}
                    }
                }
            }
        }
    }
    
    false
}

/// Create Warmup RequestsimulationResponse
/// 
/// ReturnoneSimple的Response，Does not consume upstreamQuota
fn create_warmup_response(request: &ClaudeRequest, is_stream: bool) -> Response {
    let model = &request.model;
    let message_id = format!("msg_warmup_{}", chrono::Utc::now().timestamp_millis());
    
    if is_stream {
        // Streaming response：Sendstandard SSE EventSequence
        let events = vec![
            // message_start
            format!(
                "event: message_start\ndata: {{\"type\":\"message_start\",\"message\":{{\"id\":\"{}\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"model\":\"{}\",\"stop_reason\":null,\"stop_sequence\":null,\"usage\":{{\"input_tokens\":1,\"output_tokens\":0}}}}}}\n\n",
                message_id, model
            ),
            // content_block_start
            "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n".to_string(),
            // content_block_delta
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"OK\"}}\n\n".to_string(),
            // content_block_stop
            "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n".to_string(),
            // message_delta
            "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\",\"stop_sequence\":null},\"usage\":{\"output_tokens\":1}}\n\n".to_string(),
            // message_stop
            "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n".to_string(),
        ];
        
        let body = events.join("");
        
        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/event-stream")
            .header(header::CACHE_CONTROL, "no-cache")
            .header(header::CONNECTION, "keep-alive")
            .header("X-Warmup-Intercepted", "true")
            .body(Body::from(body))
            .unwrap()
    } else {
        // Non-streaming response
        let response = json!({
            "id": message_id,
            "type": "message",
            "role": "assistant",
            "content": [{
                "type": "text",
                "text": "OK"
            }],
            "model": model,
            "stop_reason": "end_turn",
            "stop_sequence": null,
            "usage": {
                "input_tokens": 1,
                "output_tokens": 1
            }
        });
        
        (
            StatusCode::OK,
            [("X-Warmup-Intercepted", "true")],

    
    Json(response)
        ).into_response()
    }
}

// ===== [Helper] Synchronous Upstream Call =====
// Reusable function for making non-streaming calls to Gemini API
// Used by Layer 3 and potentially other internal operations

/// Call Gemini API synchronously and return the response text
/// 
/// This is used for internal operations that need to wait for a complete response,
/// such as generating summaries or other background tasks.
async fn call_gemini_sync(
    model: &str,
    request: &ClaudeRequest,
    token_manager: &Arc<crate::proxy::TokenManager>,
    trace_id: &str,
) -> Result<String, String> {
    // Get token and transform request
    let (access_token, project_id, _) = token_manager
        .get_token("gemini", false, None, model)
        .await
        .map_err(|e| format!("Failed to get account: {}", e))?;
    
    let gemini_body = crate::proxy::mappers::claude::transform_claude_request_in(request, &project_id, false)
        .map_err(|e| format!("Failed to transform request: {}", e))?;
    
    // Call Gemini API
    let upstream_url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
        model
    );
    
    debug!("[{}] Calling Gemini API: {}", trace_id, model);
    
    let response = reqwest::Client::new()
        .post(&upstream_url)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Type", "application/json")
        .json(&gemini_body)
        .send()
        .await
        .map_err(|e| format!("API call failed: {}", e))?;
    
    if !response.status().is_success() {
        return Err(format!(
            "API returned {}: {}", 
            response.status(), 
            response.text().await.unwrap_or_default()
        ));
    }
    
    let gemini_response: Value = response.json().await
        .map_err(|e| format!("Failed to parse response: {}", e))?;
    
    // Extract text from response
    gemini_response
        .get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.get(0))
        .and_then(|p| p.get("text"))
        .and_then(|t| t.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "Failed to extract text from response".to_string())
}

// ===== [Layer 3] Fork Conversation + XML Summary =====
// This is the ultimate context compression strategy
// Borrowed from Practical-Guide-to-Context-Engineering + Claude Code official practice

/// Try to compress context by generating an XML summary and forking the conversation
/// 
/// This function:
/// 1. Extracts the last valid thinking signature
/// 2. Calls a cheap model (gemini-2.5-flash-lite) to generate XML summary
/// 3. Creates a new message sequence with summary as prefix
/// 4. Preserves the signature in the summary
/// 5. Returns the forked request
/// 
/// Returns Ok(forked_request) on success, Err(error_message) on failure
async fn try_compress_with_summary(
    original_request: &ClaudeRequest,
    trace_id: &str,
    token_manager: &Arc<crate::proxy::TokenManager>,
) -> Result<ClaudeRequest, String> {
    info!("[{}] [Layer-3] Starting context compression with XML summary", trace_id);
    
    // 1. Extract last valid signature
    let last_signature = ContextManager::extract_last_valid_signature(&original_request.messages);
    
    if let Some(ref sig) = last_signature {
        debug!("[{}] [Layer-3] Extracted signature (len: {})", trace_id, sig.len());
    }
    
    // 2. Build summary request
    let mut summary_messages = original_request.messages.clone();
    
    // Add instruction to include signature in summary
    let signature_instruction = if let Some(ref sig) = last_signature {
        format!("\n\n**CRITICAL**: The last thinking signature is:\n```\n{}\n```\nYou MUST include this EXACTLY in the <latest_thinking_signature> section.", sig)
    } else {
        "\n\n**Note**: No thinking signature found in history. Leave <latest_thinking_signature> empty.".to_string()
    };
    
    // Append summary request as the last user message
    summary_messages.push(Message {
        role: "user".to_string(),
        content: MessageContent::String(format!(
            "{}{}",
            CONTEXT_SUMMARY_PROMPT,
            signature_instruction
        )),
    });
    
    let summary_request = ClaudeRequest {
        model: INTERNAL_BACKGROUND_TASK.to_string(),
        messages: summary_messages,
        system: None,
        stream: false,
        max_tokens: Some(8000),
        temperature: Some(0.3),
        tools: None,
        thinking: None,
        metadata: None,
        top_p: None,
        top_k: None,
        output_config: None,
        size: None,
        quality: None,
    };
    
    debug!("[{}] [Layer-3] Calling {} for summary generation", trace_id, INTERNAL_BACKGROUND_TASK);
    
    // 3. Call upstream using helper function (reuse existing infrastructure)
    let xml_summary = call_gemini_sync(
        INTERNAL_BACKGROUND_TASK,
        &summary_request,
        token_manager,
        trace_id,
    ).await?;
    
    info!("[{}] [Layer-3] Generated XML summary (len: {} chars)", trace_id, xml_summary.len());
    
    // 4. Create forked conversation with summary as prefix
    let mut forked_messages = vec![
        Message {
            role: "user".to_string(),
            content: MessageContent::String(format!(
                "Context has been compressed. Here is the structured summary of our conversation history:\n\n{}",
                xml_summary
            )),
        },
        Message {
            role: "assistant".to_string(),
            content: MessageContent::String(
                "I have reviewed the compressed context summary. I understand the current state and will continue from here.".to_string()
            ),
        },
    ];
    
    // 5. Append the user's latest message (if exists and is not the summary request)
    if let Some(last_msg) = original_request.messages.last() {
        if last_msg.role == "user" {
            // Check if it's not the summary instruction we just added
            if !matches!(&last_msg.content, MessageContent::String(s) if s.contains(CONTEXT_SUMMARY_PROMPT)) {
                forked_messages.push(last_msg.clone());
            }
        }
    }
    
    info!(
        "[{}] [Layer-3] Fork successful: {} messages → {} messages",
        trace_id,
        original_request.messages.len(),
        forked_messages.len()
    );
    
    // 6. Return forked request
    Ok(ClaudeRequest {
        model: original_request.model.clone(),
        messages: forked_messages,
        system: original_request.system.clone(),
        stream: original_request.stream,
        max_tokens: original_request.max_tokens,
        temperature: original_request.temperature,
        tools: original_request.tools.clone(),
        thinking: original_request.thinking.clone(),
        metadata: original_request.metadata.clone(),
        top_p: original_request.top_p,
        top_k: original_request.top_k,
        output_config: original_request.output_config.clone(),
        size: original_request.size.clone(),
        quality: original_request.quality.clone(),
    })
}
