use anyhow::Result;
use clap::{Args, Subcommand};
use std::path::Path;
use crate::db;

const MAX_INPUT_BYTES: u64 = 50 * 1024 * 1024;

fn check_length(s: &str, max: usize) -> Result<String, String> {
    if s.len() > max {
        Err(format!("value exceeds maximum length of {} characters", max))
    } else {
        Ok(s.to_string())
    }
}

fn validate_agent(s: &str) -> Result<String, String> { check_length(s, 128) }
fn validate_intent(s: &str) -> Result<String, String> { check_length(s, 512) }
fn validate_model(s: &str) -> Result<String, String> { check_length(s, 256) }
fn validate_agent_id(s: &str) -> Result<String, String> { check_length(s, 128) }
fn validate_agent_name(s: &str) -> Result<String, String> { check_length(s, 128) }

fn validate_git_hash(s: &str) -> Result<String, String> {
    let len = s.len();
    if (len == 40 || len == 64) && s.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(s.to_string())
    } else {
        Err("git hash must be a 40-character SHA-1 or 64-character SHA-256 hex string".to_string())
    }
}

#[derive(Args)]
pub struct CommitArgs {
    /// Agent identifier (e.g., "claude-code-frontend")
    #[arg(long, value_parser = validate_agent)]
    pub agent: String,
    /// Human-readable intent
    #[arg(long, value_parser = validate_intent)]
    pub intent: Option<String>,
    /// Prompt text (if omitted, read from stdin)
    #[arg(long)]
    pub prompt: Option<String>,
    /// Model identifier (e.g., "claude-3.5-sonnet")
    #[arg(long, value_parser = validate_model)]
    pub model: String,
    /// JSON parameters (default: "{}")
    #[arg(long, default_value = "{}")]
    pub parameters: String,
    /// Output file (if omitted, read from stdin)
    #[arg(long)]
    pub output: Option<String>,
    /// Associate with a Git commit hash (optional)
    #[arg(long, value_parser = validate_git_hash)]
    pub git_hash: Option<String>,
}

#[derive(Args)]
pub struct LogArgs {
    /// Filter by agent ID
    #[arg(long)]
    pub agent: Option<String>,
    /// Limit number of commits
    #[arg(long, default_value = "20")]
    pub limit: u32,
    /// Show commits after this timestamp (UNIX ms)
    #[arg(long)]
    pub since: Option<i64>,
}

#[derive(Args)]
pub struct ShowArgs {
    /// Commit ID (or prefix)
    pub id: String,
}

#[derive(Args)]
pub struct DiffArgs {
    /// First commit ID (or "HEAD")
    pub commit1: String,
    /// Second commit ID (or "HEAD~1")
    pub commit2: String,
    /// Use semantic diff (embeddings)
    #[arg(long)]
    pub semantic: bool,
}

#[derive(Args)]
pub struct BlameArgs {
    /// File to blame
    pub file: String,
    /// Line range (e.g., "10-20")
    #[arg(long)]
    pub lines: Option<String>,
}

#[derive(Args)]
pub struct MergeArgs {
    /// Source commit (ID or prefix)
    pub source: String,
    /// Target commit (ID or prefix)
    pub target: String,
    /// Use LLM-assisted merge (configured via .aigit/config.toml)
    #[arg(long)]
    pub llm: bool,
    /// Write merge result to this file instead of stdout
    #[arg(long)]
    pub output: Option<String>,
    /// Suppress progress output (used internally by MCP server)
    #[arg(skip)]
    pub quiet: bool,
}

#[derive(Args)]
pub struct ContextArgs {
    /// File path to query history for (optional; shows all recent commits if omitted)
    pub path: Option<String>,
    /// Number of commits to show
    #[arg(long, default_value = "10")]
    pub limit: u32,
    /// Output as JSON (for machine consumption)
    #[arg(long)]
    pub json: bool,
}

#[derive(Subcommand)]
pub enum AgentCommands {
    /// List registered agents
    List,
    /// Add a new agent
    Add {
        /// Agent ID
        #[arg(value_parser = validate_agent_id)]
        id: String,
        /// Human-readable name
        #[arg(long, value_parser = validate_agent_name)]
        name: String,
        /// Description
        #[arg(long)]
        description: Option<String>,
        /// JSON config
        #[arg(long, default_value = "{}")]
        config: String,
    },
}

#[derive(Subcommand)]
pub enum HookCommands {
    /// Install hooks
    Install {
        /// Install into .git/hooks/ (auto-tracks Git commits)
        #[arg(long)]
        git: bool,
        /// Install Claude Code PostToolUse/PreToolUse hooks into .claude/
        #[arg(long)]
        claude: bool,
    },
    /// Uninstall hooks
    Uninstall {
        /// Remove from .git/hooks/
        #[arg(long)]
        git: bool,
        /// Remove Claude Code hooks from .claude/
        #[arg(long)]
        claude: bool,
    },
    /// Run a specific hook by name
    Run {
        /// Hook name
        name: String,
        /// Git commit hash (passed by post-commit hook)
        #[arg(long, value_parser = validate_git_hash)]
        git_hash: Option<String>,
    },
    /// List installed hooks
    List,
}

#[derive(Subcommand)]
pub enum BranchCommands {
    /// List all branches
    List,
    /// Create a new agent-specific branch
    Create {
        /// Branch name
        name: String,
        /// Agent ID this branch belongs to
        #[arg(long)]
        agent: String,
        /// Human-readable intent for this branch
        #[arg(long)]
        intent: Option<String>,
    },
    /// Delete a branch (commits are retained)
    Delete {
        /// Branch name
        name: String,
        /// Agent ID this branch belongs to
        #[arg(long)]
        agent: String,
    },
}

#[derive(clap::Args)]
pub struct StatusArgs {}

#[derive(clap::Args)]
pub struct ConflictsArgs {
    /// Only consider the last N commits per file (0 = all commits)
    #[arg(long, default_value = "10")]
    pub window: u32,
}

#[derive(clap::Args)]
pub struct ConflictCheckArgs {
    /// File path to check for multi-agent conflicts
    pub path: String,
    /// Agent ID to exclude from conflict detection (e.g. the current agent)
    #[arg(long)]
    pub agent: Option<String>,
    /// Only consider the last N commits for this file (0 = all commits)
    #[arg(long, default_value = "10")]
    pub window: u32,
}

#[derive(clap::Args)]
pub struct ResolveArgs {
    /// File path to resolve conflicts for
    pub path: String,
    /// Write resolved content to this file instead of stdout
    #[arg(long)]
    pub output: Option<String>,
    /// Use LLM-assisted merge (requires .aigit/config.toml [llm] section or ANTHROPIC_API_KEY)
    #[arg(long)]
    pub llm: bool,
}

#[derive(clap::Args)]
pub struct McpArgs {
    /// Write .mcp.json registration file to the current directory
    #[arg(long)]
    pub install: bool,
}

