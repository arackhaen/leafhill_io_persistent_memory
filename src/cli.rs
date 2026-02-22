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
        /// Max results
        #[arg(long, short, default_value = "20")]
        limit: usize,
    },

    /// List conversation entries
    List {
        /// Filter by session ID
        #[arg(long, short)]
        session: Option<String>,
        /// Max results
        #[arg(long, short, default_value = "20")]
        limit: usize,
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

        Commands::Log { command: log_cmd } => match log_cmd {
            LogCommands::Search { query, session, limit } => {
                match db.search_conversations(&query, session.as_deref(), limit) {
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
            LogCommands::List { session, limit } => {
                match db.list_conversations(session.as_deref(), limit) {
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
    println!("[{}] {} (session: {})", entry.created_at, entry.role, entry.session_id);
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
