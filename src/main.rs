mod archive;
mod backup;
mod cli;
mod db;
mod hook;
mod mcp;
mod rdbms_export;

use clap::Parser;
use cli::{Cli, Commands};
use std::path::PathBuf;

fn get_db_path() -> PathBuf {
    if let Ok(path) = std::env::var("CLAUDE_MEMORY_DB") {
        PathBuf::from(path)
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(home).join(".claude").join("memory.db")
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let db_path = get_db_path();

    match cli.command {
        Commands::Serve => {
            mcp::serve(&db_path)?;
            Ok(())
        }
        Commands::HookHandler => {
            hook::handle_hook(&db_path);
            Ok(())
        }
        other => {
            cli::run_cli(other, &db_path);
            Ok(())
        }
    }
}
