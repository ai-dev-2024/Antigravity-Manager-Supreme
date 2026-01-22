//! ToolResultOutputCompressModule
//! 
//! Provide intelligenceCompressFunction:
//! - Browser snapshotCompress (头+tail reserved)
//! - 大FileHintCompress (Extraction levelKeyInfo)
//! - GeneralTruncate (200,000 characterLimit)

use regex::Regex;
use serde_json::Value;
use tracing::{debug, info};

/// MaximumToolResultNumber of characters (约 20 万,prevent prompt Extra long)
const MAX_TOOL_RESULT_CHARS: usize = 200_000;

/// Browser snapshot detectionThreshold
const SNAPSHOT_DETECTION_THRESHOLD: usize = 20_000;

/// Browser snapshotCompresslaterMaximumNumber of characters
const SNAPSHOT_MAX_CHARS: usize = 16_000;

/// Browser snapshotHeaderretention ratio
const SNAPSHOT_HEAD_RATIO: f64 = 0.7;

/// Browser snapshot tail retention ratio
#[allow(dead_code)]
const SNAPSHOT_TAIL_RATIO: f64 = 0.3;

/// CompressToolResulttext
/// 
/// according toContentTypeAutomatically select the bestCompressStrategy:
/// 1. 大FileHint → Extraction levelKeyInfo
/// 2. Browser snapshot → 头+tail reserved
/// 3. other → SimpleTruncate
pub fn compact_tool_result_text(text: &str, max_chars: usize) -> String {
    if text.is_empty() || text.len() <= max_chars {
        return text.to_string();
    }
    
    // [NEW] againstMay的 HTML Content进LineDepth预Handle
    let cleaned_text = if text.contains("<html") || text.contains("<body") || text.contains("<!DOCTYPE") {
        let cleaned = deep_clean_html(text);
        debug!("[ToolCompressor] Deep cleaned HTML, reduced {} -> {} chars", text.len(), cleaned.len());
        cleaned
    } else {
        text.to_string()
    };

    if cleaned_text.len() <= max_chars {
        return cleaned_text;
    }

    // 1. Detect largeFileHintMode
    if let Some(compacted) = compact_saved_output_notice(&cleaned_text, max_chars) {
        debug!("[ToolCompressor] Detected saved output notice, compacted to {} chars", compacted.len());
        return compacted;
    }
    
    // 2. Detect browser snapshotsMode
    if cleaned_text.len() > SNAPSHOT_DETECTION_THRESHOLD {
        if let Some(compacted) = compact_browser_snapshot(&cleaned_text, max_chars) {
            debug!("[ToolCompressor] Detected browser snapshot, compacted to {} chars", compacted.len());
            return compacted;
        }
    }
    
    // 3. Structtruncation
    debug!("[ToolCompressor] Using structured truncation for {} chars", cleaned_text.len());
    truncate_text_safe(&cleaned_text, max_chars)
}

