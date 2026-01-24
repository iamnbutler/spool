# Pull Request: Refactor stream API and add migration system (0.3.1 → 0.4.0)

## Overview
This PR refactors the stream command API for consistency and adds a migration system to handle breaking changes between versions.

## Key Changes

### 1. New Stream Subcommand Structure
**Before (0.3.1):**
- `spool stream <task-id> [name]` - Single command that added/removed tasks from streams

**After (0.4.0):**
- `spool stream list` - List all streams with task counts
- `spool stream show <name>` - Show all tasks in a specific stream
- `spool stream add <task-id> <name>` - Add a task to a stream
- `spool stream remove <task-id>` - Remove a task from its stream

### 2. Migration System
Added automatic migration from 0.3.1 to 0.4.0:
- New `src/migration.rs` module handles version detection and migration
- `.spool/.version` file tracks schema version (should be committed to git)
- Migration runs once automatically on first command after upgrade
- Displays clear breaking change message to users

### 3. Improved Documentation
- Clarified terminology (task/stream/spool)
- Added stream command documentation to CLI_GUIDE.md
- Updated README with new commands
- Created MIGRATION_0.4.0.md guide

### 4. Shell Mode Updates
Updated interactive shell to match new API:
- `stream list` - List streams
- `stream show <name>` - Show stream
- `stream add <id> <name>` - Add to stream
- `stream remove <id>` - Remove from stream

## Files Changed

### New Files
- `src/migration.rs` - Migration system and version tracking
- `MIGRATION_0.4.0.md` - User-facing migration guide
- `CHANGES_SUMMARY.md` - Technical summary

### Modified Files
- `src/cli.rs` - New stream subcommands and functions (+164 lines)
- `src/main.rs` - Migration check and stream command routing
- `src/context.rs` - Added `version_path()` method, init creates .version file
- `src/lib.rs` - Exported migration module
- `src/shell.rs` - Updated shell commands for new stream API
- `tests/cli_integration_tests.rs` - Added stream command tests, migration tests
- `README.md` - Updated command examples
- `docs/CLI_GUIDE.md` - Added stream commands section, terminology

## Testing
All tests pass ✅ (110 total):
- 17 unit tests (lib)
- 36 integration tests (CLI) 
- 57 other tests (state, events, validation, writer, context, archive)

New tests added:
- `test_stream_list_empty`
- `test_stream_list_shows_streams`
- `test_stream_show`
- `test_stream_show_empty`
- `test_stream_add`
- `test_stream_remove`
- `test_migration_creates_version_file`

## Breaking Changes ⚠️
The old `spool stream <task-id> [name]` command has been removed. Users must update their scripts/workflows to use the new subcommands.

**Non-breaking:** All `--stream` flags remain unchanged:
- `spool add --stream <name>`
- `spool list --stream <name>`
- `spool update --stream <name>`

## Migration Notes
- Migration is automatic and one-time
- No data format changes (events are unchanged)
- `.spool/.version` file should be committed to git
- Safe to rollback by removing .version file and downgrading binary

## Terminology Clarification
- **Task**: A unit of work to be completed
- **Stream**: A collection of tasks (workstream/project)
- **Spool**: The overall system managing tasks and streams

This makes the hierarchy clear: **Spool > Streams > Tasks**

---

**Note:** Version numbers were not manually edited per request - that will happen in a separate PR.

## How to Create PR

Visit: https://github.com/iamnbutler/spool/compare/main...01KFPS06SNJHE420CHX7D7D9G1

Or use GitHub CLI:
```bash
gh pr create --base main --head 01KFPS06SNJHE420CHX7D7D9G1 --title "Refactor stream API and add migration system (0.3.1 → 0.4.0)" --body "$(cat PR_DESCRIPTION.md)"
```
