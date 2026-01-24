use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};

use crate::archive::collect_all_events;
use crate::context::SpoolContext;
use crate::state::{load_or_materialize_state, Task, TaskStatus};
use crate::writer::{
    assign_task as write_assign, complete_task as write_complete, create_task as write_create,
    get_current_branch, get_current_user, reopen_task as write_reopen, set_stream as write_stream,
    update_task as write_update, CreateTaskParams,
};

#[derive(Parser)]
#[command(name = "spool")]
#[command(about = "Git-native task management system")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize .spool/ directory structure
    Init,
    /// Create a new task
    Add {
        /// Task title
        title: String,
        /// Task description
        #[arg(short, long)]
        description: Option<String>,
        /// Priority (p0, p1, p2, p3)
        #[arg(short, long)]
        priority: Option<String>,
        /// Assignee (@username)
        #[arg(short, long)]
        assignee: Option<String>,
        /// Tags (can be used multiple times)
        #[arg(short, long)]
        tag: Vec<String>,
        /// Stream to add the task to
        #[arg(long)]
        stream: Option<String>,
    },
    /// List tasks with optional filtering
    List {
        /// Status filter: open, complete, or all (default: open)
        #[arg(short, long, default_value = "open")]
        status: String,
        /// Filter by assignee
        #[arg(short, long)]
        assignee: Option<String>,
        /// Filter by tag
        #[arg(short, long)]
        tag: Option<String>,
        /// Filter by priority
        #[arg(short, long)]
        priority: Option<String>,
        /// Filter by stream
        #[arg(long)]
        stream: Option<String>,
        /// Output format: table, json, or ids
        #[arg(short, long, default_value = "table")]
        format: String,
    },
    /// Show details of a specific task
    Show {
        /// Task ID to show
        id: String,
        /// Show raw event history
        #[arg(long)]
        events: bool,
    },
    /// Rebuild .index.json and .state.json from events
    Rebuild,
    /// Archive completed tasks older than N days
    Archive {
        /// Days after completion to archive (default: 30)
        #[arg(short, long, default_value = "30")]
        days: u32,
        /// Show what would be archived without doing it
        #[arg(long)]
        dry_run: bool,
    },
    /// Validate event files for correctness
    Validate {
        /// Fail on warnings too
        #[arg(long)]
        strict: bool,
    },
    /// Start interactive shell mode
    Shell,
    /// Mark a task as complete
    Complete {
        /// Task ID to complete
        id: String,
        /// Resolution: done, wontfix, duplicate, obsolete
        #[arg(short, long, default_value = "done")]
        resolution: String,
    },
    /// Reopen a completed task
    Reopen {
        /// Task ID to reopen
        id: String,
    },
    /// Update a task's fields
    Update {
        /// Task ID to update
        id: String,
        /// New title
        #[arg(short, long)]
        title: Option<String>,
        /// New description
        #[arg(short, long)]
        description: Option<String>,
        /// New priority
        #[arg(short, long)]
        priority: Option<String>,
        /// Move to stream (use "" to remove from stream)
        #[arg(long)]
        stream: Option<String>,
    },
    /// Assign a task to a user
    Assign {
        /// Task ID to assign
        id: String,
        /// Assignee (@username)
        assignee: String,
    },
    /// Assign a task to yourself
    Claim {
        /// Task ID to claim
        id: String,
    },
    /// Manage streams (workstreams/projects)
    Stream {
        #[command(subcommand)]
        command: StreamCommands,
    },
    /// Unassign a task
    Free {
        /// Task ID to free
        id: String,
    },
}

#[derive(Subcommand)]
pub enum StreamCommands {
    /// List all streams
    List {
        /// Output format: table, json, or ids
        #[arg(short, long, default_value = "table")]
        format: String,
    },
    /// Show details about a stream
    Show {
        /// Stream name
        name: String,
        /// Output format: table or json
        #[arg(short, long, default_value = "table")]
        format: String,
    },
    /// Add a task to a stream
    Add {
        /// Task ID to add to stream
        id: String,
        /// Stream name
        name: String,
    },
    /// Remove a task from its stream
    Remove {
        /// Task ID to remove from stream
        id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputFormat {
    Table,
    Json,
    Ids,
}

impl OutputFormat {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "json" => OutputFormat::Json,
            "ids" => OutputFormat::Ids,
            _ => OutputFormat::Table,
        }
    }
}

