# Getting Started with Spool

Spool is a git-native task management system. Tasks are stored as events in your repository, giving you full history and branch-aware workflows.

## Installation

```bash
cargo install spool
```

Or build from source:
```bash
git clone https://github.com/your-username/spool.git
cd spool
cargo install --path .
```

## Quick Start

### 1. Initialize Spool

In your project directory:

```bash
spool init
```

This creates `.spool/` with:
- `events/` - Daily event logs (committed to git)
- `archive/` - Monthly rollups of completed tasks
- `.gitignore` - Ignores derived cache files

### 2. Create a Task

```bash
spool add "Implement user authentication" -p p1 -t feature
```

Options:
- `-p, --priority` - Priority level (p0, p1, p2, p3)
- `-t, --tag` - Add tags (can use multiple times)
- `-d, --description` - Task description
- `-a, --assignee` - Assign to someone (@username)

### 3. List Tasks

```bash
# List open tasks (default)
spool list

# List all tasks
spool list --status all

# Filter by assignee
spool list --assignee @alice

# Output as JSON
spool list --format json
```

### 4. View Task Details

```bash
spool show task-abc123

# Include event history
spool show task-abc123 --events
```

### 5. Update Tasks

```bash
# Update title
spool update task-abc123 --title "New title"

# Update priority
spool update task-abc123 --priority p0

# Assign to yourself
spool claim task-abc123

# Assign to someone else
spool assign task-abc123 @bob

# Unassign
spool free task-abc123
```

### 6. Complete Tasks

```bash
# Mark as done
spool complete task-abc123

# With resolution
spool complete task-abc123 --resolution wontfix
```

Resolutions: `done`, `wontfix`, `duplicate`, `obsolete`

### 7. Interactive Shell

For rapid task management:

```bash
spool shell
```

Commands work the same but without the `spool` prefix:
```
> add "Quick task" -p p2
> list
> complete task-xyz
> quit
```

## Git Integration

Spool events are regular files - commit them with your code:

```bash
git add .spool/events/
git commit -m "Add authentication task"
```

Tasks follow branches. When you merge, events merge cleanly (append-only JSONL).

## Next Steps

- See [CLI Guide](CLI_GUIDE.md) for complete command reference
- Run `spool --help` for all options
