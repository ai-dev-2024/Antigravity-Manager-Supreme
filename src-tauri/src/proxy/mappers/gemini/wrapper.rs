// Gemini v1internal Packet装/解Packet
use serde_json::{json, Value};

/// Packet装RequestBody as v1internal Format
pub fn wrap_request(body: &Value, project_id: &str, mapped_model: &str, session_id: Option<&str>) -> Value {
    // priorityUsingincoming mapped_model，SecondTrying从 body Get
    let original_model = body.get("model").and_then(|v| v.as_str()).unwrap_or(mapped_model);
    
    // If mapped_model YesEmpty的，则Using original_model
    let final_model_name = if !mapped_model.is_empty() {
        mapped_model
    } else {
        original_model
    };

    // Copy body so thatModified
    let mut inner_request = body.clone();

    // Depthclean up [undefined] string (Cherry Studio 等ClientCommon injections)
    crate::proxy::mappers::common_utils::deep_clean_undefined(&mut inner_request);

    // [FIX #765] Inject thought_signature into functionCall parts
    if let Some(s_id) = session_id {
        if let Some(contents) = inner_request.get_mut("contents").and_then(|c| c.as_array_mut()) {
            for content in contents {
                if let Some(parts) = content.get_mut("parts").and_then(|p| p.as_array_mut()) {
                    for part in parts {
                        if part.get("functionCall").is_some() {
                            // Only inject if it doesn't already have one
                            if part.get("thoughtSignature").is_none() {
                                if let Some(sig) = crate::proxy::SignatureCache::global().get_session_signature(s_id) {
                                    if let Some(obj) = part.as_object_mut() {
                                        obj.insert("thoughtSignature".to_string(), json!(sig));
                                        tracing::debug!("[Gemini-Wrap] Injected signature (len: {}) for session: {}", sig.len(), s_id);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // [FIX] Removed forced maxOutputTokens (64000) as it exceeds limits for Gemini 1.5 Flash/Pro standard models (8192).
    // This caused upstream to return empty/invalid responses, leading to 'NoneType' object has no attribute 'strip' in Python clients.
    // relying on upstream defaults or user provided values is safer.

    // extract tools Listto advanceLineNetwork detection (Gemini styleMayYesNested)
    let tools_val: Option<Vec<Value>> = inner_request.get("tools").and_then(|t| t.as_array()).map(|arr| {
        arr.clone()
    });

    // Use shared grounding/config logic
    let config = crate::proxy::mappers::common_utils::resolve_request_config(original_model, final_model_name, &tools_val, None, None);
    
    // Clean tool declarations (remove forbidden Schema fields like multipleOf, and remove redundant search decls)
    if let Some(tools) = inner_request.get_mut("tools") {
        if let Some(tools_arr) = tools.as_array_mut() {
            for tool in tools_arr {
                if let Some(decls) = tool.get_mut("functionDeclarations") {
                    if let Some(decls_arr) = decls.as_array_mut() {
                        // 1. FilterLost contactGatewayKey字Function
                        decls_arr.retain(|decl| {
                            if let Some(name) = decl.get("name").and_then(|v| v.as_str()) {
                                if name == "web_search" || name == "google_search" {
                                    return false;
                                }
                            }
                            true
                        });

                        // 2. CleanRemaining Schema
                        for decl in decls_arr {
                            if let Some(params) = decl.get_mut("parameters") {
                                crate::proxy::common::json_schema::clean_json_schema(params);
                            }
                        }
                    }
                }
            }
        }
    }

    tracing::debug!("[Debug] Gemini Wrap: original='{}', mapped='{}', final='{}', type='{}'", 
        original_model, final_model_name, config.final_model, config.request_type);
    
    // Inject googleSearch tool if needed
    if config.inject_google_search {
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
                 gen_obj.remove("responseModalities"); // Cherry Studio sends this, might conflict
                 gen_obj.insert("imageConfig".to_string(), image_config);
             }
         }
    } else {
        // [NEW] Only in nonPicturegenerateModeInject Antigravity identity (RawSimplified version)
        let antigravity_identity = "You are Antigravity, a powerful agentic AI coding assistant designed by the Google Deepmind team working on Advanced Agentic Coding.\n\
        You are pair programming with a USER to solve their coding task. The task may require creating a new codebase, modifying or debugging an existing codebase, or simply answering a question.\n\
        **Absolute paths only**\n\
        **Proactiveness**";
        
        // [HYBRID] CheckYesNoAlready have systemInstruction
        if let Some(system_instruction) = inner_request.get_mut("systemInstruction") {
            // [NEW] Complete role: user
            if let Some(obj) = system_instruction.as_object_mut() {
                if !obj.contains_key("role") {
                     obj.insert("role".to_string(), json!("user"));
                }
            }

            if let Some(parts) = system_instruction.get_mut("parts") {
                if let Some(parts_array) = parts.as_array_mut() {
                    // Checkfirst one part YesNo已Packet含 Antigravity identity
                    let has_antigravity = parts_array.get(0)
                        .and_then(|p| p.get("text"))
                        .and_then(|t| t.as_str())
                        .map(|s| s.contains("You are Antigravity"))
                        .unwrap_or(false);
                    
                    if !has_antigravity {
                        // insert in front Antigravity identity
                        parts_array.insert(0, json!({"text": antigravity_identity}));
                    }
                }
            }
        } else {
            // None systemInstruction,CreateoneNew
            inner_request["systemInstruction"] = json!({
                "role": "user",
                "parts": [{"text": antigravity_identity}]
            });
        }
    }

    let final_request = json!({
        "project": project_id,
        "requestId": format!("agent-{}", uuid::Uuid::new_v4()), // Corrected to agent- Prefix
        "request": inner_request,
        "model": config.final_model,
        "userAgent": "antigravity",
        "requestType": config.request_type
    });

    final_request
}

#[cfg(test)]
mod test_fixes {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_wrap_request_with_signature() {
        let session_id = "test-session-sig";
        let signature = "test-signature-must-be-longer-than-fifty-characters-to-be-cached-by-signature-cache-12345"; // > 50 chars
        crate::proxy::SignatureCache::global().cache_session_signature(session_id, signature.to_string());

        let body = json!({
            "model": "gemini-pro",
            "contents": [{
                "role": "user",
                "parts": [{
                    "functionCall": {
                        "name": "get_weather",
                        "args": {"location": "London"}
                    }
                }]
            }]
        });

        let result = wrap_request(&body, "proj", "gemini-pro", Some(session_id));
        let injected_sig = result["request"]["contents"][0]["parts"][0]["thoughtSignature"].as_str().unwrap();
        assert_eq!(injected_sig, signature);
    }
}

/// 解PacketResponse（extract response Field）
pub fn unwrap_response(response: &Value) -> Value {
    response.get("response").unwrap_or(response).clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_wrap_request() {
        let body = json!({
            "model": "gemini-2.5-flash",
            "contents": [{"role": "user", "parts": [{"text": "Hi"}]}]
        });

        let result = wrap_request(&body, "test-project", "gemini-2.5-flash", None);
        assert_eq!(result["project"], "test-project");
        assert_eq!(result["model"], "gemini-2.5-flash");
        assert!(result["requestId"].as_str().unwrap().starts_with("agent-"));
    }

    #[test]
    fn test_unwrap_response() {
        let wrapped = json!({
            "response": {
                "candidates": [{"content": {"parts": [{"text": "Hello"}]}}]
            }
        });

        let result = unwrap_response(&wrapped);
        assert!(result.get("candidates").is_some());
        assert!(result.get("response").is_none());
    }

    #[test]
    fn test_antigravity_identity_injection_with_role() {
        let body = json!({
            "model": "gemini-pro",
            "messages": []
        });
        
        let result = wrap_request(&body, "test-proj", "gemini-pro", None);
        
        // Validate systemInstruction
        let sys = result.get("request").unwrap().get("systemInstruction").unwrap();
        
        // 1. Validate role: "user"
        assert_eq!(sys.get("role").unwrap(), "user");
        
        // 2. Validate Antigravity identity injection
        let parts = sys.get("parts").unwrap().as_array().unwrap();
        assert!(!parts.is_empty());
        let first_text = parts[0].get("text").unwrap().as_str().unwrap();
        assert!(first_text.contains("You are Antigravity"));
    }

    #[test]
    fn test_user_instruction_preservation() {
        let body = json!({
            "model": "gemini-pro",
            "systemInstruction": {
                "role": "user",
                "parts": [{"text": "User custom prompt"}]
            }
        });

        let result = wrap_request(&body, "test-proj", "gemini-pro", None);
        let sys = result.get("request").unwrap().get("systemInstruction").unwrap();
        let parts = sys.get("parts").unwrap().as_array().unwrap();

        // Should have 2 parts: Antigravity + User
        assert_eq!(parts.len(), 2);
        assert!(parts[0].get("text").unwrap().as_str().unwrap().contains("You are Antigravity"));
        assert_eq!(parts[1].get("text").unwrap().as_str().unwrap(), "User custom prompt");
    }

    #[test]
    fn test_duplicate_prevention() {
        let body = json!({
            "model": "gemini-pro",
            "systemInstruction": {
                "parts": [{"text": "You are Antigravity..."}]
            }
        });

        let result = wrap_request(&body, "test-proj", "gemini-pro", None);
        let sys = result.get("request").unwrap().get("systemInstruction").unwrap();
        let parts = sys.get("parts").unwrap().as_array().unwrap();

        // Should NOT inject duplicate, so only 1 part remains
        assert_eq!(parts.len(), 1);
    }
}
