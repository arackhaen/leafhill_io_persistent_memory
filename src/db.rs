use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

// ── Validation Enums ─────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus { Pending, InProgress, Completed, Blocked, Deleted }

impl FromStr for TaskStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "in_progress" => Ok(Self::InProgress),
            "completed" => Ok(Self::Completed),
            "blocked" => Ok(Self::Blocked),
            "deleted" => Ok(Self::Deleted),
            _ => Err(format!("Invalid status '{}'. Must be one of: pending, in_progress, completed, blocked, deleted", s)),
        }
    }
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::InProgress => write!(f, "in_progress"),
            Self::Completed => write!(f, "completed"),
            Self::Blocked => write!(f, "blocked"),
            Self::Deleted => write!(f, "deleted"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskPriority { Low, Medium, High }

impl FromStr for TaskPriority {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            _ => Err(format!("Invalid priority '{}'. Must be one of: low, medium, high", s)),
        }
    }
}

impl fmt::Display for TaskPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskType { Claude, Human, Hybrid }

impl FromStr for TaskType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "claude" => Ok(Self::Claude),
            "human" => Ok(Self::Human),
            "hybrid" => Ok(Self::Hybrid),
            _ => Err(format!("Invalid task_type '{}'. Must be one of: claude, human, hybrid", s)),
        }
    }
}

impl fmt::Display for TaskType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Claude => write!(f, "claude"),
            Self::Human => write!(f, "human"),
            Self::Hybrid => write!(f, "hybrid"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryType { Summary, RawUser, RawAssistant, PreCompact }

impl FromStr for EntryType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "summary" => Ok(Self::Summary),
            "raw_user" => Ok(Self::RawUser),
            "raw_assistant" => Ok(Self::RawAssistant),
            "pre_compact" => Ok(Self::PreCompact),
            _ => Err(format!("Invalid entry_type '{}'. Must be one of: summary, raw_user, raw_assistant, pre_compact", s)),
        }
    }
}

impl fmt::Display for EntryType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Summary => write!(f, "summary"),
            Self::RawUser => write!(f, "raw_user"),
            Self::RawAssistant => write!(f, "raw_assistant"),
            Self::PreCompact => write!(f, "pre_compact"),
        }
    }
}

