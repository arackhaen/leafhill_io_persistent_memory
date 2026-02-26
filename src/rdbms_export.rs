use crate::db::Database;
use tokio_postgres::{Client, NoTls};

const ALL_TABLES: &[&str] = &["memories", "conversations", "tasks", "task_deps", "links"];

fn create_tables_ddl() -> Vec<(&'static str, &'static str)> {
    vec![
        ("memories",
            "CREATE TABLE IF NOT EXISTS memories (\
                id BIGINT PRIMARY KEY, \
                category VARCHAR(255) NOT NULL, \
                \"key\" VARCHAR(255) NOT NULL, \
                value TEXT NOT NULL, \
                tags TEXT, \
                created_at TEXT NOT NULL, \
                updated_at TEXT NOT NULL, \
                UNIQUE(category, \"key\"))"),
        ("conversations",
            "CREATE TABLE IF NOT EXISTS conversations (\
                id BIGINT PRIMARY KEY, \
                session_id TEXT NOT NULL, \
                role TEXT NOT NULL, \
                content TEXT NOT NULL, \
                project TEXT, \
                entry_type TEXT, \
                raw_id BIGINT, \
                model TEXT, \
                input_tokens BIGINT, \
                output_tokens BIGINT, \
                cache_creation_tokens BIGINT, \
                cache_read_tokens BIGINT, \
                message_timestamp TEXT, \
                created_at TEXT NOT NULL)"),
        ("tasks",
            "CREATE TABLE IF NOT EXISTS tasks (\
                id BIGINT PRIMARY KEY, \
                project TEXT NOT NULL, \
                subject TEXT NOT NULL, \
                description TEXT, \
                status TEXT NOT NULL, \
                priority TEXT, \
                task_type TEXT, \
                parent_id BIGINT, \
                due_date TEXT, \
                created_by TEXT, \
                assignee TEXT, \
                owner TEXT, \
                session_id TEXT, \
                created_at TEXT NOT NULL, \
                updated_at TEXT NOT NULL)"),
        ("task_deps",
            "CREATE TABLE IF NOT EXISTS task_deps (\
                blocker_id BIGINT NOT NULL, \
                blocked_id BIGINT NOT NULL, \
                PRIMARY KEY (blocker_id, blocked_id))"),
        ("links",
            "CREATE TABLE IF NOT EXISTS links (\
                id BIGINT PRIMARY KEY, \
                source_type VARCHAR(255) NOT NULL, \
                source_id BIGINT NOT NULL, \
                target_type VARCHAR(255) NOT NULL, \
                target_id BIGINT NOT NULL, \
                relation TEXT, \
                created_at TEXT NOT NULL, \
                UNIQUE(source_type, source_id, target_type, target_id))"),
    ]
}

