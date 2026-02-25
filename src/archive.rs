use crate::db::{ConversationEntry, Database, Link, Memory, Task};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

const SCHEMA_VERSION: &str = "1.0";

#[derive(Debug, Serialize, Deserialize)]
pub struct ArchiveEnvelope {
    pub schema_version: String,
    pub created_at: String,
    pub source_db: String,
    pub entity_types: Vec<String>,
    pub filters: ArchiveFilters,
    pub counts: ArchiveCounts,
    pub data: ArchiveData,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ArchiveFilters {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub older_than_days: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ArchiveCounts {
    #[serde(skip_serializing_if = "is_zero")]
    pub memories: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub conversations: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub tasks: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub task_deps: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub links: usize,
}

fn is_zero(v: &usize) -> bool {
    *v == 0
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ArchiveData {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub memories: Vec<Memory>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub conversations: Vec<ConversationEntry>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<Task>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub task_deps: Vec<(i64, i64)>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<Link>,
}

pub fn run_archive_create(
    db: &Database,
    db_path: &str,
    output: &Path,
    entity_type: &str,
    older_than_days: Option<i64>,
    project: Option<&str>,
    category: Option<&str>,
    keep: bool,
    force: bool,
) -> Result<(), String> {
    if output.exists() && !force {
        return Err(format!(
            "Output file already exists: {}. Use --force to overwrite.",
            output.display()
        ));
    }

    let mut data = ArchiveData::default();
    let mut entity_types = Vec::new();
    let mut all_link_ids: HashSet<i64> = HashSet::new();

    let archive_memories = entity_type == "memories" || entity_type == "all";
    let archive_conversations = entity_type == "conversations" || entity_type == "all";
    let archive_tasks = entity_type == "tasks" || entity_type == "all";

    // Collect entities
    if archive_memories {
        data.memories = db.query_memories_for_archive(category, older_than_days)
            .map_err(|e| format!("Failed to query memories: {}", e))?;
        if !data.memories.is_empty() {
            entity_types.push("memories".to_string());
            // Cascade: collect links for these memories
            let mem_ids: Vec<i64> = data.memories.iter().map(|m| m.id).collect();
            let links = db.get_links_for_entity_ids("memory", &mem_ids)
                .map_err(|e| format!("Failed to query links for memories: {}", e))?;
            for link in &links {
                all_link_ids.insert(link.id);
            }
            data.links.extend(links);
        }
    }

    if archive_conversations {
        data.conversations = db.query_conversations_for_archive(project, older_than_days)
            .map_err(|e| format!("Failed to query conversations: {}", e))?;
        if !data.conversations.is_empty() {
            entity_types.push("conversations".to_string());
            // Cascade: collect links for these conversations
            let conv_ids: Vec<i64> = data.conversations.iter().map(|c| c.id).collect();
            let links = db.get_links_for_entity_ids("conversation", &conv_ids)
                .map_err(|e| format!("Failed to query links for conversations: {}", e))?;
            for link in &links {
                all_link_ids.insert(link.id);
            }
            data.links.extend(links);
        }
    }

    if archive_tasks {
        let mut tasks = db.query_tasks_for_archive(project, older_than_days)
            .map_err(|e| format!("Failed to query tasks: {}", e))?;

        if !tasks.is_empty() {
            entity_types.push("tasks".to_string());

            // Cascade: collect subtasks recursively
            let root_ids: Vec<i64> = tasks.iter().map(|t| t.id).collect();
            let subtask_ids = db.get_subtask_ids_recursive(&root_ids)
                .map_err(|e| format!("Failed to get subtasks: {}", e))?;

            // Fetch full subtask records (that aren't already in our list)
            let existing_ids: HashSet<i64> = root_ids.iter().copied().collect();
            for sid in &subtask_ids {
                if !existing_ids.contains(sid) {
                    if let Ok(task) = db.get_task(*sid) {
                        tasks.push(task);
                    }
                }
            }

            let all_task_ids: Vec<i64> = tasks.iter().map(|t| t.id).collect();

            // Cascade: collect task_deps
            data.task_deps = db.get_task_deps_for_task_ids(&all_task_ids)
                .map_err(|e| format!("Failed to query task deps: {}", e))?;

            // Cascade: collect links for these tasks
            let links = db.get_links_for_entity_ids("task", &all_task_ids)
                .map_err(|e| format!("Failed to query links for tasks: {}", e))?;
            for link in &links {
                all_link_ids.insert(link.id);
            }
            data.links.extend(links);

            data.tasks = tasks;
        }
    }

    // Deduplicate links (may have been collected from multiple entity types)
    let mut seen_link_ids: HashSet<i64> = HashSet::new();
    data.links.retain(|l| seen_link_ids.insert(l.id));

    // Check if anything was collected
    let total = data.memories.len() + data.conversations.len() + data.tasks.len();
    if total == 0 {
        println!("No entities match the given filters. No archive file created.");
        return Ok(());
    }

    let envelope = ArchiveEnvelope {
        schema_version: SCHEMA_VERSION.to_string(),
        created_at: Utc::now().to_rfc3339(),
        source_db: db_path.to_string(),
        entity_types,
        filters: ArchiveFilters {
            older_than_days,
            project: project.map(|s| s.to_string()),
            category: category.map(|s| s.to_string()),
        },
        counts: ArchiveCounts {
            memories: data.memories.len(),
            conversations: data.conversations.len(),
            tasks: data.tasks.len(),
            task_deps: data.task_deps.len(),
            links: data.links.len(),
        },
        data,
    };

    // Write atomically: temp file then rename
    if let Some(parent) = output.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory {}: {}", parent.display(), e))?;
        }
    }

    let tmp_path = output.with_extension("tmp");
    let json = serde_json::to_string_pretty(&envelope)
        .map_err(|e| format!("Failed to serialize archive: {}", e))?;
    std::fs::write(&tmp_path, &json)
        .map_err(|e| format!("Failed to write temp file: {}", e))?;
    std::fs::rename(&tmp_path, output)
        .map_err(|e| format!("Failed to rename temp file to output: {}", e))?;

    // Delete from database (unless --keep)
    if !keep {
        // Deletion order: links → task_deps → tasks → conversations → memories
        let link_ids: Vec<i64> = envelope.data.links.iter().map(|l| l.id).collect();
        if !link_ids.is_empty() {
            db.delete_links_by_ids(&link_ids)
                .map_err(|e| format!("Failed to delete archived links: {}", e))?;
        }

        let task_ids: Vec<i64> = envelope.data.tasks.iter().map(|t| t.id).collect();
        if !task_ids.is_empty() {
            db.delete_task_deps_for_task_ids(&task_ids)
                .map_err(|e| format!("Failed to delete archived task deps: {}", e))?;
            db.delete_tasks_by_ids(&task_ids)
                .map_err(|e| format!("Failed to delete archived tasks: {}", e))?;
        }

        let conv_ids: Vec<i64> = envelope.data.conversations.iter().map(|c| c.id).collect();
        if !conv_ids.is_empty() {
            db.delete_conversations_by_ids(&conv_ids)
                .map_err(|e| format!("Failed to delete archived conversations: {}", e))?;
        }

        let mem_ids: Vec<i64> = envelope.data.memories.iter().map(|m| m.id).collect();
        if !mem_ids.is_empty() {
            db.delete_memories_by_ids(&mem_ids)
                .map_err(|e| format!("Failed to delete archived memories: {}", e))?;
        }
    }

    let file_size = std::fs::metadata(output)
        .map(|m| m.len())
        .unwrap_or(0);
    let size_display = if file_size >= 1_048_576 {
        format!("{:.1} MB", file_size as f64 / 1_048_576.0)
    } else if file_size >= 1024 {
        format!("{:.1} KB", file_size as f64 / 1024.0)
    } else {
        format!("{} bytes", file_size)
    };

    println!("Archive created: {}", output.display());
    println!("  Size: {}", size_display);
    println!("  Entities archived:");
    if envelope.counts.memories > 0 {
        println!("    memories: {}", envelope.counts.memories);
    }
    if envelope.counts.conversations > 0 {
        println!("    conversations: {}", envelope.counts.conversations);
    }
    if envelope.counts.tasks > 0 {
        println!("    tasks: {}", envelope.counts.tasks);
    }
    if envelope.counts.task_deps > 0 {
        println!("    task_deps: {}", envelope.counts.task_deps);
    }
    if envelope.counts.links > 0 {
        println!("    links: {}", envelope.counts.links);
    }
    if keep {
        println!("  Source data retained (--keep).");
    } else {
        println!("  Source data removed from database.");
    }

    Ok(())
}

pub fn run_archive_restore(db: &Database, input: &Path) -> Result<(), String> {
    let json = std::fs::read_to_string(input)
        .map_err(|e| format!("Failed to read archive file: {}", e))?;

    let envelope: ArchiveEnvelope = serde_json::from_str(&json)
        .map_err(|e| format!("Failed to parse archive file: {}", e))?;

    if envelope.schema_version != SCHEMA_VERSION {
        return Err(format!(
            "Incompatible archive schema version: found '{}', expected '{}'",
            envelope.schema_version, SCHEMA_VERSION
        ));
    }

    let mut restored = ArchiveCounts::default();
    let mut skipped = ArchiveCounts::default();

    // Restore order: memories → conversations → tasks → task_deps → links
    for mem in &envelope.data.memories {
        match db.restore_memory(mem) {
            Ok(true) => restored.memories += 1,
            Ok(false) => skipped.memories += 1,
            Err(e) => return Err(format!("Failed to restore memory {}: {}", mem.id, e)),
        }
    }

    for entry in &envelope.data.conversations {
        match db.restore_conversation(entry) {
            Ok(true) => restored.conversations += 1,
            Ok(false) => skipped.conversations += 1,
            Err(e) => return Err(format!("Failed to restore conversation {}: {}", entry.id, e)),
        }
    }

    // Restore tasks in parent-first order (sort by parent_id nulls first)
    let mut tasks_sorted: Vec<&Task> = envelope.data.tasks.iter().collect();
    tasks_sorted.sort_by_key(|t| t.parent_id.unwrap_or(0));

    for task in &tasks_sorted {
        match db.restore_task(task) {
            Ok(true) => restored.tasks += 1,
            Ok(false) => skipped.tasks += 1,
            Err(e) => return Err(format!("Failed to restore task {}: {}", task.id, e)),
        }
    }

    for (blocker_id, blocked_id) in &envelope.data.task_deps {
        match db.restore_task_dep(*blocker_id, *blocked_id) {
            Ok(true) => restored.task_deps += 1,
            Ok(false) => skipped.task_deps += 1,
            Err(e) => return Err(format!("Failed to restore task dep ({}, {}): {}", blocker_id, blocked_id, e)),
        }
    }

    for link in &envelope.data.links {
        match db.restore_link(link) {
            Ok(true) => restored.links += 1,
            Ok(false) => skipped.links += 1,
            Err(e) => return Err(format!("Failed to restore link {}: {}", link.id, e)),
        }
    }

    println!("Archive restored from: {}", input.display());
    println!("  Restored / Skipped:");
    let total_restored = restored.memories + restored.conversations + restored.tasks + restored.task_deps + restored.links;
    let total_skipped = skipped.memories + skipped.conversations + skipped.tasks + skipped.task_deps + skipped.links;

    if envelope.counts.memories > 0 {
        println!("    memories: {} restored, {} skipped", restored.memories, skipped.memories);
    }
    if envelope.counts.conversations > 0 {
        println!("    conversations: {} restored, {} skipped", restored.conversations, skipped.conversations);
    }
    if envelope.counts.tasks > 0 {
        println!("    tasks: {} restored, {} skipped", restored.tasks, skipped.tasks);
    }
    if envelope.counts.task_deps > 0 {
        println!("    task_deps: {} restored, {} skipped", restored.task_deps, skipped.task_deps);
    }
    if envelope.counts.links > 0 {
        println!("    links: {} restored, {} skipped", restored.links, skipped.links);
    }
    println!("  Total: {} restored, {} skipped", total_restored, total_skipped);

    Ok(())
}
