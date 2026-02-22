use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use crate::db::Database;

/// MCP JSON-RPC server over stdio.
/// Handles initialize, tools/list, tools/call, and notifications.
pub fn serve(db_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::open(db_path)?;
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    eprintln!("leafhill-persistent-memory: MCP server started (db: {:?})", db_path);

    for line in stdin.lock().lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let request: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("leafhill-persistent-memory: invalid JSON: {}", e);
                continue;
            }
        };

        // Notifications have no "id" — don't respond
        if request.get("id").is_none() {
            let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");
            eprintln!("leafhill-persistent-memory: notification: {}", method);
            continue;
        }

        let id = request["id"].clone();
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");

        let response = match method {
            "initialize" => handle_initialize(&id),
            "tools/list" => handle_tools_list(&id),
            "tools/call" => handle_tools_call(&id, &request["params"], &db),
            "ping" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {}
            }),
            _ => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": -32601,
                    "message": format!("Method not found: {}", method)
                }
            }),
        };

        let response_str = serde_json::to_string(&response)?;
        writeln!(stdout, "{}", response_str)?;
        stdout.flush()?;
    }

    eprintln!("leafhill-persistent-memory: stdin closed, shutting down");
    Ok(())
}

fn handle_initialize(id: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "leafhill-persistent-memory",
                "version": env!("CARGO_PKG_VERSION")
            },
            "instructions": "Persistent memory server for Claude Code. Use store_memory to save insights, preferences, patterns, and facts across sessions. Use search_memories to find relevant past knowledge. Use log_conversation to record significant exchanges. Categories: 'preferences', 'patterns', 'facts', 'insights', 'decisions'."
        }
    })
}

fn handle_tools_list(id: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "tools": [
                {
                    "name": "store_memory",
                    "description": "Store or update a persistent memory. If a memory with the same category+key exists, it will be updated. Use categories like 'preferences', 'patterns', 'facts', 'insights', 'decisions'.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "category": {
                                "type": "string",
                                "description": "Category/namespace (e.g. 'preferences', 'patterns', 'facts', 'insights', 'decisions')"
                            },
                            "key": {
                                "type": "string",
                                "description": "Unique key within the category"
                            },
                            "value": {
                                "type": "string",
                                "description": "The memory content/value"
                            },
                            "tags": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Optional tags for classification"
                            }
                        },
                        "required": ["category", "key", "value"]
                    }
                },
                {
                    "name": "search_memories",
                    "description": "Search memories using full-text search. Returns matching memories ranked by relevance.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "query": {
                                "type": "string",
                                "description": "Full-text search query"
                            },
                            "category": {
                                "type": "string",
                                "description": "Optional category filter"
                            },
                            "limit": {
                                "type": "integer",
                                "description": "Max results (default 20)"
                            }
                        },
                        "required": ["query"]
                    }
                },
                {
                    "name": "list_memories",
                    "description": "List memories, optionally filtered by category. Returns most recently updated first.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "category": {
                                "type": "string",
                                "description": "Optional category filter"
                            },
                            "limit": {
                                "type": "integer",
                                "description": "Max results (default 50)"
                            }
                        },
                        "required": []
                    }
                },
                {
                    "name": "delete_memory",
                    "description": "Delete a memory by its category and key.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "category": {
                                "type": "string",
                                "description": "Category of the memory"
                            },
                            "key": {
                                "type": "string",
                                "description": "Key of the memory"
                            }
                        },
                        "required": ["category", "key"]
                    }
                },
                {
                    "name": "log_conversation",
                    "description": "Log a conversation entry for persistent history. Record significant user/assistant exchanges.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "session_id": {
                                "type": "string",
                                "description": "Session identifier for grouping entries"
                            },
                            "role": {
                                "type": "string",
                                "description": "Role: 'user' or 'assistant'"
                            },
                            "content": {
                                "type": "string",
                                "description": "The conversation content"
                            },
                            "project": {
                                "type": "string",
                                "description": "Optional project/repo context"
                            }
                        },
                        "required": ["session_id", "role", "content"]
                    }
                },
                {
                    "name": "search_conversations",
                    "description": "Search conversation history using full-text search.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "query": {
                                "type": "string",
                                "description": "Full-text search query"
                            },
                            "session_id": {
                                "type": "string",
                                "description": "Optional session ID filter"
                            },
                            "limit": {
                                "type": "integer",
                                "description": "Max results (default 20)"
                            }
                        },
                        "required": ["query"]
                    }
                }
            ]
        }
    })
}

