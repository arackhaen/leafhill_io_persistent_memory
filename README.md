# leafhill_io_persistent_memory

Persistent Memory for Claude Code — a Rust MCP server providing persistent memory, conversation logging, task management, and semantic linking across sessions.

## Features

- **Memory storage** — Store and search insights, preferences, patterns, and facts
- **Conversation logging** — Record and search significant exchanges
- **Task management** — Create, update, and track tasks with dependencies and priorities
- **Semantic linking** — Link tasks, memories, and conversations with relation labels
- **Backup** — Create SQLite backup copies of the database
- **Archive** — Export entities to JSON files with optional restore
- **RDBMS export** — Export data to PostgreSQL for external analysis

## Installation

Build from source (requires Rust 1.75+):

```bash
cargo build --release
```

The binary is at `target/release/leafhill-persistent-memory`.

## Usage

### MCP Server

```bash
leafhill-persistent-memory serve
```

Runs as an MCP server over stdio. Configure in your Claude Code settings.

### CLI Commands

#### Backup

Create a full SQLite backup of the database:

```bash
leafhill-persistent-memory backup /path/to/backup.db
leafhill-persistent-memory backup /path/to/backup.db --force  # overwrite existing
```

#### Archive

Export entities to a JSON file (default: moves data out of the database):

```bash
# Archive all memories
leafhill-persistent-memory archive create /path/to/archive.json --entity-type memories

# Archive tasks for a specific project, keeping originals
leafhill-persistent-memory archive create /path/to/archive.json --entity-type tasks --project myproject --keep

# Archive conversations by date range
leafhill-persistent-memory archive create /path/to/archive.json --entity-type conversations --before 2026-01-01

# Restore from archive (merge with skip-duplicates)
leafhill-persistent-memory archive restore /path/to/archive.json
```

Supported entity types: `memories`, `conversations`, `tasks`, `all`

Archive cascades automatically: archiving a task includes its subtasks, dependencies, and related links.

#### Export to PostgreSQL

Export data to an external PostgreSQL database:

```bash
# Export all tables
leafhill-persistent-memory export "postgres://user:pass@host:5432/dbname"

# Export specific tables
leafhill-persistent-memory export "postgres://user:pass@host/db" --tables "memories,tasks"
```

Uses `ON CONFLICT DO NOTHING` — safe to run repeatedly (idempotent). Tables are created automatically if they don't exist.

#### Other CLI Commands

```bash
# Memory operations
leafhill-persistent-memory memory list
leafhill-persistent-memory memory search "query"

# Task operations
leafhill-persistent-memory task list --project myproject
leafhill-persistent-memory task get 42

# Log operations
leafhill-persistent-memory log search "query"

# Link operations
leafhill-persistent-memory link list --type task --id 42
```

## Database

Data is stored in SQLite at `~/.claude/memory.db`. Override with the `CLAUDE_MEMORY_DB` environment variable.

## License

See LICENSE file.
