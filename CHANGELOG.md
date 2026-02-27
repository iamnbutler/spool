# Changelog

All notable changes to Spool will be documented in this file.

## [1.2.1] - 2026-02-27

### Fixed
- Replaced `file_name().unwrap()` with proper error handling in `state.rs` and `validation.rs`, eliminating potential panics when processing tasks with non-UTF-8 or unusual file paths (#41)

### Changed
- Updated dependencies (`cargo update` 2026-02-26) (#42)

## [1.0.0] - 2026-01-25

### Added
- **spool-ui**: Terminal UI for spool using ratatui
  - Task list with priority coloring and status markers
  - Detail panel with event history
  - Status filtering (open/complete/all)
  - Sorting (priority/created/title)
  - Search (title, description, tags)
  - Stream navigation
  - Inline task creation and completion
- Workspace structure: `spool` (lib), `spool-cli` (CLI binary), `spool-ui` (TUI binary)

### Changed
- **Breaking**: Reorganized as cargo workspace with separate crates
- Removed shell command (replaced by TUI)

### TUI Keybindings
| Key | Action |
|-----|--------|
| `j/k` | Navigate tasks (or scroll detail when focused) |
| `c` | Complete task |
| `r` | Reopen task |
| `n` | New task |
| `v` | Cycle view (Open/Complete/All) |
| `s` | Cycle sort |
| `S` | Cycle stream |
| `/` | Search |
| `q` | Quit |

## [0.4.0] - 2026-01-24

### Added
- **Streams as first-class entities**: Streams are now proper entities with IDs, names, descriptions, and metadata
- `spool stream add <name>` - Create a new stream
- `spool stream list` - List all streams with task counts
- `spool stream show <id>` - Show stream details and tasks
- `spool stream show --name <name>` - Look up stream by name
- `spool stream update <id>` - Update stream name/description
- `spool stream delete <id>` - Delete empty streams
- `--no-stream` flag on `spool list` to filter orphaned tasks
- Migration system for upgrading from 0.3.x to 0.4.0
- New event operations: `create_stream`, `update_stream`, `delete_stream`

### Changed
- **Breaking**: Stream API restructured as subcommands instead of `spool stream <task-id> [name]`
- Tasks now reference streams by ID instead of name
- Stream existence is validated when assigning tasks

### Migration
- Existing implicit streams (task stream names) are automatically converted to stream entities on first run
- The migration creates `CreateStream` events for each unique stream name found
- Tasks are updated to reference stream IDs

## [0.3.1] - 2026-01-19

### Added
- `spool stream <id> [name]` command to move tasks between streams
- `--stream` flag on `spool update` command
- Stream field displayed in `spool show` output
- Shell: `stream` command and `--stream` flag on `update`

## [0.3.0] - 2026-01-19

### Added
- Repository field in Cargo.toml for crates.io linking

### Changed
- Refactored lint suppressions into proper named structs (`TaskIndexBuilder`, `AddArgs`, `ListArgs`)
- Consolidated `scripts/` into `script/` directory
- Renamed misleading `md5_hash()` to `simple_hash()` in concurrency module

### Fixed
- Removed duplicate `get_current_branch()` function (now imports from writer module)
- Removed unused `_seen_ids` parameter from validation
- Updated deprecated `actions-rs/toolchain` to `dtolnay/rust-toolchain` in CI
- Added missing `SetStream` operation test coverage
- Fixed benchmark Task creation to include `stream` field

## [0.2.0] - 2026-01-18

### Added
- **Streams**: Group tasks into collections with `--stream` flag
  - `spool add "Task" --stream my-stream` - Create task in a stream
  - `spool list --stream my-stream` - Filter tasks by stream
  - `set_stream` operation to move tasks between streams
- **CLI commands** promoted from shell-only:
  - `spool add` - Create tasks directly from CLI
  - `spool assign <id> @user` - Assign task to a user
  - `spool claim <id>` - Assign task to yourself
  - `spool free <id>` - Unassign task
- `CreateTaskParams` struct for cleaner task creation API

### Changed
- Shell now uses named argument structs instead of tuples

## [0.1.0] - 2026-01-13

### Added

Initial release of Spool, a git-native task management system.

#### Core Features

- **Event-sourced architecture**: All task data stored as append-only JSONL event logs
- **Git-native**: Events are committed with your code, enabling branch-aware workflows
- **Full history**: Every change to every task is preserved and auditable

#### Commands

- `spool init` - Initialize `.spool/` directory structure in a repository
- `spool list` - List tasks with filtering by status, assignee, tag, or priority
- `spool show` - Display detailed task information with optional event history
- `spool rebuild` - Regenerate index and state caches from event logs
- `spool archive` - Move completed tasks to monthly archive files
- `spool validate` - Check event files for correctness and consistency

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
