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

/// Write raw lines to a file
fn write_lines(dir: &std::path::Path, filename: &str, lines: &[&str]) {
    let path = dir.join(filename);
    let mut file = fs::File::create(path).unwrap();
    for line in lines {
        writeln!(file, "{}", line).unwrap();
    }
}

#[test]
fn test_validation_empty_spool() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);

    let ctx = create_test_context(&spool_dir);
    let result = spool::validation::validate(&ctx, false).unwrap();

    assert!(result.errors.is_empty());
    assert!(result.warnings.is_empty());
}

#[test]
fn test_validation_valid_events() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);

    let events = vec![
        json!({
            "v": 1, "op": "create", "id": "task-001",
            "ts": "2024-01-15T10:00:00Z", "by": "tester", "branch": "main",
            "d": {"title": "Valid Task"}
        }),
        json!({
            "v": 1, "op": "update", "id": "task-001",
            "ts": "2024-01-15T11:00:00Z", "by": "tester", "branch": "main",
            "d": {"title": "Updated Task"}
        }),
    ];

    write_events(&spool_dir.join("events"), "2024-01-15.jsonl", &events);

    let ctx = create_test_context(&spool_dir);
    let result = spool::validation::validate(&ctx, false).unwrap();

    assert!(result.errors.is_empty());
    assert!(result.warnings.is_empty());
}

#[test]
fn test_validation_invalid_json() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);

    write_lines(
        &spool_dir.join("events"),
        "2024-01-15.jsonl",
        &[
            "not valid json at all",
            r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"tester","branch":"main","d":{}}"#,
        ],
    );

    let ctx = create_test_context(&spool_dir);
    let result = spool::validation::validate(&ctx, false).unwrap();

    assert!(!result.errors.is_empty());
    assert!(result.errors.iter().any(|e| e.contains("Invalid JSON")));
}

#[test]
fn test_validation_missing_required_field() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);

    // Event missing 'by' field
    write_lines(
        &spool_dir.join("events"),
        "2024-01-15.jsonl",
        &[
            r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","branch":"main","d":{}}"#,
        ],
    );

    let ctx = create_test_context(&spool_dir);
    let result = spool::validation::validate(&ctx, false).unwrap();

    assert!(!result.errors.is_empty());
    assert!(result
        .errors
        .iter()
        .any(|e| e.contains("Missing required field 'by'")));
}

#[test]
fn test_validation_invalid_timestamp() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);

    write_lines(
        &spool_dir.join("events"),
        "2024-01-15.jsonl",
        &[
            r#"{"v":1,"op":"create","id":"task-001","ts":"not-a-timestamp","by":"tester","branch":"main","d":{}}"#,
        ],
    );

    let ctx = create_test_context(&spool_dir);
    let result = spool::validation::validate(&ctx, false).unwrap();

    assert!(!result.errors.is_empty());
    assert!(result
        .errors
        .iter()
        .any(|e| e.contains("Invalid timestamp")));
}

#[test]
fn test_validation_unknown_schema_version() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);

    // Schema version 99 instead of 1
    write_lines(
        &spool_dir.join("events"),
        "2024-01-15.jsonl",
        &[
            r#"{"v":99,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"tester","branch":"main","d":{}}"#,
        ],
    );

    let ctx = create_test_context(&spool_dir);
    let result = spool::validation::validate(&ctx, false).unwrap();

    assert!(!result.warnings.is_empty());
    assert!(result
        .warnings
        .iter()
        .any(|w| w.contains("Unknown schema version")));
}

#[test]
fn test_validation_event_before_create() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);

    // Update event without preceding create
    write_lines(
        &spool_dir.join("events"),
        "2024-01-15.jsonl",
        &[
            r#"{"v":1,"op":"update","id":"task-orphan","ts":"2024-01-15T10:00:00Z","by":"tester","branch":"main","d":{}}"#,
        ],
    );

    let ctx = create_test_context(&spool_dir);
    let result = spool::validation::validate(&ctx, false).unwrap();

    assert!(!result.warnings.is_empty());
    assert!(result.warnings.iter().any(|w| w.contains("before create")));
}

#[test]
fn test_validation_duplicate_create() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);

    let events = vec![
        json!({
            "v": 1, "op": "create", "id": "task-dup",
            "ts": "2024-01-15T10:00:00Z", "by": "tester", "branch": "main",
            "d": {}
        }),
        json!({
            "v": 1, "op": "create", "id": "task-dup",
            "ts": "2024-01-15T11:00:00Z", "by": "tester", "branch": "main",
            "d": {}
        }),
    ];

    write_events(&spool_dir.join("events"), "2024-01-15.jsonl", &events);

    let ctx = create_test_context(&spool_dir);
    let result = spool::validation::validate(&ctx, false).unwrap();

    assert!(!result.warnings.is_empty());
    assert!(result
        .warnings
        .iter()
        .any(|w| w.contains("Duplicate create")));
}

#[test]
fn test_validation_orphaned_blocked_by() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);

    let events = vec![json!({
        "v": 1, "op": "create", "id": "task-ref",
        "ts": "2024-01-15T10:00:00Z", "by": "tester", "branch": "main",
        "d": {"title": "Task with orphan ref", "blocked_by": ["nonexistent-task"]}
    })];

    write_events(&spool_dir.join("events"), "2024-01-15.jsonl", &events);

    let ctx = create_test_context(&spool_dir);
    let result = spool::validation::validate(&ctx, false).unwrap();

    assert!(!result.warnings.is_empty());
    assert!(result
        .warnings
        .iter()
        .any(|w| w.contains("non-existent blocked_by")));
}

#[test]
fn test_validation_strict_mode_errors() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);

    // Invalid JSON will cause an error
    write_lines(
        &spool_dir.join("events"),
        "2024-01-15.jsonl",
        &["invalid json"],
    );

    let ctx = create_test_context(&spool_dir);
    let result = spool::validation::validate(&ctx, true);

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("errors"));
}

#[test]
fn test_validation_strict_mode_warnings() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);

    // Update without create causes a warning
    write_lines(
        &spool_dir.join("events"),
        "2024-01-15.jsonl",
        &[
            r#"{"v":1,"op":"update","id":"orphan","ts":"2024-01-15T10:00:00Z","by":"tester","branch":"main","d":{}}"#,
        ],
    );

    let ctx = create_test_context(&spool_dir);
    let result = spool::validation::validate(&ctx, true);

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("warnings"));
}

#[test]
fn test_validation_empty_lines_ignored() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);

    write_lines(
        &spool_dir.join("events"),
        "2024-01-15.jsonl",
        &[
            "",
            r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"tester","branch":"main","d":{}}"#,
            "   ",
            "",
        ],
    );

    let ctx = create_test_context(&spool_dir);
    let result = spool::validation::validate(&ctx, false).unwrap();

    assert!(result.errors.is_empty());
}

#[test]
fn test_validation_archive_files() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);

    // Valid archived events
    let events = vec![json!({
        "v": 1, "op": "create", "id": "archived-task",
        "ts": "2023-12-01T10:00:00Z", "by": "tester", "branch": "main",
        "d": {"title": "Archived Task"}
    })];

    write_events(&spool_dir.join("archive"), "2023-12.jsonl", &events);

    let ctx = create_test_context(&spool_dir);
    let result = spool::validation::validate(&ctx, false).unwrap();

    assert!(result.errors.is_empty());
    assert!(result.warnings.is_empty());
}