pub fn list_tasks(
    ctx: &SpoolContext,
    status_filter: Option<&str>,
    assignee: Option<&str>,
    tag: Option<&str>,
    priority: Option<&str>,
    stream: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let state = load_or_materialize_state(ctx)?;

    let mut tasks: Vec<&Task> = state
        .tasks
        .values()
        .filter(|t| {
            // Status filter
            let status_match = match status_filter {
                Some("open") => t.status == TaskStatus::Open,
                Some("complete") => t.status == TaskStatus::Complete,
                Some("all") | None => true,
                _ => true,
            };

            // Assignee filter
            let assignee_match = assignee
                .map(|a| t.assignee.as_deref() == Some(a))
                .unwrap_or(true);

            // Tag filter
            let tag_match = tag.map(|tg| t.tags.iter().any(|t| t == tg)).unwrap_or(true);

            // Priority filter
            let priority_match = priority
                .map(|p| t.priority.as_deref() == Some(p))
                .unwrap_or(true);

            // Stream filter
            let stream_match = stream
                .map(|s| t.stream.as_deref() == Some(s))
                .unwrap_or(true);

            status_match && assignee_match && tag_match && priority_match && stream_match
        })
        .collect();

    // Sort by created date
    tasks.sort_by_key(|t| t.created);

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&tasks)?;
            println!("{}", json);
        }
        OutputFormat::Ids => {
            for task in &tasks {
                println!("{}", task.id);
            }
        }
        OutputFormat::Table => {
            if tasks.is_empty() {
                println!("No tasks found.");
                return Ok(());
            }

            println!("{:<15} {:<10} {:<12} TITLE", "ID", "PRIORITY", "ASSIGNEE");
            for task in &tasks {
                let priority = task.priority.as_deref().unwrap_or("-");
                let assignee = task.assignee.as_deref().unwrap_or("-");
                let title = if task.title.len() > 50 {
                    format!("{}...", &task.title[..47])
                } else {
                    task.title.clone()
                };
                println!(
                    "{:<15} {:<10} {:<12} {}",
                    task.id, priority, assignee, title
                );
            }
        }
    }

    Ok(())
}

pub fn show_task(ctx: &SpoolContext, id: &str, show_events: bool) -> Result<()> {
    let state = load_or_materialize_state(ctx)?;

    let task = state
        .tasks
        .get(id)
        .ok_or_else(|| anyhow!("Task not found: {}", id))?;

    println!("ID:       {}", task.id);
    println!("Title:    {}", task.title);
    println!("Status:   {:?}", task.status);
    if let Some(s) = &task.stream {
        println!("Stream:   {}", s);
    }
    if let Some(p) = &task.priority {
        println!("Priority: {}", p);
    }
    if let Some(a) = &task.assignee {
        println!("Assignee: {}", a);
    }
    if !task.tags.is_empty() {
        println!("Tags:     {}", task.tags.join(", "));
    }
    if let Some(d) = &task.description {
        println!("Description:\n  {}", d.replace('\n', "\n  "));
    }
    println!(
        "Created:  {} by {} on {}",
        task.created, task.created_by, task.created_branch
    );
    println!("Updated:  {}", task.updated);
    if let Some(c) = task.completed {
        println!(
            "Completed: {} ({})",
            c,
            task.resolution.as_deref().unwrap_or("done")
        );
    }
    if let Some(a) = &task.archived {
        println!("Archived: {}", a);
    }
    if let Some(p) = &task.parent {
        println!("Parent:   {}", p);
    }
    if !task.blocks.is_empty() {
        println!("Blocks:   {}", task.blocks.join(", "));
    }
    if !task.blocked_by.is_empty() {
        println!("Blocked by: {}", task.blocked_by.join(", "));
    }

    if !task.comments.is_empty() {
        println!("\nComments:");
        for comment in &task.comments {
            println!("  [{} - {}]", comment.ts, comment.by);
            println!("  {}", comment.body.replace('\n', "\n  "));
            if let Some(r) = &comment.r#ref {
                println!("  ref: {}", r);
            }
            println!();
        }
    }

    if show_events {
        println!("\nEvent History:");
        let all_events = collect_all_events(ctx)?;
        if let Some(events) = all_events.get(id) {
            for event in events {
                println!(
                    "  {} {} by {} on {}",
                    event.ts, event.op, event.by, event.branch
                );
            }
        }
    }

    Ok(())
}

