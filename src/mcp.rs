use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use crate::db::{Database, TaskStatus, TaskPriority, TaskType, EntryType};

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
                            },
                            "entry_type": {
                                "type": "string",
                                "description": "Entry type: 'summary', 'raw_user', or 'raw_assistant'. Default: 'summary' when called by Claude."
                            },
                            "raw_id": {
                                "type": "integer",
                                "description": "Optional ID of the raw conversation entry this summary relates to."
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
                            "entry_type": {
                                "type": "string",
                                "description": "Filter by entry type: 'summary', 'raw_user', 'raw_assistant'. Omit to search all."
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
                    "name": "get_conversation_context",
                    "description": "Get all conversation summaries for a session in chronological order. Use this to load prior session context.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "session_id": {
                                "type": "string",
                                "description": "Session ID to retrieve context for"
                            },
                            "limit": {
                                "type": "integer",
                                "description": "Max results (default 50)"
                            }
                        },
                        "required": ["session_id"]
                    }
                },
                {
                    "name": "create_task",
                    "description": "Create a persistent task. Tasks survive across sessions and can be linked to conversations and memories.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "project": { "type": "string", "description": "Project scope (e.g. 'myapp')" },
                            "subject": { "type": "string", "description": "Short task title in imperative form" },
                            "description": { "type": "string", "description": "Detailed requirements and context" },
                            "priority": { "type": "string", "description": "low, medium, or high (default: medium)" },
                            "task_type": { "type": "string", "description": "claude, human, or hybrid (default: claude)" },
                            "parent_id": { "type": "integer", "description": "Parent task ID for subtasks" },
                            "due_date": { "type": "string", "description": "ISO date (YYYY-MM-DD)" },
                            "created_by": { "type": "string", "description": "Session ID or human name/email" },
                            "assignee": { "type": "string", "description": "Who does the work" },
                            "owner": { "type": "string", "description": "Who owns/approves the task" },
                            "session_id": { "type": "string", "description": "Claude session ID" }
                        },
                        "required": ["project", "subject"]
                    }
                },
                {
                    "name": "update_task",
                    "description": "Update a task's fields. Only provided fields are changed.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "task_id": { "type": "integer", "description": "Task ID to update" },
                            "subject": { "type": "string", "description": "New subject" },
                            "description": { "type": "string", "description": "New description" },
                            "status": { "type": "string", "description": "pending, in_progress, completed, blocked, or deleted" },
                            "priority": { "type": "string", "description": "low, medium, or high" },
                            "task_type": { "type": "string", "description": "claude, human, or hybrid" },
                            "assignee": { "type": "string", "description": "New assignee" },
                            "owner": { "type": "string", "description": "New owner" },
                            "due_date": { "type": "string", "description": "New due date (YYYY-MM-DD)" },
                            "session_id": { "type": "string", "description": "Claude session that last touched this" }
                        },
                        "required": ["task_id"]
                    }
                },
                {
                    "name": "get_task",
                    "description": "Get a task by ID with its dependencies and linked entities.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "task_id": { "type": "integer", "description": "Task ID" }
                        },
                        "required": ["task_id"]
                    }
                },
                {
                    "name": "list_tasks",
                    "description": "List tasks with optional filters. Excludes deleted tasks unless status=deleted is specified.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "project": { "type": "string", "description": "Filter by project" },
                            "status": { "type": "string", "description": "Filter by status" },
                            "assignee": { "type": "string", "description": "Filter by assignee" },
                            "task_type": { "type": "string", "description": "Filter by type: claude, human, hybrid" },
                            "priority": { "type": "string", "description": "Filter by priority" },
                            "limit": { "type": "integer", "description": "Max results (default 50)" }
                        },
                        "required": []
                    }
                },
                {
                    "name": "search_tasks",
                    "description": "Full-text search across task subjects and descriptions.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "query": { "type": "string", "description": "Search query" },
                            "project": { "type": "string", "description": "Filter by project" },
                            "status": { "type": "string", "description": "Filter by status" },
                            "limit": { "type": "integer", "description": "Max results (default 20)" }
                        },
                        "required": ["query"]
                    }
                },
                {
                    "name": "delete_task",
                    "description": "Soft-delete a task (sets status to 'deleted').",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "task_id": { "type": "integer", "description": "Task ID to delete" }
                        },
                        "required": ["task_id"]
                    }
                },
                {
                    "name": "add_task_dep",
                    "description": "Add a dependency: blocker must complete before blocked can start.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "blocker_id": { "type": "integer", "description": "Task ID that must complete first" },
                            "blocked_id": { "type": "integer", "description": "Task ID that's waiting" }
                        },
                        "required": ["blocker_id", "blocked_id"]
                    }
                },
                {
                    "name": "remove_task_dep",
                    "description": "Remove a dependency between two tasks.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "blocker_id": { "type": "integer", "description": "Blocker task ID" },
                            "blocked_id": { "type": "integer", "description": "Blocked task ID" }
                        },
                        "required": ["blocker_id", "blocked_id"]
                    }
                },
                {
                    "name": "create_link",
                    "description": "Link any two entities (task, memory, conversation). Creates a semantic connection with an optional relation label.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "source_type": { "type": "string", "description": "Entity type: task, memory, or conversation" },
                            "source_id": { "type": "integer", "description": "Source entity ID" },
                            "target_type": { "type": "string", "description": "Entity type: task, memory, or conversation" },
                            "target_id": { "type": "integer", "description": "Target entity ID" },
                            "relation": { "type": "string", "description": "Relation label: discusses, relates_to, caused_by, resolves, requires_input, etc." }
                        },
                        "required": ["source_type", "source_id", "target_type", "target_id"]
                    }
                },
                {
                    "name": "get_links",
                    "description": "Get all links for an entity (both directions).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "entity_type": { "type": "string", "description": "Entity type: task, memory, or conversation" },
                            "entity_id": { "type": "integer", "description": "Entity ID" }
                        },
                        "required": ["entity_type", "entity_id"]
                    }
                },
                {
                    "name": "delete_link",
                    "description": "Delete a link by its ID.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "link_id": { "type": "integer", "description": "Link ID to delete" }
                        },
                        "required": ["link_id"]
                    }
                },
                {
                    "name": "search_linked",
                    "description": "Find all entities linked to a given entity, optionally filtered by target type.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "entity_type": { "type": "string", "description": "Source entity type" },
                            "entity_id": { "type": "integer", "description": "Source entity ID" },
                            "target_type": { "type": "string", "description": "Filter linked entities by type" }
                        },
                        "required": ["entity_type", "entity_id"]
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
        "get_conversation_context" => tool_get_conversation_context(&args, db),
        "create_task" => tool_create_task(&args, db),
        "update_task" => tool_update_task(&args, db),
        "get_task" => tool_get_task(&args, db),
        "list_tasks" => tool_list_tasks(&args, db),
        "search_tasks" => tool_search_tasks(&args, db),
        "delete_task" => tool_delete_task(&args, db),
        "add_task_dep" => tool_add_task_dep(&args, db),
        "remove_task_dep" => tool_remove_task_dep(&args, db),
        "create_link" => tool_create_link(&args, db),
        "get_links" => tool_get_links(&args, db),
        "delete_link" => tool_delete_link(&args, db),
        "search_linked" => tool_search_linked(&args, db),
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
    let entry_type = args.get("entry_type").and_then(|v| v.as_str());
    let raw_id = args.get("raw_id").and_then(|v| v.as_i64());

    if let Some(et) = entry_type {
        et.parse::<EntryType>().map_err(|e| e)?;
    }

    let entry = db.log_conversation(session_id, role, content, project, entry_type, raw_id)
        .map_err(|e| format!("Log error: {}", e))?;

    Ok(serde_json::to_string_pretty(&entry).unwrap_or_default())
}

