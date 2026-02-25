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

        sql.push_str(&format!(" ORDER BY id ASC LIMIT ?{}", idx));
        p.push(Box::new(100_000i64));

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(p.iter()), Self::row_to_memory)?;
        rows.collect()
    }

    pub fn query_conversations_for_archive(
        &self,
        project: Option<&str>,
        older_than_days: Option<i64>,
    ) -> rusqlite::Result<Vec<ConversationEntry>> {
        let mut sql = String::from(
            "SELECT id, session_id, role, content, project, entry_type, raw_id, created_at FROM conversations"
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

        sql.push_str(&format!(" ORDER BY id ASC LIMIT ?{}", idx));
        p.push(Box::new(100_000i64));

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(p.iter()), Self::row_to_conversation)?;
        rows.collect()
    }

    pub fn query_tasks_for_archive(
        &self,
        project: Option<&str>,
        older_than_days: Option<i64>,
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

        sql.push_str(&format!(" ORDER BY id ASC LIMIT ?{}", idx));
        p.push(Box::new(100_000i64));

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
            "INSERT OR IGNORE INTO conversations (id, session_id, role, content, project, entry_type, raw_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![entry.id, entry.session_id, entry.role, entry.content, entry.project, entry.entry_type, entry.raw_id, entry.created_at],
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
            "SELECT id, session_id, role, content, project, entry_type, raw_id, created_at
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
