use serde_json::Value;
use std::io::{self, Read};
use std::path::PathBuf;

use crate::db::Database;

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
        _ => {
            eprintln!("leafhill-hook: ignoring event: {}", event);
        }
    }
}
