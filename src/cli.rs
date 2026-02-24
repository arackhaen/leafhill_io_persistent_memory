use clap::{Parser, Subcommand};
use crate::db::Database;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "leafhill-persistent-memory")]
#[command(about = "leafhill.io Persistent Claude Memory - SQLite-backed persistent memory for Claude Code sessions")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the MCP server (stdio transport)
    Serve,

    /// Store a memory (upserts if category+key exists)
    Store {
        /// Category (e.g. preferences, patterns, facts, insights)
        category: String,
        /// Unique key within the category
        key: String,
        /// The memory value/content
        value: String,
        /// Comma-separated tags
        #[arg(long)]
        tags: Option<String>,
    },

    /// Full-text search across memories
    Search {
        /// Search query
        query: String,
        /// Filter by category
        #[arg(long, short)]
        category: Option<String>,
        /// Max results
        #[arg(long, short, default_value = "20")]
        limit: usize,
    },

    /// List memories
    List {
        /// Filter by category
        #[arg(long, short)]
        category: Option<String>,
        /// Max results
        #[arg(long, short, default_value = "50")]
        limit: usize,
    },

    /// Delete a memory
    Delete {
        /// Category
        category: String,
        /// Key
        key: String,
    },

    /// Handle Claude Code hook events (reads JSON from stdin)
    HookHandler,

    /// Task management operations
    Task {
        #[command(subcommand)]
        command: TaskCommands,
    },

    /// Conversation log operations
    Log {
        #[command(subcommand)]
        command: LogCommands,
    },
}

#[derive(Subcommand)]
pub enum LogCommands {
    /// Search conversation history
    Search {
        /// Search query
        query: String,
        /// Filter by session ID
        #[arg(long, short)]
        session: Option<String>,
        /// Filter by entry type (raw_user, raw_assistant, summary)
        #[arg(long, name = "type")]
        entry_type: Option<String>,
        /// Max results
        #[arg(long, short, default_value = "20")]
        limit: usize,
    },

    /// List conversation entries
    List {
        /// Filter by session ID
        #[arg(long, short)]
        session: Option<String>,
        /// Filter by entry type (raw_user, raw_assistant, summary)
        #[arg(long, name = "type")]
        entry_type: Option<String>,
        /// Max results
        #[arg(long, short, default_value = "20")]
        limit: usize,
    },

    /// Get all summaries for a session
    Context {
        /// Session ID
        session_id: String,
        /// Max results
        #[arg(long, short, default_value = "50")]
        limit: usize,
    },

    /// Prune old conversation entries
    Prune {
        /// Delete entries older than N days
        #[arg(long)]
        older_than: i64,
        /// Only prune entries of this type
        #[arg(long, name = "type")]
        entry_type: Option<String>,
    },
}

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

