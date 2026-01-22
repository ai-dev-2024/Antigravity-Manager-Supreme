// Claude RequestConvert (Claude → Gemini v1internal)
// correspond transformClaudeRequestIn

use super::models::*;
use crate::proxy::mappers::signature_store::get_thought_signature; // Deprecated, kept for fallback
use crate::proxy::mappers::tool_result_compressor;
use crate::proxy::session_manager::SessionManager;
use serde_json::{json, Value};
use std::collections::HashMap;

// ===== Safety Settings Configuration =====

/// Safety threshold levels for Gemini API
/// Can be configured via GEMINI_SAFETY_THRESHOLD environment variable
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SafetyThreshold {
    /// Disable all safety filters (default for proxy compatibility)
    Off,
    /// Block low probability and above
    BlockLowAndAbove,
    /// Block medium probability and above
    BlockMediumAndAbove,
    /// Only block high probability content
    BlockOnlyHigh,
    /// Don't block anything (BLOCK_NONE)
    BlockNone,
}

impl SafetyThreshold {
    /// Get threshold from environment variable or default to Off
    pub fn from_env() -> Self {
        match std::env::var("GEMINI_SAFETY_THRESHOLD").as_deref() {
            Ok("OFF") | Ok("off") => SafetyThreshold::Off,
            Ok("LOW") | Ok("low") => SafetyThreshold::BlockLowAndAbove,
            Ok("MEDIUM") | Ok("medium") => SafetyThreshold::BlockMediumAndAbove,
            Ok("HIGH") | Ok("high") => SafetyThreshold::BlockOnlyHigh,
            Ok("NONE") | Ok("none") => SafetyThreshold::BlockNone,
            _ => SafetyThreshold::Off, // Default: maintain current behavior
        }
    }

    /// Convert to Gemini API threshold string
    pub fn to_gemini_threshold(&self) -> &'static str {
        match self {
            SafetyThreshold::Off => "OFF",
            SafetyThreshold::BlockLowAndAbove => "BLOCK_LOW_AND_ABOVE",
            SafetyThreshold::BlockMediumAndAbove => "BLOCK_MEDIUM_AND_ABOVE",
            SafetyThreshold::BlockOnlyHigh => "BLOCK_ONLY_HIGH",
            SafetyThreshold::BlockNone => "BLOCK_NONE",
        }
    }
}

/// Build safety settings based on configuration
fn build_safety_settings() -> Value {
    let threshold = SafetyThreshold::from_env();
    let threshold_str = threshold.to_gemini_threshold();

    json!([
        { "category": "HARM_CATEGORY_HARASSMENT", "threshold": threshold_str },
        { "category": "HARM_CATEGORY_HATE_SPEECH", "threshold": threshold_str },
        { "category": "HARM_CATEGORY_SEXUALLY_EXPLICIT", "threshold": threshold_str },
        { "category": "HARM_CATEGORY_DANGEROUS_CONTENT", "threshold": threshold_str },
        { "category": "HARM_CATEGORY_CIVIC_INTEGRITY", "threshold": threshold_str },
    ])
}

