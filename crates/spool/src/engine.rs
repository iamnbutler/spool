//! Task invariants live here, inside the same transaction as event publication.
//! Both the CLI and TUI use this path; no check-then-write gap is permitted.

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashSet;

use crate::concurrency::FileLock;
use crate::context::SpoolContext;
use crate::event::{Event, Operation};
use crate::state::{materialize_events, Claim, State, Stream, Task, TaskStatus};
use crate::store::{publish, read_events};

pub const DEFAULT_LEASE_SECONDS: u32 = 900;

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct SpoolError {
    pub code: &'static str,
    pub message: String,
}

impl SpoolError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

fn fail<T>(code: &'static str, message: impl Into<String>) -> Result<T> {
    Err(SpoolError::new(code, message).into())
}

#[derive(Debug, Clone, Serialize)]
pub struct Identity {
    pub agent: String,
    pub source: String,
    pub explicit: bool,
}

impl Identity {
    pub fn resolve(flag: Option<&str>) -> Result<Self> {
        if let Some(agent) = flag {
            nonempty(agent, "Agent identity")?;
            return Ok(Self {
                agent: agent.to_string(),
                source: "--agent".into(),
                explicit: true,
            });
        }
        for key in ["SPOOL_AGENT", "TELEPHONE_ADDR"] {
            if let Ok(agent) = std::env::var(key) {
                nonempty(&agent, "Agent identity")?;
                return Ok(Self {
                    agent,
                    source: key.into(),
                    explicit: true,
                });
            }
        }
        Ok(Self {
            agent: crate::writer::get_human_user()?,
            source: "git/USER".into(),
            explicit: false,
        })
    }

    pub fn require_agent(&self) -> Result<()> {
        if !self.explicit {
            return fail("identity_required", "Use --agent, SPOOL_AGENT, or TELEPHONE_ADDR with a unique identity for this agent session before claiming work");
        }
        Ok(())
    }
}

pub fn nonempty(value: &str, field: &str) -> Result<()> {
    if value.trim().is_empty() {
        return fail("invalid_input", format!("{field} must not be empty"));
    }
    Ok(())
}

pub fn task<'a>(state: &'a State, id: &str) -> Result<&'a Task> {
    if let Some(task) = state.tasks.get(id) {
        return Ok(task);
    }
    let mut matches = state.tasks.values().filter(|task| task.id.starts_with(id));
    match (matches.next(), matches.next()) {
        (Some(task), None) if !id.is_empty() => Ok(task),
        (Some(_), Some(_)) => fail("ambiguous_id", format!("Ambiguous task ID prefix: {id}")),
        _ => fail("not_found", format!("Task not found: {id}")),
    }
}

pub fn stream<'a>(state: &'a State, id_or_name: &str) -> Result<&'a Stream> {
    if let Some(stream) = state.streams.get(id_or_name) {
        return Ok(stream);
    }
    let mut matches = state.streams.values().filter(|stream| {
        stream.name.eq_ignore_ascii_case(id_or_name) || stream.id.starts_with(id_or_name)
    });
    match (matches.next(), matches.next()) {
        (Some(stream), None) if !id_or_name.is_empty() => Ok(stream),
        (Some(_), Some(_)) => fail("ambiguous_id", format!("Ambiguous stream: {id_or_name}")),
        _ => fail("not_found", format!("Stream not found: {id_or_name}")),
    }
}

/// Include both spellings of legacy edges. Missing prerequisites block work.
pub fn dependencies(_state: &State, task: &Task) -> Vec<String> {
    let mut ids = task.blocked_by.clone();
    ids.sort();
    ids.dedup();
    ids
}

pub fn blockers(state: &State, task: &Task) -> Vec<String> {
    dependencies(state, task)
        .into_iter()
        .filter(|id| {
            state
                .tasks
                .get(id)
                .map(|task| task.status != TaskStatus::Complete)
                .unwrap_or(true)
        })
        .collect()
}

