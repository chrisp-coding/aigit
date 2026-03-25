# aigit – Context (Phase 1 Complete)

## Project Overview
AI‑native version control for multi‑agent collaboration. Local‑first, Git‑integrated, tracks prompts, models, agents, intent.

## Key Decisions
- **Data store:** SQLite in `.aigit/` directory.
- **Embeddings:** `all‑MiniLM‑L6‑v2` (optional, Phase 4).
- **Merge assist:** Local Ollama (qwen2.5‑coder:7b) or API (Phase 3).
- **Git integration:** Separate DB with `git_hash` linking; post‑commit hook does retrospective linking.
- **Primary use‑case:** Multi‑agent collaboration (specialized AI agents working together).

## Phase 0 Goals (Complete)
✅ Basic tracking: `init`, `commit`, `log`, `show`
✅ SQLite schema created and migrations ready
✅ Additional commands: `diff`, `blame`, `merge`, `agents`, `hook`
✅ Git integration (`git2`) enabled, basic functions in `git.rs`

## Phase 1 Goals (Complete)
✅ Git integration wired: `get_current_hash`, `get_parent_hash`, `get_parent_timestamp`, `get_commits_for_file`, `get_modified_files`, `get_file_blame`
✅ `blame` maps Git blame lines to aigit commits via `get_commit_by_git_hash`
✅ `context` command: aigit history for a file or repo; `--json` output for agents
✅ `hook install --git` installs real `.git/hooks/post-commit`; `hook run post-commit` does retrospective Git hash linking
✅ `status` command: modified files with/without aigit coverage
✅ `branch` subcommand: list/create/delete agent‑scoped branches
✅ Auto‑detect Git hash and parent commit in `commit`
✅ Auto‑extract artifact from `--output` path
✅ Unit tests in `db.rs`; integration tests in `tests/integration.rs`

## Phase 3 Goals (In Progress)
✅ `aigit conflicts` – shows files where >1 agent has recent commits (`--window N`, default 10)
✅ `merge --output <path>` – write merge result to file instead of stdout
✅ Hook timing fix – post‑commit hook now correctly re‑links commits pre‑stamped with the old parent hash
✅ `diff --semantic` falls back gracefully with a warning instead of erroring
✅ `hook list` reports git‑installed hooks as `[git] post-commit (aigit-managed)`

1. Claude Code PostToolUse hook – auto‑calls `aigit commit` after file writes.
2. Claude Code PreToolUse hook – warns when another agent recently touched the file.
3. `aigit mcp` – MCP server exposing aigit tools (log, show, diff, blame, merge).
4. `merge --llm` – LLM‑assisted merge via Anthropic API or Ollama.

## Files to Review
- `SPEC.md` – full specification
- `CLAUDE.md` – up‑to‑date developer context
- `FIXES_SUMMARY.md` – bug fixes and improvements
- `AGENT_API.md` – how AI agents should interact with aigit
- `TODO.md` – phased task list

## Test Agents (from MiroFish simulation)
- Dr. Aris Thorne (AI Ethics)
- Professor Elena Voss (Design)
- High‑School Teacher & Tech‑Ed Advocate (Education)
- DeepTech Partners (VC)
- Can create agent profiles: `aigit agents add`

## Environment
- Linux WSL2 (x86‑64)
- Rust installed
- SQLite installed
- Git installed
- Workspace: `/home/chris/projects/aigit`

## Commands to Verify Setup
```bash
cd /home/chris/projects/aigit
cargo build
cargo test
cargo run -- --help  # verify all commands

# In a test repository:
mkdir test-repo && cd test-repo && git init
cargo run -- init
echo "test prompt" | cargo run -- commit --agent test --model test --output test.rs
cargo run -- log
cargo run -- context
```

---

*Last updated: 2026‑03‑24 (Phase 1 complete; Phase 3 in progress)*