pub async fn init(base: &std::path::Path) -> Result<()> {
    use std::fs;

    let aigit_dir = base.join(".aigit");
    if aigit_dir.exists() {
        anyhow::bail!("aigit repository already initialized");
    }

    #[cfg(unix)]
    {
        use std::fs::DirBuilder;
        use std::os::unix::fs::DirBuilderExt;
        DirBuilder::new().mode(0o700).create(&aigit_dir)?;
    }
    #[cfg(not(unix))]
    {
        fs::create_dir_all(&aigit_dir)?;
    }
    println!("Created directory {}", aigit_dir.display());

    // Write .gitignore so .aigit contents are never accidentally committed
    fs::write(aigit_dir.join(".gitignore"), "*\n")?;

    // Create config file from example if not exists
    let config_path = aigit_dir.join("config.toml");
    if !config_path.exists() {
        let example = base.join("config.example.toml");
        if example.exists() {
            fs::copy(&example, &config_path)?;
            println!("Created config.toml");
        }
    }

    // Initialize database
    let db_path = aigit_dir.join("db.sqlite");
    let db = db::Database::connect(&db_path).await?;
    db.migrate().await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&db_path, fs::Permissions::from_mode(0o600))?;
    }
    println!("Database initialized at {}", db_path.display());

    // Create hooks directory
    let hooks_dir = aigit_dir.join("hooks");
    fs::create_dir_all(&hooks_dir)?;

    println!("aigit repository initialized successfully.");
    Ok(())
}

pub async fn commit(args: CommitArgs, base: &std::path::Path) -> Result<()> {
    use std::fs;
    use std::io::{self, BufReader, Read};

    // Validate --parameters is valid JSON before doing any I/O
    serde_json::from_str::<serde_json::Value>(&args.parameters)
        .map_err(|e| anyhow::anyhow!("--parameters is not valid JSON: {}", e))?;

    // Read prompt from stdin if not provided
    let prompt_from_stdin = args.prompt.is_none();
    let prompt = match args.prompt {
        Some(text) => text,
        None => {
            let mut buffer = String::new();
            io::stdin().take(MAX_INPUT_BYTES).read_to_string(&mut buffer)?;
            if buffer.len() as u64 == MAX_INPUT_BYTES {
                anyhow::bail!("stdin input exceeds the 50 MB limit");
            }
            buffer
        }
    };

    // Find .aigit directory before doing any file I/O
    let aigit_dir = base.join(".aigit");
    if !aigit_dir.exists() {
        anyhow::bail!("aigit repository not initialized. Run 'aigit init' first.");
    }

    let canonical_base = std::fs::canonicalize(base)
        .unwrap_or_else(|_| base.to_path_buf());

    // Capture the output file path before consuming args.output (for artifacts)
    let output_path: Option<String> = args.output.clone();

    // Read output from file or stdin (stdin only allowed when prompt came from flag)
    let output = match args.output {
        Some(ref path) => {
            let canonical = std::fs::canonicalize(path)
                .map_err(|e| anyhow::anyhow!("--output path is invalid: {}", e))?;
            if !canonical.starts_with(&canonical_base) {
                anyhow::bail!("--output path '{}' is outside the project directory", path);
            }
            let file = fs::File::open(&canonical)?;
            let metadata = file.metadata()?;
            if metadata.len() > MAX_INPUT_BYTES {
                anyhow::bail!("--output file exceeds the 50 MB limit");
            }
            let mut buffer = String::new();
            BufReader::new(file).take(MAX_INPUT_BYTES).read_to_string(&mut buffer)?;
            buffer
        }
        None => {
            if prompt_from_stdin {
                anyhow::bail!("--output is required when prompt is read from stdin");
            }
            let mut buffer = String::new();
            io::stdin().take(MAX_INPUT_BYTES).read_to_string(&mut buffer)?;
            if buffer.len() as u64 == MAX_INPUT_BYTES {
                anyhow::bail!("stdin input exceeds the 50 MB limit");
            }
            buffer
        }
    };

    let db_path = aigit_dir.join("db.sqlite");
    let db = db::Database::connect(db_path).await?;

    // Auto-detect Git hash if not provided
    let git_hash = match args.git_hash {
        Some(h) => Some(h),
        None => crate::git::get_current_hash(base).unwrap_or(None),
    };

    // Resolve parent aigit commit: try Git parent first, fall back to last aigit commit by agent.
    //
    // The fallback fires when the Git parent hash exists but has no corresponding aigit commit
    // (e.g. the repo predates aigit adoption, or the post-commit hook was never run for that
    // commit). In that case we use the most recent aigit commit by the same agent, which may
    // not be a true ancestor of the current work. This is a deliberate best-effort choice:
    // it keeps the DAG connected rather than leaving commits parentless, accepting that the
    // linkage may be approximate for pre-aigit history.
    let parent_ids = {
        let mut ids = vec![];
        let mut found = false;
        if let Ok(Some(parent_git_hash)) = crate::git::get_parent_hash(base) {
            if let Ok(Some(parent_commit)) = db.get_commit_by_git_hash(&parent_git_hash).await {
                ids.push(parent_commit.id);
                found = true;
            }
        }
        if !found {
            if let Ok(Some(last)) = db.get_latest_commit_by_agent(&args.agent).await {
                ids.push(last.id);
            }
        }
        ids
    };

    let agent_id = args.agent;
    let new_commit = db::NewCommit {
        git_hash,
        agent_id: agent_id.clone(),
        intent: args.intent,
        prompt,
        model: args.model,
        parameters: args.parameters,
        output,
        artifacts: output_path.map(|p| vec![p]).unwrap_or_default(),
        parent_ids,
    };

    let commit_id = db.insert_commit(new_commit).await?;
    println!("Committed with ID: {}", commit_id);

    // Advance HEAD on any branches registered for this agent
    if let Ok(branches) = db.list_branches_for_agent(&agent_id).await {
        for branch in branches {
            let _ = db.set_branch_head(&branch.name, &agent_id, &commit_id).await;
        }
    }

    Ok(())
}

pub async fn log(args: LogArgs, base: &std::path::Path) -> Result<()> {
    let aigit_dir = base.join(".aigit");
    if !aigit_dir.exists() {
        anyhow::bail!("aigit repository not initialized. Run 'aigit init' first.");
    }

    let db_path = aigit_dir.join("db.sqlite");
    let db = db::Database::connect(db_path).await?;

    let commits = db.list_commits(args.agent.as_deref(), args.limit, args.since).await?;

    if commits.is_empty() {
        println!("No commits found.");
        return Ok(());
    }

    for commit in commits {
        let time = chrono::DateTime::from_timestamp_millis(commit.timestamp)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| commit.timestamp.to_string());
        let intent = commit.intent.as_deref().unwrap_or("(no intent)");
        println!("{} | {} | {} | {}", &commit.id[..commit.id.len().min(12)], time, commit.agent_id, intent);
    }

    Ok(())
}

