# Fixes Summary

Bug fixes and correctness improvements, newest first.

---

## 2026-03-24 — Database optimizations and correctness fixes

### Critical: `foreign_keys=ON` not enforced on pooled connections
**File**: `src/db.rs` — `Database::connect`
SQLite defaults to foreign key enforcement **off**. The migration file set `PRAGMA foreign_keys = ON`, but that pragma only applies to the connection that runs the migration — every subsequent pooled connection inherited the default (off). This silently disabled `ON DELETE CASCADE` on `embeddings.commit_id` and the FK on `branches.head_commit_id`.
**Fix**: Added `.pragma("foreign_keys", "ON")` to `SqliteConnectOptions` so every connection enforces FK constraints.

### N+1 query in `blame` command
**File**: `src/cli.rs` — `blame()`
For each Git blame entry, `get_commit_by_git_hash` was called once per entry, producing up to N sequential queries for a file with N distinct commit hashes.
**Fix**: Collect all unique hashes, fetch in a single `WHERE git_hash IN (...)` query via `get_commits_by_git_hashes`, look up results from a `HashMap`.

### N+1 query in `context` command
**File**: `src/cli.rs` — `context()`
Same pattern: one `get_commit_by_git_hash` call per hash returned by `get_commits_for_file`.
**Fix**: Same batch lookup approach as `blame`.

### Full table scan on `artifacts` JSON column
**File**: `src/db.rs` — `get_latest_commit_for_artifact`, `get_commits_for_artifact`
Artifact path lookups used `WHERE artifacts LIKE '%path%'` — a leading-wildcard `LIKE` that cannot use an index, forcing a full table scan. Also had false-positive risk (e.g. `src/lib.rs` matching `src/lib.rs.bak`).
**Fix**: Added `commit_artifacts(commit_id, artifact_path)` normalized table (migration `20260318000001`) with an index on `artifact_path`. `insert_commit` writes to both tables; lookup methods now use `JOIN commit_artifacts` with exact matching.

### `conflicts` command loaded entire commits table
**File**: `src/cli.rs` — `conflicts()`
`list_commits(None, u32::MAX, None)` fetched every row including the large `output` and `prompt` columns, then parsed the `artifacts` JSON for each row in Rust.
**Fix**: New `get_artifact_commit_rows()` method queries `commit_artifacts JOIN commits` returning only `(artifact_path, agent_id, intent, commit_id)`. No `output`/`prompt` loaded.

---

## Earlier fixes (pre-2026-03-24)

*(Bug fixes prior to this session were not individually logged. See git history for details.)*
