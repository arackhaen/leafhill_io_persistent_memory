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
- **PreCompact transcript storage** — Automatically preserve full session transcripts before context compaction

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

Export entities to a JSON file (default: non-destructive, data stays in the database):

```bash
# Archive all memories
leafhill-persistent-memory archive create /path/to/archive.json --entity-type memories

# Archive tasks for a specific project
leafhill-persistent-memory archive create /path/to/archive.json --entity-type tasks --project myproject

# Archive conversations older than 30 days
leafhill-persistent-memory archive create /path/to/archive.json --entity-type conversations --older-than 30

# Restore from archive (merge with skip-duplicates)
leafhill-persistent-memory archive restore /path/to/archive.json
```

Supported entity types: `memories`, `conversations`, `tasks`, `all`

Archive cascades automatically: archiving a task includes its subtasks, dependencies, and related links.

Use `--purge` to remove source data from the database after archiving.

#### Export to PostgreSQL

Export data to an external PostgreSQL database:

```bash
# Export all tables
leafhill-persistent-memory export "postgres://user:pass@host:5432/dbname"

# Export specific tables
leafhill-persistent-memory export "postgres://user:pass@host/db" --tables "memories,tasks"
```

Uses `ON CONFLICT DO NOTHING` — safe to run repeatedly (idempotent). Tables are created automatically if they don't exist.

#### Hook Handler

The binary doubles as a Claude Code hook handler for automatic conversation capture:

```bash
leafhill-persistent-memory hook-handler
```

Reads hook event JSON from stdin. Supports these events:

- **SessionStart** — Injects session ID and project context into the conversation
- **UserPromptSubmit** — Logs raw user prompts
- **Stop** — Logs raw assistant responses
- **PreCompact** — Stores the full session transcript before context compaction

##### PreCompact Transcript Storage

When Claude Code auto-compacts context, the PreCompact hook preserves the complete transcript to SQLite. Each user/assistant message becomes a separate searchable entry with `entry_type='pre_compact'`.

Stored metadata per message:
- `model` — Model identifier (e.g., `claude-opus-4-6`)
- `input_tokens`, `output_tokens` — Token usage
- `cache_creation_tokens`, `cache_read_tokens` — Cache token accounting
- `message_timestamp` — Original ISO 8601 timestamp from the transcript

Content extraction rules:
- User string messages: stored as-is
- User tool_result arrays: text content extracted
- Assistant messages: text and thinking blocks stored, tool_use blocks skipped

All error paths exit 0 (PreCompact never blocks compaction).

Configure in `~/.claude/settings.json`:

```json
{
  "hooks": {
    "PreCompact": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "/path/to/leafhill-persistent-memory hook-handler",
            "timeout": 600
          }
        ]
      }
    ]
  }
}
```

Query stored transcripts:

```bash
leafhill-persistent-memory log list --entry-type pre_compact
leafhill-persistent-memory log search "search term"
```

#### Other CLI Commands

```bash
# Memory operations
leafhill-persistent-memory list
leafhill-persistent-memory search "query"

# Task operations
leafhill-persistent-memory task list --project myproject
leafhill-persistent-memory task get 42

# Log operations
leafhill-persistent-memory log search "query"

# Link operations
leafhill-persistent-memory link list task 42
```

## Database

Data is stored in SQLite at `~/.claude/memory.db`. Override with the `CLAUDE_MEMORY_DB` environment variable.

## License

See LICENSE file.
