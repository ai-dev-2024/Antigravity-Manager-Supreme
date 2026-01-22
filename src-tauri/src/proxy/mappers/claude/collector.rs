// Stream Collector - 将 SSE StreamConvertfor complete JSON Response
// Used for non Stream Requestof automaticConvert

use super::models::*;
use bytes::Bytes;
use futures::StreamExt;
use serde_json::{json, Value};
use std::io;

/// SSE EventType
#[derive(Debug, Clone)]
struct SseEvent {
    event_type: String,
    data: Value,
}

/// Parse SSE Line
fn parse_sse_line(line: &str) -> Option<(String, String)> {
    if let Some(colon_pos) = line.find(':') {
        let key = &line[..colon_pos];
        let value = line[colon_pos + 1..].trim_start();
        Some((key.to_string(), value.to_string()))
    } else {
        None
    }
}

/// 将 SSE Stream collected as complete Claude Response
///
/// 此FunctionReceiveone SSE 字Throttle，ParseAllEvent，and rebuild the complete ClaudeResponse Object。
/// This makes non Stream ClientCanenjoy transparently Stream Mode的QuotaAdvantages。
pub async fn collect_stream_to_json<S>(
    mut stream: S,
) -> Result<ClaudeResponse, String>
where
    S: futures::Stream<Item = Result<Bytes, io::Error>> + Unpin,
{
    let mut events = Vec::new();
    let mut current_event_type = String::new();
    let mut current_data = String::new();

    // 1. collectAll SSE Event
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| format!("Stream error: {}", e))?;
        let text = String::from_utf8_lossy(&chunk);

        for line in text.lines() {
            if line.is_empty() {
                // EmptyLineexpressEventEnd
                if !current_data.is_empty() {
                    if let Ok(data) = serde_json::from_str::<Value>(&current_data) {
                        events.push(SseEvent {
                            event_type: current_event_type.clone(),
                            data,
                        });
                    }
                    current_event_type.clear();
                    current_data.clear();
                }
            } else if let Some((key, value)) = parse_sse_line(line) {
                match key.as_str() {
                    "event" => current_event_type = value,
                    "data" => current_data = value,
                    _ => {}
                }
            }
        }
    }

    // 2. reconstruction ClaudeResponse
    let mut response = ClaudeResponse {
        id: "msg_unknown".to_string(),
        type_: "message".to_string(),
        role: "assistant".to_string(),
        model: String::new(),
        content: Vec::new(),
        stop_reason: "end_turn".to_string(),
        stop_sequence: None,
        usage: Usage {
            input_tokens: 0,
            output_tokens: 0,
            cache_read_input_tokens: None,
            cache_creation_input_tokens: None,
            server_tool_use: None,
        },
    };

    // for accumulationContentBlock
    let mut current_text = String::new();
    let mut current_thinking = String::new();
    let mut current_signature: Option<String> = None;
    let mut current_tool_use: Option<Value> = None;
    let mut current_tool_input = String::new();

    for event in events {
        match event.event_type.as_str() {
            "message_start" => {
                // extractBasicInfo
                if let Some(message) = event.data.get("message") {
                    if let Some(id) = message.get("id").and_then(|v| v.as_str()) {
                        response.id = id.to_string();
                    }
                    if let Some(model) = message.get("model").and_then(|v| v.as_str()) {
                        response.model = model.to_string();
                    }
                    if let Some(usage) = message.get("usage") {
                        if let Ok(u) = serde_json::from_value::<Usage>(usage.clone()) {
                            response.usage = u;
                        }
                    }
                }
            }

            "content_block_start" => {
                if let Some(content_block) = event.data.get("content_block") {
                    if let Some(block_type) = content_block.get("type").and_then(|v| v.as_str()) {
                        match block_type {
                            "text" => current_text.clear(),
                            "thinking" => {
                                current_thinking.clear();
                                // Extract signature from content_block
                                current_signature = content_block.get("signature")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string());
                            }
                            "tool_use" => {
                                current_tool_use = Some(content_block.clone());
                                current_tool_input.clear();
                            }
                            _ => {}
                        }
                    }
                }
            }

            "content_block_delta" => {
                if let Some(delta) = event.data.get("delta") {
                    if let Some(delta_type) = delta.get("type").and_then(|v| v.as_str()) {
                        match delta_type {
                            "text_delta" => {
                                if let Some(text) = delta.get("text").and_then(|v| v.as_str()) {
                                    current_text.push_str(text);
                                }
                            }
                            "thinking_delta" => {
                                if let Some(thinking) = delta.get("thinking").and_then(|v| v.as_str()) {
                                    current_thinking.push_str(thinking);
                                }
                                // In case signature comes in delta (less likely but possible update)
                                if let Some(sig) = delta.get("signature").and_then(|v| v.as_str()) {
                                    current_signature = Some(sig.to_string());
                                }
                            }
                            "input_json_delta" => {
                                if let Some(partial_json) = delta.get("partial_json").and_then(|v| v.as_str()) {
                                    current_tool_input.push_str(partial_json);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }

            "content_block_stop" => {
                // CompleteCurrentBlock
                if !current_text.is_empty() {
                    response.content.push(ContentBlock::Text {
                        text: current_text.clone(),
                    });
                    current_text.clear();
                } else if !current_thinking.is_empty() {
                    response.content.push(ContentBlock::Thinking {
                        thinking: current_thinking.clone(),
                        signature: current_signature.take(),
                        cache_control: None,
                    });
                    current_thinking.clear();
                } else if let Some(tool_use) = current_tool_use.take() {
                    // build tool_use Block
                    let id = tool_use.get("id").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                    let name = tool_use.get("name").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                    let input = if !current_tool_input.is_empty() {
                        serde_json::from_str(&current_tool_input).unwrap_or(json!({}))
                    } else {
                        json!({})
                    };

                    response.content.push(ContentBlock::ToolUse {
                        id,
                        name,
                        input,
                        signature: None,
                        cache_control: None,
                    });
                    current_tool_input.clear();
                }
            }

            "message_delta" => {
                if let Some(delta) = event.data.get("delta") {
                    if let Some(stop_reason) = delta.get("stop_reason").and_then(|v| v.as_str()) {
                        response.stop_reason = stop_reason.to_string();
                    }
                }
                if let Some(usage) = event.data.get("usage") {
                    if let Ok(u) = serde_json::from_value::<Usage>(usage.clone()) {
                        response.usage = u;
                    }
                }
            }

            "message_stop" => {
                // Stream End
                break;
            }

            "error" => {
                // ErrorEvent
                return Err(format!("Stream error: {:?}", event.data));
            }

            _ => {
                // ignore unknownEventType
            }
        }
    }

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;

    #[tokio::test]
    async fn test_collect_simple_text_response() {
        // Simulate aSimplethe text ofResponse SSE Stream
        let sse_data = vec![
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_123\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"claude-3-5-sonnet\",\"content\":[],\"stop_reason\":null,\"usage\":{\"input_tokens\":10,\"output_tokens\":0}}}\n\n",
            "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\n",
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\" World\"}}\n\n",
            "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
            "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":5}}\n\n",
            "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
        ];

        let byte_stream = stream::iter(
            sse_data.into_iter().map(|s| Ok::<Bytes, io::Error>(Bytes::from(s)))
        );

        let result = collect_stream_to_json(byte_stream).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.id, "msg_123");
        assert_eq!(response.model, "claude-3-5-sonnet");
        assert_eq!(response.content.len(), 1);
        
        if let ContentBlock::Text { text } = &response.content[0] {
            assert_eq!(text, "Hello World");
        } else {
            panic!("Expected Text block");
        }
    }

    #[tokio::test]
    async fn test_collect_thinking_response_with_signature() {
        // Simulate aPacket含 Thinking Block 和Sign的 SSE Stream
        let sse_data = vec![
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_think\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"claude-3-7-sonnet\",\"content\":[],\"stop_reason\":null,\"usage\":{\"input_tokens\":10,\"output_tokens\":0}}}\n\n",
            // content_block_start 中Packet含 signature
            "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\", \"signature\": \"sig_123456\"}}\n\n",
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"I am \"}}\n\n",
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"thinking\"}}\n\n",
            "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
            "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":10}}\n\n",
            "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
        ];

        let byte_stream = stream::iter(
            sse_data.into_iter().map(|s| Ok::<Bytes, io::Error>(Bytes::from(s)))
        );

        let result = collect_stream_to_json(byte_stream).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        
        if let ContentBlock::Thinking { thinking, signature, .. } = &response.content[0] {
            assert_eq!(thinking, "I am thinking");
            // ValidateSignYesNowas extracted correctly
            assert_eq!(signature.as_deref(), Some("sig_123456"));
        } else {
            panic!("Expected Thinking block");
        }
    }
}