pub fn is_ready(state: &State, task: &Task, agent: &str, now: DateTime<Utc>) -> bool {
    task.status == TaskStatus::Open
        && task.archived.is_none()
        && !task.claim.as_ref().is_some_and(|claim| claim.is_live(now))
        && (task.claim.is_some()
            || task
                .assignee
                .as_deref()
                .map(|owner| owner == agent)
                .unwrap_or(true))
        && blockers(state, task).is_empty()
}

pub fn work_status(state: &State, task: &Task, now: DateTime<Utc>) -> &'static str {
    if task.status == TaskStatus::Complete {
        "complete"
    } else if task.claim.as_ref().is_some_and(|claim| claim.is_live(now)) {
        "in_progress"
    } else if !blockers(state, task).is_empty() {
        "blocked"
    } else {
        "open"
    }
}

pub fn sort_tasks(tasks: &mut Vec<&Task>) {
    tasks.sort_by(|a, b| {
        (a.priority.as_deref().unwrap_or("p2"), a.created, &a.id).cmp(&(
            b.priority.as_deref().unwrap_or("p2"),
            b.created,
            &b.id,
        ))
    });
}

pub fn view(state: &State, task: &Task, agent: &str, full: bool) -> Value {
    let now = Utc::now();
    let mut value = if full {
        serde_json::to_value(task).expect("Task is serializable")
    } else {
        json!({
            "id": task.id, "title": task.title, "status": task.status,
            "priority": task.priority.as_deref().unwrap_or("p2"),
            "assignee": task.assignee, "stream": task.stream, "claim": task.claim,
        })
    };
    value["work_status"] = json!(work_status(state, task, now));
    value["ready"] = json!(is_ready(state, task, agent, now));
    value["blockers"] = json!(blockers(state, task));
    if let Some(claim) = &task.claim {
        value["claim"]["expired"] = json!(!claim.is_live(now));
    }
    value
}

fn has_path(state: &State, start: &str, target: &str, parent: bool) -> bool {
    let mut pending = vec![start.to_string()];
    let mut seen = HashSet::new();
    while let Some(id) = pending.pop() {
        if id == target {
            return true;
        }
        if seen.insert(id.clone()) {
            if let Some(task) = state.tasks.get(&id) {
                if parent {
                    pending.extend(task.parent.iter().cloned());
                } else {
                    pending.extend(dependencies(state, task));
                }
            }
        }
    }
    false
}

fn owner(task: &Task, agent: &str, token: Option<&str>, allow_expired: bool) -> Result<()> {
    if let Some(claim) = &task.claim {
        // Recovery must also work for abandoned tasks with open prerequisites:
        // those cannot be claimed again yet. An explicit token still fences
        // stale invocations, even when the current claim has expired.
        if allow_expired && !claim.is_live(Utc::now()) && token.is_none() {
            return Ok(());
        }
        if claim.agent != agent || token != Some(claim.token.as_str()) {
            return fail(
                "claim_conflict",
                format!(
                    "Task {} is claimed by {}; supply that session's current claim token",
                    task.id, claim.agent
                ),
            );
        }
        if !allow_expired && !claim.is_live(Utc::now()) {
            return fail(
                "lease_expired",
                format!(
                    "Claim for {} expired; claim the task again before changing it",
                    task.id
                ),
            );
        }
    } else if token.is_some() {
        return fail(
            "stale_token",
            format!("Task {} no longer has that claim", task.id),
        );
    }
    Ok(())
}

fn claimable(state: &State, task: &Task, agent: &str) -> Result<()> {
    if task.status != TaskStatus::Open || task.archived.is_some() {
        return fail("not_open", format!("Task is not open: {}", task.id));
    }
    if let Some(claim) = &task.claim {
        if claim.is_live(Utc::now()) {
            return fail(
                "claim_conflict",
                format!(
                    "Task {} is claimed by {} until {}",
                    task.id, claim.agent, claim.expires_at
                ),
            );
        }
    }
    if let Some(assignee) = task.assignee.as_ref().filter(|_| task.claim.is_none()) {
        if assignee != agent {
            return fail(
                "assigned",
                format!("Task {} is assigned to {assignee}", task.id),
            );
        }
    }
    let blockers = blockers(state, task);
    if !blockers.is_empty() {
        return fail(
            "blocked",
            format!("Task {} is blocked by {}", task.id, blockers.join(", ")),
        );
    }
    Ok(())
}