pub async fn show(args: ShowArgs, base: &std::path::Path) -> Result<()> {
    let aigit_dir = base.join(".aigit");
    if !aigit_dir.exists() {
        anyhow::bail!("aigit repository not initialized. Run 'aigit init' first.");
    }

    let db_path = aigit_dir.join("db.sqlite");
    let db = db::Database::connect(db_path).await?;

    let commit = db.get_commit_by_prefix(&args.id).await?;
    match commit {
        Some(c) => {
            let time = chrono::DateTime::from_timestamp_millis(c.timestamp)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| c.timestamp.to_string());
            println!("Commit: {}", c.id);
            println!("Time: {}", time);
            println!("Agent: {}", c.agent_id);
            if let Some(intent) = c.intent {
                println!("Intent: {}", intent);
            }
            println!("Model: {}", c.model);
            if let Some(git_hash) = c.git_hash {
                println!("Git hash: {}", git_hash);
            }
            println!("Parameters: {}", c.parameters);
            println!("\nPrompt:\n{}", c.prompt);
            println!("\nOutput:\n{}", c.output);
        }
        None => {
            anyhow::bail!("Commit not found: {}", args.id);
        }
    }

    Ok(())
}

pub async fn diff(args: DiffArgs, base: &std::path::Path) -> Result<()> {
    use similar::{ChangeTag, TextDiff};

    let aigit_dir = base.join(".aigit");
    if !aigit_dir.exists() {
        anyhow::bail!("aigit repository not initialized. Run 'aigit init' first.");
    }

    let db_path = aigit_dir.join("db.sqlite");
    let db = db::Database::connect(db_path).await?;

    let c1 = db.get_commit_by_prefix(&args.commit1).await?
        .ok_or_else(|| anyhow::anyhow!("Commit not found: {}", args.commit1))?;
    let c2 = db.get_commit_by_prefix(&args.commit2).await?
        .ok_or_else(|| anyhow::anyhow!("Commit not found: {}", args.commit2))?;

    let label1 = format!("a/{} ({})", &c1.id[..c1.id.len().min(12)], c1.agent_id);
    let label2 = format!("b/{} ({})", &c2.id[..c2.id.len().min(12)], c2.agent_id);

    println!("diff --aigit {} {}", &c1.id[..c1.id.len().min(12)], &c2.id[..c2.id.len().min(12)]);
    println!("--- {}", label1);
    println!("+++ {}", label2);

    if args.semantic {
        eprintln!("Warning: --semantic is not yet implemented (planned for Phase 4: embeddings). Falling back to textual diff.");
    }

    let text_diff = TextDiff::from_lines(&c1.output, &c2.output);
    for group in text_diff.grouped_ops(3) {
        for op in &group {
            for change in text_diff.iter_changes(op) {
                let prefix = match change.tag() {
                    ChangeTag::Delete => "-",
                    ChangeTag::Insert => "+",
                    ChangeTag::Equal  => " ",
                };
                print!("{}{}", prefix, change);
            }
        }
    }

    Ok(())
}

pub async fn blame(args: BlameArgs, base: &std::path::Path) -> Result<()> {
    let aigit_dir = base.join(".aigit");
    if !aigit_dir.exists() {
        anyhow::bail!("aigit repository not initialized. Run 'aigit init' first.");
    }
    let db_path = aigit_dir.join("db.sqlite");
    let db = db::Database::connect(db_path).await?;
    
    println!("Blame for: {}", args.file);

    // Parse optional line range filter (e.g. "10-20" or "5")
    let line_filter: Option<(u32, u32)> = args.lines.as_deref().and_then(|s| {
        if let Some((a, b)) = s.split_once('-') {
            let start = a.trim().parse::<u32>().ok()?;
            let end = b.trim().parse::<u32>().ok()?;
            Some((start, end))
        } else {
            let n = s.trim().parse::<u32>().ok()?;
            Some((n, n))
        }
    });

    // Get Git blame entries
    let blame_entries = crate::git::get_file_blame(base, Path::new(&args.file))?;
    
    if blame_entries.is_empty() {
        println!("No Git blame available for file (not tracked or not in Git repo).");
        println!("Falling back to artifact search:");
        
        // Fallback to original artifact search
        let commits = db.list_commits(None, 1000, None).await?;
        let mut found = false;

        for commit in commits {
            let artifacts_json = &commit.artifacts;
            match serde_json::from_str::<Vec<String>>(artifacts_json) {
                Ok(artifacts) => {
                    if artifacts.contains(&args.file) {
                        println!("- {}: {} ({})",
                            commit.id,
                            commit.intent.as_deref().unwrap_or("(no intent)"),
                            &commit.agent_id);
                        found = true;
                    }
                }
                Err(_) => {
                    eprintln!("Warning: commit {} has corrupted artifacts data", &commit.id[..commit.id.len().min(12)]);
                }
            }
        }
        
        if !found {
            println!("No aigit commits found for file.");
        }
    } else {
        println!("Line | Git Commit          | Aigit Commit (Agent, Intent)");
        println!("{}", "-".repeat(70));

        // Batch-fetch all aigit commits for the unique git hashes in this blame output
        let unique_hashes: Vec<String> = {
            let mut seen = std::collections::HashSet::new();
            blame_entries.iter()
                .filter(|e| seen.insert(e.commit_hash.clone()))
                .map(|e| e.commit_hash.clone())
                .collect()
        };
        let aigit_map = db.get_commits_by_git_hashes(&unique_hashes).await?;

        for entry in blame_entries {
            // Apply line range filter
            if let Some((filter_start, filter_end)) = line_filter {
                if entry.line_end < filter_start || entry.line_start > filter_end {
                    continue;
                }
            }

            let aigit_commit = aigit_map.get(&entry.commit_hash);

            let line_range = if entry.line_start == entry.line_end {
                format!("{}", entry.line_start)
            } else {
                format!("{}-{}", entry.line_start, entry.line_end)
            };
            
            let git_commit_short = &entry.commit_hash[..entry.commit_hash.len().min(12)];
            
            match aigit_commit {
                Some(commit) => {
                    println!("{:4} | {:20} | {} ({}: {})", 
                        line_range,
                        git_commit_short,
                        commit.id,
                        &commit.agent_id,
                        commit.intent.as_deref().unwrap_or("(no intent)"));
                }
                None => {
                    println!("{:4} | {:20} | (Git only)", line_range, git_commit_short);
                }
            }
        }
    }
    
    Ok(())
}

