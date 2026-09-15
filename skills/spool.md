# Spool: agent task management

Use streams for projects/epics and tasks for concrete pieces of work. Build this checkout's CLI with `cargo build -p spool-cli`; use `./target/debug/spool` while developing Spool. Run `spool prime` for the current command contract.

Give each agent session a unique `SPOOL_AGENT`, pass `--agent`, or reuse its actual `TELEPHONE_ADDR`. Git user names are not unique agent identities. Preserve the identity through the task's lifetime.

1. Read `spool status --json` and `spool list --mine --json` for existing context.
2. Use `spool next --json` to select and claim ready work atomically. It returns `null` if there is none. Use `claim <id>` for a specific task.
3. Keep the returned `claim.token` with that task. Read the full description and notes before editing code.
4. Renew with `spool renew <id> --token <token>` before the 15-minute lease expires. `--lease-seconds` changes the duration.
5. Record decisions, blockers, and evidence with `spool comment <id> '…' --ref <file-or-url>`.
6. Validate the work, then `spool complete <id> --token <token> --note 'Result and validation'`. Release unfinished work with a resumption note, or hand it to another agent.
7. Run `spool sync` before committing `.spool` history. Follow this repository's branch and PR policy.

`assign` reserves future work. `claim` establishes the active worker's lease. A current token and matching identity are required to mutate a claimed task; collaborators can always add notes. After expiry, claim again and use the new token, or release the abandoned task with a recovery note before changing its plan. On a conflict, read the current task and choose other work. Do not free or overwrite another agent's live claim. After an ambiguous I/O failure, inspect the task and `list --mine` before retrying a mutation.

Create a stream with `spool stream add <name> -d 'Objective'`. Add tasks with a clear scope, acceptance criteria, file ownership, and output path. Use `spool block <task> --by <prerequisite>` for dependencies. Missing/open prerequisites block claims; cycles are rejected. Separate tasks can touch the same files, so agree on file ownership or use isolated worktrees.

Keep context bounded: use `--stream`, `--tag`, `--search`, and `--limit`; list JSON contains summaries, while `show` includes full notes and `--events` includes history.

For Telephone handoffs:

```sh
spool handoff <id> --to <actual-recipient-address> --token <token> \
  --note 'Scope, progress, branch/PR, validation, and next step' --json
```

This saves a durable assignment and note and releases the claim. If messaging is authorized, send the returned `notification.message` to `notification.to` with Telephone. Spool does not send it automatically. The receiving agent must read and claim the task before working. A message or assignment is not proof that work has started.

All Git worktrees share the repository's live board. `sync` exports durable history into the current checkout, including other worktrees' events. Claims stay local and are excluded from Git. Independent clones do not coordinate claims; agents working together must use the same local board and compatible Spool versions.
