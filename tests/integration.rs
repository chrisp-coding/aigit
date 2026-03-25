use aigit::cli::{
    AgentCommands, BlameArgs, CommitArgs, ConflictsArgs, ContextArgs, DiffArgs, LogArgs,
    MergeArgs, ShowArgs,
};
use tempfile::tempdir;

// Helper: init a fresh repo in a tempdir and return the dir.
async fn init_repo() -> tempfile::TempDir {
    let dir = tempdir().unwrap();
    aigit::cli::init(dir.path()).await.unwrap();
    assert!(dir.path().join(".aigit/db.sqlite").exists());
    dir
}

fn commit_args(agent: &str, prompt: &str, output_path: &std::path::Path) -> CommitArgs {
    CommitArgs {
        agent: agent.to_string(),
        intent: Some("test intent".to_string()),
        prompt: Some(prompt.to_string()),
        model: "test-model".to_string(),
        parameters: "{}".to_string(),
        output: Some(output_path.to_string_lossy().to_string()),
        git_hash: None,
    }
}

fn write_output(dir: &std::path::Path, name: &str, content: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, content).unwrap();
    path
}

// ── init ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_init_creates_aigit_dir() {
    let dir = tempdir().unwrap();
    aigit::cli::init(dir.path()).await.unwrap();
    assert!(dir.path().join(".aigit").is_dir());
    assert!(dir.path().join(".aigit/db.sqlite").exists());
    assert!(dir.path().join(".aigit/hooks").is_dir());
}

#[tokio::test]
async fn test_init_twice_errors() {
    let dir = tempdir().unwrap();
    aigit::cli::init(dir.path()).await.unwrap();
    let result = aigit::cli::init(dir.path()).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("already initialized"));
}

// ── commit ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_commit_succeeds() {
    let dir = init_repo().await;
    let out = write_output(dir.path(), "out.rs", "fn main() { println!(\"hello\"); }");
    let args = commit_args("agent-a", "write hello world", &out);
    aigit::cli::commit(args, dir.path()).await.unwrap();
}

#[tokio::test]
async fn test_commit_without_init_errors() {
    let dir = tempdir().unwrap();
    let out = write_output(dir.path(), "out.txt", "output");
    let args = commit_args("agent-a", "prompt", &out);
    let result = aigit::cli::commit(args, dir.path()).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not initialized"));
}

// ── log ───────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_log_empty_repo() {
    let dir = init_repo().await;
    // Should succeed with "No commits found." message, not error.
    aigit::cli::log(LogArgs { agent: None, limit: 20, since: None }, dir.path())
        .await
        .unwrap();
}

#[tokio::test]
async fn test_log_after_commit() {
    let dir = init_repo().await;
    let out = write_output(dir.path(), "out.txt", "o");
    aigit::cli::commit(commit_args("agent-a", "p", &out), dir.path()).await.unwrap();
    aigit::cli::commit(commit_args("agent-b", "p", &out), dir.path()).await.unwrap();

    // No filter — expect both
    aigit::cli::log(LogArgs { agent: None, limit: 20, since: None }, dir.path())
        .await
        .unwrap();

    // Agent filter
    aigit::cli::log(LogArgs { agent: Some("agent-a".into()), limit: 20, since: None }, dir.path())
        .await
        .unwrap();
}

// ── show ──────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_show_existing_commit() {
    let dir = init_repo().await;
    let out = write_output(dir.path(), "out.txt", "my output");
    aigit::cli::commit(commit_args("agent-a", "my prompt", &out), dir.path())
        .await
        .unwrap();

    // Retrieve ID from DB to pass to show
    let db = aigit::db::Database::connect(dir.path().join(".aigit/db.sqlite"))
        .await
        .unwrap();
    let commits = db.list_commits(None, 1, None).await.unwrap();
    let id = &commits[0].id;

    aigit::cli::show(ShowArgs { id: id[..8].to_string() }, dir.path())
        .await
        .unwrap();
}

#[tokio::test]
async fn test_show_missing_commit_errors() {
    let dir = init_repo().await;
    let result = aigit::cli::show(ShowArgs { id: "deadbeef".to_string() }, dir.path()).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

// ── diff ──────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_diff_two_commits() {
    let dir = init_repo().await;
    let db = aigit::db::Database::connect(dir.path().join(".aigit/db.sqlite"))
        .await
        .unwrap();

    let id1 = db
        .insert_commit(aigit::db::NewCommit {
            git_hash: None,
            agent_id: "agent-a".into(),
            intent: None,
            prompt: "p".into(),
            model: "m".into(),
            parameters: "{}".into(),
            output: "line one\nline two\n".into(),
            artifacts: vec![],
            parent_ids: vec![],
        })
        .await
        .unwrap();

    let id2 = db
        .insert_commit(aigit::db::NewCommit {
            git_hash: None,
            agent_id: "agent-b".into(),
            intent: None,
            prompt: "p".into(),
            model: "m".into(),
            parameters: "{}".into(),
            output: "line one\nline three\n".into(),
            artifacts: vec![],
            parent_ids: vec![],
        })
        .await
        .unwrap();

    aigit::cli::diff(
        DiffArgs { commit1: id1.clone(), commit2: id2.clone(), semantic: false },
        dir.path(),
    )
    .await
    .unwrap();
}

