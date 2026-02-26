use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;

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
    let mut tasks: HashMap<String, Task> = HashMap::new();
    let mut streams: HashMap<String, Stream> = HashMap::new();

    // First process archive files
    for file in ctx.get_archive_files()? {
        let events = ctx.parse_events_from_file(&file)?;
        apply_events(&mut tasks, &mut streams, events);
    }

    // Then process event files (in chronological order)
    for file in ctx.get_event_files()? {
        let events = ctx.parse_events_from_file(&file)?;
        apply_events(&mut tasks, &mut streams, events);
    }

    Ok(State {
        tasks,
        streams,
        rebuilt: Utc::now(),
    })
}

fn apply_events(
    tasks: &mut HashMap<String, Task>,
    streams: &mut HashMap<String, Stream>,
    events: Vec<Event>,
) {
    for event in events {
        apply_event(tasks, streams, event);
    }
}

fn apply_event(
    tasks: &mut HashMap<String, Task>,
    streams: &mut HashMap<String, Stream>,
    event: Event,
) {
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
        Operation::Link => {
            if let Some(task) = tasks.get_mut(&event.id) {
                let d = &event.d;
                if let (Some(rel), Some(target)) = (
                    d.get("rel").and_then(|v| v.as_str()),
                    d.get("target").and_then(|v| v.as_str()),
                ) {
                    match rel {
                        "blocks" => {
                            if !task.blocks.contains(&target.to_string()) {
                                task.blocks.push(target.to_string());
                            }
                        }
                        "blocked_by" => {
                            if !task.blocked_by.contains(&target.to_string()) {
                                task.blocked_by.push(target.to_string());
                            }
                        }
                        "parent" => task.parent = Some(target.to_string()),
                        _ => {}
                    }
                }
                task.updated = event.ts;
            }
        }
        Operation::Unlink => {
            if let Some(task) = tasks.get_mut(&event.id) {
                let d = &event.d;
                if let (Some(rel), Some(target)) = (
                    d.get("rel").and_then(|v| v.as_str()),
                    d.get("target").and_then(|v| v.as_str()),
                ) {
                    match rel {
                        "blocks" => task.blocks.retain(|x| x != target),
                        "blocked_by" => task.blocked_by.retain(|x| x != target),
                        "parent" => {
                            if task.parent.as_deref() == Some(target) {
                                task.parent = None;
                            }
                        }
                        _ => {}
                    }
                }
                task.updated = event.ts;
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
            }
        }
        Operation::Reopen => {
            if let Some(task) = tasks.get_mut(&event.id) {
                task.status = TaskStatus::Open;
                task.completed = None;
                task.resolution = None;
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
}

/// Internal struct for building task index entries
struct TaskIndexBuilder {
    status: TaskStatus,
    created: String,
    updated: String,
    completed: Option<String>,
    archived: Option<String>,
}

pub fn build_index(ctx: &SpoolContext) -> Result<Index> {
    let mut task_files: HashMap<String, HashSet<String>> = HashMap::new();
    let mut task_info: HashMap<String, TaskIndexBuilder> = HashMap::new();

    for file in ctx.get_event_files()? {
        let filename = file
            .file_name()
            .ok_or_else(|| {
                anyhow::anyhow!("event file path has no filename component: {:?}", file)
            })?
            .to_string_lossy()
            .to_string();
        let events = ctx.parse_events_from_file(&file)?;
        for event in events {
            task_files
                .entry(event.id.clone())
                .or_default()
                .insert(filename.clone());

            let date = event.ts.format("%Y-%m-%d").to_string();

            match event.op {
                Operation::Create => {
                    task_info.insert(
                        event.id.clone(),
                        TaskIndexBuilder {
                            status: TaskStatus::Open,
                            created: date.clone(),
                            updated: date,
                            completed: None,
                            archived: None,
                        },
                    );
                }
                Operation::Complete => {
                    if let Some(info) = task_info.get_mut(&event.id) {
                        info.status = TaskStatus::Complete;
                        info.updated = date.clone();
                        info.completed = Some(date);
                    }
                }
                Operation::Reopen => {
                    if let Some(info) = task_info.get_mut(&event.id) {
                        info.status = TaskStatus::Open;
                        info.updated = date;
                        info.completed = None;
                    }
                }
                Operation::Archive => {
                    if let Some(info) = task_info.get_mut(&event.id) {
                        info.updated = date;
                        info.archived = event
                            .d
                            .get("ref")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                    }
                }
                _ => {
                    if let Some(info) = task_info.get_mut(&event.id) {
                        info.updated = date;
                    }
                }
            }
        }
    }

    let mut tasks = HashMap::new();
    for (id, info) in task_info {
        let files: Vec<String> = task_files
            .get(&id)
            .map(|s| {
                let mut v: Vec<_> = s.iter().cloned().collect();
                v.sort();
                v
            })
            .unwrap_or_default();

        tasks.insert(
            id,
            TaskIndex {
                status: info.status,
                created: info.created,
                updated: info.updated,
                completed: info.completed,
                files,
                archived: info.archived,
            },
        );
    }

    Ok(Index {
        tasks,
        rebuilt: Utc::now(),
    })
}

pub fn load_or_materialize_state(ctx: &SpoolContext) -> Result<State> {
    let state_path = ctx.state_path();
    if state_path.exists() {
        let content = fs::read_to_string(&state_path)?;
        let state: State = serde_json::from_str(&content)?;
        Ok(state)
    } else {
        materialize(ctx)
    }
}

pub fn rebuild(ctx: &SpoolContext) -> Result<()> {
    println!("Rebuilding index and state...");

    let index = build_index(ctx)?;
    let index_json = serde_json::to_string_pretty(&index)?;
    fs::write(ctx.index_path(), index_json)?;
    println!("  Wrote .index.json ({} tasks)", index.tasks.len());

    let state = materialize(ctx)?;
    let state_json = serde_json::to_string_pretty(&state)?;
    fs::write(ctx.state_path(), state_json)?;
    println!(
        "  Wrote .state.json ({} tasks, {} streams)",
        state.tasks.len(),
        state.streams.len()
    );

    println!("Rebuild complete.");
    Ok(())
}
