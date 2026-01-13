# Fabric CLI User Guide

Fabric is a git-native task management system that stores all task data as append-only event logs in your repository. Tasks are tracked through events, enabling full history, branch-aware workflows, and seamless git integration.

## Table of Contents

- [Quick Start](#quick-start)
- [Core Concepts](#core-concepts)
- [Commands](#commands)
- [Event Schema](#event-schema)
- [Git Integration](#git-integration)
- [Workflows](#workflows)

## Quick Start

```bash
# Initialize fabric in your repository
fabric init

# List open tasks
fabric list

# Show task details
fabric show <task-id>

# Validate event files
fabric validate

# Rebuild index and state from events
fabric rebuild

# Archive old completed tasks
fabric archive --days 30
```

## Core Concepts

### Event Sourcing

Fabric uses event sourcing as its core architecture. Instead of storing mutable task records, every change is recorded as an immutable event. This provides:

- **Full history**: See how any task evolved over time
- **Branch-aware**: Events include branch information for merge conflict resolution
- **Audit trail**: Know who changed what and when
- **Reproducibility**: State can be rebuilt from events at any point

### Directory Structure

After running `fabric init`, your repository will contain:

```
.fabric/
├── events/           # Daily event logs (YYYY-MM-DD.jsonl)
├── archive/          # Monthly rollups of completed tasks
├── .index.json       # Task index cache (gitignored)
├── .state.json       # Materialized state cache (gitignored)
└── .gitignore        # Ignores derived files
```

**Committed files:**
- `events/*.jsonl` - Source of truth for all task data
- `archive/*.jsonl` - Archived task events

**Derived files (gitignored):**
- `.index.json` - Maps task IDs to metadata for fast lookups
- `.state.json` - Current snapshot of all tasks

### Task Lifecycle

Tasks progress through these states:

```
Create → Open → Complete → Archive
              ↓         ↑
              → Reopen →
```

## Commands

### `fabric init`

Initialize the `.fabric/` directory structure in the current repository.

```bash
fabric init
```

Creates:
- `.fabric/events/` - Directory for daily event logs
- `.fabric/archive/` - Directory for monthly archives
- `.fabric/.gitignore` - Ignores derived cache files

**Note:** Run this once at the root of your repository.

### `fabric list`

List tasks with optional filtering.

```bash
fabric list [OPTIONS]
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
fabric list

# List completed tasks
fabric list --status complete

# List tasks assigned to @alice
fabric list --assignee @alice

# List high-priority tasks as JSON
fabric list --priority high --format json

# Get just task IDs for scripting
fabric list --format ids
```

### `fabric show`

Display detailed information about a specific task.

```bash
fabric show <task-id> [OPTIONS]
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
fabric show k8b2x-a1c3

# Show task with full event history
fabric show k8b2x-a1c3 --events
```

### `fabric rebuild`

Rebuild the `.index.json` and `.state.json` cache files from event logs.

```bash
fabric rebuild
```

**When to use:**
- After cloning a repository with fabric
- After pulling changes that include new events
- If cache files become corrupted
- After resolving merge conflicts in event files

**Git hook integration:**
```bash
# .git/hooks/post-merge
#!/bin/sh
fabric rebuild
```

### `fabric archive`

Archive completed tasks older than a specified number of days.

```bash
fabric archive [OPTIONS]
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
fabric archive --dry-run

# Archive tasks completed more than 30 days ago
fabric archive

# Archive tasks completed more than 7 days ago
fabric archive --days 7
```

### `fabric validate`

Validate event files for correctness and consistency.

```bash
fabric validate [OPTIONS]
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
fabric validate

# Strict validation (fails on warnings)
fabric validate --strict
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
git add src/feature.rs .fabric/events/
git commit -m "Implement feature X and update task status"
```

**2. Use post-merge hooks to rebuild state**

```bash
#!/bin/sh
# .git/hooks/post-merge
fabric rebuild
```

**3. Use post-checkout hooks for branch switching**

```bash
#!/bin/sh
# .git/hooks/post-checkout
fabric rebuild
```

**4. Handle merge conflicts in event files**

Event files are append-only, so conflicts are typically resolved by keeping both sets of events:

```bash
# When conflict occurs in .fabric/events/2026-01-13.jsonl:
# Keep both the incoming and local changes, then rebuild
git checkout --theirs .fabric/events/2026-01-13.jsonl
# Or manually merge by keeping all events from both versions
fabric validate
fabric rebuild
```

### Branch-Aware Workflows

Since events include branch information, fabric supports workflows like:

- **Feature branches**: Create tasks on feature branches, see branch context
- **Code review**: Comments include branch/commit context
- **Merge tracking**: See which branch originally created a task

## Workflows

### Daily Standup

```bash
# See what's assigned to you
fabric list --assignee @$(whoami)

# See all open tasks
fabric list

# Check task details
fabric show <task-id>
```

### Starting Work

```bash
# Find available work
fabric list --status open

# (Tasks would be assigned via event creation)
# View task details before starting
fabric show <task-id>
```

### Completing Work

```bash
# (Complete event would be created)
# View the completed task
fabric show <task-id>
```

### Maintenance

```bash
# Validate event files periodically
fabric validate

# Archive old completed tasks monthly
fabric archive --days 30

# After pulling changes, rebuild state
fabric rebuild
```

### Scripting Integration

```bash
# Get task IDs for scripting
TASKS=$(fabric list --format ids)

# Output as JSON for processing
fabric list --format json | jq '.[] | select(.priority == "high")'

# Check validation in CI
fabric validate --strict || exit 1
```

## Troubleshooting

### "Not in a fabric directory"

Run `fabric init` in your repository root, or navigate to a directory that contains `.fabric/`.

### Stale or missing state

Run `fabric rebuild` to regenerate `.index.json` and `.state.json` from events.

### Validation errors

Check the specific error messages from `fabric validate`. Common issues:
- Missing required fields in events
- Invalid JSON syntax
- Events for tasks that were never created

### Merge conflicts

For event file conflicts, keep both sets of events (they're append-only), then run `fabric rebuild`.
