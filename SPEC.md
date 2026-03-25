# aigit – AI‑Native Version Control

## Vision
A local‑first, Git‑integrated tool that tracks AI‑generated content (code, text, images) as first‑class version‑controlled artifacts. Enables semantic diffing, LLM‑assisted merging, and multi‑agent collaboration by preserving prompt, model, intent, and agent identity alongside each change.

## Why
- **Multi‑agent collaboration:** Specialized AI agents (refactoring, documentation, testing) can work on the same codebase without stepping on each other; the system understands intent and can resolve conflicts intelligently.
- **Provenance & debugging:** Know exactly which prompt/model generated any line of code or text; replay the generative history.
- **Semantic evolution:** Diff not just lines, but quality, tone, and intent shifts across prompt iterations.
- **Privacy:** Everything stays local; optional E2E‑encrypted sync for teams.

## User Stories
### As a developer using multiple AI agents:
1. I want each agent to “sign” its commits with its role (e.g., `--agent frontend‑specialist`) so I can filter the log by agent.
2. I want to see a semantic diff between two prompt versions that shows how the output changed in clarity, conciseness, or style.
3. When two agents edit the same function with different intents (speed vs. readability), I want the system to detect the conflict and suggest a merged version that balances both goals.
4. I want to ask “why did this line change?” and get the prompt and agent that generated it.

### As a team using AI‑generated content:
1. We want to review AI‑generated changes in a PR with full context (prompt, model, parameters).
2. We want to enforce that certain agents (e.g., security‑audit) must review changes before merging.
3. We want to experiment with prompt branches and merge the best results back to main.

## Data Model (SQLite)

### Table: `commits`
| Column | Type | Description |
|--------|------|-------------|
| `id` | TEXT PRIMARY KEY | UUID v7 (time‑ordered) |
| `git_hash` | TEXT | Associated Git commit hash (nullable) |
| `agent_id` | TEXT | Identifier of the AI agent (e.g., “claude‑code‑frontend”) |
| `intent` | TEXT | Human‑readable intent (“make button responsive”, “add error handling”) |
| `prompt` | TEXT | Full prompt text |
| `model` | TEXT | Model identifier (“claude‑3.5‑sonnet”, “gpt‑4”) |
| `parameters` | JSON | `{temperature: 0.7, max_tokens: 1000, …}` |
| `output` | TEXT | Generated content (code, text, markdown) |
| `output_hash` | TEXT | SHA‑256 of output for deduplication |
| `artifacts` | JSON | Paths to generated files (e.g., `[“src/button.rs”, “docs/api.md”]`) |
| `timestamp` | INTEGER | Unix millisecond timestamp |
| `parent_ids` | JSON | Array of parent commit IDs (for branching) |

### Table: `commit_artifacts`
| Column | Type | Description |
|--------|------|-------------|
| `commit_id` | TEXT REFERENCES commits(id) | Link to commit |
| `artifact_path` | TEXT | Path to a generated file |

Normalized form of the JSON `artifacts` column. Added in migration `20260318000001_optimizations.sql` with an index on `artifact_path` and a backfill from existing rows. `insert_commit` writes to this table on every new commit. Artifact lookups use `JOIN commit_artifacts` instead of `LIKE %path%` scans.

### Table: `embeddings`
| Column | Type | Description |
|--------|------|-------------|
| `commit_id` | TEXT REFERENCES commits(id) | Link to commit |
| `output_embedding` | BLOB | Vector embedding of output (via local model) |
| `prompt_embedding` | BLOB | Vector embedding of prompt |
| `created_at` | INTEGER | Timestamp |

### Table: `agents`
| Column | Type | Description |
|--------|------|-------------|
| `agent_id` | TEXT PRIMARY KEY | Unique agent identifier |
| `name` | TEXT | Human‑readable name (“Frontend Specialist”) |
| `description` | TEXT | Capabilities, preferred models, etc. |
| `config` | JSON | Default parameters, allowed models |

## CLI Commands

