use std::fs;
use tempfile::TempDir;

use fabric::context::FabricContext;
use fabric::event::{Event, Operation};
use fabric::writer::{
    complete_task, create_task, get_current_branch, get_current_user, reopen_task, update_task,
    write_event,
};

fn setup_fabric_dir(temp_dir: &TempDir) -> std::path::PathBuf {
    let fabric_dir = temp_dir.path().join(".fabric");
    fs::create_dir_all(fabric_dir.join("events")).unwrap();
    fs::create_dir_all(fabric_dir.join("archive")).unwrap();
    fabric_dir
}

fn create_test_context(fabric_dir: &std::path::Path) -> FabricContext {
    FabricContext::new(fabric_dir.to_path_buf())
}

#[test]
fn test_write_event_creates_file() {
    let temp_dir = TempDir::new().unwrap();
    let fabric_dir = setup_fabric_dir(&temp_dir);
    let ctx = create_test_context(&fabric_dir);

    let event = Event {
        v: 1,
        op: Operation::Create,
        id: "test-001".to_string(),
        ts: chrono::Utc::now(),
        by: "@tester".to_string(),
        branch: "main".to_string(),
        d: serde_json::json!({"title": "Test task"}),
    };

    write_event(&ctx, &event).unwrap();

    // Verify file was created
    let event_files = ctx.get_event_files().unwrap();
    assert_eq!(event_files.len(), 1);

    // Verify content
    let content = fs::read_to_string(&event_files[0]).unwrap();
    assert!(content.contains("test-001"));
    assert!(content.contains("Test task"));
}

#[test]
fn test_write_event_appends_to_existing_file() {
    let temp_dir = TempDir::new().unwrap();
    let fabric_dir = setup_fabric_dir(&temp_dir);
    let ctx = create_test_context(&fabric_dir);

    let event1 = Event {
        v: 1,
        op: Operation::Create,
        id: "test-001".to_string(),
        ts: chrono::Utc::now(),
        by: "@tester".to_string(),
        branch: "main".to_string(),
        d: serde_json::json!({"title": "First task"}),
    };

    let event2 = Event {
        v: 1,
        op: Operation::Create,
        id: "test-002".to_string(),
        ts: chrono::Utc::now(),
        by: "@tester".to_string(),
        branch: "main".to_string(),
        d: serde_json::json!({"title": "Second task"}),
    };

    write_event(&ctx, &event1).unwrap();
    write_event(&ctx, &event2).unwrap();

    // Verify still one file
    let event_files = ctx.get_event_files().unwrap();
    assert_eq!(event_files.len(), 1);

    // Verify both events in file
    let content = fs::read_to_string(&event_files[0]).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("test-001"));
    assert!(lines[1].contains("test-002"));
}

#[test]
fn test_create_task_returns_id() {
    let temp_dir = TempDir::new().unwrap();
    let fabric_dir = setup_fabric_dir(&temp_dir);
    let ctx = create_test_context(&fabric_dir);

    let id = create_task(
        &ctx,
        "Test task",
        None,
        None,
        None,
        vec![],
        "@tester",
        "main",
    )
    .unwrap();

    // ID should be non-empty
    assert!(!id.is_empty());

    // Verify event was written
    let event_files = ctx.get_event_files().unwrap();
    assert_eq!(event_files.len(), 1);

    let content = fs::read_to_string(&event_files[0]).unwrap();
    assert!(content.contains(&id));
    assert!(content.contains("Test task"));
}

#[test]
fn test_create_task_with_all_fields() {
    let temp_dir = TempDir::new().unwrap();
    let fabric_dir = setup_fabric_dir(&temp_dir);
    let ctx = create_test_context(&fabric_dir);

    let id = create_task(
        &ctx,
        "Full task",
        Some("Task description"),
        Some("p1"),
        Some("@dev"),
        vec!["bug".to_string(), "urgent".to_string()],
        "@tester",
        "feature-branch",
    )
    .unwrap();

    let event_files = ctx.get_event_files().unwrap();
    let content = fs::read_to_string(&event_files[0]).unwrap();

    assert!(content.contains(&id));
    assert!(content.contains("Full task"));
    assert!(content.contains("Task description"));
    assert!(content.contains("p1"));
    assert!(content.contains("@dev"));
    assert!(content.contains("bug"));
    assert!(content.contains("urgent"));
    assert!(content.contains("feature-branch"));
}

#[test]
fn test_update_task_writes_event() {
    let temp_dir = TempDir::new().unwrap();
    let fabric_dir = setup_fabric_dir(&temp_dir);
    let ctx = create_test_context(&fabric_dir);

    update_task(
        &ctx,
        "task-001",
        Some("New title"),
        Some("New description"),
        Some("p0"),
        "@tester",
        "main",
    )
    .unwrap();

    let event_files = ctx.get_event_files().unwrap();
    let content = fs::read_to_string(&event_files[0]).unwrap();

    assert!(content.contains("task-001"));
    assert!(content.contains("update"));
    assert!(content.contains("New title"));
    assert!(content.contains("New description"));
    assert!(content.contains("p0"));
}

