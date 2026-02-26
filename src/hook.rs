use serde_json::Value;
use std::io::{self, BufRead, Read};
use std::path::PathBuf;

use crate::db::{Database, PreCompactMessage};

/// Extract project name from cwd path.
fn project_from_cwd(cwd: &str) -> &str {
    let p = cwd.rsplit('/').next().unwrap_or("unknown0");
    if p.is_empty() { "unknown0" } else { p }
}

/// Derive a formatted session_id from hook JSON fields.
/// Format: {session_id}-{project_name}
/// No timestamp — hooks are independent processes and timestamps would differ.
fn derive_session_id(session_id: &str, cwd: &str) -> String {
    format!("{}-{}", session_id, project_from_cwd(cwd))
}

pub fn handle_hook(db_path: &PathBuf) {
    let mut input = String::new();
    if io::stdin().read_to_string(&mut input).is_err() {
        eprintln!("leafhill-hook: failed to read stdin");
        return;
    }

    let hook: Value = match serde_json::from_str(&input) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("leafhill-hook: invalid JSON: {}", e);
            return;
        }
    };

    let event = hook.get("hook_event_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let session_id = hook.get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let cwd = hook.get("cwd")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    match event {
        "SessionStart" => {
            let formatted_sid = derive_session_id(session_id, cwd);
            let project = project_from_cwd(cwd);
            let output = serde_json::json!({
                "additionalContext": format!(
                    "[leafhill-memory] session_id={} project={}. \
                     After EVERY exchange, call log_conversation with this session_id, \
                     role=\"summary\", entry_type=\"summary\", and a concise summary of \
                     what was discussed/done.",
                    formatted_sid, project
                )
            });
            println!("{}", serde_json::to_string(&output).unwrap_or_default());
        }
        "UserPromptSubmit" => {
            let prompt = hook.get("prompt")
                .map(|v| v.as_str().unwrap_or(&v.to_string()).to_string())
                .unwrap_or_default();
            if prompt.is_empty() { return; }
            let formatted_sid = derive_session_id(session_id, cwd);
            let project = project_from_cwd(cwd);
            let db = match Database::open(db_path) {
                Ok(db) => db,
                Err(e) => { eprintln!("leafhill-hook: db error: {}", e); return; }
            };
            if let Err(e) = db.log_conversation(
                &formatted_sid, "user", &prompt,
                Some(project), Some("raw_user"), None,
            ) {
                eprintln!("leafhill-hook: failed to log UserPromptSubmit: {}", e);
            }
        }
        "Stop" => {
            let stop_active = hook.get("stop_hook_active")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if stop_active { return; }
            let message = hook.get("last_assistant_message")
                .map(|v| v.as_str().unwrap_or(&v.to_string()).to_string())
                .unwrap_or_default();
            if message.is_empty() { return; }
            let formatted_sid = derive_session_id(session_id, cwd);
            let project = project_from_cwd(cwd);
            let db = match Database::open(db_path) {
                Ok(db) => db,
                Err(e) => { eprintln!("leafhill-hook: db error: {}", e); return; }
            };
            if let Err(e) = db.log_conversation(
                &formatted_sid, "assistant", &message,
                Some(project), Some("raw_assistant"), None,
            ) {
                eprintln!("leafhill-hook: failed to log Stop: {}", e);
            }
        }
        "PreCompact" => {
            let transcript_path = hook.get("transcript_path")
                .and_then(|v| v.as_str());
            let transcript_path = match transcript_path {
                Some(p) => p,
                None => {
                    eprintln!("leafhill-hook: PreCompact: no transcript_path");
                    return;
                }
            };

            let file = match std::fs::File::open(transcript_path) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("leafhill-hook: PreCompact: cannot open transcript: {}", e);
                    return;
                }
            };

            let formatted_sid = derive_session_id(session_id, cwd);
            let project = project_from_cwd(cwd).to_string();
            let mut messages: Vec<PreCompactMessage> = Vec::new();

            let reader = io::BufReader::new(file);
            for line in reader.lines() {
                let line = match line {
                    Ok(l) => l,
                    Err(e) => {
                        eprintln!("leafhill-hook: PreCompact: read error: {}", e);
                        continue;
                    }
                };
                if line.trim().is_empty() { continue; }

                let event_obj: Value = match serde_json::from_str(&line) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("leafhill-hook: PreCompact: malformed JSONL line: {}", e);
                        continue;
                    }
                };

                let event_type = event_obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if event_type != "user" && event_type != "assistant" {
                    continue;
                }

                let message = match event_obj.get("message") {
                    Some(m) => m,
                    None => continue,
                };

                let role = message.get("role")
                    .and_then(|v| v.as_str())
                    .unwrap_or(event_type)
                    .to_string();

                let content = extract_content(message);
                if content.is_empty() { continue; }

                let model = message.get("model")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let usage = message.get("usage");
                let input_tokens = usage
                    .and_then(|u| u.get("input_tokens"))
                    .and_then(|v| v.as_i64());
                let output_tokens = usage
                    .and_then(|u| u.get("output_tokens"))
                    .and_then(|v| v.as_i64());
                let cache_creation_tokens = usage
                    .and_then(|u| u.get("cache_creation_input_tokens"))
                    .and_then(|v| v.as_i64());
                let cache_read_tokens = usage
                    .and_then(|u| u.get("cache_read_input_tokens"))
                    .and_then(|v| v.as_i64());

                let message_timestamp = event_obj.get("timestamp")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                messages.push(PreCompactMessage {
                    session_id: formatted_sid.clone(),
                    role,
                    content,
                    project: project.clone(),
                    model,
                    input_tokens,
                    output_tokens,
                    cache_creation_tokens,
                    cache_read_tokens,
                    message_timestamp,
                });
            }

            if messages.is_empty() { return; }

            let db = match Database::open(db_path) {
                Ok(db) => db,
                Err(e) => { eprintln!("leafhill-hook: db error: {}", e); return; }
            };
            match db.store_pre_compact_batch(&messages) {
                Ok(count) => {
                    eprintln!("leafhill-hook: PreCompact: stored {} messages", count);
                }
                Err(e) => {
                    eprintln!("leafhill-hook: PreCompact: db write failed: {}", e);
                }
            }
        }
        _ => {
            eprintln!("leafhill-hook: ignoring event: {}", event);
        }
    }
}

