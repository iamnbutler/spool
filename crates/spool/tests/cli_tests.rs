use clap::Parser;
use spool::cli::{Cli, Commands, OutputFormat, StreamCommands};

#[test]
fn test_output_format_from_str() {
    assert_eq!(OutputFormat::from_str("table"), OutputFormat::Table);
    assert_eq!(OutputFormat::from_str("json"), OutputFormat::Json);
    assert_eq!(OutputFormat::from_str("ids"), OutputFormat::Ids);

    // Unknown format defaults to Table
    assert_eq!(OutputFormat::from_str("unknown"), OutputFormat::Table);
    assert_eq!(OutputFormat::from_str(""), OutputFormat::Table);
}

#[test]
fn test_cli_parse_init() {
    let cli = Cli::parse_from(["spool", "init"]);
    assert!(matches!(cli.command, Commands::Init));
}

#[test]
fn test_cli_parse_list_defaults() {
    let cli = Cli::parse_from(["spool", "list"]);

    if let Commands::List {
        status,
        assignee,
        tag,
        priority,
        stream,
        stream_name,
        no_stream,
        format,
    } = cli.command
    {
        assert_eq!(status, "open");
        assert!(assignee.is_none());
        assert!(tag.is_none());
        assert!(priority.is_none());
        assert!(stream.is_none());
        assert!(stream_name.is_none());
        assert!(!no_stream);
        assert_eq!(format, "table");
    } else {
        panic!("Expected List command");
    }
}

#[test]
fn test_cli_parse_list_with_filters() {
    let cli = Cli::parse_from([
        "spool",
        "list",
        "--status",
        "complete",
        "--assignee",
        "dev1",
        "--tag",
        "bug",
        "--priority",
        "p1",
        "--format",
        "json",
    ]);

    if let Commands::List {
        status,
        assignee,
        tag,
        priority,
        format,
        ..
    } = cli.command
    {
        assert_eq!(status, "complete");
        assert_eq!(assignee.as_deref(), Some("dev1"));
        assert_eq!(tag.as_deref(), Some("bug"));
        assert_eq!(priority.as_deref(), Some("p1"));
        assert_eq!(format, "json");
    } else {
        panic!("Expected List command");
    }
}

#[test]
fn test_cli_parse_list_short_flags() {
    let cli = Cli::parse_from([
        "spool", "list", "-s", "all", "-a", "user", "-t", "feature", "-p", "p2", "-f", "ids",
    ]);

    if let Commands::List {
        status,
        assignee,
        tag,
        priority,
        format,
        ..
    } = cli.command
    {
        assert_eq!(status, "all");
        assert_eq!(assignee.as_deref(), Some("user"));
        assert_eq!(tag.as_deref(), Some("feature"));
        assert_eq!(priority.as_deref(), Some("p2"));
        assert_eq!(format, "ids");
    } else {
        panic!("Expected List command");
    }
}

#[test]
fn test_cli_parse_list_no_stream() {
    let cli = Cli::parse_from(["spool", "list", "--no-stream"]);

    if let Commands::List {
        no_stream, stream, ..
    } = cli.command
    {
        assert!(no_stream);
        assert!(stream.is_none());
    } else {
        panic!("Expected List command");
    }
}

#[test]
fn test_cli_parse_show() {
    let cli = Cli::parse_from(["spool", "show", "task-123"]);

    if let Commands::Show { id, events } = cli.command {
        assert_eq!(id, "task-123");
        assert!(!events);
    } else {
        panic!("Expected Show command");
    }
}

#[test]
fn test_cli_parse_show_with_events() {
    let cli = Cli::parse_from(["spool", "show", "task-456", "--events"]);

    if let Commands::Show { id, events } = cli.command {
        assert_eq!(id, "task-456");
        assert!(events);
    } else {
        panic!("Expected Show command");
    }
}

#[test]
fn test_cli_parse_rebuild() {
    let cli = Cli::parse_from(["spool", "rebuild"]);
    assert!(matches!(cli.command, Commands::Rebuild));
}

#[test]
fn test_cli_parse_archive_defaults() {
    let cli = Cli::parse_from(["spool", "archive"]);

    if let Commands::Archive { days, dry_run } = cli.command {
        assert_eq!(days, 30);
        assert!(!dry_run);
    } else {
        panic!("Expected Archive command");
    }
}