pub async fn merge(args: MergeArgs, base: &std::path::Path) -> Result<()> {
    use similar::{ChangeTag, TextDiff};

    let aigit_dir = base.join(".aigit");
    if !aigit_dir.exists() {
        anyhow::bail!("aigit repository not initialized. Run 'aigit init' first.");
    }

    let db_path = aigit_dir.join("db.sqlite");
    let db = db::Database::connect(db_path).await?;

    // Load source and target commits
    let source = db.get_commit_by_prefix(&args.source).await?
        .ok_or_else(|| anyhow::anyhow!("Source commit not found: {}", args.source))?;
    let target = db.get_commit_by_prefix(&args.target).await?
        .ok_or_else(|| anyhow::anyhow!("Target commit not found: {}", args.target))?;

    let source_label = format!("{} | {} | intent: \"{}\"",
        &source.id[..source.id.len().min(12)], source.agent_id, source.intent.as_deref().unwrap_or("no intent"));
    let target_label = format!("{} | {} | intent: \"{}\"",
        &target.id[..target.id.len().min(12)], target.agent_id, target.intent.as_deref().unwrap_or("no intent"));

    if !args.quiet {
        println!("Merging {} into {}", &source.id[..source.id.len().min(12)], &target.id[..target.id.len().min(12)]);
        println!("Source: {}", source_label);
        println!("Target: {}", target_label);
        println!();
    }

    if args.llm {
        let llm_config = crate::llm::load_llm_config(&aigit_dir);
        match llm_config {
            Ok(cfg) => {
                let prompt = format!(
                    "You are a code merge assistant. Two AI agents edited the same content with different intents.\n                     Agent A ({}): intent=\"{}\"\n                     === Agent A output ===\n                     {}\n                     === Agent B output ===\n                     Agent B ({}): intent=\"{}\"\n                     {}\n                     ===\n                     Produce a single merged version that satisfies both intents.                      Output only the merged content with no explanation, preamble, or markdown fences.",
                    source.agent_id,
                    source.intent.as_deref().unwrap_or("none"),
                    source.output,
                    target.agent_id,
                    target.intent.as_deref().unwrap_or("none"),
                    target.output,
                );
                match crate::llm::call_llm(&cfg, &prompt).await {
                    Ok(result) => {
                        if let Some(ref out_path) = args.output {
                            std::fs::write(out_path, &result).map_err(|e| {
                                anyhow::anyhow!("Failed to write merge output to '{}': {}", out_path, e)
                            })?;
                            if !args.quiet {
                                println!("LLM merge result written to: {}", out_path);
                            }
                        } else {
                            println!("{}", result);
                        }
                        return Ok(());
                    }
                    Err(e) => {
                        eprintln!("LLM merge failed: {}. Falling back to textual merge.", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("LLM config error: {}. Falling back to textual merge.", e);
            }
        }
    }

    // Simple textual merge with conflict markers
    let diff = TextDiff::from_lines(&source.output, &target.output);

    let mut merged = String::new();

    for op in diff.ops() {
        let changes = diff.iter_changes(op).collect::<Vec<_>>();
        let has_delete = changes.iter().any(|c| c.tag() == ChangeTag::Delete);
        let has_insert = changes.iter().any(|c| c.tag() == ChangeTag::Insert);

        if !has_delete && !has_insert {
            // Unchanged lines — values already include trailing newline
            for change in &changes {
                merged.push_str(change.value());
            }
        } else if has_delete && has_insert {
            // Conflict: replace op (different content in same location)
            merged.push_str(&format!("<<<<<<< {}\n", source_label));
            for change in changes.iter().filter(|c| c.tag() == ChangeTag::Delete) {
                merged.push_str(change.value());
            }
            merged.push_str("=======\n");
            for change in changes.iter().filter(|c| c.tag() == ChangeTag::Insert) {
                merged.push_str(change.value());
            }
            merged.push_str(&format!(">>>>>>> {}\n", target_label));
        } else if has_delete {
            // Lines only in source (deleted in target)
            merged.push_str(&format!("<<<<<<< {}\n", source_label));
            for change in &changes {
                merged.push_str(change.value());
            }
            merged.push_str("=======\n");
            merged.push_str(&format!(">>>>>>> {}\n", target_label));
        } else {
            // Lines only in target (inserted)
            merged.push_str(&format!("<<<<<<< {}\n", source_label));
            merged.push_str("=======\n");
            for change in &changes {
                merged.push_str(change.value());
            }
            merged.push_str(&format!(">>>>>>> {}\n", target_label));
        }
    }
    
    if let Some(ref out_path) = args.output {
        std::fs::write(out_path, &merged)
            .map_err(|e| anyhow::anyhow!("Failed to write merge output to '{}': {}", out_path, e))?;
        if !args.quiet {
            println!("Merge result written to: {}", out_path);
            println!("Note: This is a basic textual merge. Use --llm for LLM-assisted merge.");
        }
    } else if args.quiet {
        print!("{}", merged);
    } else {
        println!("Merge result (with conflict markers):\n");
        println!("{}", merged);
        println!("\nNote: This is a basic textual merge. Use --llm for LLM-assisted merge.");
    }

    Ok(())
}

pub async fn agents(sub: AgentCommands, base: &std::path::Path) -> Result<()> {
    let aigit_dir = base.join(".aigit");
    if !aigit_dir.exists() {
        anyhow::bail!("aigit repository not initialized. Run 'aigit init' first.");
    }

    let db_path = aigit_dir.join("db.sqlite");
    let db = db::Database::connect(db_path).await?;

    match sub {
        AgentCommands::List => {
            let agents = db.list_agents().await?;
            if agents.is_empty() {
                println!("No agents registered.");
            } else {
                println!("ID | Name | Description");
                println!("{}", "-".repeat(50));
                for agent in agents {
                    let desc = agent.description.as_deref().unwrap_or("(no description)");
                    println!("{} | {} | {}", agent.agent_id, agent.name, desc);
                }
            }
        }
        AgentCommands::Add { id, name, description, config } => {
            db.insert_agent(&id, &name, description.as_deref(), &config).await?;
            println!("Agent '{}' added successfully.", id);
        }
    }

    Ok(())
}

pub async fn branch(sub: BranchCommands, base: &std::path::Path) -> Result<()> {
    let aigit_dir = base.join(".aigit");
    if !aigit_dir.exists() {
        anyhow::bail!("aigit repository not initialized. Run 'aigit init' first.");
    }

    let db_path = aigit_dir.join("db.sqlite");
    let db = db::Database::connect(db_path).await?;

    match sub {
        BranchCommands::List => {
            let branches = db.list_branches().await?;
            if branches.is_empty() {
                println!("No branches found.");
            } else {
                println!("{:<20} {:<20} {:<30} Head Commit", "Name", "Agent", "Intent");
                println!("{}", "─".repeat(80));
                for b in branches {
                    let intent = b.intent.as_deref().unwrap_or("(none)");
                    let head = b.head_commit_id.as_deref()
                        .map(|id| &id[..id.len().min(12)])
                        .unwrap_or("(none)");
                    println!("{:<20} {:<20} {:<30} {}", b.name, b.agent_id, intent, head);
                }
            }
        }
        BranchCommands::Create { name, agent, intent } => {
            // Set initial HEAD to the most recent commit by this agent, if any
            let head = db.get_latest_commit_by_agent(&agent).await?
                .map(|c| c.id);
            db.insert_branch(&name, &agent, intent.as_deref(), head.as_deref()).await?;
            println!("Branch '{}' created for agent '{}'.", name, agent);
            if let Some(ref h) = head {
                println!("  HEAD: {}", &h[..h.len().min(12)]);
            }
        }
        BranchCommands::Delete { name, agent } => {
            let deleted = db.delete_branch(&name, &agent).await?;
            if deleted {
                println!("Branch '{}' (agent '{}') deleted.", name, agent);
            } else {
                anyhow::bail!("Branch '{}' for agent '{}' not found.", name, agent);
            }
        }
    }

    Ok(())
}

pub async fn status(_args: StatusArgs, base: &std::path::Path) -> Result<()> {
    let aigit_dir = base.join(".aigit");
    if !aigit_dir.exists() {
        anyhow::bail!("aigit repository not initialized. Run 'aigit init' first.");
    }

    let db_path = aigit_dir.join("db.sqlite");
    let db = db::Database::connect(db_path).await?;

    println!("aigit status");
    println!("{}", "─".repeat(60));

    // Show active branches
    let all_branches = db.list_branches().await?;
    if all_branches.is_empty() {
        println!("Branches: (none)");
    } else {
        println!("Branches:");
        for b in &all_branches {
            let head = b.head_commit_id.as_deref()
                .map(|id| &id[..id.len().min(12)])
                .unwrap_or("(none)");
            println!("  {} [{}]  HEAD: {}", b.name, b.agent_id, head);
        }
    }
    println!();

    // Get modified files from Git
    let modified_files = crate::git::get_modified_files(base)?;

    if modified_files.is_empty() {
        println!("No modified files in Git working tree.");
        return Ok(());
    }

    // Check aigit coverage for each modified file
    let mut covered: Vec<(String, db::Commit)> = vec![];
    let mut uncovered: Vec<String> = vec![];

    for file in &modified_files {
        match db.get_latest_commit_for_artifact(file).await? {
            Some(commit) => covered.push((file.clone(), commit)),
            None => uncovered.push(file.clone()),
        }
    }

    if !covered.is_empty() {
        println!("Files with aigit coverage:");
        for (file, commit) in &covered {
            let time = chrono::DateTime::from_timestamp_millis(commit.timestamp)
                .map(|dt| dt.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| commit.timestamp.to_string());
            let intent = commit.intent.as_deref().unwrap_or("(no intent)");
            println!("  {:<35} last: {}  agent: {}  intent: \"{}\"",
                file, time, commit.agent_id, intent);
        }
        println!();
    }

    if !uncovered.is_empty() {
        println!("Files modified in Git with no aigit coverage:");
        for file in &uncovered {
            println!("  {}  (not tracked by any agent)", file);
        }
        println!();
    }

    Ok(())
}

pub async fn context(args: ContextArgs, base: &std::path::Path) -> Result<()> {
    let aigit_dir = base.join(".aigit");
    if !aigit_dir.exists() {
        anyhow::bail!("aigit repository not initialized. Run 'aigit init' first.");
    }

    let db_path = aigit_dir.join("db.sqlite");
    let db = db::Database::connect(db_path).await?;

    // Collect aigit commits relevant to the given path (or all recent commits).
    let commits: Vec<db::Commit> = if let Some(ref file_path) = args.path {
        let git_hashes = crate::git::get_commits_for_file(base, Path::new(file_path))?;

        // Batch-fetch aigit commits for all git hashes, reassemble in order
        let hash_map = db.get_commits_by_git_hashes(&git_hashes).await?;
        let mut matched: Vec<db::Commit> = vec![];
        for hash in &git_hashes {
            if let Some(c) = hash_map.get(hash) {
                matched.push(c.clone());
                if matched.len() >= args.limit as usize {
                    break;
                }
            }
        }

        // Fallback: artifact search using indexed commit_artifacts table
        if matched.is_empty() {
            let mut all = db.get_commits_for_artifact(file_path).await?;
            all.truncate(args.limit as usize);
            matched = all;
        }

        matched
    } else {
        db.list_commits(None, args.limit, None).await?
    };

    if commits.is_empty() {
        if let Some(ref p) = args.path {
            println!("No aigit commits found for: {}", p);
        } else {
            println!("No aigit commits found.");
        }
        return Ok(());
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&commits)?);
        return Ok(());
    }

    if let Some(ref p) = args.path {
        println!("Context for: {}", p);
    } else {
        println!("Recent aigit context ({} commits):", commits.len());
    }
    println!("{}", "─".repeat(70));

    for (i, commit) in commits.iter().enumerate() {
        let time = chrono::DateTime::from_timestamp_millis(commit.timestamp)
            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| commit.timestamp.to_string());

        let intent = commit.intent.as_deref().unwrap_or("(no intent)");
        let git_ref = commit.git_hash.as_deref().unwrap_or("(no git hash)");

        // Truncate prompt to first 120 chars for readability
        let prompt_snippet = {
            let p = commit.prompt.trim();
            if p.len() > 120 {
                format!("{}…", &p[..120])
            } else {
                p.to_string()
            }
        };

        println!("[{}] {} | {} | intent: \"{}\"", i + 1, time, commit.agent_id, intent);
        println!("    git: {} | aigit: {}", git_ref, &commit.id[..commit.id.len().min(12)]);
        println!("    prompt: {}", prompt_snippet);
        println!();
    }

    Ok(())
}

