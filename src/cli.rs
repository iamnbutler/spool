use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};

use crate::archive::collect_all_events;
use crate::context::FabricContext;
use crate::state::{load_or_materialize_state, Task, TaskStatus};

#[derive(Parser)]
#[command(name = "fabric")]
#[command(about = "Git-native task management system")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize .fabric/ directory structure
    Init,
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
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputFormat {
    Table,
    Json,
    Ids,
}

impl OutputFormat {
    pub fn from_str(s: &str) -> Self {
        match s {
            "json" => OutputFormat::Json,
            "ids" => OutputFormat::Ids,
            _ => OutputFormat::Table,
        }
    }
}

pub fn list_tasks(
    ctx: &FabricContext,
    status_filter: Option<&str>,
    assignee: Option<&str>,
    tag: Option<&str>,
    priority: Option<&str>,
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

            status_match && assignee_match && tag_match && priority_match
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

            println!(
                "{:<15} {:<10} {:<12} {}",
                "ID", "PRIORITY", "ASSIGNEE", "TITLE"
            );
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

pub fn show_task(ctx: &FabricContext, id: &str, show_events: bool) -> Result<()> {
    let state = load_or_materialize_state(ctx)?;

    let task = state
        .tasks
        .get(id)
        .ok_or_else(|| anyhow!("Task not found: {}", id))?;

    println!("ID:       {}", task.id);
    println!("Title:    {}", task.title);
    println!("Status:   {:?}", task.status);
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