#[test]
fn test_cli_parse_archive_with_options() {
    let cli = Cli::parse_from(["spool", "archive", "--days", "60", "--dry-run"]);

    if let Commands::Archive { days, dry_run } = cli.command {
        assert_eq!(days, 60);
        assert!(dry_run);
    } else {
        panic!("Expected Archive command");
    }
}

#[test]
fn test_cli_parse_archive_short_flag() {
    let cli = Cli::parse_from(["spool", "archive", "-d", "7"]);

    if let Commands::Archive { days, dry_run } = cli.command {
        assert_eq!(days, 7);
        assert!(!dry_run);
    } else {
        panic!("Expected Archive command");
    }
}

#[test]
fn test_cli_parse_validate() {
    let cli = Cli::parse_from(["spool", "validate"]);

    if let Commands::Validate { strict } = cli.command {
        assert!(!strict);
    } else {
        panic!("Expected Validate command");
    }
}

#[test]
fn test_cli_parse_validate_strict() {
    let cli = Cli::parse_from(["spool", "validate", "--strict"]);

    if let Commands::Validate { strict } = cli.command {
        assert!(strict);
    } else {
        panic!("Expected Validate command");
    }
}

#[test]
fn test_output_format_equality() {
    assert!(OutputFormat::Table == OutputFormat::Table);
    assert!(OutputFormat::Json == OutputFormat::Json);
    assert!(OutputFormat::Ids == OutputFormat::Ids);
    assert!(OutputFormat::Table != OutputFormat::Json);
}

#[test]
fn test_output_format_clone() {
    let format = OutputFormat::Json;
    let cloned = format;
    assert_eq!(format, cloned);
}

#[test]
fn test_cli_parse_complete_defaults() {
    let cli = Cli::parse_from(["spool", "complete", "task-123"]);

    if let Commands::Complete { id, resolution } = cli.command {
        assert_eq!(id, "task-123");
        assert_eq!(resolution, "done");
    } else {
        panic!("Expected Complete command");
    }
}

#[test]
fn test_cli_parse_complete_with_resolution() {
    let cli = Cli::parse_from(["spool", "complete", "task-456", "--resolution", "wontfix"]);

    if let Commands::Complete { id, resolution } = cli.command {
        assert_eq!(id, "task-456");
        assert_eq!(resolution, "wontfix");
    } else {
        panic!("Expected Complete command");
    }
}

#[test]
fn test_cli_parse_complete_short_flag() {
    let cli = Cli::parse_from(["spool", "complete", "task-789", "-r", "duplicate"]);

    if let Commands::Complete { id, resolution } = cli.command {
        assert_eq!(id, "task-789");
        assert_eq!(resolution, "duplicate");
    } else {
        panic!("Expected Complete command");
    }
}

#[test]
fn test_cli_parse_reopen() {
    let cli = Cli::parse_from(["spool", "reopen", "task-abc"]);

    if let Commands::Reopen { id } = cli.command {
        assert_eq!(id, "task-abc");
    } else {
        panic!("Expected Reopen command");
    }
}

#[test]
fn test_cli_parse_update_with_title() {
    let cli = Cli::parse_from(["spool", "update", "task-123", "--title", "New title"]);

    if let Commands::Update {
        id,
        title,
        description,
        priority,
        stream: _,
    } = cli.command
    {
        assert_eq!(id, "task-123");
        assert_eq!(title.as_deref(), Some("New title"));
        assert!(description.is_none());
        assert!(priority.is_none());
    } else {
        panic!("Expected Update command");
    }
}

#[test]
fn test_cli_parse_update_with_description() {
    let cli = Cli::parse_from([
        "spool",
        "update",
        "task-456",
        "--description",
        "Updated description",
    ]);

    if let Commands::Update {
        id,
        title,
        description,
        priority,
        stream: _,
    } = cli.command
    {
        assert_eq!(id, "task-456");
        assert!(title.is_none());
        assert_eq!(description.as_deref(), Some("Updated description"));
        assert!(priority.is_none());
    } else {
        panic!("Expected Update command");
    }
}

