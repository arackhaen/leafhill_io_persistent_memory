use std::io::Write;
use std::process::{Command, Stdio};

fn temp_db(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("leafhill-mcp-{}-{}", name, std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("test.db")
}

fn cleanup(db: &std::path::Path) {
    let _ = std::fs::remove_dir_all(db.parent().unwrap());
}

/// Send JSON-RPC requests to the MCP server and return stdout
fn mcp_request(db: &std::path::Path, requests: &[serde_json::Value]) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_leafhill-persistent-memory"))
        .env("CLAUDE_MEMORY_DB", db)
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start serve");

    {
        let stdin = child.stdin.as_mut().expect("failed to get stdin");
        for req in requests {
            writeln!(stdin, "{}", serde_json::to_string(req).unwrap()).unwrap();
        }
    }
    // Close stdin to signal EOF
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("failed to wait");
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn test_mcp_tool_dispatch() {
    let db = temp_db("dispatch");

    let requests = vec![
        // Initialize
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {}
        }),
        // Store a memory
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "store_memory",
                "arguments": {
                    "category": "test",
                    "key": "k1",
                    "value": "hello world"
                }
            }
        }),
    ];

    let stdout = mcp_request(&db, &requests);
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(lines.len() >= 2, "expected at least 2 responses, got {}", lines.len());

    // Check initialize response
    let init: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert!(init["result"]["serverInfo"]["name"].as_str().unwrap().contains("leafhill"));

    // Check store_memory response
    let store: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(store["result"]["isError"], false);
    let text = store["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("hello world"));

    cleanup(&db);
}

#[test]
fn test_mcp_invalid_enum_error() {
    let db = temp_db("invalid-enum");

    let requests = vec![
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {}
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "create_task",
                "arguments": {
                    "project": "test",
                    "subject": "Test task",
                    "priority": "super_urgent"
                }
            }
        }),
    ];

    let stdout = mcp_request(&db, &requests);
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(lines.len() >= 2);

    let resp: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(resp["result"]["isError"], true);
    let text = resp["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Invalid priority"));
    assert!(text.contains("super_urgent"));

    cleanup(&db);
}

#[test]
fn test_mcp_unknown_tool() {
    let db = temp_db("unknown-tool");

    let requests = vec![
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {}
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "nonexistent_tool",
                "arguments": {}
            }
        }),
    ];

    let stdout = mcp_request(&db, &requests);
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(lines.len() >= 2);

    let resp: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(resp["result"]["isError"], true);
    let text = resp["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Unknown tool"));

    cleanup(&db);
}
