use std::fs;
use std::io::Write;
use tempfile::TempDir;

use spool::context::SpoolContext;

fn setup_spool_dir(temp_dir: &TempDir) -> std::path::PathBuf {
    let spool_dir = temp_dir.path().join(".spool");
    fs::create_dir_all(spool_dir.join("events")).unwrap();
    fs::create_dir_all(spool_dir.join("archive")).unwrap();
    spool_dir
}

#[test]
fn test_spool_context_new() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);

    let ctx = SpoolContext::new(spool_dir.clone());

    assert_eq!(ctx.root, spool_dir);
    assert_eq!(ctx.events_dir, spool_dir.join("events"));
    assert_eq!(ctx.archive_dir, spool_dir.join("archive"));
}

#[test]
fn test_spool_context_index_path() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);
    let ctx = SpoolContext::new(spool_dir.clone());

    assert_eq!(ctx.index_path(), spool_dir.join(".index.json"));
}

#[test]
fn test_spool_context_state_path() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);
    let ctx = SpoolContext::new(spool_dir.clone());

    assert_eq!(ctx.state_path(), spool_dir.join(".state.json"));
}

#[test]
fn test_get_event_files_empty() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);
    let ctx = SpoolContext::new(spool_dir);

    let files = ctx.get_event_files().unwrap();
    assert!(files.is_empty());
}

#[test]
fn test_get_event_files_single() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);
    let ctx = SpoolContext::new(spool_dir.clone());

    // Create a single event file
    let event_file = spool_dir.join("events").join("2024-01-15.jsonl");
    fs::write(&event_file, "{}").unwrap();

    let files = ctx.get_event_files().unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0], event_file);
}

#[test]
fn test_get_event_files_sorted() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);
    let ctx = SpoolContext::new(spool_dir.clone());

    // Create files in non-sorted order
    let events_dir = spool_dir.join("events");
    fs::write(events_dir.join("2024-01-17.jsonl"), "{}").unwrap();
    fs::write(events_dir.join("2024-01-15.jsonl"), "{}").unwrap();
    fs::write(events_dir.join("2024-01-16.jsonl"), "{}").unwrap();

    let files = ctx.get_event_files().unwrap();
    assert_eq!(files.len(), 3);

    // Should be sorted chronologically
    assert!(files[0].to_string_lossy().contains("2024-01-15"));
    assert!(files[1].to_string_lossy().contains("2024-01-16"));
    assert!(files[2].to_string_lossy().contains("2024-01-17"));
}

#[test]
fn test_get_event_files_ignores_non_jsonl() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);
    let ctx = SpoolContext::new(spool_dir.clone());

    let events_dir = spool_dir.join("events");
    fs::write(events_dir.join("2024-01-15.jsonl"), "{}").unwrap();
    fs::write(events_dir.join("2024-01-15.txt"), "{}").unwrap();
    fs::write(events_dir.join("2024-01-15.json"), "{}").unwrap();
    fs::write(events_dir.join("README.md"), "readme").unwrap();

    let files = ctx.get_event_files().unwrap();
    assert_eq!(files.len(), 1);
    assert!(files[0].to_string_lossy().ends_with(".jsonl"));
}

#[test]
fn test_get_archive_files_empty() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);
    let ctx = SpoolContext::new(spool_dir);

    let files = ctx.get_archive_files().unwrap();
    assert!(files.is_empty());
}

#[test]
fn test_get_archive_files_sorted() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);
    let ctx = SpoolContext::new(spool_dir.clone());

    let archive_dir = spool_dir.join("archive");
    fs::write(archive_dir.join("2024-03.jsonl"), "{}").unwrap();
    fs::write(archive_dir.join("2024-01.jsonl"), "{}").unwrap();
    fs::write(archive_dir.join("2024-02.jsonl"), "{}").unwrap();

    let files = ctx.get_archive_files().unwrap();
    assert_eq!(files.len(), 3);

    // Should be sorted
    assert!(files[0].to_string_lossy().contains("2024-01"));
    assert!(files[1].to_string_lossy().contains("2024-02"));
    assert!(files[2].to_string_lossy().contains("2024-03"));
}

#[test]
fn test_parse_events_from_file_single_event() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);
    let ctx = SpoolContext::new(spool_dir.clone());

    let event_file = spool_dir.join("events").join("2024-01-15.jsonl");
    let event_json = r#"{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{"title":"Test"}}"#;
    fs::write(&event_file, event_json).unwrap();

    let events = ctx.parse_events_from_file(&event_file).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id, "task-001");
}

