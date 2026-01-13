# Changelog

All notable changes to Fabric will be documented in this file.

## [0.1.0] - 2026-01-13

### Added

Initial release of Fabric, a git-native task management system.

#### Core Features

- **Event-sourced architecture**: All task data stored as append-only JSONL event logs
- **Git-native**: Events are committed with your code, enabling branch-aware workflows
- **Full history**: Every change to every task is preserved and auditable

#### Commands

- `fabric init` - Initialize `.fabric/` directory structure in a repository
- `fabric list` - List tasks with filtering by status, assignee, tag, or priority
- `fabric show` - Display detailed task information with optional event history
- `fabric rebuild` - Regenerate index and state caches from event logs
- `fabric archive` - Move completed tasks to monthly archive files
- `fabric validate` - Check event files for correctness and consistency

#### Task Operations Supported

- Create tasks with title, description, priority, assignee, and tags
- Update task metadata
- Assign/unassign tasks
- Add comments with optional references
- Link tasks (blocks, blocked_by, parent relationships)
- Complete and reopen tasks
- Archive old completed tasks

#### Output Formats

- Table format (default) for human-readable output
- JSON format for programmatic access
- IDs-only format for shell scripting

### Technical Details

- Written in Rust for performance and reliability
- Uses serde for JSON serialization
- Integrates with git for branch detection and author identification
- Generates unique task IDs using timestamp + random suffix

### Platforms

- macOS (x86_64, aarch64)
- Linux (x86_64)

### Documentation

- Comprehensive CLI user guide
- Event schema reference
- Git integration best practices
