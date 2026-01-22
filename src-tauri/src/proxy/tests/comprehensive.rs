#[cfg(test)]
mod tests {
    use crate::proxy::mappers::claude::models::{
        ClaudeRequest, Message, MessageContent, ContentBlock, ThinkingConfig
    };
    use crate::proxy::mappers::claude::request::transform_claude_request_in;
    use crate::proxy::mappers::claude::thinking_utils::{analyze_conversation_state, close_tool_loop_for_thinking};
    use serde_json::json;

    
    // ==================================================================================
    // Scene one：first Thinking Request (P0-2 Fix)
    // Validate在NonehistorySigncase，Launched for the first time Thinking RequestYesNobe releasedLine (Perimssive Mode)
    // ==================================================================================
    #[test]
    fn test_first_thinking_request_permissive_mode() {
        // 1. Construct a completeNewRequest (no historyMessage)
        let req = ClaudeRequest {
            model: "claude-3-7-sonnet-20250219".to_string(),
            messages: vec![
                Message {
                    role: "user".to_string(),
                    content: MessageContent::String("Hello, please think.".to_string()),
                }
            ],
            system: None,
            tools: None, // 无Toolcall
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

        // 2. ExecuteConvert
        // IfThe fix takes effect，HereShouldSuccessReturn，且 thinkingConfig reserved
        let result = transform_claude_request_in(&req, "test-project", false);
        assert!(result.is_ok(), "First thinking request should be allowed");

        let body = result.unwrap();
        let request = &body["request"];
        
        // Validate thinkingConfig YesNoexist (即 thinking ModebeenDisable)
        let has_thinking_config = request.get("generationConfig")
            .and_then(|g| g.get("thinkingConfig"))
            .is_some();
            
        assert!(has_thinking_config, "Thinking config should be preserved for first request without tool calls");
    }

    // ==================================================================================
    // Scene 2：Toolcycle recovery (P1-4 Fix)
    // ValidateWhenhistoryMessageLost in Thinking BlockWhen causing an infinite loop，YesNoWill automatically inject synthesisMessageto close the loop
    // ==================================================================================
    #[test]
    fn test_tool_loop_recovery() {
        // 1. Construct a "Broken Tool Loop" scene
        // Assistant (ToolUse) -> User (ToolResult)
        // 但 Assistant Messagemissing in Thinking Block (simulated quilt stripping)
        let mut messages = vec![
            Message {
                role: "user".to_string(),
                content: MessageContent::String("Check weather".to_string()),
            },
            Message {
                role: "assistant".to_string(),
                content: MessageContent::Array(vec![
                    // Only ToolUse，None Thinking (Broken State)
                    ContentBlock::ToolUse {
                        id: "call_1".to_string(),
                        name: "get_weather".to_string(),
                        input: json!({"location": "Beijing"}),
                        signature: None,
                        cache_control: None,
                    }
                ]),
            },
            Message {
                role: "user".to_string(),
                content: MessageContent::Array(vec![
                    ContentBlock::ToolResult {
                        tool_use_id: "call_1".to_string(),
                        content: json!("Sunny"),
                        is_error: None,
                    }
                ]),
            }
        ];

        // 2. analyzeCurrentStatus
        let state = analyze_conversation_state(&messages);
        assert!(state.in_tool_loop, "Should detect tool loop");

        // 3. Executerecovery logic
        close_tool_loop_for_thinking(&mut messages);

        // 4. ValidateYesNoInjected syntheticMessage
        assert_eq!(messages.len(), 5, "Should have injected 2 synthetic messages");
        
        // Validatepenultimate articleYes Assistant 的 "Completed" Message
        let injected_assistant = &messages[3];
        assert_eq!(injected_assistant.role, "assistant");
        
        // ValidateFinallyone pieceYes User 的 "Proceed" Message
        let injected_user = &messages[4];
        assert_eq!(injected_user.role, "user");
        
        // soCurrentStatusno moreYes "in_tool_loop" (Finallyone pieceYes User Text)，ModelCanBeginNew Thinking
        let new_state = analyze_conversation_state(&messages);
        assert!(!new_state.in_tool_loop, "Tool loop should be broken/closed");
    }

    // ==================================================================================
    // Scene three：跨ModelCompatible性 (P1-5 Fix) - simulation
    // because request.rs in is_model_compatible YesPrivate的，We integrateTestValidateEffect
    // ==================================================================================
    /* 
       Notice：because is_model_compatible 和CachelogicDepthintegrated in transform_claude_request_in 中，
       and depend onGlobalSingleton SignatureCache，CellTestDifficult to simulate "CacheoldSignbut switchedModel" 的Status。
       HereMainpassValidate "不CompatibleSigndiscarded" side effects（即 thoughtSignature FieldMessage）来Test。
       But because SignatureCache YesGlobal的，we can'tTestEasy to presetStatus。
       therefore，this sceneMainrely Verification Guide manual inTest。
       Or，usCanTest request.rs 中Publicsome of helper (IfIf yes)，But currentlyNone。
    */

}
