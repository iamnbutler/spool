use spool::context::SpoolContext;
use spool::state::{Task, TaskStatus};
use serde_json::json;
use std::fs;
use std::io::Write;
use tempfile::TempDir;

/// Helper to create a spool directory structure for testing
fn setup_spool_dir(temp_dir: &TempDir) -> std::path::PathBuf {
    let spool_dir = temp_dir.path().join(".spool");
    fs::create_dir_all(spool_dir.join("events")).unwrap();
    fs::create_dir_all(spool_dir.join("archive")).unwrap();
    spool_dir
}

/// Create a SpoolContext for testing
fn create_test_context(spool_dir: &std::path::Path) -> SpoolContext {
    SpoolContext::new(spool_dir.to_path_buf())
}

/// Write events to a file
fn write_events(dir: &std::path::Path, filename: &str, events: &[serde_json::Value]) {
    let path = dir.join(filename);
    let mut file = fs::File::create(path).unwrap();
    for event in events {
        writeln!(file, "{}", serde_json::to_string(event).unwrap()).unwrap();
    }
}

#[test]
fn test_task_status_default() {
    let status = TaskStatus::default();
    assert_eq!(status, TaskStatus::Open);
}

#[test]
fn test_task_status_serialization() {
    assert_eq!(
        serde_json::to_string(&TaskStatus::Open).unwrap(),
        "\"open\""
    );
    assert_eq!(
        serde_json::to_string(&TaskStatus::Complete).unwrap(),
        "\"complete\""
    );
}

#[test]
fn test_task_status_deserialization() {
    let open: TaskStatus = serde_json::from_str("\"open\"").unwrap();
    let complete: TaskStatus = serde_json::from_str("\"complete\"").unwrap();

    assert_eq!(open, TaskStatus::Open);
    assert_eq!(complete, TaskStatus::Complete);
}

#[test]
fn test_task_default() {
    let task = Task::default();

    assert!(task.id.is_empty());
    assert!(task.title.is_empty());
    assert_eq!(task.status, TaskStatus::Open);
    assert!(task.description.is_none());
    assert!(task.priority.is_none());
    assert!(task.tags.is_empty());
    assert!(task.assignee.is_none());
    assert!(task.completed.is_none());
    assert!(task.resolution.is_none());
    assert!(task.parent.is_none());
    assert!(task.blocks.is_empty());
    assert!(task.blocked_by.is_empty());
    assert!(task.comments.is_empty());
    assert!(task.archived.is_none());
}

#[test]
fn test_task_serialization_skips_none_fields() {
    let task = Task {
        id: "test-1".to_string(),
        title: "Test Task".to_string(),
        status: TaskStatus::Open,
        created: chrono::Utc::now(),
        created_by: "tester".to_string(),
        created_branch: "main".to_string(),
        updated: chrono::Utc::now(),
        ..Default::default()
    };

    let json_str = serde_json::to_string(&task).unwrap();

    // Optional None fields should not appear in output
    assert!(!json_str.contains("\"description\""));
    assert!(!json_str.contains("\"priority\""));
    assert!(!json_str.contains("\"assignee\""));
    assert!(!json_str.contains("\"completed\""));
    assert!(!json_str.contains("\"resolution\""));
    assert!(!json_str.contains("\"parent\""));
    assert!(!json_str.contains("\"archived\""));
}

#[test]
fn test_state_materialization_create_event() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);

    // Create a single create event
    let events = vec![json!({
        "v": 1,
        "op": "create",
        "id": "task-001",
        "ts": "2024-01-15T10:00:00Z",
        "by": "tester",
        "branch": "main",
        "d": {
            "title": "First Task",
            "description": "Task description",
            "priority": "p2",
            "tags": ["bug"],
            "assignee": "dev1"
        }
    })];

    write_events(&spool_dir.join("events"), "2024-01-15.jsonl", &events);

    let ctx = create_test_context(&spool_dir);
    let state = spool::state::materialize(&ctx).unwrap();

    assert_eq!(state.tasks.len(), 1);
    let task = state.tasks.get("task-001").unwrap();
    assert_eq!(task.id, "task-001");
    assert_eq!(task.title, "First Task");
    assert_eq!(task.description.as_deref(), Some("Task description"));
    assert_eq!(task.priority.as_deref(), Some("p2"));
    assert_eq!(task.tags, vec!["bug"]);
    assert_eq!(task.assignee.as_deref(), Some("dev1"));
    assert_eq!(task.status, TaskStatus::Open);
}