pub async fn conflicts(args: ConflictsArgs, base: &std::path::Path) -> Result<()> {
    use std::collections::HashMap;

    let aigit_dir = base.join(".aigit");
    if !aigit_dir.exists() {
        anyhow::bail!("aigit repository not initialized. Run 'aigit init' first.");
    }

    let db_path = aigit_dir.join("db.sqlite");
    let db = db::Database::connect(db_path).await?;

    // Fetch (artifact_path, agent_id, intent) rows ordered newest-first.
    // Uses the normalized commit_artifacts table — does not load prompt/output.
    let rows = db.get_artifact_commit_rows().await?;

    // For each artifact path, track which agents have touched it and their most
    // recent intent.  We respect the --window limit: once we have `window`
    // commits for a given path (across all agents) we stop counting that path.
    // window == 0 means unlimited.
    let window = args.window as usize;

    // artifact_path -> HashMap<agent_id, most_recent_intent>
    let mut artifact_agents: HashMap<String, HashMap<String, Option<String>>> = HashMap::new();
    // artifact_path -> total commit count so far (for window enforcement)
    let mut artifact_count: HashMap<String, usize> = HashMap::new();

    for row in &rows {
        let count = artifact_count.entry(row.artifact_path.clone()).or_insert(0);
        if window > 0 && *count >= window {
            continue;
        }
        *count += 1;

        let agents = artifact_agents.entry(row.artifact_path.clone()).or_default();
        // Only record the first (most recent) intent per agent since rows are newest-first.
        agents.entry(row.agent_id.clone()).or_insert_with(|| row.intent.clone());
    }

    // Filter to files where more than one distinct agent has commits.
    let mut conflicts: Vec<(String, HashMap<String, Option<String>>)> = artifact_agents
        .into_iter()
        .filter(|(_, agents)| agents.len() > 1)
        .collect();

    // Sort by artifact path for stable output.
    conflicts.sort_by(|a, b| a.0.cmp(&b.0));

    if conflicts.is_empty() {
        println!("No multi-agent conflicts detected.");
        return Ok(());
    }

    println!("Multi-agent conflicts ({} file(s)):", conflicts.len());
    println!("{}", "─".repeat(70));

    for (artifact, agents) in &conflicts {
        println!("  {}", artifact);
        let mut agent_list: Vec<(&String, &Option<String>)> = agents.iter().collect();
        agent_list.sort_by_key(|(id, _)| id.as_str());
        for (agent_id, intent) in agent_list {
            let intent_str = intent.as_deref().unwrap_or("(no intent)");
            println!("    agent: {}  last intent: \"{}\"", agent_id, intent_str);
        }
    }

    Ok(())
}