/// Extract text content from a transcript message.
/// For string content: return as-is.
/// For content arrays: extract text and thinking blocks, skip tool_use/tool_result.
pub(crate) fn extract_content(message: &Value) -> String {
    let content = match message.get("content") {
        Some(c) => c,
        None => return String::new(),
    };

    // String content (typical for user messages)
    if let Some(s) = content.as_str() {
        return s.to_string();
    }

    // Array content (assistant messages, tool results)
    if let Some(arr) = content.as_array() {
        let mut parts: Vec<String> = Vec::new();
        for block in arr {
            let block_type = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
            match block_type {
                "text" => {
                    if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                        parts.push(text.to_string());
                    }
                }
                "thinking" => {
                    if let Some(thinking) = block.get("thinking").and_then(|v| v.as_str()) {
                        parts.push(format!("[thinking] {}", thinking));
                    }
                }
                "tool_result" => {
                    // User tool_result: serialize as JSON for completeness
                    if let Some(result_content) = block.get("content") {
                        if let Some(s) = result_content.as_str() {
                            parts.push(s.to_string());
                        } else {
                            parts.push(result_content.to_string());
                        }
                    }
                }
                // Skip tool_use blocks
                _ => {}
            }
        }
        return parts.join("\n");
    }

    // Fallback: serialize whatever it is
    content.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── extract_content tests ────────────────────────────────────────

    #[test]
    fn test_extract_content_string() {
        let msg = json!({"content": "hello world"});
        assert_eq!(extract_content(&msg), "hello world");
    }

    #[test]
    fn test_extract_content_text_and_thinking() {
        let msg = json!({
            "content": [
                {"type": "text", "text": "response text"},
                {"type": "thinking", "thinking": "internal thought"},
                {"type": "tool_use", "name": "bash", "input": {}}
            ]
        });
        let result = extract_content(&msg);
        assert!(result.contains("response text"));
        assert!(result.contains("[thinking] internal thought"));
        assert!(!result.contains("bash")); // tool_use skipped
    }

    #[test]
    fn test_extract_content_tool_result() {
        let msg = json!({
            "content": [
                {"type": "tool_result", "content": "tool output text"}
            ]
        });
        assert_eq!(extract_content(&msg), "tool output text");
    }

    #[test]
    fn test_extract_content_tool_result_object() {
        let msg = json!({
            "content": [
                {"type": "tool_result", "content": {"key": "value"}}
            ]
        });
        let result = extract_content(&msg);
        assert!(result.contains("key"));
        assert!(result.contains("value"));
    }

    #[test]
    fn test_extract_content_missing() {
        let msg = json!({"role": "user"});
        assert_eq!(extract_content(&msg), "");
    }

    #[test]
    fn test_extract_content_empty_array() {
        let msg = json!({"content": []});
        assert_eq!(extract_content(&msg), "");
    }

    // ── derive_session_id tests ──────────────────────────────────────

    #[test]
    fn test_derive_session_id_format() {
        let result = derive_session_id("abc123", "/home/user/myproject");
        assert_eq!(result, "abc123-myproject");
    }

    // ── project_from_cwd tests ───────────────────────────────────────

    #[test]
    fn test_project_from_cwd_normal() {
        assert_eq!(project_from_cwd("/home/user/myproject"), "myproject");
    }

    #[test]
    fn test_project_from_cwd_trailing_slash() {
        // rsplit('/') on trailing slash gives empty first, then the dir name
        let result = project_from_cwd("/home/user/myproject/");
        // With trailing slash, rsplit('/').next() is "", which maps to "unknown0"
        assert_eq!(result, "unknown0");
    }

    #[test]
    fn test_project_from_cwd_empty() {
        assert_eq!(project_from_cwd(""), "unknown0");
    }
}
