use chrono::Utc;
use spool::event::{Event, Operation};
use serde_json::json;

#[test]
fn test_operation_serialization() {
    let ops = vec![
        (Operation::Create, "create"),
        (Operation::Update, "update"),
        (Operation::Assign, "assign"),
        (Operation::Comment, "comment"),
        (Operation::Link, "link"),
        (Operation::Unlink, "unlink"),
        (Operation::Complete, "complete"),
        (Operation::Reopen, "reopen"),
        (Operation::Archive, "archive"),
    ];

    for (op, expected_str) in ops {
        let json = serde_json::to_string(&op).unwrap();
        assert_eq!(json, format!("\"{}\"", expected_str));
    }
}

#[test]
fn test_operation_deserialization() {
    let cases = vec![
        ("\"create\"", Operation::Create),
        ("\"update\"", Operation::Update),
        ("\"assign\"", Operation::Assign),
        ("\"comment\"", Operation::Comment),
        ("\"link\"", Operation::Link),
        ("\"unlink\"", Operation::Unlink),
        ("\"complete\"", Operation::Complete),
        ("\"reopen\"", Operation::Reopen),
        ("\"archive\"", Operation::Archive),
    ];

    for (json_str, expected_op) in cases {
        let op: Operation = serde_json::from_str(json_str).unwrap();
        assert_eq!(op, expected_op);
    }
}

#[test]
fn test_operation_display() {
    assert_eq!(Operation::Create.to_string(), "create");
    assert_eq!(Operation::Update.to_string(), "update");
    assert_eq!(Operation::Assign.to_string(), "assign");
    assert_eq!(Operation::Comment.to_string(), "comment");
    assert_eq!(Operation::Link.to_string(), "link");
    assert_eq!(Operation::Unlink.to_string(), "unlink");
    assert_eq!(Operation::Complete.to_string(), "complete");
    assert_eq!(Operation::Reopen.to_string(), "reopen");
    assert_eq!(Operation::Archive.to_string(), "archive");
}

#[test]
fn test_event_serialization_roundtrip() {
    let event = Event {
        v: 1,
        op: Operation::Create,
        id: "task-123".to_string(),
        ts: Utc::now(),
        by: "user@example.com".to_string(),
        branch: "main".to_string(),
        d: json!({"title": "Test task", "priority": "p2"}),
    };

    let json_str = serde_json::to_string(&event).unwrap();
    let parsed: Event = serde_json::from_str(&json_str).unwrap();

    assert_eq!(parsed.v, event.v);
    assert_eq!(parsed.op, event.op);
    assert_eq!(parsed.id, event.id);
    assert_eq!(parsed.by, event.by);
    assert_eq!(parsed.branch, event.branch);
    assert_eq!(parsed.d["title"], event.d["title"]);
}

#[test]
fn test_event_from_json_line() {
    let json_line = r#"{"v":1,"op":"create","id":"abc-1234","ts":"2024-01-15T10:30:00Z","by":"test@example.com","branch":"feature","d":{"title":"My Task"}}"#;

    let event: Event = serde_json::from_str(json_line).unwrap();

    assert_eq!(event.v, 1);
    assert_eq!(event.op, Operation::Create);
    assert_eq!(event.id, "abc-1234");
    assert_eq!(event.by, "test@example.com");
    assert_eq!(event.branch, "feature");
    assert_eq!(event.d["title"].as_str().unwrap(), "My Task");
}

#[test]
fn test_event_with_all_data_fields() {
    let event = Event {
        v: 1,
        op: Operation::Create,
        id: "full-task".to_string(),
        ts: Utc::now(),
        by: "author".to_string(),
        branch: "main".to_string(),
        d: json!({
            "title": "Complete Task",
            "description": "A detailed description",
            "priority": "p1",
            "tags": ["bug", "urgent"],
            "assignee": "developer",
            "parent": "parent-123",
            "blocks": ["task-a", "task-b"],
            "blocked_by": ["task-c"]
        }),
    };

    let json_str = serde_json::to_string(&event).unwrap();
    let parsed: Event = serde_json::from_str(&json_str).unwrap();

    assert_eq!(parsed.d["tags"].as_array().unwrap().len(), 2);
    assert_eq!(parsed.d["blocks"].as_array().unwrap().len(), 2);
    assert_eq!(parsed.d["blocked_by"].as_array().unwrap().len(), 1);
}