pub async fn hook(sub: HookCommands, base: &std::path::Path) -> Result<()> {
    use std::fs;

    match sub {
        HookCommands::Install { git, claude } => {
            if claude {
                hook_install_claude(base)?;
            } else if git {
                // Install into .git/hooks/
                let repo_root = match crate::git::get_repo_root(base)? {
                    Some(root) => root,
                    None => anyhow::bail!("Not in a Git repository. Run 'git init' first."),
                };
                let git_hooks_dir = repo_root.join(".git").join("hooks");
                if !git_hooks_dir.exists() {
                    fs::create_dir_all(&git_hooks_dir)?;
                }
                
                // Create post-commit hook that calls 'aigit hook run post-commit'
                let hook_content = r#"#!/bin/bash
set -e
GIT_HASH=$(git rev-parse HEAD)
# Find aigit binary: prefer PATH, fall back to cargo run in repo root
if command -v aigit &>/dev/null; then
    aigit hook run post-commit --git-hash "$GIT_HASH"
else
    REPO_ROOT=$(git rev-parse --show-toplevel)
    cargo run --manifest-path "$REPO_ROOT/Cargo.toml" --quiet -- hook run post-commit --git-hash "$GIT_HASH"
fi
"#;
                
                let hook_path = git_hooks_dir.join("post-commit");
                fs::write(&hook_path, hook_content)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755))?;
                }
                
                println!("Installed post-commit hook at: {}", hook_path.display());
                println!("Run 'aigit hook install --git' in each repo to enable auto-tracking.");
            } else {
                // Install example hook into .aigit/hooks/
                let hooks_dir = base.join(".aigit").join("hooks");
                if !hooks_dir.exists() {
                    fs::create_dir_all(&hooks_dir)?;
                }
                
                let hook_content = r#"#!/bin/bash
# aigit auto‑tracking hook
# This hook runs aigit commit automatically when Git commits happen
echo "aigit hook: tracking Git commit"
# Add your aigit commit logic here
"#;
                
                let hook_path = hooks_dir.join("pre-commit");
                fs::write(&hook_path, hook_content)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755))?;
                }
                
                println!("Created example hook at: {}", hook_path.display());
                println!("Note: Full hook automation is Phase 1. Manual integration required.");
            }
        }
        HookCommands::Uninstall { git, claude } => {
            if claude {
                hook_uninstall_claude(base)?;
            } else if git {
                // Remove from .git/hooks/
                let repo_root = match crate::git::get_repo_root(base)? {
                    Some(root) => root,
                    None => anyhow::bail!("Not in a Git repository."),
                };
                let hook_path = repo_root.join(".git").join("hooks").join("post-commit");
                if hook_path.exists() {
                    fs::remove_file(&hook_path)?;
                    println!("Removed Git hook: {}", hook_path.display());
                } else {
                    println!("No Git hook found at: {}", hook_path.display());
                }
            } else {
                // Remove from .aigit/hooks/
                let hooks_dir = base.join(".aigit").join("hooks");
                if hooks_dir.exists() {
                    for entry in fs::read_dir(&hooks_dir)? {
                        let entry = entry?;
                        let path = entry.path();
                        if path.is_file() {
                            fs::remove_file(&path)?;
                            println!("Removed hook: {}", path.display());
                        }
                    }
                    fs::remove_dir(&hooks_dir)?;
                    println!("Removed hooks directory: {}", hooks_dir.display());
                } else {
                    println!("No hooks directory found.");
                }
            }
        }
        HookCommands::Run { name, git_hash } => {
            match name.as_str() {
                "post-commit" => {
                    let git_hash = git_hash.ok_or_else(|| {
                        anyhow::anyhow!("post-commit hook requires --git-hash <hash>")
                    })?;

                    let aigit_dir = base.join(".aigit");
                    if !aigit_dir.exists() {
                        return Ok(());
                    }
                    let db = db::Database::connect(aigit_dir.join("db.sqlite")).await?;

                    // Window: all aigit commits since the previous Git commit timestamp.
                    // get_parent_timestamp() reads HEAD~1 of the current HEAD (which is the
                    // just-made git commit), so it returns the timestamp of the commit that was
                    // HEAD *before* the git commit ran — exactly bounding our search window.
                    let since_ms = crate::git::get_parent_timestamp(base)?
                        .unwrap_or(0) * 1000;

                    // Case 1: aigit commits stored with git_hash IS NULL (prompt was captured
                    //         before any git HEAD existed, or aigit commit had no git context).
                    let mut unlinked = db.get_unlinked_commits_since(since_ms).await?;

                    // Case 2: aigit commits stored with git_hash = old parent hash.
                    //         This happens when `aigit commit` runs before `git commit`: it
                    //         captures get_current_hash() which is the *pre-commit* HEAD, not
                    //         the new commit hash.  We must update those too.
                    let old_parent_hash = crate::git::get_parent_hash(base)?;
                    if let Some(ref old_hash) = old_parent_hash {
                        let pre_linked = db
                            .get_commits_with_git_hash_since(old_hash, since_ms)
                            .await?;
                        // Avoid double-counting commits that happen to also be NULL (shouldn't
                        // occur, but guard anyway).
                        for c in pre_linked {
                            if !unlinked.iter().any(|u| u.id == c.id) {
                                unlinked.push(c);
                            }
                        }
                    }

                    if !unlinked.is_empty() {
                        // Retrospective linking: associate existing aigit commits with this git hash
                        for commit in &unlinked {
                            db.set_git_hash(&commit.id, &git_hash).await?;
                        }
                        println!("aigit: linked {} commit(s) to Git hash {}",
                            unlinked.len(), &git_hash[..git_hash.len().min(12)]);
                    } else {
                        // Fallback: record the git commit message as a new aigit commit
                        let msg = crate::git::get_head_commit_message(base)?.unwrap_or_default();
                        let new_commit = db::NewCommit {
                            git_hash: Some(git_hash.clone()),
                            agent_id: "git-hook".to_string(),
                            intent: Some("git commit".to_string()),
                            prompt: msg,
                            model: "unknown".to_string(),
                            parameters: "{}".to_string(),
                            output: String::new(),
                            artifacts: vec![],
                            parent_ids: vec![],
                        };
                        let id = db.insert_commit(new_commit).await?;
                        println!("aigit: recorded Git commit {} as aigit commit {}",
                            &git_hash[..git_hash.len().min(12)], &id[..id.len().min(12)]);
                    }
                }
                other => {
                    anyhow::bail!("Unknown hook: \'{}\'. Supported hooks: post-commit", other);
                }
            }
        }
        HookCommands::List => {
            let mut found_any = false;

            // Check .aigit/hooks/ for internally-managed hook files
            let hooks_dir = base.join(".aigit").join("hooks");
            if hooks_dir.exists() {
                let mut count = 0usize;
                for entry in fs::read_dir(&hooks_dir)? {
                    let entry = entry?;
                    println!("  [aigit] {}", entry.file_name().to_string_lossy());
                    count += 1;
                    found_any = true;
                }
                if count > 0 {
                    println!("  (source: {})", hooks_dir.display());
                }
            }

            // Check .git/hooks/post-commit for the aigit-installed Git hook.
            // The hook script written by `hook install --git` contains the signature
            // string "hook run post-commit" which is unique to aigit.
            const AIGIT_HOOK_SIGNATURE: &str = "hook run post-commit";
            if let Some(repo_root) = crate::git::get_repo_root(base)? {
                let git_hook_path = repo_root.join(".git").join("hooks").join("post-commit");
                if git_hook_path.exists() {
                    let contents = fs::read_to_string(&git_hook_path).unwrap_or_default();
                    if contents.contains(AIGIT_HOOK_SIGNATURE) {
                        println!("  [git]   post-commit  (aigit-managed)");
                        println!("  (source: {})", git_hook_path.display());
                        found_any = true;
                    }
                }
            }

            // Check .claude/settings.json for aigit-managed Claude Code hooks.
            const CLAUDE_HOOK_SIGNATURE: &str = "aigit-post-tool";
            let claude_settings_path = base.join(".claude").join("settings.json");
            if claude_settings_path.exists() {
                let contents = fs::read_to_string(&claude_settings_path).unwrap_or_default();
                if contents.contains(CLAUDE_HOOK_SIGNATURE) {
                    println!("  [claude] PostToolUse/PreToolUse  (aigit-managed)");
                    println!("  (source: {})", claude_settings_path.display());
                    found_any = true;
                }
            }

            if !found_any {
                println!("No aigit hooks installed.");
                println!("Run 'aigit hook install --git' to install the Git post-commit hook.");
                println!("Run 'aigit hook install --claude' to install Claude Code hooks.");
            }
        }
    }

    Ok(())
}