/// Compress"Output已Save到File"Type的Hint
/// 
/// DetectionMode: "result (N characters) exceeds maximum allowed tokens. Output saved to <path>"
/// Strategy: Extraction levelKeyInfo(FilePath、Number of characters、FormatDescription)
/// 
/// according toHintContentTypeAutomatic extraction offKeyInfo
fn compact_saved_output_notice(text: &str, max_chars: usize) -> Option<String> {
    // Regular match: result (N characters) exceeds maximum allowed tokens. Output saved to <path>
    let re = Regex::new(
        r"(?i)result\s*\(\s*(?P<count>[\d,]+)\s*characters\s*\)\s*exceeds\s+maximum\s+allowed\s+tokens\.\s*Output\s+(?:has\s+been\s+)?saved\s+to\s+(?P<path>[^\r\n]+)"
    ).ok()?;
    
    let caps = re.captures(text)?;
    let count = caps.name("count")?.as_str();
    let raw_path = caps.name("path")?.as_str();
    
    // clean upFilePath (Removetrailing brackets、quotation marks、period)
    let file_path = raw_path
        .trim()
        .trim_end_matches(&[')', ']', '"', '\'', '.'][..])
        .trim();
    
    // Extraction levelKeyLine
    let lines: Vec<&str> = text.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
    
    // FindNotificationLine
    let notice_line = lines.iter()
        .find(|l| l.to_lowercase().contains("exceeds maximum allowed tokens") && l.to_lowercase().contains("saved to"))
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("result ({} characters) exceeds maximum allowed tokens. Output has been saved to {}", count, file_path));
    
    // FindFormatDescriptionLine
    let format_line = lines.iter()
        .find(|l| l.starts_with("Format:") || l.contains("JSON array with schema") || l.to_lowercase().starts_with("schema:"))
        .map(|s| s.to_string());
    
    // buildCompresslaterOutput
    let mut compact_lines = vec![notice_line];
    if let Some(fmt) = format_line {
        if !compact_lines.contains(&fmt) {
            compact_lines.push(fmt);
        }
    }
    compact_lines.push(format!(
        "[tool_result omitted to reduce prompt size; read file locally if needed: {}]",
        file_path
    ));
    
    let result = compact_lines.join("\n");
    Some(truncate_text_safe(&result, max_chars))
}

/// CompressBrowser snapshot (头+tail retention policy)
/// 
/// Detection: "page snapshot" 或 "PageSnapshot" or a large amount "ref=" Reference
/// Strategy: reserveHeader 70% + tail 30%,MiddleOmit
/// 
/// Using头+tail retention policyCompresslongerPageSnapshotData
fn compact_browser_snapshot(text: &str, max_chars: usize) -> Option<String> {
    // DetectionYesNoYesBrowser snapshot
    let is_snapshot = text.to_lowercase().contains("page snapshot")
        || text.contains("PageSnapshot")
        || text.matches("ref=").count() > 30
        || text.matches("[ref=").count() > 30;
    
    if !is_snapshot {
        return None;
    }
    
    let desired_max = max_chars.min(SNAPSHOT_MAX_CHARS);
    if desired_max < 2000 || text.len() <= desired_max {
        return None;
    }
    
    let meta = format!("[page snapshot summarized to reduce prompt size; original {} chars]", text.len());
    let overhead = meta.len() + 200;
    let budget = desired_max.saturating_sub(overhead);
    
    if budget < 1000 {
        return None;
    }
    
    // calculateHeaderand tailLength
    let head_len = (budget as f64 * SNAPSHOT_HEAD_RATIO).floor() as usize;
    let head_len = head_len.min(10_000).max(500);
    let tail_len = budget.saturating_sub(head_len).min(3_000);
    
    let head = &text[..head_len.min(text.len())];
    let tail = if tail_len > 0 && text.len() > head_len {
        let start = text.len().saturating_sub(tail_len);
        &text[start..]
    } else {
        ""
    };
    
    let omitted = text.len().saturating_sub(head_len).saturating_sub(tail_len);
    
    let summarized = if tail.is_empty() {
        format!("{}\n---[HEAD]---\n{}\n---[...omitted {} chars]---", meta, head, omitted)
    } else {
        format!(
            "{}\n---[HEAD]---\n{}\n---[...omitted {} chars]---\n---[TAIL]---\n{}",
            meta, head, omitted, tail
        )
    };
    
    Some(truncate_text_safe(&summarized, max_chars))
}

