# Design: Backup, RDBMS Export, and Archive Features

**Date:** 2026-02-24
**Version target:** 1.3.0
**Status:** Approved

---

## Understanding Summary

- **What:** Three new features for the leafhill-persistent-memory Rust binary: (1) SQLite backup to file, (2) RDBMS-agnostic export using sqlx, (3) selective archive with restore
- **Why:** Enable data safety (backup), external analysis/reporting (RDBMS export), and data lifecycle management (archive/restore)
- **Who:** End users of the leafhill-persistent-memory CLI tool
- **Key constraints:**
  - All features are CLI subcommands in the existing Rust binary
  - RDBMS export uses `sqlx` for database-agnostic support (PostgreSQL, MySQL, MariaDB)
  - Archive format is JSON, restore merges with skip-duplicates strategy
  - RDBMS export is one-way only (SQLite → RDBMS)
  - Backup uses SQLite `VACUUM INTO` for lossless file copy
  - Medium data scale (thousands of records, up to ~100MB) — batching needed for export
- **Non-goals:**
  - Bidirectional RDBMS sync
  - Automatic/scheduled backups
  - Archive in SQLite format

## Assumptions

1. The `sqlx` crate will be added as a new dependency (with async runtime via `tokio`)
2. RDBMS connection string will be provided via CLI argument
3. The RDBMS target schema mirrors the SQLite schema (same tables/columns, minus FTS5)
4. Archive files are self-contained JSON with schema version for forward compatibility
5. Archive default is move (delete from SQLite); --keep flag retains originals
6. FTS5 indexes are not exported to RDBMS (SQLite-specific; triggers keep them in sync locally)

## Architecture

**Approach B: Separate modules per feature**

Three new source files, each owning its CLI subcommand independently:

```
src/
├── main.rs           # Updated: new command variants
├── cli.rs            # Updated: new Clap subcommands
├── db.rs             # Updated: new query methods for archive
├── backup.rs         # NEW: backup logic
├── rdbms_export.rs   # NEW: RDBMS export logic
├── archive.rs        # NEW: archive create + restore logic
├── mcp.rs            # Unchanged
└── hook.rs           # Unchanged
```

New dependencies in Cargo.toml:
- `sqlx = { version = "0.8", features = ["runtime-tokio", "postgres", "mysql"] }`
- `tokio = { version = "1", features = ["rt", "macros"] }`
- `chrono = "0.4"` (for timestamp generation in archive envelope)

---

## Feature 1: Backup (`src/backup.rs`)

### CLI Interface

```
leafhill-persistent-memory backup <output-path> [--force]
```

### Behavior

1. Validates that `<output-path>` does not exist (or `--force` is set)
2. Creates parent directories if needed
3. Calls SQLite `VACUUM INTO '<path>'` — creates a compacted, consistent copy
4. Prints file size and record counts per table on success
5. Exit code 0 on success, 1 on failure

### Implementation

```rust
pub fn run_backup(db: &Database, output: &Path, force: bool) -> Result<()>
```

- Uses `db.conn.execute("VACUUM INTO ?1", params![path_str])`
- Opens the backup file read-only to query record counts for the summary
- No new dependencies needed (pure rusqlite)

### Error Cases

- Output path exists and --force not set → error with message
- SQLite VACUUM INTO fails (disk full, permissions) → propagate error
- Parent directory creation fails → propagate error

---

## Feature 2: RDBMS Export (`src/rdbms_export.rs`)

### CLI Interface

```
leafhill-persistent-memory export <database-url> [--tables memories,conversations,tasks,task_deps,links] [--batch-size 500]
```

### Behavior

