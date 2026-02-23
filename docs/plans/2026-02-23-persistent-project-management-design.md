# Persistent Project Management with Semantic Connections

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement the plan created from this design.

**Goal:** Add persistent task management to leafhill-persistent-memory with semantic links connecting tasks, conversations, and memories into a navigable knowledge graph.

**Approach:** Single database, new tables (Approach A). Tasks, dependencies, and links live alongside existing memories and conversations tables.

---

## 1. Data Model

### `tasks` table

| Column | Type | Notes |
|--------|------|-------|
| id | INTEGER PK | Auto-increment |
| project | TEXT NOT NULL | Project scope (e.g. "myapp") |
| subject | TEXT NOT NULL | Short title, imperative form |
| description | TEXT | Detailed requirements, context |
| status | TEXT NOT NULL DEFAULT 'pending' | `pending`, `in_progress`, `completed`, `blocked`, `deleted` |
| priority | TEXT DEFAULT 'medium' | `low`, `medium`, `high` |
| task_type | TEXT DEFAULT 'claude' | `claude`, `human`, `hybrid` |
| parent_id | INTEGER | FK to tasks.id for subtask hierarchy |
| due_date | TEXT | ISO date string (nullable) |
| created_by | TEXT | Session ID (Claude) or name/email (human) |
| assignee | TEXT | Who does the work |
| owner | TEXT | Who owns/approves |
| session_id | TEXT | Claude session that created/last touched |
| created_at | TEXT NOT NULL DEFAULT datetime('now') | |
| updated_at | TEXT NOT NULL DEFAULT datetime('now') | |

FTS5 virtual table `tasks_fts` on `subject` and `description`.

### `task_deps` table

| Column | Type | Notes |
|--------|------|-------|
| blocker_id | INTEGER NOT NULL | Task that must complete first |
| blocked_id | INTEGER NOT NULL | Task that's waiting |
| PRIMARY KEY | (blocker_id, blocked_id) | Prevents duplicates |

### `links` table (universal entity connector)

| Column | Type | Notes |
|--------|------|-------|
| id | INTEGER PK | Auto-increment |
| source_type | TEXT NOT NULL | `task`, `memory`, `conversation` |
| source_id | INTEGER NOT NULL | Row ID in source table |
| target_type | TEXT NOT NULL | `task`, `memory`, `conversation` |
| target_id | INTEGER NOT NULL | Row ID in target table |
| relation | TEXT | `discusses`, `relates_to`, `caused_by`, `resolves`, `requires_input`, etc. |
| created_at | TEXT NOT NULL DEFAULT datetime('now') | |
| UNIQUE | (source_type, source_id, target_type, target_id) | No duplicate links |

Links are bidirectional by convention (query both directions with one row).

---

## 2. MCP Tools

### Task tools

| Tool | Parameters | Description |
|------|-----------|-------------|
| `create_task` | project, subject, description?, priority?, task_type?, parent_id?, due_date?, created_by?, assignee?, owner?, session_id? | Create a task. Returns created task. |
| `update_task` | task_id, subject?, description?, status?, priority?, task_type?, assignee?, owner?, due_date?, session_id? | Update any field(s). |
| `get_task` | task_id | Get full task with deps and linked entities. |
| `list_tasks` | project?, status?, assignee?, task_type?, priority?, limit? | List tasks with filters. |
| `search_tasks` | query, project?, status?, limit? | Full-text search on subject + description. |
| `delete_task` | task_id | Soft-delete (status=deleted). |

### Dependency tools

| Tool | Parameters | Description |
|------|-----------|-------------|
| `add_task_dep` | blocker_id, blocked_id | Add blocker→blocked relationship. |
| `remove_task_dep` | blocker_id, blocked_id | Remove dependency. |

### Link tools

| Tool | Parameters | Description |
|------|-----------|-------------|
| `create_link` | source_type, source_id, target_type, target_id, relation? | Link any two entities. |
| `get_links` | entity_type, entity_id | Get all links for an entity. |
| `delete_link` | link_id | Remove a link. |
| `search_linked` | entity_type, entity_id, target_type? | Find all linked entities, optionally filtered by target type. |

---

## 3. CLI Commands

```
# Task management
leafhill-persistent-memory task create <project> <subject> [--description] [--priority] [--type] [--assignee] [--owner] [--due] [--parent]
leafhill-persistent-memory task list [--project] [--status] [--assignee] [--type] [--priority]
leafhill-persistent-memory task get <id>
leafhill-persistent-memory task update <id> [--status] [--subject] [--assignee] [--priority] [--due] ...
leafhill-persistent-memory task search <query> [--project] [--status]
leafhill-persistent-memory task delete <id>

# Dependencies
leafhill-persistent-memory task deps <id>
leafhill-persistent-memory task add-dep <blocker> <blocked>
leafhill-persistent-memory task remove-dep <blocker> <blocked>

# Links
leafhill-persistent-memory link create <source_type> <source_id> <target_type> <target_id> [--relation]
leafhill-persistent-memory link list <entity_type> <entity_id>
leafhill-persistent-memory link delete <link_id>
```

---

## 4. Automatic Link Creation

Driven by CLAUDE.md instructions (same pattern as conversation logging):

- **Task creation:** Claude searches conversations/memories for related content and creates links.
- **Conversation logging:** If a summary mentions an active task, Claude creates a `discusses` link.
- **Memory storage:** If a new memory relates to an active task, Claude creates a `relates_to` link.

No hooks needed — Claude follows CLAUDE.md guidance as part of normal workflow.

---

## 5. Benefits

1. **Cross-session continuity** — Tasks persist. Start in session 1, resume in session 5 with full context via links.
2. **Semantic search** — Query across tasks, conversations, and memories simultaneously.
3. **Human-Claude collaboration** — Tasks for humans (approve, review, answer, ideate) alongside Claude tasks.
4. **Knowledge graph** — Links build a navigable web of project knowledge over time.
5. **Reduced token usage** — Query linked context instead of re-reading files.
6. **Project overview** — `task list --project=X` shows full project state across all sessions.

---

## 6. Design Decisions

- **Single database** — All entities in one SQLite file for JOINable queries.
- **Explicit links over tags** — Queryable, directional relationships with relation labels.
- **Soft delete** — Tasks are never physically deleted, just marked `status=deleted`.
- **Bidirectional links** — One row per link, query both `source→target` and `target→source`.
- **Instruction-driven linking** — CLAUDE.md tells Claude when to create links. No automated hooks.
- **task_type field** — Distinguishes Claude work, human work, and collaborative tasks.