### Core
```
aigit init                     # Initialize aigit repo (creates .aigit/ dir)
aigit status                   # Show Git-modified files with/without aigit coverage
aigit commit [--agent ID] [--intent TEXT] [--prompt TEXT] [--model ID] [--parameters JSON] [--output FILE]
aigit log [--agent ID] [--since UNIX_MS] [--limit N]  # Show commit history
aigit diff <commit1> <commit2> [--semantic]  # Diff outputs; --semantic prints a warning and falls back to text diff until Phase 4
aigit blame <file> [--lines L1-L2]           # Show which agent/prompt generated each line
aigit show <commit>                          # Show full commit details
aigit context [file] [--limit N] [--json]    # Show aigit history for a file or repo
```

### Collaboration
```
aigit branch list                          # List agent-scoped branches
aigit branch create <name> --agent ID [--intent TEXT]
aigit branch delete <name> --agent ID
aigit merge <source> <target> [--llm] [--output FILE]  # Merge two commits (with intent-annotated conflict markers; --output writes result to file)
aigit conflicts [--window N]               # Files touched by >1 agent in the last N commits (default 10)
aigit agents list                          # List registered agents
aigit agents add <id> --name "..." [--description "..."] [--config JSON]
```

### Integration
```
aigit hook install [--git] [--claude]      # Install hook; --git installs .git/hooks/post-commit;
                                           # --claude writes .claude/hooks/ scripts and patches .claude/settings.json
aigit hook uninstall [--git] [--claude]
aigit hook run <name> [--git-hash HASH]    # Run a specific hook (e.g. post-commit)
aigit hook list                            # List installed hooks (git and Claude Code)
aigit conflict-check <file> [--agent ID] [--window N]  # Exit 1 if file has multi-agent conflicts
aigit resolve <file> [--output FILE] [--llm]            # Merge the two most recent conflicting commits
aigit mcp [--install]                      # Start stdio JSON-RPC 2.0 MCP server; --install writes .mcp.json
```

## Architecture

### Components
1. **CLI (Rust)** – uses `clap` for argument parsing, `sqlx` for SQLite, `git2‑rs` for Git integration.
2. **Embedding service (optional)** – local ONNX model (all‑MiniLM‑L6‑v2) to generate embeddings for semantic diffing; can be disabled.
3. **Merge‑assist LLM** (`src/llm.rs`) – calls Anthropic API or local Ollama to resolve conflicts. Config via `.aigit/config.toml [llm]` section; env vars `ANTHROPIC_API_KEY`, `AIGIT_LLM_PROVIDER`, `AIGIT_LLM_MODEL` override config.
4. **Git hooks** – post‑commit hook to auto‑capture AI‑generated changes when using Claude Code/Cursor.
5. **Claude Code hooks** (`src/mcp.rs`) – `hook install --claude` writes `.claude/hooks/aigit-post-tool.sh` (PostToolUse) and `.claude/hooks/aigit-pre-tool.sh` (PreToolUse); patches `.claude/settings.json`.
6. **MCP server** (`src/mcp.rs`) – stdio JSON-RPC 2.0 server exposing `aigit_log`, `aigit_show`, `aigit_diff`, `aigit_blame`, `aigit_context`, `aigit_conflict_check`, and `aigit_merge` tools.

### Directory layout
```
.aigit/
├── db.sqlite              # SQLite database
├── config.toml            # Agent definitions, model defaults, embedding settings
└── hooks/                 # aigit hook scripts (Git hooks live in .git/hooks/)
```

### Integration with AI editors
- **Claude Code**: Use `--print` output, pipe to `aigit commit`.
- **Cursor**: Use Cursor’s custom command support to call `aigit commit`.
- **General**: Manual `aigit commit` with `--output` flag.

## Phased Roadmap

### Phase 0 (Week 1) – Basic tracking ✅ Complete
- [x] SQLite schema, Rust CLI skeleton.
- [x] `init`, `commit`, `log`, `show` commands.
- [x] Store prompt, model, output, agent, intent.
- [x] Git hash association (optional).
- **Deliverable**: Can manually commit AI‑generated content and view history.

### Phase 1 (Week 2) – Git integration & diffing ✅ Complete
- [x] Git hooks: `hook install --git` installs `.git/hooks/post-commit`; `hook run post-commit` retrospectively links aigit commits to Git hashes.
- [x] `blame` command integrates with `git.rs` Git blame; maps Git commit hashes to aigit commits; line range filter.
- [x] Text‑based `diff` (using `similar` crate).
- [x] `merge` with textual merge and intent‑annotated conflict markers.
- [x] `context` command for AI agents to query history before editing.
- [x] `branch` subcommand (list/create/delete agent‑scoped branches).
- [x] `status` command: modified files with/without aigit coverage.
- [x] Auto‑detect Git hash and parent commit in `commit`.
- [x] Auto‑extract artifact from `--output` path.
- [x] Unit and integration tests.
- **Deliverable**: Full Git integration; line‑level attribution; agent context queries.

