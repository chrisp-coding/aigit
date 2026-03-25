use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous}, FromRow, SqlitePool};
use std::collections::HashMap;
use std::path::Path;
use tracing;

pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn connect(path: impl AsRef<Path>) -> Result<Self> {
        let path_str = path.as_ref().display().to_string();
        tracing::debug!("Connecting to database at {}", path_str);
        
        let options = SqliteConnectOptions::new()
            .filename(&path_str)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .pragma("foreign_keys", "ON");
        
        let pool = SqlitePoolOptions::new()
            .connect_with(options)
            .await?;
        tracing::debug!("Database connection established");
        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> Result<()> {
        tracing::debug!("Running database migrations");
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        tracing::debug!("Migrations completed");
        Ok(())
    }

    /// Insert a new commit into the database.
    pub async fn insert_commit(&self, commit: NewCommit) -> Result<String> {
        let id = uuid::Uuid::now_v7().to_string();
        let output_hash = compute_output_hash(&commit.output);
        let timestamp = chrono::Utc::now().timestamp_millis();
        let parent_ids = serde_json::to_string(&commit.parent_ids)?;
        let artifacts = serde_json::to_string(&commit.artifacts)?;

        sqlx::query(
            r#"
            INSERT INTO commits (
                id, git_hash, agent_id, intent, prompt, model, parameters,
                output, output_hash, artifacts, timestamp, parent_ids
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(commit.git_hash)
        .bind(commit.agent_id)
        .bind(commit.intent)
        .bind(commit.prompt)
        .bind(commit.model)
        .bind(commit.parameters)
        .bind(commit.output)
        .bind(output_hash)
        .bind(artifacts)
        .bind(timestamp)
        .bind(parent_ids)
        .execute(&self.pool)
        .await?;

        // Populate normalized commit_artifacts table
        for artifact in &commit.artifacts {
            if artifact.is_empty() {
                continue;
            }
            sqlx::query(
                r#"INSERT OR IGNORE INTO commit_artifacts (commit_id, artifact_path) VALUES (?, ?)"#,
            )
            .bind(&id)
            .bind(artifact)
            .execute(&self.pool)
            .await?;
        }

        Ok(id)
    }

    /// Retrieve a commit by ID prefix (returns error if ambiguous).
    pub async fn get_commit_by_prefix(&self, prefix: &str) -> Result<Option<Commit>> {
        let pattern = format!("{}%", escape_like(prefix));
        let mut rows = sqlx::query_as::<_, Commit>(
            r#"SELECT * FROM commits WHERE id LIKE ? ESCAPE '\' LIMIT 2"#
        )
        .bind(&pattern)
        .fetch_all(&self.pool)
        .await?;
        match rows.len() {
            0 => Ok(None),
            1 => Ok(Some(rows.remove(0))),
            _ => anyhow::bail!("Ambiguous commit prefix '{}': matches multiple commits", prefix),
        }
    }

    /// Retrieve a commit by Git hash (exact match).
    pub async fn get_commit_by_git_hash(&self, git_hash: &str) -> Result<Option<Commit>> {
        let commit = sqlx::query_as::<_, Commit>(
            r#"SELECT * FROM commits WHERE git_hash = ?"#
        )
        .bind(git_hash)
        .fetch_optional(&self.pool)
        .await?;
        Ok(commit)
    }

    /// Batch-fetch commits by a set of Git hashes; returns a map of git_hash -> Commit.
    pub async fn get_commits_by_git_hashes(&self, hashes: &[String]) -> Result<HashMap<String, Commit>> {
        if hashes.is_empty() {
            return Ok(HashMap::new());
        }
        let mut qb = sqlx::QueryBuilder::new("SELECT * FROM commits WHERE git_hash IN (");
        let mut sep = qb.separated(", ");
        for h in hashes {
            sep.push_bind(h.as_str());
        }
        qb.push(")");
        let commits = qb.build_query_as::<Commit>().fetch_all(&self.pool).await?;
        Ok(commits
            .into_iter()
            .filter_map(|c| c.git_hash.clone().map(|h| (h, c)))
            .collect())
    }

    /// Get the most recent commit by a specific agent (for parent detection fallback).
    pub async fn get_latest_commit_by_agent(&self, agent_id: &str) -> Result<Option<Commit>> {
        let commit = sqlx::query_as::<_, Commit>(
            r#"SELECT * FROM commits WHERE agent_id = ? ORDER BY timestamp DESC LIMIT 1"#
        )
        .bind(agent_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(commit)
    }

    /// List commits with optional filters.
    pub async fn list_commits(
        &self,
        agent: Option<&str>,
        limit: u32,
        since: Option<i64>,
    ) -> Result<Vec<Commit>> {
        let limit = limit as i64;
        let commits = match (agent, since) {
            (Some(a), Some(s)) => sqlx::query_as::<_, Commit>(
                "SELECT * FROM commits WHERE agent_id = ? AND timestamp >= ? ORDER BY timestamp DESC LIMIT ?"
            ).bind(a).bind(s).bind(limit).fetch_all(&self.pool).await?,
            (Some(a), None) => sqlx::query_as::<_, Commit>(
                "SELECT * FROM commits WHERE agent_id = ? ORDER BY timestamp DESC LIMIT ?"
            ).bind(a).bind(limit).fetch_all(&self.pool).await?,
            (None, Some(s)) => sqlx::query_as::<_, Commit>(
                "SELECT * FROM commits WHERE timestamp >= ? ORDER BY timestamp DESC LIMIT ?"
            ).bind(s).bind(limit).fetch_all(&self.pool).await?,
            (None, None) => sqlx::query_as::<_, Commit>(
                "SELECT * FROM commits ORDER BY timestamp DESC LIMIT ?"
            ).bind(limit).fetch_all(&self.pool).await?,
        };
        Ok(commits)
    }

    /// Insert a new agent into the database.
    pub async fn insert_agent(
        &self,
        agent_id: &str,
        name: &str,
        description: Option<&str>,
        config: &str,
    ) -> Result<()> {
        // Validate config is valid JSON
        let _: serde_json::Value = serde_json::from_str(config)
            .map_err(|e| anyhow::anyhow!("Invalid JSON config: {}", e))?;

        sqlx::query(
            r#"INSERT INTO agents (agent_id, name, description, config) VALUES (?, ?, ?, ?)"#
        )
        .bind(agent_id)
        .bind(name)
        .bind(description)
        .bind(config)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// List all registered agents.
    pub async fn list_agents(&self) -> Result<Vec<Agent>> {
        let agents = sqlx::query_as::<_, Agent>(
            r#"SELECT * FROM agents ORDER BY created_at DESC"#
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(agents)
    }

    // --- Branch methods ---

    /// Create a new agent-specific branch.
    pub async fn insert_branch(
        &self,
        name: &str,
        agent_id: &str,
        intent: Option<&str>,
        head_commit_id: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO branches (name, agent_id, intent, head_commit_id) VALUES (?, ?, ?, ?)"#
        )
        .bind(name)
        .bind(agent_id)
        .bind(intent)
        .bind(head_commit_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// List all branches.
    pub async fn list_branches(&self) -> Result<Vec<Branch>> {
        let branches = sqlx::query_as::<_, Branch>(
            r#"SELECT * FROM branches ORDER BY created_at DESC"#
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(branches)
    }

    /// List all branches for a specific agent.
    pub async fn list_branches_for_agent(&self, agent_id: &str) -> Result<Vec<Branch>> {
        let branches = sqlx::query_as::<_, Branch>(
            r#"SELECT * FROM branches WHERE agent_id = ? ORDER BY created_at DESC"#
        )
        .bind(agent_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(branches)
    }

    /// Delete a branch. Returns true if a row was deleted, false if not found.
    pub async fn delete_branch(&self, name: &str, agent_id: &str) -> Result<bool> {
        let result = sqlx::query(
            r#"DELETE FROM branches WHERE name = ? AND agent_id = ?"#
        )
        .bind(name)
        .bind(agent_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Advance a branch's HEAD to a new commit.
    pub async fn set_branch_head(&self, name: &str, agent_id: &str, commit_id: &str) -> Result<()> {
        let now = chrono::Utc::now().timestamp_millis();
        sqlx::query(
            r#"UPDATE branches SET head_commit_id = ?, updated_at = ? WHERE name = ? AND agent_id = ?"#
        )
        .bind(commit_id)
        .bind(now)
        .bind(name)
        .bind(agent_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // --- Status helpers ---

    /// Find the most recent aigit commit that touches the given artifact path.
    pub async fn get_latest_commit_for_artifact(&self, path: &str) -> Result<Option<Commit>> {
        let commit = sqlx::query_as::<_, Commit>(
            r#"SELECT c.* FROM commits c
               JOIN commit_artifacts ca ON ca.commit_id = c.id
               WHERE ca.artifact_path = ?
               ORDER BY c.timestamp DESC LIMIT 1"#,
        )
        .bind(path)
        .fetch_optional(&self.pool)
        .await?;
        Ok(commit)
    }

    /// Return all commits that reference the given artifact path, ordered newest first.
    #[allow(dead_code)]
    pub async fn get_commits_for_artifact(&self, path: &str) -> Result<Vec<Commit>> {
        let commits = sqlx::query_as::<_, Commit>(
            r#"SELECT c.* FROM commits c
               JOIN commit_artifacts ca ON ca.commit_id = c.id
               WHERE ca.artifact_path = ?
               ORDER BY c.timestamp DESC"#,
        )
        .bind(path)
        .fetch_all(&self.pool)
        .await?;
        Ok(commits)
    }

    /// Return (artifact_path, agent_id, intent, commit_id) rows for the conflicts command.
    /// Uses the normalized commit_artifacts table to avoid loading prompt/output columns.
    pub async fn get_artifact_commit_rows(&self) -> Result<Vec<ArtifactAgentRow>> {
        let rows = sqlx::query_as::<_, ArtifactAgentRow>(
            r#"SELECT ca.artifact_path, c.agent_id, c.intent, c.id AS commit_id
               FROM commit_artifacts ca
               JOIN commits c ON c.id = ca.commit_id
               ORDER BY c.timestamp DESC"#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // --- Hook helpers ---

    /// Return all commits with no git_hash that were created at or after `since_ms`.
    pub async fn get_unlinked_commits_since(&self, since_ms: i64) -> Result<Vec<Commit>> {
        let commits = sqlx::query_as::<_, Commit>(
            r#"SELECT * FROM commits WHERE git_hash IS NULL AND timestamp >= ? ORDER BY timestamp ASC"#
        )
        .bind(since_ms)
        .fetch_all(&self.pool)
        .await?;
        Ok(commits)
    }

    /// Return all commits whose git_hash matches a specific hash (e.g. an old parent hash)
    /// and were created at or after `since_ms`. Used by the post-commit hook to re-link
    /// aigit commits that captured the pre-commit HEAD instead of NULL.
    pub async fn get_commits_with_git_hash_since(
        &self,
        git_hash: &str,
        since_ms: i64,
    ) -> Result<Vec<Commit>> {
        let commits = sqlx::query_as::<_, Commit>(
            r#"SELECT * FROM commits WHERE git_hash = ? AND timestamp >= ? ORDER BY timestamp ASC"#,
        )
        .bind(git_hash)
        .bind(since_ms)
        .fetch_all(&self.pool)
        .await?;
        Ok(commits)
    }

    /// Set the git_hash for a specific commit.
    pub async fn set_git_hash(&self, commit_id: &str, git_hash: &str) -> Result<()> {
        sqlx::query(r#"UPDATE commits SET git_hash = ? WHERE id = ?"#)
            .bind(git_hash)
            .bind(commit_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Commit {
    pub id: String,
    pub git_hash: Option<String>,
    pub agent_id: String,
    pub intent: Option<String>,
    pub prompt: String,
    pub model: String,
    pub parameters: String,
    pub output: String,
    pub output_hash: String,
    pub artifacts: String,
    pub timestamp: i64,
    pub parent_ids: String,
    pub created_at: i64,
}

#[derive(Debug, FromRow)]
#[allow(dead_code)]
pub struct Agent {
    pub agent_id: String,
    pub name: String,
    pub description: Option<String>,
    pub config: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, FromRow)]
#[allow(dead_code)]
pub struct Branch {
    pub name: String,
    pub agent_id: String,
    pub intent: Option<String>,
    pub head_commit_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, FromRow)]
#[allow(dead_code)]
pub struct ArtifactAgentRow {
    pub artifact_path: String,
    pub agent_id: String,
    pub intent: Option<String>,
    pub commit_id: String,
}

#[derive(Debug)]
pub struct NewCommit {
    pub git_hash: Option<String>,
    pub agent_id: String,
    pub intent: Option<String>,
    pub prompt: String,
    pub model: String,
    pub parameters: String,
    pub output: String,
    pub artifacts: Vec<String>,
    pub parent_ids: Vec<String>,
}

fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

fn compute_output_hash(output: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(output.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    async fn setup() -> (Database, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let db = Database::connect(dir.path().join("test.sqlite")).await.unwrap();
        db.migrate().await.unwrap();
        (db, dir)
    }

    fn sample_commit(agent: &str) -> NewCommit {
        NewCommit {
            git_hash: None,
            agent_id: agent.to_string(),
            intent: Some("test intent".to_string()),
            prompt: "do something useful".to_string(),
            model: "claude-3.5-sonnet".to_string(),
            parameters: "{}".to_string(),
            output: "fn main() {}".to_string(),
            artifacts: vec![],
            parent_ids: vec![],
        }
    }

    #[tokio::test]
    async fn test_insert_and_retrieve_commit() {
        let (db, _dir) = setup().await;
        let id = db.insert_commit(sample_commit("agent-a")).await.unwrap();

        let commit = db.get_commit_by_prefix(&id[..8]).await.unwrap();
        assert!(commit.is_some());
        let c = commit.unwrap();
        assert_eq!(c.id, id);
        assert_eq!(c.agent_id, "agent-a");
        assert_eq!(c.intent.as_deref(), Some("test intent"));
        assert_eq!(c.model, "claude-3.5-sonnet");
    }

    #[tokio::test]
    async fn test_full_id_lookup() {
        let (db, _dir) = setup().await;
        let id = db.insert_commit(sample_commit("agent-a")).await.unwrap();
        let commit = db.get_commit_by_prefix(&id).await.unwrap();
        assert!(commit.is_some());
    }

    #[tokio::test]
    async fn test_prefix_not_found_returns_none() {
        let (db, _dir) = setup().await;
        let result = db.get_commit_by_prefix("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_ambiguous_prefix_errors() {
        let (db, _dir) = setup().await;
        // Insert two commits and manufacture a shared prefix by checking the IDs.
        // UUID v7 shares a time-based prefix — insert both and use a 1-char prefix
        // that is guaranteed to match both (the hex alphabet means 1 char = 1/16 chance;
        // instead we just verify the error path by using an empty string prefix).
        db.insert_commit(sample_commit("agent-a")).await.unwrap();
        db.insert_commit(sample_commit("agent-b")).await.unwrap();
        // Empty prefix matches everything — should be ambiguous with 2+ rows.
        let result = db.get_commit_by_prefix("").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Ambiguous"));
    }

    #[tokio::test]
    async fn test_output_hash_stored() {
        let (db, _dir) = setup().await;
        let output = "hello world";
        let expected_hash = compute_output_hash(output);
        let mut nc = sample_commit("agent-a");
        nc.output = output.to_string();
        let id = db.insert_commit(nc).await.unwrap();
        let c = db.get_commit_by_prefix(&id).await.unwrap().unwrap();
        assert_eq!(c.output_hash, expected_hash);
    }

    #[tokio::test]
    async fn test_list_commits_no_filter() {
        let (db, _dir) = setup().await;
        db.insert_commit(sample_commit("agent-a")).await.unwrap();
        db.insert_commit(sample_commit("agent-b")).await.unwrap();
        let commits = db.list_commits(None, 10, None).await.unwrap();
        assert_eq!(commits.len(), 2);
    }

    #[tokio::test]
    async fn test_list_commits_agent_filter() {
        let (db, _dir) = setup().await;
        db.insert_commit(sample_commit("agent-a")).await.unwrap();
        db.insert_commit(sample_commit("agent-b")).await.unwrap();
        db.insert_commit(sample_commit("agent-a")).await.unwrap();

        let commits = db.list_commits(Some("agent-a"), 10, None).await.unwrap();
        assert_eq!(commits.len(), 2);
        assert!(commits.iter().all(|c| c.agent_id == "agent-a"));
    }

    #[tokio::test]
    async fn test_list_commits_limit() {
        let (db, _dir) = setup().await;
        for _ in 0..5 {
            db.insert_commit(sample_commit("agent-a")).await.unwrap();
        }
        let commits = db.list_commits(None, 3, None).await.unwrap();
        assert_eq!(commits.len(), 3);
    }

    #[tokio::test]
    async fn test_list_commits_since_filter() {
        let (db, _dir) = setup().await;
        let before = chrono::Utc::now().timestamp_millis();
        db.insert_commit(sample_commit("agent-a")).await.unwrap();
        let after = chrono::Utc::now().timestamp_millis();

        let none = db.list_commits(None, 10, Some(after + 1000)).await.unwrap();
        assert_eq!(none.len(), 0);

        let some = db.list_commits(None, 10, Some(before)).await.unwrap();
        assert_eq!(some.len(), 1);
    }

    #[tokio::test]
    async fn test_get_commit_by_git_hash() {
        let (db, _dir) = setup().await;
        let mut nc = sample_commit("agent-a");
        nc.git_hash = Some("abc123def456".to_string());
        let id = db.insert_commit(nc).await.unwrap();

        let found = db.get_commit_by_git_hash("abc123def456").await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, id);

        let not_found = db.get_commit_by_git_hash("doesnotexist").await.unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_insert_and_list_agents() {
        let (db, _dir) = setup().await;
        db.insert_agent("ai-1", "AI Agent One", Some("does stuff"), "{}").await.unwrap();
        db.insert_agent("ai-2", "AI Agent Two", None, r#"{"key":"val"}"#).await.unwrap();

        let agents = db.list_agents().await.unwrap();
        assert_eq!(agents.len(), 2);
        assert!(agents.iter().any(|a| a.agent_id == "ai-1"));
        assert!(agents.iter().any(|a| a.agent_id == "ai-2"));
    }

    #[tokio::test]
    async fn test_insert_agent_invalid_json_fails() {
        let (db, _dir) = setup().await;
        let result = db.insert_agent("bad", "Bad Agent", None, "not json").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid JSON"));
    }

    #[tokio::test]
    async fn test_compute_output_hash_deterministic() {
        let h1 = compute_output_hash("hello");
        let h2 = compute_output_hash("hello");
        let h3 = compute_output_hash("world");
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
        // SHA-256 produces 64 hex chars
        assert_eq!(h1.len(), 64);
    }
}