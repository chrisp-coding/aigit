use anyhow::Result;
use clap::{Args, Subcommand};
use serde_json;
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
    /// Use LLM-assisted merge (Phase 3)
    #[arg(long)]
    pub llm: bool,
    /// Write merge result to this file instead of stdout
    #[arg(long)]
    pub output: Option<String>,
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
    /// Install Git hooks
    Install {
        /// Install into .git/hooks/ (auto-tracks Git commits)
        #[arg(long)]
        git: bool,
    },
    /// Uninstall Git hooks
    Uninstall {
        /// Remove from .git/hooks/
        #[arg(long)]
        git: bool,
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
        None => crate::git::get_current_hash().unwrap_or(None),
    };

    // Resolve parent aigit commit: try Git parent first, fall back to last aigit commit by agent
    let parent_ids = {
        let mut ids = vec![];
        let mut found = false;
        if let Ok(Some(parent_git_hash)) = crate::git::get_parent_hash() {
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
    let blame_entries = crate::git::get_file_blame(Path::new(&args.file))?;
    
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
        
        for entry in blame_entries {
            // Apply line range filter
            if let Some((filter_start, filter_end)) = line_filter {
                if entry.line_end < filter_start || entry.line_start > filter_end {
                    continue;
                }
            }

            // Try to find aigit commit with matching git_hash
            let aigit_commit = db.get_commit_by_git_hash(&entry.commit_hash).await?;

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

    println!("Merging {} into {}", &source.id[..source.id.len().min(12)], &target.id[..target.id.len().min(12)]);
    println!("Source: {}", source_label);
    println!("Target: {}", target_label);
    println!();

    if args.llm {
        println!("LLM‑assisted merge is Phase 3 (not yet implemented).");
        println!("Falling back to textual merge.");
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
        println!("Merge result written to: {}", out_path);
        println!("Note: This is a basic textual merge. Use --llm for LLM‑assisted merge (Phase 3).");
    } else {
        println!("Merge result (with conflict markers):\n");
        println!("{}", merged);
        println!("\nNote: This is a basic textual merge. Use --llm for LLM‑assisted merge (Phase 3).");
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
                println!("{:<20} {:<20} {:<30} {}", "Name", "Agent", "Intent", "Head Commit");
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
    let modified_files = crate::git::get_modified_files()?;

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
        let git_hashes = crate::git::get_commits_for_file(Path::new(file_path))?;

        let mut matched = vec![];
        // Look up aigit commits by git hash
        for hash in &git_hashes {
            if let Some(c) = db.get_commit_by_git_hash(hash).await? {
                matched.push(c);
                if matched.len() >= args.limit as usize {
                    break;
                }
            }
        }

        // Fallback: artifact search if no git-hash matches found
        if matched.is_empty() {
            let all = db.list_commits(None, args.limit, None).await?;
            for commit in all {
                if matched.len() >= args.limit as usize {
                    break;
                }
                match serde_json::from_str::<Vec<String>>(&commit.artifacts) {
                    Ok(artifacts) => {
                        if artifacts.iter().any(|a| a.contains(file_path.as_str())) {
                            matched.push(commit);
                        }
                    }
                    Err(_) => {
                        eprintln!("Warning: commit {} has corrupted artifacts data", &commit.id[..commit.id.len().min(12)]);
                    }
                }
            }
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

    // Fetch all commits, ordered newest-first, to scan artifact paths.
    let all_commits = db.list_commits(None, u32::MAX, None).await?;

    // For each artifact path, track which agents have touched it and their most
    // recent intent.  We respect the --window limit: once we have `window`
    // commits for a given path (across all agents) we stop counting that path.
    // window == 0 means unlimited.
    let window = args.window as usize;

    // artifact_path -> HashMap<agent_id, most_recent_intent>
    let mut artifact_agents: HashMap<String, HashMap<String, Option<String>>> = HashMap::new();
    // artifact_path -> total commit count so far (for window enforcement)
    let mut artifact_count: HashMap<String, usize> = HashMap::new();

    for commit in &all_commits {
        let artifacts: Vec<String> = match serde_json::from_str(&commit.artifacts) {
            Ok(v) => v,
            Err(_) => {
                eprintln!(
                    "Warning: commit {} has corrupted artifacts data",
                    &commit.id[..commit.id.len().min(12)]
                );
                continue;
            }
        };

        for artifact in artifacts {
            if artifact.is_empty() {
                continue;
            }
            let count = artifact_count.entry(artifact.clone()).or_insert(0);
            if window > 0 && *count >= window {
                continue;
            }
            *count += 1;

            let agents = artifact_agents.entry(artifact.clone()).or_default();
            // Only record the first (most recent) intent per agent since commits are newest-first.
            agents.entry(commit.agent_id.clone()).or_insert_with(|| commit.intent.clone());
        }
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
        HookCommands::Install { git } => {
            if git {
                // Install into .git/hooks/
                let repo_root = match crate::git::get_repo_root()? {
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
    AIGIT=aigit
else
    REPO_ROOT=$(git rev-parse --show-toplevel)
    AIGIT="cargo run --manifest-path \"$REPO_ROOT/Cargo.toml\" --quiet --"
fi
$AIGIT hook run post-commit --git-hash "$GIT_HASH"
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
        HookCommands::Uninstall { git } => {
            if git {
                // Remove from .git/hooks/
                let repo_root = match crate::git::get_repo_root()? {
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
                    let since_ms = crate::git::get_parent_timestamp()?
                        .unwrap_or(0) * 1000;

                    // Case 1: aigit commits stored with git_hash IS NULL (prompt was captured
                    //         before any git HEAD existed, or aigit commit had no git context).
                    let mut unlinked = db.get_unlinked_commits_since(since_ms).await?;

                    // Case 2: aigit commits stored with git_hash = old parent hash.
                    //         This happens when `aigit commit` runs before `git commit`: it
                    //         captures get_current_hash() which is the *pre-commit* HEAD, not
                    //         the new commit hash.  We must update those too.
                    let old_parent_hash = crate::git::get_parent_hash()?;
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
                        let msg = crate::git::get_head_commit_message()?.unwrap_or_default();
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
            if let Some(repo_root) = crate::git::get_repo_root()? {
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

            if !found_any {
                println!("No aigit hooks installed.");
                println!("Run 'aigit hook install --git' to install the Git post-commit hook.");
            }
        }
    }
    
    Ok(())
}