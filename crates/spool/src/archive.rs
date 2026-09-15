use anyhow::{anyhow, Result};
use chrono::Utc;
use std::collections::HashMap;

use crate::concurrency::FileLock;
use crate::context::SpoolContext;
use crate::event::{Event, Operation};
use crate::state::{materialize, TaskStatus};
use crate::store::{publish, read_events};
use crate::writer::get_current_branch;

pub fn archive_tasks(ctx: &SpoolContext, days: u32, dry_run: bool) -> Result<Vec<String>> {
    let ids = archive_tasks_quiet(ctx, days, dry_run)?;
    if ids.is_empty() {
        println!("No tasks to archive.");
    } else if dry_run {
        println!("Would archive {} tasks.", ids.len());
    } else {
        println!("Archived {} tasks.", ids.len());
    }
    Ok(ids)
}

pub fn archive_tasks_quiet(ctx: &SpoolContext, days: u32, dry_run: bool) -> Result<Vec<String>> {
    let _lock = FileLock::acquire(ctx)?;
    let state = materialize(ctx)?;
    let cutoff = Utc::now()
        .checked_sub_signed(chrono::Duration::days(days.into()))
        .ok_or_else(|| anyhow!("Archive age is out of range"))?;
    let mut tasks: Vec<_> = state
        .tasks
        .values()
        .filter(|task| {
            task.status == TaskStatus::Complete
                && task.completed.is_some_and(|date| date < cutoff)
                && task.archived.is_none()
        })
        .collect();
    tasks.sort_by_key(|task| (task.completed, &task.id));
    let ids = tasks.iter().map(|task| task.id.clone()).collect();
    if dry_run || tasks.is_empty() {
        return Ok(ids);
    }

    let events = read_events(ctx, false)?;
    let mut timestamp = read_events(ctx, true)?
        .last()
        .map(|event| event.ts)
        .unwrap_or_else(Utc::now)
        .max(Utc::now());
    let branch = get_current_branch()?;
    // Copy first, then mark archived. If interrupted, exact duplicates replay
    // once and a retry can finish without rewriting or deleting any history.
    for task in tasks {
        for event in events
            .iter()
            .filter(|event| event.id == task.id && !event.is_local())
        {
            publish(&ctx.archive_dir, event)?;
        }
        timestamp += chrono::Duration::nanoseconds(1);
        let marker = Event {
            v: 1,
            op: Operation::Archive,
            id: task.id.clone(),
            ts: timestamp,
            by: "@spool".into(),
            branch: branch.clone(),
            d: serde_json::json!({"ref": task.completed.unwrap().format("%Y-%m").to_string()}),
        };
        publish(&ctx.events_dir, &marker)?;
    }
    Ok(ids)
}

pub fn collect_all_events(ctx: &SpoolContext) -> Result<HashMap<String, Vec<Event>>> {
    let mut events_by_task: HashMap<String, Vec<Event>> = HashMap::new();
    for event in read_events(ctx, true)? {
        events_by_task
            .entry(event.id.clone())
            .or_default()
            .push(event);
    }
    Ok(events_by_task)
}