fn handle_tools_call(id: &Value, params: &Value, db: &Database) -> Value {
    let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(json!({}));

    let result = match tool_name {
        "store_memory" => tool_store_memory(&args, db),
        "search_memories" => tool_search_memories(&args, db),
        "list_memories" => tool_list_memories(&args, db),
        "delete_memory" => tool_delete_memory(&args, db),
        "log_conversation" => tool_log_conversation(&args, db),
        "search_conversations" => tool_search_conversations(&args, db),
        _ => Err(format!("Unknown tool: {}", tool_name)),
    };

    match result {
        Ok(content) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": [{ "type": "text", "text": content }],
                "isError": false
            }
        }),
        Err(e) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": [{ "type": "text", "text": e }],
                "isError": true
            }
        }),
    }
}

fn tool_store_memory(args: &Value, db: &Database) -> Result<String, String> {
    let category = args.get("category").and_then(|v| v.as_str())
        .ok_or("missing 'category'")?;
    let key = args.get("key").and_then(|v| v.as_str())
        .ok_or("missing 'key'")?;
    let value = args.get("value").and_then(|v| v.as_str())
        .ok_or("missing 'value'")?;
    let tags: Option<Vec<String>> = args.get("tags")
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let mem = db.store_memory(category, key, value, tags.as_deref())
        .map_err(|e| format!("DB error: {}", e))?;

    Ok(serde_json::to_string_pretty(&mem).unwrap_or_default())
}

fn tool_search_memories(args: &Value, db: &Database) -> Result<String, String> {
    let query = args.get("query").and_then(|v| v.as_str())
        .ok_or("missing 'query'")?;
    let category = args.get("category").and_then(|v| v.as_str());
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;

    let memories = db.search_memories(query, category, limit)
        .map_err(|e| format!("Search error: {}", e))?;

    if memories.is_empty() {
        Ok("No memories found matching the query.".to_string())
    } else {
        Ok(format!("Found {} memories:\n{}",
            memories.len(),
            serde_json::to_string_pretty(&memories).unwrap_or_default()))
    }
}

fn tool_list_memories(args: &Value, db: &Database) -> Result<String, String> {
    let category = args.get("category").and_then(|v| v.as_str());
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;

    let memories = db.list_memories(category, limit)
        .map_err(|e| format!("List error: {}", e))?;

    if memories.is_empty() {
        Ok("No memories found.".to_string())
    } else {
        Ok(format!("Found {} memories:\n{}",
            memories.len(),
            serde_json::to_string_pretty(&memories).unwrap_or_default()))
    }
}

fn tool_delete_memory(args: &Value, db: &Database) -> Result<String, String> {
    let category = args.get("category").and_then(|v| v.as_str())
        .ok_or("missing 'category'")?;
    let key = args.get("key").and_then(|v| v.as_str())
        .ok_or("missing 'key'")?;

    let deleted = db.delete_memory(category, key)
        .map_err(|e| format!("Delete error: {}", e))?;

    if deleted {
        Ok(format!("Deleted memory: {}:{}", category, key))
    } else {
        Err(format!("Memory not found: {}:{}", category, key))
    }
}

fn tool_log_conversation(args: &Value, db: &Database) -> Result<String, String> {
    let session_id = args.get("session_id").and_then(|v| v.as_str())
        .ok_or("missing 'session_id'")?;
    let role = args.get("role").and_then(|v| v.as_str())
        .ok_or("missing 'role'")?;
    let content = args.get("content").and_then(|v| v.as_str())
        .ok_or("missing 'content'")?;
    let project = args.get("project").and_then(|v| v.as_str());

    let entry = db.log_conversation(session_id, role, content, project)
        .map_err(|e| format!("Log error: {}", e))?;

    Ok(serde_json::to_string_pretty(&entry).unwrap_or_default())
}

fn tool_search_conversations(args: &Value, db: &Database) -> Result<String, String> {
    let query = args.get("query").and_then(|v| v.as_str())
        .ok_or("missing 'query'")?;
    let session_id = args.get("session_id").and_then(|v| v.as_str());
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;

    let entries = db.search_conversations(query, session_id, limit)
        .map_err(|e| format!("Search error: {}", e))?;

    if entries.is_empty() {
        Ok("No conversations found matching the query.".to_string())
    } else {
        Ok(format!("Found {} conversation entries:\n{}",
            entries.len(),
            serde_json::to_string_pretty(&entries).unwrap_or_default()))
    }
}
