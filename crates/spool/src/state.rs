use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::context::SpoolContext;
use crate::event::{Event, Operation};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Task {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub status: TaskStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<String>,
    pub created: DateTime<Utc>,
    pub created_by: String,
    pub created_branch: String,
    pub updated: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default)]
    pub blocks: Vec<String>,
    #[serde(default)]
    pub blocked_by: Vec<String>,
    #[serde(default)]
    pub comments: Vec<Comment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim: Option<Claim>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claim {
    pub agent: String,
    pub token: String,
    pub started_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub branch: String,
    pub worktree: String,
}

impl Claim {
    pub fn is_live(&self, now: DateTime<Utc>) -> bool {
        self.expires_at > now
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    #[default]
    Open,
    Complete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub ts: DateTime<Utc>,
    pub by: String,
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#ref: Option<String>,
}

/// A stream is a collection of tasks representing a project or workstream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stream {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub created: DateTime<Utc>,
    pub created_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Index {
    pub tasks: HashMap<String, TaskIndex>,
    pub rebuilt: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskIndex {
    pub status: TaskStatus,
    pub created: String,
    pub updated: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed: Option<String>,
    pub files: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct State {
    pub tasks: HashMap<String, Task>,
    #[serde(default)]
    pub streams: HashMap<String, Stream>,
    pub rebuilt: DateTime<Utc>,
}

pub fn materialize(ctx: &SpoolContext) -> Result<State> {
    materialize_events(crate::store::read_events(ctx, true)?)
}

pub(crate) fn materialize_events(events: Vec<Event>) -> Result<State> {
    let mut tasks: HashMap<String, Task> = HashMap::new();
    let mut streams: HashMap<String, Stream> = HashMap::new();
    for event in events {
        apply_event(&mut tasks, &mut streams, event)?;
    }

    // Normalize legacy one-sided relationships once per replay. Queue queries
    // can then inspect a task's own prerequisites instead of scanning the board.
    let edges: HashSet<(String, String)> = tasks
        .values()
        .flat_map(|task| {
            task.blocks
                .iter()
                .map(|target| (task.id.clone(), target.clone()))
                .chain(
                    task.blocked_by
                        .iter()
                        .map(|source| (source.clone(), task.id.clone())),
                )
        })
        .collect();
    for (source, target) in edges {
        if let Some(task) = tasks.get_mut(&source) {
            edit_relationship(task, "blocks", &target, true);
        }
        if let Some(task) = tasks.get_mut(&target) {
            edit_relationship(task, "blocked_by", &source, true);
        }
    }

    Ok(State {
        tasks,
        streams,
        rebuilt: Utc::now(),
    })
}

fn apply_event(
    tasks: &mut HashMap<String, Task>,
    streams: &mut HashMap<String, Stream>,
    event: Event,
) -> Result<()> {
    match event.op {
        Operation::Create => {
            let d = &event.d;
            let task = Task {
                id: event.id.clone(),
                title: d
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                description: d
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                status: TaskStatus::Open,
                priority: d.get("priority").and_then(|v| v.as_str()).map(String::from),
                tags: d
                    .get("tags")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                assignee: d.get("assignee").and_then(|v| v.as_str()).map(String::from),
                created: event.ts,
                created_by: event.by.clone(),
                created_branch: event.branch.clone(),
                updated: event.ts,
                completed: None,
                resolution: None,
                parent: d.get("parent").and_then(|v| v.as_str()).map(String::from),
                blocks: d
                    .get("blocks")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                blocked_by: d
                    .get("blocked_by")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                comments: Vec::new(),
                archived: None,
                stream: d.get("stream").and_then(|v| v.as_str()).map(String::from),
                claim: None,
            };
            tasks.insert(event.id, task);
        }
        Operation::Update => {
            if let Some(task) = tasks.get_mut(&event.id) {
                let d = &event.d;
                if let Some(title) = d.get("title").and_then(|v| v.as_str()) {
                    task.title = title.to_string();
                }
                if let Some(desc) = d.get("description").and_then(|v| v.as_str()) {
                    task.description = Some(desc.to_string());
                }
                if let Some(priority) = d.get("priority").and_then(|v| v.as_str()) {
                    task.priority = Some(priority.to_string());
                }
                if let Some(tags) = d.get("tags").and_then(|v| v.as_array()) {
                    task.tags = tags
                        .iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect();
                }
                // New writers can update fields and stream in one atomic event.
                if d.get("stream").is_some() {
                    task.stream = d.get("stream").and_then(|v| v.as_str()).map(String::from);
                }
                task.updated = event.ts;
            }
        }
        Operation::Claim | Operation::Renew => {
            let claim: Claim = serde_json::from_value(event.d["claim"].clone())?;
            if let Some(task) = tasks.get_mut(&event.id) {
                if task.status == TaskStatus::Open && task.archived.is_none() {
                    task.claim = Some(claim);
                }
            }
        }
        Operation::Handoff => {
            if let Some(task) = tasks.get_mut(&event.id) {
                task.claim = None;
                task.assignee = event.d.get("to").and_then(|v| v.as_str()).map(String::from);
                task.comments.push(Comment {
                    ts: event.ts,
                    by: event.by,
                    body: event.d["body"].as_str().unwrap_or_default().to_string(),
                    r#ref: event
                        .d
                        .get("ref")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                });
                task.updated = event.ts;
            }
        }
        Operation::Assign => {
            if let Some(task) = tasks.get_mut(&event.id) {
                task.assignee = event.d.get("to").and_then(|v| {
                    if v.is_null() {
                        None
                    } else {
                        v.as_str().map(String::from)
                    }
                });
                task.updated = event.ts;
            }
        }
        Operation::Comment => {
            if let Some(task) = tasks.get_mut(&event.id) {
                let d = &event.d;
                task.comments.push(Comment {
                    ts: event.ts,
                    by: event.by,
                    body: d
                        .get("body")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    r#ref: d.get("ref").and_then(|v| v.as_str()).map(String::from),
                });
                task.updated = event.ts;
            }
        }
        Operation::Link | Operation::Unlink => {
            if let (Some(rel), Some(target)) = (
                event.d.get("rel").and_then(|v| v.as_str()),
                event.d.get("target").and_then(|v| v.as_str()),
            ) {
                let add = event.op == Operation::Link;
                if let Some(task) = tasks.get_mut(&event.id) {
                    edit_relationship(task, rel, target, add);
                    task.updated = event.ts;
                }
                let inverse = match rel {
                    "blocks" => Some("blocked_by"),
                    "blocked_by" => Some("blocks"),
                    _ => None,
                };
                if let Some(inverse) = inverse {
                    if let Some(task) = tasks.get_mut(target) {
                        edit_relationship(task, inverse, &event.id, add);
                    }
                }
            }
        }
        Operation::Complete => {
            if let Some(task) = tasks.get_mut(&event.id) {
                task.status = TaskStatus::Complete;
                task.completed = Some(event.ts);
                task.resolution = event
                    .d
                    .get("resolution")
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .or(Some("done".to_string()));
                task.updated = event.ts;
                task.claim = None;
                if let Some(body) = event.d.get("body").and_then(|v| v.as_str()) {
                    task.comments.push(Comment {
                        ts: event.ts,
                        by: event.by,
                        body: body.to_string(),
                        r#ref: event
                            .d
                            .get("ref")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                    });
                }
            }
        }
        Operation::Reopen => {
            if let Some(task) = tasks.get_mut(&event.id) {
                task.status = TaskStatus::Open;
                task.completed = None;
                task.resolution = None;
                task.claim = None;
                task.archived = None;
                task.updated = event.ts;
            }
        }
        Operation::Archive => {
            if let Some(task) = tasks.get_mut(&event.id) {
                task.archived = event
                    .d
                    .get("ref")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                task.updated = event.ts;
            }
        }
        Operation::SetStream => {
            if let Some(task) = tasks.get_mut(&event.id) {
                task.stream = event.d.get("stream").and_then(|v| {
                    if v.is_null() {
                        None
                    } else {
                        v.as_str().map(String::from)
                    }
                });
                task.updated = event.ts;
            }
        }
        Operation::CreateStream => {
            let d = &event.d;
            let stream = Stream {
                id: event.id.clone(),
                name: d
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                description: d
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                created: event.ts,
                created_by: event.by.clone(),
            };
            streams.insert(event.id, stream);
        }
        Operation::UpdateStream => {
            if let Some(stream) = streams.get_mut(&event.id) {
                let d = &event.d;
                if let Some(name) = d.get("name").and_then(|v| v.as_str()) {
                    stream.name = name.to_string();
                }
                if let Some(desc) = d.get("description").and_then(|v| v.as_str()) {
                    stream.description = Some(desc.to_string());
                }
            }
        }
        Operation::DeleteStream => {
            streams.remove(&event.id);
        }
    }
    Ok(())
}

fn edit_relationship(task: &mut Task, rel: &str, target: &str, add: bool) {
    let values = match rel {
        "blocks" => &mut task.blocks,
        "blocked_by" => &mut task.blocked_by,
        "parent" => {
            if add {
                task.parent = Some(target.to_string());
            } else if task.parent.as_deref() == Some(target) {
                task.parent = None;
            }
            return;
        }
        _ => return,
    };
    if add && !values.iter().any(|value| value == target) {
        values.push(target.to_string());
        values.sort();
    } else if !add {
        values.retain(|value| value != target);
    }
}

pub fn build_index(ctx: &SpoolContext) -> Result<Index> {
    let state = materialize(ctx)?;
    let mut task_files: HashMap<String, HashSet<String>> = HashMap::new();
    let mut files = ctx.get_archive_files()?;
    files.extend(ctx.get_event_files()?);
    for file in files {
        let filename = file.strip_prefix(&ctx.root)?.to_string_lossy().to_string();
        for event in ctx.parse_events_from_file(&file)? {
            task_files
                .entry(event.id)
                .or_default()
                .insert(filename.clone());
        }
    }
    let tasks = state
        .tasks
        .into_iter()
        .map(|(id, task)| {
            let mut files: Vec<_> = task_files
                .remove(&id)
                .unwrap_or_default()
                .into_iter()
                .collect();
            files.sort();
            (
                id,
                TaskIndex {
                    status: task.status,
                    created: task.created.format("%Y-%m-%d").to_string(),
                    updated: task.updated.format("%Y-%m-%d").to_string(),
                    completed: task
                        .completed
                        .map(|date| date.format("%Y-%m-%d").to_string()),
                    archived: task.archived,
                    files,
                },
            )
        })
        .collect();
    Ok(Index {
        tasks,
        rebuilt: Utc::now(),
    })
}

pub fn load_or_materialize_state(ctx: &SpoolContext) -> Result<State> {
    let _lock = crate::concurrency::FileLock::acquire(ctx)?;
    materialize(ctx)
}

pub fn rebuild(ctx: &SpoolContext) -> Result<()> {
    let _lock = crate::concurrency::FileLock::acquire(ctx)?;
    println!("Rebuilding index and state...");

    let index = build_index(ctx)?;
    crate::store::write_json(&ctx.index_path(), &index)?;
    println!("  Wrote .index.json ({} tasks)", index.tasks.len());

    let state = materialize(ctx)?;
    crate::store::write_json(&ctx.state_path(), &state)?;
    println!(
        "  Wrote .state.json ({} tasks, {} streams)",
        state.tasks.len(),
        state.streams.len()
    );

    println!("Rebuild complete.");
    Ok(())
}