/// clean upMessagein cache_control Field
/// 
/// thisFunction会DepthTraverseAllMessageContentBlock,Remove cache_control Field。
/// This isnecessary,Because:
/// 1. VS Code 等Clientwill make historyMessage(Packet含 cache_control)Send back intact
/// 2. Anthropic API Not acceptedRequest中Packet含 cache_control Field
/// 3. even thoughYesForward到 Gemini,也Shouldclean to keepProtocolpurity
/// 
/// [FIX #593] EnhanceVersion:AdddetailedLogused forDebug MCP ToolCompatiblesexual problems
pub fn clean_cache_control_from_messages(messages: &mut [Message]) {
    tracing::info!(
        "[DEBUG-593] Starting cache_control cleanup for {} messages",
        messages.len()
    );
    
    let mut total_cleaned = 0;
    
    for (idx, msg) in messages.iter_mut().enumerate() {
        if let MessageContent::Array(blocks) = &mut msg.content {
            for (block_idx, block) in blocks.iter_mut().enumerate() {
                match block {
                    ContentBlock::Thinking { cache_control, .. } => {
                        if cache_control.is_some() {
                            tracing::info!(
                                "[ISSUE-744] Found cache_control in Thinking block at message[{}].content[{}]: {:?}",
                                idx,
                                block_idx,
                                cache_control
                            );
                            *cache_control = None;
                            total_cleaned += 1;
                        }
                    }
                    ContentBlock::Image { cache_control, .. } => {
                        if cache_control.is_some() {
                            tracing::debug!(
                                "[Cache-Control-Cleaner] Removed cache_control from Image block at message[{}].content[{}]",
                                idx,
                                block_idx
                            );
                            *cache_control = None;
                            total_cleaned += 1;
                        }
                    }
                    ContentBlock::Document { cache_control, .. } => {
                        if cache_control.is_some() {
                            tracing::debug!(
                                "[Cache-Control-Cleaner] Removed cache_control from Document block at message[{}].content[{}]",
                                idx,
                                block_idx
                            );
                            *cache_control = None;
                            total_cleaned += 1;
                        }
                    }
                    ContentBlock::ToolUse { cache_control, .. } => {
                        if cache_control.is_some() {
                            tracing::debug!(
                                "[Cache-Control-Cleaner] Removed cache_control from ToolUse block at message[{}].content[{}]",
                                idx,
                                block_idx
                            );
                            *cache_control = None;
                            total_cleaned += 1;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    
    if total_cleaned > 0 {
        tracing::info!(
            "[DEBUG-593] Cache control cleanup complete: removed {} cache_control fields",
            total_cleaned
        );
    } else {
        tracing::debug!("[DEBUG-593] No cache_control fields found");
    }
}

/// [FIX #593] recursionDepthclean up JSON in cache_control Field
/// 
/// used forHandleNestedStructand non-standardPosition的 cache_control。
/// This isFinallya line of defense,make sureSend给 Antigravity 的RequestHit or missPacket含Any cache_control。
fn deep_clean_cache_control(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if map.remove("cache_control").is_some() {
                tracing::debug!("[DEBUG-593] Removed cache_control from nested JSON object");
            }
            for (_, v) in map.iter_mut() {
                deep_clean_cache_control(v);
            }
        }
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                deep_clean_cache_control(item);
            }
        }
        _ => {}
    }
}

/// [FIX #564] Sort blocks in assistant messages to ensure thinking blocks are first
/// 
/// When context compression (kilo) reorders message blocks, thinking blocks may appear
/// after text blocks. Claude/Anthropic API requires thinking blocks to be first if
/// any thinking blocks exist in the message. This function pre-sorts blocks to ensure
/// thinking/redacted_thinking blocks always come before other block types.
fn sort_thinking_blocks_first(messages: &mut [Message]) {
    for msg in messages.iter_mut() {
        if msg.role == "assistant" {
            if let MessageContent::Array(blocks) = &mut msg.content {
                // [FIX #709] Triple-stage partition: [Thinking, Text, ToolUse]
                // This ensures protocol compliance while maintaining logical order.
                
                let mut thinking_blocks: Vec<ContentBlock> = Vec::new();
                let mut text_blocks: Vec<ContentBlock> = Vec::new();
                let mut tool_blocks: Vec<ContentBlock> = Vec::new();
                let mut other_blocks: Vec<ContentBlock> = Vec::new();
                
                let original_len = blocks.len();
                let mut needs_reorder = false;
                let mut saw_non_thinking = false;

                for (_i, block) in blocks.iter().enumerate() {
                    match block {
                        ContentBlock::Thinking { .. } | ContentBlock::RedactedThinking { .. } => {
                            if saw_non_thinking {
                                needs_reorder = true;
                            }
                        }
                        ContentBlock::Text { .. } => {
                            saw_non_thinking = true;
                        }
                        ContentBlock::ToolUse { .. } => {
                            saw_non_thinking = true;
                            // Check if tool is after text (this is normal, but we want a strict group order)
                        }
                        _ => saw_non_thinking = true,
                    }
                }

                if needs_reorder || original_len > 1 {
                    // For safety, we always perform the triple partition if there's more than one block.
                    // This also handles empty text block filtering.
                    for block in blocks.drain(..) {
                        match &block {
                            ContentBlock::Thinking { .. } | ContentBlock::RedactedThinking { .. } => {
                                thinking_blocks.push(block);
                            }
                            ContentBlock::Text { text } => {
                                // Filter out purely empty or structural text like "(no content)"
                                if !text.trim().is_empty() && text != "(no content)" {
                                    text_blocks.push(block);
                                }
                            }
                            ContentBlock::ToolUse { .. } => {
                                tool_blocks.push(block);
                            }
                            _ => {
                                other_blocks.push(block);
                            }
                        }
                    }

                    // Reconstruct in strict order: Thinking -> Text/Other -> Tool
                    blocks.extend(thinking_blocks);
                    blocks.extend(text_blocks);
                    blocks.extend(other_blocks);
                    blocks.extend(tool_blocks);

                    if needs_reorder {
                        tracing::warn!(
                            "[FIX #709] Reordered assistant messages to [Thinking, Text, Tool] structure."
                        );
                    }
                }
            }
        }
    }
}

/// Merge ClaudeRequest consecutive sameRoleMessage
/// 
/// scene: When从 Spec/Plan Modeswitch backEncodeMode时，MayTwo consecutive "user" Message
/// (one pieceYes ToolResult，one pieceYes <system-reminder>)。
/// this would violateRolealternation rule，lead to 400 Report an error。
pub fn merge_consecutive_messages(messages: &mut Vec<Message>) {
    if messages.len() <= 1 {
        return;
    }

    let mut merged: Vec<Message> = Vec::with_capacity(messages.len());
    let old_messages = std::mem::take(messages);
    let mut messages_iter = old_messages.into_iter();

    if let Some(mut current) = messages_iter.next() {
        for next in messages_iter {
            if current.role == next.role {
                // MergeContent
                match (&mut current.content, next.content) {
                    (MessageContent::Array(current_blocks), MessageContent::Array(next_blocks)) => {
                        current_blocks.extend(next_blocks);
                    }
                    (MessageContent::Array(current_blocks), MessageContent::String(next_text)) => {
                        current_blocks.push(ContentBlock::Text { text: next_text });
                    }
                    (MessageContent::String(current_text), MessageContent::String(next_text)) => {
                        *current_text = format!("{}\n\n{}", current_text, next_text);
                    }
                    (MessageContent::String(current_text), MessageContent::Array(next_blocks)) => {
                        let mut new_blocks = vec![ContentBlock::Text { text: current_text.clone() }];
                        new_blocks.extend(next_blocks);
                        current.content = MessageContent::Array(new_blocks);
                    }
                }
            } else {
                merged.push(current);
                current = next;
            }
        }
        merged.push(current);
    }

    *messages = merged;
}

/// Convert Claude Request为 Gemini v1internal Format

/// [FIX #709] Reorder serialized Gemini parts to ensure thinking blocks are first
fn reorder_gemini_parts(parts: &mut Vec<Value>) {
    if parts.len() <= 1 {
        return;
    }

    let mut thinking_parts = Vec::new();
    let mut text_parts = Vec::new();
    let mut tool_parts = Vec::new();
    let mut other_parts = Vec::new();

    for part in parts.drain(..) {
        if part.get("thought").and_then(|t| t.as_bool()) == Some(true) {
            thinking_parts.push(part);
        } else if part.get("functionCall").is_some() {
            tool_parts.push(part);
        } else if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
            // Filter empty text parts that might have been created during merging
            if !text.trim().is_empty() && text != "(no content)" {
                text_parts.push(part);
            }
        } else {
            other_parts.push(part);
        }
    }

    parts.extend(thinking_parts);
    parts.extend(text_parts);
    parts.extend(other_parts);
    parts.extend(tool_parts);
}

pub fn transform_claude_request_in(
    claude_req: &ClaudeRequest,
    project_id: &str,
    is_retry: bool,
) -> Result<Value, String> {
    // [CRITICAL FIX] Clean up beforehandAllMessagein cache_control Field
    // this solved VS Code Plugin等Clientin multiple roundsConversationlieutenant general historyMessage的 cache_control Field
    // Caused by sending it back intact "Extra inputs are not permitted" Error
    let mut cleaned_req = claude_req.clone();

    // [FIX #813] Mergeconsecutive sameRoleMessage (Consecutive User Messages)
    // make sureRequestconform to Anthropic 和 Gemini 的RolealternatelyProtocol
    merge_consecutive_messages(&mut cleaned_req.messages);

    clean_cache_control_from_messages(&mut cleaned_req.messages);
    
    // [FIX #564] Pre-sort thinking blocks to be first in assistant messages
    // This handles cases where context compression (kilo) incorrectly reorders blocks
    sort_thinking_blocks_first(&mut cleaned_req.messages);
    
    let claude_req = &cleaned_req; // Follow-upUsingafter cleaningRequest

    // [NEW] Generate session ID for signature tracking
    // This enables session-isolated signature storage, preventing cross-conversation pollution
    let session_id = SessionManager::extract_session_id(claude_req);
    tracing::debug!("[Claude-Request] Session ID: {}", session_id);

    // DetectionYesNoHave internet connectionTool (server tool or built-in tool)
    let has_web_search_tool = claude_req
        .tools
        .as_ref()
        .map(|tools| {
            tools.iter().any(|t| {
                t.is_web_search() 
                    || t.name.as_deref() == Some("google_search")
                    || t.type_.as_deref() == Some("web_search_20250305")
            })
        })
        .unwrap_or(false);

    // for storage tool_use id -> name Mapping
    let mut tool_id_to_name: HashMap<String, String> = HashMap::new();

    // DetectionYesNo有 mcp__ beginningTool
    let has_mcp_tools = claude_req
        .tools
        .as_ref()
        .map(|tools| {
            tools.iter().any(|t| {
                t.name.as_deref().map(|n| n.starts_with("mcp__")).unwrap_or(false)
            })
        })
        .unwrap_or(false);

    // [New] pre-builtToolName到Raw Schema 的Mapping，for follow-upParameterTypeCorrection
    let mut tool_name_to_schema = HashMap::new();
    if let Some(tools) = &claude_req.tools {
        for tool in tools {
            if let (Some(name), Some(schema)) = (&tool.name, &tool.input_schema) {
                tool_name_to_schema.insert(name.clone(), schema.clone());
            }
        }
    }

    // 1. System Instruction (injectionDynamicIdentity protection & MCP XML Protocol)
    let system_instruction = build_system_instruction(&claude_req.system, &claude_req.model, has_mcp_tools);

    //  Map model name (Use standard mapping)
    // [IMPROVED] extract web search Model为Constant，Easy to maintain
    const WEB_SEARCH_FALLBACK_MODEL: &str = "gemini-2.5-flash";

    let mapped_model = if has_web_search_tool {
        tracing::debug!(
            "[Claude-Request] Web search tool detected, using fallback model: {}",
            WEB_SEARCH_FALLBACK_MODEL
        );
        WEB_SEARCH_FALLBACK_MODEL.to_string()
    } else {
        crate::proxy::common::model_mapping::map_claude_model_to_gemini(&claude_req.model)
    };
    
    // 将 Claude Toolconvert to Value Arrayto detect networking
    let tools_val: Option<Vec<Value>> = claude_req.tools.as_ref().map(|list| {
        list.iter().map(|t| serde_json::to_value(t).unwrap_or(json!({}))).collect()
    });


    // Resolve grounding config
    let config = crate::proxy::mappers::common_utils::resolve_request_config(
        &claude_req.model, 
        &mapped_model, 
        &tools_val,
        claude_req.size.as_deref(),      // [NEW] Pass size parameter
        claude_req.quality.as_deref()    // [NEW] Pass quality parameter
    );
    
    // [CRITICAL FIX] Disable dummy thought injection for Vertex AI
    // [CRITICAL FIX] Disable dummy thought injection for Vertex AI
    // Vertex AI rejects thinking blocks without valid signatures
    // Even if thinking is enabled, we should NOT inject dummy blocks for historical messages
    let allow_dummy_thought = false;
    
    // Check if thinking is enabled in the request
    let mut is_thinking_enabled = claude_req
        .thinking
        .as_ref()
        .map(|t| t.type_ == "enabled")
        .unwrap_or_else(|| {
            // [Claude Code v2.0.67+] Default thinking enabled for Opus 4.5
            // If no thinking config is provided, enable by default for Opus models
            should_enable_thinking_by_default(&claude_req.model)
        });

    // [NEW FIX] Check if target model supports thinking
    // Only models with "-thinking" suffix or Claude models support thinking
    // Regular Gemini models (gemini-2.5-flash, gemini-2.5-pro) do NOT support thinking
    let target_model_supports_thinking = mapped_model.contains("-thinking") 
        || mapped_model.starts_with("claude-");
    
    if is_thinking_enabled && !target_model_supports_thinking {
        tracing::warn!(
            "[Thinking-Mode] Target model '{}' does not support thinking. Force disabling thinking mode.",
            mapped_model
        );
        is_thinking_enabled = false;
    }

    // [New Strategy] intelligentFallback: CheckhistoryMessageYesNo与 Thinking ModeCompatible
    // Ifin no condition Thinking 的Toolin call chain，MustTemporaryDisable Thinking
    if is_thinking_enabled {
        let should_disable = should_disable_thinking_due_to_history(&claude_req.messages);
        if should_disable {
             tracing::warn!("[Thinking-Mode] Automatically disabling thinking checks due to incompatible tool-use history (mixed application)");
             is_thinking_enabled = false;
        }
    }

    // [FIX #295 & #298] If thinking enabled but no signature available,
    // disable thinking to prevent Gemini 3 Pro rejection
    if is_thinking_enabled {
        let global_sig = get_thought_signature();
        
        // Check if there are any thinking blocks in message history
        let has_thinking_history = claude_req.messages.iter().any(|m| {
            if m.role == "assistant" {
                if let MessageContent::Array(blocks) = &m.content {
                    return blocks.iter().any(|b| matches!(b, ContentBlock::Thinking { .. }));
                }
            }
            false
        });
        
        // Check if there are function calls in the request
        let has_function_calls = claude_req.messages.iter().any(|m| {
            if let MessageContent::Array(blocks) = &m.content {
                blocks
                    .iter()
                    .any(|b| matches!(b, ContentBlock::ToolUse { .. }))
            } else {
                false
            }
        });

        // [FIX #298] For first-time thinking requests (no thinking history),
        // we use permissive mode and let upstream handle validation.
        // We only enforce strict signature checks when function calls are involved.
        let needs_signature_check = has_function_calls;
        
        if !has_thinking_history && is_thinking_enabled {
             tracing::info!(
                "[Thinking-Mode] First thinking request detected. Using permissive mode - \
                 signature validation will be handled by upstream API."
            );
        }

        if needs_signature_check
            && !has_valid_signature_for_function_calls(&claude_req.messages, &global_sig, &session_id)
        {
            tracing::warn!(
                "[Thinking-Mode] [FIX #295] No valid signature found for function calls. \
                 Disabling thinking to prevent Gemini 3 Pro rejection."
            );
            is_thinking_enabled = false;
        }
    }

    // 4. Generation Config & Thinking (Pass final is_thinking_enabled)
    let generation_config = build_generation_config(claude_req, has_web_search_tool, is_thinking_enabled);

    // 2. Contents (Messages)
    let contents = build_google_contents(
        &claude_req.messages,
        claude_req,
        &mut tool_id_to_name,
        &tool_name_to_schema,
        is_thinking_enabled,
        allow_dummy_thought,
        &mapped_model,
        &session_id,
        is_retry,
    )?;

    // 3. Tools
    let tools = build_tools(&claude_req.tools, has_web_search_tool)?;

    // 5. Safety Settings (configurable via GEMINI_SAFETY_THRESHOLD env var)
    let safety_settings = build_safety_settings();

    // Build inner request
    let mut inner_request = json!({
        "contents": contents,
        "safetySettings": safety_settings,
    });

    // Depthclean up [undefined] string (Cherry Studio 等ClientCommon injections)
    crate::proxy::mappers::common_utils::deep_clean_undefined(&mut inner_request);

    if let Some(sys_inst) = system_instruction {
        inner_request["systemInstruction"] = sys_inst;
    }

    if !generation_config.is_null() {
        inner_request["generationConfig"] = generation_config;
    }

    if let Some(tools_val) = tools {
        inner_request["tools"] = tools_val;
        // explicitSetToolConfigMode为 VALIDATED
        inner_request["toolConfig"] = json!({
            "functionCallingConfig": {
                "mode": "VALIDATED"
            }
        });
    }

    // Inject googleSearch tool if needed (and not already done by build_tools)
    if config.inject_google_search && !has_web_search_tool {
        crate::proxy::mappers::common_utils::inject_google_search_tool(&mut inner_request);
    }

    // Inject imageConfig if present (for image generation models)
    if let Some(image_config) = config.image_config {
        if let Some(obj) = inner_request.as_object_mut() {
            // 1. Remove tools (image generation does not support tools)
            obj.remove("tools");

            // 2. Remove systemInstruction (image generation does not support system prompts)
            obj.remove("systemInstruction");

            // 3. Clean generationConfig (remove thinkingConfig, responseMimeType, responseModalities etc.)
            let gen_config = obj.entry("generationConfig").or_insert_with(|| json!({}));
            if let Some(gen_obj) = gen_config.as_object_mut() {
                gen_obj.remove("thinkingConfig");
                gen_obj.remove("responseMimeType");
                gen_obj.remove("responseModalities");
                gen_obj.insert("imageConfig".to_string(), image_config);
            }
        }
    }

    // generate requestId
    let request_id = format!("agent-{}", uuid::Uuid::new_v4());

    // build finalRequest体
    let mut body = json!({
        "project": project_id,
        "requestId": request_id,
        "request": inner_request,
        "model": config.final_model,
        "userAgent": "antigravity",
        "requestType": config.request_type,
    });

    // Ifprovided metadata.user_id，Then reuse as sessionId
    if let Some(metadata) = &claude_req.metadata {
        if let Some(user_id) = &metadata.user_id {
            body["request"]["sessionId"] = json!(user_id);
        }
    }

    // [FIX #593] Finallya line of defense: recursionDepthclean upAll cache_control Field
    // make sureSend给 Antigravity 的RequestHit or missPacket含Any cache_control
    deep_clean_cache_control(&mut body);
    tracing::debug!("[DEBUG-593] Final deep clean complete, request ready to send");

    Ok(body)
}

/// CheckYesNoBecausehistoryMessagereasonNeedDisable Thinking
/// 
/// scene: IfFinallyone piece Assistant Messagein Tool Use StreamCheng Zhong，但None Thinking Block，
/// DescriptionThis isa reason Thinking ModelinitiatedStream程。Forced to open at this time Thinking will lead to:
/// "final assistant message must start with a thinking block" Error。
/// We cannot fake legal Thinking (BecauseSignquestion)，UniquesolutionYesThis roundRequesttemporaryDisable Thinking。
fn should_disable_thinking_due_to_history(messages: &[Message]) -> bool {
    // Search in reverse orderFinallyone piece Assistant Message
    for msg in messages.iter().rev() {
        if msg.role == "assistant" {
            if let MessageContent::Array(blocks) = &msg.content {
                let has_tool_use = blocks.iter().any(|b| matches!(b, ContentBlock::ToolUse { .. }));
                let has_thinking = blocks.iter().any(|b| matches!(b, ContentBlock::Thinking { .. }));
                
                // If有Toolcall，但None Thinking Block -> 不Compatible
                if has_tool_use && !has_thinking {
                    tracing::info!("[Thinking-Mode] Detected ToolUse without Thinking in history. Requesting disable.");
                    return true;
                }
            }
            // Just find the nearest one Assistant Message就EndCheck
            // BecauseValidateruleMainagainstCurrentclosed loopStatus
            return false;
        }
    }
    false
}

/// Check if thinking mode should be enabled by default for a given model
///
/// Claude Code v2.0.67+ enables thinking by default for Opus 4.5 models.
/// This function determines if the model should have thinking enabled
/// when no explicit thinking configuration is provided.
fn should_enable_thinking_by_default(model: &str) -> bool {
    let model_lower = model.to_lowercase();

    // Enable thinking by default for Opus 4.5 variants
    if model_lower.contains("opus-4-5") || model_lower.contains("opus-4.5") {
        tracing::debug!(
            "[Thinking-Mode] Auto-enabling thinking for Opus 4.5 model: {}",
            model
        );
        return true;
    }

    // Also enable for explicit thinking model variants
    if model_lower.contains("-thinking") {
        return true;
    }

    false
}

/// Minimum length for a valid thought_signature
const MIN_SIGNATURE_LENGTH: usize = 50;

/// [FIX #295] Check if we have any valid signature available for function calls
/// This prevents Gemini 3 Pro from rejecting requests due to missing thought_signature
/// 
/// [NEW FIX] Now also checks Session Cache to support retry scenarios
fn has_valid_signature_for_function_calls(
    messages: &[Message],
    global_sig: &Option<String>,
    session_id: &str,  // NEW: Add session_id parameter
) -> bool {
    // 1. Check global store (deprecated but kept for compatibility)
    if let Some(sig) = global_sig {
        if sig.len() >= MIN_SIGNATURE_LENGTH {
            tracing::debug!(
                "[Signature-Check] Found valid signature in global store (len: {})",
                sig.len()
            );
            return true;
        }
    }

    // 2. [NEW] Check Session Cache - this is critical for retry scenarios
    // When retrying, the signature may not be in messages but exists in Session Cache
    if let Some(sig) = crate::proxy::SignatureCache::global().get_session_signature(session_id) {
        if sig.len() >= MIN_SIGNATURE_LENGTH {
            tracing::info!(
                "[Signature-Check] Found valid signature in SESSION cache (session: {}, len: {})",
                session_id,
                sig.len()
            );
            return true;
        }
    }

    // 3. Check if any message has a thinking block with valid signature
    for msg in messages.iter().rev() {
        if msg.role == "assistant" {
            if let MessageContent::Array(blocks) = &msg.content {
                for block in blocks {
                    if let ContentBlock::Thinking {
                        signature: Some(sig),
                        ..
                    } = block
                    {
                        if sig.len() >= MIN_SIGNATURE_LENGTH {
                            tracing::debug!(
                                "[Signature-Check] Found valid signature in message history (len: {})",
                                sig.len()
                            );
                            return true;
                        }
                    }
                }
            }
        }
    }
    
    tracing::warn!(
        "[Signature-Check] No valid signature found (session: {}, checked: global store, session cache, message history)",
        session_id
    );
    false
}

/// build System Instruction (SupportDynamicidentityMapping与 Prompt isolation)
fn build_system_instruction(system: &Option<SystemPrompt>, _model_name: &str, has_mcp_tools: bool) -> Option<Value> {
    let mut parts = Vec::new();

    // [NEW] Antigravity Identity command (RawSimplified version)
    let antigravity_identity = "You are Antigravity, a powerful agentic AI coding assistant designed by the Google Deepmind team working on Advanced Agentic Coding.\n\
    You are pair programming with a USER to solve their coding task. The task may require creating a new codebase, modifying or debugging an existing codebase, or simply answering a question.\n\
    **Absolute paths only**\n\
    **Proactiveness**";
    
    // [HYBRID] CheckUserYesNoprovided Antigravity identity
    let mut user_has_antigravity = false;
    if let Some(sys) = system {
        match sys {
            SystemPrompt::String(text) => {
                if text.contains("You are Antigravity") {
                    user_has_antigravity = true;
                }
            }
            SystemPrompt::Array(blocks) => {
                for block in blocks {
                    if block.block_type == "text" && block.text.contains("You are Antigravity") {
                        user_has_antigravity = true;
                        break;
                    }
                }
            }
        }
    }

    // IfUserNonesupply Antigravity identity,then inject
    if !user_has_antigravity {
        parts.push(json!({"text": antigravity_identity}));
    }

    // AddUser的SystemHint词
    if let Some(sys) = system {
        match sys {
            SystemPrompt::String(text) => {
                // [FIX] Filter OpenCode DefaultHint词，but keepUserCustominstruction (Instructions from: ...)
                if text.contains("You are an interactive CLI tool") {
                    // extractUserCustomcommand part
                    if let Some(idx) = text.find("Instructions from:") {
                        let custom_part = &text[idx..];
                        tracing::info!("[Claude-Request] Extracted custom instructions (len: {}), filtered default prompt", custom_part.len());
                        parts.push(json!({"text": custom_part}));
                    } else {
                        tracing::info!("[Claude-Request] Filtering out OpenCode default system instruction (len: {})", text.len());
                    }
                } else {
                    parts.push(json!({"text": text}));
                }
            }
            SystemPrompt::Array(blocks) => {
                for block in blocks {
                    if block.block_type == "text" {
                        // [FIX] Filter OpenCode DefaultHint词，but keepUserCustominstruction
                        if block.text.contains("You are an interactive CLI tool") {
                            if let Some(idx) = block.text.find("Instructions from:") {
                                let custom_part = &block.text[idx..];
                                tracing::info!("[Claude-Request] Extracted custom instructions from block (len: {})", custom_part.len());
                                parts.push(json!({"text": custom_part}));
                            } else {
                                tracing::info!("[Claude-Request] Filtering out OpenCode default system block (len: {})", block.text.len());
                            }
                        } else {
                            parts.push(json!({"text": block.text}));
                        }
                    }
                }
            }
        }
    }

    // [NEW] MCP XML Bridge: Ifexist mcp__ beginningTool，injectionSpecializedcallProtocol
    // This canValidAvoidance part MCP link in standard tool_use Protocol下Parseunstable problem
    if has_mcp_tools {
        let mcp_xml_prompt = "\n\
        ==== MCP XML ToolcallProtocol (Workaround) ====\n\
        When你NeedcallName以 `mcp__` beginning MCP Tool时：\n\
        1) priorityTrying XML Formatcall：Output `<mcp__tool_name>{\"arg\":\"value\"}</mcp__tool_name>`。\n\
        2) MustdirectOutput XML Block，No need markdown Packet装，Content为 JSON Formatof ginseng。\n\
        3) This approach has higher connectivity and fault tolerance，Suitable for largeResultReturnscene。\n\
        ===========================================";
        parts.push(json!({"text": mcp_xml_prompt}));
    }

    // IfUserNonesupplyAnySystemHint词,AddEndmark
    if !user_has_antigravity {
        parts.push(json!({"text": "\n--- [SYSTEM_PROMPT_END] ---"}));
    }

    Some(json!({
        "role": "user",
        "parts": parts
    }))
}

/// build Contents (Messages)
fn build_contents(
    content: &MessageContent,
    is_assistant: bool,
    _claude_req: &ClaudeRequest,
    is_thinking_enabled: bool,
    session_id: &str,
    allow_dummy_thought: bool,
    is_retry: bool,
    tool_id_to_name: &mut HashMap<String, String>,
    tool_name_to_schema: &HashMap<String, Value>,
    mapped_model: &str,
    last_thought_signature: &mut Option<String>,
    pending_tool_use_ids: &mut Vec<String>,
    last_user_task_text_normalized: &mut Option<String>,
    previous_was_tool_result: &mut bool,
    _existing_tool_result_ids: &std::collections::HashSet<String>,
) -> Result<Vec<Value>, String> {
    let mut parts = Vec::new();
    // Track tool results in the current turn to identify missing ones
    let mut current_turn_tool_result_ids = std::collections::HashSet::new();

    // Track if we have already seen non-thinking content in this message.
    // Anthropic/Gemini protocol: Thinking blocks MUST come first.
    let mut saw_non_thinking = false;

    match content {
        MessageContent::String(text) => {
            if text != "(no content)" {
                if !text.trim().is_empty() {
                    parts.push(json!({"text": text.trim()}));
                }
            }
        }
        MessageContent::Array(blocks) => {
            for item in blocks {
                match item {
                    ContentBlock::Text { text } => {
                        if text != "(no content)" {
                            // [NEW] TaskDeduplication logic: IfCurrentYes User Message，and follow closely ToolResult After，
                            // Checkthe textYesNowith previous roundTaskThe description is exactly the same。
                            if !is_assistant && *previous_was_tool_result {
                                if let Some(last_task) = last_user_task_text_normalized {
                                    let current_normalized = text.replace(|c: char| c.is_whitespace(), "");
                                    if !current_normalized.is_empty() && current_normalized == *last_task {
                                        tracing::info!("[Claude-Request] Dropping duplicated task text echo (len: {})", text.len());
                                        continue;
                                    }
                                }
                            }

                            parts.push(json!({"text": text}));
                            saw_non_thinking = true;

                            // Recordmost recent User TaskText for subsequent comparison
                            if !is_assistant {
                                *last_user_task_text_normalized = Some(text.replace(|c: char| c.is_whitespace(), ""));
                            }
                            *previous_was_tool_result = false;
                        }
                    }
                    ContentBlock::Thinking { thinking, signature, .. } => {
                        tracing::debug!("[DEBUG-TRANSFORM] Processing thinking block. Sig: {:?}", signature);

                        // [HOTFIX] Gemini Protocol Enforcement: Thinking block MUST be the first block.
                        // If we already have content (like Text), we must downgrade this thinking block to Text.
                        if saw_non_thinking || !parts.is_empty() {
                            tracing::warn!("[Claude-Request] Thinking block found at non-zero index (prev parts: {}). Downgrading to Text.", parts.len());
                            if !thinking.is_empty() {
                                parts.push(json!({
                                    "text": thinking
                                }));
                                saw_non_thinking = true;
                            }
                            continue;
                        }

                        // [FIX] If thinking is disabled (smart downgrade), convert ALL thinking blocks to text
                        // to avoid "thinking is disabled but message contains thinking" error
                        if !is_thinking_enabled {
                            tracing::warn!("[Claude-Request] Thinking disabled. Downgrading thinking block to text.");
                            if !thinking.is_empty() {
                                parts.push(json!({
                                    "text": thinking
                                }));
                            }
                            continue;
                        }

                        // [FIX] Empty thinking blocks cause "Field required" errors.
                        // We downgrade them to Text to avoid structural errors and signature mismatch.
                        if thinking.is_empty() {
                            tracing::warn!("[Claude-Request] Empty thinking block detected. Downgrading to Text.");
                            parts.push(json!({
                                "text": "..."
                            }));
                            continue;
                        }

                        // [FIX #752] Strict signature validation
                        // Only use signatures that are cached and compatible with the target model
                        if let Some(sig) = signature {
                            let cached_family = crate::proxy::SignatureCache::global().get_signature_family(sig);

                            match cached_family {
                                Some(family) => {
                                    // Check compatibility
                                    // [NEW] If is_retry is true, force incompatibility to strip historical signatures
                                    // which likely caused the previous 400 error.
                                    let compatible = !is_retry && is_model_compatible(&family, mapped_model);
                                    
                                    if !compatible {
                                        tracing::warn!(
                                            "[Thinking-Signature] {} signature (Family: {}, Target: {}). Downgrading to text.",
                                            if is_retry { "Stripping historical" } else { "Incompatible" },
                                            family, mapped_model
                                        );
                                        parts.push(json!({"text": thinking}));
                                        saw_non_thinking = true;
                                        continue;
                                    }
                                    // Compatible and not a retry: use signature
                                    *last_thought_signature = Some(sig.clone());
                                    let mut part = json!({
                                        "text": thinking,
                                        "thought": true,
                                        "thoughtSignature": sig
                                    });
                                    crate::proxy::common::json_schema::clean_json_schema(&mut part);
                                    parts.push(part);
                                }
                                None => {
                                    // Unknown signature origin: downgrade to text for safety
                                    tracing::warn!(
                                        "[Thinking-Signature] Unknown signature origin (len: {}). Downgrading to text for safety.",
                                        sig.len()
                                    );
                                    parts.push(json!({"text": thinking}));
                                    saw_non_thinking = true;
                                    continue;
                                }
                            }
                        } else {
                            // No signature: downgrade to text
                            tracing::warn!("[Thinking-Signature] No signature provided. Downgrading to text.");
                            parts.push(json!({"text": thinking}));
                            saw_non_thinking = true;
                        }
                    }
                    ContentBlock::RedactedThinking { data } => {
                        // [FIX] 将 RedactedThinking asNormaltextHandle，reserveContext
                        tracing::debug!("[Claude-Request] Degrade RedactedThinking to text");
                        parts.push(json!({
                            "text": format!("[Redacted Thinking: {}]", data)
                        }));
                        saw_non_thinking = true;
                        continue;
                    }
                    ContentBlock::Image { source, .. } => {
                        if source.source_type == "base64" {
                            parts.push(json!({
                                "inlineData": {
                                    "mimeType": source.media_type,
                                    "data": source.data
                                }
                            }));
                            saw_non_thinking = true;
                        }
                    }
                    ContentBlock::Document { source, .. } => {
                        if source.source_type == "base64" {
                            parts.push(json!({
                                "inlineData": {
                                    "mimeType": source.media_type,
                                    "data": source.data
                                }
                            }));
                            saw_non_thinking = true;
                        }
                    }
                    ContentBlock::ToolUse { id, name, input, signature, .. } => {
                        let mut final_input = input.clone();
                        
                        // [New] useGeneralengine modificationsParameterType (replace the previous hardEncode的 shell Toolfix logic)
                        if let Some(original_schema) = tool_name_to_schema.get(name) {
                            crate::proxy::common::json_schema::fix_tool_call_args(&mut final_input, original_schema);
                        }

                        let mut part = json!({
                            "functionCall": {
                                "name": name,
                                "args": final_input,
                                "id": id
                            }
                        });
                        saw_non_thinking = true;

                        // Track pending tool use
                        if is_assistant {
                            pending_tool_use_ids.push(id.clone());
                        }

                        // [New] recursive cleanupParameter中Mayillegality of existenceValidation field
                        crate::proxy::common::json_schema::clean_json_schema(&mut part);

                        // storage id -> name Mapping
                        tool_id_to_name.insert(id.clone(), name.clone());

                        // Signature resolution logic
                        // Priority: Client -> Context -> Session Cache -> Tool Cache -> Global Store (deprecated)
                        // [CRITICAL FIX] Do NOT use skip_thought_signature_validator for Vertex AI
                        // Vertex AI rejects this sentinel value, so we only add thoughtSignature if we have a real one
                        let final_sig = signature.as_ref()
                            .or(last_thought_signature.as_ref())
                            .cloned()
                            .or_else(|| {
                                // [NEW v3.3.17] Try session-based signature cache first (Layer 3)
                                // This provides conversation-level isolation
                                crate::proxy::SignatureCache::global().get_session_signature(session_id)
                                    .map(|s| {
                                        tracing::info!(
                                            "[Claude-Request] Recovered signature from SESSION cache (session: {}, len: {})",
                                            session_id, s.len()
                                        );
                                        s
                                    })
                            })
                            .or_else(|| {
                                // Try tool-specific signature cache (Layer 1)
                                crate::proxy::SignatureCache::global().get_tool_signature(id)
                                    .map(|s| {
                                        tracing::info!("[Claude-Request] Recovered signature from TOOL cache for tool_id: {}", id);
                                        s
                                    })
                            })
                            .or_else(|| {
                                // [DEPRECATED] Global store fallback - kept for backward compatibility
                                let global_sig = get_thought_signature();
                                if global_sig.is_some() {
                                    tracing::warn!(
                                        "[Claude-Request] Using deprecated GLOBAL thought_signature fallback (length: {}). \
                                         This indicates session cache miss.",
                                        global_sig.as_ref().unwrap().len()
                                    );
                                }
                                global_sig
                            });
                        // [FIX #752] Validate signature before using
                        // Only add thoughtSignature if we have a valid and compatible one
                        if let Some(sig) = final_sig {
                            // [NEW] If this is a retry, do NOT backfill signatures to avoid issues.
                            if is_retry && signature.is_none() {
                                tracing::warn!("[Tool-Signature] Skipping signature backfill for tool_use: {} during retry.", id);
                            } else {
                                // Check signature length
                                if sig.len() >= MIN_SIGNATURE_LENGTH {
                                    // Check signature compatibility (optional for tool_use)
                                    let cached_family = crate::proxy::SignatureCache::global()
                                        .get_signature_family(&sig);

                                    let should_use_sig = match cached_family {
                                        Some(family) => {
                                            // For tool_use, check compatibility
                                            if is_model_compatible(&family, mapped_model) {
                                                true
                                            } else {
                                                tracing::warn!(
                                                    "[Tool-Signature] Incompatible signature for tool_use: {} (Family: {}, Target: {})",
                                                    id, family, mapped_model
                                                );
                                                false
                                            }
                                        }
                                        None => {
                                            // Unknown origin: only use in non-thinking mode
                                            if is_thinking_enabled {
                                                tracing::warn!(
                                                    "[Tool-Signature] Unknown signature origin for tool_use: {} (len: {}). Dropping in thinking mode.",
                                                    id, sig.len()
                                                );
                                                false
                                            } else {
                                                // In non-thinking mode, allow unknown signatures
                                                true
                                            }
                                        }
                                    };
                                    if should_use_sig {
                                        part["thoughtSignature"] = json!(sig);
                                    }
                                } else {
                                    tracing::warn!(
                                        "[Tool-Signature] Signature too short for tool_use: {} (len: {})",
                                        id, sig.len()
                                    );
                                }
                            }
                        } else {
                            // [NEW] Handle missing signature for Gemini thinking models
                            // Use skip_thought_signature_validator as a sentinel value
                            let is_google_cloud = mapped_model.starts_with("projects/");
                            if is_thinking_enabled && !is_google_cloud {
                                tracing::debug!("[Tool-Signature] Adding GEMINI_SKIP_SIGNATURE for tool_use: {}", id);
                                part["thoughtSignature"] = json!("skip_thought_signature_validator");
                            }
                        }
                        parts.push(part);
                    }
                    ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                        ..
                    } => {
                        // Mark this tool ID as resolved in this turn
                        current_turn_tool_result_ids.insert(tool_use_id.clone());
                        // priorityUsingBeforeRecord的 name，Else用 tool_use_id
                        let func_name = tool_id_to_name
                            .get(tool_use_id)
                            .cloned()
                            .unwrap_or_else(|| tool_use_id.clone());

                        // [FIX #593] ToolOutputCompress: HandleExtra largeToolOutput
                        // UsingintelligentCompressStrategy(Browser snapshot、大FileHint等)
                        let mut compacted_content = content.clone();
                        if let Some(blocks) = compacted_content.as_array_mut() {
                            tool_result_compressor::sanitize_tool_result_blocks(blocks);
                        }

                        // Smart Truncation: strict image removal
                        // Remove all Base64 images from historical tool results to save context.
                        // Only allow text.
                        let mut merged_content = match &compacted_content {
                            serde_json::Value::String(s) => s.clone(),
                            serde_json::Value::Array(arr) => arr
                                .iter()
                                .filter_map(|block| {
                                    if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                                        Some(text.to_string())
                                    } else if block.get("source").is_some() {
                                        // If it's an image/document, replace with placeholder
                                        if block.get("type").and_then(|v| v.as_str()) == Some("image") {
                                            Some("[image omitted to save context]".to_string())
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join("\n"),
                            _ => content.to_string(),
                        };

                        // Smart Truncation: max chars limit
                        const MAX_TOOL_RESULT_CHARS: usize = 200_000;
                        if merged_content.len() > MAX_TOOL_RESULT_CHARS {
                            tracing::warn!("Truncating tool result from {} chars to {}", merged_content.len(), MAX_TOOL_RESULT_CHARS);
                            let mut truncated = merged_content.chars().take(MAX_TOOL_RESULT_CHARS).collect::<String>();
                            truncated.push_str("\n...[truncated output]");
                            merged_content = truncated;
                        }

                        // [Optimization] IfResult为Empty，Inject explicitConfirmSignal，preventModelhallucination
                        if merged_content.trim().is_empty() {
                            if is_error.unwrap_or(false) {
                                merged_content =
                                    "Tool execution failed with no output.".to_string();
                            } else {
                                merged_content = "Command executed successfully.".to_string();
                            }
                        }

                        parts.push(json!({
                            "functionResponse": {
                                "name": func_name,
                                "response": {"result": merged_content},
                                "id": tool_use_id
                            }
                        }));

                        // [FIX] Tool Result 也NeedbackfillSign（IfContextThere is）
                        if let Some(sig) = last_thought_signature.as_ref() {
                            if let Some(last_part) = parts.last_mut() {
                                last_part["thoughtSignature"] = json!(sig);
                            }
                        }

                        // markStatus，for next item User MessageDeduplication judgment
                        *previous_was_tool_result = true;
                    }
                    // ContentBlock::RedactedThinking handled above at line 583
                    ContentBlock::ServerToolUse { .. } | ContentBlock::WebSearchToolResult { .. } => {
                        // SearchResult block should not beClientSend back to upstream (Already by tool_result substitute)
                        continue;
                    }
                }
            }
        }
    }

    // If this is a User message, check if we need to inject missing tool results
    if !is_assistant && !pending_tool_use_ids.is_empty() {
        let missing_ids: Vec<_> = pending_tool_use_ids.iter()
            .filter(|id| !current_turn_tool_result_ids.contains(*id))
            .cloned()
            .collect();

        if !missing_ids.is_empty() {
            tracing::warn!("[Elastic-Recovery] Injecting {} missing tool results into User message (IDs: {:?})", missing_ids.len(), missing_ids);
            for id in missing_ids.iter().rev() { // Insert in reverse order to maintain order at index 0? No, just insert at 0.
                let name = tool_id_to_name.get(id).cloned().unwrap_or(id.clone());
                let synthetic_part = json!({
                    "functionResponse": {
                        "name": name,
                        "response": {
                            "result": "Tool execution interrupted. No result provided."
                        },
                        "id": id
                    }
                });
                // Prepend to ensure they are present before any text
                parts.insert(0, synthetic_part);
            }
        }
        // All pending IDs are now handled (either present or injected)
        pending_tool_use_ids.clear();
    }

    // Fix for "Thinking enabled, assistant message must start with thinking block" 400 error
    // [Optimization] Apply this to ALL assistant messages in history, not just the last one.
    // Vertex AI requires every assistant message to start with a thinking block when thinking is enabled.
    if allow_dummy_thought && is_assistant && is_thinking_enabled {
        let has_thought_part = parts
            .iter()
            .any(|p| {
                p.get("thought").and_then(|v| v.as_bool()).unwrap_or(false)
                    || p.get("thoughtSignature").is_some()
                    || p.get("thought").and_then(|v| v.as_str()).is_some() // in some casesMayYes text + thought: true combination
            });

        if !has_thought_part {
            // Prepend a dummy thinking block to satisfy Gemini v1internal requirements
            parts.insert(
                0,
                json!({
                    "text": "Thinking...",
                    "thought": true
                }),
            );
            tracing::debug!("Injected dummy thought block for historical assistant message at index {}", parts.len());
        } else {
            // [Crucial Check] Even if there is thought Block，也Mustensure it is located parts first place (Index 0)
            // 且MustPacket含 thought: true mark
            let first_is_thought = parts.get(0).map_or(false, |p| {
                (p.get("thought").is_some() || p.get("thoughtSignature").is_some())
                    && p.get("text").is_some() // for v1internal，generally text + thought: true 才YesCompliance thinkingBlock
            });

            if !first_is_thought {
                // IfThe first item does not fit the thinkingBlockfeature，Force one to be added
                parts.insert(
                    0,
                    json!({
                        "text": "...",
                        "thought": true
                    }),
                );
                tracing::debug!("First part of model message at {} is not a valid thought block. Prepending dummy.", parts.len());
            } else {
                // Make sure the first itemPacketContained thought: true (preventOnly signature situation)
                if let Some(p0) = parts.get_mut(0) {
                    if p0.get("thought").is_none() {
                        p0.as_object_mut().map(|obj| obj.insert("thought".to_string(), json!(true)));
                    }
                }
            }
        }
    }

    Ok(parts)
}

/// build Contents (Messages)
fn build_google_content(
    msg: &Message,
    claude_req: &ClaudeRequest,
    is_thinking_enabled: bool,
    session_id: &str,
    allow_dummy_thought: bool,
    is_retry: bool,
    tool_id_to_name: &mut HashMap<String, String>,
    tool_name_to_schema: &HashMap<String, Value>,
    mapped_model: &str,
    last_thought_signature: &mut Option<String>,
    pending_tool_use_ids: &mut Vec<String>,
    last_user_task_text_normalized: &mut Option<String>,
    previous_was_tool_result: &mut bool,
    existing_tool_result_ids: &std::collections::HashSet<String>,
) -> Result<Value, String> {
    let role = if msg.role == "assistant" {
        "model"
    } else {
        &msg.role
    };

    // Proactive Tool Chain Repair:
    // If we are about to process an Assistant message, but we still have pending tool_use_ids,
    // it means the previous turn was interrupted or the user ignored the tool.
    // We MUST inject a synthetic User message with error results to close the loop.
    if role == "model" && !pending_tool_use_ids.is_empty() {
        tracing::warn!("[Elastic-Recovery] Detected interrupted tool chain (Assistant -> Assistant). Injecting synthetic User message for IDs: {:?}", pending_tool_use_ids);

        let synthetic_parts: Vec<serde_json::Value> = pending_tool_use_ids.iter()
            .filter(|id| !existing_tool_result_ids.contains(*id)) // [FIX #632] Only inject if ID is truly missing
            .map(|id| {
                let name = tool_id_to_name.get(id).cloned().unwrap_or(id.clone());
                json!({
                    "functionResponse": {
                        "name": name,
                        "response": {
                            "result": "Tool execution interrupted. No result provided."
                        },
                        "id": id
                    }
                })
            }).collect();

        if !synthetic_parts.is_empty() {
            return Ok(json!({
                "role": "user",
                "parts": synthetic_parts
            }));
        }
        // Clear pending IDs as we have handled them
        pending_tool_use_ids.clear();
    }

    let parts = build_contents(
        &msg.content,
        msg.role == "assistant",
        claude_req,
        is_thinking_enabled,
        session_id,
        allow_dummy_thought,
        is_retry,
        tool_id_to_name,
        tool_name_to_schema,
        mapped_model,
        last_thought_signature,
        pending_tool_use_ids,
        last_user_task_text_normalized,
        previous_was_tool_result,
        existing_tool_result_ids,
    )?;

    if parts.is_empty() {
        return Ok(json!(null)); // Indicate no content to add
    }

    Ok(json!({
        "role": role,
        "parts": parts
    }))
}

/// build Contents (Messages)
fn build_google_contents(
    messages: &[Message],
    claude_req: &ClaudeRequest,
    tool_id_to_name: &mut HashMap<String, String>,
    tool_name_to_schema: &HashMap<String, Value>,
    is_thinking_enabled: bool,
    allow_dummy_thought: bool,
    mapped_model: &str,
    session_id: &str, // [NEW v3.3.17] Session ID for signature caching
    is_retry: bool,
) -> Result<Value, String> {
    let mut contents = Vec::new();
    let mut last_thought_signature: Option<String> = None;
    let mut _accumulated_usage: Option<Value> = None;
    // Track pending tool_use IDs for recovery
    let mut pending_tool_use_ids: Vec<String> = Vec::new();

    // [NEW] used to identify andFilter Claude Code DuplicateechoedTaskinstruction
    let mut last_user_task_text_normalized: Option<String> = None;
    let mut previous_was_tool_result = false;

    let _msg_count = messages.len();

    // [FIX #632] Pre-scan all messages to identify all tool_result IDs that ALREADY exist in the conversation.
    // This prevents Elastic-Recovery from injecting duplicate results if they are present later in the chain.
    let mut existing_tool_result_ids = std::collections::HashSet::new();
    for msg in messages {
        if let MessageContent::Array(blocks) = &msg.content {
            for block in blocks {
                if let ContentBlock::ToolResult { tool_use_id, .. } = block {
                    existing_tool_result_ids.insert(tool_use_id.clone());
                }
            }
        }
    }

    for (_i, msg) in messages.iter().enumerate() {
        let google_content = build_google_content(
            msg,
            claude_req,
            is_thinking_enabled,
            session_id,
            allow_dummy_thought,
            is_retry,
            tool_id_to_name,
            tool_name_to_schema,
            mapped_model,
            &mut last_thought_signature,
            &mut pending_tool_use_ids,
            &mut last_user_task_text_normalized,
            &mut previous_was_tool_result,
            &existing_tool_result_ids,
        )?;

        if !google_content.is_null() {
            contents.push(google_content);
        }
    }

    // [Removed] ensure_last_assistant_has_thinking
    // Corrupted signature issues proved we cannot fake thinking blocks.
    // Instead we rely on should_disable_thinking_due_to_history to prevent this state.

    // [FIX P3-3] Strict Role Alternation (Message Merging)
    // Merge adjacent messages with the same role to satisfy Gemini's strict alternation rule
    let mut merged_contents = merge_adjacent_roles(contents);

    // [FIX P3-4] Deep "Un-thinking" Cleanup
    // If thinking is disabled (e.g. smart downgrade), recursively remove any stray 'thought'/'thoughtSignature'
    // This is critical because converting Thinking->Text isn't enough; metadata must be gone.
    if !is_thinking_enabled {
        for msg in &mut merged_contents {
            clean_thinking_fields_recursive(msg);
        }
    }

    Ok(json!(merged_contents))
}

/// Merge adjacent messages with the same role
fn merge_adjacent_roles(mut contents: Vec<Value>) -> Vec<Value> {
    if contents.is_empty() {
        return contents;
    }

    let mut merged = Vec::new();
    let mut current_msg = contents.remove(0);

    for msg in contents {
        let current_role = current_msg["role"].as_str().unwrap_or_default();
        let next_role = msg["role"].as_str().unwrap_or_default();

        if current_role == next_role {
            // Merge parts
            if let Some(current_parts) = current_msg.get_mut("parts").and_then(|p| p.as_array_mut()) {
                if let Some(next_parts) = msg.get("parts").and_then(|p| p.as_array()) {
                    current_parts.extend(next_parts.clone());

                    // [FIX #709] Core Fix: After merging parts from adjacent messages,
                    // we must RE-SORT them to ensure any thinking blocks from the
                    // second message are moved to the very front of the combined array.
                    reorder_gemini_parts(current_parts);
                }
            }
        } else {
            merged.push(current_msg);
            current_msg = msg;
        }
    }
    merged.push(current_msg);
    merged
}

/// build Tools
fn build_tools(tools: &Option<Vec<Tool>>, has_web_search: bool) -> Result<Option<Value>, String> {
    if let Some(tools_list) = tools {
        let mut function_declarations: Vec<Value> = Vec::new();
        let mut has_google_search = has_web_search;

        for tool in tools_list {
            // 1. Detect server tools / built-in tools like web_search
            if tool.is_web_search() {
                has_google_search = true;
                continue;
            }

            if let Some(t_type) = &tool.type_ {
                if t_type == "web_search_20250305" {
                    has_google_search = true;
                    continue;
                }
            }

            // 2. Detect by name
            if let Some(name) = &tool.name {
                if name == "web_search" || name == "google_search" {
                    has_google_search = true;
                    continue;
                }

                // 3. Client tools require input_schema
                let mut input_schema = tool.input_schema.clone().unwrap_or(json!({
                    "type": "object",
                    "properties": {}
                }));
                crate::proxy::common::json_schema::clean_json_schema(&mut input_schema);

                function_declarations.push(json!({
                    "name": name,
                    "description": tool.description,
                    "parameters": input_schema
                }));
            }
        }

        let mut tool_obj = serde_json::Map::new();

        // [repair] solve "Multiple tools are supported only when they are all search tools" 400 Error
        // principle：Gemini v1internal Interfacevery picky，generallyNot allowedin the sameToolmixed in definition Google Search 和 Function Declarations。
        // for Claude CLI Waiting to be carried MCP Tool的Client，Mustpriority guarantee Function Declarations working normally。
        if !function_declarations.is_empty() {
            // IfThere is localTool，then onlyUsinglocalTool，give up injected Google Search
            tool_obj.insert("functionDeclarations".to_string(), json!(function_declarations));

            // [IMPROVED] Recordjump over googleSearch Reason for injection
            if has_google_search {
                tracing::info!(
                    "[Claude-Request] Skipping googleSearch injection due to {} existing function declarations. \
                     Gemini v1internal does not support mixed tool types.",
                    function_declarations.len()
                );
            }
        } else if has_google_search {
            // Only在NonelocalTool时，only allowed to inject Google Search
            tool_obj.insert("googleSearch".to_string(), json!({}));
        }

        if !tool_obj.is_empty() {
            return Ok(Some(json!([tool_obj])));
        }
    }

    Ok(None)
}

/// build Generation Config
fn build_generation_config(
    claude_req: &ClaudeRequest,
    has_web_search: bool,
    is_thinking_enabled: bool
) -> Value {
    let mut config = json!({});

    // Thinking Config
    if let Some(thinking) = &claude_req.thinking {
        // [New Check] Must is_thinking_enabled 为TrueJust generated thinkingConfig
        if thinking.type_ == "enabled" && is_thinking_enabled {
            let mut thinking_config = json!({"includeThoughts": true});

            if let Some(budget_tokens) = thinking.budget_tokens {
                let mut budget = budget_tokens;
                // gemini-2.5-flash upper limit 24576
                let is_flash_model =
                    has_web_search || claude_req.model.contains("gemini-2.5-flash");
                if is_flash_model {
                    budget = budget.min(24576);
                }
                thinking_config["thinkingBudget"] = json!(budget);
            }

            config["thinkingConfig"] = thinking_config;
        }
    }

    // otherParameter
    if let Some(temp) = claude_req.temperature {
        config["temperature"] = json!(temp);
    }
    if let Some(top_p) = claude_req.top_p {
        config["topP"] = json!(top_p);
    }
    if let Some(top_k) = claude_req.top_k {
        config["topK"] = json!(top_k);
    }

    // Effort level mapping (Claude API v2.0.67+)
    // Maps Claude's output_config.effort to Gemini's effortLevel
    if let Some(output_config) = &claude_req.output_config {
        if let Some(effort) = &output_config.effort {
            config["effortLevel"] = json!(match effort.to_lowercase().as_str() {
                "high" => "HIGH",
                "medium" => "MEDIUM",
                "low" => "LOW",
                _ => "HIGH" // Default to HIGH for unknown values
            });
            tracing::debug!(
                "[Generation-Config] Effort level set: {} -> {}",
                effort,
                config["effortLevel"]
            );
        }
    }

    // web_search force candidateCount=1
    /*if has_web_search {
        config["candidateCount"] = json!(1);
    }*/

    // max_tokens Mapping为 maxOutputTokens
    // Respect client-provided max_tokens (aligns with OpenAI mapper behavior)
    let mut final_max_tokens: i64 = claude_req.max_tokens.map(|t| t as i64).unwrap_or(16384);
    
    // [NEW] make sure maxOutputTokens greater than thinkingBudget (API Strong constraints)
    if let Some(thinking_config) = config.get("thinkingConfig") {
        if let Some(budget) = thinking_config.get("thinkingBudget").and_then(|t| t.as_u64()) {
            if final_max_tokens <= budget as i64 {
                final_max_tokens = (budget + 8192) as i64;
                tracing::info!(
                    "[Generation-Config] Bumping maxOutputTokens to {} due to thinking budget of {}", 
                    final_max_tokens, budget
                );
            }
        }
    }
    
    config["maxOutputTokens"] = json!(final_max_tokens);

    // [Optimization] SetGlobalStopSequence,preventModelhallucinateConversationmark
    // Notice: 不Packet含 "[DONE]" YesBecause:
    //   1. "[DONE]" Yes SSE ProtocolstandardsEndmark,in code/Documentoften appear in
    //   2. treat it as stopSequence will lead toModelOutputaccidentally truncated (as explained SSE Protocol时)
    //   3. Gemini Stream的True正End由 finishReason Fieldcontrol,No dependencies required stopSequence
    //   4. SSE level "data: [DONE]" Already in mod.rs alone inHandle
    config["stopSequences"] = json!([
        "<|user|>",
        "<|end_of_turn|>",
        "\n\nHuman:"
    ]);

    config
}

/// Recursively remove 'thought' and 'thoughtSignature' fields
/// Used when downgrading thinking (e.g. during 400 retry)
pub fn clean_thinking_fields_recursive(val: &mut Value) {
    match val {
        Value::Object(map) => {
            map.remove("thought");
            map.remove("thoughtSignature");
            for (_, v) in map.iter_mut() {
                clean_thinking_fields_recursive(v);
            }
        }
        Value::Array(arr) => {
            for v in arr.iter_mut() {
                clean_thinking_fields_recursive(v);
            }
        }
        _ => {}
    }
}


/// Check if two model strings are compatible (same family)
fn is_model_compatible(cached: &str, target: &str) -> bool {
    // Simple heuristic: check if they share the same base prefix
    // e.g. "gemini-1.5-pro" vs "gemini-1.5-pro-002" -> Compatible
    // "gemini-1.5-pro" vs "gemini-2.0-flash" -> Incompatible
    
    // Normalize
    let c = cached.to_lowercase();
    let t = target.to_lowercase();
    
    if c == t { return true; }
    
    // Check specific families
    // Vertex AI signatures are very strict. 1.5-pro vs 1.5-flash are NOT cross-compatible.
    // 2.0-flash vs 2.0-pro are also NOT cross-compatible.
    
    // Exact model string match (already handled by c == t)
    
    // Grouped family match (Claude models are more permissive)
    if c.contains("claude-3-5") && t.contains("claude-3-5") { return true; }
    if c.contains("claude-3-7") && t.contains("claude-3-7") { return true; }
    
    // Gemini models: strict family match required for signatures
    if c.contains("gemini-1.5-pro") && t.contains("gemini-1.5-pro") { return true; }
    if c.contains("gemini-1.5-flash") && t.contains("gemini-1.5-flash") { return true; }
    if c.contains("gemini-2.0-flash") && t.contains("gemini-2.0-flash") { return true; }
    if c.contains("gemini-2.0-pro") && t.contains("gemini-2.0-pro") { return true; }
    
    // Fallback: strict match required
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proxy::common::json_schema::clean_json_schema;


    #[test]
    fn test_ephemeral_injection_debug() {
        // This test simulates the issue where cache_control might be injected
        let json_with_null = json!({
            "model": "claude-3-5-sonnet-20241022",
            "messages": [
                {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "thinking",
                            "thinking": "test",
                            "signature": "sig_1234567890",
                            "cache_control": null
                        }
                    ]
                }
            ]
        });

        let req: ClaudeRequest = serde_json::from_value(json_with_null).unwrap();
        if let MessageContent::Array(blocks) = &req.messages[0].content {
            if let ContentBlock::Thinking { cache_control, .. } = &blocks[0] {
                assert!(cache_control.is_none(), "Deserialization should result in None for null cache_control");
            }
        }
        
        // Now test serialization
        let serialized = serde_json::to_value(&req).unwrap();
        println!("Serialized: {}", serialized);
        assert!(serialized["messages"][0]["content"][0].get("cache_control").is_none());
    }

    #[test]
    fn test_simple_request() {
        let req = ClaudeRequest {
            model: "claude-sonnet-4-5".to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: MessageContent::String("Hello".to_string()),
            }],
            system: None,
            tools: None,
            stream: false,
            max_tokens: None,
            temperature: None,
            top_p: None,
            top_k: None,
            thinking: None,
            metadata: None,
            output_config: None,
            size: None,
            quality: None,
        };

        let result = transform_claude_request_in(&req, "test-project", false);
        assert!(result.is_ok());

        let body = result.unwrap();
        assert_eq!(body["project"], "test-project");
        assert!(body["requestId"].as_str().unwrap().starts_with("agent-"));
    }

    #[test]
    fn test_clean_json_schema() {
        let mut schema = json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "location": {
                    "type": "string",
                    "description": "The city and state, e.g. San Francisco, CA",
                    "minLength": 1,
                    "exclusiveMinimum": 0
                },
                "unit": {
                    "type": ["string", "null"],
                    "enum": ["celsius", "fahrenheit"],
                    "default": "celsius"
                },
                "date": {
                    "type": "string",
                    "format": "date"
                }
            },
            "required": ["location"]
        });

        clean_json_schema(&mut schema);

        // Check removed fields
        assert!(schema.get("$schema").is_none());
        assert!(schema.get("additionalProperties").is_none());
        assert!(schema["properties"]["location"].get("minLength").is_none());
        assert!(schema["properties"]["unit"].get("default").is_none());
        assert!(schema["properties"]["date"].get("format").is_none());

        // Check union type handling ["string", "null"] -> "string"
        assert_eq!(schema["properties"]["unit"]["type"], "string");

        // Check types are lowercased
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["properties"]["location"]["type"], "string");
        assert_eq!(schema["properties"]["date"]["type"], "string");
    }

    #[test]
    fn test_complex_tool_result() {
        let req = ClaudeRequest {
            model: "claude-3-5-sonnet-20241022".to_string(),
            messages: vec![
                Message {
                    role: "user".to_string(),
                    content: MessageContent::String("Run command".to_string()),
                },
                Message {
                    role: "assistant".to_string(),
                    content: MessageContent::Array(vec![
                        ContentBlock::ToolUse {
                            id: "call_1".to_string(),
                            name: "run_command".to_string(),
                            input: json!({"command": "ls"}),
                            signature: None,
                            cache_control: None,
                        }
                    ]),
                },
                Message {
                    role: "user".to_string(),
                    content: MessageContent::Array(vec![ContentBlock::ToolResult {
                        tool_use_id: "call_1".to_string(),
                        content: json!([
                            {"type": "text", "text": "file1.txt\n"},
                            {"type": "text", "text": "file2.txt"}
                        ]),
                        is_error: Some(false),
                    }]),
                },
            ],
            system: None,
            tools: None,
            stream: false,
            max_tokens: None,
            temperature: None,
            top_p: None,
            top_k: None,
            thinking: None,
            metadata: None,
            output_config: None,
            size: None,
            quality: None,
        };

        let result = transform_claude_request_in(&req, "test-project", false);
        assert!(result.is_ok());

        let body = result.unwrap();
        let contents = body["request"]["contents"].as_array().unwrap();

        // Check the tool result message (last message)
        let tool_resp_msg = &contents[2];
        let parts = tool_resp_msg["parts"].as_array().unwrap();
        let func_resp = &parts[0]["functionResponse"];

        assert_eq!(func_resp["name"], "run_command");
        assert_eq!(func_resp["id"], "call_1");

        // Verify merged content
        let resp_text = func_resp["response"]["result"].as_str().unwrap();
        assert!(resp_text.contains("file1.txt"));
        assert!(resp_text.contains("file2.txt"));
        assert!(resp_text.contains("\n"));
    }

    #[test]
    fn test_cache_control_cleanup() {
        // simulation VS Code PluginSend的Packet含 cache_control historyMessage
        let req = ClaudeRequest {
            model: "claude-sonnet-4-5".to_string(),
            messages: vec![
                Message {
                    role: "user".to_string(),
                    content: MessageContent::String("Hello".to_string()),
                },
                Message {
                    role: "assistant".to_string(),
                    content: MessageContent::Array(vec![
                        ContentBlock::Thinking {
                            thinking: "Let me think...".to_string(),
                            signature: Some("sig123".to_string()),
                            cache_control: Some(json!({"type": "ephemeral"})), // thisShouldbe cleared
                        },
                        ContentBlock::Text {
                            text: "Here is my response".to_string(),
                        },
                    ]),
                },
                Message {
                    role: "user".to_string(),
                    content: MessageContent::Array(vec![
                        ContentBlock::Image {
                            source: ImageSource {
                                source_type: "base64".to_string(),
                                media_type: "image/png".to_string(),
                                data: "iVBORw0KGgo=".to_string(),
                            },
                            cache_control: Some(json!({"type": "ephemeral"})), // This tooShouldbe cleared
                        },
                    ]),
                },
            ],
            system: None,
            tools: None,
            stream: false,
            max_tokens: None,
            temperature: None,
            top_p: None,
            top_k: None,
            thinking: None,
            metadata: None,
            output_config: None,
            size: None,
            quality: None,
        };

        let result = transform_claude_request_in(&req, "test-project", false);
        assert!(result.is_ok());

        // ValidateRequestSuccessConvert
        let body = result.unwrap();
        assert_eq!(body["project"], "test-project");
        
        // Notice: cache_control The cleanup occurs inInside,We cannot directly JSON OutputValidate
        // 但IfNoneclean up,Follow-upSend到 Anthropic API An error will be reported when
        // thisTestMainMake sure the cleanup logic doesn't result inConvertFailed
    }

    #[test]
    fn test_thinking_mode_auto_disable_on_tool_use_history() {
        // [scene] historyMessageone of themToolcall chain，且 Assistant MessageNone Thinking Block
        // expect: SystemautomaticFallback，Disable Thinking Mode，to avoid 400 Error
        let req = ClaudeRequest {
            model: "claude-sonnet-4-5".to_string(),
            messages: vec![
                Message {
                    role: "user".to_string(),
                    content: MessageContent::String("Check files".to_string()),
                },
                // Assistant UsingTool，But in Africa Thinking Mode下
                Message {
                    role: "assistant".to_string(),
                    content: MessageContent::Array(vec![
                        ContentBlock::Text {
                            text: "Checking...".to_string(),
                        },
                        ContentBlock::ToolUse {
                            id: "tool_1".to_string(),
                            name: "list_files".to_string(),
                            input: json!({}),
                            cache_control: None, 
                            signature: None 
                        },
                    ]),
                },
                // UserReturnToolResult
                Message {
                    role: "user".to_string(),
                    content: MessageContent::Array(vec![
                        ContentBlock::ToolResult {
                            tool_use_id: "tool_1".to_string(),
                            content: serde_json::Value::String("file1.txt\nfile2.txt".to_string()),
                            is_error: Some(false),
                            // cache_control: None, // removed
                        },
                    ]),
                },
            ],
            system: None,
            tools: Some(vec![
                Tool {
                    name: Some("list_files".to_string()),
                    description: Some("List files".to_string()),
                    input_schema: Some(json!({"type": "object"})),
                    type_: None,
                    // cache_control: None, // removed
                }
            ]),
            stream: false,
            max_tokens: None,
            temperature: None,
            top_p: None,
            top_k: None,
            thinking: Some(ThinkingConfig {
                type_: "enabled".to_string(),
                budget_tokens: Some(1024),
            }),
            metadata: None,
            output_config: None,
            size: None,
            quality: None,
        };

        let result = transform_claude_request_in(&req, "test-project", false);
        assert!(result.is_ok());

        let body = result.unwrap();
        let request = &body["request"];

        // Validate: generationConfig should not bePacket含 thinkingConfig (Because被Fallback了)
        // even thoughRequestclear inEnable了 thinking
        if let Some(gen_config) = request.get("generationConfig") {
             assert!(gen_config.get("thinkingConfig").is_none(), "thinkingConfig should be removed due to downgrade");
        }
        
        // Validate: can still be generatedValid的Request体
        assert!(request.get("contents").is_some());
    }



    #[test]
    fn test_thinking_block_not_prepend_when_disabled() {
        // ValidateWhen thinking 未Enable时,Will not complete thinking Block
        let req = ClaudeRequest {
            model: "claude-sonnet-4-5".to_string(),
            messages: vec![
                Message {
                    role: "user".to_string(),
                    content: MessageContent::String("Hello".to_string()),
                },
                Message {
                    role: "assistant".to_string(),
                    content: MessageContent::Array(vec![
                        ContentBlock::Text {
                            text: "Response".to_string(),
                        },
                    ]),
                },
            ],
            system: None,
            tools: None,
            stream: false,
            max_tokens: None,
            temperature: None,
            top_p: None,
            top_k: None,
            thinking: None, // 未Enable thinking
            metadata: None,
            output_config: None,
            size: None,
            quality: None,
        };

        let result = transform_claude_request_in(&req, "test-project", false);
        assert!(result.is_ok());

        let body = result.unwrap();
        let contents = body["request"]["contents"].as_array().unwrap();

        let last_model_msg = contents
            .iter()
            .rev()
            .find(|c| c["role"] == "model")
            .unwrap();

        let parts = last_model_msg["parts"].as_array().unwrap();
        
        // ValidateNoneComplete thinking Block
        assert_eq!(parts.len(), 1, "Should only have the original text block");
        assert_eq!(parts[0]["text"], "Response");
    }

    #[test]
    fn test_thinking_block_empty_content_fix() {
        // [scene] ClientSendgot oneContent为Empty的 thinking Block
        // expect: autofill "..."
        let req = ClaudeRequest {
            model: "claude-sonnet-4-5".to_string(),
            messages: vec![
                Message {
                    role: "assistant".to_string(),
                    content: MessageContent::Array(vec![
                        ContentBlock::Thinking {
                            thinking: "".to_string(), // EmptyContent
                            signature: Some("sig".to_string()),
                            cache_control: None,
                        },
                        ContentBlock::Text { text: "Hi".to_string() }
                    ]),
                },
            ],
            system: None,
            tools: None,
            stream: false,
            max_tokens: None,
            temperature: None,
            top_p: None,
            top_k: None,
            thinking: Some(ThinkingConfig {
                type_: "enabled".to_string(),
                budget_tokens: Some(1024),
            }),
            metadata: None,
            output_config: None,
            size: None,
            quality: None,
        };

        let result = transform_claude_request_in(&req, "test-project", false);
        assert!(result.is_ok(), "Transformation failed");
        let body = result.unwrap();
        let contents = body["request"]["contents"].as_array().unwrap();
        let parts = contents[0]["parts"].as_array().unwrap();
        
        // Validate thinking Block
        assert_eq!(parts[0]["text"], "...", "Empty thinking should be filled with ...");
        assert!(parts[0].get("thought").is_none(), "Empty thinking should be downgraded to text");
    }

    #[test]
    fn test_redacted_thinking_degradation() {
        // [scene] ClientPacket含 RedactedThinking
        // expect: Fallback为Normaltext，Without thought: true
        let req = ClaudeRequest {
            model: "claude-sonnet-4-5".to_string(),
            messages: vec![
                Message {
                    role: "assistant".to_string(),
                    content: MessageContent::Array(vec![
                        ContentBlock::RedactedThinking {
                            data: "some data".to_string(),
                        },
                         ContentBlock::Text { text: "Hi".to_string() }
                    ]),
                },
            ],
            system: None,
            tools: None,
            stream: false,
            max_tokens: None,
            temperature: None,
            top_p: None,
            top_k: None,
            thinking: None,
            metadata: None,
            output_config: None,
            size: None,
            quality: None,
        };

        let result = transform_claude_request_in(&req, "test-project", false);
        assert!(result.is_ok());
        let body = result.unwrap();
        let parts = body["request"]["contents"][0]["parts"].as_array().unwrap();

        // Validate RedactedThinking -> Text
        let text = parts[0]["text"].as_str().unwrap();
        assert!(text.contains("[Redacted Thinking: some data]"));
        assert!(parts[0].get("thought").is_none(), "Redacted thinking should NOT have thought: true");
    }

    // ==================================================================================
    // [FIX #564] Test: Thinking blocks are sorted to be first after context compression
    // ==================================================================================
    #[test]
    fn test_thinking_blocks_sorted_first_after_compression() {
        // Simulate kilo context compression reordering: text BEFORE thinking
        let mut messages = vec![
            Message {
                role: "assistant".to_string(),
                content: MessageContent::Array(vec![
                    // Wrong order: Text before Thinking (simulates kilo compression)
                    ContentBlock::Text { text: "Some regular text".to_string() },
                    ContentBlock::Thinking { 
                        thinking: "My thinking process".to_string(),
                        signature: Some("valid_signature_1234567890_abcdefghij_klmnopqrstuvwxyz_test".to_string()),
                        cache_control: None,
                    },
                    ContentBlock::Text { text: "More text".to_string() },
                ]),
            }
        ];
        
        // Apply the fix
        sort_thinking_blocks_first(&mut messages);
        
        // Verify thinking is now first
        if let MessageContent::Array(blocks) = &messages[0].content {
            assert_eq!(blocks.len(), 3, "Should still have 3 blocks");
            assert!(matches!(blocks[0], ContentBlock::Thinking { .. }), "Thinking should be first");
            assert!(matches!(blocks[1], ContentBlock::Text { .. }), "Text should be second");
            assert!(matches!(blocks[2], ContentBlock::Text { .. }), "Text should be third");
            
            // Verify content preserved
            if let ContentBlock::Thinking { thinking, .. } = &blocks[0] {
                assert_eq!(thinking, "My thinking process");
            }
        } else {
            panic!("Expected Array content");
        }
    }

    #[test]
    fn test_thinking_blocks_no_reorder_when_already_first() {
        // Correct order: Thinking already first - should not trigger reorder
        let mut messages = vec![
            Message {
                role: "assistant".to_string(),
                content: MessageContent::Array(vec![
                    ContentBlock::Thinking { 
                        thinking: "My thinking".to_string(),
                        signature: Some("sig123".to_string()),
                        cache_control: None,
                    },
                    ContentBlock::Text { text: "Some text".to_string() },
                ]),
            }
        ];
        
        // Apply the fix (should be no-op)
        sort_thinking_blocks_first(&mut messages);
        
        // Verify order unchanged
        if let MessageContent::Array(blocks) = &messages[0].content {
            assert!(matches!(blocks[0], ContentBlock::Thinking { .. }), "Thinking should still be first");
            assert!(matches!(blocks[1], ContentBlock::Text { .. }), "Text should still be second");
        }
    }

    #[test]
    fn test_merge_consecutive_messages() {
        let mut messages = vec![
            Message {
                role: "user".to_string(),
                content: MessageContent::String("Hello".to_string()),
            },
            Message {
                role: "user".to_string(),
                content: MessageContent::Array(vec![
                    ContentBlock::Text { text: "World".to_string() }
                ]),
            },
            Message {
                role: "assistant".to_string(),
                content: MessageContent::String("Hi".to_string()),
            },
            Message {
                role: "user".to_string(),
                content: MessageContent::Array(vec![
                    ContentBlock::ToolResult {
                        tool_use_id: "test_id".to_string(),
                        content: serde_json::json!("result"),
                        is_error: None,
                    }
                ]),
            },
            Message {
                role: "user".to_string(),
                content: MessageContent::Array(vec![
                    ContentBlock::Text { text: "System Reminder".to_string() }
                ]),
            },
        ];

        merge_consecutive_messages(&mut messages);

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, "user");
        if let MessageContent::Array(blocks) = &messages[0].content {
            assert_eq!(blocks.len(), 2);
            match &blocks[0] {
                ContentBlock::Text { text } => assert_eq!(text, "Hello"),
                _ => panic!("Expected text block"),
            }
            match &blocks[1] {
                ContentBlock::Text { text } => assert_eq!(text, "World"),
                _ => panic!("Expected text block"),
            }
        } else {
            panic!("Expected array content at index 0");
        }

        assert_eq!(messages[1].role, "assistant");

        assert_eq!(messages[2].role, "user");
        if let MessageContent::Array(blocks) = &messages[2].content {
            assert_eq!(blocks.len(), 2);
            match &blocks[0] {
                ContentBlock::ToolResult { tool_use_id, .. } => assert_eq!(tool_use_id, "test_id"),
                _ => panic!("Expected tool_result block"),
            }
            match &blocks[1] {
                ContentBlock::Text { text } => assert_eq!(text, "System Reminder"),
                _ => panic!("Expected text block"),
            }
        } else {
            panic!("Expected array content at index 2");
        }
    }
}

