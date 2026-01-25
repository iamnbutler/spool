# Spool

Git-native, event-sourced task management.

```bash
cargo install spool
```

Tasks are stored as append-only event logs in `.spool/events/`. Every change is tracked, branches work naturally, and merge conflicts resolve automatically.

## Usage

### Initialize

```bash
cd your-project
spool init
```

Creates `.spool/` with `events/` and `archive/` directories. Commit this to git.

### Create tasks

```bash
spool add "Implement authentication"
spool add "Fix login bug" -p p0 -t bug -d "Users getting logged out unexpectedly"
spool add "Backend API" -p p1 -t feature -a @alice --stream <stream-id>
```

Options: `-p` priority (p0-p3), `-t` tag (repeatable), `-d` description, `-a` assignee, `--stream` stream ID.

### List tasks

```bash
spool list                          # Open tasks (default)
spool list -s all                   # All tasks
spool list -s complete              # Completed only
spool list -a @alice                # By assignee
spool list -p p0 -t bug             # By priority and tag
spool list --stream <id>            # By stream
spool list --no-stream              # Tasks without a stream
spool list -f json                  # JSON output
spool list -f ids                   # IDs only (for scripting)
```

### Show task details

```bash
spool show <task-id>
spool show <task-id> --events       # Include event history
```

### Update tasks

```bash
spool update <id> -t "New title"
spool update <id> -d "New description"
spool update <id> -p p1
spool update <id> --stream <stream-id>
```

### Assign tasks

```bash
spool assign <id> @alice            # Assign to user
spool claim <id>                    # Assign to yourself
spool free <id>                     # Unassign
```

### Complete tasks

```bash
spool complete <id>                 # Default resolution: done
spool complete <id> -r wontfix      # Other: duplicate, obsolete
spool reopen <id>                   # Reopen completed task
```

### Streams

Streams group tasks into collections (features, sprints, areas).

```bash
spool stream add "api" -d "Backend API work"
spool stream list
spool stream show <id>
spool stream show --name "api"
spool stream update <id> -n "new-name" -d "New description"
spool stream delete <id>            # Must have no tasks
```

### Maintenance

```bash
spool rebuild                       # Regenerate caches from events
spool archive --days 30             # Archive old completed tasks
spool archive --dry-run             # Preview what would be archived
spool validate                      # Check event file integrity
spool validate --strict             # Fail on warnings too
```

## How it works

### Event sourcing

Instead of mutable records, every change is an immutable event:

```json
{"v":1,"op":"create","id":"k8b2x-a1c3","ts":"2026-01-13T12:00:00Z","by":"@alice","branch":"main","d":{"title":"Fix bug"}}
{"v":1,"op":"assign","id":"k8b2x-a1c3","ts":"2026-01-13T12:01:00Z","by":"@bob","branch":"main","d":{"to":"@bob"}}
{"v":1,"op":"complete","id":"k8b2x-a1c3","ts":"2026-01-13T14:00:00Z","by":"@bob","branch":"main","d":{"resolution":"done"}}
```

Events are stored in daily JSONL files: `.spool/events/2026-01-13.jsonl`

State is materialized by replaying events. Caches (`.index.json`, `.state.json`) are gitignored and rebuilt on demand with `spool rebuild`.

### Directory structure

```
.spool/
├── events/           # Daily event logs (committed)
│   └── 2026-01-13.jsonl
├── archive/          # Monthly archives (committed)
│   └── 2026-01.jsonl
├── .index.json       # Cache (gitignored)
├── .state.json       # Cache (gitignored)
└── .gitignore
```

### Operations

| Operation | Description |
|-----------|-------------|
| `create` | Create task with title, description, priority, assignee, tags |
| `update` | Update task fields |
| `assign` | Change assignee (null to unassign) |
| `complete` | Mark complete with resolution |
| `reopen` | Reopen completed task |
| `comment` | Add comment |
| `link` / `unlink` | Manage relationships (blocks, blocked_by, parent) |
| `set_stream` | Set or remove task's stream |
| `create_stream` | Create stream |
| `update_stream` | Update stream metadata |
| `delete_stream` | Delete stream |
| `archive` | Archive completed task |

### Task IDs

Format: `{timestamp}-{random}` where timestamp is Unix ms in base36 and random is 4 alphanumeric chars.

Example: `k8b2x-a1c3`

## Git integration

### Commit with code

```bash
git add src/feature.rs .spool/events/
git commit -m "Implement feature and update task"
```

### Post-merge hook

```bash
#!/bin/sh
# .git/hooks/post-merge
spool rebuild
```

### Merge conflicts

Event files are append-only. On conflict, keep both sets of events:

```bash
# Keep all events from both versions, then:
spool validate
spool rebuild
```

### CI validation

```bash
spool validate --strict || exit 1
```

## Scripting

```bash
# Get task IDs
TASKS=$(spool list -f ids)

# Process JSON
spool list -f json | jq '.[] | select(.priority == "p0")'

# Batch operations
for id in $(spool list -f ids -t bug); do
  spool update $id -p p1
done
```

## License

MIT

