use serde_json::Value;
use std::io::{self, Read};
use std::path::PathBuf;
use chrono::Local;

use crate::db::Database;

/// Derive a formatted session_id from hook JSON fields.
/// Format: {session_id}-{YYYY-MM-DD-HHMMSS}-{project_name}
fn derive_session_id(session_id: &str, cwd: &str) -> String {
    let project = cwd.rsplit('/').next().unwrap_or("unknown0");
    let project = if project.is_empty() { "unknown0" } else { project };
    let timestamp = Local::now().format("%Y-%m-%d-%H%M%S");
    format!("{}-{}-{}", session_id, timestamp, project)
}

fn derive_project(cwd: &str) -> String {
    let project = cwd.rsplit('/').next().unwrap_or("unknown0");
    if project.is_empty() { "unknown0".to_string() } else { project.to_string() }
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
            let project = derive_project(cwd);
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
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if prompt.is_empty() { return; }
            let formatted_sid = derive_session_id(session_id, cwd);
            let project = derive_project(cwd);
            let db = match Database::open(db_path) {
                Ok(db) => db,
                Err(e) => { eprintln!("leafhill-hook: db error: {}", e); return; }
            };
            let _ = db.log_conversation(
                &formatted_sid, "user", prompt,
                Some(&project), Some("raw_user"), None,
            );
        }
        "Stop" => {
            let stop_active = hook.get("stop_hook_active")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if stop_active { return; }
            let message = hook.get("last_assistant_message")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if message.is_empty() { return; }
            let formatted_sid = derive_session_id(session_id, cwd);
            let project = derive_project(cwd);
            let db = match Database::open(db_path) {
                Ok(db) => db,
                Err(e) => { eprintln!("leafhill-hook: db error: {}", e); return; }
            };
            let _ = db.log_conversation(
                &formatted_sid, "assistant", message,
                Some(&project), Some("raw_assistant"), None,
            );
        }
        _ => {
            eprintln!("leafhill-hook: ignoring event: {}", event);
        }
    }
}
