# Spool Specification

Spool is a version-controlled, append-only task management system designed for high-volume, parallel-branch workflows with seamless conflict resolution.

## Goals

1. **Git-native**: All task data lives in the repository, checked into version control
2. **Conflict-resistant**: Parallel branches can modify tasks with minimal/mechanical merge conflicts
3. **Agent-scale**: Support 50-200+ tasks/day from automated agents
4. **Auditable**: Full history of who changed what, when, and from which branch
5. **Queryable**: Efficiently list, filter, and inspect tasks without full replay

## File Structure

```
.spool/
├── events/
│   ├── 2025-07-14.jsonl    # Daily event log (append-only)
│   ├── 2025-07-15.jsonl
│   └── 2025-07-16.jsonl
├── archive/
│   ├── 2025-05.jsonl       # Monthly rollup (immutable)
│   └── 2025-06.jsonl
├── .gitignore              # Ignores derived files below
├── .index.json             # Derived: task_id → status, date range
└── .state.json             # Derived: current materialized state
```

### `.gitignore`

Place this file at `.spool/.gitignore`:

```gitignore
# Derived files - rebuilt from events on checkout/merge
# These are caches for fast queries, not source of truth

# Task index: maps task_id → status, date range, file locations
.index.json

# Materialized state: current snapshot of all tasks
.state.json

# Any temporary files from tooling
*.tmp
*.bak
```

## Event Schema

Each line in a `.jsonl` file is a single event:

```json
{
  "v": 1,
  "op": "create",
  "id": "x7k2m9",
  "ts": "2025-07-14T14:30:52.123Z",
  "by": "@nate",
  "branch": "feat/auth",
  "d": {
    "title": "Implement OAuth flow",
    "priority": "high",
    "tags": ["auth", "security"]
  }
}
```

### Fields

| Field    | Type    | Required | Description                                          |
| -------- | ------- | -------- | ---------------------------------------------------- |
| `v`      | integer | yes      | Schema version (currently `1`)                       |
| `op`     | string  | yes      | Operation type (see below)                           |
| `id`     | string  | yes      | Task identifier (see ID Generation)                  |
| `ts`     | string  | yes      | ISO 8601 timestamp with milliseconds                 |
| `by`     | string  | yes      | Author identifier (e.g., `@username`, `@agent-name`) |
| `branch` | string  | yes      | Git branch where event originated                    |
| `d`      | object  | yes      | Operation-specific data payload                      |

### Operations

#### `create`

Creates a new task.

```json
{
  "op": "create",
  "d": {
    "title": "string (required)",
    "description": "string (optional)",
    "priority": "low | medium | high | critical (optional)",
    "tags": ["array", "of", "strings"],
    "assignee": "@username (optional)",
    "parent": "task_id (optional, for subtasks)",
    "blocks": ["task_id", "..."],
    "blocked_by": ["task_id", "..."]
  }
}
```

#### `update`

Updates task fields. Only include changed fields.

```json
{
  "op": "update",
  "d": {
    "title": "New title",
    "priority": "critical"
  }
}
```

#### `assign`

Changes task assignment.

```json
{
  "op": "assign",
  "d": {
    "to": "@username | null"
  }
}
```

#### `comment`

Adds a comment to a task.

```json
{
  "op": "comment",
  "d": {
    "body": "Comment text, can be multiline",
    "ref": "optional reference (commit SHA, URL, etc.)"
  }
}
```

#### `link`

Creates a relationship between tasks.

```json
{
  "op": "link",
  "d": {
    "rel": "blocks | blocked_by | related | parent | child",
    "target": "other_task_id"
  }
}
```

#### `unlink`

Removes a relationship between tasks.

```json
{
  "op": "unlink",
  "d": {
    "rel": "blocks | blocked_by | related | parent | child",
    "target": "other_task_id"
  }
}
```

#### `complete`

Marks a task as completed.

```json
{
  "op": "complete",
  "d": {
    "resolution": "done | wontfix | duplicate | obsolete (optional)",
    "note": "optional completion note"
  }
}
```

#### `reopen`

Reopens a completed task.

```json
{
  "op": "reopen",
  "d": {
    "reason": "optional reason for reopening"
  }
}
```

#### `archive`

Marks a task as archived (used during archival process).

```json
{
  "op": "archive",
  "d": {
    "ref": "2025-06"
  }
}
```

## ID Generation

Task IDs must be:

- Unique across all branches
- Collision-resistant for parallel creation
- Somewhat human-readable
- Sortable by creation time

### Format

```
{timestamp}-{random}
```