fn string<'a>(data: &'a Value, field: &str) -> Result<&'a str> {
    let value = data
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| SpoolError::new("invalid_input", format!("Missing or invalid {field}")))?;
    nonempty(value, field)?;
    Ok(value)
}

/// Resolve IDs and validate every mutation against the state under the lock.
fn prepare(state: &State, event: &mut Event, token: Option<&str>) -> Result<()> {
    nonempty(&event.by, "Actor")?;
    if event.v != 1 || !event.d.is_object() {
        return fail(
            "invalid_input",
            "Expected a v1 event with an object payload",
        );
    }
    for field in ["title", "name", "body"] {
        if event.d.get(field).is_some() {
            string(&event.d, field)?;
        }
    }
    if let Some(priority) = event.d.get("priority") {
        if !matches!(priority.as_str(), Some("p0" | "p1" | "p2" | "p3")) {
            return fail("invalid_input", "Priority must be p0, p1, p2, or p3");
        }
    }
    if let Some(stream_value) = event.d.get("stream") {
        if !stream_value.is_null() {
            let resolved = stream(state, string(&event.d, "stream")?)?;
            event.d["stream"] = json!(resolved.id);
        }
    }
    if let Some(to) = event.d.get("to") {
        if !to.is_null() {
            string(&event.d, "to")?;
        }
    }
    if event
        .d
        .get("assignee")
        .is_some_and(|value| !value.is_null())
    {
        string(&event.d, "assignee")?;
    }
    match event.op {
        Operation::Create | Operation::CreateStream => {
            nonempty(&event.id, "ID")?;
            if state.tasks.contains_key(&event.id) || state.streams.contains_key(&event.id) {
                return fail("already_exists", format!("ID already exists: {}", event.id));
            }
            if event.op == Operation::Create {
                string(&event.d, "title")?;
            }
        }
        Operation::UpdateStream | Operation::DeleteStream => {
            event.id = stream(state, &event.id)?.id.clone();
            if event.op == Operation::DeleteStream
                && state.tasks.values().any(|task| {
                    task.stream.as_deref() == Some(&event.id) && task.archived.is_none()
                })
            {
                return fail(
                    "stream_not_empty",
                    "Cannot delete stream: tasks are still assigned. Move or archive them first.",
                );
            }
        }
        _ => {
            event.id = task(state, &event.id)?.id.clone();
            let current = &state.tasks[&event.id];
            if !matches!(event.op, Operation::Comment | Operation::Claim) {
                let releasing =
                    event.op == Operation::Handoff && event.d.get("to").is_some_and(Value::is_null);
                // Adding a consumer of A changes B's prerequisites. Both edge
                // spellings must guard the dependent task's claim, even when
                // the event is recorded on the prerequisite's history.
                let guarded = if matches!(event.op, Operation::Link | Operation::Unlink)
                    && event.d.get("rel").and_then(Value::as_str) == Some("blocks")
                {
                    task(state, string(&event.d, "target")?)?
                } else {
                    current
                };
                owner(guarded, &event.by, token, releasing)?;
            }
        }
    }
    if event.op == Operation::Update
        && (event.d.get("add_tags").is_some() || event.d.get("remove_tags").is_some())
    {
        let mut tags = state.tasks[&event.id].tags.clone();
        for (field, add) in [("add_tags", true), ("remove_tags", false)] {
            if let Some(values) = event.d.get(field) {
                let values: Vec<String> = serde_json::from_value(values.clone())?;
                for tag in values {
                    nonempty(&tag, "Tag")?;
                    if add && !tags.contains(&tag) {
                        tags.push(tag);
                    } else if !add {
                        tags.retain(|value| value != &tag);
                    }
                }
            }
        }
        tags.sort();
        event.d.as_object_mut().unwrap().remove("add_tags");
        event.d.as_object_mut().unwrap().remove("remove_tags");
        event.d["tags"] = json!(tags);
    }
    if let Some(tags) = event.d.get("tags") {
        for tag in serde_json::from_value::<Vec<String>>(tags.clone())? {
            nonempty(&tag, "Tag")?;
        }
    }
    if matches!(event.op, Operation::CreateStream | Operation::UpdateStream) {
        if event.d.get("name").is_some() {
            let name = string(&event.d, "name")?.trim().to_lowercase();
            if state
                .streams
                .values()
                .any(|stream| stream.id != event.id && stream.name.eq_ignore_ascii_case(&name))
            {
                return fail("already_exists", format!("Stream already exists: {name}"));
            }
            event.d["name"] = json!(name);
        } else if event.op == Operation::CreateStream {
            return fail("invalid_input", "Stream name is required");
        }
    }
    match event.op {
        Operation::Update | Operation::UpdateStream
            if event.d.as_object().is_some_and(|data| data.is_empty()) =>
        {
            return fail("invalid_input", "No fields to update");
        }
        Operation::Complete => {
            let current = &state.tasks[&event.id];
            if current.status == TaskStatus::Complete {
                return fail("already_complete", "Task is already complete");
            }
            let resolution = event
                .d
                .get("resolution")
                .and_then(Value::as_str)
                .unwrap_or("done");
            if !matches!(resolution, "done" | "wontfix" | "duplicate" | "obsolete") {
                return fail(
                    "invalid_input",
                    "Resolution must be done, wontfix, duplicate, or obsolete",
                );
            }
            let blocked = blockers(state, current);
            if resolution == "done" && !blocked.is_empty() {
                return fail(
                    "blocked",
                    format!("Task is blocked by {}", blocked.join(", ")),
                );
            }
        }
        Operation::Reopen if state.tasks[&event.id].status == TaskStatus::Open => {
            return fail("already_open", "Task is already open");
        }
        Operation::Claim => {
            claimable(state, &state.tasks[&event.id], &event.by)?;
            let claim: Claim = serde_json::from_value(event.d["claim"].clone())?;
            if claim.agent != event.by || !claim.is_live(Utc::now()) || claim.token.is_empty() {
                return fail("invalid_input", "Invalid claim");
            }
        }
        Operation::Renew => {
            let current = &state.tasks[&event.id];
            let claim: Claim = serde_json::from_value(event.d["claim"].clone())?;
            if current.claim.as_ref().map(|old| &old.token) != Some(&claim.token) {
                return fail("stale_token", "Claim changed before renewal");
            }
        }
        Operation::Handoff => {
            string(&event.d, "body")?;
            if state.tasks[&event.id].status != TaskStatus::Open {
                return fail("not_open", "Only open tasks can be handed off");
            }
        }
        Operation::Comment => {
            string(&event.d, "body")?;
        }
        Operation::Link | Operation::Unlink => {
            let relation = string(&event.d, "rel")?.to_string();
            let target = task(state, string(&event.d, "target")?)?.id.clone();
            if !matches!(relation.as_str(), "blocks" | "blocked_by" | "parent") {
                return fail(
                    "invalid_input",
                    "Relationship must be blocks, blocked_by, or parent",
                );
            }
            if target == event.id {
                return fail("cycle", "A task cannot depend on itself");
            }
            let (dependent, prerequisite) = if relation == "blocks" {
                (&target, &event.id)
            } else {
                (&event.id, &target)
            };
            if event.op == Operation::Link
                && has_path(state, prerequisite, dependent, relation == "parent")
            {
                return fail("cycle", "Dependency would create a cycle");
            }
            event.d["target"] = json!(target);
        }
        _ => {}
    }
    Ok(())
}

