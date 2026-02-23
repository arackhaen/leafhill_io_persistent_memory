use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
        self.conn.execute(
            "INSERT INTO conversations (session_id, role, content, project, entry_type, raw_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![session_id, role, content, project, entry_type, raw_id],
        )?;

        let id = self.conn.last_insert_rowid();
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, role, content, project, entry_type, raw_id, created_at
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
            "SELECT c.id, c.session_id, c.role, c.content, c.project, c.entry_type, c.raw_id, c.created_at
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
            "SELECT id, session_id, role, content, project, entry_type, raw_id, created_at
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
            created_at: row.get(7)?,
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
            "SELECT id, session_id, role, content, project, entry_type, raw_id, created_at
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
}