fn tool_search_conversations(args: &Value, db: &Database) -> Result<String, String> {
    let query = args.get("query").and_then(|v| v.as_str())
        .ok_or("missing 'query'")?;
    let session_id = args.get("session_id").and_then(|v| v.as_str());
    let entry_type = args.get("entry_type").and_then(|v| v.as_str());
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;

    let entries = db.search_conversations(query, session_id, entry_type, limit)
        .map_err(|e| format!("Search error: {}", e))?;

    if entries.is_empty() {
        Ok("No conversations found matching the query.".to_string())
    } else {
        Ok(format!("Found {} conversation entries:\n{}",
            entries.len(),
            serde_json::to_string_pretty(&entries).unwrap_or_default()))
    }
}

fn tool_get_conversation_context(args: &Value, db: &Database) -> Result<String, String> {
    let session_id = args.get("session_id").and_then(|v| v.as_str())
        .ok_or("missing 'session_id'")?;
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;

    let entries = db.get_conversation_context(session_id, limit)
        .map_err(|e| format!("Context error: {}", e))?;

    if entries.is_empty() {
        Ok("No conversation context found for this session.".to_string())
    } else {
        Ok(format!("Session context ({} summaries):\n{}",
            entries.len(),
            serde_json::to_string_pretty(&entries).unwrap_or_default()))
    }
}