// ── merge ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_merge_two_commits() {
    let dir = init_repo().await;
    let out_a = write_output(dir.path(), "out_a.txt", "alpha content\n");
    let out_b = write_output(dir.path(), "out_b.txt", "beta content\n");
    aigit::cli::commit(commit_args("agent-a", "p", &out_a), dir.path()).await.unwrap();
    aigit::cli::commit(commit_args("agent-b", "p", &out_b), dir.path()).await.unwrap();

    let db = aigit::db::Database::connect(dir.path().join(".aigit/db.sqlite"))
        .await
        .unwrap();
    let commits = db.list_commits(None, 2, None).await.unwrap();
    let id1 = commits[1].id.clone();
    let id2 = commits[0].id.clone();

    aigit::cli::merge(
        MergeArgs { source: id1, target: id2, llm: false, output: None, quiet: false },
        dir.path(),
    )
    .await
    .unwrap();
}

// ── agents ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_agents_list_empty() {
    let dir = init_repo().await;
    aigit::cli::agents(AgentCommands::List, dir.path()).await.unwrap();
}

#[tokio::test]
async fn test_agents_add_and_list() {
    let dir = init_repo().await;
    aigit::cli::agents(
        AgentCommands::Add {
            id: "my-agent".to_string(),
            name: "My Agent".to_string(),
            description: Some("does things".to_string()),
            config: "{}".to_string(),
        },
        dir.path(),
    )
    .await
    .unwrap();

    aigit::cli::agents(AgentCommands::List, dir.path()).await.unwrap();
}

#[tokio::test]
async fn test_agents_add_invalid_json_errors() {
    let dir = init_repo().await;
    let result = aigit::cli::agents(
        AgentCommands::Add {
            id: "bad".to_string(),
            name: "Bad".to_string(),
            description: None,
            config: "not json".to_string(),
        },
        dir.path(),
    )
    .await;
    assert!(result.is_err());
}

// ── context ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_context_no_commits() {
    let dir = init_repo().await;
    aigit::cli::context(ContextArgs { path: None, limit: 10, json: false }, dir.path())
        .await
        .unwrap();
}

#[tokio::test]
async fn test_context_json_output() {
    let dir = init_repo().await;
    let out = write_output(dir.path(), "out.txt", "output");
    aigit::cli::commit(commit_args("agent-a", "prompt", &out), dir.path()).await.unwrap();
    aigit::cli::context(ContextArgs { path: None, limit: 10, json: true }, dir.path())
        .await
        .unwrap();
}

// ── blame ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_blame_non_tracked_file() {
    let dir = init_repo().await;
    // File exists but is not in a Git repo — blame should fall back gracefully.
    let file = dir.path().join("test.rs");
    std::fs::write(&file, "fn main() {}\n").unwrap();

    aigit::cli::blame(
        BlameArgs { file: file.to_string_lossy().to_string(), lines: None },
        dir.path(),
    )
    .await
    .unwrap();
}

// ── diff --semantic graceful fallback (Fix 2) ─────────────────────────────────

#[tokio::test]
async fn test_diff_semantic_falls_back_gracefully() {
    let dir = init_repo().await;
    let db = aigit::db::Database::connect(dir.path().join(".aigit/db.sqlite"))
        .await
        .unwrap();

    let id1 = db
        .insert_commit(aigit::db::NewCommit {
            git_hash: None,
            agent_id: "agent-a".into(),
            intent: None,
            prompt: "p".into(),
            model: "m".into(),
            parameters: "{}".into(),
            output: "alpha\n".into(),
            artifacts: vec![],
            parent_ids: vec![],
        })
        .await
        .unwrap();

    let id2 = db
        .insert_commit(aigit::db::NewCommit {
            git_hash: None,
            agent_id: "agent-b".into(),
            intent: None,
            prompt: "p".into(),
            model: "m".into(),
            parameters: "{}".into(),
            output: "beta\n".into(),
            artifacts: vec![],
            parent_ids: vec![],
        })
        .await
        .unwrap();

    // --semantic should warn but not error out
    aigit::cli::diff(
        DiffArgs { commit1: id1, commit2: id2, semantic: true },
        dir.path(),
    )
    .await
    .unwrap();
}

