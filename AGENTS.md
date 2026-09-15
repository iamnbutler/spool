- use `spool` to track tasks
  - spool `streams` for epics/projects
  - spool tasks for individual tasks

## Workflow

- All changes must be made in individual PRs - on PR per logical change

## Developing Spool

- Build the checkout CLI with `cargo build -p spool-cli`; use `./target/debug/spool` so the installed release does not operate on a newer board.
- Read `./target/debug/spool prime` and [the agent guide](skills/spool.md).
- Use a unique session identity and keep the token returned by `claim` or `next`.
- Run `./target/debug/spool sync` before committing `.spool` history.
- Check `cargo test --workspace --locked`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo fmt --check`.