fn tool_create_task(args: &Value, db: &Database) -> Result<String, String> {
    let project = args.get("project").and_then(|v| v.as_str())
        .ok_or("missing 'project'")?;
    let subject = args.get("subject").and_then(|v| v.as_str())
        .ok_or("missing 'subject'")?;
    let description = args.get("description").and_then(|v| v.as_str());
    let priority = args.get("priority").and_then(|v| v.as_str());
    let task_type = args.get("task_type").and_then(|v| v.as_str());
    let parent_id = args.get("parent_id").and_then(|v| v.as_i64());
    let due_date = args.get("due_date").and_then(|v| v.as_str());
    let created_by = args.get("created_by").and_then(|v| v.as_str());
    let assignee = args.get("assignee").and_then(|v| v.as_str());
    let owner = args.get("owner").and_then(|v| v.as_str());
    let session_id = args.get("session_id").and_then(|v| v.as_str());

    if let Some(p) = priority {
        p.parse::<TaskPriority>().map_err(|e| e)?;
    }
    if let Some(tt) = task_type {
        tt.parse::<TaskType>().map_err(|e| e)?;
    }

    let task = db.create_task(project, subject, description, priority, task_type,
        parent_id, due_date, created_by, assignee, owner, session_id)
        .map_err(|e| format!("DB error: {}", e))?;

    Ok(serde_json::to_string_pretty(&task).unwrap_or_default())
}

fn tool_update_task(args: &Value, db: &Database) -> Result<String, String> {
    let task_id = args.get("task_id").and_then(|v| v.as_i64())
        .ok_or("missing 'task_id'")?;

    if let Some(s) = args.get("status").and_then(|v| v.as_str()) {
        s.parse::<TaskStatus>().map_err(|e| e)?;
    }
    if let Some(p) = args.get("priority").and_then(|v| v.as_str()) {
        p.parse::<TaskPriority>().map_err(|e| e)?;
    }
    if let Some(tt) = args.get("task_type").and_then(|v| v.as_str()) {
        tt.parse::<TaskType>().map_err(|e| e)?;
    }

    let task = db.update_task(task_id, args)
        .map_err(|e| format!("DB error: {}", e))?;

    Ok(serde_json::to_string_pretty(&task).unwrap_or_default())
}

fn tool_get_task(args: &Value, db: &Database) -> Result<String, String> {
    let task_id = args.get("task_id").and_then(|v| v.as_i64())
        .ok_or("missing 'task_id'")?;

    let task = db.get_task(task_id)
        .map_err(|e| format!("DB error: {}", e))?;
    let (blockers, blocked) = db.get_task_deps(task_id)
        .map_err(|e| format!("DB error: {}", e))?;
    let links = db.get_links("task", task_id)
        .map_err(|e| format!("DB error: {}", e))?;

    let result = serde_json::json!({
        "task": task,
        "blocked_by": blockers,
        "blocks": blocked,
        "links": links,
    });

    Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
}

fn tool_list_tasks(args: &Value, db: &Database) -> Result<String, String> {
    let project = args.get("project").and_then(|v| v.as_str());
    let status = args.get("status").and_then(|v| v.as_str());
    let assignee = args.get("assignee").and_then(|v| v.as_str());
    let task_type = args.get("task_type").and_then(|v| v.as_str());
    let priority = args.get("priority").and_then(|v| v.as_str());
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;

    if let Some(s) = status {
        s.parse::<TaskStatus>().map_err(|e| e)?;
    }
    if let Some(tt) = task_type {
        tt.parse::<TaskType>().map_err(|e| e)?;
    }
    if let Some(p) = priority {
        p.parse::<TaskPriority>().map_err(|e| e)?;
    }

    let tasks = db.list_tasks(project, status, assignee, task_type, priority, limit)
        .map_err(|e| format!("List error: {}", e))?;

    if tasks.is_empty() {
        Ok("No tasks found.".to_string())
    } else {
        Ok(format!("Found {} tasks:\n{}",
            tasks.len(),
            serde_json::to_string_pretty(&tasks).unwrap_or_default()))
    }
}

fn tool_search_tasks(args: &Value, db: &Database) -> Result<String, String> {
    let query = args.get("query").and_then(|v| v.as_str())
        .ok_or("missing 'query'")?;
    let project = args.get("project").and_then(|v| v.as_str());
    let status = args.get("status").and_then(|v| v.as_str());
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;

    let tasks = db.search_tasks(query, project, status, limit)
        .map_err(|e| format!("Search error: {}", e))?;

    if tasks.is_empty() {
        Ok("No tasks found matching the query.".to_string())
    } else {
        Ok(format!("Found {} tasks:\n{}",
            tasks.len(),
            serde_json::to_string_pretty(&tasks).unwrap_or_default()))
    }
}

