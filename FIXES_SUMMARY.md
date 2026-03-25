# aigit Bug Fixes & Improvements - March 19, 2026

## Fixed Issues

### 1. ✅ `show` command prefix matching bug
**Problem:** `show` used `get_commit()` which requires full UUID, not prefixes.
**Fix:** Changed to `get_commit_by_prefix()` in `src/cli.rs`.
**Test:** `aigit show 019d0817` now works (was: "Commit not found").

### 2. ✅ Git integration enabled
**Problem:** `git2` dependency was commented out in `Cargo.toml`.
**Fix:** Uncommented `git2 = { version = "0.18", optional = true }`.
**Note:** Git features still need implementation in `git.rs`.

### 3. ✅ Semantic diff flag handling
**Problem:** `diff --semantic` flag was ignored.
**Fix:** Added check in `diff()` function that prints note about Phase 4.
**Test:** `aigit diff --semantic 019d0817 019d0818` shows note.

### 4. ✅ Basic `blame` implementation
**Problem:** `blame` was just a stub ("not yet implemented").
**Fix:** Implemented basic version that:
- Takes file path argument
- Searches commits where file appears in artifacts (parses JSON)
- Shows commit ID, intent, agent
- Notes that full Git integration is Phase 1

### 5. ✅ Basic `hook` implementation
**Problem:** `hook` was just a stub.
**Fix:** Implemented three subcommands:
- `hook install`: Creates example pre-commit hook script
- `hook run <name>`: Placeholder (Phase 1)
- `hook list`: Lists hooks in `.aigit/hooks/`

## Files Modified (2026-03-19)

### `src/cli.rs`
- Line ~228: `show()` now uses `get_commit_by_prefix()`
- Line ~258: `diff()` checks `args.semantic` and prints note
- Lines ~210-245: `blame()` basic implementation
- Lines ~310-360: `hook()` basic implementation
- Line ~4: Added `serde_json` import

### `Cargo.toml`
- Line ~24: Uncommented `git2` dependency

## Remaining Issues / Phase 1+ Features (as of 2026-03-19)

### Still Stubbed / Minimal (at time of this log)
- **Git integration (`git.rs`):** Still just `todo!()` - needs actual Git operations
- **`blame` with real Git blame:** Current version searches artifacts, not Git history
- **`hook` automation:** Example script created, but no auto-tracking
- **Semantic diff:** Flag acknowledged, but no embeddings/vector search

### Potential Bugs (at time of this log)
- `blame` might not find files if artifacts field is empty/null (depends on `commit` command)
- `hook install` creates Unix-only permissions (uses `#[cfg(unix)]`)

## Testing Results (2026-03-19)
All modified commands execute without panic:
- ✅ `show` with prefix works
- ✅ `diff --semantic` prints note
- ✅ `blame <file>` runs (may find no commits if artifacts empty)
- ✅ `hook install/list` work
- ✅ Original functionality (`init`, `commit`, `log`, `agents`) unchanged

---

## Phase 1 Completion (2026-03-24)

All items listed as "remaining" above have been resolved:

- ✅ **`git.rs` fully implemented**: `get_current_hash`, `get_repo_root`, `get_parent_hash`, `get_parent_timestamp`, `get_head_commit_message`, `get_commits_for_file`, `get_modified_files`, `get_file_blame`
- ✅ **`blame` wired to Git blame**: Maps Git commit hashes to aigit commits via `get_commit_by_git_hash`; falls back to artifact search when not in a Git repo; `--lines` range filter supported
- ✅ **`hook install --git`** installs a real `.git/hooks/post-commit`; `hook run post-commit` retrospectively links unlinked aigit commits to the current Git hash; fallback records the Git commit message as a new aigit commit
- ✅ **`commit`** auto-detects Git hash and parent commits (Git parent first, falls back to last aigit commit by agent); auto-extracts artifact from `--output` path
- ✅ **New commands**: `context` (aigit history for a file or repo, `--json` output), `branch` (list/create/delete agent-scoped branches), `status` (modified files with/without aigit coverage)
- ✅ **Unit tests** in `db.rs` (13 tests covering CRUD, filtering, hash lookup, agents)
- ✅ **Integration tests** in `tests/integration.rs` (16 tests covering all major commands)

