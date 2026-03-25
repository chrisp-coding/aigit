# aigit – Claude Code Context

## Project Overview
**aigit** is an AI‑native version control system for tracking AI‑generated content (code, text, images) as first‑class version‑controlled artifacts. It stores prompts, model parameters, agent identity, and intent alongside each commit, enabling semantic diffing, LLM‑assisted merging, and multi‑agent collaboration.

**Core value**: Regular Git tracks *what* changed; aigit tracks **why** it changed (the prompt/intent) and **who** changed it (which agent/persona).

## Current Status (Phase 1 Complete – Phase 3 In Progress)
**Implemented** (Phase 0 and Phase 1):
- ✅ Project skeleton (`Cargo.toml`, `src/`, `migrations/`, `tests/`)
- ✅ SQLite schema (see `migrations/`)
- ✅ CLI command structure (`clap`-based)
- ✅ Database layer (`db.rs`) with full CRUD
- ✅ `init` command – creates `.aigit/` directory, config, runs migrations
- ✅ `commit` command – stores prompt, model, output, agent, intent; auto‑detects Git hash; auto‑extracts artifact from `--output` path; resolves parent commits from Git history with fallback to last aigit commit by agent; advances HEAD on agent branches
- ✅ `log` command – lists commits with agent/timestamp filters
- ✅ `show` command – displays full commit details (supports prefix matching)
- ✅ `diff` command – textual diff using `similar` crate; `--semantic` flag prints a warning and falls back to textual diff gracefully (no hard error)
- ✅ `blame` command – integrates with `git.rs` Git blame; maps Git commit hashes to aigit commits; falls back to artifact search when file is not in a Git repo; `--lines` range filter supported
- ✅ `merge` command – textual merge with intent‑annotated conflict markers; `--llm` flag acknowledged, falls back to textual merge; `--output <path>` writes result to a file instead of stdout
- ✅ `agents` subcommand – list/add agents (validates JSON config)
- ✅ `hook` subcommand – install/uninstall/run/list; `--git` flag installs a real `.git/hooks/post-commit` script; `hook run post-commit` does retrospective Git hash linking (covers NULL-hash commits and pre-linked commits with the old parent hash); fallback records Git commit message as aigit commit; `hook list` reports git-installed hooks as `[git] post-commit (aigit-managed)`
- ✅ `conflicts` command – reports files touched by more than one distinct agent; `--window N` (default 10) limits to the N most recent commits per file; shows each conflicting file, the agents that touched it, and their most recent intents
- ✅ `context` subcommand – shows recent aigit commits for a file or repo (Git hash lookup with artifact fallback); `--json` for machine consumption
- ✅ `branch` subcommand – list/create/delete agent‑scoped branches; HEAD advances automatically on each `commit`
- ✅ `status` command – shows Git‑modified files with and without aigit coverage
- ✅ Git integration (`git2`) – fully implemented: `get_current_hash`, `get_repo_root`, `get_parent_hash`, `get_parent_timestamp`, `get_head_commit_message`, `get_commits_for_file`, `get_modified_files`, `get_file_blame`; db also exposes `get_commits_with_git_hash_since` for hook timing fix
- ✅ Unit tests in `db.rs` (13 tests covering CRUD, filtering, hash lookup, agents)
- ✅ Integration tests in `tests/integration.rs` (init, commit, log, show, diff, merge, agents, context, blame)
- ✅ Database optimizations – WAL mode, `synchronous=NORMAL`, `foreign_keys=ON` set on every connection; `commit_artifacts` normalized table with indexed lookups; partial index on `git_hash`; composite index on `(agent_id, timestamp DESC)`; index on `branches(agent_id)`; `Commit` struct derives `Clone`; new `get_commits_by_git_hashes` batch method; new `get_artifact_commit_rows` / `ArtifactAgentRow` for targeted conflict queries; N+1 queries eliminated in `blame`, `context`, and `conflicts`

**Partially implemented / stubbed** (Phase 3–4):
- 🔄 Semantic diffing – `--semantic` flag prints a warning and falls back to textual diff; embeddings table exists but is never populated (full implementation requires Phase 4 embeddings model)
- 🔄 LLM‑assisted merge – `--llm` flag falls back to textual merge; no LLM calls made
- 🔄 Claude Code PostToolUse/PreToolUse hooks – not yet written

**Not yet implemented** (Phase 3–4):
- ❌ Embeddings generation & semantic search
- ❌ LLM‑assisted conflict resolution (`merge --llm`)
- ❌ MCP server (`aigit mcp`)
- ❌ `aigit search` semantic query
- ❌ `aigit export` command

**Environment**:
- **Host**: Linux (WSL2, x86‑64)
- **Rust**: Installed
- **SQLite**: Installed (system default)
- **Workspace**: `/home/chris/projects/aigit`

## Key Architectural Decisions
1. **Data store**: SQLite in `.aigit/db.sqlite`
   - Why not Git‑style content‑addressable store? SQL enables rich queries (filter by agent, intent, similarity) and structured metadata (prompt, model, parameters as JSON).
