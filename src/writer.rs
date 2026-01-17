use anyhow::Result;
use chrono::Utc;
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};

use crate::context::FabricContext;
use crate::event::{Event, Operation};
use crate::id::generate_id;

/// Write an event to the current day's event file
pub fn write_event(ctx: &FabricContext, event: &Event) -> Result<()> {
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let event_file = ctx.events_dir.join(format!("{}.jsonl", today));

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&event_file)?;
    let mut writer = BufWriter::new(file);

    let json = serde_json::to_string(event)?;
    writeln!(writer, "{}", json)?;
    writer.flush()?;

    Ok(())
}

/// Create a new task and return its ID
pub fn create_task(
    ctx: &FabricContext,
    title: &str,
    description: Option<&str>,
    priority: Option<&str>,
    assignee: Option<&str>,
    tags: Vec<String>,
    by: &str,
    branch: &str,
) -> Result<String> {
    let id = generate_id();

    let mut d = serde_json::json!({
        "title": title,
    });

    if let Some(desc) = description {
        d["description"] = serde_json::Value::String(desc.to_string());
    }
    if let Some(p) = priority {
        d["priority"] = serde_json::Value::String(p.to_string());
    }
    if let Some(a) = assignee {
        d["assignee"] = serde_json::Value::String(a.to_string());
    }
    if !tags.is_empty() {
        d["tags"] =
            serde_json::Value::Array(tags.into_iter().map(serde_json::Value::String).collect());
    }

    let event = Event {
        v: 1,
        op: Operation::Create,
        id: id.clone(),
        ts: Utc::now(),
        by: by.to_string(),
        branch: branch.to_string(),
        d,
    };

    write_event(ctx, &event)?;

    Ok(id)
}

/// Get the current git branch
pub fn get_current_branch() -> Result<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Ok("main".to_string())
    }
}

/// Update a task's fields
pub fn update_task(
    ctx: &FabricContext,
    id: &str,
    title: Option<&str>,
    description: Option<&str>,
    priority: Option<&str>,
    by: &str,
    branch: &str,
) -> Result<()> {
    let mut d = serde_json::Map::new();

    if let Some(t) = title {
        d.insert("title".to_string(), serde_json::Value::String(t.to_string()));
    }
    if let Some(desc) = description {
        d.insert("description".to_string(), serde_json::Value::String(desc.to_string()));
    }
    if let Some(p) = priority {
        d.insert("priority".to_string(), serde_json::Value::String(p.to_string()));
    }

    if d.is_empty() {
        return Err(anyhow::anyhow!("No fields to update"));
    }

    let event = Event {
        v: 1,
        op: Operation::Update,
        id: id.to_string(),
        ts: Utc::now(),
        by: by.to_string(),
        branch: branch.to_string(),
        d: serde_json::Value::Object(d),
    };

    write_event(ctx, &event)
}

/// Complete a task
pub fn complete_task(
    ctx: &FabricContext,
    id: &str,
    resolution: Option<&str>,
    by: &str,
    branch: &str,
) -> Result<()> {
    let event = Event {
        v: 1,
        op: Operation::Complete,
        id: id.to_string(),
        ts: Utc::now(),
        by: by.to_string(),
        branch: branch.to_string(),
        d: serde_json::json!({
            "resolution": resolution.unwrap_or("done")
        }),
    };

    write_event(ctx, &event)
}

/// Reopen a completed task
pub fn reopen_task(ctx: &FabricContext, id: &str, by: &str, branch: &str) -> Result<()> {
    let event = Event {
        v: 1,
        op: Operation::Reopen,
        id: id.to_string(),
        ts: Utc::now(),
        by: by.to_string(),
        branch: branch.to_string(),
        d: serde_json::json!({}),
    };

    write_event(ctx, &event)
}

/// Get the current user (from git config or environment)
pub fn get_current_user() -> Result<String> {
    // Try git config first
    let output = std::process::Command::new("git")
        .args(["config", "user.name"])
        .output()?;

    if output.status.success() {
        let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !name.is_empty() {
            return Ok(format!("@{}", name.to_lowercase().replace(' ', "-")));
        }
    }

    // Fall back to USER environment variable
    if let Ok(user) = std::env::var("USER") {
        return Ok(format!("@{}", user));
    }

    Ok("@unknown".to_string())
}