pub fn complete_task(ctx: &SpoolContext, id: &str, resolution: Option<&str>) -> Result<()> {
    let state = load_or_materialize_state(ctx)?;

    // Verify task exists
    let task = state
        .tasks
        .get(id)
        .ok_or_else(|| anyhow!("Task not found: {}", id))?;

    // Check if already complete
    if task.status == TaskStatus::Complete {
        return Err(anyhow!("Task is already complete: {}", id));
    }

    let user = get_current_user()?;
    let branch = get_current_branch()?;

    write_complete(ctx, id, resolution, &user, &branch)?;
    println!("Completed task: {} ({})", id, resolution.unwrap_or("done"));

    Ok(())
}

pub fn reopen_task(ctx: &SpoolContext, id: &str) -> Result<()> {
    let state = load_or_materialize_state(ctx)?;

    // Verify task exists
    let task = state
        .tasks
        .get(id)
        .ok_or_else(|| anyhow!("Task not found: {}", id))?;

    // Check if already open
    if task.status == TaskStatus::Open {
        return Err(anyhow!("Task is already open: {}", id));
    }

    let user = get_current_user()?;
    let branch = get_current_branch()?;

    write_reopen(ctx, id, &user, &branch)?;
    println!("Reopened task: {}", id);

    Ok(())
}

pub fn update_task(
    ctx: &SpoolContext,
    id: &str,
    title: Option<&str>,
    description: Option<&str>,
    priority: Option<&str>,
    stream: Option<&str>,
) -> Result<()> {
    let state = load_or_materialize_state(ctx)?;

    // Verify task exists
    state
        .tasks
        .get(id)
        .ok_or_else(|| anyhow!("Task not found: {}", id))?;

    let user = get_current_user()?;
    let branch = get_current_branch()?;

    // Handle field updates
    if title.is_some() || description.is_some() || priority.is_some() {
        write_update(ctx, id, title, description, priority, &user, &branch)?;
    }

    // Handle stream change separately (different operation type)
    if let Some(s) = stream {
        let stream_value = if s.is_empty() { None } else { Some(s) };
        write_stream(ctx, id, stream_value, &user, &branch)?;
    }

    let mut updates = Vec::new();
    if title.is_some() {
        updates.push("title");
    }
    if description.is_some() {
        updates.push("description");
    }
    if priority.is_some() {
        updates.push("priority");
    }
    if stream.is_some() {
        updates.push("stream");
    }
    println!("Updated task {}: {}", id, updates.join(", "));

    Ok(())
}

pub fn add_task(
    ctx: &SpoolContext,
    title: &str,
    description: Option<&str>,
    priority: Option<&str>,
    assignee: Option<&str>,
    tags: Vec<String>,
    stream: Option<&str>,
) -> Result<()> {
    let user = get_current_user()?;
    let branch = get_current_branch()?;

    let id = write_create(
        ctx,
        CreateTaskParams {
            title,
            description,
            priority,
            assignee,
            tags,
            stream,
        },
        &user,
        &branch,
    )?;
    println!("Created task: {}", id);

    Ok(())
}

pub fn assign_task(ctx: &SpoolContext, id: &str, assignee: &str) -> Result<()> {
    let state = load_or_materialize_state(ctx)?;

    // Verify task exists
    state
        .tasks
        .get(id)
        .ok_or_else(|| anyhow!("Task not found: {}", id))?;

    let user = get_current_user()?;
    let branch = get_current_branch()?;

    write_assign(ctx, id, Some(assignee), &user, &branch)?;
    println!("Assigned task {} to {}", id, assignee);

    Ok(())
}

pub fn claim_task(ctx: &SpoolContext, id: &str) -> Result<()> {
    let state = load_or_materialize_state(ctx)?;

    // Verify task exists
    state
        .tasks
        .get(id)
        .ok_or_else(|| anyhow!("Task not found: {}", id))?;

    let user = get_current_user()?;
    let branch = get_current_branch()?;

    write_assign(ctx, id, Some(&user), &user, &branch)?;
    println!("Claimed task {} (assigned to {})", id, user);

    Ok(())
}

