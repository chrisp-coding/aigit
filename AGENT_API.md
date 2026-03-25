# aigit – AI Agent Integration Guide

This document describes how an AI agent should interact with aigit to get project context and record its work.

## Core Workflow

An agent working on a file should follow three steps:

### 1. Before editing — query context

Ask aigit what has happened to the file before touching it. This gives you the intent behind past changes so you can make decisions that are consistent with prior work.

```bash
# Get the history and intent behind a specific file
aigit context src/auth.rs

# Get recent project-wide history
aigit context --limit 5

# Machine-readable output (JSON)
aigit context src/auth.rs --json
```

Example output:
```
Context for: src/auth.rs
──────────────────────────────────────────────────────────────────────
[1] 2026-03-19 14:23 | claude-code | intent: "add JWT validation"
    git: abc12345 | aigit: 9f3a1b2c
    prompt: "Add JWT token validation to the auth middleware. Tokens should expire after 24h..."

[2] 2026-03-18 09:11 | frontend-agent | intent: "fix session cookie handling"
    git: def67890 | aigit: 4e2d8a1f
    prompt: "The session cookie is not being cleared on logout. Fix the auth middleware..."
```

Use this to understand:
- Why the code is structured the way it is
- What problems previous agents were solving
- What constraints or decisions were already made

### 2. After editing — commit with intent

After making changes, record the prompt, your agent ID, and a clear intent. Always include `--intent` — this is the primary signal future agents use to understand your work.

```bash
aigit commit \
  --agent "claude-code" \
  --model "claude-sonnet-4-6" \
  --intent "refactor auth to use RS256 signing" \
  --prompt "The current JWT implementation uses HS256. Migrate to RS256 with a rotating key pair..." \
  --output src/auth.rs
```

The `git_hash` is auto-detected from the current Git HEAD. The parent aigit commit is inferred from Git history — no manual linking needed.

### 3. During conflicts — detect and merge with intent context

To find out which files have been touched by more than one agent, run:

```bash
aigit conflicts          # scan the last 10 commits per file (default)
aigit conflicts --window 20  # expand the scan window
```

Output lists each conflicting file followed by the agents that touched it and their most recent intents, giving you the information you need to decide how to resolve the divergence.

When two agents have diverged on the same file, `aigit merge` shows the intent of each side inside the conflict markers, not just the code:

```bash
aigit merge <source-commit-id> <target-commit-id>

# Write the merged result to a file instead of stdout
aigit merge <source-commit-id> <target-commit-id> --output merged.rs
```

Conflict markers include the agent name and intent:
```
<<<<<<< 9f3a1b2c | claude-code | intent: "add RS256 signing"
... source code ...
=======
... target code ...
>>>>>>> 4e2d8a1f | frontend-agent | intent: "add token refresh"
```

Use the intent labels to decide which side to keep or how to combine them.

## Command Reference

| Command | Purpose |
|---------|---------|
| `aigit context [file]` | Get intent history for a file or repo |
| `aigit commit --intent "..."` | Record an AI-generated change with its intent |
| `aigit log --agent <id>` | See all commits by a specific agent |
| `aigit show <id>` | Inspect full prompt, output, and metadata for a commit |
| `aigit diff <id1> <id2>` | Diff two aigit commits (shows what changed) |
| `aigit merge <src> <tgt>` | Merge two commits with intent-annotated conflict markers |
| `aigit merge <src> <tgt> --output <file>` | Same, but write the result to a file instead of stdout |
| `aigit blame <file>` | See which agent (and why) wrote each block of a file |
| `aigit conflicts` | List files where more than one agent has recent commits |
| `aigit conflicts --window N` | Same, limited to the N most recent commits per file (default 10) |

## Tips for Agents

- **Always set `--intent`**: A one-line summary of what you were trying to accomplish. Future agents (and humans) will read this to understand your work.
- **Match your prompt exactly**: Pass the actual prompt you used, not a summary. This is used for semantic search and conflict resolution in later phases.
- **Check context before major rewrites**: If you're about to change a file significantly, run `aigit context <file>` first. Prior intent may reveal constraints you shouldn't break.
- **Use `--json` for scripting**: `aigit context --json` returns a machine-readable list of commits you can pipe into other tools.