/// Convert a validation error string into a rusqlite::Error for use in DB methods.
fn validation_err(msg: String) -> rusqlite::Error {
    rusqlite::Error::InvalidParameterName(msg)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Memory {
    pub id: i64,
    pub category: String,
    pub key: String,
    pub value: String,
    pub tags: Option<Vec<String>>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConversationEntry {
    pub id: i64,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub project: Option<String>,
    pub entry_type: Option<String>,
    pub raw_id: Option<i64>,
    pub model: Option<String>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_creation_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub message_timestamp: Option<String>,
    pub created_at: String,
}

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

pub struct PreCompactMessage {
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub project: String,
    pub model: Option<String>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_creation_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub message_timestamp: Option<String>,
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &PathBuf) -> rusqlite::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = Connection::open(path)?;
        let db = Database { conn };
        db.migrate()?;
        Ok(db)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> rusqlite::Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Database { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS memories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                category TEXT NOT NULL,
                key TEXT NOT NULL,
                value TEXT NOT NULL,
                tags TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(category, key)
            );

            CREATE TABLE IF NOT EXISTS conversations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                project TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
                key, value, tags, content=memories, content_rowid=id
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS conversations_fts USING fts5(
                content, content=conversations, content_rowid=id
            );

            -- Triggers to keep FTS in sync
            CREATE TRIGGER IF NOT EXISTS memories_ai AFTER INSERT ON memories BEGIN
                INSERT INTO memories_fts(rowid, key, value, tags)
                VALUES (new.id, new.key, new.value, new.tags);
            END;

            CREATE TRIGGER IF NOT EXISTS memories_ad AFTER DELETE ON memories BEGIN
                INSERT INTO memories_fts(memories_fts, rowid, key, value, tags)
                VALUES ('delete', old.id, old.key, old.value, old.tags);
            END;

            CREATE TRIGGER IF NOT EXISTS memories_au AFTER UPDATE ON memories BEGIN
                INSERT INTO memories_fts(memories_fts, rowid, key, value, tags)
                VALUES ('delete', old.id, old.key, old.value, old.tags);
                INSERT INTO memories_fts(rowid, key, value, tags)
                VALUES (new.id, new.key, new.value, new.tags);
            END;

            CREATE TRIGGER IF NOT EXISTS conversations_ai AFTER INSERT ON conversations BEGIN
                INSERT INTO conversations_fts(rowid, content)
                VALUES (new.id, new.content);
            END;

            CREATE TRIGGER IF NOT EXISTS conversations_ad AFTER DELETE ON conversations BEGIN
                INSERT INTO conversations_fts(conversations_fts, rowid, content)
                VALUES ('delete', old.id, old.content);
            END;

            CREATE TRIGGER IF NOT EXISTS conversations_au AFTER UPDATE ON conversations BEGIN
                INSERT INTO conversations_fts(conversations_fts, rowid, content)
                VALUES ('delete', old.id, old.content);
                INSERT INTO conversations_fts(rowid, content)
                VALUES (new.id, new.content);
            END;

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
            "
        )?;
        self.conn.execute("ALTER TABLE conversations ADD COLUMN entry_type TEXT", []).ok();
        self.conn.execute("ALTER TABLE conversations ADD COLUMN raw_id INTEGER", []).ok();
        // v1.4: metadata columns for PreCompact transcript storage
        self.conn.execute("ALTER TABLE conversations ADD COLUMN model TEXT", []).ok();
        self.conn.execute("ALTER TABLE conversations ADD COLUMN input_tokens INTEGER", []).ok();
        self.conn.execute("ALTER TABLE conversations ADD COLUMN output_tokens INTEGER", []).ok();
        self.conn.execute("ALTER TABLE conversations ADD COLUMN cache_creation_tokens INTEGER", []).ok();
        self.conn.execute("ALTER TABLE conversations ADD COLUMN cache_read_tokens INTEGER", []).ok();
        self.conn.execute("ALTER TABLE conversations ADD COLUMN message_timestamp TEXT", []).ok();
        Ok(())
    }

    pub fn store_memory(
        &self,
        category: &str,
        key: &str,
        value: &str,
        tags: Option<&[String]>,
    ) -> rusqlite::Result<Memory> {
        let tags_json = tags.map(|t| serde_json::to_string(t).unwrap_or_default());

        self.conn.execute(
            "INSERT INTO memories (category, key, value, tags)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(category, key) DO UPDATE SET
                value = excluded.value,
                tags = excluded.tags,
                updated_at = datetime('now')",
            params![category, key, value, tags_json],
        )?;

        let id = self.conn.last_insert_rowid();
        let mut stmt = self.conn.prepare(
            "SELECT id, category, key, value, tags, created_at, updated_at
             FROM memories WHERE (id = ?1) OR (category = ?2 AND key = ?3)
             ORDER BY updated_at DESC LIMIT 1"
        )?;

        stmt.query_row(params![id, category, key], |row| {
            Ok(Memory {
                id: row.get(0)?,
                category: row.get(1)?,
                key: row.get(2)?,
                value: row.get(3)?,
                tags: row.get::<_, Option<String>>(4)?
                    .and_then(|s| serde_json::from_str(&s).ok()),
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })
    }

    pub fn search_memories(
        &self,
        query: &str,
        category: Option<&str>,
        limit: usize,
    ) -> rusqlite::Result<Vec<Memory>> {
        let sql = if category.is_some() {
            "SELECT m.id, m.category, m.key, m.value, m.tags, m.created_at, m.updated_at
             FROM memories_fts f
             JOIN memories m ON m.id = f.rowid
             WHERE memories_fts MATCH ?1 AND m.category = ?2
             ORDER BY rank
             LIMIT ?3"
        } else {
            "SELECT m.id, m.category, m.key, m.value, m.tags, m.created_at, m.updated_at
             FROM memories_fts f
             JOIN memories m ON m.id = f.rowid
             WHERE memories_fts MATCH ?1
             ORDER BY rank
             LIMIT ?3"
        };

        let mut stmt = self.conn.prepare(sql)?;
        let rows = if let Some(cat) = category {
            stmt.query_map(params![query, cat, limit as i64], Self::row_to_memory)?
        } else {
            stmt.query_map(params![query, "", limit as i64], Self::row_to_memory)?
        };

        rows.collect()
    }

    pub fn list_memories(
        &self,
        category: Option<&str>,
        limit: usize,
    ) -> rusqlite::Result<Vec<Memory>> {
        let (sql, p): (&str, Vec<Box<dyn rusqlite::types::ToSql>>) = if let Some(cat) = category {
            (
                "SELECT id, category, key, value, tags, created_at, updated_at
                 FROM memories WHERE category = ?1
                 ORDER BY updated_at DESC LIMIT ?2",
                vec![Box::new(cat.to_string()), Box::new(limit as i64)],
            )
        } else {
            (
                "SELECT id, category, key, value, tags, created_at, updated_at
                 FROM memories ORDER BY updated_at DESC LIMIT ?1",
                vec![Box::new(limit as i64)],
            )
        };

        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(p.iter()), Self::row_to_memory)?;
        rows.collect()
    }

    pub fn delete_memory(&self, category: &str, key: &str) -> rusqlite::Result<bool> {
        let affected = self.conn.execute(
            "DELETE FROM memories WHERE category = ?1 AND key = ?2",
            params![category, key],
        )?;
        Ok(affected > 0)
    }

    pub fn log_conversation(
        &self,
        session_id: &str,
        role: &str,
        content: &str,
        project: Option<&str>,
        entry_type: Option<&str>,
        raw_id: Option<i64>,
    ) -> rusqlite::Result<ConversationEntry> {
        if let Some(et) = entry_type {
            EntryType::from_str(et).map_err(validation_err)?;
        }
        self.conn.execute(
            "INSERT INTO conversations (session_id, role, content, project, entry_type, raw_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![session_id, role, content, project, entry_type, raw_id],
        )?;

        let id = self.conn.last_insert_rowid();
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, role, content, project, entry_type, raw_id, model, input_tokens, output_tokens, cache_creation_tokens, cache_read_tokens, message_timestamp, created_at
             FROM conversations WHERE id = ?1"
        )?;

        stmt.query_row(params![id], Self::row_to_conversation)
    }

    pub fn search_conversations(
        &self,
        query: &str,
        session_id: Option<&str>,
        entry_type: Option<&str>,
        limit: usize,
    ) -> rusqlite::Result<Vec<ConversationEntry>> {
        let mut sql = String::from(
            "SELECT c.id, c.session_id, c.role, c.content, c.project, c.entry_type, c.raw_id, \
             c.model, c.input_tokens, c.output_tokens, c.cache_creation_tokens, c.cache_read_tokens, \
             c.message_timestamp, c.created_at
             FROM conversations_fts f
             JOIN conversations c ON c.id = f.rowid
             WHERE conversations_fts MATCH ?1"
        );
        let mut p: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(query.to_string())];
        let mut idx = 2;

        if let Some(sid) = session_id {
            sql.push_str(&format!(" AND c.session_id = ?{}", idx));
            p.push(Box::new(sid.to_string()));
            idx += 1;
        }

        if let Some(et) = entry_type {
            sql.push_str(&format!(" AND c.entry_type = ?{}", idx));
            p.push(Box::new(et.to_string()));
            idx += 1;
        }

        sql.push_str(&format!(" ORDER BY rank LIMIT ?{}", idx));
        p.push(Box::new(limit as i64));

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(
            rusqlite::params_from_iter(p.iter()),
            Self::row_to_conversation,
        )?;

        rows.collect()
    }

    pub fn list_conversations(
        &self,
        session_id: Option<&str>,
        entry_type: Option<&str>,
        limit: usize,
    ) -> rusqlite::Result<Vec<ConversationEntry>> {
        let mut sql = String::from(
            "SELECT id, session_id, role, content, project, entry_type, raw_id, model, input_tokens, output_tokens, cache_creation_tokens, cache_read_tokens, message_timestamp, created_at
             FROM conversations"
        );
        let mut p: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut idx = 1;
        let mut has_where = false;

        if let Some(sid) = session_id {
            sql.push_str(&format!(" WHERE session_id = ?{}", idx));
            p.push(Box::new(sid.to_string()));
            idx += 1;
            has_where = true;
        }

        if let Some(et) = entry_type {
            if has_where {
                sql.push_str(&format!(" AND entry_type = ?{}", idx));
            } else {
                sql.push_str(&format!(" WHERE entry_type = ?{}", idx));
            }
            p.push(Box::new(et.to_string()));
            idx += 1;
        }

        sql.push_str(&format!(" ORDER BY created_at DESC LIMIT ?{}", idx));
        p.push(Box::new(limit as i64));

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(
            rusqlite::params_from_iter(p.iter()),
            Self::row_to_conversation,
        )?;
        rows.collect()
    }

    fn row_to_memory(row: &rusqlite::Row) -> rusqlite::Result<Memory> {
        Ok(Memory {
            id: row.get(0)?,
            category: row.get(1)?,
            key: row.get(2)?,
            value: row.get(3)?,
            tags: row.get::<_, Option<String>>(4)?
                .and_then(|s| serde_json::from_str(&s).ok()),
            created_at: row.get(5)?,
            updated_at: row.get(6)?,
        })
    }

    fn row_to_conversation(row: &rusqlite::Row) -> rusqlite::Result<ConversationEntry> {
        Ok(ConversationEntry {
            id: row.get(0)?,
            session_id: row.get(1)?,
            role: row.get(2)?,
            content: row.get(3)?,
            project: row.get(4)?,
            entry_type: row.get(5)?,
            raw_id: row.get(6)?,
            model: row.get(7)?,
            input_tokens: row.get(8)?,
            output_tokens: row.get(9)?,
            cache_creation_tokens: row.get(10)?,
            cache_read_tokens: row.get(11)?,
            message_timestamp: row.get(12)?,
            created_at: row.get(13)?,
        })
    }

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

    pub fn get_conversation_context(
        &self,
        session_id: &str,
        limit: usize,
    ) -> rusqlite::Result<Vec<ConversationEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, role, content, project, entry_type, raw_id, model, input_tokens, output_tokens, cache_creation_tokens, cache_read_tokens, message_timestamp, created_at
             FROM conversations
             WHERE session_id = ?1 AND entry_type = 'summary'
             ORDER BY created_at ASC
             LIMIT ?2"
        )?;
        let rows = stmt.query_map(params![session_id, limit as i64], Self::row_to_conversation)?;
        rows.collect()
    }

    pub fn prune_conversations(
        &self,
        older_than_days: i64,
        entry_type: Option<&str>,
    ) -> rusqlite::Result<usize> {
        if let Some(et) = entry_type {
            self.conn.execute(
                "DELETE FROM conversations WHERE created_at < datetime('now', ?1) AND entry_type = ?2",
                params![format!("-{} days", older_than_days), et],
            )
        } else {
            self.conn.execute(
                "DELETE FROM conversations WHERE created_at < datetime('now', ?1)",
                params![format!("-{} days", older_than_days)],
            )
        }
    }

    // ── Task CRUD ──────────────────────────────────────────────────────

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
        if let Some(p) = priority {
            TaskPriority::from_str(p).map_err(validation_err)?;
        }
        if let Some(tt) = task_type {
            TaskType::from_str(tt).map_err(validation_err)?;
        }
        self.conn.execute(
            "INSERT INTO tasks (project, subject, description, priority, task_type, parent_id, due_date, created_by, assignee, owner, session_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![project, subject, description, priority, task_type, parent_id, due_date, created_by, assignee, owner, session_id],
        )?;
        let id = self.conn.last_insert_rowid();
        self.get_task(id)
    }

    pub fn get_task(&self, id: i64) -> rusqlite::Result<Task> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project, subject, description, status, priority, task_type, parent_id, due_date, created_by, assignee, owner, session_id, created_at, updated_at
             FROM tasks WHERE id = ?1"
        )?;
        stmt.query_row(params![id], Self::row_to_task)
    }

    pub fn update_task(
        &self,
        id: i64,
        updates: &serde_json::Value,
    ) -> rusqlite::Result<Task> {
        // Validate enum fields if present
        if let Some(s) = updates.get("status").and_then(|v| v.as_str()) {
            TaskStatus::from_str(s).map_err(validation_err)?;
        }
        if let Some(p) = updates.get("priority").and_then(|v| v.as_str()) {
            TaskPriority::from_str(p).map_err(validation_err)?;
        }
        if let Some(tt) = updates.get("task_type").and_then(|v| v.as_str()) {
            TaskType::from_str(tt).map_err(validation_err)?;
        }

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

    // ── Task Dependencies ────────────────────────────────────────────────

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

    // ── Links ────────────────────────────────────────────────────────────

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

    // ── Backup ────────────────────────────────────────────────────────────

    pub fn backup_to(&self, path: &str) -> rusqlite::Result<()> {
        self.conn.execute("VACUUM INTO ?1", params![path])?;
        Ok(())
    }

    pub fn table_counts(&self) -> rusqlite::Result<Vec<(String, i64)>> {
        let tables = ["memories", "conversations", "tasks", "task_deps", "links"];
        let mut counts = Vec::new();
        for table in &tables {
            let count: i64 = self.conn.query_row(
                &format!("SELECT COUNT(*) FROM {}", table),
                [],
                |row| row.get(0),
            )?;
            counts.push((table.to_string(), count));
        }
        Ok(counts)
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

    // ── Archive queries ──────────────────────────────────────────────────

    pub fn query_memories_for_archive(
        &self,
        category: Option<&str>,
        older_than_days: Option<i64>,
        limit: Option<usize>,
    ) -> rusqlite::Result<Vec<Memory>> {
        let mut sql = String::from(
            "SELECT id, category, key, value, tags, created_at, updated_at FROM memories"
        );
        let mut p: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut idx = 1;
        let mut has_where = false;

        if let Some(cat) = category {
            sql.push_str(&format!(" WHERE category = ?{}", idx));
            p.push(Box::new(cat.to_string()));
            idx += 1;
            has_where = true;
        }

        if let Some(days) = older_than_days {
            let clause = format!(
                " {} updated_at < datetime('now', ?{})",
                if has_where { "AND" } else { "WHERE" },
                idx
            );
            sql.push_str(&clause);
            p.push(Box::new(format!("-{} days", days)));
            idx += 1;
        }

        sql.push_str(" ORDER BY id ASC");
        if let Some(n) = limit {
            sql.push_str(&format!(" LIMIT ?{}", idx));
            p.push(Box::new(n as i64));
        }

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(p.iter()), Self::row_to_memory)?;
        rows.collect()
    }

    pub fn query_conversations_for_archive(
        &self,
        project: Option<&str>,
        older_than_days: Option<i64>,
        limit: Option<usize>,
    ) -> rusqlite::Result<Vec<ConversationEntry>> {
        let mut sql = String::from(
            "SELECT id, session_id, role, content, project, entry_type, raw_id, model, input_tokens, output_tokens, cache_creation_tokens, cache_read_tokens, message_timestamp, created_at FROM conversations"
        );
        let mut p: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut idx = 1;
        let mut has_where = false;

        if let Some(proj) = project {
            sql.push_str(&format!(" WHERE project = ?{}", idx));
            p.push(Box::new(proj.to_string()));
            idx += 1;
            has_where = true;
        }

        if let Some(days) = older_than_days {
            let clause = format!(
                " {} created_at < datetime('now', ?{})",
                if has_where { "AND" } else { "WHERE" },
                idx
            );
            sql.push_str(&clause);
            p.push(Box::new(format!("-{} days", days)));
            idx += 1;
        }

        sql.push_str(" ORDER BY id ASC");
        if let Some(n) = limit {
            sql.push_str(&format!(" LIMIT ?{}", idx));
            p.push(Box::new(n as i64));
        }

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(p.iter()), Self::row_to_conversation)?;
        rows.collect()
    }

    pub fn query_tasks_for_archive(
        &self,
        project: Option<&str>,
        older_than_days: Option<i64>,
        limit: Option<usize>,
    ) -> rusqlite::Result<Vec<Task>> {
        let mut sql = String::from(
            "SELECT id, project, subject, description, status, priority, task_type, parent_id, due_date, created_by, assignee, owner, session_id, created_at, updated_at FROM tasks"
        );
        let mut p: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut idx = 1;
        let mut has_where = false;

        if let Some(proj) = project {
            sql.push_str(&format!(" WHERE project = ?{}", idx));
            p.push(Box::new(proj.to_string()));
            idx += 1;
            has_where = true;
        }

        if let Some(days) = older_than_days {
            let clause = format!(
                " {} updated_at < datetime('now', ?{})",
                if has_where { "AND" } else { "WHERE" },
                idx
            );
            sql.push_str(&clause);
            p.push(Box::new(format!("-{} days", days)));
            idx += 1;
        }

        sql.push_str(" ORDER BY id ASC");
        if let Some(n) = limit {
            sql.push_str(&format!(" LIMIT ?{}", idx));
            p.push(Box::new(n as i64));
        }

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(p.iter()), Self::row_to_task)?;
        rows.collect()
    }

    pub fn get_subtask_ids_recursive(&self, task_ids: &[i64]) -> rusqlite::Result<Vec<i64>> {
        if task_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut all_ids: Vec<i64> = Vec::new();
        let mut current_parents = task_ids.to_vec();

        loop {
            if current_parents.is_empty() {
                break;
            }
            let placeholders: String = current_parents.iter().enumerate()
                .map(|(i, _)| format!("?{}", i + 1))
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!(
                "SELECT id FROM tasks WHERE parent_id IN ({})",
                placeholders
            );
            let mut stmt = self.conn.prepare(&sql)?;
            let params: Vec<Box<dyn rusqlite::types::ToSql>> = current_parents.iter()
                .map(|id| Box::new(*id) as Box<dyn rusqlite::types::ToSql>)
                .collect();
            let children: Vec<i64> = stmt.query_map(
                rusqlite::params_from_iter(params.iter()),
                |row| row.get(0),
            )?.collect::<rusqlite::Result<_>>()?;

            if children.is_empty() {
                break;
            }
            all_ids.extend(&children);
            current_parents = children;
        }

        Ok(all_ids)
    }

    pub fn get_task_deps_for_task_ids(&self, task_ids: &[i64]) -> rusqlite::Result<Vec<(i64, i64)>> {
        if task_ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders: String = task_ids.iter().enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT blocker_id, blocked_id FROM task_deps WHERE blocker_id IN ({ph}) OR blocked_id IN ({ph})",
            ph = placeholders
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let params: Vec<Box<dyn rusqlite::types::ToSql>> = task_ids.iter()
            .map(|id| Box::new(*id) as Box<dyn rusqlite::types::ToSql>)
            .collect();
        let rows = stmt.query_map(
            rusqlite::params_from_iter(params.iter()),
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        rows.collect()
    }

    pub fn get_links_for_entity_ids(
        &self,
        entity_type: &str,
        entity_ids: &[i64],
    ) -> rusqlite::Result<Vec<Link>> {
        if entity_ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders: String = entity_ids.iter().enumerate()
            .map(|(i, _)| format!("?{}", i + 2))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT id, source_type, source_id, target_type, target_id, relation, created_at
             FROM links
             WHERE (source_type = ?1 AND source_id IN ({ph}))
                OR (target_type = ?1 AND target_id IN ({ph}))",
            ph = placeholders
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(entity_type.to_string())];
        for id in entity_ids {
            params.push(Box::new(*id));
        }
        let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), Self::row_to_link)?;
        rows.collect()
    }

    // ── Archive deletes ──────────────────────────────────────────────────

    pub fn delete_links_by_ids(&self, ids: &[i64]) -> rusqlite::Result<usize> {
        self.delete_by_ids("links", ids)
    }

    pub fn delete_task_deps_for_task_ids(&self, task_ids: &[i64]) -> rusqlite::Result<usize> {
        if task_ids.is_empty() {
            return Ok(0);
        }
        let placeholders: String = task_ids.iter().enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "DELETE FROM task_deps WHERE blocker_id IN ({ph}) OR blocked_id IN ({ph})",
            ph = placeholders
        );
        let params: Vec<Box<dyn rusqlite::types::ToSql>> = task_ids.iter()
            .map(|id| Box::new(*id) as Box<dyn rusqlite::types::ToSql>)
            .collect();
        self.conn.execute(&sql, rusqlite::params_from_iter(params.iter()))
    }

    pub fn delete_memories_by_ids(&self, ids: &[i64]) -> rusqlite::Result<usize> {
        self.delete_by_ids("memories", ids)
    }

    pub fn delete_conversations_by_ids(&self, ids: &[i64]) -> rusqlite::Result<usize> {
        self.delete_by_ids("conversations", ids)
    }

    pub fn delete_tasks_by_ids(&self, ids: &[i64]) -> rusqlite::Result<usize> {
        self.delete_by_ids("tasks", ids)
    }

    fn delete_by_ids(&self, table: &str, ids: &[i64]) -> rusqlite::Result<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let placeholders: String = ids.iter().enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!("DELETE FROM {} WHERE id IN ({})", table, placeholders);
        let params: Vec<Box<dyn rusqlite::types::ToSql>> = ids.iter()
            .map(|id| Box::new(*id) as Box<dyn rusqlite::types::ToSql>)
            .collect();
        self.conn.execute(&sql, rusqlite::params_from_iter(params.iter()))
    }

    // ── Archive restore ──────────────────────────────────────────────────

    pub fn restore_memory(&self, mem: &Memory) -> rusqlite::Result<bool> {
        let tags_json = mem.tags.as_ref().map(|t| serde_json::to_string(t).unwrap_or_default());
        let affected = self.conn.execute(
            "INSERT OR IGNORE INTO memories (id, category, key, value, tags, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![mem.id, mem.category, mem.key, mem.value, tags_json, mem.created_at, mem.updated_at],
        )?;
        Ok(affected > 0)
    }

    pub fn restore_conversation(&self, entry: &ConversationEntry) -> rusqlite::Result<bool> {
        let affected = self.conn.execute(
            "INSERT OR IGNORE INTO conversations (id, session_id, role, content, project, entry_type, raw_id, \
             model, input_tokens, output_tokens, cache_creation_tokens, cache_read_tokens, \
             message_timestamp, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![entry.id, entry.session_id, entry.role, entry.content, entry.project,
                    entry.entry_type, entry.raw_id, entry.model, entry.input_tokens,
                    entry.output_tokens, entry.cache_creation_tokens, entry.cache_read_tokens,
                    entry.message_timestamp, entry.created_at],
        )?;
        Ok(affected > 0)
    }

    pub fn restore_task(&self, task: &Task) -> rusqlite::Result<bool> {
        let affected = self.conn.execute(
            "INSERT OR IGNORE INTO tasks (id, project, subject, description, status, priority, task_type, parent_id, due_date, created_by, assignee, owner, session_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![task.id, task.project, task.subject, task.description, task.status, task.priority, task.task_type, task.parent_id, task.due_date, task.created_by, task.assignee, task.owner, task.session_id, task.created_at, task.updated_at],
        )?;
        Ok(affected > 0)
    }

    pub fn restore_task_dep(&self, blocker_id: i64, blocked_id: i64) -> rusqlite::Result<bool> {
        let affected = self.conn.execute(
            "INSERT OR IGNORE INTO task_deps (blocker_id, blocked_id) VALUES (?1, ?2)",
            params![blocker_id, blocked_id],
        )?;
        Ok(affected > 0)
    }

    pub fn restore_link(&self, link: &Link) -> rusqlite::Result<bool> {
        let affected = self.conn.execute(
            "INSERT OR IGNORE INTO links (id, source_type, source_id, target_type, target_id, relation, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![link.id, link.source_type, link.source_id, link.target_type, link.target_id, link.relation, link.created_at],
        )?;
        Ok(affected > 0)
    }

    // ── PreCompact batch insert ─────────────────────────────────────────

    pub fn store_pre_compact_batch(
        &self,
        messages: &[PreCompactMessage],
    ) -> rusqlite::Result<usize> {
        let tx = self.conn.unchecked_transaction()?;
        let mut count = 0usize;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO conversations (session_id, role, content, project, entry_type, \
                 model, input_tokens, output_tokens, cache_creation_tokens, cache_read_tokens, \
                 message_timestamp) \
                 VALUES (?1, ?2, ?3, ?4, 'pre_compact', ?5, ?6, ?7, ?8, ?9, ?10)"
            )?;
            for msg in messages {
                stmt.execute(params![
                    msg.session_id, msg.role, msg.content, msg.project,
                    msg.model, msg.input_tokens, msg.output_tokens,
                    msg.cache_creation_tokens, msg.cache_read_tokens,
                    msg.message_timestamp,
                ])?;
                count += 1;
            }
        }
        tx.commit()?;
        Ok(count)
    }

    // ── Export (full table reads) ────────────────────────────────────────

    pub fn export_all_memories(&self) -> rusqlite::Result<Vec<Memory>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, category, key, value, tags, created_at, updated_at
             FROM memories ORDER BY id ASC"
        )?;
        let rows = stmt.query_map([], Self::row_to_memory)?;
        rows.collect()
    }

    pub fn export_all_conversations(&self) -> rusqlite::Result<Vec<ConversationEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, role, content, project, entry_type, raw_id, model, input_tokens, output_tokens, cache_creation_tokens, cache_read_tokens, message_timestamp, created_at
             FROM conversations ORDER BY id ASC"
        )?;
        let rows = stmt.query_map([], Self::row_to_conversation)?;
        rows.collect()
    }

    pub fn export_all_tasks(&self) -> rusqlite::Result<Vec<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project, subject, description, status, priority, task_type, parent_id, due_date, created_by, assignee, owner, session_id, created_at, updated_at
             FROM tasks ORDER BY CASE WHEN parent_id IS NULL THEN 0 ELSE 1 END, id ASC"
        )?;
        let rows = stmt.query_map([], Self::row_to_task)?;
        rows.collect()
    }

    pub fn export_all_task_deps(&self) -> rusqlite::Result<Vec<(i64, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT blocker_id, blocked_id FROM task_deps ORDER BY blocker_id, blocked_id"
        )?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect()
    }

    pub fn export_all_links(&self) -> rusqlite::Result<Vec<Link>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source_type, source_id, target_type, target_id, relation, created_at
             FROM links ORDER BY id ASC"
        )?;
        let rows = stmt.query_map([], Self::row_to_link)?;
        rows.collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Init / Migration ─────────────────────────────────────────────

    #[test]
    fn test_init_db_creates_tables() {
        let db = Database::open_in_memory().unwrap();
        // Verify core tables exist by querying them
        let count: i64 = db.conn.query_row("SELECT COUNT(*) FROM memories", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 0);
        let count: i64 = db.conn.query_row("SELECT COUNT(*) FROM conversations", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 0);
        let count: i64 = db.conn.query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 0);
        let count: i64 = db.conn.query_row("SELECT COUNT(*) FROM links", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 0);
        let count: i64 = db.conn.query_row("SELECT COUNT(*) FROM task_deps", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_init_db_is_idempotent() {
        let db = Database::open_in_memory().unwrap();
        // Calling migrate again should not error
        db.migrate().unwrap();
        db.migrate().unwrap();
    }

    // ── Memory CRUD ──────────────────────────────────────────────────

    #[test]
    fn test_store_and_get_memory() {
        let db = Database::open_in_memory().unwrap();
        let mem = db.store_memory("facts", "test-key", "test-value", None).unwrap();
        assert_eq!(mem.category, "facts");
        assert_eq!(mem.key, "test-key");
        assert_eq!(mem.value, "test-value");
        assert!(mem.tags.is_none());
    }

    #[test]
    fn test_store_memory_with_tags() {
        let db = Database::open_in_memory().unwrap();
        let tags = vec!["rust".to_string(), "testing".to_string()];
        let mem = db.store_memory("patterns", "tag-test", "value", Some(&tags)).unwrap();
        let stored_tags = mem.tags.unwrap();
        assert_eq!(stored_tags, vec!["rust", "testing"]);
    }

    #[test]
    fn test_store_memory_upsert() {
        let db = Database::open_in_memory().unwrap();
        db.store_memory("facts", "key1", "original", None).unwrap();
        let updated = db.store_memory("facts", "key1", "updated", None).unwrap();
        assert_eq!(updated.value, "updated");

        // Should still be only one memory
        let all = db.list_memories(Some("facts"), 50).unwrap();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn test_search_memories() {
        let db = Database::open_in_memory().unwrap();
        db.store_memory("facts", "rust-lang", "Rust is a systems language", None).unwrap();
        db.store_memory("facts", "python-lang", "Python is interpreted", None).unwrap();

        let results = db.search_memories("systems language", None, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key, "rust-lang");
    }

    #[test]
    fn test_list_memories_with_category_filter() {
        let db = Database::open_in_memory().unwrap();
        db.store_memory("facts", "f1", "fact one", None).unwrap();
        db.store_memory("insights", "i1", "insight one", None).unwrap();

        let facts = db.list_memories(Some("facts"), 50).unwrap();
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].category, "facts");

        let all = db.list_memories(None, 50).unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_delete_memory() {
        let db = Database::open_in_memory().unwrap();
        db.store_memory("facts", "to-delete", "gone soon", None).unwrap();

        assert!(db.delete_memory("facts", "to-delete").unwrap());
        assert!(!db.delete_memory("facts", "to-delete").unwrap()); // already gone

        let all = db.list_memories(None, 50).unwrap();
        assert_eq!(all.len(), 0);
    }

    // ── Conversation CRUD ────────────────────────────────────────────

    #[test]
    fn test_log_conversation() {
        let db = Database::open_in_memory().unwrap();
        let entry = db.log_conversation("sess-1", "user", "hello", Some("proj"), Some("raw_user"), None).unwrap();
        assert_eq!(entry.session_id, "sess-1");
        assert_eq!(entry.role, "user");
        assert_eq!(entry.content, "hello");
        assert_eq!(entry.project.as_deref(), Some("proj"));
        assert_eq!(entry.entry_type.as_deref(), Some("raw_user"));
    }

    #[test]
    fn test_search_conversations() {
        let db = Database::open_in_memory().unwrap();
        db.log_conversation("s1", "user", "rust programming help", Some("proj"), Some("raw_user"), None).unwrap();
        db.log_conversation("s1", "assistant", "python scripting help", Some("proj"), Some("raw_assistant"), None).unwrap();

        let results = db.search_conversations("rust programming", None, None, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].content.contains("rust"));
    }

    #[test]
    fn test_list_conversations_with_filters() {
        let db = Database::open_in_memory().unwrap();
        db.log_conversation("s1", "user", "msg1", Some("proj"), Some("raw_user"), None).unwrap();
        db.log_conversation("s1", "assistant", "msg2", Some("proj"), Some("summary"), None).unwrap();
        db.log_conversation("s2", "user", "msg3", Some("proj"), Some("raw_user"), None).unwrap();

        // Filter by session
        let s1 = db.list_conversations(Some("s1"), None, 50).unwrap();
        assert_eq!(s1.len(), 2);

        // Filter by entry_type
        let summaries = db.list_conversations(None, Some("summary"), 50).unwrap();
        assert_eq!(summaries.len(), 1);
    }

    #[test]
    fn test_get_conversation_context() {
        let db = Database::open_in_memory().unwrap();
        db.log_conversation("s1", "assistant", "summary 1", Some("proj"), Some("summary"), None).unwrap();
        db.log_conversation("s1", "user", "raw msg", Some("proj"), Some("raw_user"), None).unwrap();
        db.log_conversation("s1", "assistant", "summary 2", Some("proj"), Some("summary"), None).unwrap();

        let ctx = db.get_conversation_context("s1", 50).unwrap();
        // Should only return summaries
        assert_eq!(ctx.len(), 2);
        assert!(ctx.iter().all(|e| e.entry_type.as_deref() == Some("summary")));
    }

    #[test]
    fn test_prune_conversations() {
        let db = Database::open_in_memory().unwrap();
        // Insert with a manually backdated timestamp
        db.conn.execute(
            "INSERT INTO conversations (session_id, role, content, project, entry_type, created_at) VALUES (?1, ?2, ?3, ?4, ?5, datetime('now', '-60 days'))",
            params!["s1", "user", "old msg", "proj", "raw_user"],
        ).unwrap();
        db.log_conversation("s1", "user", "recent msg", Some("proj"), Some("raw_user"), None).unwrap();

        let pruned = db.prune_conversations(30, None).unwrap();
        assert_eq!(pruned, 1);

        let remaining = db.list_conversations(None, None, 50).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].content, "recent msg");
    }

    // ── Task CRUD ────────────────────────────────────────────────────

    #[test]
    fn test_create_task() {
        let db = Database::open_in_memory().unwrap();
        let task = db.create_task("proj", "Do something", Some("details"), Some("high"), Some("claude"), None, None, None, None, None, None).unwrap();
        assert_eq!(task.project, "proj");
        assert_eq!(task.subject, "Do something");
        assert_eq!(task.description.as_deref(), Some("details"));
        assert_eq!(task.status, "pending");
        assert_eq!(task.priority.as_deref(), Some("high"));
        assert_eq!(task.task_type.as_deref(), Some("claude"));
    }

    #[test]
    fn test_update_task() {
        let db = Database::open_in_memory().unwrap();
        let task = db.create_task("proj", "Task 1", None, None, None, None, None, None, None, None, None).unwrap();

        let updates = serde_json::json!({"status": "in_progress", "priority": "high"});
        let updated = db.update_task(task.id, &updates).unwrap();
        assert_eq!(updated.status, "in_progress");
        assert_eq!(updated.priority.as_deref(), Some("high"));
    }

    #[test]
    fn test_get_task() {
        let db = Database::open_in_memory().unwrap();
        let task = db.create_task("proj", "Find me", None, None, None, None, None, None, None, None, None).unwrap();
        let found = db.get_task(task.id).unwrap();
        assert_eq!(found.subject, "Find me");
    }

    #[test]
    fn test_list_tasks_with_filters() {
        let db = Database::open_in_memory().unwrap();
        db.create_task("proj-a", "Task A", None, Some("high"), Some("claude"), None, None, None, None, None, None).unwrap();
        db.create_task("proj-b", "Task B", None, Some("low"), Some("human"), None, None, None, None, None, None).unwrap();

        let proj_a = db.list_tasks(Some("proj-a"), None, None, None, None, 50).unwrap();
        assert_eq!(proj_a.len(), 1);

        let high = db.list_tasks(None, None, None, None, Some("high"), 50).unwrap();
        assert_eq!(high.len(), 1);
        assert_eq!(high[0].subject, "Task A");
    }

    #[test]
    fn test_search_tasks() {
        let db = Database::open_in_memory().unwrap();
        db.create_task("proj", "Fix authentication bug", Some("Login fails"), None, None, None, None, None, None, None, None).unwrap();
        db.create_task("proj", "Add dark mode", Some("UI feature"), None, None, None, None, None, None, None, None).unwrap();

        let results = db.search_tasks("authentication", None, None, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].subject, "Fix authentication bug");
    }

    #[test]
    fn test_soft_delete_task() {
        let db = Database::open_in_memory().unwrap();
        let task = db.create_task("proj", "Delete me", None, None, None, None, None, None, None, None, None).unwrap();

        let updates = serde_json::json!({"status": "deleted"});
        let deleted = db.update_task(task.id, &updates).unwrap();
        assert_eq!(deleted.status, "deleted");

        // Default list excludes deleted
        let visible = db.list_tasks(None, None, None, None, None, 50).unwrap();
        assert!(visible.iter().all(|t| t.status != "deleted"));
    }

    // ── Task Dependencies ────────────────────────────────────────────

    #[test]
    fn test_task_deps() {
        let db = Database::open_in_memory().unwrap();
        let t1 = db.create_task("proj", "Blocker", None, None, None, None, None, None, None, None, None).unwrap();
        let t2 = db.create_task("proj", "Blocked", None, None, None, None, None, None, None, None, None).unwrap();

        db.add_task_dep(t1.id, t2.id).unwrap();

        let (blockers, _) = db.get_task_deps(t2.id).unwrap();
        assert_eq!(blockers.len(), 1);
        assert_eq!(blockers[0].id, t1.id);

        let (_, blocked_by_t1) = db.get_task_deps(t1.id).unwrap();
        assert_eq!(blocked_by_t1.len(), 1);
        assert_eq!(blocked_by_t1[0].id, t2.id);

        assert!(db.remove_task_dep(t1.id, t2.id).unwrap());
        assert!(!db.remove_task_dep(t1.id, t2.id).unwrap()); // already removed
    }

    // ── Links ────────────────────────────────────────────────────────

    #[test]
    fn test_create_and_get_link() {
        let db = Database::open_in_memory().unwrap();
        let link = db.create_link("task", 1, "memory", 2, Some("relates_to")).unwrap();
        assert_eq!(link.source_type, "task");
        assert_eq!(link.source_id, 1);
        assert_eq!(link.target_type, "memory");
        assert_eq!(link.target_id, 2);
        assert_eq!(link.relation.as_deref(), Some("relates_to"));

        // Get from source side
        let links = db.get_links("task", 1).unwrap();
        assert_eq!(links.len(), 1);

        // Get from target side
        let links = db.get_links("memory", 2).unwrap();
        assert_eq!(links.len(), 1);
    }

    #[test]
    fn test_delete_link() {
        let db = Database::open_in_memory().unwrap();
        let link = db.create_link("task", 1, "memory", 2, None).unwrap();
        assert!(db.delete_link(link.id).unwrap());
        assert!(!db.delete_link(link.id).unwrap()); // already gone

        let links = db.get_links("task", 1).unwrap();
        assert_eq!(links.len(), 0);
    }

    // ── PreCompact Batch Insert ──────────────────────────────────────

    #[test]
    fn test_store_pre_compact_batch() {
        let db = Database::open_in_memory().unwrap();
        let messages = vec![
            PreCompactMessage {
                session_id: "s1".to_string(),
                role: "user".to_string(),
                content: "hello".to_string(),
                project: "proj".to_string(),
                model: None,
                input_tokens: None,
                output_tokens: None,
                cache_creation_tokens: None,
                cache_read_tokens: None,
                message_timestamp: Some("2026-02-25T10:00:00Z".to_string()),
            },
            PreCompactMessage {
                session_id: "s1".to_string(),
                role: "assistant".to_string(),
                content: "hi there".to_string(),
                project: "proj".to_string(),
                model: Some("claude-opus-4-6".to_string()),
                input_tokens: Some(100),
                output_tokens: Some(50),
                cache_creation_tokens: Some(10),
                cache_read_tokens: Some(20),
                message_timestamp: Some("2026-02-25T10:00:01Z".to_string()),
            },
        ];

        let count = db.store_pre_compact_batch(&messages).unwrap();
        assert_eq!(count, 2);

        let entries = db.list_conversations(None, Some("pre_compact"), 50).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].model.as_deref(), None);
        assert_eq!(entries[1].model.as_deref(), Some("claude-opus-4-6"));
        assert_eq!(entries[1].input_tokens, Some(100));
        assert_eq!(entries[1].output_tokens, Some(50));
    }

    #[test]
    fn test_store_pre_compact_batch_empty() {
        let db = Database::open_in_memory().unwrap();
        let count = db.store_pre_compact_batch(&[]).unwrap();
        assert_eq!(count, 0);
    }

    // ── Validation Tests ─────────────────────────────────────────────

    #[test]
    fn test_enum_roundtrip() {
        // Verify FromStr -> Display roundtrips for all enums
        assert_eq!("pending".parse::<TaskStatus>().unwrap().to_string(), "pending");
        assert_eq!("in_progress".parse::<TaskStatus>().unwrap().to_string(), "in_progress");
        assert_eq!("completed".parse::<TaskStatus>().unwrap().to_string(), "completed");
        assert_eq!("blocked".parse::<TaskStatus>().unwrap().to_string(), "blocked");
        assert_eq!("deleted".parse::<TaskStatus>().unwrap().to_string(), "deleted");

        assert_eq!("low".parse::<TaskPriority>().unwrap().to_string(), "low");
        assert_eq!("medium".parse::<TaskPriority>().unwrap().to_string(), "medium");
        assert_eq!("high".parse::<TaskPriority>().unwrap().to_string(), "high");

        assert_eq!("claude".parse::<TaskType>().unwrap().to_string(), "claude");
        assert_eq!("human".parse::<TaskType>().unwrap().to_string(), "human");
        assert_eq!("hybrid".parse::<TaskType>().unwrap().to_string(), "hybrid");

        assert_eq!("summary".parse::<EntryType>().unwrap().to_string(), "summary");
        assert_eq!("raw_user".parse::<EntryType>().unwrap().to_string(), "raw_user");
        assert_eq!("raw_assistant".parse::<EntryType>().unwrap().to_string(), "raw_assistant");
        assert_eq!("pre_compact".parse::<EntryType>().unwrap().to_string(), "pre_compact");
    }

    #[test]
    fn test_enum_rejects_invalid() {
        assert!("oops".parse::<TaskStatus>().is_err());
        assert!("PENDING".parse::<TaskStatus>().is_err()); // case sensitive
        assert!("".parse::<TaskStatus>().is_err());

        assert!("critical".parse::<TaskPriority>().is_err());
        assert!("bot".parse::<TaskType>().is_err());
        assert!("transcript".parse::<EntryType>().is_err());
    }

    #[test]
    fn test_enum_error_messages() {
        let err = "oops".parse::<TaskStatus>().unwrap_err();
        assert!(err.contains("Invalid status 'oops'"));
        assert!(err.contains("pending, in_progress, completed, blocked, deleted"));

        let err = "xxx".parse::<TaskPriority>().unwrap_err();
        assert!(err.contains("Invalid priority 'xxx'"));
        assert!(err.contains("low, medium, high"));

        let err = "bot".parse::<TaskType>().unwrap_err();
        assert!(err.contains("Invalid task_type 'bot'"));
        assert!(err.contains("claude, human, hybrid"));

        let err = "raw".parse::<EntryType>().unwrap_err();
        assert!(err.contains("Invalid entry_type 'raw'"));
        assert!(err.contains("summary, raw_user, raw_assistant, pre_compact"));
    }

    #[test]
    fn test_create_task_rejects_invalid_priority() {
        let db = Database::open_in_memory().unwrap();
        let result = db.create_task("proj", "Task", None, Some("critical"), None, None, None, None, None, None, None);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("Invalid priority"));
    }

    #[test]
    fn test_create_task_rejects_invalid_task_type() {
        let db = Database::open_in_memory().unwrap();
        let result = db.create_task("proj", "Task", None, None, Some("bot"), None, None, None, None, None, None);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("Invalid task_type"));
    }

    #[test]
    fn test_update_task_rejects_invalid_status() {
        let db = Database::open_in_memory().unwrap();
        let task = db.create_task("proj", "Task", None, None, None, None, None, None, None, None, None).unwrap();

        let updates = serde_json::json!({"status": "oops"});
        let result = db.update_task(task.id, &updates);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("Invalid status"));
    }

    #[test]
    fn test_log_conversation_rejects_invalid_entry_type() {
        let db = Database::open_in_memory().unwrap();
        let result = db.log_conversation("s1", "user", "hello", Some("proj"), Some("invalid_type"), None);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("Invalid entry_type"));
    }

    #[test]
    fn test_valid_values_still_accepted() {
        let db = Database::open_in_memory().unwrap();
        // Valid create_task
        db.create_task("proj", "Task", None, Some("high"), Some("claude"), None, None, None, None, None, None).unwrap();
        // Valid update_task
        let task = db.create_task("proj", "Task2", None, None, None, None, None, None, None, None, None).unwrap();
        db.update_task(task.id, &serde_json::json!({"status": "in_progress", "priority": "low", "task_type": "human"})).unwrap();
        // Valid log_conversation
        db.log_conversation("s1", "user", "hi", Some("proj"), Some("summary"), None).unwrap();
        db.log_conversation("s1", "user", "hi", Some("proj"), Some("raw_user"), None).unwrap();
        db.log_conversation("s1", "user", "hi", Some("proj"), Some("pre_compact"), None).unwrap();
    }

    // ── Archive Query Tests ─────────────────────────────────────────

    #[test]
    fn test_archive_query_no_limit() {
        let db = Database::open_in_memory().unwrap();
        for i in 0..5 {
            db.store_memory("cat", &format!("k{}", i), "val", None).unwrap();
        }
        let results = db.query_memories_for_archive(None, None, None).unwrap();
        assert_eq!(results.len(), 5);
    }

    #[test]
    fn test_archive_query_with_limit() {
        let db = Database::open_in_memory().unwrap();
        for i in 0..5 {
            db.store_memory("cat", &format!("k{}", i), "val", None).unwrap();
        }
        let results = db.query_memories_for_archive(None, None, Some(3)).unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_archive_conversations_with_limit() {
        let db = Database::open_in_memory().unwrap();
        for i in 0..5 {
            db.log_conversation(&format!("s{}", i), "user", "msg", None, Some("summary"), None).unwrap();
        }
        let all = db.query_conversations_for_archive(None, None, None).unwrap();
        assert_eq!(all.len(), 5);
        let limited = db.query_conversations_for_archive(None, None, Some(2)).unwrap();
        assert_eq!(limited.len(), 2);
    }

    #[test]
    fn test_archive_tasks_with_limit() {
        let db = Database::open_in_memory().unwrap();
        for i in 0..5 {
            db.create_task("proj", &format!("task{}", i), None, None, None, None, None, None, None, None, None).unwrap();
        }
        let all = db.query_tasks_for_archive(None, None, None).unwrap();
        assert_eq!(all.len(), 5);
        let limited = db.query_tasks_for_archive(None, None, Some(2)).unwrap();
        assert_eq!(limited.len(), 2);
    }
}
