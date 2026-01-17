use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn fabric_cmd() -> Command {
    Command::cargo_bin("fabric").unwrap()
}

fn setup_initialized_fabric(temp_dir: &TempDir) {
    let fabric_dir = temp_dir.path().join(".fabric");
    fs::create_dir_all(fabric_dir.join("events")).unwrap();
    fs::create_dir_all(fabric_dir.join("archive")).unwrap();
}

fn write_test_events(temp_dir: &TempDir, events: &str) {
    let events_dir = temp_dir.path().join(".fabric/events");
    fs::write(events_dir.join("2024-01-15.jsonl"), events).unwrap();
}

// =============================================================================
// Init Command
// =============================================================================

#[test]
fn test_init_creates_fabric_dir() {
    let temp_dir = TempDir::new().unwrap();

    fabric_cmd()
        .current_dir(temp_dir.path())
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("Created .fabric/"));

    assert!(temp_dir.path().join(".fabric").is_dir());
    assert!(temp_dir.path().join(".fabric/events").is_dir());
    assert!(temp_dir.path().join(".fabric/archive").is_dir());
}

#[test]
fn test_init_fails_if_exists() {
    let temp_dir = TempDir::new().unwrap();
    fs::create_dir(temp_dir.path().join(".fabric")).unwrap();

    fabric_cmd()
        .current_dir(temp_dir.path())
        .arg("init")
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

// =============================================================================
// List Command
// =============================================================================

#[test]
fn test_list_empty() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_fabric(&temp_dir);

    fabric_cmd()
        .current_dir(temp_dir.path())
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("No tasks found"));
}

#[test]
fn test_list_shows_tasks() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_fabric(&temp_dir);
    write_test_events(
        &temp_dir,
        r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Test task","priority":"p1"}}"#,
    );

    fabric_cmd()
        .current_dir(temp_dir.path())
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("task-001"))
        .stdout(predicate::str::contains("Test task"))
        .stdout(predicate::str::contains("p1"));
}

#[test]
fn test_list_json_format() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_fabric(&temp_dir);
    write_test_events(
        &temp_dir,
        r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Test task"}}"#,
    );

    fabric_cmd()
        .current_dir(temp_dir.path())
        .args(["list", "--format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"id\":"))
        .stdout(predicate::str::contains("\"title\":"));
}

#[test]
fn test_list_ids_format() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_fabric(&temp_dir);
    write_test_events(
        &temp_dir,
        r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Test task"}}"#,
    );

    fabric_cmd()
        .current_dir(temp_dir.path())
        .args(["list", "--format", "ids"])
        .assert()
        .success()
        .stdout(predicate::str::is_match("^task-001\n$").unwrap());
}

#[test]
fn test_list_status_filter_complete() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_fabric(&temp_dir);
    write_test_events(
        &temp_dir,
        concat!(
            r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Open task"}}"#,
            "\n",
            r#"{"v":1,"op":"create","id":"task-002","ts":"2024-01-15T11:00:00Z","by":"@tester","branch":"main","d":{"title":"Completed task"}}"#,
            "\n",
            r#"{"v":1,"op":"complete","id":"task-002","ts":"2024-01-15T12:00:00Z","by":"@tester","branch":"main","d":{"resolution":"done"}}"#
        ),
    );

    // Default shows only open
    fabric_cmd()
        .current_dir(temp_dir.path())
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("task-001"))
        .stdout(predicate::str::contains("task-002").not());

    // --status complete shows only completed
    fabric_cmd()
        .current_dir(temp_dir.path())
        .args(["list", "--status", "complete"])
        .assert()
        .success()
        .stdout(predicate::str::contains("task-001").not())
        .stdout(predicate::str::contains("task-002"));

    // --status all shows both
    fabric_cmd()
        .current_dir(temp_dir.path())
        .args(["list", "--status", "all"])
        .assert()
        .success()
        .stdout(predicate::str::contains("task-001"))
        .stdout(predicate::str::contains("task-002"));
}

// =============================================================================
// Show Command
// =============================================================================