- `timestamp`: Unix epoch milliseconds, base36 encoded
- `random`: 4 random alphanumeric characters

### Example

```
lz5k2m-x7k2
```

This gives us:

- ~2.8 trillion possible IDs per millisecond
- Chronological sorting
- Short enough to type/reference

### Generation (pseudocode)

```
function generateId():
  timestamp = base36(Date.now())
  random = randomAlphanumeric(4)
  return `${timestamp}-${random}`
```

## State Materialization

Current state is derived by replaying events. The materialized state is cached in `.state.json` (gitignored).

### Task State Object

```json
{
  "id": "lz5k2m-x7k2",
  "title": "Implement OAuth flow",
  "description": "...",
  "status": "open | complete",
  "priority": "high",
  "tags": ["auth", "security"],
  "assignee": "@nate",
  "created": "2025-07-14T14:30:52.123Z",
  "created_by": "@luke",
  "created_branch": "feat/auth",
  "updated": "2025-07-15T09:00:00.000Z",
  "completed": null,
  "resolution": null,
  "parent": null,
  "blocks": [],
  "blocked_by": [],
  "comments": [
    {
      "ts": "2025-07-14T15:00:00.000Z",
      "by": "@terkel",
      "body": "Should we use PKCE?"
    }
  ]
}
```

### Replay Algorithm

```
function materialize(eventFiles):
  tasks = {}

  for file in sorted(eventFiles):
    for line in file:
      event = JSON.parse(line)

      switch event.op:
        case "create":
          tasks[event.id] = {
            id: event.id,
            status: "open",
            created: event.ts,
            created_by: event.by,
            created_branch: event.branch,
            updated: event.ts,
            comments: [],
            blocks: [],
            blocked_by: [],
            ...event.d
          }

        case "update":
          tasks[event.id] = {
            ...tasks[event.id],
            ...event.d,
            updated: event.ts
          }

        case "assign":
          tasks[event.id].assignee = event.d.to
          tasks[event.id].updated = event.ts

        case "comment":
          tasks[event.id].comments.push({
            ts: event.ts,
            by: event.by,
            body: event.d.body,
            ref: event.d.ref
          })
          tasks[event.id].updated = event.ts

        case "complete":
          tasks[event.id].status = "complete"
          tasks[event.id].completed = event.ts
          tasks[event.id].resolution = event.d.resolution || "done"
          tasks[event.id].updated = event.ts

        case "reopen":
          tasks[event.id].status = "open"
          tasks[event.id].completed = null
          tasks[event.id].resolution = null
          tasks[event.id].updated = event.ts

        case "link":
          // Add to appropriate array

        case "unlink":
          // Remove from appropriate array

        case "archive":
          // Mark as archived, note reference file

  return tasks
```

### Conflict Resolution (Same-Task Updates)

When multiple branches update the same task, we use **field-level last-write-wins**:

1. Events are ordered by `ts` (timestamp)
2. For each field, the latest event's value wins
3. Arrays (tags, blocks, etc.) use set semantics—add/remove operations, not replacement

This means:

- Branch A sets `priority: "high"` at T1
- Branch B sets `assignee: "@luke"` at T2
- Result: `priority: "high"` AND `assignee: "@luke"` (no conflict)

True conflicts (same field, same task, different values) resolve to latest timestamp.

## Index File

The `.index.json` file is a derived cache for fast queries:

```json
{
  "tasks": {
    "lz5k2m-x7k2": {
      "status": "open",
      "created": "2025-07-14",
      "updated": "2025-07-15",
      "files": ["2025-07-14.jsonl", "2025-07-15.jsonl"]
    },
    "lz5k3n-a3b4": {
      "status": "complete",
      "created": "2025-07-14",
      "updated": "2025-07-16",
      "completed": "2025-07-16",
      "files": ["2025-07-14.jsonl", "2025-07-16.jsonl"],
      "archived": "2025-07"
    }
  },
  "rebuilt": "2025-07-16T10:00:00.000Z"
}
```

This enables:

- O(1) lookup of task status
- Quick filtering (open vs. complete)
- Knowing which event files to scan for a task's history

## Archival

### Trigger

Archival runs when:

- Manually invoked (`spool archive`)
- Optionally via git hook (post-merge, scheduled)

### Process

1. Identify completed tasks older than N days (default: 30)
2. For each task to archive:
   a. Collect all events from daily files
   b. Append to `archive/{YYYY-MM}.jsonl` (month of completion)
   c. Append `archive` event to current day's event file
3. Optionally compress old archive files (`.jsonl.gz`)

### Archive File Format

Same JSONL format as daily files. Events are grouped by task but otherwise unchanged.