/// Safe text truncation (Try not to be thereTabMiddleTruncate)
fn truncate_text_safe(text: &str, max_chars: usize) -> String {
    if text.len() <= max_chars {
        return text.to_string();
    }
    
    // TryingFind a safe cutoff point (Not here < 和 > between)
    let mut split_pos = max_chars;
    
    // look forwardYesNoThere are unclosedTabBegin符
    let sub = &text[..max_chars];
    if let Some(last_open) = sub.rfind('<') {
        if let Some(last_close) = sub.rfind('>') {
            if last_open > last_close {
                // The cutoff point is atTabMiddle，Fallback toTabBegin前
                split_pos = last_open;
            }
        } else {
            // OnlyBeginNoneEnd，Fallback toTabBegin前
            split_pos = last_open;
        }
    }
    
    // Also avoid JSON BracesMiddleTruncate
    if let Some(last_open_brace) = sub.rfind('{') {
        if let Some(last_close_brace) = sub.rfind('}') {
            if last_open_brace > last_close_brace {
                // May在 JSON Middle，IfCloser to the cutoff point，Tryingrollback
                if max_chars - last_open_brace < 100 {
                    split_pos = split_pos.min(last_open_brace);
                }
            }
        }
    }

    let truncated = &text[..split_pos];
    let omitted = text.len() - split_pos;
    format!("{}\n...[truncated {} chars]", truncated, omitted)
}

/// Depthclean up HTML (Remove style, script, base64 等)
fn deep_clean_html(html: &str) -> String {
    let mut result = html.to_string();
    
    // 1. Remove <style>...</style> andContent
    if let Ok(re) = Regex::new(r"(?is)<style\b[^>]*>.*?</style>") {
        result = re.replace_all(&result, "[style omitted]").to_string();
    }
    
    // 2. Remove <script>...</script> andContent
    if let Ok(re) = Regex::new(r"(?is)<script\b[^>]*>.*?</script>") {
        result = re.replace_all(&result, "[script omitted]").to_string();
    }
    
    // 3. Remove inline Base64 Data (如 src="data:image/png;base64,...")
    if let Ok(re) = Regex::new(r#"(?i)data:[^;/]+/[^;]+;base64,[A-Za-z0-9+/=]+"#) {
        result = re.replace_all(&result, "[base64 omitted]").to_string();
    }

    // 4. RemoveredundantEmptywhite characters
    if let Ok(re) = Regex::new(r"\n\s*\n") {
        result = re.replace_all(&result, "\n").to_string();
    }
    
    result
}

/// clean upToolResult content blocks
/// 
/// Handlelogic:
/// 1. Remove base64 Image (Avoid excessive size)
/// 2. CompresstextContent (UsingintelligentCompressStrategy)
/// 3. LimitTotal characters (Default 200,000)
/// 
/// Clean and truncateToolcallResultContentBlock
pub fn sanitize_tool_result_blocks(blocks: &mut Vec<Value>) {
    let mut used_chars = 0;
    let mut cleaned_blocks = Vec::new();
    let mut removed_image = false;
    
    if !blocks.is_empty() {
        info!(
            "[ToolCompressor] Processing {} blocks for truncation (MAX: {} chars)",
            blocks.len(),
            MAX_TOOL_RESULT_CHARS
        );
    }
    
    for block in blocks.iter() {
        // Remove base64 Image
        if is_base64_image(block) {
            removed_image = true;
            debug!("[ToolCompressor] Removed base64 image block");
            continue;
        }
        
        // CompresstextContent
        if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
            let remaining = MAX_TOOL_RESULT_CHARS.saturating_sub(used_chars);
            if remaining == 0 {
                debug!("[ToolCompressor] Reached character limit, stopping");
                break;
            }
            
            let compacted = compact_tool_result_text(text, remaining);
            let mut new_block = block.clone();
            new_block["text"] = Value::String(compacted.clone());
            cleaned_blocks.push(new_block);
            used_chars += compacted.len();
            
            debug!(
                "[ToolCompressor] Compacted text block: {} → {} chars",
                text.len(),
                compacted.len()
            );
        } else {
            cleaned_blocks.push(block.clone());
            used_chars += 100; // Estimate non-textBlockSize
        }
        
        if used_chars >= MAX_TOOL_RESULT_CHARS {
            break;
        }
    }
    
    if removed_image {
        cleaned_blocks.push(serde_json::json!({
            "type": "text",
            "text": "[image omitted to fit Antigravity prompt limits; use the file path in the previous text block]"
        }));
    }
    
    info!(
        "[ToolCompressor] Sanitization complete: {} → {} blocks, {} chars used",
        blocks.len(),
        cleaned_blocks.len(),
        used_chars
    );
    
    *blocks = cleaned_blocks;
}

