// Common utilities for request mapping across all protocols
// Provides unified grounding/networking logic

use serde_json::{json, Value};

/// Request configuration after grounding resolution
#[derive(Debug, Clone)]
pub struct RequestConfig {
    /// The request type: "agent", "web_search", or "image_gen"
    pub request_type: String,
    /// Whether to inject the googleSearch tool
    pub inject_google_search: bool,
    /// The final model name (with suffixes stripped)
    pub final_model: String,
    /// Image generation configuration (if request_type is image_gen)
    pub image_config: Option<Value>,
}

pub fn resolve_request_config(
    original_model: &str,
    mapped_model: &str,
    tools: &Option<Vec<Value>>,
    size: Option<&str>,    // [NEW] Image size parameter
    quality: Option<&str>, // [NEW] Image quality parameter
) -> RequestConfig {
    // 1. Image Generation Check (Priority)
    if mapped_model.starts_with("gemini-3-pro-image") {
        // [MODIFIED] Use unified parameter parsing function
        let (image_config, parsed_base_model) =
            parse_image_config_with_params(original_model, size, quality);

        return RequestConfig {
            request_type: "image_gen".to_string(),
            inject_google_search: false,
            final_model: parsed_base_model,
            image_config: Some(image_config),
        };
    }

    // DetectionYesNoHave internet connectionTooldefinition (built-inFunctioncall)
    let has_networking_tool = detects_networking_tool(tools);
    // DetectionYesNoPacketIncluding non-internetTool (如 MCP localTool)
    let _has_non_networking = contains_non_networking_tool(tools);

    // Strip -online suffix from original model if present (to detect networking intent)
    let is_online_suffix = original_model.ends_with("-online");

    // High-quality grounding allowlist (Only for models known to support search and be relatively 'safe')
    let _is_high_quality_model = mapped_model == "gemini-2.5-flash"
        || mapped_model == "gemini-1.5-pro"
        || mapped_model.starts_with("gemini-1.5-pro-")
        || mapped_model.starts_with("gemini-2.5-flash-")
        || mapped_model.starts_with("gemini-2.0-flash")
        || mapped_model.starts_with("gemini-3-")
        || mapped_model.contains("claude-3-5-sonnet")
        || mapped_model.contains("claude-3-opus")
        || mapped_model.contains("claude-sonnet")
        || mapped_model.contains("claude-opus")
        || mapped_model.contains("claude-4");

    // Determine if we should enable networking
    // [FIX] Disablebased onModelautomatic networking logic，preventPictureRequestBeing networkedSearchResultcover。
    // only inUserexplicitRequestWhen connected to the InternetEnable：1) -online Suffix 2) Portable networkingTooldefinition
    let enable_networking = is_online_suffix || has_networking_tool;

    // The final model to send upstream should be the MAPPED model,
    // but if searching, we MUST ensure the model name is one the backend associates with search.
    // Force a stable search model for search requests.
    let mut final_model = mapped_model.trim_end_matches("-online").to_string();

    // [FIX] Map logic aliases back to physical model names for upstream compatibility
    final_model = match final_model.as_str() {
        "gemini-3-pro-preview" => "gemini-3-pro-high".to_string(), // Preview maps back to High
        "gemini-3-pro-image-preview" => "gemini-3-pro-image".to_string(),
        "gemini-3-flash-preview" => "gemini-3-flash".to_string(),
        _ => final_model,
    };

    if enable_networking {
        // [FIX] Only gemini-2.5-flash supports googleSearch tool
        // All other models (including Gemini 3 Pro, thinking models, Claude aliases) must downgrade
        if final_model != "gemini-2.5-flash" {
            tracing::info!(
                "[Common-Utils] Downgrading {} to gemini-2.5-flash for web search (only gemini-2.5-flash supports googleSearch)",
                final_model
            );
            final_model = "gemini-2.5-flash".to_string();
        }
    }

    RequestConfig {
        request_type: if enable_networking {
            "web_search".to_string()
        } else {
            "agent".to_string()
        },
        inject_google_search: enable_networking,
        final_model,
        image_config: None,
    }
}