fn tool_delete_task(args: &Value, db: &Database) -> Result<String, String> {
    let task_id = args.get("task_id").and_then(|v| v.as_i64())
        .ok_or("missing 'task_id'")?;

    let updates = serde_json::json!({"status": "deleted"});
    let task = db.update_task(task_id, &updates)
        .map_err(|e| format!("DB error: {}", e))?;

    Ok(format!("Task {} deleted.\n{}", task_id,
        serde_json::to_string_pretty(&task).unwrap_or_default()))
}

fn tool_add_task_dep(args: &Value, db: &Database) -> Result<String, String> {
    let blocker_id = args.get("blocker_id").and_then(|v| v.as_i64())
        .ok_or("missing 'blocker_id'")?;
    let blocked_id = args.get("blocked_id").and_then(|v| v.as_i64())
        .ok_or("missing 'blocked_id'")?;

    db.add_task_dep(blocker_id, blocked_id)
        .map_err(|e| format!("DB error: {}", e))?;

    Ok(format!("Dependency added: task {} blocks task {}", blocker_id, blocked_id))
}

fn tool_remove_task_dep(args: &Value, db: &Database) -> Result<String, String> {
    let blocker_id = args.get("blocker_id").and_then(|v| v.as_i64())
        .ok_or("missing 'blocker_id'")?;
    let blocked_id = args.get("blocked_id").and_then(|v| v.as_i64())
        .ok_or("missing 'blocked_id'")?;

    let removed = db.remove_task_dep(blocker_id, blocked_id)
        .map_err(|e| format!("DB error: {}", e))?;

    if removed {
        Ok(format!("Dependency removed: task {} no longer blocks task {}", blocker_id, blocked_id))
    } else {
        Err(format!("Dependency not found: {} -> {}", blocker_id, blocked_id))
    }
}

fn tool_create_link(args: &Value, db: &Database) -> Result<String, String> {
    let source_type = args.get("source_type").and_then(|v| v.as_str())
        .ok_or("missing 'source_type'")?;
    let source_id = args.get("source_id").and_then(|v| v.as_i64())
        .ok_or("missing 'source_id'")?;
    let target_type = args.get("target_type").and_then(|v| v.as_str())
        .ok_or("missing 'target_type'")?;
    let target_id = args.get("target_id").and_then(|v| v.as_i64())
        .ok_or("missing 'target_id'")?;
    let relation = args.get("relation").and_then(|v| v.as_str());

    let link = db.create_link(source_type, source_id, target_type, target_id, relation)
        .map_err(|e| format!("DB error: {}", e))?;

    Ok(serde_json::to_string_pretty(&link).unwrap_or_default())
}

fn tool_get_links(args: &Value, db: &Database) -> Result<String, String> {
    let entity_type = args.get("entity_type").and_then(|v| v.as_str())
        .ok_or("missing 'entity_type'")?;
    let entity_id = args.get("entity_id").and_then(|v| v.as_i64())
        .ok_or("missing 'entity_id'")?;

    let links = db.get_links(entity_type, entity_id)
        .map_err(|e| format!("DB error: {}", e))?;

    if links.is_empty() {
        Ok("No links found for this entity.".to_string())
    } else {
        Ok(format!("Found {} links:\n{}",
            links.len(),
            serde_json::to_string_pretty(&links).unwrap_or_default()))
    }
}

fn tool_delete_link(args: &Value, db: &Database) -> Result<String, String> {
    let link_id = args.get("link_id").and_then(|v| v.as_i64())
        .ok_or("missing 'link_id'")?;

    let deleted = db.delete_link(link_id)
        .map_err(|e| format!("DB error: {}", e))?;

    if deleted {
        Ok(format!("Link {} deleted.", link_id))
    } else {
        Err(format!("Link not found: {}", link_id))
    }
}

fn tool_search_linked(args: &Value, db: &Database) -> Result<String, String> {
    let entity_type = args.get("entity_type").and_then(|v| v.as_str())
        .ok_or("missing 'entity_type'")?;
    let entity_id = args.get("entity_id").and_then(|v| v.as_i64())
        .ok_or("missing 'entity_id'")?;
    let target_type = args.get("target_type").and_then(|v| v.as_str());

    let links = db.search_linked(entity_type, entity_id, target_type)
        .map_err(|e| format!("Search error: {}", e))?;

    if links.is_empty() {
        Ok("No linked entities found.".to_string())
    } else {
        Ok(format!("Found {} linked entities:\n{}",
            links.len(),
            serde_json::to_string_pretty(&links).unwrap_or_default()))
    }
}
