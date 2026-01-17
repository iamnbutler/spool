use clap::Parser;
use spool::cli::{Cli, Commands, OutputFormat};

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
        format,
    } = cli.command
    {
        assert_eq!(status, "open");
        assert!(assignee.is_none());
        assert!(tag.is_none());
        assert!(priority.is_none());
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
fn test_cli_parse_shell() {
    let cli = Cli::parse_from(["spool", "shell"]);
    assert!(matches!(cli.command, Commands::Shell));
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
    } = cli.command
    {
        assert_eq!(title, "My task title");
        assert!(description.is_none());
        assert!(priority.is_none());
        assert!(assignee.is_none());
        assert!(tag.is_empty());
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