1. Parses the database URL to determine RDBMS type (postgres://, mysql://)
2. Connects to the target database using sqlx
3. Creates target tables if they don't exist (DDL per RDBMS dialect)
4. For each selected table (default: all 5 core tables):
   a. Reads rows from SQLite in batches (default 500)
   b. Inserts into RDBMS with ON CONFLICT DO NOTHING / ON DUPLICATE KEY IGNORE
5. Prints per-table row counts (inserted vs. skipped) on completion

### Schema Mapping

| SQLite | PostgreSQL | MySQL |
|--------|-----------|-------|
| `INTEGER PRIMARY KEY AUTOINCREMENT` | `SERIAL PRIMARY KEY` | `INT PRIMARY KEY AUTO_INCREMENT` |
| `TEXT NOT NULL` | `TEXT NOT NULL` | `TEXT NOT NULL` |
| `TEXT DEFAULT (datetime('now'))` | `TEXT DEFAULT CURRENT_TIMESTAMP` | `TEXT DEFAULT (CURRENT_TIMESTAMP)` |
| `UNIQUE(col1, col2)` | `UNIQUE(col1, col2)` | `UNIQUE(col1, col2)` |
| `REFERENCES tasks(id)` | `REFERENCES tasks(id)` | `REFERENCES tasks(id)` |

FTS5 tables (`memories_fts`, `conversations_fts`, `tasks_fts`) and their associated triggers are **not** exported.

### Table Export Order

Tables must be exported in dependency order to satisfy foreign key constraints:

1. `memories` (no FK dependencies)
2. `conversations` (no FK dependencies)
3. `tasks` (self-referential FK: parent_id)
4. `task_deps` (FK: blocker_id, blocked_id → tasks)
5. `links` (no FK, but references entity IDs semantically)

For `tasks`, parent tasks must be inserted before subtasks. Query orders by `parent_id NULLS FIRST`.

### Implementation

```rust
pub async fn run_export(db: &Database, url: &str, tables: &[String], batch_size: usize) -> Result<()>
```

- Uses `sqlx::AnyPool` for RDBMS-agnostic connections
- DDL statements are dialect-specific (matched on URL scheme)
- Reads from SQLite via existing `db.conn` (rusqlite, synchronous)
- Writes to RDBMS via sqlx (async)
- Main function wrapped in `tokio::runtime::Runtime::new()` block in CLI dispatch

### Error Cases

- Invalid database URL → error with message
- Connection failure → error with connection details (no password)
- Table creation failure → propagate DDL error
- Batch insert failure → report which table/batch failed, abort remaining

---

## Feature 3: Archive (`src/archive.rs`)

### CLI Interface

```
leafhill-persistent-memory archive create <output-path> --type <memories|conversations|tasks|all> [--older-than <days>] [--project <name>] [--category <cat>] [--keep] [--force]
leafhill-persistent-memory archive restore <input-path>
```

### Archive JSON Format

```json
{
  "schema_version": "1.0",
  "created_at": "2026-02-24T12:00:00Z",
  "source_db": "~/.claude/memory.db",
  "entity_types": ["memories"],
  "filters": {
    "older_than_days": 30,
    "project": "myapp",
    "category": "facts"
  },
  "counts": {
    "memories": 42,
    "links": 5
  },
  "data": {
    "memories": [ ... ],
    "conversations": [ ... ],
    "tasks": [ ... ],
    "task_deps": [ ... ],
    "links": [ ... ]
  }
}
```

### Archive Create Behavior

1. Validates output path (refuses overwrite unless `--force`)
2. Queries entities matching filters:
   - `--type memories`: memories table, optionally filtered by `--category` and `--older-than`
   - `--type conversations`: conversations table, optionally filtered by `--project` and `--older-than`
   - `--type tasks`: tasks table, optionally filtered by `--project` and `--older-than`
   - `--type all`: all entity types, with applicable filters
3. **Cascade collection:**
   - For tasks: includes subtasks (recursive via `parent_id`), related `task_deps`, and `links`
   - For memories: includes related `links`
   - For conversations: includes related `links`
4. Serializes to JSON with envelope metadata
5. Writes atomically (write to temp file, then rename)
6. **Default: deletes archived records from SQLite** (in a transaction)
   - Deletion order respects FK constraints: links → task_deps → tasks → conversations → memories
7. **--keep flag:** skips the deletion step
8. Prints summary: entity counts archived, file size

### Archive Restore Behavior

1. Reads and parses JSON file
2. Validates `schema_version` (must be compatible)
3. Inserts records using `INSERT OR IGNORE` (skip duplicates):
   - Insert order respects FK constraints: memories → conversations → tasks → task_deps → links
4. Prints summary: restored count vs. skipped count per entity type

### Implementation

```rust
pub fn run_archive_create(db: &Database, output: &Path, entity_type: &str, filters: ArchiveFilters, keep: bool, force: bool) -> Result<()>
pub fn run_archive_restore(db: &Database, input: &Path) -> Result<()>
```

### Cascade Rules

| Archived Entity | Also Archived |
|----------------|---------------|
| Memory (id=X) | Links where (source_type='memory', source_id=X) or (target_type='memory', target_id=X) |
| Conversation (id=X) | Links where (source_type='conversation', source_id=X) or (target_type='conversation', target_id=X) |
| Task (id=X) | Subtasks (parent_id=X, recursive), task_deps referencing any archived task, links referencing any archived task |

### Error Cases

- Output path exists and --force not set → error
- No entities match filters → warning, no file created
- JSON parse failure on restore → error with details
- Incompatible schema_version → error with expected vs. found
- Deletion after archive fails → error (archive file remains, data not deleted — safe state)

### Technical Evaluation (per user request)

**Potential issues with archive/restore:**

1. **ID conflicts on restore.** Archived records have original IDs. If new records were created after archival, IDs may collide. Mitigation: `INSERT OR IGNORE` skips conflicts. Consequence: if a new record took the same ID, the archived record is silently skipped. This is acceptable for the skip-duplicates strategy.

2. **Cross-entity reference integrity on restore.** Links reference entity IDs. If an archived link points to memory ID=5, but memory ID=5 was re-created with different content after archival, the link would point to wrong data. Mitigation: this is a known limitation of skip-duplicates. Document it clearly.

3. **Partial archives.** If the user archives memories but not the conversations that reference them via links, restored links may point to non-existent memories. Mitigation: cascade archival includes related links, so they move together.

4. **Archive file tampering.** A manually edited JSON file could inject unexpected data on restore. Mitigation: validate schema_version and data types during restore. Do not execute arbitrary SQL from archive content.

---

## Decision Log

| # | Decision | Alternatives Considered | Rationale |
|---|----------|------------------------|-----------|
| 1 | All features as CLI subcommands in existing Rust binary | Separate scripts, standalone tools | Single binary, consistent UX |
| 2 | Approach B: separate modules per feature | Unified export module, external tooling | Clean isolation, no shared logic duplication risk |
| 3 | Backup uses SQLite VACUUM INTO | SQL dump, JSON export | Lossless, fastest, simplest restore |
| 4 | RDBMS export via sqlx (Rust-native, RDBMS-agnostic) | SQLAlchemy (Python), SQL dump, tokio-postgres only | Stays in Rust, supports Postgres+MySQL+MariaDB |
| 5 | One-way RDBMS export only | Bidirectional sync | YAGNI, avoids conflict resolution complexity |
| 6 | Archive format: JSON with schema version envelope | SQLite file, both formats | Human-readable, inspectable, forward-compatible |
| 7 | Archive default: move (delete after archive) | Copy only, user chooses | Reduces DB size; --keep flag for safety |
| 8 | Restore strategy: merge, skip duplicates | Overwrite, user chooses | Safest default, no data loss |
| 9 | Archive cascades to links and task_deps | Warn and skip, orphan links | Clean data, no dangling references |
| 10 | Parent task archive cascades to subtasks | Independent archival | Preserves hierarchy integrity |
| 11 | Archive entity types: any (memories, conversations, tasks) | Conversations only, full project | Maximum flexibility |
| 12 | Medium scale design (batching for exports) | Small/simple, large/streaming | Matches expected data growth |

---

## Implementation Plan

### Phase 1: Backup (no new dependencies)
1. Create `src/backup.rs` with `run_backup()` function
2. Add `Backup` command variant to CLI in `src/cli.rs` and `src/main.rs`
3. Test: backup creates valid SQLite file, --force behavior, error cases

### Phase 2: Archive (minimal new dependencies)
1. Add `chrono` dependency to Cargo.toml
2. Add archive query methods to `src/db.rs` (filtered reads, batch deletes)
3. Create `src/archive.rs` with create and restore functions
4. Add `Archive` command variants to CLI in `src/cli.rs` and `src/main.rs`
5. Test: archive create/restore round-trip, cascade behavior, --keep flag, skip duplicates

### Phase 3: RDBMS Export (sqlx + tokio dependencies)
1. Add `sqlx` and `tokio` dependencies to Cargo.toml
2. Create `src/rdbms_export.rs` with DDL generation and batch export
3. Add `Export` command variant to CLI in `src/cli.rs` and `src/main.rs`
4. Test: export to PostgreSQL, schema creation, batch inserts, conflict handling

### Phase 4: Integration
1. Update `application_version.txt` to 1.3.0
2. Update README.md with new commands
3. Update CLAUDE.md version reference
