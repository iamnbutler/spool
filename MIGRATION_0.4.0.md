# Migration to 0.4.0

This document describes the changes in version 0.4.0 and how to migrate from 0.3.1.

## What's New in 0.4.0

### Consistent Stream API

The stream command API has been redesigned to be more consistent with other commands:

**Old (0.3.1):**
```bash
spool stream <task-id> [name]    # Add or remove task from stream
```

**New (0.4.0):**
```bash
spool stream list                # List all streams
spool stream show <name>         # Show tasks in a stream
spool stream add <task-id> <name>  # Add task to stream
spool stream remove <task-id>    # Remove task from stream
```

### Improved Terminology

Documentation now clearly defines:
- **Task**: A unit of work
- **Stream**: A collection of tasks (workstream/project)
- **Spool**: The overall system managing tasks and streams

### Version Tracking

A new `.spool/.version` file tracks the schema version. This enables:
- Automatic migration detection
- Version compatibility checks
- Future schema upgrades

## Migration Process

### Automatic Migration

The migration happens automatically on the first command you run after upgrading:

```bash
$ spool list
Detected spool data without version marker. Assuming version 0.3.1.
Migrating spool from version 0.3.1 to 0.4.0...
Migration to 0.4.0 complete!

BREAKING CHANGE: The 'spool stream' command syntax has changed:
  Old: spool stream <task-id> [name]
  New: spool stream add <task-id> <name>
       spool stream remove <task-id>
       spool stream list
       spool stream show <name>
```

### What Changes During Migration

1. Creates `.spool/.version` file with content `0.4.0`
2. No data format changes (events remain unchanged)
3. Only the CLI API changes

### For Fresh Installations

New `spool init` automatically creates the `.version` file, so no migration is needed.

## Breaking Changes

### Removed Command

The old `spool stream <task-id> [name]` command has been removed.

**Migration:**
- Replace `spool stream <id> <name>` with `spool stream add <id> <name>`
- Replace `spool stream <id>` (to remove) with `spool stream remove <id>`

### Examples

**Adding a task to a stream:**
```bash
# Old (0.3.1)
spool stream task-abc123 backend

# New (0.4.0)
spool stream add task-abc123 backend
```

**Removing a task from a stream:**
```bash
# Old (0.3.1)
spool stream task-abc123

# New (0.4.0)
spool stream remove task-abc123
```

## Non-Breaking Changes

The following commands still work exactly as before:
- `spool add --stream <name>` - Create task in a stream
- `spool list --stream <name>` - Filter tasks by stream
- `spool update --stream <name>` - Move task to stream

## Rollback

If you need to rollback to 0.3.1:
1. The data format is unchanged, so downgrading the binary is safe
2. Delete the `.spool/.version` file
3. Avoid using the new `stream list` and `stream show` commands

Note: Any usage of new commands (stream list/show) won't cause data corruption, they just won't work in 0.3.1.