#[test]
fn test_state_materialization_update_event() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);

    let events = vec![
        json!({
            "v": 1, "op": "create", "id": "task-002",
            "ts": "2024-01-15T10:00:00Z", "by": "tester", "branch": "main",
            "d": {"title": "Original Title", "priority": "p3"}
        }),
        json!({
            "v": 1, "op": "update", "id": "task-002",
            "ts": "2024-01-15T11:00:00Z", "by": "tester", "branch": "main",
            "d": {"title": "Updated Title", "priority": "p1"}
        }),
    ];

    write_events(&spool_dir.join("events"), "2024-01-15.jsonl", &events);

    let ctx = create_test_context(&spool_dir);
    let state = spool::state::materialize(&ctx).unwrap();

    let task = state.tasks.get("task-002").unwrap();
    assert_eq!(task.title, "Updated Title");
    assert_eq!(task.priority.as_deref(), Some("p1"));
}

#[test]
fn test_state_materialization_complete_and_reopen() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);

    let events = vec![
        json!({
            "v": 1, "op": "create", "id": "task-003",
            "ts": "2024-01-15T10:00:00Z", "by": "tester", "branch": "main",
            "d": {"title": "Completeable Task"}
        }),
        json!({
            "v": 1, "op": "complete", "id": "task-003",
            "ts": "2024-01-15T12:00:00Z", "by": "tester", "branch": "main",
            "d": {"resolution": "fixed"}
        }),
    ];

    write_events(&spool_dir.join("events"), "2024-01-15.jsonl", &events);

    let ctx = create_test_context(&spool_dir);
    let state = spool::state::materialize(&ctx).unwrap();

    let task = state.tasks.get("task-003").unwrap();
    assert_eq!(task.status, TaskStatus::Complete);
    assert!(task.completed.is_some());
    assert_eq!(task.resolution.as_deref(), Some("fixed"));
}

#[test]
fn test_state_materialization_assign_event() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);

    let events = vec![
        json!({
            "v": 1, "op": "create", "id": "task-004",
            "ts": "2024-01-15T10:00:00Z", "by": "tester", "branch": "main",
            "d": {"title": "Assignment Test"}
        }),
        json!({
            "v": 1, "op": "assign", "id": "task-004",
            "ts": "2024-01-15T11:00:00Z", "by": "manager", "branch": "main",
            "d": {"to": "developer1"}
        }),
    ];

    write_events(&spool_dir.join("events"), "2024-01-15.jsonl", &events);

    let ctx = create_test_context(&spool_dir);
    let state = spool::state::materialize(&ctx).unwrap();

    let task = state.tasks.get("task-004").unwrap();
    assert_eq!(task.assignee.as_deref(), Some("developer1"));
}

#[test]
fn test_state_materialization_link_unlink() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);

    let events = vec![
        json!({
            "v": 1, "op": "create", "id": "task-005",
            "ts": "2024-01-15T10:00:00Z", "by": "tester", "branch": "main",
            "d": {"title": "Link Test"}
        }),
        json!({
            "v": 1, "op": "link", "id": "task-005",
            "ts": "2024-01-15T11:00:00Z", "by": "tester", "branch": "main",
            "d": {"rel": "blocks", "target": "task-other"}
        }),
        json!({
            "v": 1, "op": "link", "id": "task-005",
            "ts": "2024-01-15T11:05:00Z", "by": "tester", "branch": "main",
            "d": {"rel": "blocked_by", "target": "task-blocker"}
        }),
    ];

    write_events(&spool_dir.join("events"), "2024-01-15.jsonl", &events);

    let ctx = create_test_context(&spool_dir);
    let state = spool::state::materialize(&ctx).unwrap();

    let task = state.tasks.get("task-005").unwrap();
    assert!(task.blocks.contains(&"task-other".to_string()));
    assert!(task.blocked_by.contains(&"task-blocker".to_string()));
}

#[test]
fn test_state_materialization_comment() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);

    let events = vec![
        json!({
            "v": 1, "op": "create", "id": "task-006",
            "ts": "2024-01-15T10:00:00Z", "by": "tester", "branch": "main",
            "d": {"title": "Comment Test"}
        }),
        json!({
            "v": 1, "op": "comment", "id": "task-006",
            "ts": "2024-01-15T11:00:00Z", "by": "reviewer", "branch": "main",
            "d": {"body": "This looks good!", "ref": "commit-abc123"}
        }),
    ];

    write_events(&spool_dir.join("events"), "2024-01-15.jsonl", &events);

    let ctx = create_test_context(&spool_dir);
    let state = spool::state::materialize(&ctx).unwrap();

    let task = state.tasks.get("task-006").unwrap();
    assert_eq!(task.comments.len(), 1);
    assert_eq!(task.comments[0].body, "This looks good!");
    assert_eq!(task.comments[0].by, "reviewer");
    assert_eq!(task.comments[0].r#ref.as_deref(), Some("commit-abc123"));
}