#[test]
fn test_show_task() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_fabric(&temp_dir);
    write_test_events(
        &temp_dir,
        r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Test task","description":"A description","priority":"p1"}}"#,
    );

    fabric_cmd()
        .current_dir(temp_dir.path())
        .args(["show", "task-001"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ID:"))
        .stdout(predicate::str::contains("task-001"))
        .stdout(predicate::str::contains("Test task"))
        .stdout(predicate::str::contains("A description"))
        .stdout(predicate::str::contains("p1"));
}

#[test]
fn test_show_task_not_found() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_fabric(&temp_dir);

    fabric_cmd()
        .current_dir(temp_dir.path())
        .args(["show", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn test_show_with_events_flag() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_fabric(&temp_dir);
    write_test_events(
        &temp_dir,
        concat!(
            r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Test task"}}"#,
            "\n",
            r#"{"v":1,"op":"update","id":"task-001","ts":"2024-01-15T11:00:00Z","by":"@tester","branch":"main","d":{"title":"Updated task"}}"#
        ),
    );

    fabric_cmd()
        .current_dir(temp_dir.path())
        .args(["show", "task-001", "--events"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Event History:"))
        .stdout(predicate::str::contains("create"))
        .stdout(predicate::str::contains("update"));
}

// =============================================================================
// Complete Command
// =============================================================================

#[test]
fn test_complete_task() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_fabric(&temp_dir);
    write_test_events(
        &temp_dir,
        r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Test task"}}"#,
    );

    fabric_cmd()
        .current_dir(temp_dir.path())
        .args(["complete", "task-001"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Completed task: task-001"));

    // Verify task is now complete
    fabric_cmd()
        .current_dir(temp_dir.path())
        .args(["list", "--status", "complete"])
        .assert()
        .success()
        .stdout(predicate::str::contains("task-001"));
}

#[test]
fn test_complete_task_with_resolution() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_fabric(&temp_dir);
    write_test_events(
        &temp_dir,
        r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Test task"}}"#,
    );

    fabric_cmd()
        .current_dir(temp_dir.path())
        .args(["complete", "task-001", "--resolution", "wontfix"])
        .assert()
        .success()
        .stdout(predicate::str::contains("wontfix"));
}

#[test]
fn test_complete_already_complete_errors() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_fabric(&temp_dir);
    write_test_events(
        &temp_dir,
        concat!(
            r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Test task"}}"#,
            "\n",
            r#"{"v":1,"op":"complete","id":"task-001","ts":"2024-01-15T11:00:00Z","by":"@tester","branch":"main","d":{"resolution":"done"}}"#
        ),
    );

    fabric_cmd()
        .current_dir(temp_dir.path())
        .args(["complete", "task-001"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already complete"));
}

// =============================================================================
// Reopen Command
// =============================================================================

#[test]
fn test_reopen_task() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_fabric(&temp_dir);
    write_test_events(
        &temp_dir,
        concat!(
            r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Test task"}}"#,
            "\n",
            r#"{"v":1,"op":"complete","id":"task-001","ts":"2024-01-15T11:00:00Z","by":"@tester","branch":"main","d":{"resolution":"done"}}"#
        ),
    );

    fabric_cmd()
        .current_dir(temp_dir.path())
        .args(["reopen", "task-001"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Reopened task: task-001"));

    // Verify task is now open
    fabric_cmd()
        .current_dir(temp_dir.path())
        .args(["list", "--status", "open"])
        .assert()
        .success()
        .stdout(predicate::str::contains("task-001"));
}

#[test]
fn test_reopen_already_open_errors() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_fabric(&temp_dir);
    write_test_events(
        &temp_dir,
        r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Test task"}}"#,
    );

    fabric_cmd()
        .current_dir(temp_dir.path())
        .args(["reopen", "task-001"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already open"));
}

// =============================================================================
// Update Command
// =============================================================================

#[test]
fn test_update_task_title() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_fabric(&temp_dir);
    write_test_events(
        &temp_dir,
        r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Old title"}}"#,
    );

    fabric_cmd()
        .current_dir(temp_dir.path())
        .args(["update", "task-001", "--title", "New title"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated task"));

    // Verify title changed
    fabric_cmd()
        .current_dir(temp_dir.path())
        .args(["show", "task-001"])
        .assert()
        .success()
        .stdout(predicate::str::contains("New title"));
}

#[test]
fn test_update_task_not_found() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_fabric(&temp_dir);

    fabric_cmd()
        .current_dir(temp_dir.path())
        .args(["update", "nonexistent", "--title", "New title"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

// =============================================================================
// Rebuild Command
// =============================================================================

#[test]
fn test_rebuild_regenerates_state() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_fabric(&temp_dir);
    write_test_events(
        &temp_dir,
        r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Test task"}}"#,
    );

    // Delete state file if exists
    let state_path = temp_dir.path().join(".fabric/.state.json");
    let _ = fs::remove_file(&state_path);

    fabric_cmd()
        .current_dir(temp_dir.path())
        .arg("rebuild")
        .assert()
        .success();

    // State file should be recreated
    assert!(state_path.exists());
}

// =============================================================================
// Validate Command
// =============================================================================

#[test]
fn test_validate_valid_events() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_fabric(&temp_dir);
    write_test_events(
        &temp_dir,
        r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Test task"}}"#,
    );

    fabric_cmd()
        .current_dir(temp_dir.path())
        .arg("validate")
        .assert()
        .success();
}

// =============================================================================
// Error Cases
// =============================================================================

#[test]
fn test_command_outside_fabric_dir() {
    let temp_dir = TempDir::new().unwrap();
    // Don't initialize fabric

    fabric_cmd()
        .current_dir(temp_dir.path())
        .arg("list")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not in a fabric directory"));
}

#[test]
fn test_missing_required_arg() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_fabric(&temp_dir);

    fabric_cmd()
        .current_dir(temp_dir.path())
        .arg("show") // Missing task ID
        .assert()
        .failure();
}

// =============================================================================
// Add Command
// =============================================================================

#[test]
fn test_add_task() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_fabric(&temp_dir);

    fabric_cmd()
        .current_dir(temp_dir.path())
        .args(["add", "New task from CLI"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created task:"));

    // Verify task appears in list
    fabric_cmd()
        .current_dir(temp_dir.path())
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("New task from CLI"));
}

#[test]
fn test_add_task_with_options() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_fabric(&temp_dir);

    fabric_cmd()
        .current_dir(temp_dir.path())
        .args([
            "add",
            "Task with options",
            "-d",
            "A description",
            "-p",
            "p0",
            "-a",
            "@tester",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created task:"));

    // Verify task details
    let output = fabric_cmd()
        .current_dir(temp_dir.path())
        .args(["list", "--format", "json"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    assert!(stdout.contains("Task with options"));
    assert!(stdout.contains("p0"));
}

// =============================================================================
// Assign Command
// =============================================================================

#[test]
fn test_assign_task() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_fabric(&temp_dir);
    write_test_events(
        &temp_dir,
        r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Test task"}}"#,
    );

    fabric_cmd()
        .current_dir(temp_dir.path())
        .args(["assign", "task-001", "@alice"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Assigned task task-001 to @alice"));

    // Verify assignment
    fabric_cmd()
        .current_dir(temp_dir.path())
        .args(["show", "task-001"])
        .assert()
        .success()
        .stdout(predicate::str::contains("@alice"));
}

#[test]
fn test_assign_task_not_found() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_fabric(&temp_dir);

    fabric_cmd()
        .current_dir(temp_dir.path())
        .args(["assign", "nonexistent", "@alice"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

// =============================================================================
// Claim Command
// =============================================================================

#[test]
fn test_claim_task() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_fabric(&temp_dir);
    write_test_events(
        &temp_dir,
        r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Test task"}}"#,
    );

    fabric_cmd()
        .current_dir(temp_dir.path())
        .args(["claim", "task-001"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Claimed task task-001"));
}

#[test]
fn test_claim_task_not_found() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_fabric(&temp_dir);

    fabric_cmd()
        .current_dir(temp_dir.path())
        .args(["claim", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

// =============================================================================
// Free Command
// =============================================================================

#[test]
fn test_free_task() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_fabric(&temp_dir);
    write_test_events(
        &temp_dir,
        r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Test task","assignee":"@alice"}}"#,
    );

    fabric_cmd()
        .current_dir(temp_dir.path())
        .args(["free", "task-001"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Freed task task-001"));
}

#[test]
fn test_free_task_not_found() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_fabric(&temp_dir);

    fabric_cmd()
        .current_dir(temp_dir.path())
        .args(["free", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}
