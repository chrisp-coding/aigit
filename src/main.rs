use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::Path;

mod cli;
mod db;
mod git;
mod llm;
mod mcp;

#[derive(Parser)]
#[command(name = "aigit", about = "AI-native version control", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize aigit repository
    Init,
    /// Commit AI-generated content
    Commit(cli::CommitArgs),
    /// Show commit history
    Log(cli::LogArgs),
    /// Show commit details
    Show(cli::ShowArgs),
    /// Diff two commits
    Diff(cli::DiffArgs),
    /// Merge two commits
    Merge(cli::MergeArgs),
    /// Blame lines to agents/prompts
    Blame(cli::BlameArgs),
    /// Manage agents
    #[command(subcommand)]
    Agents(cli::AgentCommands),
    /// Manage Git hooks
    #[command(subcommand)]
    Hook(cli::HookCommands),
    /// Show project context for a file or the whole repo (for AI agents)
    Context(cli::ContextArgs),
    /// Manage agent-specific branches
    #[command(subcommand)]
    Branch(cli::BranchCommands),
    /// Show working tree status with aigit coverage
    Status(cli::StatusArgs),
    /// Show files where multiple agents have recent commits (potential conflicts)
    Conflicts(cli::ConflictsArgs),
    /// Check a single file for multi-agent conflicts (exits 1 if conflict found)
    ConflictCheck(cli::ConflictCheckArgs),
    /// Resolve conflicts for a file using LLM merge
    Resolve(cli::ResolveArgs),
    /// Intercept a Claude Code Write/Edit event, auto-merge with existing content via LLM
    MergeContent(cli::MergeContentArgs),
    /// Start the aigit MCP server (stdio JSON-RPC 2.0)
    Mcp(cli::McpArgs),
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let base = Path::new(".");
    let cli = Cli::parse();
    match cli.command {
        Commands::Init => cli::init(base).await,
        Commands::Commit(args) => cli::commit(args, base).await,
        Commands::Log(args) => cli::log(args, base).await,
        Commands::Show(args) => cli::show(args, base).await,
        Commands::Diff(args) => cli::diff(args, base).await,
        Commands::Merge(args) => cli::merge(args, base).await,
        Commands::Blame(args) => cli::blame(args, base).await,
        Commands::Agents(sub) => cli::agents(sub, base).await,
        Commands::Hook(sub) => cli::hook(sub, base).await,
        Commands::Context(args) => cli::context(args, base).await,
        Commands::Branch(sub) => cli::branch(sub, base).await,
        Commands::Status(args) => cli::status(args, base).await,
        Commands::Conflicts(args) => cli::conflicts(args, base).await,
        Commands::ConflictCheck(args) => cli::conflict_check(args, base).await,
        Commands::Resolve(args) => cli::resolve(args, base).await,
        Commands::MergeContent(args) => cli::merge_content(args, base).await,
        Commands::Mcp(args) => crate::mcp::run(args, base).await,
    }
}