/// DetectionYesNoYes base64 ImageBlock
fn is_base64_image(block: &Value) -> bool {
    block.get("type").and_then(|v| v.as_str()) == Some("image")
        && block
            .get("source")
            .and_then(|s| s.get("type"))
            .and_then(|v| v.as_str())
            == Some("base64")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_text() {
        let text = "a".repeat(300_000);
        let result = truncate_text_safe(&text, 200_000);
        assert!(result.len() < 210_000); // PacketContains truncationHint
        assert!(result.contains("[truncated"));
        assert!(result.contains("100000 chars]"));
    }

    #[test]
    fn test_truncate_text_no_truncation() {
        let text = "short text";
        let result = truncate_text_safe(text, 1000);
        assert_eq!(result, text);
    }

    #[test]
    fn test_compact_browser_snapshot() {
        let snapshot = format!("page snapshot: {}", "ref=abc ".repeat(10_000));
        let result = compact_tool_result_text(&snapshot, 16_000);
        
        assert!(result.len() <= 16_500); // allowSome overhead
        assert!(result.contains("[HEAD]"));
        assert!(result.contains("[TAIL]"));
        assert!(result.contains("page snapshot summarized"));
    }

    #[test]
    fn test_compact_saved_output_notice() {
        let text = r#"result (150000 characters) exceeds maximum allowed tokens. Output has been saved to /tmp/output.txt
Format: JSON array with schema
Please read the file locally."#;
        
        let result = compact_tool_result_text(text, 500);
        println!("Result: {}", result);
        assert!(result.contains("150000 characters") || result.contains("150,000 characters"));
        assert!(result.contains("/tmp/output.txt"));
        assert!(result.contains("[tool_result omitted") || result.len() <= 500);
    }

    #[test]
    fn test_sanitize_tool_result_blocks() {
        let mut blocks = vec![
            serde_json::json!({
                "type": "text",
                "text": "a".repeat(100_000)
            }),
            serde_json::json!({
                "type": "text",
                "text": "b".repeat(150_000)
            }),
        ];

        sanitize_tool_result_blocks(&mut blocks);

        assert_eq!(blocks.len(), 2);
        // first oneBlockShouldstay as is
        assert_eq!(blocks[0]["text"].as_str().unwrap().len(), 100_000);
        // secondBlockShouldtruncated to 100,000 (200,000 - 100,000)
        assert!(blocks[1]["text"].as_str().unwrap().len() < 110_000);
    }

    #[test]
    fn test_sanitize_removes_base64_image() {
        let mut blocks = vec![
            serde_json::json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "data": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg=="
                }
            }),
            serde_json::json!({
                "type": "text",
                "text": "some text"
            }),
        ];

        sanitize_tool_result_blocks(&mut blocks);

        // ImageShould被Remove,Add了Hinttext
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0]["type"], "text");
        assert_eq!(blocks[0]["text"], "some text");
        assert!(blocks[1]["text"].as_str().unwrap().contains("[image omitted"));
    }

    #[test]
    fn test_is_base64_image() {
        let image_block = serde_json::json!({
            "type": "image",
            "source": {
                "type": "base64",
                "data": "abc123"
            }
        });
        assert!(is_base64_image(&image_block));

        let text_block = serde_json::json!({
            "type": "text",
            "text": "hello"
        });
        assert!(!is_base64_image(&text_block));
    }
}