/// Legacy wrapper for backward compatibility and simple usage
#[allow(dead_code)]
pub fn parse_image_config(model_name: &str) -> (Value, String) {
    parse_image_config_with_params(model_name, None, None)
}

/// Extended version that accepts OpenAI size and quality parameters
///
/// This function supports parsing image configuration from:
/// 1. OpenAI API parameters (size, quality) - takes priority
/// 2. Model name suffixes (e.g., -16x9, -4k) - fallback for backward compatibility
///
/// # Arguments
/// * `model_name` - The model name (may contain suffixes like -16x9-4k)
/// * `size` - Optional OpenAI size parameter (e.g., "1280x720", "1792x1024")
/// * `quality` - Optional OpenAI quality parameter ("standard", "hd", "medium")
///
/// # Returns
/// (image_config, clean_model_name) where image_config contains aspectRatio and optionally imageSize
pub fn parse_image_config_with_params(
    model_name: &str,
    size: Option<&str>,
    quality: Option<&str>,
) -> (Value, String) {
    let mut aspect_ratio = "1:1";

    // 1. Prioritize from size ParameterParseaspect ratio
    if let Some(s) = size {
        aspect_ratio = calculate_aspect_ratio_from_size(s);
    } else {
        // 2. Fallback toModelSuffixParse（keep backwardsCompatible）
        if model_name.contains("-21x9") || model_name.contains("-21-9") {
            aspect_ratio = "21:9";
        } else if model_name.contains("-16x9") || model_name.contains("-16-9") {
            aspect_ratio = "16:9";
        } else if model_name.contains("-9x16") || model_name.contains("-9-16") {
            aspect_ratio = "9:16";
        } else if model_name.contains("-4x3") || model_name.contains("-4-3") {
            aspect_ratio = "4:3";
        } else if model_name.contains("-3x4") || model_name.contains("-3-4") {
            aspect_ratio = "3:4";
        } else if model_name.contains("-1x1") || model_name.contains("-1-1") {
            aspect_ratio = "1:1";
        }
    }

    let mut config = serde_json::Map::new();
    config.insert("aspectRatio".to_string(), json!(aspect_ratio));

    // 3. Prioritize from quality ParameterParseresolution
    if let Some(q) = quality {
        match q {
            "hd" => {
                config.insert("imageSize".to_string(), json!("4K"));
            }
            "medium" => {
                config.insert("imageSize".to_string(), json!("2K"));
            }
            _ => {} // "standard" or other，不Set
        }
    } else {
        // 4. Fallback toModelSuffixParse（keep backwardsCompatible）
        let is_hd = model_name.contains("-4k") || model_name.contains("-hd");
        let is_2k = model_name.contains("-2k");

        if is_hd {
            config.insert("imageSize".to_string(), json!("4K"));
        } else if is_2k {
            config.insert("imageSize".to_string(), json!("2K"));
        }
    }

    // The upstream model must be EXACTLY "gemini-3-pro-image"
    (
        serde_json::Value::Object(config),
        "gemini-3-pro-image".to_string(),
    )
}

/// DynamicCalculate aspect ratio（solve hardEncodequestion）
///
/// 从 "WIDTHxHEIGHT" FormatstringParseand calculate the aspect ratio，
/// UsingTolerances match common standard proportions。
///
/// # Arguments
/// * `size` - size string，Format为 "WIDTHxHEIGHT" (e.g., "1280x720", "1792x1024")
///
/// # Returns
/// Standard aspect ratio string ("1:1", "16:9", "9:16", "4:3", "3:4", "21:9")
fn calculate_aspect_ratio_from_size(size: &str) -> &'static str {
    if let Some((w_str, h_str)) = size.split_once('x') {
        if let (Ok(width), Ok(height)) = (w_str.parse::<f64>(), h_str.parse::<f64>()) {
            if width > 0.0 && height > 0.0 {
                let ratio = width / height;

                // Tolerance Matches Common Ratios（Tolerance 0.1）
                if (ratio - 21.0 / 9.0).abs() < 0.1 {
                    return "21:9";
                }
                if (ratio - 16.0 / 9.0).abs() < 0.1 {
                    return "16:9";
                }
                if (ratio - 4.0 / 3.0).abs() < 0.1 {
                    return "4:3";
                }
                if (ratio - 3.0 / 4.0).abs() < 0.1 {
                    return "3:4";
                }
                if (ratio - 9.0 / 16.0).abs() < 0.1 {
                    return "9:16";
                }
                if (ratio - 1.0).abs() < 0.1 {
                    return "1:1";
                }
            }
        }
    }

    "1:1" // Defaultrollback
}