### Phase 1.5 – Database optimizations ✅ Complete
- [x] WAL journal mode, `synchronous=NORMAL`, `foreign_keys=ON` set on every connection.
- [x] `commit_artifacts` normalized table with index on `artifact_path`; backfilled from JSON column.
- [x] Partial index on `commits(git_hash) WHERE git_hash IS NOT NULL`.
- [x] Composite index on `commits(agent_id, timestamp DESC)`; index on `branches(agent_id)`.
- [x] `get_commits_by_git_hashes` batch method (single `IN` query) replaces N+1 per-hash lookups.
- [x] `get_artifact_commit_rows` / `ArtifactAgentRow` — targeted conflict query that never loads output/prompt columns.
- [x] N+1 queries eliminated in `blame`, `context`, and `conflicts` commands.

### Phase 2 – Claude Code integration ✅ Complete
- [x] Claude Code PostToolUse hook – auto‑calls `aigit commit` after file writes (`hook install --claude`).
- [x] Claude Code PreToolUse hook – warns when another agent recently touched the target file (`hook install --claude`).
- [x] MCP server (`aigit mcp`) exposing 7 tools over stdio JSON-RPC 2.0; `--install` writes `.mcp.json`.
- [x] `aigit conflicts` command: files where >1 agent has recent commits (`--window N`, default 10).
- [x] LLM‑assisted merge (`merge --llm`) via Anthropic API or local Ollama (`src/llm.rs`).
- [x] `aigit resolve <file>` – per‑file LLM merge invocation; `--output` and `--llm` flags supported.
- [x] `aigit conflict-check <file>` – exits 1 on conflict; used by PreToolUse hook.
- **Deliverable**: Claude Code agents auto‑track work and detect/resolve conflicts.

### Phase 3 – Semantic features (current focus)
- [ ] Embedding generation (local ONNX model, `all‑MiniLM‑L6‑v2`).
- [ ] `diff --semantic` that reports cosine similarity and highlights semantic shifts.
- [ ] `aigit search "<query>"` – find commits by semantic similarity to a prompt.
- [ ] Semantic conflict scoring: flag conflicts where intents are semantically opposed.
- **Deliverable**: Semantic diffing; intent-aware conflict detection.

### Phase 4 (Future) – Polish & ecosystem
- [ ] VS Code extension for visual history.
- [ ] E2E‑encrypted cloud sync (optional).
- [ ] GitHub Actions integration for CI.
- [ ] Open‑source release.

## Testing with MiroFish Simulation
We can reuse the MiroFish container to simulate multi‑agent collaboration:
1. Create a simple Rust project (e.g., a CLI calculator).
2. Define two agents in MiroFish: “refactor‑agent” (optimizes code) and “doc‑agent” (adds comments).
3. Run simulation where each agent makes commits via `aigit` (using the backend API).
4. Observe how `aigit log`, `diff`, and `merge` handle the interaction.
5. Use the simulation to refine conflict‑detection and merge‑assist logic.

## Open Questions
- **Embedding model:** Which local model? `all‑MiniLM‑L6‑v2` is small (80 MB) and fast.
- **Merge‑assist LLM:** Resolved — `src/llm.rs` supports both Anthropic API and Ollama; open question is whether to add OpenAI as a third provider.
- **Git integration depth:** Store aigit commits as Git notes? Keep separate DB?
- **Performance:** SQLite with 10k commits; embedding generation async.

## Next Immediate Steps
1. Integrate `all-MiniLM-L6-v2` ONNX model; populate `embeddings` table on each `aigit commit`.
2. Implement `diff --semantic` using cosine similarity of stored embeddings.
3. Add `aigit search "<query>"` for semantic commit lookup.
4. Add semantic conflict scoring (flag intents that are semantically opposed).

---

*Spec version: 0.3 (2026‑03‑25)*
*Author: Kai (Chris Woodcox’s AI assistant)*