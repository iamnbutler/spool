# Spool

Git-native, event-sourced task management.

Tasks are stored as append-only event logs in your repository. Every change is tracked, branches work naturally, and conflicts resolve automatically.

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

```bash
# Initialize in your project
spool init

# Create tasks
spool add "Implement authentication" -p p1 -t feature
spool add "Fix login bug" -p p0 -t bug

# List and manage
spool list
spool show task-abc123
spool assign task-abc123 @alice
spool complete task-abc123

# Interactive mode
spool shell
```

## Commands

| Command | Description |
|---------|-------------|
| `init` | Initialize `.spool/` directory |
| `add` | Create a new task |
| `list` | List tasks with filters |
| `show` | Show task details |
| `update` | Update task fields |
| `assign` | Assign task to user |
| `claim` | Assign task to yourself |
| `free` | Unassign task |
| `complete` | Mark task complete |
| `reopen` | Reopen completed task |
| `shell` | Interactive mode |
| `rebuild` | Regenerate caches |
| `archive` | Archive old tasks |
| `validate` | Check event files |

## How It Works

Spool stores tasks as events in `.spool/events/`:

```
.spool/
├── events/
│   ├── 2024-01-15.jsonl
│   └── 2024-01-16.jsonl
├── archive/
└── .gitignore
```

Events are append-only JSONL. State is materialized by replaying events. Caches (`.index.json`, `.state.json`) are gitignored and rebuilt on demand.

## Documentation

- [Getting Started](docs/getting-started.md)
- [CLI Guide](docs/CLI_GUIDE.md)
- [Contributing](CONTRIBUTING.md)

## License

MIT
