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

## Branching Strategy

This project uses trunk-based development with short-lived feature branches.

- `main` — always buildable, always passes `cargo test`
- `feat/<name>` — one feature or TODO item per branch
- `fix/<name>` — bug fixes
- `chore/<name>` — dependency updates, CI changes, tooling

**Workflow:**

```bash
# Start from latest main
git fetch origin && git checkout -b feat/mcp-server origin/main

# Keep your branch current (rebase, not merge)
git fetch origin && git rebase origin/main

# Before opening a PR, squash fixup commits
git rebase -i origin/main
```

PRs must pass `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test` (enforced by CI).

## Submitting Changes

- Keep PRs focused — one concern per PR
- All tests must pass (`cargo test`)
- Run `cargo clippy -- -D warnings` and address all warnings before submitting
- Run `cargo fmt` to format code
- Write a clear PR description explaining the *why*, not just the *what*

### Commit message style

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>: short imperative summary (50 chars max after prefix)

Optional longer explanation of why this change is needed.
Reference relevant issues or context.
```

**Types:**
- `feat:` — new feature (bumps minor version)
- `fix:` — bug fix (bumps patch version)
- `chore:` — dependency updates, CI changes, tooling
- `docs:` — documentation only
- `refactor:` — code change with no behavior change
- `test:` — adding or fixing tests

**Breaking changes:** append `!` after the type (`feat!:`) or add `BREAKING CHANGE:` in the footer.

**Examples:**
```
feat: add merge --llm flag for LLM-assisted conflict resolution

fix: open_repo discovers from base path instead of hardcoded "."

Fixes test isolation issue where git2 would walk up into the project
repo when tests ran inside it.

chore: add GitHub Actions CI workflow
```

## Reporting Issues

Open an issue on GitHub with:
- What you ran (exact command)
- What you expected
- What actually happened (include any error output)
- Your OS and Rust version (`rustc --version`)
