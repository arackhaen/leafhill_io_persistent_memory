use std::process::Command;

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_leafhill-persistent-memory"))
}

/// Helper: create a temp dir and return (db_path, archive_path)
fn temp_paths(name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("leafhill-test-{}-{}", name, std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("test.db");
    let archive = dir.join("archive.json");
    (db, archive)
}

fn cleanup(dir: &std::path::Path) {
    let _ = std::fs::remove_dir_all(dir);
}

/// Store a memory using the CLI, returns the command output
fn store_memory(db: &std::path::Path, category: &str, key: &str, value: &str) -> std::process::Output {
    binary()
        .env("CLAUDE_MEMORY_DB", db)
        .args(["store", category, key, value])
        .output()
        .expect("failed to run store")
}

#[test]
fn test_archive_roundtrip() {
    let (db, archive) = temp_paths("roundtrip");
    let dir = db.parent().unwrap().to_path_buf();

    // Store some memories
    let out = store_memory(&db, "cat1", "k1", "value one");
    assert!(out.status.success(), "store failed: {}", String::from_utf8_lossy(&out.stderr));
    store_memory(&db, "cat1", "k2", "value two");
    store_memory(&db, "cat2", "k3", "value three");

    // Create archive
    let out = binary()
        .env("CLAUDE_MEMORY_DB", &db)
        .args(["archive", "create", archive.to_str().unwrap(), "--entity-type", "memories"])
        .output()
        .expect("failed to run archive create");
    assert!(out.status.success(), "archive create failed: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("memories: 3"));

    // Read archive file and verify it's valid JSON
    let json_str = std::fs::read_to_string(&archive).unwrap();
    let envelope: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(envelope["counts"]["memories"], 3);

    // Restore into a new DB
    let db2 = dir.join("test2.db");
    let out = binary()
        .env("CLAUDE_MEMORY_DB", &db2)
        .args(["archive", "restore", archive.to_str().unwrap()])
        .output()
        .expect("failed to run archive restore");
    assert!(out.status.success(), "archive restore failed: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("3 restored"));

    // Verify the restored data
    let out = binary()
        .env("CLAUDE_MEMORY_DB", &db2)
        .args(["list"])
        .output()
        .expect("failed to run list");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("3 memories"));

    cleanup(&dir);
}

#[test]
fn test_archive_with_limit() {
    let (db, archive) = temp_paths("limit");
    let dir = db.parent().unwrap().to_path_buf();

    // Store 5 memories
    for i in 0..5 {
        store_memory(&db, "cat", &format!("k{}", i), &format!("val{}", i));
    }

    // Archive with --limit 2
    let out = binary()
        .env("CLAUDE_MEMORY_DB", &db)
        .args(["archive", "create", archive.to_str().unwrap(), "--entity-type", "memories", "--limit", "2"])
        .output()
        .expect("failed to run archive create");
    assert!(out.status.success(), "archive create failed: {}", String::from_utf8_lossy(&out.stderr));

    let json_str = std::fs::read_to_string(&archive).unwrap();
    let envelope: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(envelope["counts"]["memories"], 2);

    cleanup(&dir);
}

#[test]
fn test_archive_purge() {
    let (db, archive) = temp_paths("purge");
    let dir = db.parent().unwrap().to_path_buf();

    // Store memories
    store_memory(&db, "cat", "k1", "val1");
    store_memory(&db, "cat", "k2", "val2");

    // Archive with --purge
    let out = binary()
        .env("CLAUDE_MEMORY_DB", &db)
        .args(["archive", "create", archive.to_str().unwrap(), "--entity-type", "memories", "--purge"])
        .output()
        .expect("failed to run archive create");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Source data removed"));

    // Verify source data is gone
    let out = binary()
        .env("CLAUDE_MEMORY_DB", &db)
        .args(["list"])
        .output()
        .expect("failed to run list");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("No memories found"));

    cleanup(&dir);
}