/// Inject current googleSearch tool and ensure no duplicate legacy search tools
pub fn inject_google_search_tool(body: &mut Value) {
    if let Some(obj) = body.as_object_mut() {
        let tools_entry = obj.entry("tools").or_insert_with(|| json!([]));
        if let Some(tools_arr) = tools_entry.as_array_mut() {
            // [SafetyChecksum] IfArray中AlreadyPacket含 functionDeclarations，Injection is strictly prohibited googleSearch
            // Because Gemini v1internal Not supportedat onceRequestmedium mixed use search 和 functions
            let has_functions = tools_arr.iter().any(|t| {
                t.as_object()
                    .map_or(false, |o| o.contains_key("functionDeclarations"))
            });

            if has_functions {
                tracing::debug!(
                    "Skipping googleSearch injection due to existing functionDeclarations"
                );
                return;
            }

            // Firstclean upAlready exists的 googleSearch 或 googleSearchRetrieval，Just in caseDuplicateconflict
            tools_arr.retain(|t| {
                if let Some(o) = t.as_object() {
                    !(o.contains_key("googleSearch") || o.contains_key("googleSearchRetrieval"))
                } else {
                    true
                }
            });

            // Inject a unified googleSearch (v1internal specification)
            tools_arr.push(json!({
                "googleSearch": {}
            }));
        }
    }
}

/// Depthiterative cleanupClientSend的 [undefined] dirty string，prevent Gemini InterfaceChecksumFailed
pub fn deep_clean_undefined(value: &mut Value) {
    match value {
        Value::Object(map) => {
            // RemoveValue为 "[undefined]" 的Key
            map.retain(|_, v| {
                if let Some(s) = v.as_str() {
                    s != "[undefined]"
                } else {
                    true
                }
            });
            // Recursive processingNested
            for v in map.values_mut() {
                deep_clean_undefined(v);
            }
        }
        Value::Array(arr) => {
            for v in arr.iter_mut() {
                deep_clean_undefined(v);
            }
        }
        _ => {}
    }
}

/// Detects if the tool list contains a request for networking/web search.
/// Supported keywords: "web_search", "google_search", "web_search_20250305"
pub fn detects_networking_tool(tools: &Option<Vec<Value>>) -> bool {
    if let Some(list) = tools {
        for tool in list {
            // 1. straight hair style (Claude/Simple OpenAI/Anthropic Builtin/Vertex): { "name": "..." } 或 { "type": "..." }
            if let Some(n) = tool.get("name").and_then(|v| v.as_str()) {
                if n == "web_search"
                    || n == "google_search"
                    || n == "web_search_20250305"
                    || n == "google_search_retrieval"
                {
                    return true;
                }
            }

            if let Some(t) = tool.get("type").and_then(|v| v.as_str()) {
                if t == "web_search_20250305"
                    || t == "google_search"
                    || t == "web_search"
                    || t == "google_search_retrieval"
                {
                    return true;
                }
            }

            // 2. OpenAI Nested style: { "type": "function", "function": { "name": "..." } }
            if let Some(func) = tool.get("function") {
                if let Some(n) = func.get("name").and_then(|v| v.as_str()) {
                    let keywords = [
                        "web_search",
                        "google_search",
                        "web_search_20250305",
                        "google_search_retrieval",
                    ];
                    if keywords.contains(&n) {
                        return true;
                    }
                }
            }

            // 3. Gemini native style: { "functionDeclarations": [ { "name": "..." } ] }
            if let Some(decls) = tool.get("functionDeclarations").and_then(|v| v.as_array()) {
                for decl in decls {
                    if let Some(n) = decl.get("name").and_then(|v| v.as_str()) {
                        if n == "web_search"
                            || n == "google_search"
                            || n == "google_search_retrieval"
                        {
                            return true;
                        }
                    }
                }
            }

            // 4. Gemini googleSearch statement (含 googleSearchRetrieval Variants)
            if tool.get("googleSearch").is_some() || tool.get("googleSearchRetrieval").is_some() {
                return true;
            }
        }
    }
    false
}

