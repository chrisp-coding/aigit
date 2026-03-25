-- Partial index on git_hash: most rows have NULL, so exclude them
DROP INDEX IF EXISTS idx_commits_git_hash;
CREATE INDEX idx_commits_git_hash ON commits(git_hash) WHERE git_hash IS NOT NULL;

-- Composite index for agent-filtered queries with timestamp ordering
CREATE INDEX IF NOT EXISTS idx_commits_agent_timestamp ON commits(agent_id, timestamp DESC);

-- Index on branches(agent_id) for agent-filtered branch lookups
CREATE INDEX IF NOT EXISTS idx_branches_agent_id ON branches(agent_id);

-- Normalized artifacts table for indexed path lookups (replaces LIKE scan on JSON column)
CREATE TABLE IF NOT EXISTS commit_artifacts (
    commit_id TEXT NOT NULL REFERENCES commits(id) ON DELETE CASCADE,
    artifact_path TEXT NOT NULL,
    PRIMARY KEY (commit_id, artifact_path)
);
CREATE INDEX IF NOT EXISTS idx_commit_artifacts_path ON commit_artifacts(artifact_path);

-- Backfill from existing JSON artifacts column
INSERT OR IGNORE INTO commit_artifacts (commit_id, artifact_path)
SELECT c.id, j.value
FROM commits c, json_each(c.artifacts) j
WHERE j.value IS NOT NULL AND j.value != '';
