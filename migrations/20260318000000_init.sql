-- Enable foreign keys
PRAGMA foreign_keys = ON;

-- Table: commits
CREATE TABLE commits (
    id TEXT PRIMARY KEY,                     -- UUID v7
    git_hash TEXT,                           -- Associated Git commit hash
    agent_id TEXT NOT NULL,                  -- Agent identifier
    intent TEXT,                             -- Human-readable intent
    prompt TEXT NOT NULL,                    -- Full prompt text
    model TEXT NOT NULL,                     -- Model identifier
    parameters TEXT NOT NULL DEFAULT '{}',   -- JSON parameters
    output TEXT NOT NULL,                    -- Generated content
    output_hash TEXT NOT NULL,               -- SHA-256 of output
    artifacts TEXT NOT NULL DEFAULT '[]',    -- JSON array of file paths
    timestamp INTEGER NOT NULL,              -- Unix milliseconds
    parent_ids TEXT NOT NULL DEFAULT '[]',   -- JSON array of parent commit IDs
    created_at INTEGER NOT NULL DEFAULT (unixepoch('now', 'subsec') * 1000)
);

-- Indexes for common queries
CREATE INDEX idx_commits_agent ON commits(agent_id);
CREATE INDEX idx_commits_git_hash ON commits(git_hash);
CREATE INDEX idx_commits_timestamp ON commits(timestamp);
CREATE INDEX idx_commits_output_hash ON commits(output_hash);

-- Table: embeddings
CREATE TABLE embeddings (
    commit_id TEXT NOT NULL REFERENCES commits(id) ON DELETE CASCADE,
    output_embedding BLOB,                   -- Vector embedding of output
    prompt_embedding BLOB,                   -- Vector embedding of prompt
    created_at INTEGER NOT NULL DEFAULT (unixepoch('now', 'subsec') * 1000),
    PRIMARY KEY (commit_id)
);

-- Table: agents
CREATE TABLE agents (
    agent_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    config TEXT NOT NULL DEFAULT '{}',       -- JSON config
    created_at INTEGER NOT NULL DEFAULT (unixepoch('now', 'subsec') * 1000),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch('now', 'subsec') * 1000)
);

-- Table: branches (agent-specific branches)
CREATE TABLE branches (
    name TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    intent TEXT,
    head_commit_id TEXT REFERENCES commits(id),
    created_at INTEGER NOT NULL DEFAULT (unixepoch('now', 'subsec') * 1000),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch('now', 'subsec') * 1000),
    PRIMARY KEY (name, agent_id)
);