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

- [ ] Write a Claude Code `PostToolUse` hook that fires after file writes
  - Captures the tool input (file path, content) and calls `aigit commit`
  - Reads agent identity from `AIGIT_AGENT` env var or `.aigit/config.toml`
  - Reads the active prompt/intent from a session context file
- [ ] Write a Claude Code `PreToolUse` hook that fires before file writes
  - Checks whether the target file was last touched by a *different* agent
  - If conflict risk detected, writes a warning to stderr so Claude sees it
- [ ] Document how to install hooks in `.claude/settings.json`

### 3b: MCP Server (Agent Query Interface)

- [ ] Implement an `aigit mcp` subcommand that starts a Model Context Protocol server
  - Exposes tools: `aigit_log`, `aigit_show`, `aigit_diff`, `aigit_blame`, `aigit_merge`
  - Lets Claude Code agents query history and intent without shell commands
- [ ] Register the MCP server in project `.mcp.json` for automatic discovery
- [ ] Add `aigit_conflict_check` MCP tool: given a file path, returns the last
      agent/commit that touched it and flags if a different agent is about to edit

### 3c: Conflict Detection & Resolution

- [ ] Track which files each commit touches (populate `artifacts` field reliably)
- [x] Add `aigit conflicts` command: shows files where >1 agent has recent commits (`--window N` to limit scan depth)
- [x] Add `aigit merge --output <path>` to write merge result to a file (implemented; `--llm` still falls back to textual merge)
- [ ] Implement `aigit merge --llm` using Anthropic API (or local Ollama)
  - Sends both versions + both agents' prompts/intents to an LLM
  - Returns a merged version that reconciles the intents
- [ ] Add `aigit resolve <file>` command that invokes LLM merge for a specific file

### 3d: Agent Identity for Claude Code

- [ ] Define a standard agent naming convention for Claude Code sessions
  (e.g., `claude-code:<task-type>` or read from `CLAUDE_AGENT_ID`)
- [ ] Add `aigit agents add` support for Claude Code agent profiles with
      default model, allowed file patterns, and merge priority

## Phase 4: Semantic Features

- [ ] Integrate `all-MiniLM-L6-v2` ONNX model for embedding generation
- [ ] Populate `embeddings` table on each `aigit commit`
- [ ] Implement `aigit diff --semantic` using cosine similarity of embeddings
- [ ] Add `aigit search "<query>"` to find commits by semantic similarity to a prompt
- [ ] Semantic conflict scoring: flag conflicts where intents are semantically opposed

## Phase 5: Polish & Testing

- [ ] Add unit tests for `db.rs` (CRUD operations)
- [ ] Add integration tests: init → commit → log → diff pipeline
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