// ── merge --output writes to file (Fix 3) ─────────────────────────────────────

#[tokio::test]
async fn test_merge_output_to_file() {
    let dir = init_repo().await;
    let out_a = write_output(dir.path(), "out_a.txt", "alpha content\n");
    let out_b = write_output(dir.path(), "out_b.txt", "beta content\n");
    aigit::cli::commit(commit_args("agent-a", "p", &out_a), dir.path()).await.unwrap();
    aigit::cli::commit(commit_args("agent-b", "p", &out_b), dir.path()).await.unwrap();

    let db = aigit::db::Database::connect(dir.path().join(".aigit/db.sqlite"))
        .await
        .unwrap();
    let commits = db.list_commits(None, 2, None).await.unwrap();
    let id1 = commits[1].id.clone();
    let id2 = commits[0].id.clone();

    let merge_out = dir.path().join("merged.txt");
    aigit::cli::merge(
        MergeArgs {
            source: id1,
            target: id2,
            llm: false,
            output: Some(merge_out.to_string_lossy().to_string()),
            quiet: false,
        },
        dir.path(),
    )
    .await
    .unwrap();

    assert!(merge_out.exists(), "merge output file should have been created");
    let contents = std::fs::read_to_string(&merge_out).unwrap();
    assert!(!contents.is_empty(), "merge output file should not be empty");
}

// ── conflicts command (Fix 5) ─────────────────────────────────────────────────

#[tokio::test]
async fn test_conflicts_no_commits() {
    let dir = init_repo().await;
    aigit::cli::conflicts(ConflictsArgs { window: 10 }, dir.path())
        .await
        .unwrap();
}

#[tokio::test]
async fn test_conflicts_single_agent_no_conflict() {
    let dir = init_repo().await;
    let out = write_output(dir.path(), "shared.rs", "fn a() {}");
    aigit::cli::commit(commit_args("agent-a", "write fn a", &out), dir.path())
        .await
        .unwrap();
    aigit::cli::commit(commit_args("agent-a", "refine fn a", &out), dir.path())
        .await
        .unwrap();

    // Only one agent touched the file — no conflict.
    aigit::cli::conflicts(ConflictsArgs { window: 10 }, dir.path())
        .await
        .unwrap();
}

#[tokio::test]
async fn test_conflicts_two_agents_detected() {
    let dir = init_repo().await;
    let shared = write_output(dir.path(), "shared.rs", "fn a() {}");

    // Two different agents commit against the same artifact path.
    aigit::cli::commit(commit_args("agent-a", "initial impl", &shared), dir.path())
        .await
        .unwrap();
    aigit::cli::commit(commit_args("agent-b", "alternative impl", &shared), dir.path())
        .await
        .unwrap();

    // Should detect a conflict on shared.rs — the function should succeed
    // and report the conflict (output goes to stdout; we just verify no error).
    aigit::cli::conflicts(ConflictsArgs { window: 10 }, dir.path())
        .await
        .unwrap();
}

// ── hook linking: commits with old parent hash are re-linked (Fix 1 db layer) ─

#[tokio::test]
async fn test_get_commits_with_git_hash_since() {
    use tempfile::tempdir;
    let tmp = tempdir().unwrap();
    let db = aigit::db::Database::connect(tmp.path().join("test.sqlite"))
        .await
        .unwrap();
    db.migrate().await.unwrap();

    let old_hash = "a".repeat(40);
    let new_hash = "b".repeat(40);

    // Insert a commit that captured the old parent hash (simulating aigit commit
    // running before git commit, so git_hash = current HEAD = old parent).
    let id = db
        .insert_commit(aigit::db::NewCommit {
            git_hash: Some(old_hash.clone()),
            agent_id: "agent-x".into(),
            intent: Some("test".into()),
            prompt: "p".into(),
            model: "m".into(),
            parameters: "{}".into(),
            output: "o".into(),
            artifacts: vec![],
            parent_ids: vec![],
        })
        .await
        .unwrap();

    let since = chrono::Utc::now().timestamp_millis() - 5000;
    let found = db
        .get_commits_with_git_hash_since(&old_hash, since)
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, id);

    // After set_git_hash, the commit should now have the new hash.
    db.set_git_hash(&id, &new_hash).await.unwrap();
    let updated = db
        .get_commits_with_git_hash_since(&old_hash, since)
        .await
        .unwrap();
    assert_eq!(updated.len(), 0, "old hash no longer matches after update");

    let by_new = db.get_commit_by_git_hash(&new_hash).await.unwrap();
    assert!(by_new.is_some());
}