#[test]
fn test_cli_parse_update_with_priority() {
    let cli = Cli::parse_from(["spool", "update", "task-789", "--priority", "p0"]);

    if let Commands::Update {
        id,
        title,
        description,
        priority,
        stream: _,
    } = cli.command
    {
        assert_eq!(id, "task-789");
        assert!(title.is_none());
        assert!(description.is_none());
        assert_eq!(priority.as_deref(), Some("p0"));
    } else {
        panic!("Expected Update command");
    }
}

#[test]
fn test_cli_parse_update_all_fields() {
    let cli = Cli::parse_from([
        "spool",
        "update",
        "task-full",
        "--title",
        "Full update",
        "--description",
        "Complete description",
        "--priority",
        "p1",
    ]);

    if let Commands::Update {
        id,
        title,
        description,
        priority,
        stream: _,
    } = cli.command
    {
        assert_eq!(id, "task-full");
        assert_eq!(title.as_deref(), Some("Full update"));
        assert_eq!(description.as_deref(), Some("Complete description"));
        assert_eq!(priority.as_deref(), Some("p1"));
    } else {
        panic!("Expected Update command");
    }
}

#[test]
fn test_cli_parse_update_short_flags() {
    let cli = Cli::parse_from([
        "spool",
        "update",
        "task-short",
        "-t",
        "Short title",
        "-d",
        "Short desc",
        "-p",
        "p2",
    ]);

    if let Commands::Update {
        id,
        title,
        description,
        priority,
        stream: _,
    } = cli.command
    {
        assert_eq!(id, "task-short");
        assert_eq!(title.as_deref(), Some("Short title"));
        assert_eq!(description.as_deref(), Some("Short desc"));
        assert_eq!(priority.as_deref(), Some("p2"));
    } else {
        panic!("Expected Update command");
    }
}

#[test]
fn test_cli_parse_update_no_options() {
    let cli = Cli::parse_from(["spool", "update", "task-empty"]);

    if let Commands::Update {
        id,
        title,
        description,
        priority,
        stream: _,
    } = cli.command
    {
        assert_eq!(id, "task-empty");
        assert!(title.is_none());
        assert!(description.is_none());
        assert!(priority.is_none());
    } else {
        panic!("Expected Update command");
    }
}

#[test]
fn test_cli_parse_add_basic() {
    let cli = Cli::parse_from(["spool", "add", "My task title"]);

    if let Commands::Add {
        title,
        description,
        priority,
        assignee,
        tag,
        stream,
    } = cli.command
    {
        assert_eq!(title, "My task title");
        assert!(description.is_none());
        assert!(priority.is_none());
        assert!(assignee.is_none());
        assert!(tag.is_empty());
        assert!(stream.is_none());
    } else {
        panic!("Expected Add command");
    }
}

#[test]
fn test_cli_parse_add_with_all_options() {
    let cli = Cli::parse_from([
        "spool",
        "add",
        "Full task",
        "-d",
        "A description",
        "-p",
        "p1",
        "-a",
        "@alice",
        "-t",
        "bug",
        "-t",
        "urgent",
    ]);

    if let Commands::Add {
        title,
        description,
        priority,
        assignee,
        tag,
        stream: _,
    } = cli.command
    {
        assert_eq!(title, "Full task");
        assert_eq!(description.as_deref(), Some("A description"));
        assert_eq!(priority.as_deref(), Some("p1"));
        assert_eq!(assignee.as_deref(), Some("@alice"));
        assert_eq!(tag, vec!["bug", "urgent"]);
    } else {
        panic!("Expected Add command");
    }
}

#[test]
fn test_cli_parse_assign() {
    let cli = Cli::parse_from(["spool", "assign", "task-123", "@bob"]);

    if let Commands::Assign { id, assignee } = cli.command {
        assert_eq!(id, "task-123");
        assert_eq!(assignee, "@bob");
    } else {
        panic!("Expected Assign command");
    }
}

#[test]
fn test_cli_parse_claim() {
    let cli = Cli::parse_from(["spool", "claim", "task-456"]);

    if let Commands::Claim { id } = cli.command {
        assert_eq!(id, "task-456");
    } else {
        panic!("Expected Claim command");
    }
}

#[test]
fn test_cli_parse_free() {
    let cli = Cli::parse_from(["spool", "free", "task-789"]);

    if let Commands::Free { id } = cli.command {
        assert_eq!(id, "task-789");
    } else {
        panic!("Expected Free command");
    }
}

// Stream command tests

