# aigit – AI-Native Version Control

*Local-first version control for AI-generated content, built for multi-agent collaboration.*

## What

`aigit` is a CLI tool that tracks AI-generated artifacts (code, text, prompts) as first-class version-controlled objects. Alongside each commit it stores:

- **Who** generated it — agent identity and model
- **Why** it was generated — intent and full prompt
- **What** it produced — output content and file paths

This makes AI-generated workflows reproducible, auditable, and collaborative across multiple agents.

Regular Git tracks *what* changed. `aigit` tracks **why** it changed and **which agent** changed it.

## Quick Start

### Prerequisites

- Rust (install via [rustup](https://rustup.rs))
- SQLite (system package — usually pre-installed)

### Install

```bash
git clone https://github.com/chrisp-coding/aigit
cd aigit
cargo install --path .
```

### Initialize a repository

```bash
cd your-project
aigit init
```

### Record an AI-generated commit

```bash
# Prompt from flag, output from file
aigit commit \
  --agent claude-code \
  --intent "refactor auth module" \
  --prompt "Extract the JWT logic into a separate module" \
  --model claude-sonnet-4-6 \
  --output src/auth.rs

# Prompt from stdin, output from file
echo "Add error handling to the parser" | aigit commit \
  --agent claude-code \
  --model claude-sonnet-4-6 \
  --output src/parser.rs
```

### Explore history

```bash
# List recent commits
aigit log

# Filter by agent
aigit log --agent claude-code

# Show full details of a commit
aigit show <commit-id-prefix>

# Text diff between two commits
aigit diff <commit1> <commit2>

# Attempt semantic diff (prints a warning and falls back to text diff until Phase 4 embeddings are ready)
aigit diff --semantic <commit1> <commit2>

# Blame a file — map lines back to agents/prompts
aigit blame src/auth.rs

# Show context relevant to a file (for AI agents loading history)
aigit context src/auth.rs

# Show files where more than one agent has recent commits
aigit conflicts

# Limit conflict scan to the last 20 commits per file (default is 10)
aigit conflicts --window 20
```

### Manage agents and branches

```bash
# Register an agent profile
aigit agents add my-agent --name "My Agent" --description "Refactoring specialist"

# List agents
aigit agents list

# Create an agent-scoped branch
aigit branch create feature-x --agent my-agent --intent "build feature X"

# Show aigit coverage of modified files
aigit status
```

## Feature Status

| Feature | Status |
|---|---|
| `init`, `commit`, `log`, `show` | ✅ Implemented |
| Text diff (`diff`) | ✅ Implemented |
| Text merge with conflict markers (`merge`) | ✅ Implemented (`--output <path>` writes result to file) |
| LLM-assisted merge (`merge --llm`) | ✅ Implemented (Anthropic API or local Ollama via `src/llm.rs`) |
| Per-file LLM conflict resolution (`resolve`) | ✅ Implemented (`--llm` auto-commits result as `aigit-resolver`) |
| Conflict check with exit code (`conflict-check`) | ✅ Implemented (exits 1 on conflict; used by PreToolUse hook) |
| Git blame integration (`blame`) | ✅ Implemented |
| Agent management (`agents`) | ✅ Implemented |
| Agent-scoped branches (`branch`) | ✅ Implemented |
| Working tree coverage (`status`) | ✅ Implemented |
| Context command for AI agents (`context`) | ✅ Implemented |
| Git hook installation (`hook`) | ✅ Implemented (`hook install --git` installs post-commit; `hook install --claude` installs Claude Code hooks) |
| Conflict detection (`conflicts`) | ✅ Implemented (`--window N` limits commit scan depth) |
| Claude Code auto-tracking hooks | ✅ Implemented (`hook install --claude` writes PostToolUse/PreToolUse hooks) |
| MCP server for agent queries | ✅ Implemented (`aigit mcp`; `--install` writes `.mcp.json`) |
| Semantic diff (`diff --semantic`) | 🔄 Phase 4 — falls back to textual diff with a warning |
| Embeddings + semantic search | 🔄 Phase 4 |

## Roadmap

- **Phase 1** ✅ — Core tracking: `init`, `commit`, `log`, `show`, `diff`, `blame`, `merge`, `context`
- **Phase 2** ✅ — Git integration: hook scripts, blame wiring, branch management, status
- **Phase 2 (Claude Code)** ✅ — Auto-tracking hooks, MCP server, conflict detection, LLM merge, `resolve`, `conflict-check`
- **Phase 2.5** ✅ — Security hardening: SSRF prevention, path traversal guards, prompt injection mitigation, hook hardening, file permissions
- **Phase 3** 🔄 — Semantic features: embeddings (`all-MiniLM-L6-v2`), similarity search, semantic diff
- **Phase 4** 🔄 — Polish: crates.io publish, VS Code extension, cloud sync

## Development

```bash
cargo build          # compile
cargo test           # run all tests
cargo run -- --help  # CLI help
```

See [SPEC.md](./SPEC.md) for detailed architecture and data model.
See [AGENT_API.md](./AGENT_API.md) for how AI agents should interact with aigit.

## License

MIT OR Apache-2.0
