# Contributing to aigit

Thanks for your interest in contributing. This document covers how to set up a development environment, run tests, add migrations, and submit changes.

## Development Setup

### Prerequisites

- Rust (stable, via [rustup](https://rustup.rs))
- SQLite (system package)
- `sqlx-cli` for managing migrations:

```bash
cargo install sqlx-cli --no-default-features --features sqlite
```

### Build

```bash
git clone https://github.com/cwoodcox/aigit
cd aigit
cargo build
```

### Run the CLI locally

```bash
cargo run -- --help
cargo run -- init
cargo run -- log
```

## Running Tests

```bash
cargo test
```

The test suite has two layers:

- **Unit tests** (`src/db.rs`) — test the database layer in isolation using temporary SQLite files
- **Integration tests** (`tests/integration.rs`) — test CLI functions end-to-end using `tempdir`

All tests use real SQLite databases (no mocks). Each test gets its own `tempdir` so they run in parallel safely.

To run only one layer:

```bash
cargo test --lib                  # unit tests only
cargo test --test integration     # integration tests only
```

## Database Migrations

Schema lives in `migrations/*.sql`. To add a migration:

```bash
sqlx migrate add <description>    # creates a new timestamped .sql file
# edit the new file
sqlx migrate run                  # apply (or let `aigit init` do it at runtime)
```

Migrations are embedded in the binary via `sqlx::migrate!()` — no separate migration step is needed at runtime.

**Rules:**
- Migrations must be forward-only and additive where possible
- If you drop or rename a column, add a new migration — never edit an existing one
- Test that `aigit init` works cleanly on a fresh directory after your migration

## Code Structure

```
src/
├── main.rs     — CLI entry point and command routing
├── cli.rs      — Command implementations; all functions accept base: &Path
├── db.rs       — Database layer (SQLite via sqlx)
├── git.rs      — Git integration (git2 crate)
└── lib.rs      — Re-exports for integration tests
migrations/     — SQL schema files
tests/
└── integration.rs — End-to-end CLI tests
```

### Key conventions

- All CLI functions accept `base: &Path` (the project root, not hardcoded `.`)
- Database connections are created per-command — no global connection pool
- All DB queries use runtime `sqlx::query()` (not compile-time `query!()`) to avoid needing a live DB at compile time
- ID display is truncated to 12 characters (UUID v7 — 8 chars causes collisions under rapid insertion)

## Adding a New Command

1. Add `Args` or `Commands` struct to `cli.rs`
2. Add the variant to `Commands` enum in `main.rs`
3. Add the match arm in `main()` passing `base`
4. Implement the function in `cli.rs` with signature `pub async fn my_cmd(args: MyArgs, base: &std::path::Path) -> Result<()>`
5. Add an integration test in `tests/integration.rs`

## Submitting Changes

- Keep PRs focused — one concern per PR
- All tests must pass (`cargo test`)
- Run `cargo clippy` and address warnings before submitting
- Run `cargo fmt` to format code
- Write a clear PR description explaining the *why*, not just the *what*

### Commit message style

```
Short imperative summary (50 chars max)

Optional longer explanation of why this change is needed.
Reference relevant issues or context.
```

## Reporting Issues

Open an issue on GitHub with:
- What you ran (exact command)
- What you expected
- What actually happened (include any error output)
- Your OS and Rust version (`rustc --version`)