#[test]
fn test_cli_parse_stream_add() {
    let cli = Cli::parse_from(["spool", "stream", "add", "My Project"]);

    if let Commands::Stream { command } = cli.command {
        if let StreamCommands::Add { name, description } = command {
            assert_eq!(name, "My Project");
            assert!(description.is_none());
        } else {
            panic!("Expected Stream Add command");
        }
    } else {
        panic!("Expected Stream command");
    }
}

#[test]
fn test_cli_parse_stream_add_with_description() {
    let cli = Cli::parse_from([
        "spool",
        "stream",
        "add",
        "Backend Work",
        "-d",
        "API development tasks",
    ]);

    if let Commands::Stream { command } = cli.command {
        if let StreamCommands::Add { name, description } = command {
            assert_eq!(name, "Backend Work");
            assert_eq!(description.as_deref(), Some("API development tasks"));
        } else {
            panic!("Expected Stream Add command");
        }
    } else {
        panic!("Expected Stream command");
    }
}

#[test]
fn test_cli_parse_stream_list() {
    let cli = Cli::parse_from(["spool", "stream", "list"]);

    if let Commands::Stream { command } = cli.command {
        if let StreamCommands::List { format } = command {
            assert_eq!(format, "table");
        } else {
            panic!("Expected Stream List command");
        }
    } else {
        panic!("Expected Stream command");
    }
}

#[test]
fn test_cli_parse_stream_list_json() {
    let cli = Cli::parse_from(["spool", "stream", "list", "-f", "json"]);

    if let Commands::Stream { command } = cli.command {
        if let StreamCommands::List { format } = command {
            assert_eq!(format, "json");
        } else {
            panic!("Expected Stream List command");
        }
    } else {
        panic!("Expected Stream command");
    }
}

#[test]
fn test_cli_parse_stream_show() {
    let cli = Cli::parse_from(["spool", "stream", "show", "stream-123"]);

    if let Commands::Stream { command } = cli.command {
        if let StreamCommands::Show { id, name } = command {
            assert_eq!(id.as_deref(), Some("stream-123"));
            assert!(name.is_none());
        } else {
            panic!("Expected Stream Show command");
        }
    } else {
        panic!("Expected Stream command");
    }
}

#[test]
fn test_cli_parse_stream_show_by_name() {
    let cli = Cli::parse_from(["spool", "stream", "show", "--name", "my-stream"]);

    if let Commands::Stream { command } = cli.command {
        if let StreamCommands::Show { id, name } = command {
            assert!(id.is_none());
            assert_eq!(name.as_deref(), Some("my-stream"));
        } else {
            panic!("Expected Stream Show command");
        }
    } else {
        panic!("Expected Stream command");
    }
}

#[test]
fn test_cli_parse_stream_update() {
    let cli = Cli::parse_from(["spool", "stream", "update", "stream-456", "-n", "New Name"]);

    if let Commands::Stream { command } = cli.command {
        if let StreamCommands::Update {
            id,
            name,
            description,
        } = command
        {
            assert_eq!(id, "stream-456");
            assert_eq!(name.as_deref(), Some("New Name"));
            assert!(description.is_none());
        } else {
            panic!("Expected Stream Update command");
        }
    } else {
        panic!("Expected Stream command");
    }
}

#[test]
fn test_cli_parse_stream_update_all() {
    let cli = Cli::parse_from([
        "spool",
        "stream",
        "update",
        "stream-789",
        "-n",
        "Updated Name",
        "-d",
        "Updated description",
    ]);

    if let Commands::Stream { command } = cli.command {
        if let StreamCommands::Update {
            id,
            name,
            description,
        } = command
        {
            assert_eq!(id, "stream-789");
            assert_eq!(name.as_deref(), Some("Updated Name"));
            assert_eq!(description.as_deref(), Some("Updated description"));
        } else {
            panic!("Expected Stream Update command");
        }
    } else {
        panic!("Expected Stream command");
    }
}

#[test]
fn test_cli_parse_stream_delete() {
    let cli = Cli::parse_from(["spool", "stream", "delete", "stream-abc"]);

    if let Commands::Stream { command } = cli.command {
        if let StreamCommands::Delete { id } = command {
            assert_eq!(id, "stream-abc");
        } else {
            panic!("Expected Stream Delete command");
        }
    } else {
        panic!("Expected Stream command");
    }
}