#[test]
fn test_update_task_partial_fields() {
    let temp_dir = TempDir::new().unwrap();
    let fabric_dir = setup_fabric_dir(&temp_dir);
    let ctx = create_test_context(&fabric_dir);

    // Update only title
    update_task(
        &ctx,
        "task-001",
        Some("Only title"),
        None,
        None,
        "@tester",
        "main",
    )
    .unwrap();

    let event_files = ctx.get_event_files().unwrap();
    let content = fs::read_to_string(&event_files[0]).unwrap();

    assert!(content.contains("Only title"));
    assert!(!content.contains("description"));
    assert!(!content.contains("priority"));
}

#[test]
fn test_update_task_no_fields_errors() {
    let temp_dir = TempDir::new().unwrap();
    let fabric_dir = setup_fabric_dir(&temp_dir);
    let ctx = create_test_context(&fabric_dir);

    let result = update_task(&ctx, "task-001", None, None, None, "@tester", "main");

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("No fields to update"));
}

#[test]
fn test_complete_task_writes_event() {
    let temp_dir = TempDir::new().unwrap();
    let fabric_dir = setup_fabric_dir(&temp_dir);
    let ctx = create_test_context(&fabric_dir);

    complete_task(&ctx, "task-001", Some("done"), "@tester", "main").unwrap();

    let event_files = ctx.get_event_files().unwrap();
    let content = fs::read_to_string(&event_files[0]).unwrap();

    assert!(content.contains("task-001"));
    assert!(content.contains("complete"));
    assert!(content.contains("done"));
}

#[test]
fn test_complete_task_default_resolution() {
    let temp_dir = TempDir::new().unwrap();
    let fabric_dir = setup_fabric_dir(&temp_dir);
    let ctx = create_test_context(&fabric_dir);

    complete_task(&ctx, "task-001", None, "@tester", "main").unwrap();

    let event_files = ctx.get_event_files().unwrap();
    let content = fs::read_to_string(&event_files[0]).unwrap();

    assert!(content.contains("done")); // Default resolution
}

#[test]
fn test_complete_task_wontfix_resolution() {
    let temp_dir = TempDir::new().unwrap();
    let fabric_dir = setup_fabric_dir(&temp_dir);
    let ctx = create_test_context(&fabric_dir);

    complete_task(&ctx, "task-001", Some("wontfix"), "@tester", "main").unwrap();

    let event_files = ctx.get_event_files().unwrap();
    let content = fs::read_to_string(&event_files[0]).unwrap();

    assert!(content.contains("wontfix"));
}

#[test]
fn test_reopen_task_writes_event() {
    let temp_dir = TempDir::new().unwrap();
    let fabric_dir = setup_fabric_dir(&temp_dir);
    let ctx = create_test_context(&fabric_dir);

    reopen_task(&ctx, "task-001", "@tester", "main").unwrap();

    let event_files = ctx.get_event_files().unwrap();
    let content = fs::read_to_string(&event_files[0]).unwrap();

    assert!(content.contains("task-001"));
    assert!(content.contains("reopen"));
}

#[test]
fn test_get_current_user_returns_formatted_user() {
    let user = get_current_user().unwrap();

    // Should start with @
    assert!(user.starts_with('@'));
    // Should not be empty (minus the @)
    assert!(user.len() > 1);
}

#[test]
fn test_get_current_branch_returns_branch() {
    let branch = get_current_branch().unwrap();

    // Should not be empty
    assert!(!branch.is_empty());
}

#[test]
fn test_create_task_generates_unique_ids() {
    let temp_dir = TempDir::new().unwrap();
    let fabric_dir = setup_fabric_dir(&temp_dir);
    let ctx = create_test_context(&fabric_dir);

    let id1 = create_task(&ctx, "Task 1", None, None, None, vec![], "@tester", "main").unwrap();
    let id2 = create_task(&ctx, "Task 2", None, None, None, vec![], "@tester", "main").unwrap();
    let id3 = create_task(&ctx, "Task 3", None, None, None, vec![], "@tester", "main").unwrap();

    // All IDs should be unique
    assert_ne!(id1, id2);
    assert_ne!(id2, id3);
    assert_ne!(id1, id3);
}

#[test]
fn test_event_json_format() {
    let temp_dir = TempDir::new().unwrap();
    let fabric_dir = setup_fabric_dir(&temp_dir);
    let ctx = create_test_context(&fabric_dir);

    create_task(&ctx, "Test", None, None, None, vec![], "@tester", "main").unwrap();

    let event_files = ctx.get_event_files().unwrap();
    let content = fs::read_to_string(&event_files[0]).unwrap();

    // Should be valid JSON
    let parsed: serde_json::Value = serde_json::from_str(content.trim()).unwrap();

    // Verify structure
    assert_eq!(parsed["v"], 1);
    assert_eq!(parsed["op"], "create");
    assert!(parsed["id"].is_string());
    assert!(parsed["ts"].is_string());
    assert_eq!(parsed["by"], "@tester");
    assert_eq!(parsed["branch"], "main");
    assert!(parsed["d"].is_object());
}
