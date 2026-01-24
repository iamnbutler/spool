use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn spool_cmd() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("spool"))
}

fn setup_initialized_spool(temp_dir: &TempDir) {
    let spool_dir = temp_dir.path().join(".spool");
    fs::create_dir_all(spool_dir.join("events")).unwrap();
    fs::create_dir_all(spool_dir.join("archive")).unwrap();
    // Write version file to avoid migration messages in tests
    fs::write(spool_dir.join(".version"), "0.4.0").unwrap();
}

fn write_test_events(temp_dir: &TempDir, events: &str) {
    let events_dir = temp_dir.path().join(".spool/events");
    fs::write(events_dir.join("2024-01-15.jsonl"), events).unwrap();
}

#[test]
fn test_init_creates_spool_dir() {
    let temp_dir = TempDir::new().unwrap();

    spool_cmd()
        .current_dir(temp_dir.path())
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("Created .spool/"));

    assert!(temp_dir.path().join(".spool").is_dir());
    assert!(temp_dir.path().join(".spool/events").is_dir());
    assert!(temp_dir.path().join(".spool/archive").is_dir());
}

#[test]
fn test_init_fails_if_exists() {
    let temp_dir = TempDir::new().unwrap();
    fs::create_dir(temp_dir.path().join(".spool")).unwrap();

    spool_cmd()
        .current_dir(temp_dir.path())
        .arg("init")
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn test_list_empty() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);

    spool_cmd()
        .current_dir(temp_dir.path())
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("No tasks found"));
}

#[test]
fn test_list_shows_tasks() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);
    write_test_events(
        &temp_dir,
        r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Test task","priority":"p1"}}"#,
    );

    spool_cmd()
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
    setup_initialized_spool(&temp_dir);
    write_test_events(
        &temp_dir,
        r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Test task"}}"#,
    );

    spool_cmd()
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
    setup_initialized_spool(&temp_dir);
    write_test_events(
        &temp_dir,
        r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Test task"}}"#,
    );

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["list", "--format", "ids"])
        .assert()
        .success()
        .stdout(predicate::str::is_match("^task-001\n$").unwrap());
}

#[test]
fn test_list_status_filter_complete() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);
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
    spool_cmd()
        .current_dir(temp_dir.path())
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("task-001"))
        .stdout(predicate::str::contains("task-002").not());

    // --status complete shows only completed
    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["list", "--status", "complete"])
        .assert()
        .success()
        .stdout(predicate::str::contains("task-001").not())
        .stdout(predicate::str::contains("task-002"));

    // --status all shows both
    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["list", "--status", "all"])
        .assert()
        .success()
        .stdout(predicate::str::contains("task-001"))
        .stdout(predicate::str::contains("task-002"));
}

#[test]
fn test_show_task() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);
    write_test_events(
        &temp_dir,
        r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Test task","description":"A description","priority":"p1"}}"#,
    );

    spool_cmd()
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
    setup_initialized_spool(&temp_dir);

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["show", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn test_show_with_events_flag() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);
    write_test_events(
        &temp_dir,
        concat!(
            r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Test task"}}"#,
            "\n",
            r#"{"v":1,"op":"update","id":"task-001","ts":"2024-01-15T11:00:00Z","by":"@tester","branch":"main","d":{"title":"Updated task"}}"#
        ),
    );

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["show", "task-001", "--events"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Event History:"))
        .stdout(predicate::str::contains("create"))
        .stdout(predicate::str::contains("update"));
}

#[test]
fn test_complete_task() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);
    write_test_events(
        &temp_dir,
        r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Test task"}}"#,
    );

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["complete", "task-001"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Completed task: task-001"));

    // Verify task is now complete
    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["list", "--status", "complete"])
        .assert()
        .success()
        .stdout(predicate::str::contains("task-001"));
}

#[test]
fn test_complete_task_with_resolution() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);
    write_test_events(
        &temp_dir,
        r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Test task"}}"#,
    );

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["complete", "task-001", "--resolution", "wontfix"])
        .assert()
        .success()
        .stdout(predicate::str::contains("wontfix"));
}

#[test]
fn test_complete_already_complete_errors() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);
    write_test_events(
        &temp_dir,
        concat!(
            r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Test task"}}"#,
            "\n",
            r#"{"v":1,"op":"complete","id":"task-001","ts":"2024-01-15T11:00:00Z","by":"@tester","branch":"main","d":{"resolution":"done"}}"#
        ),
    );

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["complete", "task-001"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already complete"));
}

#[test]
fn test_reopen_task() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);
    write_test_events(
        &temp_dir,
        concat!(
            r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Test task"}}"#,
            "\n",
            r#"{"v":1,"op":"complete","id":"task-001","ts":"2024-01-15T11:00:00Z","by":"@tester","branch":"main","d":{"resolution":"done"}}"#
        ),
    );

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["reopen", "task-001"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Reopened task: task-001"));

    // Verify task is now open
    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["list", "--status", "open"])
        .assert()
        .success()
        .stdout(predicate::str::contains("task-001"));
}

#[test]
fn test_reopen_already_open_errors() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);
    write_test_events(
        &temp_dir,
        r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Test task"}}"#,
    );

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["reopen", "task-001"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already open"));
}

#[test]
fn test_update_task_title() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);
    write_test_events(
        &temp_dir,
        r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Old title"}}"#,
    );

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["update", "task-001", "--title", "New title"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated task"));

    // Verify title changed
    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["show", "task-001"])
        .assert()
        .success()
        .stdout(predicate::str::contains("New title"));
}

