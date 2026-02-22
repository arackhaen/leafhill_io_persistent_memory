# Automatic Conversation Logging: Design Document

## Overview

Hybrid approach to persistent conversation logging: Claude Code hooks capture raw user/assistant messages automatically, while Claude writes concise summaries via CLAUDE.md instruction after every exchange. This enables future context retrieval without excessive token usage.

## Approach

**Approach C — Hybrid: Hook Captures Raw, Claude Summarizes Inline**

Hooks store raw text as a safety net. CLAUDE.md instructs Claude to enrich with summaries after every exchange. Summaries are the primary retrieval target; raw entries provide detail when needed.

## Data Model Changes

Add two columns to the `conversations` table:

| Column | Type | Purpose |
|--------|------|---------|
| `entry_type` | TEXT | `"raw_user"`, `"raw_assistant"`, or `"summary"` |
| `raw_id` | INTEGER | For summaries: references the raw entry id. NULL for raw entries. |

Existing rows get NULL for both columns (backward compatible).

Session ID format: `{claude_session_id}-{YYYY-MM-DD-HHMMSS}-{project_name|unknownX}`

FTS table unchanged — indexes `content` regardless of entry type.

## Hook Layer — Raw Capture

Three hooks in `~/.claude/settings.json`:

### SessionStart hook
- Generates the formatted session_id
- Injects it as `additionalContext` so Claude knows the session_id to use
- Also injects project name derived from `cwd`

### UserPromptSubmit hook
- Pipes stdin JSON to `leafhill-persistent-memory hook-handler`
- Binary reads the hook JSON, extracts `prompt`, `session_id`, `cwd`
- Stores as `entry_type = "raw_user"`

### Stop hook
- Pipes stdin JSON to `leafhill-persistent-memory hook-handler`
- Binary reads the hook JSON, extracts `last_assistant_message`, `session_id`
- Checks `stop_hook_active` to prevent infinite loops — exits 0 if true
- Stores as `entry_type = "raw_assistant"`

All hooks use the same session_id derivation logic in the binary.

## Claude Summarization Layer

CLAUDE.md instruction tells Claude to:

1. After each exchange, call `log_conversation` MCP tool with:
   - `session_id`: the session_id injected by SessionStart hook
   - `role`: `"summary"`
   - `content`: concise summary covering: what was asked, actions taken, decisions made, outcome
   - `project`: from cwd
   - `entry_type`: `"summary"`

2. Optional `raw_id` parameter to link summary to raw entry (Claude may not always know this).

## Query & Retrieval Changes

### Modified MCP tools:
- `search_conversations`: add optional `entry_type` filter. Defaults to searching all.
- `list_conversations`: add optional `entry_type` filter.
- `log_conversation`: add optional `entry_type` and `raw_id` parameters.

### New MCP tool:
- `get_conversation_context`: given a `session_id`, returns all summaries in chronological order.

### New/modified CLI commands:
- `hook-handler` — reads hook JSON from stdin, stores raw entry
- `log context <session_id>` — list summaries for a session
- `log prune --older-than <days> [--type raw]` — cleanup old raw entries

## Storage & Performance

- No automatic deletion; manual prune command available
- SQLite handles tens of thousands of entries well for single-user workloads
- FTS triggers already handle insert/delete sync; no FTS schema change needed
- Summaries rank higher in searches naturally (distilled keywords)
- SessionStart hook re-injects session_id on context compaction (`source: "compact"`)

## Implementation Components

1. **db.rs** — Migration: add `entry_type` and `raw_id` columns. Update query methods with entry_type filter.
2. **cli.rs** — New subcommands: `hook-handler`, `log context`, `log prune`.
3. **mcp.rs** — Update `log_conversation`, `search_conversations`, `list_conversations`. Add `get_conversation_context`.
4. **~/.claude/settings.json** — Configure SessionStart, UserPromptSubmit, Stop hooks.
5. **CLAUDE.md** — Add instruction block for summary logging after every exchange.

No changes to: Cargo.toml dependencies, database path, MCP server protocol, existing memory tools.

## Decision Log

| # | Decision | Alternatives | Rationale |
|---|----------|-------------|-----------|
| 1 | Hybrid (hooks + Claude summaries) | Hooks only, Claude only | Safety net of raw capture + quality of Claude summaries |
| 2 | Claude summarizes via CLAUDE.md instruction | Local LLM, hook-based | No extra dependencies, Claude writes better summaries |
| 3 | Summary after every exchange | End of task only | More granular, easier to search |
| 4 | hook-handler subcommand | Shell scripts + jq | Cleaner, no jq dependency |
| 5 | Session ID = claude_id + datetime + project | Date only, auto-increment | Unique, human-readable, project-scoped |
| 6 | No auto-deletion, manual prune | TTL, auto-purge | User controls retention |
