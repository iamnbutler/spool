use chrono::{Duration, Utc};
use spool::context::SpoolContext;
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
fn test_archive_no_tasks_to_archive() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);

    // Only open tasks
    let events = vec![json!({
        "v": 1, "op": "create", "id": "open-task",
        "ts": "2024-01-15T10:00:00Z", "by": "tester", "branch": "main",
        "d": {"title": "Open Task"}
    })];

    write_events(&spool_dir.join("events"), "2024-01-15.jsonl", &events);

    let ctx = create_test_context(&spool_dir);
    let archived = spool::archive::archive_tasks(&ctx, 30, false).unwrap();

    assert!(archived.is_empty());
}

#[test]
fn test_archive_recently_completed_not_archived() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);

    // Task completed today should not be archived with default 30 day threshold
    let now = Utc::now();
    let events = vec![
        json!({
            "v": 1, "op": "create", "id": "recent-complete",
            "ts": now.to_rfc3339(), "by": "tester", "branch": "main",
            "d": {"title": "Recently Completed"}
        }),
        json!({
            "v": 1, "op": "complete", "id": "recent-complete",
            "ts": now.to_rfc3339(), "by": "tester", "branch": "main",
            "d": {}
        }),
    ];

    write_events(
        &spool_dir.join("events"),
        &format!("{}.jsonl", now.format("%Y-%m-%d")),
        &events,
    );

    let ctx = create_test_context(&spool_dir);
    let archived = spool::archive::archive_tasks(&ctx, 30, false).unwrap();

    assert!(archived.is_empty());
}

#[test]
fn test_archive_old_completed_archived() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);

    // Task completed 60 days ago should be archived
    let old_date = Utc::now() - Duration::days(60);
    let events = vec![
        json!({
            "v": 1, "op": "create", "id": "old-complete",
            "ts": old_date.to_rfc3339(), "by": "tester", "branch": "main",
            "d": {"title": "Old Completed Task"}
        }),
        json!({
            "v": 1, "op": "complete", "id": "old-complete",
            "ts": old_date.to_rfc3339(), "by": "tester", "branch": "main",
            "d": {}
        }),
    ];

    write_events(
        &spool_dir.join("events"),
        &format!("{}.jsonl", old_date.format("%Y-%m-%d")),
        &events,
    );

    let ctx = create_test_context(&spool_dir);
    let archived = spool::archive::archive_tasks(&ctx, 30, false).unwrap();

    assert_eq!(archived.len(), 1);
    assert!(archived.contains(&"old-complete".to_string()));

    // Verify archive file was created
    let archive_files: Vec<_> = fs::read_dir(spool_dir.join("archive"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "jsonl"))
        .collect();
    assert!(!archive_files.is_empty());
}

#[test]
fn test_archive_dry_run() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);

    let old_date = Utc::now() - Duration::days(60);
    let events = vec![
        json!({
            "v": 1, "op": "create", "id": "dry-run-task",
            "ts": old_date.to_rfc3339(), "by": "tester", "branch": "main",
            "d": {"title": "Dry Run Task"}
        }),
        json!({
            "v": 1, "op": "complete", "id": "dry-run-task",
            "ts": old_date.to_rfc3339(), "by": "tester", "branch": "main",
            "d": {}
        }),
    ];

    write_events(
        &spool_dir.join("events"),
        &format!("{}.jsonl", old_date.format("%Y-%m-%d")),
        &events,
    );

    let ctx = create_test_context(&spool_dir);

    // Dry run should return the task but not create archive files
    let archived = spool::archive::archive_tasks(&ctx, 30, true).unwrap();

    assert_eq!(archived.len(), 1);

    // No archive files should be created in dry run
    let archive_files: Vec<_> = fs::read_dir(spool_dir.join("archive"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "jsonl"))
        .collect();
    assert!(archive_files.is_empty());
}

#[test]
fn test_archive_custom_days_threshold() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);

    // Task completed 10 days ago
    let ten_days_ago = Utc::now() - Duration::days(10);
    let events = vec![
        json!({
            "v": 1, "op": "create", "id": "threshold-task",
            "ts": ten_days_ago.to_rfc3339(), "by": "tester", "branch": "main",
            "d": {"title": "Threshold Task"}
        }),
        json!({
            "v": 1, "op": "complete", "id": "threshold-task",
            "ts": ten_days_ago.to_rfc3339(), "by": "tester", "branch": "main",
            "d": {}
        }),
    ];

    write_events(
        &spool_dir.join("events"),
        &format!("{}.jsonl", ten_days_ago.format("%Y-%m-%d")),
        &events,
    );

    let ctx = create_test_context(&spool_dir);

    // Should not be archived with 30 day threshold
    let archived_30 = spool::archive::archive_tasks(&ctx, 30, true).unwrap();
    assert!(archived_30.is_empty());

    // Should be archived with 7 day threshold
    let archived_7 = spool::archive::archive_tasks(&ctx, 7, true).unwrap();
    assert_eq!(archived_7.len(), 1);
}