/// detectionYesNoPacketContains non-network-related localFunctionTool
pub fn contains_non_networking_tool(tools: &Option<Vec<Value>>) -> bool {
    if let Some(list) = tools {
        for tool in list {
            let mut is_networking = false;

            // Simplelogic：If它YesoneFunctionstatement and the name is notYes联GatewayKey词，is regarded as non-networkedTool
            if let Some(n) = tool.get("name").and_then(|v| v.as_str()) {
                let keywords = [
                    "web_search",
                    "google_search",
                    "web_search_20250305",
                    "google_search_retrieval",
                ];
                if keywords.contains(&n) {
                    is_networking = true;
                }
            } else if let Some(func) = tool.get("function") {
                if let Some(n) = func.get("name").and_then(|v| v.as_str()) {
                    let keywords = [
                        "web_search",
                        "google_search",
                        "web_search_20250305",
                        "google_search_retrieval",
                    ];
                    if keywords.contains(&n) {
                        is_networking = true;
                    }
                }
            } else if tool.get("googleSearch").is_some()
                || tool.get("googleSearchRetrieval").is_some()
            {
                is_networking = true;
            } else if tool.get("functionDeclarations").is_some() {
                // IfYes Gemini Stylish functionDeclarations，Go in and take a look
                if let Some(decls) = tool.get("functionDeclarations").and_then(|v| v.as_array()) {
                    for decl in decls {
                        if let Some(n) = decl.get("name").and_then(|v| v.as_str()) {
                            let keywords =
                                ["web_search", "google_search", "google_search_retrieval"];
                            if !keywords.contains(&n) {
                                return true; // discover localFunction
                            }
                        }
                    }
                }
                is_networking = true; // Even if allYesnetworking，外层也标记为networking
            }

            if !is_networking {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_high_quality_model_auto_grounding() {
        // Auto-grounding is currently disabled by default due to conflict with image gen
        let config = resolve_request_config("gpt-4o", "gemini-2.5-flash", &None, None, None);
        assert_eq!(config.request_type, "agent");
        assert!(!config.inject_google_search);
    }

    #[test]
    fn test_gemini_native_tool_detection() {
        let tools = Some(vec![json!({
            "functionDeclarations": [
                { "name": "web_search", "parameters": {} }
            ]
        })]);
        assert!(detects_networking_tool(&tools));
    }

    #[test]
    fn test_online_suffix_force_grounding() {
        let config =
            resolve_request_config("gemini-3-flash-online", "gemini-3-flash", &None, None, None);
        assert_eq!(config.request_type, "web_search");
        assert!(config.inject_google_search);
        assert_eq!(config.final_model, "gemini-2.5-flash");
    }

    #[test]
    fn test_default_no_grounding() {
        let config = resolve_request_config("claude-sonnet", "gemini-3-flash", &None, None, None);
        assert_eq!(config.request_type, "agent");
        assert!(!config.inject_google_search);
    }

    #[test]
    fn test_image_model_excluded() {
        let config = resolve_request_config(
            "gemini-3-pro-image",
            "gemini-3-pro-image",
            &None,
            None,
            None,
        );
        assert_eq!(config.request_type, "image_gen");
        assert!(!config.inject_google_search);
    }

    #[test]
    fn test_image_2k_and_ultrawide_config() {
        // Test 2K
        let (config_2k, _) = parse_image_config("gemini-3-pro-image-2k");
        assert_eq!(config_2k["imageSize"], "2K");

        // Test 21:9
        let (config_21x9, _) = parse_image_config("gemini-3-pro-image-21x9");
        assert_eq!(config_21x9["aspectRatio"], "21:9");

        // Test Combined (if logic allows, though suffix parsing is greedy)
        let (config_combined, _) = parse_image_config("gemini-3-pro-image-2k-21x9");
        assert_eq!(config_combined["imageSize"], "2K");
        assert_eq!(config_combined["aspectRatio"], "21:9");

        // Test 4K + 21:9
        let (config_4k_wide, _) = parse_image_config("gemini-3-pro-image-4k-21x9");
        assert_eq!(config_4k_wide["imageSize"], "4K");
        assert_eq!(config_4k_wide["aspectRatio"], "21:9");
    }

    #[test]
    fn test_parse_image_config_with_openai_params() {
        // Test quality parameter mapping
        let (config_hd, _) = parse_image_config_with_params("gemini-3-pro-image", None, Some("hd"));
        assert_eq!(config_hd["imageSize"], "4K");
        assert_eq!(config_hd["aspectRatio"], "1:1");

        let (config_medium, _) =
            parse_image_config_with_params("gemini-3-pro-image", None, Some("medium"));
        assert_eq!(config_medium["imageSize"], "2K");

        let (config_standard, _) =
            parse_image_config_with_params("gemini-3-pro-image", None, Some("standard"));
        assert!(config_standard.get("imageSize").is_none());

        // Test size parameter mapping with dynamic calculation
        let (config_16_9, _) =
            parse_image_config_with_params("gemini-3-pro-image", Some("1280x720"), None);
        assert_eq!(config_16_9["aspectRatio"], "16:9");

        let (config_9_16, _) =
            parse_image_config_with_params("gemini-3-pro-image", Some("720x1280"), None);
        assert_eq!(config_9_16["aspectRatio"], "9:16");

        let (config_4_3, _) =
            parse_image_config_with_params("gemini-3-pro-image", Some("800x600"), None);
        assert_eq!(config_4_3["aspectRatio"], "4:3");

        // Test combined size + quality
        let (config_combined, _) =
            parse_image_config_with_params("gemini-3-pro-image", Some("1920x1080"), Some("hd"));
        assert_eq!(config_combined["aspectRatio"], "16:9");
        assert_eq!(config_combined["imageSize"], "4K");

        // Test backward compatibility: model suffix takes precedence when no params
        let (config_compat, _) =
            parse_image_config_with_params("gemini-3-pro-image-16x9-4k", None, None);
        assert_eq!(config_compat["aspectRatio"], "16:9");
        assert_eq!(config_compat["imageSize"], "4K");

        // Test parameter priority: params override model suffix
        let (config_override, _) = parse_image_config_with_params(
            "gemini-3-pro-image-1x1-2k",
            Some("1280x720"),
            Some("hd"),
        );
        assert_eq!(config_override["aspectRatio"], "16:9"); // from size param, not model suffix
        assert_eq!(config_override["imageSize"], "4K"); // from quality param, not model suffix
    }

    #[test]
    fn test_calculate_aspect_ratio_from_size() {
        // Test standard OpenAI sizes
        assert_eq!(calculate_aspect_ratio_from_size("1280x720"), "16:9");
        assert_eq!(calculate_aspect_ratio_from_size("1920x1080"), "16:9");
        assert_eq!(calculate_aspect_ratio_from_size("720x1280"), "9:16");
        assert_eq!(calculate_aspect_ratio_from_size("1080x1920"), "9:16");
        assert_eq!(calculate_aspect_ratio_from_size("1024x1024"), "1:1");
        assert_eq!(calculate_aspect_ratio_from_size("800x600"), "4:3");
        assert_eq!(calculate_aspect_ratio_from_size("600x800"), "3:4");
        assert_eq!(calculate_aspect_ratio_from_size("2560x1080"), "21:9");

        // Test edge cases
        assert_eq!(calculate_aspect_ratio_from_size("invalid"), "1:1");
        assert_eq!(calculate_aspect_ratio_from_size("1920x0"), "1:1");
        assert_eq!(calculate_aspect_ratio_from_size("0x1080"), "1:1");
        assert_eq!(calculate_aspect_ratio_from_size("abc x def"), "1:1");
    }
}
