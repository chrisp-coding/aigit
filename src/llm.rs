use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone)]
pub struct LlmConfig {
    pub provider: String,
    pub model: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
}

impl std::fmt::Debug for LlmConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmConfig")
            .field("provider", &self.provider)
            .field("model", &self.model)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field("base_url", &self.base_url)
            .finish()
    }
}

#[derive(Deserialize)]
struct ConfigFile {
    llm: Option<LlmSection>,
}

#[derive(Deserialize)]
struct LlmSection {
    provider: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
    base_url: Option<String>,
}

/// Load LLM config from `.aigit/config.toml`, falling back to env vars.
/// Precedence: env vars > config file > built-in defaults.
pub fn load_llm_config(aigit_dir: &Path) -> Result<LlmConfig> {
    // Start with config-file values (lowest precedence)
    let mut provider = "anthropic".to_string();
    let mut model = "claude-sonnet-4-6".to_string();
    let mut api_key: Option<String> = None;
    let mut base_url: Option<String> = None;

    let config_path = aigit_dir.join("config.toml");
    if config_path.exists() {
        let contents = std::fs::read_to_string(&config_path)?;
        if let Ok(cfg) = toml::from_str::<ConfigFile>(&contents) {
            if let Some(llm) = cfg.llm {
                if let Some(p) = llm.provider {
                    provider = p;
                }
                if let Some(m) = llm.model {
                    model = m;
                }
                if let Some(k) = llm.api_key {
                    api_key = Some(k);
                }
                base_url = llm.base_url;
            }
        }
    }

    // Env vars override config file (higher precedence)
    if let Ok(p) = std::env::var("AIGIT_LLM_PROVIDER") {
        provider = p;
    }
    if let Ok(m) = std::env::var("AIGIT_LLM_MODEL") {
        model = m;
    }
    if let Ok(k) = std::env::var("ANTHROPIC_API_KEY") {
        api_key = Some(k);
    }

    // For ollama, default base_url if not set
    if provider == "ollama" && base_url.is_none() {
        base_url = Some("http://localhost:11434".to_string());
    }

    Ok(LlmConfig {
        provider,
        model,
        api_key,
        base_url,
    })
}

// ── Anthropic message types ────────────────────────────────────────────────

#[derive(Serialize)]
struct AnthropicRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    messages: Vec<AnthropicMessage<'a>>,
}

#[derive(Serialize)]
struct AnthropicMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
}

#[derive(Deserialize)]
struct AnthropicContent {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
}

// ── Ollama types ───────────────────────────────────────────────────────────

#[derive(Serialize)]
struct OllamaRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    stream: bool,
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
}

// ── URL validation ─────────────────────────────────────────────────────────

/// Validate a base_url before use to prevent SSRF.
/// - Anthropic: must use https://.
/// - Ollama: must be loopback (localhost / 127.0.0.1 / ::1) unless https://.
fn validate_base_url(url: &str, provider: &str) -> Result<()> {
    let is_https = url.starts_with("https://");
    let is_http = url.starts_with("http://");
    if !is_https && !is_http {
        anyhow::bail!("LLM base_url '{}' must start with https:// or http://", url);
    }
    match provider {
        "anthropic" => {
            if !is_https {
                anyhow::bail!("Anthropic base_url must use https:// (got '{}')", url);
            }
        }
        "ollama" => {
            if !is_https {
                let loopback = ["http://localhost", "http://127.0.0.1", "http://[::1]"];
                if !loopback.iter().any(|p| url.starts_with(p)) {
                    anyhow::bail!(
                        "Ollama base_url '{}' must be a loopback address \
                         (localhost / 127.0.0.1 / [::1]) or use https://",
                        url
                    );
                }
            }
        }
        _ => {}
    }
    Ok(())
}

// ── Public API ─────────────────────────────────────────────────────────────

/// Call the configured LLM with a prompt and return the response text.
pub async fn call_llm(config: &LlmConfig, prompt: &str) -> Result<String> {
    match config.provider.as_str() {
        "anthropic" => call_anthropic(config, prompt).await,
        "ollama" => call_ollama(config, prompt).await,
        "claude-cli" => call_claude_cli(prompt).await,
        other => anyhow::bail!(
            "Unknown LLM provider '{}'. Set provider = \"anthropic\", \"ollama\", or \"claude-cli\" in .aigit/config.toml.",
            other
        ),
    }
}

/// Try to call the `claude` CLI if it is in PATH; return None if unavailable.
/// Used by merge-content to prefer Claude Code's own auth over a separately
/// configured API key.
pub async fn try_claude_cli(prompt: &str) -> Option<String> {
    // Only attempt if `claude` is in PATH.
    if which::which("claude").is_err() {
        return None;
    }
    call_claude_cli(prompt).await.ok()
}

