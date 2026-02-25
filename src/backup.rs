use crate::db::Database;
use std::path::Path;

pub fn run_backup(db: &Database, output: &Path, force: bool) -> Result<(), String> {
    if output.exists() && !force {
        return Err(format!(
            "Output file already exists: {}. Use --force to overwrite.",
            output.display()
        ));
    }

    if let Some(parent) = output.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory {}: {}", parent.display(), e))?;
        }
    }

    // If force and file exists, remove it first (VACUUM INTO won't overwrite)
    if output.exists() && force {
        std::fs::remove_file(output)
            .map_err(|e| format!("Failed to remove existing file: {}", e))?;
    }

    let output_str = output.to_str().ok_or("Invalid output path encoding")?;

    db.backup_to(output_str)
        .map_err(|e| format!("Backup failed: {}", e))?;

    let metadata = std::fs::metadata(output)
        .map_err(|e| format!("Failed to read backup file metadata: {}", e))?;

    let size_bytes = metadata.len();
    let size_display = if size_bytes >= 1_048_576 {
        format!("{:.1} MB", size_bytes as f64 / 1_048_576.0)
    } else if size_bytes >= 1024 {
        format!("{:.1} KB", size_bytes as f64 / 1024.0)
    } else {
        format!("{} bytes", size_bytes)
    };

    let counts = db.table_counts()
        .map_err(|e| format!("Failed to read table counts: {}", e))?;

    println!("Backup created: {}", output.display());
    println!("  Size: {}", size_display);
    println!("  Records:");
    for (table, count) in &counts {
        println!("    {}: {}", table, count);
    }

    Ok(())
}
