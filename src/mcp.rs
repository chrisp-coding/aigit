use anyhow::Result;
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::Path;

use crate::cli::McpArgs;
use crate::db;

const SERVER_NAME: &str = "aigit";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Entry point: start the stdio JSON-RPC 2.0 MCP server.
pub async fn run(args: McpArgs, base: &Path) -> Result<()> {
    if args.install {
        return install_mcp_json(base);
    }

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());

    const MAX_LINE_BYTES: usize = 10 * 1024 * 1024; // 10 MB per JSON-RPC message

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) if l.trim().is_empty() => continue,
            Ok(l) => l,
            Err(_) => break,
        };
        if line.len() > MAX_LINE_BYTES {
            let resp = json!({
                "jsonrpc": "2.0",
                "id": null,
                "error": { "code": -32700, "message": "Request too large" }
            });
            writeln!(out, "{}", resp)?;
            out.flush()?;
            continue;
        }

        let request: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": { "code": -32700, "message": format!("Parse error: {}", e) }
                });
                writeln!(out, "{}", resp)?;
                out.flush()?;
                continue;
            }
        };

        // JSON-RPC notifications have no "id" field — must not send a response.
        let is_notification = request.get("id").is_none();
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");

        if is_notification {
            // Fire-and-forget: process it but never write a response.
            let _ = handle_request(method, &request, base).await;
            continue;
        }

        let response = handle_request(method, &request, base).await;

        let resp = match response {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err(e) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32603, "message": e.to_string() }
            }),
        };

        writeln!(out, "{}", resp)?;
        out.flush()?;
    }

    Ok(())
}

async fn handle_request(method: &str, request: &Value, base: &Path) -> Result<Value> {
    match method {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": SERVER_NAME, "version": SERVER_VERSION }
        })),

        "notifications/initialized" => Ok(json!({})),

        "tools/list" => Ok(json!({
            "tools": tool_definitions()
        })),

        "tools/call" => {
            let params = request
                .get("params")
                .ok_or_else(|| anyhow::anyhow!("missing params"))?;
            let name = params
                .get("name")
                .and_then(|n| n.as_str())
                .ok_or_else(|| anyhow::anyhow!("missing tool name"))?;
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            call_tool(name, &args, base).await
        }

        other => anyhow::bail!("Method not found: {}", other),
    }
}

fn tool_definitions() -> Value {
    json!([
        {
            "name": "aigit_log",
            "description": "List recent aigit commits. Filter by agent or time window.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "agent": { "type": "string", "description": "Filter by agent ID" },
                    "limit": { "type": "integer", "description": "Max commits to return (default 20)" },
                    "since": { "type": "integer", "description": "Only show commits after this Unix ms timestamp" }
                }
            }
        },
        {
            "name": "aigit_show",
            "description": "Show full details of an aigit commit by ID or ID prefix.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Commit ID or prefix" }
                },
                "required": ["id"]
            }
        },
        {
            "name": "aigit_diff",
            "description": "Show a textual diff between two aigit commits.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "commit1": { "type": "string", "description": "First commit ID or prefix" },
                    "commit2": { "type": "string", "description": "Second commit ID or prefix" }
                },
                "required": ["commit1", "commit2"]
            }
        },
        {
            "name": "aigit_blame",
            "description": "Show which agent/commit generated each line of a file.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "File path to blame" },
                    "lines": { "type": "string", "description": "Optional line range e.g. \"10-20\"" }
                },
                "required": ["file"]
            }
        },
        {
            "name": "aigit_context",
            "description": "Show recent aigit commit history for a file or the whole repo.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path (omit for repo-wide)" },
                    "limit": { "type": "integer", "description": "Max commits (default 10)" }
                }
            }
        },
        {
            "name": "aigit_conflict_check",
            "description": "Check whether a file has been modified by multiple agents recently.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path to check" },
                    "agent": { "type": "string", "description": "Exclude this agent from conflict detection" },
                    "window": { "type": "integer", "description": "Only consider last N commits (default 10)" }
                },
                "required": ["path"]
            }
        },
        {
            "name": "aigit_merge",
            "description": "Merge two aigit commits. Optionally use LLM-assisted merge.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "source": { "type": "string", "description": "Source commit ID or prefix" },
                    "target": { "type": "string", "description": "Target commit ID or prefix" },
                    "llm": { "type": "boolean", "description": "Use LLM-assisted merge (default false)" }
                },
                "required": ["source", "target"]
            }
        }
    ])
}