fn hook_install_claude(base: &std::path::Path) -> Result<()> {
    use std::fs;

    let claude_dir = base.join(".claude");
    let hooks_dir = claude_dir.join("hooks");
    fs::create_dir_all(&hooks_dir)?;

    // PostToolUse hook: auto-commits after Write/Edit tool calls
    let post_tool_content = r#"#!/bin/bash
# aigit PostToolUse hook — auto-commits AI-generated file writes to aigit
INPUT=$(cat)
TOOL=$(echo "$INPUT" | jq -r '.tool_name // empty')
[[ "$TOOL" == "Write" || "$TOOL" == "Edit" ]] || exit 0
FILE=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty')
[[ -n "$FILE" ]] || exit 0
AGENT="${AIGIT_AGENT:-claude-code}"
MODEL="${AIGIT_MODEL:-claude-sonnet-4-6}"
INTENT="${AIGIT_INTENT:-}"
# Extract a prompt from the tool event. For Write use file_text; for Edit use new_string.
# Truncate to 4096 chars to stay within aigit's limits.
PROMPT=$(echo "$INPUT" | jq -r '
  if .tool_name == "Write" then .tool_input.file_text // ""
  elif .tool_name == "Edit" then .tool_input.new_string // ""
  else "" end' | head -c 4096)
[[ -n "$PROMPT" ]] || PROMPT="Claude Code ${TOOL}: ${FILE}"
aigit_commit() {
    if command -v aigit &>/dev/null; then
        aigit "$@"
    else
        REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null)
        cargo run --manifest-path "$REPO_ROOT/Cargo.toml" --quiet -- "$@"
    fi
}
if [[ -n "$INTENT" ]]; then
    aigit_commit commit --agent "$AGENT" --model "$MODEL" --intent "$INTENT" --prompt "$PROMPT" --output "$FILE" 2>/dev/null || true
else
    aigit_commit commit --agent "$AGENT" --model "$MODEL" --prompt "$PROMPT" --output "$FILE" 2>/dev/null || true
fi
exit 0
"#;

    // PreToolUse hook: warns when another agent recently touched the target file
    let pre_tool_content = r#"#!/bin/bash
# aigit PreToolUse hook — warns when another agent recently touched the target file
INPUT=$(cat)
TOOL=$(echo "$INPUT" | jq -r '.tool_name // empty')
[[ "$TOOL" == "Write" || "$TOOL" == "Edit" ]] || exit 0
FILE=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty')
[[ -n "$FILE" ]] || exit 0
AGENT="${AIGIT_AGENT:-claude-code}"
aigit_run() {
    if command -v aigit &>/dev/null; then
        aigit "$@"
    else
        REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null)
        cargo run --manifest-path "$REPO_ROOT/Cargo.toml" --quiet -- "$@"
    fi
}
WARNING=$(aigit_run conflict-check "$FILE" --agent "$AGENT" 2>&1)
if [[ $? -ne 0 && -n "$WARNING" ]]; then
    echo "aigit conflict warning: $WARNING" >&2