pub fn run_cli(command: Commands, db_path: &PathBuf) {
    let db = match Database::open(db_path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Failed to open database at {:?}: {}", db_path, e);
            std::process::exit(1);
        }
    };

    match command {
        Commands::Serve => unreachable!("serve handled in main"),
        Commands::HookHandler => unreachable!("hook-handler handled in main"),

        Commands::Store { category, key, value, tags } => {
            let tag_vec: Option<Vec<String>> = tags.map(|t| {
                t.split(',').map(|s| s.trim().to_string()).collect()
            });
            match db.store_memory(&category, &key, &value, tag_vec.as_deref()) {
                Ok(mem) => {
                    println!("Stored: [{}:{}]", mem.category, mem.key);
                    println!("  Value: {}", mem.value);
                    if let Some(tags) = &mem.tags {
                        println!("  Tags: {}", tags.join(", "));
                    }
                    println!("  Updated: {}", mem.updated_at);
                }
                Err(e) => {
                    eprintln!("Failed to store: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Search { query, category, limit } => {
            match db.search_memories(&query, category.as_deref(), limit) {
                Ok(memories) => {
                    if memories.is_empty() {
                        println!("No memories found.");
                    } else {
                        for mem in &memories {
                            print_memory(mem);
                        }
                        println!("\n({} results)", memories.len());
                    }
                }
                Err(e) => {
                    eprintln!("Search failed: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::List { category, limit } => {
            match db.list_memories(category.as_deref(), limit) {
                Ok(memories) => {
                    if memories.is_empty() {
                        println!("No memories found.");
                    } else {
                        for mem in &memories {
                            print_memory(mem);
                        }
                        println!("\n({} memories)", memories.len());
                    }
                }
                Err(e) => {
                    eprintln!("List failed: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Delete { category, key } => {
            match db.delete_memory(&category, &key) {
                Ok(true) => println!("Deleted: [{}:{}]", category, key),
                Ok(false) => {
                    eprintln!("Not found: [{}:{}]", category, key);
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("Delete failed: {}", e);
                    std::process::exit(1);
                }
            }
        }

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
                let mut updates = serde_json::Map::new();
                if let Some(v) = status { updates.insert("status".into(), serde_json::Value::String(v)); }
                if let Some(v) = subject { updates.insert("subject".into(), serde_json::Value::String(v)); }
                if let Some(v) = description { updates.insert("description".into(), serde_json::Value::String(v)); }
                if let Some(v) = assignee { updates.insert("assignee".into(), serde_json::Value::String(v)); }
                if let Some(v) = owner { updates.insert("owner".into(), serde_json::Value::String(v)); }
                if let Some(v) = priority { updates.insert("priority".into(), serde_json::Value::String(v)); }
                if let Some(v) = due { updates.insert("due_date".into(), serde_json::Value::String(v)); }
                let updates = serde_json::Value::Object(updates);
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

        Commands::Log { command: log_cmd } => match log_cmd {
            LogCommands::Search { query, session, entry_type, limit } => {
                match db.search_conversations(&query, session.as_deref(), entry_type.as_deref(), limit) {
                    Ok(entries) => {
                        if entries.is_empty() {
                            println!("No conversations found.");
                        } else {
                            for entry in &entries {
                                print_conversation(entry);
                            }
                            println!("\n({} results)", entries.len());
                        }
                    }
                    Err(e) => {
                        eprintln!("Search failed: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            LogCommands::List { session, entry_type, limit } => {
                match db.list_conversations(session.as_deref(), entry_type.as_deref(), limit) {
                    Ok(entries) => {
                        if entries.is_empty() {
                            println!("No conversations found.");
                        } else {
                            for entry in &entries {
                                print_conversation(entry);
                            }
                            println!("\n({} entries)", entries.len());
                        }
                    }
                    Err(e) => {
                        eprintln!("List failed: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            LogCommands::Context { session_id, limit } => {
                match db.get_conversation_context(&session_id, limit) {
                    Ok(entries) => {
                        if entries.is_empty() {
                            println!("No summaries found for session.");
                        } else {
                            for entry in &entries {
                                print_conversation(entry);
                            }
                            println!("\n({} summaries)", entries.len());
                        }
                    }
                    Err(e) => {
                        eprintln!("Context failed: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            LogCommands::Prune { older_than, entry_type } => {
                if older_than <= 0 {
                    eprintln!("--older-than must be a positive number of days");
                    std::process::exit(1);
                }
                match db.prune_conversations(older_than, entry_type.as_deref()) {
                    Ok(count) => println!("Pruned {} entries", count),
                    Err(e) => {
                        eprintln!("Prune failed: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        },
    }
}

fn print_memory(mem: &crate::db::Memory) {
    println!("---");
    println!("[{}:{}] (id: {})", mem.category, mem.key, mem.id);
    println!("  {}", mem.value);
    if let Some(tags) = &mem.tags {
        println!("  Tags: {}", tags.join(", "));
    }
    println!("  Updated: {}", mem.updated_at);
}

fn print_conversation(entry: &crate::db::ConversationEntry) {
    println!("---");
    let etype = entry.entry_type.as_deref().unwrap_or("unknown");
    println!("[{}] {} [{}] (session: {})", entry.created_at, entry.role, etype, entry.session_id);
    if let Some(project) = &entry.project {
        println!("  Project: {}", project);
    }
    let preview: String = entry.content.chars().take(200).collect();
    if entry.content.len() > 200 {
        println!("  {}...", preview);
    } else {
        println!("  {}", preview);
    }
}

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