#[test]
fn test_archive_already_archived_not_rearchived() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);

    let old_date = Utc::now() - Duration::days(60);
    let events = vec![
        json!({
            "v": 1, "op": "create", "id": "already-archived",
            "ts": old_date.to_rfc3339(), "by": "tester", "branch": "main",
            "d": {"title": "Already Archived Task"}
        }),
        json!({
            "v": 1, "op": "complete", "id": "already-archived",
            "ts": old_date.to_rfc3339(), "by": "tester", "branch": "main",
            "d": {}
        }),
        json!({
            "v": 1, "op": "archive", "id": "already-archived",
            "ts": (old_date + Duration::days(1)).to_rfc3339(), "by": "@spool", "branch": "main",
            "d": {"ref": old_date.format("%Y-%m").to_string()}
        }),
    ];

    write_events(
        &spool_dir.join("events"),
        &format!("{}.jsonl", old_date.format("%Y-%m-%d")),
        &events,
    );

    let ctx = create_test_context(&spool_dir);
    let archived = spool::archive::archive_tasks(&ctx, 30, true).unwrap();

    // Task is already archived, so it shouldn't be returned again
    assert!(archived.is_empty());
}

#[test]
fn test_collect_all_events() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);

    let events1 = vec![
        json!({
            "v": 1, "op": "create", "id": "task-a",
            "ts": "2024-01-15T10:00:00Z", "by": "tester", "branch": "main",
            "d": {"title": "Task A"}
        }),
        json!({
            "v": 1, "op": "create", "id": "task-b",
            "ts": "2024-01-15T11:00:00Z", "by": "tester", "branch": "main",
            "d": {"title": "Task B"}
        }),
    ];

    let events2 = vec![json!({
        "v": 1, "op": "update", "id": "task-a",
        "ts": "2024-01-16T10:00:00Z", "by": "tester", "branch": "main",
        "d": {"title": "Task A Updated"}
    })];

    write_events(&spool_dir.join("events"), "2024-01-15.jsonl", &events1);
    write_events(&spool_dir.join("events"), "2024-01-16.jsonl", &events2);

    let ctx = create_test_context(&spool_dir);
    let all_events = spool::archive::collect_all_events(&ctx).unwrap();

    // Should have 2 tasks
    assert_eq!(all_events.len(), 2);

    // Task A should have 2 events (create + update)
    assert_eq!(all_events.get("task-a").unwrap().len(), 2);

    // Task B should have 1 event (create)
    assert_eq!(all_events.get("task-b").unwrap().len(), 1);
}

#[test]
fn test_archive_multiple_tasks_grouped_by_month() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);

    // Tasks completed in different months
    let month1 = Utc::now() - Duration::days(90);
    let month2 = Utc::now() - Duration::days(60);

    let events = vec![
        json!({
            "v": 1, "op": "create", "id": "task-month1",
            "ts": month1.to_rfc3339(), "by": "tester", "branch": "main",
            "d": {"title": "Month 1 Task"}
        }),
        json!({
            "v": 1, "op": "complete", "id": "task-month1",
            "ts": month1.to_rfc3339(), "by": "tester", "branch": "main",
            "d": {}
        }),
        json!({
            "v": 1, "op": "create", "id": "task-month2",
            "ts": month2.to_rfc3339(), "by": "tester", "branch": "main",
            "d": {"title": "Month 2 Task"}
        }),
        json!({
            "v": 1, "op": "complete", "id": "task-month2",
            "ts": month2.to_rfc3339(), "by": "tester", "branch": "main",
            "d": {}
        }),
    ];

    write_events(
        &spool_dir.join("events"),
        &format!("{}.jsonl", month1.format("%Y-%m-%d")),
        &events,
    );

    let ctx = create_test_context(&spool_dir);
    let archived = spool::archive::archive_tasks(&ctx, 30, false).unwrap();

    assert_eq!(archived.len(), 2);
    assert!(archived.contains(&"task-month1".to_string()));
    assert!(archived.contains(&"task-month2".to_string()));
}