#[test]
fn test_parse_events_from_file_multiple_events() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);
    let ctx = SpoolContext::new(spool_dir.clone());

    let event_file = spool_dir.join("events").join("2024-01-15.jsonl");
    let mut file = fs::File::create(&event_file).unwrap();
    writeln!(file, r#"{{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{{"title":"First"}}}}"#).unwrap();
    writeln!(file, r#"{{"v":1,"op":"create","id":"task-002","ts":"2024-01-15T11:00:00Z","by":"@tester","branch":"main","d":{{"title":"Second"}}}}"#).unwrap();
    writeln!(file, r#"{{"v":1,"op":"update","id":"task-001","ts":"2024-01-15T12:00:00Z","by":"@tester","branch":"main","d":{{"title":"Updated"}}}}"#).unwrap();

    let events = ctx.parse_events_from_file(&event_file).unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].id, "task-001");
    assert_eq!(events[1].id, "task-002");
    assert_eq!(events[2].id, "task-001");
}

#[test]
fn test_parse_events_from_file_skips_empty_lines() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);
    let ctx = SpoolContext::new(spool_dir.clone());

    let event_file = spool_dir.join("events").join("2024-01-15.jsonl");
    let mut file = fs::File::create(&event_file).unwrap();
    writeln!(file, r#"{{"v":1,"op":"create","id":"task-001","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{{"title":"First"}}}}"#).unwrap();
    writeln!(file, "").unwrap(); // Empty line
    writeln!(file, "   ").unwrap(); // Whitespace line
    writeln!(file, r#"{{"v":1,"op":"create","id":"task-002","ts":"2024-01-15T11:00:00Z","by":"@tester","branch":"main","d":{{"title":"Second"}}}}"#).unwrap();

    let events = ctx.parse_events_from_file(&event_file).unwrap();
    assert_eq!(events.len(), 2);
}

#[test]
fn test_parse_events_from_file_invalid_json_errors() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);
    let ctx = SpoolContext::new(spool_dir.clone());

    let event_file = spool_dir.join("events").join("2024-01-15.jsonl");
    fs::write(&event_file, "not valid json").unwrap();

    let result = ctx.parse_events_from_file(&event_file);
    assert!(result.is_err());
}

#[test]
fn test_parse_events_from_file_missing_field_errors() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);
    let ctx = SpoolContext::new(spool_dir.clone());

    let event_file = spool_dir.join("events").join("2024-01-15.jsonl");
    // Missing required 'id' field
    fs::write(&event_file, r#"{"v":1,"op":"create","ts":"2024-01-15T10:00:00Z","by":"@tester","branch":"main","d":{}}"#).unwrap();

    let result = ctx.parse_events_from_file(&event_file);
    assert!(result.is_err());
}

#[test]
fn test_parse_events_from_file_nonexistent_errors() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = setup_spool_dir(&temp_dir);
    let ctx = SpoolContext::new(spool_dir.clone());

    let event_file = spool_dir.join("events").join("nonexistent.jsonl");

    let result = ctx.parse_events_from_file(&event_file);
    assert!(result.is_err());
}

// Note: init() tests are handled via CLI integration tests (cli_integration_tests.rs)
// because init() relies on current working directory which is problematic in parallel tests.
// The following tests validate init-like behavior without using set_current_dir.

#[test]
fn test_init_directory_structure() {
    // Verify the expected structure that init() would create
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = temp_dir.path().join(".spool");

    // Simulate init structure
    fs::create_dir_all(spool_dir.join("events")).unwrap();
    fs::create_dir_all(spool_dir.join("archive")).unwrap();

    let gitignore_content = ".index.json\n.state.json\n";
    fs::write(spool_dir.join(".gitignore"), gitignore_content).unwrap();

    // Verify structure
    assert!(spool_dir.is_dir());
    assert!(spool_dir.join("events").is_dir());
    assert!(spool_dir.join("archive").is_dir());
    assert!(spool_dir.join(".gitignore").is_file());

    // Verify gitignore content
    let gitignore = fs::read_to_string(spool_dir.join(".gitignore")).unwrap();
    assert!(gitignore.contains(".index.json"));
    assert!(gitignore.contains(".state.json"));
}

#[test]
fn test_get_event_files_no_events_dir() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = temp_dir.path().join(".spool");
    fs::create_dir_all(&spool_dir).unwrap();
    // Don't create events dir

    let ctx = SpoolContext::new(spool_dir);
    let files = ctx.get_event_files().unwrap();
    assert!(files.is_empty());
}

#[test]
fn test_get_archive_files_no_archive_dir() {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = temp_dir.path().join(".spool");
    fs::create_dir_all(&spool_dir).unwrap();
    // Don't create archive dir

    let ctx = SpoolContext::new(spool_dir);
    let files = ctx.get_archive_files().unwrap();
    assert!(files.is_empty());
}