fi
exit 0
"#;

    let post_tool_path = hooks_dir.join("aigit-post-tool.sh");
    let pre_tool_path = hooks_dir.join("aigit-pre-tool.sh");
    fs::write(&post_tool_path, post_tool_content)?;
    fs::write(&pre_tool_path, pre_tool_content)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&post_tool_path, fs::Permissions::from_mode(0o755))?;
        fs::set_permissions(&pre_tool_path, fs::Permissions::from_mode(0o755))?;
    }
    println!("Created: {}", post_tool_path.display());
    println!("Created: {}", pre_tool_path.display());

    // Patch .claude/settings.json to register the hooks
    let settings_path = claude_dir.join("settings.json");
    let mut settings: serde_json::Value = if settings_path.exists() {
        let contents = fs::read_to_string(&settings_path)?;
        serde_json::from_str(&contents).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    let hooks_obj = settings
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("settings.json root is not an object"))?
        .entry("hooks")
        .or_insert(serde_json::json!({}))
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("hooks is not an object"))?;

    let post_hook_entry = serde_json::json!({
        "matcher": "Write|Edit",
        "hooks": [{
            "type": "command",
            "command": ".claude/hooks/aigit-post-tool.sh"
        }]
    });
    let pre_hook_entry = serde_json::json!({
        "matcher": "Write|Edit",
        "hooks": [{
            "type": "command",
            "command": ".claude/hooks/aigit-pre-tool.sh"
        }]
    });

    // Append to existing arrays or create new ones, avoiding duplicates
    let post_arr = hooks_obj
        .entry("PostToolUse")
        .or_insert(serde_json::json!([]))
        .as_array_mut()
        .ok_or_else(|| anyhow::anyhow!("PostToolUse is not an array"))?;
    if !post_arr.iter().any(|e| e.get("hooks").and_then(|h| h.as_array()).map(|h| h.iter().any(|x| x.get("command").and_then(|c| c.as_str()).map(|c| c.contains("aigit-post-tool")).unwrap_or(false))).unwrap_or(false)) {
        post_arr.push(post_hook_entry);
    }

    let pre_arr = hooks_obj
        .entry("PreToolUse")
        .or_insert(serde_json::json!([]))
        .as_array_mut()
        .ok_or_else(|| anyhow::anyhow!("PreToolUse is not an array"))?;
    if !pre_arr.iter().any(|e| e.get("hooks").and_then(|h| h.as_array()).map(|h| h.iter().any(|x| x.get("command").and_then(|c| c.as_str()).map(|c| c.contains("aigit-pre-tool")).unwrap_or(false))).unwrap_or(false)) {
        pre_arr.push(pre_hook_entry);
    }

    let settings_json = serde_json::to_string_pretty(&settings)?;
    fs::write(&settings_path, settings_json)?;
    println!("Updated: {}", settings_path.display());
    println!("Claude Code hooks installed. Set AIGIT_AGENT, AIGIT_MODEL, AIGIT_INTENT env vars to customize.");
    Ok(())
}

fn hook_uninstall_claude(base: &std::path::Path) -> Result<()> {
    use std::fs;

    let claude_dir = base.join(".claude");
    let hooks_dir = claude_dir.join("hooks");

    for name in &["aigit-post-tool.sh", "aigit-pre-tool.sh"] {
        let path = hooks_dir.join(name);
        if path.exists() {
            fs::remove_file(&path)?;
            println!("Removed: {}", path.display());
        }
    }

    // Prune aigit entries from settings.json
    let settings_path = claude_dir.join("settings.json");
    if settings_path.exists() {
        let contents = fs::read_to_string(&settings_path)?;
        if let Ok(mut settings) = serde_json::from_str::<serde_json::Value>(&contents) {
            if let Some(hooks) = settings.get_mut("hooks").and_then(|h| h.as_object_mut()) {
                for event in &["PostToolUse", "PreToolUse"] {
                    if let Some(arr) = hooks.get_mut(*event).and_then(|a| a.as_array_mut()) {
                        arr.retain(|e| {
                            !e.get("hooks")
                                .and_then(|h| h.as_array())
                                .map(|h| h.iter().any(|x| x.get("command").and_then(|c| c.as_str()).map(|c| c.contains("aigit-")).unwrap_or(false)))
                                .unwrap_or(false)
                        });
                    }
                }
            }
            fs::write(&settings_path, serde_json::to_string_pretty(&settings)?)?;
            println!("Updated: {}", settings_path.display());
        }
    }
    Ok(())
}

pub async fn conflict_check(args: ConflictCheckArgs, base: &std::path::Path) -> Result<()> {
    use std::collections::HashMap;

    let aigit_dir = base.join(".aigit");
    if !aigit_dir.exists() {
        // No aigit repo — silently exit clean (hook should not block)
        return Ok(());
    }

    let db_path = aigit_dir.join("db.sqlite");
    let db = db::Database::connect(db_path).await?;

    let rows = db.get_artifact_commit_rows().await?;
    let window = args.window as usize;

    // Collect agents that have touched this specific file within the window
    let mut agents: HashMap<String, Option<String>> = HashMap::new();
    let mut count = 0usize;

    for row in &rows {
        if row.artifact_path != args.path {
            continue;
        }
        if window > 0 && count >= window {
            break;
        }
        count += 1;
        agents.entry(row.agent_id.clone()).or_insert_with(|| row.intent.clone());
    }

    // If an agent filter is provided, remove the current agent so we only warn
    // about *other* agents having touched this file.
    if let Some(ref self_agent) = args.agent {
        agents.remove(self_agent);
    }

    if !agents.is_empty() {
        let agent_list: Vec<String> = agents
            .iter()
            .map(|(id, intent)| {
                format!(
                    "{} (intent: \"{}\")",
                    id,
                    intent.as_deref().unwrap_or("none")
                )
            })
            .collect();
        anyhow::bail!(
            "conflict: {} was recently modified by: {}",
            args.path,
            agent_list.join(", ")
        );
    }

    Ok(())
}

pub async fn resolve(args: ResolveArgs, base: &std::path::Path) -> Result<()> {
    use std::collections::HashMap;

    let aigit_dir = base.join(".aigit");
    if !aigit_dir.exists() {
        anyhow::bail!("aigit repository not initialized. Run 'aigit init' first.");
    }

    let db_path = aigit_dir.join("db.sqlite");
    let db = db::Database::connect(db_path).await?;

    // Find the two most recent commits from distinct agents for this file
    let rows = db.get_artifact_commit_rows().await?;
    let mut agent_commits: HashMap<String, String> = HashMap::new(); // agent_id -> commit_id

    for row in &rows {
        if row.artifact_path != args.path {
            continue;
        }
        agent_commits.entry(row.agent_id.clone()).or_insert(row.commit_id.clone());
        if agent_commits.len() >= 2 {
            break;
        }
    }

    if agent_commits.len() < 2 {
        anyhow::bail!(
            "No multi-agent conflict found for '{}'. Need commits from at least 2 different agents.",
            args.path
        );
    }

    // Sort by agent_id for deterministic source/target assignment
    let mut agent_list: Vec<(String, String)> = agent_commits.into_iter().collect();
    agent_list.sort_by(|a, b| a.0.cmp(&b.0));
    merge(MergeArgs {
        source: agent_list[0].1.clone(),
        target: agent_list[1].1.clone(),
        llm: args.llm,
        output: args.output,
        quiet: false,
    }, base).await
}