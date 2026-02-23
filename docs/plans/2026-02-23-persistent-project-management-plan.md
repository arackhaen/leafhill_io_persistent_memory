# Persistent Project Management Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add persistent task management with semantic links to leafhill-persistent-memory, connecting tasks, conversations, and memories into a queryable knowledge graph.

**Architecture:** Three new SQLite tables (tasks, task_deps, links) added to the existing database via idempotent migration. FTS5 for task search. New MCP tools and CLI subcommands follow existing patterns in the codebase. Links table uses a universal source_type/target_type design to connect any entity types.

**Tech Stack:** Rust, rusqlite (bundled SQLite), clap 4 (CLI), serde_json (MCP JSON-RPC), FTS5 (full-text search)

---

### Task 1: Database Schema and Structs

Add the tasks, task_deps, and links tables to the SQLite database, plus Task/Link structs and row mappers.

**Files:**
- Modify: `src/db.rs:1-106` (add structs after ConversationEntry, extend migrate())

**Step 1: Add structs to db.rs**

Add these after the `ConversationEntry` struct (after line 26):

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Task {
    pub id: i64,
    pub project: String,
    pub subject: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: Option<String>,
    pub task_type: Option<String>,
    pub parent_id: Option<i64>,
    pub due_date: Option<String>,
    pub created_by: Option<String>,
    pub assignee: Option<String>,
    pub owner: Option<String>,
    pub session_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Link {
    pub id: i64,
    pub source_type: String,
    pub source_id: i64,
    pub target_type: String,
    pub target_id: i64,
    pub relation: Option<String>,
    pub created_at: String,
}
```

**Step 2: Extend migrate() with new tables**

Add the following SQL to the `execute_batch` string in `migrate()`, after the existing conversations triggers (before the closing `"`):

```sql
CREATE TABLE IF NOT EXISTS tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project TEXT NOT NULL,
    subject TEXT NOT NULL,
    description TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    priority TEXT DEFAULT 'medium',
    task_type TEXT DEFAULT 'claude',
    parent_id INTEGER REFERENCES tasks(id),
    due_date TEXT,
    created_by TEXT,
    assignee TEXT,
    owner TEXT,
    session_id TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS task_deps (
    blocker_id INTEGER NOT NULL REFERENCES tasks(id),
    blocked_id INTEGER NOT NULL REFERENCES tasks(id),
    PRIMARY KEY (blocker_id, blocked_id)
);

CREATE TABLE IF NOT EXISTS links (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_type TEXT NOT NULL,
    source_id INTEGER NOT NULL,
    target_type TEXT NOT NULL,
    target_id INTEGER NOT NULL,
    relation TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(source_type, source_id, target_type, target_id)
);

CREATE VIRTUAL TABLE IF NOT EXISTS tasks_fts USING fts5(
    subject, description, content=tasks, content_rowid=id
);

CREATE TRIGGER IF NOT EXISTS tasks_ai AFTER INSERT ON tasks BEGIN
    INSERT INTO tasks_fts(rowid, subject, description)
    VALUES (new.id, new.subject, new.description);
END;

CREATE TRIGGER IF NOT EXISTS tasks_ad AFTER DELETE ON tasks BEGIN
    INSERT INTO tasks_fts(tasks_fts, rowid, subject, description)
    VALUES ('delete', old.id, old.subject, old.description);
END;

CREATE TRIGGER IF NOT EXISTS tasks_au AFTER UPDATE ON tasks BEGIN
    INSERT INTO tasks_fts(tasks_fts, rowid, subject, description)
    VALUES ('delete', old.id, old.subject, old.description);
    INSERT INTO tasks_fts(rowid, subject, description)
    VALUES (new.id, new.subject, new.description);
END;
```

**Step 3: Add row mapper helpers**

Add these as methods on `impl Database` (after the existing `row_to_conversation` method):

```rust
fn row_to_task(row: &rusqlite::Row) -> rusqlite::Result<Task> {
    Ok(Task {
        id: row.get(0)?,
        project: row.get(1)?,
        subject: row.get(2)?,
        description: row.get(3)?,
        status: row.get(4)?,
        priority: row.get(5)?,
        task_type: row.get(6)?,
        parent_id: row.get(7)?,
        due_date: row.get(8)?,
        created_by: row.get(9)?,
        assignee: row.get(10)?,
        owner: row.get(11)?,
        session_id: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

fn row_to_link(row: &rusqlite::Row) -> rusqlite::Result<Link> {
    Ok(Link {
        id: row.get(0)?,
        source_type: row.get(1)?,
        source_id: row.get(2)?,
        target_type: row.get(3)?,
        target_id: row.get(4)?,
        relation: row.get(5)?,
        created_at: row.get(6)?,
    })
}
```

**Step 4: Build and verify**

Run: `cargo build --release 2>&1`
Expected: Compilation succeeds with no errors. Warnings about unused structs/methods are OK at this stage.

**Step 5: Verify migration runs**

Run: `CLAUDE_MEMORY_DB=/tmp/test_pm.db ./target/release/leafhill-persistent-memory list`
Expected: "No memories found." (no crash, migration ran successfully including new tables)

Run: `rm /tmp/test_pm.db`

**Step 6: Commit**

```bash
git add src/db.rs
git commit -m "feat: add tasks, task_deps, links tables with FTS5"
```

---

### Task 2: Database Methods for Tasks

Implement CRUD operations for the tasks table.

**Files:**
- Modify: `src/db.rs` (add methods to `impl Database`)

**Step 1: Add create_task method**

```rust
pub fn create_task(
    &self,
    project: &str,
    subject: &str,
    description: Option<&str>,
    priority: Option<&str>,
    task_type: Option<&str>,
    parent_id: Option<i64>,
    due_date: Option<&str>,
    created_by: Option<&str>,
    assignee: Option<&str>,
    owner: Option<&str>,
    session_id: Option<&str>,
) -> rusqlite::Result<Task> {
    self.conn.execute(
        "INSERT INTO tasks (project, subject, description, priority, task_type, parent_id, due_date, created_by, assignee, owner, session_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![project, subject, description, priority, task_type, parent_id, due_date, created_by, assignee, owner, session_id],
    )?;
    let id = self.conn.last_insert_rowid();
    self.get_task(id)
}
```

**Step 2: Add get_task method**

```rust
pub fn get_task(&self, id: i64) -> rusqlite::Result<Task> {
    let mut stmt = self.conn.prepare(
        "SELECT id, project, subject, description, status, priority, task_type, parent_id, due_date, created_by, assignee, owner, session_id, created_at, updated_at
         FROM tasks WHERE id = ?1"
    )?;
    stmt.query_row(params![id], Self::row_to_task)
}
```

**Step 3: Add update_task method**

```rust
pub fn update_task(
    &self,
    id: i64,
    updates: &serde_json::Value,
) -> rusqlite::Result<Task> {
    let allowed = ["subject", "description", "status", "priority", "task_type",
                   "assignee", "owner", "due_date", "session_id"];
    let mut sets = Vec::new();
    let mut p: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;

    for field in &allowed {
        if let Some(val) = updates.get(field) {
            if val.is_null() {
                sets.push(format!("{} = NULL", field));
            } else if let Some(s) = val.as_str() {
                sets.push(format!("{} = ?{}", field, idx));
                p.push(Box::new(s.to_string()));
                idx += 1;
            }
        }
    }

    if sets.is_empty() {
        return self.get_task(id);
    }

    sets.push("updated_at = datetime('now')".to_string());
    let sql = format!("UPDATE tasks SET {} WHERE id = ?{}", sets.join(", "), idx);
    p.push(Box::new(id));

    self.conn.execute(&sql, rusqlite::params_from_iter(p.iter()))?;
    self.get_task(id)
}
```

**Step 4: Add list_tasks method**

```rust
pub fn list_tasks(
    &self,
    project: Option<&str>,
    status: Option<&str>,
    assignee: Option<&str>,
    task_type: Option<&str>,
    priority: Option<&str>,
    limit: usize,
) -> rusqlite::Result<Vec<Task>> {
    let mut sql = String::from(
        "SELECT id, project, subject, description, status, priority, task_type, parent_id, due_date, created_by, assignee, owner, session_id, created_at, updated_at
         FROM tasks"
    );
    let mut p: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;
    let mut has_where = false;

    let filters: Vec<(&str, Option<&str>)> = vec![
        ("project", project),
        ("status", status),
        ("assignee", assignee),
        ("task_type", task_type),
        ("priority", priority),
    ];

    for (col, val) in &filters {
        if let Some(v) = val {
            if has_where {
                sql.push_str(&format!(" AND {} = ?{}", col, idx));
            } else {
                sql.push_str(&format!(" WHERE {} = ?{}", col, idx));
                has_where = true;
            }
            p.push(Box::new(v.to_string()));
            idx += 1;
        }
    }

    // Exclude soft-deleted unless explicitly filtering for deleted
    if status.is_none() {
        if has_where {
            sql.push_str(" AND status != 'deleted'");
        } else {
            sql.push_str(" WHERE status != 'deleted'");
        }
    }

    sql.push_str(&format!(" ORDER BY updated_at DESC LIMIT ?{}", idx));
    p.push(Box::new(limit as i64));

    let mut stmt = self.conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(p.iter()), Self::row_to_task)?;
    rows.collect()
}
```

**Step 5: Add search_tasks method**

```rust
pub fn search_tasks(
    &self,
    query: &str,
    project: Option<&str>,
    status: Option<&str>,
    limit: usize,
) -> rusqlite::Result<Vec<Task>> {
    let mut sql = String::from(
        "SELECT t.id, t.project, t.subject, t.description, t.status, t.priority, t.task_type, t.parent_id, t.due_date, t.created_by, t.assignee, t.owner, t.session_id, t.created_at, t.updated_at
         FROM tasks_fts f
         JOIN tasks t ON t.id = f.rowid
         WHERE tasks_fts MATCH ?1"
    );
    let mut p: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(query.to_string())];
    let mut idx = 2;

    if let Some(proj) = project {
        sql.push_str(&format!(" AND t.project = ?{}", idx));
        p.push(Box::new(proj.to_string()));
        idx += 1;
    }

    if let Some(st) = status {
        sql.push_str(&format!(" AND t.status = ?{}", idx));
        p.push(Box::new(st.to_string()));
        idx += 1;
    }

    sql.push_str(&format!(" ORDER BY rank LIMIT ?{}", idx));
    p.push(Box::new(limit as i64));

    let mut stmt = self.conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(p.iter()), Self::row_to_task)?;
    rows.collect()
}
```

**Step 6: Build and verify**

Run: `cargo build --release 2>&1`
Expected: Compilation succeeds.

**Step 7: Commit**

```bash
git add src/db.rs
git commit -m "feat: add task CRUD database methods"
```

---

### Task 3: Database Methods for Dependencies and Links

Implement methods for the task_deps and links tables.

**Files:**
- Modify: `src/db.rs` (add methods to `impl Database`)

**Step 1: Add dependency methods**

```rust
pub fn add_task_dep(&self, blocker_id: i64, blocked_id: i64) -> rusqlite::Result<()> {
    self.conn.execute(
        "INSERT OR IGNORE INTO task_deps (blocker_id, blocked_id) VALUES (?1, ?2)",
        params![blocker_id, blocked_id],
    )?;
    Ok(())
}

pub fn remove_task_dep(&self, blocker_id: i64, blocked_id: i64) -> rusqlite::Result<bool> {
    let affected = self.conn.execute(
        "DELETE FROM task_deps WHERE blocker_id = ?1 AND blocked_id = ?2",
        params![blocker_id, blocked_id],
    )?;
    Ok(affected > 0)
}

pub fn get_task_deps(&self, task_id: i64) -> rusqlite::Result<(Vec<Task>, Vec<Task>)> {
    // Tasks that block this task (blockers)
    let mut stmt = self.conn.prepare(
        "SELECT t.id, t.project, t.subject, t.description, t.status, t.priority, t.task_type, t.parent_id, t.due_date, t.created_by, t.assignee, t.owner, t.session_id, t.created_at, t.updated_at
         FROM task_deps d JOIN tasks t ON t.id = d.blocker_id
         WHERE d.blocked_id = ?1"
    )?;
    let blockers: Vec<Task> = stmt.query_map(params![task_id], Self::row_to_task)?.collect::<rusqlite::Result<_>>()?;

    // Tasks blocked by this task
    let mut stmt = self.conn.prepare(
        "SELECT t.id, t.project, t.subject, t.description, t.status, t.priority, t.task_type, t.parent_id, t.due_date, t.created_by, t.assignee, t.owner, t.session_id, t.created_at, t.updated_at
         FROM task_deps d JOIN tasks t ON t.id = d.blocked_id
         WHERE d.blocker_id = ?1"
    )?;
    let blocked: Vec<Task> = stmt.query_map(params![task_id], Self::row_to_task)?.collect::<rusqlite::Result<_>>()?;

    Ok((blockers, blocked))
}
```

**Step 2: Add link methods**

```rust
pub fn create_link(
    &self,
    source_type: &str,
    source_id: i64,
    target_type: &str,
    target_id: i64,
    relation: Option<&str>,
) -> rusqlite::Result<Link> {
    self.conn.execute(
        "INSERT INTO links (source_type, source_id, target_type, target_id, relation)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(source_type, source_id, target_type, target_id) DO UPDATE SET
            relation = excluded.relation",
        params![source_type, source_id, target_type, target_id, relation],
    )?;
    let mut stmt = self.conn.prepare(
        "SELECT id, source_type, source_id, target_type, target_id, relation, created_at
         FROM links WHERE source_type = ?1 AND source_id = ?2 AND target_type = ?3 AND target_id = ?4"
    )?;
    stmt.query_row(params![source_type, source_id, target_type, target_id], Self::row_to_link)
}

pub fn get_links(
    &self,
    entity_type: &str,
    entity_id: i64,
) -> rusqlite::Result<Vec<Link>> {
    let mut stmt = self.conn.prepare(
        "SELECT id, source_type, source_id, target_type, target_id, relation, created_at
         FROM links
         WHERE (source_type = ?1 AND source_id = ?2)
            OR (target_type = ?1 AND target_id = ?2)
         ORDER BY created_at DESC"
    )?;
    let rows = stmt.query_map(params![entity_type, entity_id], Self::row_to_link)?;
    rows.collect()
}

pub fn delete_link(&self, link_id: i64) -> rusqlite::Result<bool> {
    let affected = self.conn.execute(
        "DELETE FROM links WHERE id = ?1",
        params![link_id],
    )?;
    Ok(affected > 0)
}

pub fn search_linked(
    &self,
    entity_type: &str,
    entity_id: i64,
    target_type: Option<&str>,
) -> rusqlite::Result<Vec<Link>> {
    let mut sql = String::from(
        "SELECT id, source_type, source_id, target_type, target_id, relation, created_at
         FROM links
         WHERE (source_type = ?1 AND source_id = ?2)"
    );
    let mut p: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
        Box::new(entity_type.to_string()),
        Box::new(entity_id),
    ];
    let mut idx = 3;

    if let Some(tt) = target_type {
        sql.push_str(&format!(" AND target_type = ?{}", idx));
        p.push(Box::new(tt.to_string()));
        idx += 1;
        // Also check reverse direction with target_type filter
        sql.push_str(&format!(
            " UNION SELECT id, source_type, source_id, target_type, target_id, relation, created_at
             FROM links WHERE target_type = ?1 AND target_id = ?2 AND source_type = ?{}",
            idx
        ));
        p.push(Box::new(tt.to_string()));
    } else {
        sql.push_str(
            " UNION SELECT id, source_type, source_id, target_type, target_id, relation, created_at
             FROM links WHERE target_type = ?1 AND target_id = ?2"
        );
    }

    sql.push_str(" ORDER BY created_at DESC");

    let mut stmt = self.conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(p.iter()), Self::row_to_link)?;
    rows.collect()
}
```

**Step 3: Build and verify**

Run: `cargo build --release 2>&1`
Expected: Compilation succeeds.

**Step 4: Commit**

```bash
git add src/db.rs
git commit -m "feat: add dependency and link database methods"
```

---

### Task 4: MCP Tools for Tasks

Add MCP tool schemas and handlers for task operations.

**Files:**
- Modify: `src/mcp.rs:88-260` (handle_tools_list — add tool schemas)
- Modify: `src/mcp.rs:262-295` (handle_tools_call — add dispatch)
- Modify: `src/mcp.rs` (add handler functions at end of file)

**Step 1: Add task tool schemas to handle_tools_list**

Add these tool objects to the `"tools"` array in `handle_tools_list`, after the existing `get_conversation_context` tool:

```rust
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
}
```

**Step 2: Add task tool dispatch to handle_tools_call**

Add these cases to the `match tool_name` block in `handle_tools_call`, before the `_ =>` wildcard:

```rust
"create_task" => tool_create_task(&args, db),
"update_task" => tool_update_task(&args, db),
"get_task" => tool_get_task(&args, db),
"list_tasks" => tool_list_tasks(&args, db),
"search_tasks" => tool_search_tasks(&args, db),
"delete_task" => tool_delete_task(&args, db),
```

**Step 3: Add task handler functions**

Add these at the end of `src/mcp.rs`:

```rust
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

    let task = db.create_task(project, subject, description, priority, task_type,
        parent_id, due_date, created_by, assignee, owner, session_id)
        .map_err(|e| format!("DB error: {}", e))?;

    Ok(serde_json::to_string_pretty(&task).unwrap_or_default())
}

fn tool_update_task(args: &Value, db: &Database) -> Result<String, String> {
    let task_id = args.get("task_id").and_then(|v| v.as_i64())
        .ok_or("missing 'task_id'")?;

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
```

**Step 4: Build and verify**

Run: `cargo build --release 2>&1`
Expected: Compilation succeeds.

**Step 5: Commit**

```bash
git add src/mcp.rs
git commit -m "feat: add MCP tools for task CRUD"
```

---

### Task 5: MCP Tools for Dependencies and Links

Add MCP tool schemas and handlers for dependency and link operations.

**Files:**
- Modify: `src/mcp.rs` (tool schemas, dispatch, handlers)

**Step 1: Add dependency and link tool schemas**

Add these to the `"tools"` array in `handle_tools_list`, after the task tools:

```rust
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
```

**Step 2: Add dispatch entries**

Add to the `match tool_name` block in `handle_tools_call`:

```rust
"add_task_dep" => tool_add_task_dep(&args, db),
"remove_task_dep" => tool_remove_task_dep(&args, db),
"create_link" => tool_create_link(&args, db),
"get_links" => tool_get_links(&args, db),
"delete_link" => tool_delete_link(&args, db),
"search_linked" => tool_search_linked(&args, db),
```

**Step 3: Add handler functions**

```rust
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
```

**Step 4: Build and verify**

Run: `cargo build --release 2>&1`
Expected: Compilation succeeds.

**Step 5: Commit**

```bash
git add src/mcp.rs
git commit -m "feat: add MCP tools for dependencies and links"
```

---

### Task 6: CLI Commands for Tasks

Add task subcommand with create, list, get, update, search, delete, deps, add-dep, remove-dep.

**Files:**
- Modify: `src/cli.rs` (add TaskCommands enum, Task variant in Commands, handlers in run_cli)

**Step 1: Add TaskCommands enum and Task variant**

Add `TaskCommands` enum after `LogCommands` in `src/cli.rs`:

```rust
#[derive(Subcommand)]
pub enum TaskCommands {
    /// Create a new task
    Create {
        /// Project name
        project: String,
        /// Task subject (short title)
        subject: String,
        /// Detailed description
        #[arg(long, short)]
        description: Option<String>,
        /// Priority: low, medium, high
        #[arg(long, short)]
        priority: Option<String>,
        /// Type: claude, human, hybrid
        #[arg(long, name = "type")]
        task_type: Option<String>,
        /// Assignee name or email
        #[arg(long, short)]
        assignee: Option<String>,
        /// Owner name or email
        #[arg(long, short)]
        owner: Option<String>,
        /// Due date (YYYY-MM-DD)
        #[arg(long)]
        due: Option<String>,
        /// Parent task ID (for subtasks)
        #[arg(long)]
        parent: Option<i64>,
    },

    /// List tasks with filters
    List {
        /// Filter by project
        #[arg(long, short)]
        project: Option<String>,
        /// Filter by status
        #[arg(long, short)]
        status: Option<String>,
        /// Filter by assignee
        #[arg(long, short)]
        assignee: Option<String>,
        /// Filter by type
        #[arg(long, name = "type")]
        task_type: Option<String>,
        /// Filter by priority
        #[arg(long)]
        priority: Option<String>,
        /// Max results
        #[arg(long, short, default_value = "50")]
        limit: usize,
    },

    /// Get a task by ID (with deps and links)
    Get {
        /// Task ID
        id: i64,
    },

    /// Update a task
    Update {
        /// Task ID
        id: i64,
        /// New status
        #[arg(long, short)]
        status: Option<String>,
        /// New subject
        #[arg(long)]
        subject: Option<String>,
        /// New description
        #[arg(long, short)]
        description: Option<String>,
        /// New assignee
        #[arg(long, short)]
        assignee: Option<String>,
        /// New owner
        #[arg(long, short)]
        owner: Option<String>,
        /// New priority
        #[arg(long, short)]
        priority: Option<String>,
        /// New due date
        #[arg(long)]
        due: Option<String>,
    },

    /// Search tasks by text
    Search {
        /// Search query
        query: String,
        /// Filter by project
        #[arg(long, short)]
        project: Option<String>,
        /// Filter by status
        #[arg(long, short)]
        status: Option<String>,
        /// Max results
        #[arg(long, short, default_value = "20")]
        limit: usize,
    },

    /// Delete a task (soft-delete)
    Delete {
        /// Task ID
        id: i64,
    },

    /// Show dependencies for a task
    Deps {
        /// Task ID
        id: i64,
    },

    /// Add a dependency between tasks
    AddDep {
        /// Blocker task ID
        blocker: i64,
        /// Blocked task ID
        blocked: i64,
    },

    /// Remove a dependency between tasks
    RemoveDep {
        /// Blocker task ID
        blocker: i64,
        /// Blocked task ID
        blocked: i64,
    },
}
```

Add the `Task` variant to the `Commands` enum (before `Log`):

```rust
/// Task management operations
Task {
    #[command(subcommand)]
    command: TaskCommands,
},
```

**Step 2: Add task CLI handlers in run_cli**

Add this match arm in `run_cli`, inside the main `match command` block (before `Commands::Log`):

```rust
Commands::Task { command: task_cmd } => match task_cmd {
    TaskCommands::Create { project, subject, description, priority, task_type, assignee, owner, due, parent } => {
        match db.create_task(&project, &subject, description.as_deref(), priority.as_deref(),
            task_type.as_deref(), parent, due.as_deref(), None, assignee.as_deref(),
            owner.as_deref(), None)
        {
            Ok(task) => print_task(&task),
            Err(e) => { eprintln!("Failed to create task: {}", e); std::process::exit(1); }
        }
    }
    TaskCommands::List { project, status, assignee, task_type, priority, limit } => {
        match db.list_tasks(project.as_deref(), status.as_deref(), assignee.as_deref(),
            task_type.as_deref(), priority.as_deref(), limit)
        {
            Ok(tasks) => {
                if tasks.is_empty() {
                    println!("No tasks found.");
                } else {
                    for t in &tasks { print_task_short(t); }
                    println!("\n({} tasks)", tasks.len());
                }
            }
            Err(e) => { eprintln!("List failed: {}", e); std::process::exit(1); }
        }
    }
    TaskCommands::Get { id } => {
        match db.get_task(id) {
            Ok(task) => {
                print_task(&task);
                if let Ok((blockers, blocked)) = db.get_task_deps(id) {
                    if !blockers.is_empty() {
                        println!("  Blocked by:");
                        for t in &blockers { println!("    #{}: {} [{}]", t.id, t.subject, t.status); }
                    }
                    if !blocked.is_empty() {
                        println!("  Blocks:");
                        for t in &blocked { println!("    #{}: {} [{}]", t.id, t.subject, t.status); }
                    }
                }
                if let Ok(links) = db.get_links("task", id) {
                    if !links.is_empty() {
                        println!("  Links:");
                        for l in &links {
                            let rel = l.relation.as_deref().unwrap_or("linked");
                            println!("    {} {}:{} -> {}:{}", rel, l.source_type, l.source_id, l.target_type, l.target_id);
                        }
                    }
                }
            }
            Err(e) => { eprintln!("Task not found: {}", e); std::process::exit(1); }
        }
    }
    TaskCommands::Update { id, status, subject, description, assignee, owner, priority, due } => {
        let updates = serde_json::json!({
            "status": status,
            "subject": subject,
            "description": description,
            "assignee": assignee,
            "owner": owner,
            "priority": priority,
            "due_date": due,
        });
        match db.update_task(id, &updates) {
            Ok(task) => { println!("Updated:"); print_task(&task); }
            Err(e) => { eprintln!("Update failed: {}", e); std::process::exit(1); }
        }
    }
    TaskCommands::Search { query, project, status, limit } => {
        match db.search_tasks(&query, project.as_deref(), status.as_deref(), limit) {
            Ok(tasks) => {
                if tasks.is_empty() {
                    println!("No tasks found.");
                } else {
                    for t in &tasks { print_task_short(t); }
                    println!("\n({} results)", tasks.len());
                }
            }
            Err(e) => { eprintln!("Search failed: {}", e); std::process::exit(1); }
        }
    }
    TaskCommands::Delete { id } => {
        let updates = serde_json::json!({"status": "deleted"});
        match db.update_task(id, &updates) {
            Ok(_) => println!("Task {} deleted.", id),
            Err(e) => { eprintln!("Delete failed: {}", e); std::process::exit(1); }
        }
    }
    TaskCommands::Deps { id } => {
        match db.get_task_deps(id) {
            Ok((blockers, blocked)) => {
                if blockers.is_empty() && blocked.is_empty() {
                    println!("No dependencies for task {}.", id);
                } else {
                    if !blockers.is_empty() {
                        println!("Blocked by:");
                        for t in &blockers { println!("  #{}: {} [{}]", t.id, t.subject, t.status); }
                    }
                    if !blocked.is_empty() {
                        println!("Blocks:");
                        for t in &blocked { println!("  #{}: {} [{}]", t.id, t.subject, t.status); }
                    }
                }
            }
            Err(e) => { eprintln!("Failed: {}", e); std::process::exit(1); }
        }
    }
    TaskCommands::AddDep { blocker, blocked } => {
        match db.add_task_dep(blocker, blocked) {
            Ok(()) => println!("Dependency added: task {} blocks task {}", blocker, blocked),
            Err(e) => { eprintln!("Failed: {}", e); std::process::exit(1); }
        }
    }
    TaskCommands::RemoveDep { blocker, blocked } => {
        match db.remove_task_dep(blocker, blocked) {
            Ok(true) => println!("Dependency removed."),
            Ok(false) => { eprintln!("Dependency not found."); std::process::exit(1); }
            Err(e) => { eprintln!("Failed: {}", e); std::process::exit(1); }
        }
    }
},
```

**Step 3: Add print helpers**

Add at the end of `src/cli.rs`:

```rust
fn print_task(task: &crate::db::Task) {
    println!("---");
    println!("#{} [{}] {} ({})", task.id, task.status, task.subject, task.project);
    if let Some(desc) = &task.description {
        println!("  {}", desc);
    }
    if let Some(p) = &task.priority { println!("  Priority: {}", p); }
    if let Some(tt) = &task.task_type { println!("  Type: {}", tt); }
    if let Some(a) = &task.assignee { println!("  Assignee: {}", a); }
    if let Some(o) = &task.owner { println!("  Owner: {}", o); }
    if let Some(d) = &task.due_date { println!("  Due: {}", d); }
    if let Some(pid) = &task.parent_id { println!("  Parent: #{}", pid); }
    if let Some(cb) = &task.created_by { println!("  Created by: {}", cb); }
    if let Some(sid) = &task.session_id { println!("  Session: {}", sid); }
    println!("  Created: {}  Updated: {}", task.created_at, task.updated_at);
}

fn print_task_short(task: &crate::db::Task) {
    let priority = task.priority.as_deref().unwrap_or("-");
    let assignee = task.assignee.as_deref().unwrap_or("-");
    let ttype = task.task_type.as_deref().unwrap_or("-");
    println!("  #{:<4} [{}] {:<10} {:<6} {:<8} {}",
        task.id, task.status, task.project, priority, assignee,
        if ttype != "-" { format!("[{}] {}", ttype, task.subject) } else { task.subject.clone() });
}
```

**Step 4: Build and verify**

Run: `cargo build --release 2>&1`
Expected: Compilation succeeds.

**Step 5: Test task CLI**

```bash
# Create test tasks
CLAUDE_MEMORY_DB=/tmp/test_pm.db ./target/release/leafhill-persistent-memory task create myapp "Fix auth bug" --description "Login fails with expired tokens" --priority high --owner samileh
CLAUDE_MEMORY_DB=/tmp/test_pm.db ./target/release/leafhill-persistent-memory task create myapp "Write tests for auth" --priority medium
CLAUDE_MEMORY_DB=/tmp/test_pm.db ./target/release/leafhill-persistent-memory task list --project myapp
CLAUDE_MEMORY_DB=/tmp/test_pm.db ./target/release/leafhill-persistent-memory task get 1
CLAUDE_MEMORY_DB=/tmp/test_pm.db ./target/release/leafhill-persistent-memory task update 1 --status in_progress
CLAUDE_MEMORY_DB=/tmp/test_pm.db ./target/release/leafhill-persistent-memory task add-dep 1 2
CLAUDE_MEMORY_DB=/tmp/test_pm.db ./target/release/leafhill-persistent-memory task deps 2
CLAUDE_MEMORY_DB=/tmp/test_pm.db ./target/release/leafhill-persistent-memory task search "auth"
CLAUDE_MEMORY_DB=/tmp/test_pm.db ./target/release/leafhill-persistent-memory task delete 2
```

Expected: Each command produces output without errors. `list` shows 2 tasks, `deps 2` shows task 1 as blocker, `search` finds both tasks, `delete` soft-deletes task 2.

Run: `rm /tmp/test_pm.db`

**Step 6: Commit**

```bash
git add src/cli.rs
git commit -m "feat: add task CLI subcommands"
```

---

### Task 7: CLI Commands for Links

Add link subcommand with create, list, delete.

**Files:**
- Modify: `src/cli.rs` (add LinkCommands enum, Link variant, handlers)

**Step 1: Add LinkCommands enum**

Add after `TaskCommands`:

```rust
#[derive(Subcommand)]
pub enum LinkCommands {
    /// Create a link between two entities
    Create {
        /// Source entity type (task, memory, conversation)
        source_type: String,
        /// Source entity ID
        source_id: i64,
        /// Target entity type (task, memory, conversation)
        target_type: String,
        /// Target entity ID
        target_id: i64,
        /// Relation label (discusses, relates_to, caused_by, resolves, etc.)
        #[arg(long, short)]
        relation: Option<String>,
    },

    /// List links for an entity
    List {
        /// Entity type (task, memory, conversation)
        entity_type: String,
        /// Entity ID
        entity_id: i64,
    },

    /// Delete a link
    Delete {
        /// Link ID
        link_id: i64,
    },
}
```

Add the `Link` variant to the `Commands` enum:

```rust
/// Link management (semantic connections between entities)
Link {
    #[command(subcommand)]
    command: LinkCommands,
},
```

**Step 2: Add link CLI handlers**

Add this match arm in `run_cli` (after `Commands::Task`):

```rust
Commands::Link { command: link_cmd } => match link_cmd {
    LinkCommands::Create { source_type, source_id, target_type, target_id, relation } => {
        match db.create_link(&source_type, source_id, &target_type, target_id, relation.as_deref()) {
            Ok(link) => print_link(&link),
            Err(e) => { eprintln!("Failed to create link: {}", e); std::process::exit(1); }
        }
    }
    LinkCommands::List { entity_type, entity_id } => {
        match db.get_links(&entity_type, entity_id) {
            Ok(links) => {
                if links.is_empty() {
                    println!("No links found.");
                } else {
                    for l in &links { print_link(l); }
                    println!("\n({} links)", links.len());
                }
            }
            Err(e) => { eprintln!("List failed: {}", e); std::process::exit(1); }
        }
    }
    LinkCommands::Delete { link_id } => {
        match db.delete_link(link_id) {
            Ok(true) => println!("Link {} deleted.", link_id),
            Ok(false) => { eprintln!("Link not found: {}", link_id); std::process::exit(1); }
            Err(e) => { eprintln!("Delete failed: {}", e); std::process::exit(1); }
        }
    }
},
```

**Step 3: Add print_link helper**

```rust
fn print_link(link: &crate::db::Link) {
    let rel = link.relation.as_deref().unwrap_or("linked");
    println!("---");
    println!("Link #{}: {}:{} --[{}]--> {}:{}",
        link.id, link.source_type, link.source_id, rel,
        link.target_type, link.target_id);
    println!("  Created: {}", link.created_at);
}
```

**Step 4: Build and verify**

Run: `cargo build --release 2>&1`
Expected: Compilation succeeds.

**Step 5: Test link CLI**

```bash
# Create a task and a link
CLAUDE_MEMORY_DB=/tmp/test_pm.db ./target/release/leafhill-persistent-memory task create myapp "Test linking"
CLAUDE_MEMORY_DB=/tmp/test_pm.db ./target/release/leafhill-persistent-memory store facts test-fact "A test fact"
CLAUDE_MEMORY_DB=/tmp/test_pm.db ./target/release/leafhill-persistent-memory link create task 1 memory 1 --relation relates_to
CLAUDE_MEMORY_DB=/tmp/test_pm.db ./target/release/leafhill-persistent-memory link list task 1
CLAUDE_MEMORY_DB=/tmp/test_pm.db ./target/release/leafhill-persistent-memory link delete 1
CLAUDE_MEMORY_DB=/tmp/test_pm.db ./target/release/leafhill-persistent-memory link list task 1
```

Expected: Link created, listed, deleted, then "No links found."

Run: `rm /tmp/test_pm.db`

**Step 6: Commit**

```bash
git add src/cli.rs
git commit -m "feat: add link CLI subcommands"
```

---

### Task 8: Integration Test, Version Bump, CLAUDE.md, Install

Full end-to-end verification, version update, CLAUDE.md instructions for automatic linking, binary installation.

**Files:**
- Modify: `Cargo.toml:3` (version bump to 1.2.0)
- Modify: `application_version.txt` (update to 1.2.0)
- Modify: `CLAUDE.md` (add task management and linking instructions)

**Step 1: Version bump**

Update `Cargo.toml` version from `"1.1.0"` to `"1.2.0"`.
Update `application_version.txt` content to `1.2.0`.

**Step 2: Update CLAUDE.md**

Add this section after the existing "Conversation Logging" section:

```markdown
## Task Management and Linking

When working on project tasks, use the persistent task management tools:

### Creating tasks
- Use `create_task` to track work items with: project, subject, description, priority, task_type (claude/human/hybrid), assignee, owner
- Set `session_id` to the current session ID from [leafhill-memory] context
- Set `created_by` to the session ID for Claude-created tasks, or the user's name for human-requested tasks
- Set `owner` to the human who owns/approves the work

### Updating tasks
- Update task status as you work: pending -> in_progress -> completed
- Use `update_task` to change any field

### Semantic linking
After EVERY significant action, consider creating links:
- When creating a task: search conversations and memories for related content, create links with `create_link`
- When logging a conversation summary that discusses a task: create a `discusses` link from the conversation to the task
- When storing a memory related to a task: create a `relates_to` link
- Use relation labels: `discusses`, `relates_to`, `caused_by`, `resolves`, `requires_input`, `blocks`

### Retrieving context
- Use `get_task` to see a task with all its dependencies and links
- Use `search_linked` to find all conversations and memories related to a task
- Use `list_tasks` with project filter for project overview
```

**Step 3: Build release**

Run: `cargo build --release 2>&1`
Expected: Compilation succeeds.

**Step 4: Verify version**

Run: `./target/release/leafhill-persistent-memory --version`
Expected: `leafhill-persistent-memory 1.2.0`

**Step 5: Full integration test**

```bash
export CLAUDE_MEMORY_DB=/tmp/test_integration.db

# Create tasks
./target/release/leafhill-persistent-memory task create myapp "Implement auth system" --description "JWT-based authentication" --priority high --owner samileh --type claude
./target/release/leafhill-persistent-memory task create myapp "Review auth design" --priority high --owner samileh --type human
./target/release/leafhill-persistent-memory task create myapp "Write auth tests" --priority medium --type claude

# Dependencies
./target/release/leafhill-persistent-memory task add-dep 1 3
./target/release/leafhill-persistent-memory task deps 3

# Store a memory
./target/release/leafhill-persistent-memory store facts auth-pattern "Use JWT with refresh tokens"

# Create links
./target/release/leafhill-persistent-memory link create task 1 memory 1 --relation relates_to
./target/release/leafhill-persistent-memory link create task 2 task 1 --relation requires_input

# Verify everything
./target/release/leafhill-persistent-memory task list --project myapp
./target/release/leafhill-persistent-memory task get 1
./target/release/leafhill-persistent-memory link list task 1
./target/release/leafhill-persistent-memory task search "auth"

# Update and delete
./target/release/leafhill-persistent-memory task update 1 --status in_progress
./target/release/leafhill-persistent-memory task delete 3
./target/release/leafhill-persistent-memory task list --project myapp

rm /tmp/test_integration.db
unset CLAUDE_MEMORY_DB
```

Expected: All commands succeed. `task get 1` shows the task with deps and links. `list` shows 2 tasks (task 3 deleted). `search` finds auth-related tasks.

**Step 6: Install binary**

```bash
rm ~/.local/bin/leafhill-persistent-memory
cp ./target/release/leafhill-persistent-memory ~/.local/bin/
leafhill-persistent-memory --version
```

Expected: `leafhill-persistent-memory 1.2.0`

**Step 7: Commit**

```bash
git add Cargo.toml application_version.txt CLAUDE.md src/
git commit -m "feat: persistent project management v1.2.0

Add task management with semantic links connecting tasks,
conversations, and memories. Includes MCP tools, CLI commands,
FTS5 search, dependencies, and human/claude task types."
```

**Step 8: Push**

```bash
git push
```