fn commit_locked(
    ctx: &SpoolContext,
    mut events: Vec<Event>,
    mut event: Event,
    token: Option<&str>,
) -> Result<State> {
    let state = materialize_events(events.clone())?;
    prepare(&state, &mut event, token)?;
    event.ts = Utc::now();
    if let Some(last) = events.last() {
        if event.ts <= last.ts {
            event.ts = last.ts + Duration::nanoseconds(1);
        }
    }
    let directory = if event.is_local() {
        ctx.local_events_dir()
    } else {
        ctx.events_dir.clone()
    };
    // Apply before publication so malformed payloads cannot poison the board.
    events.push(event.clone());
    let state = materialize_events(events)?;
    publish(&directory, &event)?;
    Ok(state)
}

pub fn commit(ctx: &SpoolContext, event: Event, token: Option<&str>) -> Result<State> {
    let _lock = FileLock::acquire(ctx)?;
    commit_locked(ctx, read_events(ctx, true)?, event, token)
}

pub fn event(op: Operation, id: &str, by: &str, data: Value) -> Result<Event> {
    Ok(Event {
        v: 1,
        op,
        id: id.to_string(),
        ts: Utc::now(),
        by: by.to_string(),
        branch: crate::writer::get_current_branch()?,
        d: data,
    })
}