/// Result of an intent conflict check.
pub struct IntentConflictCheck {
    pub conflict: bool,
    pub reason: String,
}

/// Ask the LLM whether two agent intents genuinely conflict (i.e. satisfying one
/// requires compromising the other). Uses `claude` CLI if available, otherwise
/// falls back to the aigit-configured LLM.
pub async fn check_intent_conflict(
    agent_a: &str,
    intent_a: &str,
    agent_b: &str,
    intent_b: &str,
    config: Option<&LlmConfig>,
) -> Result<IntentConflictCheck> {
    let prompt = format!(
        "Two AI agents edited the same file with different intents. \
         Determine whether these intents genuinely conflict — meaning satisfying one \
         requires compromising the other — or whether they are compatible/orthogonal \
         and could both be satisfied in a single version.\n\
         \n\
         Agent A ({}): \"{}\"\n\
         Agent B ({}): \"{}\"\n\
         \n\
         Respond with valid JSON only, no explanation outside the JSON:\n\
         {{\"conflict\": true/false, \"reason\": \"one sentence explanation\"}}",
        agent_a, intent_a, agent_b, intent_b,
    );

    let raw = match try_claude_cli(&prompt).await {
        Some(output) => output,
        None => match config {
            Some(cfg) => call_llm(cfg, &prompt).await?,
            None => anyhow::bail!(
                "No LLM available for intent conflict check. \
                 Install the claude CLI or configure .aigit/config.toml [llm]."
            ),
        },
    };

    // Extract the JSON object from the response — the LLM may wrap it in prose.
    let json_str = extract_json_object(&raw).unwrap_or(raw.trim().to_string());

    let parsed: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
        anyhow::anyhow!(
            "Intent conflict check returned non-JSON response: {} (raw: {})",
            e,
            json_str.chars().take(200).collect::<String>()
        )
    })?;

    let conflict = parsed["conflict"].as_bool().unwrap_or(false);
    let reason = parsed["reason"]
        .as_str()
        .unwrap_or("no reason provided")
        .to_string();

    Ok(IntentConflictCheck { conflict, reason })
}

/// Extract the first `{...}` JSON object from a string.
fn extract_json_object(s: &str) -> Option<String> {
    let start = s.find('{')?;
    let end = s.rfind('}')?;
    if end >= start {
        Some(s[start..=end].to_string())
    } else {
        None
    }
}

async fn call_anthropic(config: &LlmConfig, prompt: &str) -> Result<String> {
    let api_key = config
        .api_key
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!(
            "Anthropic API key not set. Set ANTHROPIC_API_KEY env var or api_key in .aigit/config.toml."
        ))?;

    let base_url = config
        .base_url
        .as_deref()
        .unwrap_or("https://api.anthropic.com");

    validate_base_url(base_url, "anthropic")?;
    let url = format!("{}/v1/messages", base_url.trim_end_matches('/'));

    let body = AnthropicRequest {
        model: &config.model,
        max_tokens: 8192,
        messages: vec![AnthropicMessage {
            role: "user",
            content: prompt,
        }],
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;
    let resp = client
        .post(&url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        // Truncate to avoid leaking sensitive account metadata from error bodies.
        let truncated = text.chars().take(256).collect::<String>();
        anyhow::bail!("Anthropic API error {}: {}", status, truncated);
    }

    let parsed: AnthropicResponse = resp.json().await?;
    parsed
        .content
        .into_iter()
        .find(|c| c.kind == "text")
        .and_then(|c| c.text)
        .ok_or_else(|| anyhow::anyhow!("Anthropic response contained no text content"))
}

async fn call_ollama(config: &LlmConfig, prompt: &str) -> Result<String> {
    let base_url = config
        .base_url
        .as_deref()
        .unwrap_or("http://localhost:11434");

    validate_base_url(base_url, "ollama")?;
    let url = format!("{}/api/generate", base_url.trim_end_matches('/'));

    let body = OllamaRequest {
        model: &config.model,
        prompt,
        stream: false,
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;
    let resp = client.post(&url).json(&body).send().await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        let truncated = text.chars().take(256).collect::<String>();
        anyhow::bail!("Ollama API error {}: {}", status, truncated);
    }

    let parsed: OllamaResponse = resp.json().await?;
    Ok(parsed.response)
}

async fn call_claude_cli(prompt: &str) -> Result<String> {
    let output = tokio::process::Command::new("claude")
        .args(["--print", prompt])
        .output()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to invoke 'claude' CLI: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let truncated: String = stderr.chars().take(256).collect();
        anyhow::bail!("claude CLI exited non-zero: {}", truncated);
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
