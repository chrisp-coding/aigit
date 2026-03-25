#!/usr/bin/env bash
# demo/run_demo.sh — aigit end-to-end demo
#
# Simulates two AI agents making conflicting edits to the same file, then
# resolves the conflict autonomously via LLM merge.
#
# Usage:
#   ANTHROPIC_API_KEY=sk-... ./demo/run_demo.sh
#
# Requires: ANTHROPIC_API_KEY set in environment

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
AIGIT="$REPO_ROOT/target/debug/aigit"

# ── Preflight checks ─────────────────────────────────────────────────────────

if [[ -z "${ANTHROPIC_API_KEY:-}" ]]; then
    echo "ERROR: ANTHROPIC_API_KEY is not set."
    echo "Usage: ANTHROPIC_API_KEY=sk-... ./demo/run_demo.sh"
    exit 1
fi

# ── Build ────────────────────────────────────────────────────────────────────

echo "==> Building aigit..."
cargo build --manifest-path "$REPO_ROOT/Cargo.toml" --quiet
echo "    Built: $AIGIT"

# ── Set up demo repo ─────────────────────────────────────────────────────────

DEMO_DIR=$(mktemp -d)
trap 'rm -rf "$DEMO_DIR"' EXIT

echo "==> Demo repo: $DEMO_DIR"
cd "$DEMO_DIR"
git init --quiet
git config user.email "demo@aigit"
git config user.name "aigit demo"
$AIGIT init

# Write the LLM config so resolve --llm knows to use the Anthropic API.
cat > .aigit/config.toml << 'TOML'
[llm]
provider = "anthropic"
model    = "claude-sonnet-4-6"
TOML

# ── Seed file ────────────────────────────────────────────────────────────────

cat > calculator.rs << 'EOF'
fn add(a: i32, b: i32) -> i32 { a + b }
fn multiply(a: i32, b: i32) -> i32 { a * b }
fn subtract(a: i32, b: i32) -> i32 { a - b }
EOF

git add calculator.rs
git commit --quiet -m "initial calculator"

# ── Agent 1: performance-agent ───────────────────────────────────────────────

cat > calculator.rs << 'EOF'
#[inline(always)]
pub fn add(a: i32, b: i32) -> i32 {
    a.wrapping_add(b)
}

#[inline(always)]
pub fn multiply(a: i32, b: i32) -> i32 {
    a.wrapping_mul(b)
}

#[inline(always)]
pub fn subtract(a: i32, b: i32) -> i32 {
    a.wrapping_sub(b)
}
EOF

echo "Rewrite calculator.rs to maximize runtime performance. Use wrapping \
arithmetic and force-inline all hot functions to eliminate call overhead." \
    | "$AIGIT" commit \
        --agent  "performance-agent" \
        --intent "optimize calculator for maximum runtime performance" \
        --model  "claude-sonnet-4-6" \
        --output calculator.rs

git add calculator.rs
git commit --quiet -m "performance-agent: wrapping arithmetic + inlining"

# ── Agent 2: readability-agent ───────────────────────────────────────────────

cat > calculator.rs << 'EOF'
/// Adds two integers and returns the sum.
pub fn add(first: i32, second: i32) -> i32 {
    let sum = first + second;
    sum
}

/// Multiplies two integers and returns the product.
pub fn multiply(first: i32, second: i32) -> i32 {
    let product = first * second;
    product
}

/// Subtracts the second integer from the first and returns the difference.
pub fn subtract(first: i32, second: i32) -> i32 {
    let difference = first - second;
    difference
}
EOF

echo "Rewrite calculator.rs for maximum readability: add doc comments, use \
descriptive parameter names, and store results in named intermediate variables." \
    | "$AIGIT" commit \
        --agent  "readability-agent" \
        --intent "improve calculator readability with doc comments and named variables" \
        --model  "claude-sonnet-4-6" \
        --output calculator.rs

git add calculator.rs
git commit --quiet -m "readability-agent: doc comments + descriptive names"

# ── Detect conflicts ─────────────────────────────────────────────────────────

divider() { printf '\n%.0s─' {1..60}; echo; }

divider
echo "  aigit conflicts"
divider
"$AIGIT" conflicts

# ── Autonomous LLM resolution ────────────────────────────────────────────────

divider
echo "  aigit resolve --llm --output calculator.rs"
divider
"$AIGIT" resolve calculator.rs --llm --output calculator.rs

# ── Show the resolved file ───────────────────────────────────────────────────

divider
echo "  Resolved calculator.rs"
divider
cat calculator.rs

# ── Full log with provenance ─────────────────────────────────────────────────

divider
echo "  aigit log"
divider
"$AIGIT" log

# ── Per-file history (agent attribution) ────────────────────────────────────

divider
echo "  aigit context calculator.rs"
divider
"$AIGIT" context calculator.rs

divider
echo "  Demo complete — all three commits recorded with full provenance."
divider