2. **Local‑first**: No cloud dependency; optional E2E‑encrypted sync later.
3. **Git integration**: Separate database with `git_hash` foreign key (not Git notes) to keep workflows intact.
4. **Embeddings**: Planned use of `all‑MiniLM‑L6‑v2` (80 MB ONNX) for semantic diffing (Phase 4).
5. **Merge assist**: Will use local Ollama (`qwen2.5‑coder:7b`) or configured API (optional).

## File Structure
```
aigit/
├── Cargo.toml              # Dependencies (clap, sqlx, tokio, serde, uuid, similar, …)
├── Cargo.lock
├── CLAUDE.md               # This file
├── CONTEXT.md              # Quick‑start checklist
├── README.md               # Public project description
├── SPEC.md                 # Full specification
├── AGENT_API.md            # How AI agents should interact with aigit
├── TODO.md                 # Phased to-do list
├── FIXES_SUMMARY.md        # Bug fixes log
├── config.example.toml     # Example agent/config
├── Makefile                # Build shortcuts
├── setup.sh                # One‑line setup
├── src/
│   ├── main.rs            # CLI entry point, command routing
│   ├── cli.rs             # Command implementations (init, commit, log, show, diff, blame, merge, agents, hook, context, branch, status, conflicts)
│   ├── db.rs              # Database layer (Database struct, Commit/Agent/Branch models, unit tests)
│   ├── git.rs             # Git integration (get_current_hash, get_repo_root, get_parent_hash, get_parent_timestamp, get_head_commit_message, get_commits_for_file, get_modified_files, get_file_blame)
│   └── lib.rs             # Re-exports cli, db, git modules (used by integration tests)
├── tests/
│   └── integration.rs     # Integration tests for all major commands
├── migrations/
│   ├── 20260318000000_init.sql  # SQLite schema (commits, embeddings, agents, branches)
│   └── 20260318000001_optimizations.sql  # commit_artifacts table, indexes, backfill
└── target/                # Build output
```

## Database Schema
See `migrations/` for full DDL. Core tables:

**commits** – each AI‑generated commit:
- `id` (UUID v7), `agent_id`, `intent`, `prompt`, `model`, `parameters` (JSON)
- `output`, `output_hash` (SHA‑256), `artifacts` (JSON paths — kept for compatibility)
- `timestamp` (Unix ms), `parent_ids` (JSON array), `git_hash` (optional Git link)
- `created_at` (Unix ms, auto)
- Indexes: partial on `git_hash WHERE git_hash IS NOT NULL`; composite on `(agent_id, timestamp DESC)`

**commit_artifacts** – normalized artifact paths for indexed lookups (added in migration `20260318000001`):
- `commit_id` (TEXT REFERENCES commits(id)), `artifact_path` (TEXT)
- Index on `artifact_path`; backfilled from existing JSON `artifacts` column on migration

**embeddings** – vector embeddings of prompt/output (for semantic search, not yet populated)
**agents** – registered agent profiles (agent_id, name, description, config JSON)
**branches** – agent‑specific branches (name + agent_id composite PK, head_commit_id, intent); index on `agent_id`

## How to Build & Run
### Prerequisites
1. Install Rust (via rustup):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source "$HOME/.cargo/env"
   ```
2. Install `sqlx‑cli` (for creating new migrations):
   ```bash
   cargo install sqlx-cli --no-default-features --features sqlite
   ```

### Build & Test
```bash
cd /home/chris/projects/aigit
cargo build           # compile
cargo test            # run unit + integration tests
cargo run -- --help   # see CLI help
```

### Initialize aigit in a project
```bash
# In any Git repository (or empty directory):
cargo run -- init     # creates `.aigit/` and database
```

### Commit AI‑generated content
```bash
# Full manual commit
cargo run -- commit \
  --agent "frontend-specialist" \
  --intent "make button responsive" \
  --prompt "Rewrite this React component for mobile" \
  --model "claude-3.5-sonnet" \
  --output ./src/Button.js

# Prompt from stdin, output from file
echo "Rewrite this function" | cargo run -- commit \
  --agent "claude-code" \
  --intent "refactor" \
  --model "claude-3.5-sonnet" \
  --output ./src/lib.rs

# Prompt from flag, output from stdin (e.g., piped from another tool)
cargo run -- commit \
  --agent "claude-code" \
  --model "claude-3.5-sonnet" \
  --prompt "Rewrite the parser" < /tmp/output.rs
