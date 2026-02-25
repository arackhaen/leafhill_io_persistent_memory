# PreCompact Transcript Storage Design

**Date**: 2026-02-25
**Version**: 1.4.0
**Status**: Approved

## Understanding Summary

- **What**: Add a PreCompact hook handler that reads the full transcript JSONL file and stores each user/assistant message as a separate conversation entry in SQLite before auto-compact runs
- **Why**: Preserve complete session history that would otherwise be compressed/lost during context compaction
- **Who**: For leafhill-persistent-memory users who want full conversation continuity across compactions
- **Key constraints**: Blocking execution acceptable; store all messages fresh with `entry_type='pre_compact'`; per-message granularity for FTS searchability
- **Non-goals**: Not preventing compaction, not summarizing, not replacing existing UserPromptSubmit/Stop hooks

## Assumptions

1. Transcript JSONL is readable at `transcript_path` when PreCompact fires
2. Only `type: "user"` and `type: "assistant"` events are stored (skip `progress`, `file-history-snapshot`, etc.)
3. For assistant messages, store text and thinking blocks, skip tool_use blocks
4. Session ID derivation uses existing `derive_session_id()` logic
5. Performance: even large transcripts complete well within 600s timeout

## Design

### Hook Configuration

Add `PreCompact` to `~/.claude/settings.json`:

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

No matcher — fires on both `auto` and `manual` compaction triggers.

### Schema Migration

Add 6 new nullable columns to the `conversations` table:

| Column | Type | Description |
|--------|------|-------------|
| `model` | `TEXT` | Model identifier (e.g., `claude-opus-4-6`) |
| `input_tokens` | `INTEGER` | Input token count |
| `output_tokens` | `INTEGER` | Output token count |
| `cache_creation_tokens` | `INTEGER` | Cache creation input tokens |
| `cache_read_tokens` | `INTEGER` | Cache read input tokens |
| `message_timestamp` | `TEXT` | Original ISO 8601 timestamp from transcript |

Migration: `ALTER TABLE conversations ADD COLUMN ...` for each. Nullable so existing rows unaffected. Also update `create_tables_ddl()` in `rdbms_export.rs` for PostgreSQL export.

### Hook Handler Logic (hook.rs)

Add `"PreCompact"` match arm to `handle_hook()`:

1. Extract `transcript_path` from hook input JSON
2. Read JSONL file line by line (streaming)
3. Filter: only `type == "user"` or `type == "assistant"` events
4. Extract per message:
   - `role`: from `message.role`
   - `content`: for user messages with string content, store as-is. For content arrays, extract text and thinking blocks, concatenate with newlines. Skip tool_use and tool_result blocks.
   - `model`: from `message.model` (assistant only)
   - Token counts: from `message.usage` (assistant only)
   - `message_timestamp`: from event `timestamp` field
5. Insert all messages in a single SQLite transaction with `entry_type='pre_compact'`
6. Output nothing to stdout

### DB Method (db.rs)

```rust
pub fn store_pre_compact_batch(
    &self,
    messages: &[PreCompactMessage],
) -> rusqlite::Result<()>
```

Where `PreCompactMessage` is a struct with all fields. Uses a single transaction for atomic batch insert.

### Content Extraction Rules

- **User messages (string content)**: store string directly
- **User messages (content array with tool_results)**: serialize array to JSON
- **Assistant messages**: extract `text` and `thinking` type blocks from content array, concatenate text values with newlines. Skip `tool_use` blocks.

### Error Handling

All error paths exit 0 (PreCompact cannot block compaction):

- Missing/unreadable transcript_path: log warning to stderr
- Malformed JSONL lines: skip line, log warning, continue
- Empty transcript: no-op
- DB write failure: log error to stderr

## Decision Log

| # | Decision | Alternatives | Reason |
|---|----------|-------------|--------|
| 1 | Full transcript storage | AI summary, both | Complete data preservation |
| 2 | SQLite only | File system, both | Single storage location |
| 3 | Per-message entries | Single entry, chunked | FTS searchability |
| 4 | Store fresh (entry_type='pre_compact') | Skip duplicates, replace | Simplicity, distinct from existing hooks |
| 5 | Blocking execution | Async | Guarantees save before compaction |
| 6 | Extend existing hook handler | Separate binary, MCP tool | Reuses existing pattern |
| 7 | No matcher (auto + manual) | Auto only | Complete coverage |
| 8 | New metadata columns | JSON envelope, raw JSONL | Clean separation, proper typing |
| 9 | 6 metadata columns | 4 columns | Full token accounting including cache |
| 10 | Text + thinking, skip tool_use | Text only, full array | Preserves reasoning, clean FTS |

## Implementation Plan

1. **Schema migration**: Add 6 columns to conversations table in `db.rs` init, update rdbms_export.rs DDL
2. **DB method**: Add `store_pre_compact_batch()` to `db.rs`
3. **Hook handler**: Add PreCompact match arm to `hook.rs` with JSONL parsing
4. **Version bump**: Update to 1.4.0, update docs