fn lease_duration(seconds: u32) -> Result<Duration> {
    if !(1..=86_400).contains(&seconds) {
        return fail(
            "invalid_input",
            "Lease duration must be between 1 and 86400 seconds",
        );
    }
    Ok(Duration::seconds(seconds.into()))
}

#[derive(Default)]
pub struct ReadyFilter<'a> {
    pub stream: Option<&'a str>,
    pub tag: Option<&'a str>,
}

pub fn claim(
    ctx: &SpoolContext,
    id: Option<&str>,
    identity: &Identity,
    seconds: u32,
    filter: ReadyFilter<'_>,
) -> Result<Option<(State, String)>> {
    identity.require_agent()?;
    let duration = lease_duration(seconds)?;
    let _lock = FileLock::acquire(ctx)?;
    let events = read_events(ctx, true)?;
    let state = materialize_events(events.clone())?;
    let stream_id = filter
        .stream
        .map(|name| stream(&state, name).map(|stream| &stream.id))
        .transpose()?;
    let id = match id {
        Some(id) => task(&state, id)?.id.clone(),
        None => {
            let mut ready: Vec<_> = state
                .tasks
                .values()
                .filter(|task| {
                    is_ready(&state, task, &identity.agent, Utc::now())
                        && stream_id
                            .map(|id| task.stream.as_ref() == Some(id))
                            .unwrap_or(true)
                        && filter
                            .tag
                            .map(|tag| {
                                task.tags
                                    .iter()
                                    .any(|value| value.eq_ignore_ascii_case(tag))
                            })
                            .unwrap_or(true)
                })
                .collect();
            sort_tasks(&mut ready);
            match ready.first() {
                Some(task) => task.id.clone(),
                None => return Ok(None),
            }
        }
    };
    claimable(&state, &state.tasks[&id], &identity.agent)?;
    let now = Utc::now();
    let token = rand::random::<[u8; 16]>()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let claim = Claim {
        agent: identity.agent.clone(),
        token,
        started_at: now,
        expires_at: now + duration,
        branch: crate::writer::get_current_branch()?,
        worktree: std::env::current_dir()?.display().to_string(),
    };
    let event = event(
        Operation::Claim,
        &id,
        &identity.agent,
        json!({"claim": claim}),
    )?;
    let state = commit_locked(ctx, events, event, None)?;
    Ok(Some((state, id)))
}

pub fn renew(
    ctx: &SpoolContext,
    id: &str,
    identity: &Identity,
    token: Option<&str>,
    seconds: u32,
) -> Result<(State, String)> {
    identity.require_agent()?;
    let duration = lease_duration(seconds)?;
    let _lock = FileLock::acquire(ctx)?;
    let events = read_events(ctx, true)?;
    let state = materialize_events(events.clone())?;
    let current = task(&state, id)?;
    owner(current, &identity.agent, token, false)?;
    let mut claim = current
        .claim
        .clone()
        .ok_or_else(|| SpoolError::new("not_claimed", "Task has no claim to renew"))?;
    claim.expires_at = Utc::now() + duration;
    let id = current.id.clone();
    let event = event(
        Operation::Renew,
        &id,
        &identity.agent,
        json!({"claim": claim}),
    )?;
    let state = commit_locked(ctx, events, event, token)?;
    Ok((state, id))
}
