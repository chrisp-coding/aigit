# aigit – To-Do List

## Phase 1: Core Infrastructure

- [x] Install Rust via rustup and verify `cargo build` succeeds
- [x] Run migrations and verify `aigit init` creates `.aigit/db.sqlite`
- [x] Wire `git.rs` `get_current_hash()` into `commit` (auto-populate `git_hash`)
- [x] Implement parent commit detection (Git parent first, falls back to last aigit commit by agent)
- [x] Auto-extract artifact paths from `--output` path into `artifacts` field
- [x] Clean up `src/cli.rs.tmp.*` leftover file

## Phase 2: Git Integration

- [x] Connect `get_file_blame()` in `git.rs` to the `blame` command output
- [x] Map Git commit hashes back to aigit commits in `blame` (via `get_commit_by_git_hash`)
- [x] Add `aigit context` command – shows recent aigit commits relevant to a file or repo
      (used by AI agents to load history as context before editing)
- [x] Install Git post-commit hook – retrospective linking + git-message fallback
- [x] Add `aigit status` command (shows modified files with/without aigit coverage)
- [x] Add `aigit branch` command (list / create / delete agent-scoped branches)

## Phase 3: Claude Code Integration (Primary Goal)

The aim is for Claude Code agents to automatically track their work in aigit
and detect/resolve conflicts when multiple agents touch the same files.

### 3a: Auto-tracking via Claude Code Hooks

- [x] Write a Claude Code `PostToolUse` hook that fires after file writes
  - Captures the tool input (file path, content) and calls `aigit commit`
  - Reads agent identity from `AIGIT_AGENT` env var or `.aigit/config.toml`
  - Reads the active prompt/intent from a session context file
- [x] Write a Claude Code `PreToolUse` hook that fires before file writes
  - Checks whether the target file was last touched by a *different* agent
  - If conflict risk detected, writes a warning to stderr so Claude sees it
- [x] Document how to install hooks in `.claude/settings.json` (`hook install --claude` automates this)

### 3b: MCP Server (Agent Query Interface)

- [x] Implement an `aigit mcp` subcommand that starts a Model Context Protocol server
  - Exposes tools: `aigit_log`, `aigit_show`, `aigit_diff`, `aigit_blame`, `aigit_merge`, `aigit_context`, `aigit_conflict_check`
  - Lets Claude Code agents query history and intent without shell commands
- [x] Register the MCP server in project `.mcp.json` for automatic discovery (`aigit mcp --install`)
- [x] Add `aigit_conflict_check` MCP tool: given a file path, returns the last
      agent/commit that touched it and flags if a different agent is about to edit

### 3c: Conflict Detection & Resolution

- [x] Track which files each commit touches — `commit_artifacts` normalized table added (migration `20260318000001`); `insert_commit` writes to both JSON column and table; artifact lookups use indexed `JOIN` instead of `LIKE %path%`
- [x] Add `aigit conflicts` command: shows files where >1 agent has recent commits (`--window N` to limit scan depth)
- [x] Add `aigit merge --output <path>` to write merge result to a file (implemented; `--llm` still falls back to textual merge)
- [x] Implement `aigit merge --llm` using Anthropic API (or local Ollama)
  - Sends both versions + both agents' prompts/intents to an LLM
  - Returns a merged version that reconciles the intents
- [x] Add `aigit resolve <file>` command that invokes LLM merge for a specific file
  - `resolve --llm` auto-commits the result as `agent_id = "aigit-resolver"` so log/blame/conflicts stay accurate
- [x] Add `aigit conflict-check <file>` command — exits 1 if conflict detected; used by PreToolUse hook
- [x] Fix `.mcp.json` to use `"mcpServers"` key (was `"servers"`) for Claude Code automatic discovery

### 3e: Security Hardening

- [x] SSRF prevention — `validate_base_url()` enforces `https://` for Anthropic; loopback-only `http://` for Ollama
- [x] Path traversal guard — `validate_write_path()` on all `--output` writes; `validate_mcp_path()` on MCP file-path arguments
- [x] Prompt injection mitigation — agent content wrapped in data-boundary markers in LLM merge/resolve prompts
- [x] ANSI escape stripping — `strip_ansi()` applied to all LLM output before write/print
- [x] Hook hardening — `set -euo pipefail` and `$FILE` path validation in generated `.claude/hooks/` scripts
- [x] File permission hardening — `config.toml` and `db.sqlite` created `0o600` on Unix
- [x] `DATABASE_URL` pinned in `setup.sh` to prevent accidental use of an ambient remote database URL
- [x] 10 MB cap on MCP message size to prevent memory exhaustion

### 3d: Agent Identity for Claude Code

- [ ] Define a standard agent naming convention for Claude Code sessions
  (e.g., `claude-code:<task-type>` or read from `CLAUDE_AGENT_ID`; hooks default to `claude-code` via `AIGIT_AGENT` env var)
- [ ] Add `aigit agents add` support for Claude Code agent profiles with
      default model, allowed file patterns, and merge priority

## Phase 4: Semantic Features

- [ ] Integrate `all-MiniLM-L6-v2` ONNX model for embedding generation
- [ ] Populate `embeddings` table on each `aigit commit`
- [ ] Implement `aigit diff --semantic` using cosine similarity of embeddings
- [ ] Add `aigit search "<query>"` to find commits by semantic similarity to a prompt
- [ ] Semantic conflict scoring: flag conflicts where intents are semantically opposed

## Phase 5: Polish & Testing

- [x] Add unit tests for `db.rs` (CRUD operations) — 13 tests implemented
- [x] Add integration tests: init → commit → log → diff pipeline — implemented in `tests/integration.rs`
- [ ] Write end-to-end test simulating two Claude Code agents editing the same file
- [ ] Write `CONTRIBUTING.md` and finalize `README.md`
- [ ] Publish initial crate to crates.io

---

## Integration Architecture (Reference)

```
Claude Code session
  │
  ├─ PreToolUse hook ──► aigit conflict_check <file>
  │                            │
  │                     warns if another agent
  │                     recently touched the file
  │
  ├─ [agent edits file]
  │
  └─ PostToolUse hook ──► aigit commit --agent $AIGIT_AGENT \
                                        --intent "$SESSION_INTENT" \
                                        --model "$CLAUDE_MODEL" \
                                        --prompt "$LAST_PROMPT" \
                                        --output <file>

Claude Code MCP tool call
  └─ aigit mcp server ──► aigit_blame, aigit_log, aigit_merge, …
```