async fn export_table(
    db: &Database,
    client: &Client,
    table: &str,
) -> Result<(usize, usize), String> {
    let mut inserted: usize = 0;
    let mut skipped: usize = 0;

    match table {
        "memories" => {
            let stmt = client.prepare(
                "INSERT INTO memories (id, category, \"key\", value, tags, created_at, updated_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT DO NOTHING"
            ).await.map_err(|e| format!("Failed to prepare memories insert: {}", e))?;

            let memories = db.export_all_memories()
                .map_err(|e| format!("Failed to read memories: {}", e))?;
            let total = memories.len();
            for mem in &memories {
                let tags_json = mem.tags.as_ref().map(|t| serde_json::to_string(t).unwrap_or_default());
                let rows = client.execute(&stmt, &[
                    &mem.id, &mem.category, &mem.key, &mem.value,
                    &tags_json, &mem.created_at, &mem.updated_at,
                ]).await.map_err(|e| format!("Failed to insert memory {}: {}", mem.id, e))?;
                if rows > 0 { inserted += 1; } else { skipped += 1; }
            }
            if total > 0 { eprintln!("  memories: {}/{} rows exported", inserted, total); }
        }
        "conversations" => {
            let stmt = client.prepare(
                "INSERT INTO conversations (id, session_id, role, content, project, entry_type, raw_id, \
                 model, input_tokens, output_tokens, cache_creation_tokens, cache_read_tokens, \
                 message_timestamp, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14) ON CONFLICT DO NOTHING"
            ).await.map_err(|e| format!("Failed to prepare conversations insert: {}", e))?;

            let entries = db.export_all_conversations()
                .map_err(|e| format!("Failed to read conversations: {}", e))?;
            let total = entries.len();
            for entry in &entries {
                let rows = client.execute(&stmt, &[
                    &entry.id, &entry.session_id, &entry.role, &entry.content,
                    &entry.project, &entry.entry_type, &entry.raw_id,
                    &entry.model, &entry.input_tokens, &entry.output_tokens,
                    &entry.cache_creation_tokens, &entry.cache_read_tokens,
                    &entry.message_timestamp, &entry.created_at,
                ]).await.map_err(|e| format!("Failed to insert conversation {}: {}", entry.id, e))?;
                if rows > 0 { inserted += 1; } else { skipped += 1; }
            }
            if total > 0 { eprintln!("  conversations: {}/{} rows exported", inserted, total); }
        }
        "tasks" => {
            let stmt = client.prepare(
                "INSERT INTO tasks (id, project, subject, description, status, priority, task_type, \
                 parent_id, due_date, created_by, assignee, owner, session_id, created_at, updated_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15) ON CONFLICT DO NOTHING"
            ).await.map_err(|e| format!("Failed to prepare tasks insert: {}", e))?;

            let tasks = db.export_all_tasks()
                .map_err(|e| format!("Failed to read tasks: {}", e))?;
            let total = tasks.len();
            for task in &tasks {
                let rows = client.execute(&stmt, &[
                    &task.id, &task.project, &task.subject, &task.description,
                    &task.status, &task.priority, &task.task_type, &task.parent_id,
                    &task.due_date, &task.created_by, &task.assignee, &task.owner,
                    &task.session_id, &task.created_at, &task.updated_at,
                ]).await.map_err(|e| format!("Failed to insert task {}: {}", task.id, e))?;
                if rows > 0 { inserted += 1; } else { skipped += 1; }
            }
            if total > 0 { eprintln!("  tasks: {}/{} rows exported", inserted, total); }
        }
        "task_deps" => {
            let stmt = client.prepare(
                "INSERT INTO task_deps (blocker_id, blocked_id) \
                 VALUES ($1, $2) ON CONFLICT DO NOTHING"
            ).await.map_err(|e| format!("Failed to prepare task_deps insert: {}", e))?;

            let deps = db.export_all_task_deps()
                .map_err(|e| format!("Failed to read task_deps: {}", e))?;
            let total = deps.len();
            for (blocker_id, blocked_id) in &deps {
                let rows = client.execute(&stmt, &[blocker_id, blocked_id])
                    .await.map_err(|e| format!("Failed to insert task_dep ({}, {}): {}", blocker_id, blocked_id, e))?;
                if rows > 0 { inserted += 1; } else { skipped += 1; }
            }
            if total > 0 { eprintln!("  task_deps: {}/{} rows exported", inserted, total); }
        }
        "links" => {
            let stmt = client.prepare(
                "INSERT INTO links (id, source_type, source_id, target_type, target_id, relation, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT DO NOTHING"
            ).await.map_err(|e| format!("Failed to prepare links insert: {}", e))?;

            let links = db.export_all_links()
                .map_err(|e| format!("Failed to read links: {}", e))?;
            let total = links.len();
            for link in &links {
                let rows = client.execute(&stmt, &[
                    &link.id, &link.source_type, &link.source_id,
                    &link.target_type, &link.target_id, &link.relation, &link.created_at,
                ]).await.map_err(|e| format!("Failed to insert link {}: {}", link.id, e))?;
                if rows > 0 { inserted += 1; } else { skipped += 1; }
            }
            if total > 0 { eprintln!("  links: {}/{} rows exported", inserted, total); }
        }
        _ => return Err(format!("Unknown table: {}", table)),
    }

    Ok((inserted, skipped))
}

pub async fn run_export(
    db: &Database,
    url: &str,
    tables: &[String],
) -> Result<(), String> {
    if !url.starts_with("postgres://") && !url.starts_with("postgresql://") {
        return Err(format!(
            "Only PostgreSQL is currently supported. URL must start with postgres:// or postgresql://. \
             MySQL/MariaDB support will be added when the Rust toolchain supports sqlx."
        ));
    }

    let (client, connection) = tokio_postgres::connect(url, NoTls)
        .await
        .map_err(|e| format!("Failed to connect to PostgreSQL: {}", e))?;

    // Spawn the connection handler
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("PostgreSQL connection error: {}", e);
        }
    });

    // Create tables
    let ddl_statements = create_tables_ddl();
    for (table_name, ddl) in &ddl_statements {
        if tables.iter().any(|t| t == table_name) {
            client.execute(*ddl, &[])
                .await
                .map_err(|e| format!("Failed to create table '{}': {}", table_name, e))?;
        }
    }

    // Export data in FK dependency order
    let export_order = ["memories", "conversations", "tasks", "task_deps", "links"];
    let mut total_inserted: usize = 0;
    let mut total_skipped: usize = 0;

    for table in &export_order {
        if tables.iter().any(|t| t == *table) {
            let (inserted, skipped) = export_table(db, &client, table).await?;
            total_inserted += inserted;
            total_skipped += skipped;
        }
    }

    println!("Export complete.");
    println!("  Target: {}", sanitize_url(url));
    println!("  Total: {} inserted, {} skipped", total_inserted, total_skipped);

    Ok(())
}

fn sanitize_url(url: &str) -> String {
    if let Some(at_pos) = url.find('@') {
        if let Some(scheme_end) = url.find("://") {
            let after_scheme = &url[scheme_end + 3..at_pos];
            if let Some(colon_pos) = after_scheme.find(':') {
                let user = &after_scheme[..colon_pos];
                return format!("{}://{}:***@{}", &url[..scheme_end], user, &url[at_pos + 1..]);
            }
        }
    }
    url.to_string()
}

pub fn parse_tables(tables_arg: Option<&str>) -> Vec<String> {
    match tables_arg {
        Some(t) => t.split(',').map(|s| s.trim().to_string()).collect(),
        None => ALL_TABLES.iter().map(|s| s.to_string()).collect(),
    }
}
