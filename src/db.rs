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
            "
        )?;
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
    ) -> rusqlite::Result<ConversationEntry> {
        self.conn.execute(
            "INSERT INTO conversations (session_id, role, content, project)
             VALUES (?1, ?2, ?3, ?4)",
            params![session_id, role, content, project],
        )?;

        let id = self.conn.last_insert_rowid();
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, role, content, project, created_at
             FROM conversations WHERE id = ?1"
        )?;

        stmt.query_row(params![id], |row| {
            Ok(ConversationEntry {
                id: row.get(0)?,
                session_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                project: row.get(4)?,
                created_at: row.get(5)?,
            })
        })
    }

    pub fn search_conversations(
        &self,
        query: &str,
        session_id: Option<&str>,
        limit: usize,
    ) -> rusqlite::Result<Vec<ConversationEntry>> {
        let sql = if session_id.is_some() {
            "SELECT c.id, c.session_id, c.role, c.content, c.project, c.created_at
             FROM conversations_fts f
             JOIN conversations c ON c.id = f.rowid
             WHERE conversations_fts MATCH ?1 AND c.session_id = ?2
             ORDER BY rank
             LIMIT ?3"
        } else {
            "SELECT c.id, c.session_id, c.role, c.content, c.project, c.created_at
             FROM conversations_fts f
             JOIN conversations c ON c.id = f.rowid
             WHERE conversations_fts MATCH ?1
             ORDER BY rank
             LIMIT ?3"
        };

        let mut stmt = self.conn.prepare(sql)?;
        let rows = if let Some(sid) = session_id {
            stmt.query_map(params![query, sid, limit as i64], Self::row_to_conversation)?
        } else {
            stmt.query_map(params![query, "", limit as i64], Self::row_to_conversation)?
        };

        rows.collect()
    }

    pub fn list_conversations(
        &self,
        session_id: Option<&str>,
        limit: usize,
    ) -> rusqlite::Result<Vec<ConversationEntry>> {
        let (sql, p): (&str, Vec<Box<dyn rusqlite::types::ToSql>>) = if let Some(sid) = session_id {
            (
                "SELECT id, session_id, role, content, project, created_at
                 FROM conversations WHERE session_id = ?1
                 ORDER BY created_at DESC LIMIT ?2",
                vec![Box::new(sid.to_string()), Box::new(limit as i64)],
            )
        } else {
            (
                "SELECT id, session_id, role, content, project, created_at
                 FROM conversations ORDER BY created_at DESC LIMIT ?1",
                vec![Box::new(limit as i64)],
            )
        };

        let mut stmt = self.conn.prepare(sql)?;
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
            created_at: row.get(5)?,
        })
    }
}
