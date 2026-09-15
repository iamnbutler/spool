# Spool

A shared task board for coding agents. Plan work in streams, claim a ready task, keep notes, and hand it off with enough context for the next agent to continue.

Agents in the same repository's Git worktrees share a live board. Claims are atomic, expire when abandoned, and carry a token that prevents a stale worker from changing reclaimed work. Durable task history travels with Git. [Telephone](https://nate.rip/telephone/) provides agent discovery and messaging.

## Try this branch

Requires Rust 1.88+ and Git 2.31+.

```sh
cargo build --release -p spool-cli
export PATH="$PWD/target/release:$PATH"
spool prime
```

This collaboration workflow is unreleased; build from this checkout. To install this checkout's binary permanently, use `cargo install --path crates/spool-cli --locked`. All participating agents should use the same version.

## The work loop

Initialize once at the repository root and give each agent session a unique identity:

```sh
spool init
export SPOOL_AGENT=api-worker-1
```

Identity precedence is `--agent`, `SPOOL_AGENT`, `TELEPHONE_ADDR`, then the Git user for ordinary human edits. Claiming and renewing work require an explicit session identity. Multiple agents must not share the same session identity.

```sh
spool ready --json
spool next --json
```

`next` selects and claims the highest-priority available task in one transaction. Its response includes the task's description, notes, prerequisites, and `claim.token`. It returns JSON `null` when no work is available. Use `claim <id>` to choose a task explicitly.

Keep the returned token for that task:

```sh
spool show <id> --json
spool renew <id> --token <token>
spool comment <id> 'Parser implemented; checking Unicode boundaries' --ref src/parser.rs
spool complete <id> --token <token> --note 'Implemented and verified with parser tests'
```

The default lease is 15 minutes. Renew before it expires; `--lease-seconds 3600` requests an hour. Durations range from 1 second to 24 hours. A current token and matching identity are required to change a claimed task. Notes can be added by any collaborator.

When pausing, leave a useful resumption point:

```sh
spool release <id> --token <token> --note 'Implementation is on branch parser; Unicode test still fails'
```

`release` clears the assignment and lease. Once a lease expires, any collaborator can release the abandoned task without a token and record a recovery note, even if prerequisites prevent reclaiming it. An expired lease can be reclaimed by another agent, including when the original worker had a reservation. Every fresh claim gets a new token. An old token cannot renew, complete, release, or edit a subsequent claim, even if the agent identity is reused.

## Plan work

Streams group related tasks into a project or epic. Put scope, acceptance criteria, file ownership, and expected output in the description.

```sh
spool stream add parser -d 'Ship the new parser'
spool add 'Define the grammar' --stream parser -p p1 -d 'Document accepted syntax and error cases'
spool add 'Implement parsing' --stream parser -p p1 -t rust
spool block <implementation-id> --by <grammar-id>
```

A task is ready when it is open, has no live claim, all prerequisites are complete, and any pending assignment matches the requesting agent. Missing prerequisites keep it blocked. Cycles and self-dependencies are rejected. Completing a prerequisite makes its dependents eligible; it never completes them automatically. Any completion resolution resolves a prerequisite. Marking a task `done` requires its prerequisites to be complete; cancellation resolutions can close blocked work.

```sh
spool unblock <id> --by <prerequisite>
spool assign <id> api-worker-1        # reserve future work
spool free <id>                      # clear a reservation
spool update <id> -d 'Revised acceptance criteria' --add-tag reviewed
spool update <id> --stream parser
spool update <id> --stream ''         # remove from a stream
spool reopen <id>
spool complete <id> -r wontfix
```

Priorities are `p0` through `p3`, with `p2` as the default. Streams accept IDs or names; new and renamed stream names are trimmed and lowercased. Task commands accept an unambiguous ID prefix. `link` / `unlink` also support `blocks`, `blocked_by`, and `parent` relationships.

## Find context quickly

```sh
spool status --json
spool list --mine --json
spool list --stream parser --limit 20 --json
spool list --status blocked --json
spool list --status in_progress --json
spool list --search Unicode --tag rust --json
spool ready --stream parser --limit 10 --json
spool next --stream parser --tag rust --json
spool show <id> --events --json
spool stream show parser --json
```

