use anyhow::Result;
use chrono::Utc;
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, OpenOptions};
use std::io::{BufWriter, Write};

use crate::context::SpoolContext;
use crate::event::{Event, Operation};
use crate::state::{materialize, Task, TaskStatus};
use crate::writer::get_current_branch;

pub fn archive_tasks(ctx: &SpoolContext, days: u32, dry_run: bool) -> Result<Vec<String>> {
    let state = materialize(ctx)?;
    let cutoff = Utc::now() - chrono::Duration::days(days as i64);

    let mut to_archive: Vec<&Task> = state
        .tasks
        .values()
        .filter(|t| {
            t.status == TaskStatus::Complete
                && t.completed.is_some_and(|c| c < cutoff)
                && t.archived.is_none()
        })
        .collect();

    to_archive.sort_by_key(|t| t.completed);

    if to_archive.is_empty() {
        println!("No tasks to archive.");
        return Ok(Vec::new());
    }

    let archived_ids: Vec<String> = to_archive.iter().map(|t| t.id.clone()).collect();

    if dry_run {
        println!("Would archive {} tasks:", to_archive.len());
        for task in &to_archive {
            println!("  {} - {}", task.id, task.title);
        }
        return Ok(archived_ids);
    }

    // Group tasks by completion month
    let mut by_month: BTreeMap<String, Vec<&Task>> = BTreeMap::new();
    for task in &to_archive {
        if let Some(completed) = task.completed {
            let month = completed.format("%Y-%m").to_string();
            by_month.entry(month).or_default().push(task);
        }
    }

    // Create archive directory if needed
    fs::create_dir_all(&ctx.archive_dir)?;

    // Collect all events for archived tasks and write to monthly files
    let all_events = collect_all_events(ctx)?;

    for (month, tasks) in &by_month {
        let archive_file = ctx.archive_dir.join(format!("{}.jsonl", month));
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&archive_file)?;
        let mut writer = BufWriter::new(file);

        for task in tasks {
            if let Some(events) = all_events.get(&task.id) {
                for event in events {
                    let json = serde_json::to_string(event)?;
                    writeln!(writer, "{}", json)?;
                }
            }
        }
        writer.flush()?;
    }

    // Emit archive events to today's event file
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let event_file = ctx.events_dir.join(format!("{}.jsonl", today));
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&event_file)?;
    let mut writer = BufWriter::new(file);

    let branch = get_current_branch()?;

    for task in &to_archive {
        if let Some(completed) = task.completed {
            let month = completed.format("%Y-%m").to_string();
            let archive_event = Event {
                v: 1,
                op: Operation::Archive,
                id: task.id.clone(),
                ts: Utc::now(),
                by: "@spool".to_string(),
                branch: branch.clone(),
                d: serde_json::json!({ "ref": month }),
            };
            let json = serde_json::to_string(&archive_event)?;
            writeln!(writer, "{}", json)?;
        }
    }
    writer.flush()?;

    println!("Archived {} tasks.", to_archive.len());
    for (month, tasks) in &by_month {
        println!("  {} tasks to archive/{}.jsonl", tasks.len(), month);
    }

    Ok(archived_ids)
}

pub fn collect_all_events(ctx: &SpoolContext) -> Result<HashMap<String, Vec<Event>>> {
    let mut events_by_task: HashMap<String, Vec<Event>> = HashMap::new();

    for file in ctx.get_event_files()? {
        let events = ctx.parse_events_from_file(&file)?;
        for event in events {
            events_by_task
                .entry(event.id.clone())
                .or_default()
                .push(event);
        }
    }

    Ok(events_by_task)
}