### Querying Archived Tasks

1. Check `.index.json` for `archived` field
2. If present, read from `archive/{ref}.jsonl` instead of daily files
3. Decompress if necessary

## CLI Interface

```
spool add <title> [options]
  --priority, -p    low|medium|high|critical
  --assign, -a      @username
  --tag, -t         tag (repeatable)
  --parent          parent_task_id
  --description, -d description text

spool list [filter]
  --status, -s      open|complete|all (default: open)
  --assignee, -a    @username
  --tag, -t         tag
  --priority, -p    priority level
  --format, -f      table|json|ids

spool show <id>
  --events          Show raw event history

spool update <id> [options]
  --title           New title
  --priority, -p    New priority
  --tag, -t         Add tag
  --untag           Remove tag
  --description, -d New description

spool assign <id> <@username|->
  # Use `-` to unassign

spool comment <id> <body>
  --ref, -r         Reference (commit, URL, etc.)

spool complete <id>
  --resolution, -r  done|wontfix|duplicate|obsolete
  --note, -n        Completion note

spool reopen <id>
  --reason, -r      Reason for reopening

spool link <id> <rel> <target_id>
  # rel: blocks|blocked_by|related|parent|child

spool unlink <id> <rel> <target_id>

spool archive
  --days, -d        Days after completion to archive (default: 30)
  --dry-run         Show what would be archived

spool rebuild
  # Force rebuild of .index.json and .state.json

spool init
  # Initialize .spool/ directory structure
```

## Git Integration

### Hooks (Optional)

**post-checkout / post-merge:**

```bash
#!/bin/sh
spool rebuild
```

This ensures derived files are fresh after branch switches or merges.

### CI Validation

Optionally validate in CI:

- All JSONL files are valid JSON per line
- All events have required fields
- No orphaned task references
- Events only append (diff should only add lines)

```bash
spool validate
  --strict          Fail on warnings
```

## Merge Workflow

### Typical Merge (No Conflicts)

Branch A adds events to `2025-07-14.jsonl`:

```
{"op":"create","id":"aaa",...}
```

Branch B adds events to `2025-07-14.jsonl`:

```
{"op":"create","id":"bbb",...}
```

Git merge: both lines kept, order determined by git. This is fine—event order within a file doesn't affect correctness (timestamps are authoritative).

### Conflict Merge

If git does report a conflict (rare, usually from editing same line):

```
<<<<<<< HEAD
{"op":"update","id":"xyz","d":{"priority":"high"},...}
=======
{"op":"update","id":"xyz","d":{"priority":"low"},...}
>>>>>>> branch-b
```

Resolution: Keep both lines. The replay algorithm uses timestamps to determine final state.

### Post-Merge

After any merge:

```bash
spool rebuild
```

This regenerates derived files from the merged event history.

## Performance Considerations

### At Scale (200 tasks/day)

- Daily file size: ~1-2MB
- Monthly archive: ~30-60MB
- Yearly total: ~400-700MB

### Replay Performance

- Full replay of 1 year: ~10-30 seconds (acceptable for rebuild)
- Incremental replay: track last-processed timestamp, only replay newer
- Indexed queries (open tasks): <100ms

### Optimizations

1. **Lazy loading**: Only parse files when needed
2. **Streaming**: Don't load entire file into memory
3. **Parallel parsing**: Process multiple day files concurrently
4. **Index-first**: Use `.index.json` for filtering before touching events

## Future Considerations

### Not in V1

- Real-time sync (WebSocket push of new events)
- Attachments (files referenced by tasks)
- Rich text in descriptions/comments
- Task templates
- Recurring tasks
- Time tracking
- Custom fields

### Migration Path

The `v` field in events allows schema evolution:

- New fields can be added (old readers ignore them)
- Field semantics can change with new version
- Migration tool can rewrite v1 → v2 events if needed

## Example Session

```bash
# Initialize
$ spool init
Created .spool/

# Add a task
$ spool add "Implement OAuth flow" -p high -t auth -t security -a @nate
Created lz5k2m-x7k2

# List open tasks
$ spool list
ID            PRIORITY  ASSIGNEE  TITLE
lz5k2m-x7k2   high      @nate     Implement OAuth flow

# Add a comment
$ spool comment lz5k2m-x7k2 "Should we use PKCE?"
Added comment to lz5k2m-x7k2

# Complete
$ spool complete lz5k2m-x7k2 -n "Merged in PR #42"
Completed lz5k2m-x7k2

# Archive old tasks
$ spool archive --days 30
Archived 15 tasks to archive/2025-06.jsonl
```