Task status remains `open` or `complete`. `work_status` describes the current view: `open`, `blocked`, `in_progress`, or `complete`. `ready` is relative to the requesting agent. `assignee` is a reservation; `claim` identifies the active worker and its lease.

`--json` works on every command. Successful commands write one JSON value to stdout; errors write a JSON object to stderr:

```json
{"error":{"code":"claim_conflict","message":"Task … is claimed by another agent"}}
```

Exit codes are `0` for success (including an empty queue), `1` for task/data/I/O errors, and `2` for claim conflicts, expired/stale claims, a busy board, or argument errors. Use `error.code` to distinguish cases. List responses omit descriptions and notes; `show`, `claim`, and `next` return full task context. Existing `list -f json` and `-f ids` remain available. Reads always replay the current event log, so no rebuild hook is needed after edits or Git operations.

## Pair with Telephone

Use a real per-session address from Telephone as `TELEPHONE_ADDR`, or pass it to Spool with `--agent`. For a polling runtime, Telephone can create the address:

```sh
export TELEPHONE_ADDR="$(telephone register --runtime generic --name parser-worker)"
spool whoami --json
telephone list
```

A handoff saves the recipient, note, and optional reference together, and releases the current claim:

```sh
spool handoff <id> --to <recipient-address> --token <token> \
  --note 'Review branch parser. Focus on error recovery; tests pass.' \
  --ref <pull-request-url> --json
```

The response includes `notification.to` and `notification.message`. Send that payload with Telephone when you want to notify the recipient:

```sh
# handoff.json contains the JSON returned above.
telephone send "$(jq -r '.notification.to' handoff.json)" \
  "$(jq -r '.notification.message' handoff.json)"
```

Spool saves the handoff without invoking Telephone. The recipient reads the task, then claims it using the assigned address as its Spool identity (`--agent` can override a different `SPOOL_AGENT`). If delivery is delayed or uncertain, the assignment and context remain discoverable through `spool list --mine` and `show`. Follow Telephone's delivery report; successful sending is not a read receipt. Spool also works with human assignees and agents that have no Telephone address.

## Git and worktrees

Inside a Git repository, the live board is in `<git-common-dir>/spool`. All linked worktrees use that directory, including worktrees created before initialization. Switching code branches does not roll back the live board. `spool status` shows the exact board and checkout paths.

Each command imports durable events found in its checkout's `.spool`. Export the shared board before committing:

```sh
spool sync
spool validate --strict
git add .spool
git commit -m 'Record task progress'
```

`sync` exports the whole shared board, including work recorded in other worktrees. It writes missing immutable events into this checkout's `.spool/events`; it does not create Git commits or push anything. Existing daily logs and archives stay readable and are never rewritten. Exact duplicate events replay once. New changes use separate content-addressed files so agents can add events on different branches without appending to the same file.

Claims and renewals live under `.local/events` in the live board. They survive process exits but are excluded from Git exports. A clone starts with task history and reservations, then establishes its own local claims. **Independent clones or machines do not share atomic claims.** This is a local collaboration tool, matching Telephone's same-machine scope. Use a local filesystem and one shared board for agents that need exclusive claims.

Outside Git, Spool uses the nearest `.spool` directly, with local lease files gitignored. `spool archive` retains history while hiding old completed tasks; `reopen` makes an archived task visible again. `spool rebuild` remains available to write diagnostic snapshots; those snapshots are never used as authoritative reads.

## Upgrading an existing board

The first run imports existing events and updates the format marker to `0.5.0`. Existing tasks, streams, comments, and completion history remain available. The important workflow changes are:

- Use a unique agent identity and keep claim tokens.
- Run `spool sync` to export shared progress before committing.
- Stop using older Spool binaries on the upgraded board; older writers do not participate in its coordination protocol.

See [the design and acceptance contract](docs/agent-coordination.md) and [the short agent guide](skills/spool.md).

## Optional TUI

`cargo run -p spool-ui` opens the board. It shows active agents and lease expiry and watches shared task and lease events. The `a` action reserves a task for the current user. Active agent claims are protected from TUI edits; use the CLI with the current token for lease operations. Press `?` for shortcuts.

## Development

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
cargo +1.88 check --workspace --locked
```

MIT licensed.
