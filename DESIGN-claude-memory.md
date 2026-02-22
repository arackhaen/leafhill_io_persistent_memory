# leafhill.io Persistent Claude Memory: Design Document

## Overview

A Rust-based MCP server + CLI tool providing SQLite-backed persistent memory for Claude Code sessions at the user level (host+user specific, not project-scoped).

## Architecture

Single Rust binary (`leafhill-persistent-memory`) with two modes:
- `leafhill-persistent-memory serve` — MCP server over stdio (spawned by Claude Code)
- `leafhill-persistent-memory <subcommand>` — CLI for direct human queries

MCP protocol is implemented as manual JSON-RPC over stdio (no SDK dependency).

## Data Model

### SQLite Schema

```sql
CREATE TABLE memories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    category TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    tags TEXT,  -- JSON array of strings
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now')),
    UNIQUE(category, key)
);

CREATE TABLE conversations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    role TEXT NOT NULL,        -- 'user' or 'assistant'
    content TEXT NOT NULL,
    project TEXT,              -- project/repo context
    created_at TEXT DEFAULT (datetime('now'))
);

CREATE VIRTUAL TABLE memories_fts USING fts5(key, value, tags, content=memories, content_rowid=id);
CREATE VIRTUAL TABLE conversations_fts USING fts5(content, content=conversations, content_rowid=id);
```

## MCP Tools

| Tool | Parameters | Description |
|------|-----------|-------------|
| `store_memory` | category, key, value, tags? | Upsert a memory. Updates if category+key exists. |
| `search_memories` | query, category?, limit? | FTS5 search across memories. |
| `list_memories` | category?, limit? | List memories, optionally by category. |
| `delete_memory` | category, key | Delete by category+key. |
| `log_conversation` | session_id, role, content, project? | Log a conversation exchange. |
| `search_conversations` | query, session_id?, limit? | FTS5 search conversation history. |

## CLI Commands

```
leafhill-persistent-memory serve                           # MCP server (stdio)
leafhill-persistent-memory store <category> <key> <value> [--tags t1,t2]
leafhill-persistent-memory search <query> [--category <cat>] [--limit N]
leafhill-persistent-memory list [--category <cat>] [--limit N]
leafhill-persistent-memory delete <category> <key>
leafhill-persistent-memory log search <query> [--session <id>] [--limit N]
leafhill-persistent-memory log list [--session <id>] [--limit N]
```

## Storage

- Default: `~/.claude/memory.db`
- Override: `CLAUDE_MEMORY_DB` environment variable

## Claude Code Integration

Registered as user-scope MCP server in `~/.claude.json`:
```json
{
  "mcpServers": {
    "leafhill-persistent-memory": {
      "type": "stdio",
      "command": "/home/samileh/.local/bin/leafhill-persistent-memory",
      "args": ["serve"],
      "env": {}
    }
  }
}
```

## Dependencies

- `rusqlite` (bundled) — SQLite with FTS5
- `clap` — CLI parsing
- `serde` / `serde_json` — serialization

## Source Location

`~/.local/share/claude-memory/` (Cargo project)

## Decision Log

| # | Decision | Alternatives | Rationale |
|---|----------|-------------|-----------|
| 1 | Rust | Python, Node.js | User preference. Single binary, no runtime deps. |
| 2 | Manual JSON-RPC | rmcp crate | rmcp requires Rust 1.85+ (edition 2024), system has 1.75.0. Manual impl is simple for stdio. |
| 3 | Key-value + categories | Free-form, entity graph | Simple, structured, queryable. |
| 4 | Raw logs + extracted insights | One or the other | Maximum flexibility and coverage. |
| 5 | Claude extracts insights | Server-side LLM | Pure storage server, no API keys needed. |
| 6 | Single binary | Separate server+CLI | Simpler deployment. |
| 7 | ~/.claude/memory.db + env override | Fixed path | Sensible default, configurable. |
| 8 | FTS5 | LIKE, external search | Built-in, fast, rankable. |
