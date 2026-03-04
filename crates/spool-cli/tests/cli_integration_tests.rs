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

// ==========================================
// List filter tests
// ==========================================

#[test]
fn test_list_filter_by_assignee() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);
    write_test_events(
        &temp_dir,
        concat!(
            r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Assigned task","assignee":"@alice"}}"#,
            "\n",
            r#"{"v":1,"op":"create","id":"task-002","ts":"2024-01-15T11:00:00Z","by":"@tester","branch":"main","d":{"title":"Unassigned task"}}"#,
        ),
    );

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["list", "--assignee", "@alice"])
        .assert()
        .success()
        .stdout(predicate::str::contains("task-001"))
        .stdout(predicate::str::contains("task-002").not());
}

#[test]
fn test_list_filter_by_priority() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);
    write_test_events(
        &temp_dir,
        concat!(
            r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"High priority","priority":"p0"}}"#,
            "\n",
            r#"{"v":1,"op":"create","id":"task-002","ts":"2024-01-15T11:00:00Z","by":"@tester","branch":"main","d":{"title":"Low priority","priority":"p2"}}"#,
        ),
    );

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["list", "--priority", "p0"])
        .assert()
        .success()
        .stdout(predicate::str::contains("task-001"))
        .stdout(predicate::str::contains("task-002").not());
}

#[test]
fn test_list_filter_by_tag() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);
    write_test_events(
        &temp_dir,
        concat!(
            r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Tagged task","tags":["rust"]}}"#,
            "\n",
            r#"{"v":1,"op":"create","id":"task-002","ts":"2024-01-15T11:00:00Z","by":"@tester","branch":"main","d":{"title":"Untagged task"}}"#,
        ),
    );

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["list", "--tag", "rust"])
        .assert()
        .success()
        .stdout(predicate::str::contains("task-001"))
        .stdout(predicate::str::contains("task-002").not());
}

// ==========================================
// Stream command tests
// ==========================================

#[test]
fn test_stream_list_empty() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["stream", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No streams found"));
}

#[test]
fn test_stream_add_and_list() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["stream", "add", "My Stream"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created stream:"))
        .stdout(predicate::str::contains("My Stream"));

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["stream", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("My Stream"));
}

#[test]
fn test_stream_show_by_name() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["stream", "add", "Backend", "--description", "Backend work"])
        .assert()
        .success();

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["stream", "show", "--name", "Backend"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Name:"))
        .stdout(predicate::str::contains("Backend"))
        .stdout(predicate::str::contains("Backend work"));
}

#[test]
fn test_stream_show_not_found() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["stream", "show", "nonexistent-id"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn test_stream_update() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["stream", "add", "Old Name"])
        .assert()
        .success();

    let id_output = spool_cmd()
        .current_dir(temp_dir.path())
        .args(["stream", "list", "--format", "ids"])
        .assert()
        .success();
    let stream_id = String::from_utf8_lossy(&id_output.get_output().stdout)
        .trim()
        .to_string();

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["stream", "update", &stream_id, "--name", "New Name"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated stream"));

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["stream", "show", "--name", "New Name"])
        .assert()
        .success()
        .stdout(predicate::str::contains("New Name"));
}

#[test]
fn test_stream_delete() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["stream", "add", "Temporary"])
        .assert()
        .success();

    let id_output = spool_cmd()
        .current_dir(temp_dir.path())
        .args(["stream", "list", "--format", "ids"])
        .assert()
        .success();
    let stream_id = String::from_utf8_lossy(&id_output.get_output().stdout)
        .trim()
        .to_string();

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["stream", "delete", &stream_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("Deleted stream:"));

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["stream", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No streams found"));
}

#[test]
fn test_stream_delete_with_tasks_fails() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["stream", "add", "Active Stream"])
        .assert()
        .success();

    let id_output = spool_cmd()
        .current_dir(temp_dir.path())
        .args(["stream", "list", "--format", "ids"])
        .assert()
        .success();
    let stream_id = String::from_utf8_lossy(&id_output.get_output().stdout)
        .trim()
        .to_string();

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["add", "Task in stream", "--stream", &stream_id])
        .assert()
        .success();

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["stream", "delete", &stream_id])
        .assert()
        .failure()
        .stderr(predicate::str::contains("tasks are still assigned"));
}

#[test]
fn test_add_task_invalid_stream_fails() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["add", "Task", "--stream", "nonexistent-stream-id"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Stream not found"));
}

#[test]
fn test_list_filter_by_stream() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["stream", "add", "Frontend"])
        .assert()
        .success();

    let id_output = spool_cmd()
        .current_dir(temp_dir.path())
        .args(["stream", "list", "--format", "ids"])
        .assert()
        .success();
    let stream_id = String::from_utf8_lossy(&id_output.get_output().stdout)
        .trim()
        .to_string();

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["add", "Frontend task", "--stream", &stream_id])
        .assert()
        .success();

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["add", "No-stream task"])
        .assert()
        .success();

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["list", "--stream", &stream_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("Frontend task"))
        .stdout(predicate::str::contains("No-stream task").not());

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["list", "--stream-name", "Frontend"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Frontend task"))
        .stdout(predicate::str::contains("No-stream task").not());
}

#[test]
fn test_list_no_stream_filter() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["stream", "add", "Backend"])
        .assert()
        .success();

    let id_output = spool_cmd()
        .current_dir(temp_dir.path())
        .args(["stream", "list", "--format", "ids"])
        .assert()
        .success();
    let stream_id = String::from_utf8_lossy(&id_output.get_output().stdout)
        .trim()
        .to_string();

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["add", "Streamed task", "--stream", &stream_id])
        .assert()
        .success();

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["add", "Orphan task"])
        .assert()
        .success();

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["list", "--no-stream"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Orphan task"))
        .stdout(predicate::str::contains("Streamed task").not());
}

#[test]
fn test_stream_list_json_format() {
    let temp_dir = TempDir::new().unwrap();
    setup_initialized_spool(&temp_dir);

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["stream", "add", "JSON Stream"])
        .assert()
        .success();

    spool_cmd()
        .current_dir(temp_dir.path())
        .args(["stream", "list", "--format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"name\":"))
        .stdout(predicate::str::contains("JSON Stream"));
}