```

### View history & inspect
```bash
cargo run -- log                          # latest 20 commits
cargo run -- log --agent claude-code      # filter by agent
cargo run -- show <commit-id-prefix>      # show full details
cargo run -- diff <commit1> <commit2>     # textual diff
cargo run -- blame src/lib.rs             # map Git blame lines to agents
cargo run -- blame src/lib.rs --lines 10-20  # specific line range
cargo run -- merge <src> <target>                    # textual merge (conflict markers)
cargo run -- merge <src> <target> --output out.rs    # write merge result to file
cargo run -- conflicts                    # files with multi-agent activity (last 10 commits)
cargo run -- conflicts --window 20        # expand window to 20 commits
cargo run -- agents list                  # list registered agents
cargo run -- context src/auth.rs          # aigit history for a file
cargo run -- context --json               # machine-readable recent history
cargo run -- branch list                  # list agent-scoped branches
cargo run -- branch create main --agent claude-code --intent "primary branch"
cargo run -- status                       # modified files with aigit coverage
cargo run -- hook install --git           # install .git/hooks/post-commit
cargo run -- hook list                    # list installed hooks
```

## Development Workflow
1. **Edit schema** → update `migrations/*.sql`, then:
   ```bash
   sqlx migrate add <name>   # create new migration
   sqlx migrate run          # apply
   ```
2. **Add CLI command** → edit `src/cli.rs` (add Args struct, implement function), add to `src/main.rs`.
3. **Database changes** → update `db.rs` with new methods.
4. **Test with actual commits** → use the `commit` command with sample data.

## Next Immediate Tasks (Phase 3 Priority)
1. **Claude Code hooks** – write PostToolUse hook that calls `aigit commit` after file writes; write PreToolUse hook that warns when another agent recently touched the target file.
2. **MCP server** – implement `aigit mcp` subcommand exposing aigit tools over Model Context Protocol.
3. **`merge --llm`** – implement LLM‑assisted merge via Anthropic API or local Ollama.
4. **`aigit resolve`** – per‑file LLM merge invocation.

## Gotchas & Notes
- **SQLx migrations**: `sqlx migrate run` must be run after `init` (already handled in `cli::init`).
- **UUID v7**: Used for time‑ordered commit IDs. Requires `uuid` crate feature `v7`.
- **Output hash**: SHA‑256 of output for deduplication; stored as hex string.
- **JSON fields**: `parameters`, `artifacts`, `parent_ids` stored as JSON text in SQLite.
- **Embeddings**: Table exists but is never populated (Phase 4).
- **Git integration**: `git_hash` column links to Git commits; post‑commit hook retrospectively links aigit commits created since the previous Git commit — both NULL-hash commits and commits pre-linked to the old parent hash (the timing fix for `aigit commit` running before `git commit`).
- **Artifact field**: Populated from the `--output` file path. `insert_commit` writes to both the legacy JSON `artifacts` column and the normalized `commit_artifacts` table. Artifact lookups (`get_latest_commit_for_artifact`, `get_commits_for_artifact`) use `JOIN commit_artifacts` with an index rather than `LIKE %path%`.
- **Parent detection**: Tries Git parent commit hash first; falls back to most recent aigit commit by the same agent.
- **Stdin rules**: If `--prompt` is omitted, prompt is read from stdin and `--output` must be a file. If `--prompt` is provided, output can also come from stdin.
- **SQLite pragmas**: `Database::connect` sets WAL journal mode, `synchronous=NORMAL`, and `foreign_keys=ON` on every connection — do not skip these when writing tests that open the database directly.
- **Batch git-hash lookup**: Use `get_commits_by_git_hashes(&[String])` instead of calling `get_commit_by_git_hash` in a loop; the former uses a single `IN (...)` query.

## Testing with MiroFish Simulation
We plan to reuse the MiroFish container (already running) to simulate multi‑agent collaboration:
1. Create simple Rust project (CLI calculator).
2. Define two agents in MiroFish: "refactor‑agent" (optimizes) and "doc‑agent" (comments).
3. Run simulation where each agent makes commits via aigit.
4. Observe `log`, `diff`, and `merge` behavior.

This will help refine conflict detection and merge‑assist logic.

## Open Questions / Decisions Needed
1. **Embedding model**: `all‑MiniLM‑L6‑v2` (80 MB) vs. something smaller/faster?
2. **Merge‑assist LLM**: Default to local Ollama (`qwen2.5‑coder:7b`) or allow API (OpenAI/Anthropic)?
3. **Git integration depth**: Should aigit commits be stored as Git notes for portability?
4. **Performance**: SQLite with 10k+ commits; embedding generation async.
5. **Artifact extraction**: Richer extraction beyond single `--output` path (Phase 3)?
6. **MCP transport**: stdio vs. HTTP for `aigit mcp`?

## Useful Commands for Development
```bash
# Create a new migration
sqlx migrate add add_some_feature

# Check SQLx queries at compile time
cargo sqlx prepare -- --lib

# Format code
cargo fmt

# Lint
cargo clippy

# Run with logging
RUST_LOG=debug cargo run -- init

# Test a full commit cycle
mkdir test-repo && cd test-repo && git init
cargo run -- init
echo "test output" > test.rs
echo "test prompt" | cargo run -- commit --agent test --model test --output test.rs
cargo run -- log
```

## Contact & Context
- **Author**: Chris Woodcox (cwoodcox‑work on GitHub)
- **Project spec**: `SPEC.md` (detailed), `CONTEXT.md` (checklist), `FIXES_SUMMARY.md` (bug fixes), `TODO.md` (phased task list), `AGENT_API.md` (agent integration guide)
- **Workspace**: `/home/chris/projects/aigit`

---
*Updated: 2026‑03‑24 (Phase 1 complete; Phase 3 in progress — `conflicts` command added, hook timing fixed, merge `--output` added)*
