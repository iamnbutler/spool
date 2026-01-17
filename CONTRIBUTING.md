# Contributing to Spool

Thank you for your interest in contributing to Spool!

## Development Setup

1. **Install Rust** (1.70.0 or later)
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **Clone and build**
   ```bash
   git clone https://github.com/your-username/spool.git
   cd spool
   cargo build
   ```

3. **Run tests**
   ```bash
   cargo test
   ```

## Code Style

- Run `cargo fmt` before committing
- Run `cargo clippy` and fix all warnings
- Add tests for new functionality

## Testing

We aim for high test coverage. When adding features:

1. **Unit tests** - Add tests in the same file or `tests/` directory
2. **Integration tests** - Add CLI tests in `tests/cli_integration_tests.rs`
3. **Run the full suite** before submitting:
   ```bash
   cargo test
   cargo clippy -- -D warnings
   cargo fmt --check
   ```

## Pull Request Process

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/my-feature`)
3. Make your changes with tests
4. Run the test suite
5. Submit a PR with a clear description

## Event Schema

When modifying the event schema (`src/event.rs`):

- Event schema v1 is **frozen** - existing events must remain valid
- New operations can be added but existing ones cannot change
- Add tests for any schema changes

## Questions?

Open an issue for questions or discussions about contributing.
