# Agent coordination contract

Spool owns durable task context and local work allocation. [Telephone](https://nate.rip/telephone/) owns discovery and message delivery. Their common identifier is a session-specific agent address. Neither tool needs to run the other's process or infer whether a message was read.

## What the investigation found

At `5b7e3a2`, the CLI's claim command was an unconditional assignment to the Git user. The optimistic concurrency module was unused by ordinary writes; its lock was a manually removed file. Readers could use `.state.json` indefinitely after writes. Worktrees discovered their own `.spool` directories, so independent workers could both claim a task. Daily JSONL appends, archive duplication, and replay by file order also made concurrent changes fragile.

The event model already held tasks, streams, comments, and relationships. This revision keeps that durable vocabulary and replaces the write/read boundary and agent workflow around it.

## Coordination boundary

The Git common directory supplies one live board and one kernel-backed advisory lock for the repository. A standalone `.spool` directory supplies the same boundary outside Git. All supported mutations acquire the lock, replay current events, validate their preconditions, and publish a complete event before returning. A competing writer validates against the preceding writer's result. Kernel locks are released when a process exits; a leftover `.lock` file is harmless and must not be removed while clients are running.

`next` performs selection and acquisition in the same transaction. Sorting is by priority, creation time, then ID. There is no reservation interval between reading the queue and taking ownership.

The contract covers cooperating clients on a local filesystem. Independent clones, older binaries, and direct file edits do not participate in the lock. Code-file ownership remains a planning concern; task claims do not lock source files.

## Task and lease state

Durable task status is `open` or `complete`. The computed work view adds `blocked` and `in_progress`. Assignments reserve future work. A live claim records its agent, random token, start/expiry time, branch, and worktree.

Claim and renewal events are local. Handoff, completion, notes, assignment, and other task changes are durable. A handoff records recipient and context together, clears the claim, and leaves work open for the recipient to claim. Releasing to the queue is a handoff with no recipient.

A mutation of claimed work checks the current token, identity, and expiry while holding the lock. A new claim always gets a new token, so a stale invocation cannot act on a renewed task generation. Expired work can be reclaimed, even after a reservation. Renewal keeps the same token and extends expiry; it cannot resurrect an expired claim. An expired claim can be released without a token by a collaborator, with a durable recovery note; this also recovers blocked work that cannot yet be reclaimed. A supplied token is always checked, so stale invocations cannot release a subsequent claim. Collaborators can append notes without holding the claim.

Task dependencies are normalized in both directions during replay, including one-sided legacy events. Missing prerequisites block work. Adding an edge checks reachability under the same write lock to prevent cycles, including simultaneous opposing edges. Completing a prerequisite changes readiness; dependents still require their own claims and completion.

## Durable storage and Git

Events retain the v1 envelope. `claim`, `renew`, and `handoff` are additive operations. New files use a timestamp and SHA-256 fingerprint of the canonical event. Publication writes and syncs a temporary file, then persists it without clobbering an existing event. A modified file with a content-addressed name is rejected. Temporary files are ignored by event readers.

Replay reads the union of durable and local events, sorts deterministically, and removes exact duplicates. New timestamps are advanced past the latest event under the lock, preserving causality even when two writes would otherwise receive the same clock value. Legacy equal-time create events precede mutations; other equal-time ties use the fingerprint. Older daily logs and monthly archives remain readable.

The live board is independent of the checked-out code branch. Commands import the current checkout's durable history. `sync` adds missing durable events to that checkout's `.spool`; it preserves old files and avoids re-exporting events already present in a legacy log. Export contains the full shared board. Claims and renewals are omitted. No command commits, pushes, or merges Git changes.

Git is an interchange and review mechanism, not a distributed claim coordinator. Concurrent offline metadata edits replay in deterministic timestamp/fingerprint order; this is not a distributed transaction. `validate --strict` detects malformed events, missing references, and dependency cycles, including problems introduced by an external merge. Keep all collaborating workers on one local board when exclusive claims matter.

## Interface and recovery

`prime` is the short onboarding contract. Every command supports JSON. Queue/list responses are summaries; `show`, `next`, and `claim` include full task context. Errors have stable `error.code` and readable `error.message`. A conflicted claim never reports success; an idle queue returns `null` with exit code zero.

After expiry or a claim conflict, reread the task and reacquire work. After an I/O failure, inspect task state and owned work before retrying; a failure after publication may have committed the event. Messages are independent: a failed Telephone send does not roll back a handoff, and retrying a handoff is not a delivery retry.

The TUI uses the same guarded writer API, displays live claims, and watches local lease events as well as durable changes. It manages reservations with its assignment actions. Lease renewal and completion of claimed work use the CLI and current token.

## Acceptance coverage

The CLI integration suite starts real competing processes and temporary Git repositories. It checks:

- Exactly one winner when many agents claim a task simultaneously.
- Unique allocations from simultaneous `next` commands, including an empty queue.
- Shared visibility and claims across worktrees created before initialization.
- Durable export/import with local claims excluded from an independent clone.
- Lease expiry, recovery, renewal, and rejection of stale tokens after reassignment or reuse of an agent identity.
- Ownership enforcement through both CLI mutations and the writer API used by the TUI.
- Dependency cycles, simultaneous opposing edges, missing prerequisites, and readiness after completion.
- Durable handoff context and a composable Telephone notification payload.
- Complete concurrent long notes, stale-cache immunity, modified-event rejection, and process-crash lock recovery.

No live Telephone peer is needed for these tests: Spool's contract ends at the recorded handoff and returned notification payload.
