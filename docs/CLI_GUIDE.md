# Spool CLI User Guide

Spool is a git-native task management system that stores all task data as append-only event logs in your repository. Tasks are tracked through events, enabling full history, branch-aware workflows, and seamless git integration.

## Table of Contents

- [Quick Start](#quick-start)
- [Core Concepts](#core-concepts)
- [Commands](#commands)
- [Event Schema](#event-schema)
- [Git Integration](#git-integration)
- [Workflows](#workflows)

## Quick Start

```bash
# Initialize spool in your repository
spool init

# List open tasks
spool list

# Show task details
spool show <task-id>

# Validate event files
spool validate

# Rebuild index and state from events
spool rebuild

# Archive old completed tasks
spool archive --days 30
```

## Core Concepts

### Event Sourcing

Spool uses event sourcing as its core architecture. Instead of storing mutable task records, every change is recorded as an immutable event. This provides:

- **Full history**: See how any task evolved over time
- **Branch-aware**: Events include branch information for merge conflict resolution
- **Audit trail**: Know who changed what and when
- **Reproducibility**: State can be rebuilt from events at any point

### Directory Structure

After running `spool init`, your repository will contain:

```
.spool/
├── events/           # Daily event logs (YYYY-MM-DD.jsonl)
├── archive/          # Monthly rollups of completed tasks
├── .index.json       # Task index cache (gitignored)
├── .state.json       # Materialized state cache (gitignored)
└── .gitignore        # Ignores derived files
```

**Committed files:**
- `events/*.jsonl` - Source of truth for all task data
- `archive/*.jsonl` - Archived task events
- `.version` - Schema version marker

**Derived files (gitignored):**
- `.index.json` - Maps task IDs to metadata for fast lookups
- `.state.json` - Current snapshot of all tasks

### Terminology

- **Task**: A unit of work to be completed
- **Stream**: A collection of related tasks, also known as a "workstream" or "project"
- **Spool**: The overall system managing all tasks and streams

### Task Lifecycle

Tasks progress through these states:

```
Create → Open → Complete → Archive
              ↓         ↑
              → Reopen →
```

## Commands

### Task Commands

### `spool init`

Initialize the `.spool/` directory structure in the current repository.

```bash
spool init
```

Creates:
- `.spool/events/` - Directory for daily event logs
- `.spool/archive/` - Directory for monthly archives
- `.spool/.gitignore` - Ignores derived cache files

**Note:** Run this once at the root of your repository.

### `spool list`

List tasks with optional filtering.

```bash
spool list [OPTIONS]
```

**Options:**
| Flag | Description | Default |
|------|-------------|---------|
| `-s, --status` | Filter by status: `open`, `complete`, or `all` | `open` |
| `-a, --assignee` | Filter by assignee | - |
| `-t, --tag` | Filter by tag | - |
| `-p, --priority` | Filter by priority | - |
| `-f, --format` | Output format: `table`, `json`, or `ids` | `table` |

**Examples:**

```bash
# List all open tasks (default)
spool list

# List completed tasks
spool list --status complete

# List tasks assigned to @alice
spool list --assignee @alice

# List high-priority tasks as JSON
spool list --priority high --format json

# Get just task IDs for scripting
spool list --format ids
```

### `spool show`

Display detailed information about a specific task.

```bash
spool show <task-id> [OPTIONS]
```

**Options:**
| Flag | Description |
|------|-------------|
| `--events` | Show raw event history |

**Output includes:**
- ID, title, status
- Priority, assignee, tags
- Description (if any)
- Creation info (timestamp, author, branch)
- Update timestamp
- Completion info (if complete)
- Parent/blocking relationships
- Comments

**Examples:**

```bash
# Show task details
spool show k8b2x-a1c3

# Show task with full event history
spool show k8b2x-a1c3 --events
```

### `spool rebuild`

Rebuild the `.index.json` and `.state.json` cache files from event logs.

```bash
spool rebuild
```

**When to use:**
- After cloning a repository with spool
- After pulling changes that include new events
- If cache files become corrupted
- After resolving merge conflicts in event files

**Git hook integration:**
```bash
# .git/hooks/post-merge
#!/bin/sh
spool rebuild
```

### `spool archive`

Archive completed tasks older than a specified number of days.

```bash
spool archive [OPTIONS]
```

**Options:**
| Flag | Description | Default |
|------|-------------|---------|
| `-d, --days` | Days after completion before archiving | 30 |
| `--dry-run` | Show what would be archived without doing it | - |

**What archiving does:**
1. Finds completed tasks older than the threshold
2. Copies their events to monthly archive files (e.g., `archive/2026-01.jsonl`)
3. Writes `archive` events to the current day's event log
4. Archived tasks remain queryable but are moved to cold storage

**Examples:**

```bash
# Preview tasks that would be archived
spool archive --dry-run

# Archive tasks completed more than 30 days ago
spool archive

# Archive tasks completed more than 7 days ago
spool archive --days 7
```

### Stream Commands

### `spool stream list`

List all streams (workstreams/projects) with task counts.

```bash
spool stream list [OPTIONS]
```

**Options:**
| Flag | Description | Default |
|------|-------------|---------|
| `-f, --format` | Output format: `table`, `json`, or `ids` | `table` |

**Examples:**

```bash
# List all streams
spool stream list

# List streams as JSON
spool stream list --format json

# Get just stream names for scripting
spool stream list --format ids
```

### `spool stream show`

Show all tasks in a specific stream.

```bash
spool stream show <name> [OPTIONS]
```

**Options:**
| Flag | Description | Default |
|------|-------------|---------|
| `-f, --format` | Output format: `table` or `json` | `table` |

**Examples:**

```bash
# Show tasks in the 'api' stream
spool stream show api

# Show as JSON
spool stream show api --format json
```

### `spool stream add`

Add a task to a stream (workstream/project).

```bash
spool stream add <task-id> <stream-name>
```

**Examples:**

```bash
# Add task to 'backend' stream
spool stream add task-abc123 backend

# Add task to 'v2-migration' stream
spool stream add k8b2x-a1c3 v2-migration
```

### `spool stream remove`

Remove a task from its stream.

```bash
spool stream remove <task-id>
```

**Examples:**

```bash
# Remove task from stream
spool stream remove task-abc123
```

### Other Commands

### `spool validate`

Validate event files for correctness and consistency.

```bash
spool validate [OPTIONS]
```

**Options:**
| Flag | Description |
|------|-------------|
| `--strict` | Fail on warnings (not just errors) |

**Checks performed:**
- JSON syntax validity
- Required fields present (`v`, `op`, `id`, `ts`, `by`, `branch`, `d`)
- Schema version compatibility
- Timestamp format (RFC 3339)
- Event ordering (create before other operations)
- Orphaned references (blocked_by, blocks, parent)

**Exit codes:**
- `0` - Validation passed
- `1` - Errors found (or warnings in `--strict` mode)

**Examples:**

```bash
# Basic validation
spool validate

# Strict validation (fails on warnings)
spool validate --strict
```

## Event Schema

Events are stored as newline-delimited JSON (JSONL) with the following structure:

```json
{
  "v": 1,
  "op": "create",
  "id": "k8b2x-a1c3",
  "ts": "2026-01-13T12:00:00Z",
  "by": "@alice",
  "branch": "main",
  "d": { ... }
}
```

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `v` | integer | Schema version (currently `1`) |
| `op` | string | Operation type |
| `id` | string | Task ID |
| `ts` | string | ISO 8601 timestamp |
| `by` | string | Author (e.g., `@username`) |
| `branch` | string | Git branch where event was created |
| `d` | object | Operation-specific data |

### Operations

| Operation | Description | Data fields |
|-----------|-------------|-------------|
| `create` | Create a new task | `title`, `description?`, `priority?`, `assignee?`, `tags?`, `parent?`, `blocks?`, `blocked_by?` |
| `update` | Update task fields | `title?`, `description?`, `priority?`, `tags?` |
| `assign` | Change assignee | `to` (null to unassign) |
| `comment` | Add a comment | `body`, `ref?` |
| `link` | Add relationship | `rel` (blocks/blocked_by/parent), `target` |
| `unlink` | Remove relationship | `rel`, `target` |
| `complete` | Mark task complete | `resolution?` |
| `reopen` | Reopen completed task | - |
| `archive` | Archive completed task | `ref` (archive file reference) |

### Task ID Format

Task IDs use the format `{timestamp}-{random}`:
- `timestamp`: Unix epoch milliseconds, base36 encoded
- `random`: 4 random alphanumeric characters

Example: `k8b2x-a1c3`

## Git Integration

### Best Practices

**1. Commit events atomically with related code changes**

When a task relates to code changes, commit the task events in the same commit:

```bash
git add src/feature.rs .spool/events/
git commit -m "Implement feature X and update task status"
```

**2. Use post-merge hooks to rebuild state**

```bash
#!/bin/sh
# .git/hooks/post-merge
spool rebuild
```

**3. Use post-checkout hooks for branch switching**

```bash
#!/bin/sh
# .git/hooks/post-checkout
spool rebuild
```

**4. Handle merge conflicts in event files**

Event files are append-only, so conflicts are typically resolved by keeping both sets of events:

```bash
# When conflict occurs in .spool/events/2026-01-13.jsonl:
# Keep both the incoming and local changes, then rebuild
git checkout --theirs .spool/events/2026-01-13.jsonl
# Or manually merge by keeping all events from both versions
spool validate
spool rebuild
```

### Branch-Aware Workflows

Since events include branch information, spool supports workflows like:

- **Feature branches**: Create tasks on feature branches, see branch context
- **Code review**: Comments include branch/commit context
- **Merge tracking**: See which branch originally created a task

## Workflows

### Daily Standup

```bash
# See what's assigned to you
spool list --assignee @$(whoami)

# See all open tasks
spool list

# Check task details
spool show <task-id>
```

### Starting Work

```bash
# Find available work
spool list --status open

# (Tasks would be assigned via event creation)
# View task details before starting
spool show <task-id>
```

### Completing Work

```bash
# (Complete event would be created)
# View the completed task
spool show <task-id>
```

### Maintenance

```bash
# Validate event files periodically
spool validate

# Archive old completed tasks monthly
spool archive --days 30

# After pulling changes, rebuild state
spool rebuild
```

### Scripting Integration

```bash
# Get task IDs for scripting
TASKS=$(spool list --format ids)

# Output as JSON for processing
spool list --format json | jq '.[] | select(.priority == "high")'

# Check validation in CI
spool validate --strict || exit 1
```

## Troubleshooting

### "Not in a spool directory"

Run `spool init` in your repository root, or navigate to a directory that contains `.spool/`.

### Stale or missing state

Run `spool rebuild` to regenerate `.index.json` and `.state.json` from events.

### Validation errors

Check the specific error messages from `spool validate`. Common issues:
- Missing required fields in events
- Invalid JSON syntax
- Events for tasks that were never created

### Merge conflicts

For event file conflicts, keep both sets of events (they're append-only), then run `spool rebuild`.