#[test]
fn test_state_materialization_multiple_files() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);

    // Events across multiple days
    let day1_events = vec![json!({
        "v": 1, "op": "create", "id": "task-007",
        "ts": "2024-01-15T10:00:00Z", "by": "tester", "branch": "main",
        "d": {"title": "Day 1 Task"}
    })];

    let day2_events = vec![json!({
        "v": 1, "op": "create", "id": "task-008",
        "ts": "2024-01-16T10:00:00Z", "by": "tester", "branch": "main",
        "d": {"title": "Day 2 Task"}
    })];

    write_events(&spool_dir.join("events"), "2024-01-15.jsonl", &day1_events);
    write_events(&spool_dir.join("events"), "2024-01-16.jsonl", &day2_events);

    let ctx = create_test_context(&spool_dir);
    let state = spool::state::materialize(&ctx).unwrap();

    assert_eq!(state.tasks.len(), 2);
    assert!(state.tasks.contains_key("task-007"));
    assert!(state.tasks.contains_key("task-008"));
}

#[test]
fn test_state_rebuild() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);

    let events = vec![
        json!({
            "v": 1, "op": "create", "id": "task-rebuild",
            "ts": "2024-01-15T10:00:00Z", "by": "tester", "branch": "main",
            "d": {"title": "Rebuild Test"}
        }),
        json!({
            "v": 1, "op": "complete", "id": "task-rebuild",
            "ts": "2024-01-15T12:00:00Z", "by": "tester", "branch": "main",
            "d": {}
        }),
    ];

    write_events(&spool_dir.join("events"), "2024-01-15.jsonl", &events);

    let ctx = create_test_context(&spool_dir);
    spool::state::rebuild(&ctx).unwrap();

    // Verify index and state files were created
    assert!(ctx.index_path().exists());
    assert!(ctx.state_path().exists());

    // Verify state can be loaded
    let state = spool::state::load_or_materialize_state(&ctx).unwrap();
    assert_eq!(state.tasks.len(), 1);
    assert_eq!(
        state.tasks.get("task-rebuild").unwrap().status,
        TaskStatus::Complete
    );
}

#[test]
fn test_state_materialization_reopen() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);

    let events = vec![
        json!({
            "v": 1, "op": "create", "id": "task-reopen",
            "ts": "2024-01-15T10:00:00Z", "by": "tester", "branch": "main",
            "d": {"title": "Reopen Test"}
        }),
        json!({
            "v": 1, "op": "complete", "id": "task-reopen",
            "ts": "2024-01-15T11:00:00Z", "by": "tester", "branch": "main",
            "d": {"resolution": "fixed"}
        }),
        json!({
            "v": 1, "op": "reopen", "id": "task-reopen",
            "ts": "2024-01-15T12:00:00Z", "by": "tester", "branch": "main",
            "d": {}
        }),
    ];

    write_events(&spool_dir.join("events"), "2024-01-15.jsonl", &events);

    let ctx = create_test_context(&spool_dir);
    let state = spool::state::materialize(&ctx).unwrap();

    let task = state.tasks.get("task-reopen").unwrap();
    assert_eq!(task.status, TaskStatus::Open);
    assert!(task.completed.is_none());
    assert!(task.resolution.is_none());
}

#[test]
fn test_state_materialization_unlink() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);

    let events = vec![
        json!({
            "v": 1, "op": "create", "id": "task-unlink",
            "ts": "2024-01-15T10:00:00Z", "by": "tester", "branch": "main",
            "d": {"title": "Unlink Test", "blocks": ["other-task"]}
        }),
        json!({
            "v": 1, "op": "unlink", "id": "task-unlink",
            "ts": "2024-01-15T11:00:00Z", "by": "tester", "branch": "main",
            "d": {"rel": "blocks", "target": "other-task"}
        }),
    ];

    write_events(&spool_dir.join("events"), "2024-01-15.jsonl", &events);

    let ctx = create_test_context(&spool_dir);
    let state = spool::state::materialize(&ctx).unwrap();

    let task = state.tasks.get("task-unlink").unwrap();
    assert!(!task.blocks.contains(&"other-task".to_string()));
}

#[test]
fn test_state_materialization_archive() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);

    let events = vec![
        json!({
            "v": 1, "op": "create", "id": "task-archive",
            "ts": "2024-01-15T10:00:00Z", "by": "tester", "branch": "main",
            "d": {"title": "Archive Test"}
        }),
        json!({
            "v": 1, "op": "archive", "id": "task-archive",
            "ts": "2024-01-15T11:00:00Z", "by": "@spool", "branch": "main",
            "d": {"ref": "2024-01"}
        }),
    ];

    write_events(&spool_dir.join("events"), "2024-01-15.jsonl", &events);

    let ctx = create_test_context(&spool_dir);
    let state = spool::state::materialize(&ctx).unwrap();

    let task = state.tasks.get("task-archive").unwrap();
    assert_eq!(task.archived.as_deref(), Some("2024-01"));
}