async fn call_tool(name: &str, args: &Value, base: &Path) -> Result<Value> {
    let aigit_dir = base.join(".aigit");
    if !aigit_dir.exists() {
        anyhow::bail!("aigit repository not initialized. Run 'aigit init' first.");
    }
    let db = db::Database::connect(aigit_dir.join("db.sqlite")).await?;

    match name {
        "aigit_log" => {
            let agent = args
                .get("agent")
                .and_then(|a| a.as_str())
                .map(|s| s.to_string());
            let limit = args.get("limit").and_then(|l| l.as_u64()).unwrap_or(20) as u32;
            let since = args.get("since").and_then(|s| s.as_i64());
            let commits = db.list_commits(agent.as_deref(), limit, since).await?;
            let text = commits
                .iter()
                .map(|c| {
                    format!(
                        "[{}] {} | {} | {}",
                        &c.id[..c.id.len().min(12)],
                        c.agent_id,
                        c.intent.as_deref().unwrap_or("no intent"),
                        chrono::DateTime::from_timestamp_millis(c.timestamp)
                            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                            .unwrap_or_else(|| c.timestamp.to_string())
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            text_result(if text.is_empty() {
                "No commits found.".into()
            } else {
                text
            })
        }

        "aigit_show" => {
            let id = args
                .get("id")
                .and_then(|i| i.as_str())
                .ok_or_else(|| anyhow::anyhow!("id is required"))?;
            let commit = db
                .get_commit_by_prefix(id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Commit not found: {}", id))?;
            text_result(format!(
                "id:      {}\nagent:   {}\nmodel:   {}\nintent:  {}\ngit:     {}\ntime:    {}\nprompt:\n{}\n\noutput:\n{}",
                commit.id,
                commit.agent_id,
                commit.model,
                commit.intent.as_deref().unwrap_or("none"),
                commit.git_hash.as_deref().unwrap_or("none"),
                chrono::DateTime::from_timestamp_millis(commit.timestamp)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| commit.timestamp.to_string()),
                commit.prompt,
                commit.output,
            ))
        }

        "aigit_diff" => {
            use similar::{ChangeTag, TextDiff};
            let c1_id = args
                .get("commit1")
                .and_then(|i| i.as_str())
                .ok_or_else(|| anyhow::anyhow!("commit1 is required"))?;
            let c2_id = args
                .get("commit2")
                .and_then(|i| i.as_str())
                .ok_or_else(|| anyhow::anyhow!("commit2 is required"))?;
            let c1 = db
                .get_commit_by_prefix(c1_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Commit not found: {}", c1_id))?;
            let c2 = db
                .get_commit_by_prefix(c2_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Commit not found: {}", c2_id))?;
            let diff = TextDiff::from_lines(&c1.output, &c2.output);
            let mut out = String::new();
            for change in diff.iter_all_changes() {
                let sign = match change.tag() {
                    ChangeTag::Delete => "-",
                    ChangeTag::Insert => "+",
                    ChangeTag::Equal => " ",
                };
                out.push_str(&format!("{}{}", sign, change.value()));
            }
            text_result(if out.is_empty() {
                "No differences.".into()
            } else {
                out
            })
        }

        "aigit_blame" => {
            let file = args
                .get("file")
                .and_then(|f| f.as_str())
                .ok_or_else(|| anyhow::anyhow!("file is required"))?;
            validate_mcp_path(file)?;
            let lines_filter = args
                .get("lines")
                .and_then(|l| l.as_str())
                .map(|s| s.to_string());

            let file_path = std::path::Path::new(file);
            let blame_entries = crate::git::get_file_blame(base, file_path).unwrap_or_default();
            let hashes: Vec<String> = blame_entries
                .iter()
                .map(|e| e.commit_hash.clone())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            let hash_map = db.get_commits_by_git_hashes(&hashes).await?;

            let (start, end) = if let Some(ref r) = lines_filter {
                parse_line_range(r)
            } else {
                (1, usize::MAX)
            };

            let mut out = String::new();
            for entry in &blame_entries {
                // Each entry covers line_start..=line_end
                for line_num in entry.line_start..=entry.line_end {
                    if (line_num as usize) < start || (line_num as usize) > end {
                        continue;
                    }
                    let agent = hash_map
                        .get(&entry.commit_hash)
                        .map(|c| c.agent_id.as_str())
                        .unwrap_or("unknown");
                    out.push_str(&format!(
                        "{:4} | {:20} | {} ({})\n",
                        line_num,
                        agent,
                        &entry.commit_hash[..entry.commit_hash.len().min(8)],
                        entry.author,
                    ));
                }
            }
            text_result(if out.is_empty() {
                format!("No blame data for '{}'.", file)
            } else {
                out
            })
        }

        "aigit_context" => {
            let path = args.get("path").and_then(|p| p.as_str());
            if let Some(p) = path {
                validate_mcp_path(p)?;
            }
            let limit = args.get("limit").and_then(|l| l.as_u64()).unwrap_or(10) as u32;

            let commits = if let Some(p) = path {
                let git_hashes = crate::git::get_commits_for_file(base, std::path::Path::new(p))
                    .unwrap_or_default();
                if !git_hashes.is_empty() {
                    let map = db.get_commits_by_git_hashes(&git_hashes).await?;
                    let mut v: Vec<_> = map.into_values().collect();
                    v.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
                    v.truncate(limit as usize);
                    v
                } else {
                    let mut v = db.get_commits_for_artifact(p).await?;
                    v.truncate(limit as usize);
                    v
                }
            } else {
                db.list_commits(None, limit, None).await?
            };

            let text = commits
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    format!(
                        "[{}] {} | {} | intent: \"{}\"\n    aigit: {}\n    prompt: {}",
                        i + 1,
                        chrono::DateTime::from_timestamp_millis(c.timestamp)
                            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                            .unwrap_or_else(|| c.timestamp.to_string()),
                        c.agent_id,
                        c.intent.as_deref().unwrap_or("none"),
                        &c.id[..c.id.len().min(12)],
                        &c.prompt.chars().take(120).collect::<String>(),
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            text_result(if text.is_empty() {
                "No context found.".into()
            } else {
                text
            })
        }

        "aigit_conflict_check" => {
            let path = args
                .get("path")
                .and_then(|p| p.as_str())
                .ok_or_else(|| anyhow::anyhow!("path is required"))?;
            let agent_filter = args
                .get("agent")
                .and_then(|a| a.as_str())
                .map(|s| s.to_string());
            let window = args.get("window").and_then(|w| w.as_u64()).unwrap_or(10) as usize;

            let rows = db.get_artifact_commit_rows().await?;
            let mut agents: std::collections::HashMap<String, Option<String>> =
                std::collections::HashMap::new();
            let mut count = 0usize;
            for row in &rows {
                if row.artifact_path != path {
                    continue;
                }
                if window > 0 && count >= window {
                    break;
                }
                count += 1;
                agents
                    .entry(row.agent_id.clone())
                    .or_insert_with(|| row.intent.clone());
            }
            if let Some(ref a) = agent_filter {
                agents.remove(a);
            }

            if agents.is_empty() {
                text_result(format!("No conflicts detected for '{}'.", path))
            } else {
                let list: Vec<String> = agents
                    .iter()
                    .map(|(id, intent)| {
                        format!(
                            "{} (intent: \"{}\")",
                            id,
                            intent.as_deref().unwrap_or("none")
                        )
                    })
                    .collect();
                text_result(format!(
                    "CONFLICT: '{}' was recently modified by: {}",
                    path,
                    list.join(", ")
                ))
            }
        }

        "aigit_merge" => {
            let source_id = args
                .get("source")
                .and_then(|s| s.as_str())
                .ok_or_else(|| anyhow::anyhow!("source is required"))?;
            let target_id = args
                .get("target")
                .and_then(|s| s.as_str())
                .ok_or_else(|| anyhow::anyhow!("target is required"))?;
            let use_llm = args.get("llm").and_then(|l| l.as_bool()).unwrap_or(false);

            // Write result to a temp file. quiet=true suppresses all progress
            // println!() calls so they don't corrupt the JSON-RPC stdout stream.
            let tmp = tempfile::NamedTempFile::new()?;
            let tmp_path = tmp.path().to_string_lossy().to_string();
            crate::cli::merge(
                crate::cli::MergeArgs {
                    source: source_id.to_string(),
                    target: target_id.to_string(),
                    llm: use_llm,
                    output: Some(tmp_path.clone()),
                    quiet: true,
                },
                base,
            )
            .await?;
            let result = std::fs::read_to_string(&tmp_path).unwrap_or_default();
            text_result(result)
        }

        other => anyhow::bail!("Unknown tool: {}", other),
    }
}

/// Reject file paths from MCP callers that contain traversal components or are absolute.
fn validate_mcp_path(path: &str) -> Result<()> {
    let p = std::path::Path::new(path);
    if p.is_absolute() {
        anyhow::bail!(
            "MCP tool file path must be relative, got absolute path: {}",
            path
        );
    }
    for component in p.components() {
        if component == std::path::Component::ParentDir {
            anyhow::bail!("MCP tool file path '{}' contains '..' traversal", path);
        }
    }
    Ok(())
}

fn text_result(text: String) -> Result<Value> {
    Ok(json!({
        "content": [{ "type": "text", "text": text }]
    }))
}

fn parse_line_range(s: &str) -> (usize, usize) {
    if let Some((a, b)) = s.split_once('-') {
        let start = a.trim().parse::<usize>().unwrap_or(1);
        let end = b.trim().parse::<usize>().unwrap_or(usize::MAX);
        (start, end)
    } else if let Ok(n) = s.trim().parse::<usize>() {
        (n, n)
    } else {
        (1, usize::MAX)
    }
}

fn install_mcp_json(base: &Path) -> Result<()> {
    let mcp_path = base.join(".mcp.json");

    // Determine the aigit command (prefer binary, fall back to cargo run)
    let aigit_cmd = if which_aigit() {
        json!({ "type": "stdio", "command": "aigit", "args": ["mcp"] })
    } else {
        let manifest = base.join("Cargo.toml");
        json!({
            "type": "stdio",
            "command": "cargo",
            "args": ["run", "--manifest-path", manifest.to_string_lossy(), "--quiet", "--", "mcp"]
        })
    };

    let mut existing: Value = if mcp_path.exists() {
        let contents = std::fs::read_to_string(&mcp_path)?;
        serde_json::from_str(&contents).unwrap_or(json!({}))
    } else {
        json!({})
    };

    existing
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!(".mcp.json root is not an object"))?
        .entry("servers")
        .or_insert(json!({}))
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("servers is not an object"))?
        .insert("aigit".to_string(), aigit_cmd);

    std::fs::write(&mcp_path, serde_json::to_string_pretty(&existing)?)?;
    println!("Wrote: {}", mcp_path.display());
    println!("aigit MCP server registered. Restart Claude Code to pick it up.");
    Ok(())
}

fn which_aigit() -> bool {
    std::process::Command::new("which")
        .arg("aigit")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