pub fn free_task(ctx: &SpoolContext, id: &str) -> Result<()> {
    let state = load_or_materialize_state(ctx)?;

    // Verify task exists
    state
        .tasks
        .get(id)
        .ok_or_else(|| anyhow!("Task not found: {}", id))?;

    let user = get_current_user()?;
    let branch = get_current_branch()?;

    write_assign(ctx, id, None, &user, &branch)?;
    println!("Freed task {} (unassigned)", id);

    Ok(())
}

pub fn stream_add(ctx: &SpoolContext, id: &str, stream: &str) -> Result<()> {
    let state = load_or_materialize_state(ctx)?;

    // Verify task exists
    state
        .tasks
        .get(id)
        .ok_or_else(|| anyhow!("Task not found: {}", id))?;

    let user = get_current_user()?;
    let branch = get_current_branch()?;

    write_stream(ctx, id, Some(stream), &user, &branch)?;
    println!("Added task {} to stream '{}'", id, stream);

    Ok(())
}

pub fn stream_remove(ctx: &SpoolContext, id: &str) -> Result<()> {
    let state = load_or_materialize_state(ctx)?;

    // Verify task exists
    state
        .tasks
        .get(id)
        .ok_or_else(|| anyhow!("Task not found: {}", id))?;

    let user = get_current_user()?;
    let branch = get_current_branch()?;

    write_stream(ctx, id, None, &user, &branch)?;
    println!("Removed task {} from stream", id);

    Ok(())
}

pub fn stream_list(ctx: &SpoolContext, format: OutputFormat) -> Result<()> {
    let state = load_or_materialize_state(ctx)?;

    // Collect all streams and their task counts
    let mut stream_map: std::collections::HashMap<String, Vec<&Task>> =
        std::collections::HashMap::new();

    for task in state.tasks.values() {
        if let Some(stream) = &task.stream {
            stream_map
                .entry(stream.clone())
                .or_default()
                .push(task);
        }
    }

    let mut streams: Vec<_> = stream_map.keys().collect();
    streams.sort();

    match format {
        OutputFormat::Json => {
            let result: Vec<_> = streams
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "name": s,
                        "task_count": stream_map[*s].len(),
                    })
                })
                .collect();
            let json = serde_json::to_string_pretty(&result)?;
            println!("{}", json);
        }
        OutputFormat::Ids => {
            for stream in &streams {
                println!("{}", stream);
            }
        }
        OutputFormat::Table => {
            if streams.is_empty() {
                println!("No streams found.");
                return Ok(());
            }

            println!("{:<30} TASKS", "STREAM");
            for stream in &streams {
                println!("{:<30} {}", stream, stream_map[*stream].len());
            }
        }
    }

    Ok(())
}

pub fn stream_show(ctx: &SpoolContext, name: &str, format: OutputFormat) -> Result<()> {
    let state = load_or_materialize_state(ctx)?;

    let mut tasks: Vec<&Task> = state
        .tasks
        .values()
        .filter(|t| t.stream.as_deref() == Some(name))
        .collect();

    tasks.sort_by_key(|t| t.created);

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&tasks)?;
            println!("{}", json);
        }
        OutputFormat::Ids => {
            for task in &tasks {
                println!("{}", task.id);
            }
        }
        OutputFormat::Table => {
            if tasks.is_empty() {
                println!("No tasks found in stream '{}'.", name);
                return Ok(());
            }

            println!("Stream: {}", name);
            println!("{:<15} {:<10} {:<12} TITLE", "ID", "PRIORITY", "ASSIGNEE");
            for task in &tasks {
                let priority = task.priority.as_deref().unwrap_or("-");
                let assignee = task.assignee.as_deref().unwrap_or("-");
                let title = if task.title.len() > 50 {
                    format!("{}...", &task.title[..47])
                } else {
                    task.title.clone()
                };
                println!(
                    "{:<15} {:<10} {:<12} {}",
                    task.id, priority, assignee, title
                );
            }
        }
    }

    Ok(())
}