**Remaining stubs**: `--semantic` diff (Phase 4 - needs embeddings model), `--llm` merge (Phase 3 - needs LLM integration).

---

## Phase 3 Fixes (2026-03-24)

### 1. ✅ Git hash timing bug fixed (`hook run post-commit`)

**Problem:** When `aigit commit` runs *before* `git commit` (the common case when using Claude Code), it calls `get_current_hash()` which returns the *current* HEAD — the parent of the commit that is about to be made. The post-commit hook was only patching commits whose `git_hash` was NULL, so commits already stamped with the old parent hash were never updated to the new Git hash. This caused `aigit blame` to fail to link lines to the correct agent.

**Fix:** The `hook run post-commit` handler now also fetches commits whose `git_hash` equals the old parent hash (via the new `db.get_commits_with_git_hash_since()` method) and updates them to the new `$GIT_HASH`, in addition to the existing NULL-hash path. Both sets are de-duplicated before the bulk update.

**Files changed:** `src/cli.rs` (`HookCommands::Run`), `src/db.rs` (`get_commits_with_git_hash_since`).

### 2. ✅ `aigit diff --semantic` now falls back gracefully

**Problem:** When `--semantic` was passed, the diff command exited with an error (unimplemented behaviour from a previous stub).

**Fix:** Instead of erroring, the command now prints a warning to stderr — `Warning: --semantic is not yet implemented (planned for Phase 4: embeddings). Falling back to textual diff.` — and then continues with the standard textual diff. The flag is a no-op for the actual diff output until Phase 4 embeddings are implemented.

**Files changed:** `src/cli.rs` (`diff()`).

### 3. ✅ `aigit merge --output <path>` added

**Problem:** The merge command wrote its result only to stdout, making it awkward to use the merged content in a subsequent workflow step.

**Fix:** Added an optional `--output <path>` flag to `MergeArgs`. When provided, the merged result (including any conflict markers) is written to the specified file path and a confirmation line is printed to stdout. Without `--output` the behaviour is unchanged (result printed to stdout).

**Files changed:** `src/cli.rs` (`MergeArgs`, `merge()`).

### 4. ✅ `aigit hook list` reports git-installed hooks

**Problem:** After running `aigit hook install --git`, `aigit hook list` reported "No aigit hooks installed." because it only scanned `.aigit/hooks/` and had no awareness of the Git-side hook it had placed in `.git/hooks/`.

**Fix:** `HookCommands::List` now also checks `.git/hooks/post-commit` for the aigit signature string `hook run post-commit`. If found, it prints `[git]   post-commit  (aigit-managed)` along with the hook's file path. Internally managed aigit hooks continue to be listed as `[aigit] <name>`.

**Files changed:** `src/cli.rs` (`HookCommands::List`).

### 5. ✅ `aigit conflicts` command added

**Problem:** There was no way to ask aigit which files had been touched by more than one agent, making multi-agent conflict detection a manual process.

**Fix:** New `conflicts` subcommand wired in `src/main.rs`. It loads all commits ordered newest-first, scans each commit's artifact paths, and builds a per-file map of `agent_id → most_recent_intent`. The `--window N` flag (default 10) stops counting a given file once N commits have been seen for it (across all agents), bounding the scan to recent activity. Files where two or more distinct agents appear are printed with each agent's most recent intent. If no conflicts exist, a clear "No multi-agent conflicts detected." message is shown.

**Files changed:** `src/main.rs` (routing), `src/cli.rs` (`ConflictsArgs`, `conflicts()`).
