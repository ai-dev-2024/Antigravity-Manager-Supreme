use serde_json::json;
use crate::proxy::mappers::claude::models::ClaudeRequest;

#[test]
fn test_claude_request_deserialization_leak() {
    // Simulate aPacket含 cache_control: null 的Request
    let incoming_json = json!({
        "model": "claude-3-5-sonnet-20241022",
        "messages": [
            {
                "role": "assistant",
                "content": [
                    {
                        "type": "thinking",
                        "thinking": "test",
                        "signature": "sig1234567890",
                        "cache_control": null
                    }
                ]
            }
        ]
    });

    let request: ClaudeRequest = serde_json::from_value(incoming_json).expect("Deserialization failed");
    
    // Check反SequencetransformedValue
    if let crate::proxy::mappers::claude::models::MessageContent::Array(blocks) = &request.messages[0].content {
        if let crate::proxy::mappers::claude::models::ContentBlock::Thinking { cache_control, .. } = &blocks[0] {
            println!("Debug: cache_control after deserialization: {:?}", cache_control);
            assert!(cache_control.is_none(), "cache_control should be None if incoming was null");
        }
    }
}