#[test]
fn test_update_task_not_found() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["update", "nonexistent", "--title", "New title"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn test_rebuild_regenerates_state() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);
    write_test_events(
        &temp_dir,
        r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Test task"}}"#,
    );

    // Delete state file if exists
    let state_path = temp_dir.path().join(".spool/.state.json");
    let _ = fs::remove_file(&state_path);

    spool_cmd()
        .current_dir(temp_dir.path())
        .arg("rebuild")
        .assert()
        .success();

    // State file should be recreated
    assert!(state_path.exists());
}

#[test]
fn test_validate_valid_events() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);
    write_test_events(
        &temp_dir,
        r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Test task"}}"#,
    );

    spool_cmd()
        .current_dir(temp_dir.path())
        .arg("validate")
        .assert()
        .success();
}

#[test]
fn test_command_outside_spool_dir() {
    let temp_dir = TempDir::new().unwrap();
    // Don't initialize spool

    spool_cmd()
        .current_dir(temp_dir.path())
        .arg("list")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not in a spool directory"));
}

#[test]
fn test_missing_required_arg() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);

    spool_cmd()
        .current_dir(temp_dir.path())
        .arg("show") // Missing task ID
        .assert()
        .failure();
}

#[test]
fn test_add_task() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["add", "New task from CLI"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created task:"));

    // Verify task appears in list
    spool_cmd()
        .current_dir(temp_dir.path())
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("New task from CLI"));
}

#[test]
fn test_add_task_with_options() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);

    spool_cmd()
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
    let output = spool_cmd()
        .current_dir(temp_dir.path())
        .args(["list", "--format", "json"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    assert!(stdout.contains("Task with options"));
    assert!(stdout.contains("p0"));
}

#[test]
fn test_assign_task() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);
    write_test_events(
        &temp_dir,
        r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Test task"}}"#,
    );

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["assign", "task-001", "@alice"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Assigned task task-001 to @alice"));

    // Verify assignment
    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["show", "task-001"])
        .assert()
        .success()
        .stdout(predicate::str::contains("@alice"));
}

#[test]
fn test_assign_task_not_found() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["assign", "nonexistent", "@alice"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn test_claim_task() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);
    write_test_events(
        &temp_dir,
        r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Test task"}}"#,
    );

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["claim", "task-001"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Claimed task task-001"));
}

#[test]
fn test_claim_task_not_found() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["claim", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn test_free_task() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);
    write_test_events(
        &temp_dir,
        r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Test task","assignee":"@alice"}}"#,
    );

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["free", "task-001"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Freed task task-001"));
}

#[test]
fn test_free_task_not_found() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["free", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn test_stream_list_empty() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);
    write_test_events(
        &temp_dir,
        r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Task without stream"}}"#,
    );

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["stream", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No streams found"));
}

#[test]
fn test_stream_list_shows_streams() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);
    write_test_events(
        &temp_dir,
        concat!(
            r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"API task","stream":"api"}}"#,
            "\n",
            r#"{"v":1,"op":"create","id":"task-002","ts":"2024-01-15T11:00:00Z","by":"@tester","branch":"main","d":{"title":"Frontend task","stream":"frontend"}}"#,
            "\n",
            r#"{"v":1,"op":"create","id":"task-003","ts":"2024-01-15T12:00:00Z","by":"@tester","branch":"main","d":{"title":"Another API task","stream":"api"}}"#
        ),
    );

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["stream", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("api"))
        .stdout(predicate::str::contains("frontend"))
        .stdout(predicate::str::contains("2")) // api has 2 tasks
        .stdout(predicate::str::contains("1")); // frontend has 1 task
}

#[test]
fn test_stream_show() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);
    write_test_events(
        &temp_dir,
        concat!(
            r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"API task","stream":"api"}}"#,
            "\n",
            r#"{"v":1,"op":"create","id":"task-002","ts":"2024-01-15T11:00:00Z","by":"@tester","branch":"main","d":{"title":"Frontend task","stream":"frontend"}}"#
        ),
    );

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["stream", "show", "api"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Stream: api"))
        .stdout(predicate::str::contains("task-001"))
        .stdout(predicate::str::contains("API task"))
        .stdout(predicate::str::contains("task-002").not());
}

#[test]
fn test_stream_show_empty() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);
    write_test_events(
        &temp_dir,
        r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Task without stream"}}"#,
    );

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["stream", "show", "nonexistent"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No tasks found in stream"));
}

#[test]
fn test_stream_add() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);
    write_test_events(
        &temp_dir,
        r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Task without stream"}}"#,
    );

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["stream", "add", "task-001", "backend"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Added task task-001 to stream 'backend'"));

    // Verify it was added
    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["stream", "show", "backend"])
        .assert()
        .success()
        .stdout(predicate::str::contains("task-001"));
}

#[test]
fn test_stream_remove() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);
    write_test_events(
        &temp_dir,
        r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Task in stream","stream":"api"}}"#,
    );

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["stream", "remove", "task-001"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Removed task task-001 from stream"));

    // Verify it was removed
    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["stream", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No streams found"));
}

#[test]
fn test_migration_creates_version_file() {
    let temp_dir = TempDir::new().unwrap();
    
    // Manually setup without version file to simulate old spool
    let spool_dir = temp_dir.path().join(".spool");
    fs::create_dir_all(spool_dir.join("events")).unwrap();
    fs::create_dir_all(spool_dir.join("archive")).unwrap();
    // DON'T write .version file to simulate 0.3.1
    
    // Add some events to simulate existing data
    write_test_events(
        &temp_dir,
        r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Old task"}}"#,
    );

    // Run any command - this should trigger migration
    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Migrating spool from version 0.3.1 to 0.4.0"));

    // Check that version file was created
    let version_file = temp_dir.path().join(".spool/.version");
    assert!(version_file.exists());
    let version_content = fs::read_to_string(&version_file).unwrap();
    assert_eq!(version_content.trim(), "0.4.0");
}
