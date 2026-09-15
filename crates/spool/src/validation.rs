use anyhow::{anyhow, Result};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;

use crate::concurrency::FileLock;
use crate::context::SpoolContext;
use crate::event::{Event, Operation};

#[derive(Debug, Default, Serialize)]
pub struct ValidationResult {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn inspect(ctx: &SpoolContext) -> Result<ValidationResult> {
    let _lock = FileLock::acquire(ctx)?;
    let mut result = ValidationResult::default();
    let mut files = ctx.get_archive_files()?;
    files.extend(ctx.get_event_files()?);
    files.extend(ctx.get_local_event_files()?);
    let mut creates = HashMap::new();
    let mut events = Vec::new();
    let mut seen = HashSet::new();
    for file in files {
        for (line, content) in fs::read_to_string(&file)?.lines().enumerate() {
            if content.trim().is_empty() {
                continue;
            }
            let location = format!("{}:{}", file.display(), line + 1);
            let value: serde_json::Value = match serde_json::from_str(content) {
                Ok(value) => value,
                Err(error) => {
                    result
                        .errors
                        .push(format!("{location}: Invalid JSON: {error}"));
                    continue;
                }
            };
            let mut invalid = false;
            for field in ["v", "op", "id", "ts", "by", "branch", "d"] {
                if value.get(field).is_none() {
                    result
                        .errors
                        .push(format!("{location}: Missing required field '{field}'"));
                    invalid = true;
                }
            }
            if invalid {
                continue;
            }
            if value.get("v").and_then(|value| value.as_u64()) != Some(1) {
                result.errors.push(format!(
                    "{location}: Unsupported schema version {}",
                    value["v"]
                ));
                continue;
            }
            if value["ts"]
                .as_str()
                .map(|value| chrono::DateTime::parse_from_rfc3339(value).is_err())
                .unwrap_or(true)
            {
                result
                    .errors
                    .push(format!("{location}: Invalid timestamp format"));
                continue;
            }
            let event: Event = match serde_json::from_value(value) {
                Ok(event) => event,
                Err(error) => {
                    result
                        .errors
                        .push(format!("{location}: Invalid event: {error}"));
                    continue;
                }
            };
            if !event.d.is_object() {
                result
                    .errors
                    .push(format!("{location}: Event payload must be an object"));
                continue;
            }
            let hash = crate::store::fingerprint(&event)?;
            if !seen.insert(hash.clone()) {
                continue;
            }
            if matches!(event.op, Operation::Create | Operation::CreateStream)
                && creates.insert(event.id.clone(), event.ts).is_some()
            {
                result
                    .warnings
                    .push(format!("{location}: Duplicate create for {}", event.id));
            }
            for field in ["title", "name", "body"] {
                if event.d.get(field).is_some_and(|value| {
                    value
                        .as_str()
                        .map(|value| value.trim().is_empty())
                        .unwrap_or(true)
                }) {
                    result.errors.push(format!("{location}: Invalid {field}"));
                }
            }
            if event.op == Operation::Create
                && event
                    .d
                    .get("title")
                    .and_then(|value| value.as_str())
                    .is_none()
            {
                result
                    .errors
                    .push(format!("{location}: Create requires a title"));
            }
            if event
                .d
                .get("priority")
                .is_some_and(|value| !matches!(value.as_str(), Some("p0" | "p1" | "p2" | "p3")))
            {
                result
                    .warnings
                    .push(format!("{location}: Unknown priority"));
            }
            events.push((event, location));
        }
    }
    for (event, location) in &events {
        if !matches!(event.op, Operation::Create | Operation::CreateStream)
            && creates
                .get(&event.id)
                .map(|created| *created > event.ts)
                .unwrap_or(true)
        {
            result
                .warnings
                .push(format!("{location}: Event for {} before create", event.id));
        }
    }
    if result.errors.is_empty() {
        match crate::state::materialize(ctx) {
            Err(error) => result
                .errors
                .push(format!("Cannot replay history: {error:#}")),
            Ok(state) => {
                for task in state.tasks.values() {
                    for id in crate::engine::dependencies(&state, task) {
                        if !state.tasks.contains_key(&id) {
                            result.warnings.push(format!(
                                "Task {} references non-existent blocked_by: {id}",
                                task.id
                            ));
                        }
                    }
                    for id in &task.blocks {
                        if !state.tasks.contains_key(id) {
                            result.warnings.push(format!(
                                "Task {} references non-existent blocks: {id}",
                                task.id
                            ));
                        }
                    }
                    if let Some(parent) = &task.parent {
                        if !state.tasks.contains_key(parent) {
                            result.warnings.push(format!(
                                "Task {} references non-existent parent: {parent}",
                                task.id
                            ));
                        }
                    }
                    if let Some(stream) = &task.stream {
                        if !state.streams.contains_key(stream) && task.archived.is_none() {
                            result.warnings.push(format!(
                                "Task {} references non-existent stream: {stream}",
                                task.id
                            ));
                        }
                    }
                    if dependency_cycle(&state, &task.id) {
                        result
                            .errors
                            .push(format!("Dependency cycle involving {}", task.id));
                    }
                }
            }
        }
    }
    result.errors.sort();
    result.warnings.sort();
    Ok(result)
}

fn dependency_cycle(state: &crate::state::State, start: &str) -> bool {
    let mut pending = crate::engine::dependencies(state, &state.tasks[start]);
    let mut seen = HashSet::new();
    while let Some(id) = pending.pop() {
        if id == start {
            return true;
        }
        if seen.insert(id.clone()) {
            if let Some(task) = state.tasks.get(&id) {
                pending.extend(crate::engine::dependencies(state, task));
            }
        }
    }
    false
}

/// Compatibility wrapper for library/TUI callers. The CLI always exits nonzero
/// for errors; strict also makes warnings fail.
pub fn validate(ctx: &SpoolContext, strict: bool) -> Result<ValidationResult> {
    let result = inspect(ctx)?;
    if result.errors.is_empty() && result.warnings.is_empty() {
        println!("Validation passed. No issues found.");
    } else {
        for error in &result.errors {
            println!("ERROR: {error}");
        }
        for warning in &result.warnings {
            println!("WARN: {warning}");
        }
    }
    if strict && !result.errors.is_empty() {
        return Err(anyhow!(
            "Validation failed with {} errors",
            result.errors.len()
        ));
    }
    if strict && !result.warnings.is_empty() {
        return Err(anyhow!(
            "Validation failed with {} warnings",
            result.warnings.len()
        ));
    }
    Ok(result)
}
