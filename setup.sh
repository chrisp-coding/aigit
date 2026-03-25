#!/bin/bash
set -e

# Install Rust if not present
if ! command -v cargo &> /dev/null; then
    echo "Installing Rust via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi

# Install sqlx-cli for migrations
if ! command -v sqlx &> /dev/null; then
    echo "Installing sqlx-cli..."
    cargo install sqlx-cli --no-default-features --features sqlite
fi

# Run migrations
echo "Running database migrations..."
sqlx database create
sqlx migrate run

# Install project pre-commit hook (enforces fmt + clippy before each commit)
HOOK_PATH=".git/hooks/pre-commit"
if [ -d ".git" ] && [ ! -f "$HOOK_PATH" ]; then
    cat > "$HOOK_PATH" << 'EOF'
#!/bin/bash
set -e
cargo fmt --check || { echo "Run 'cargo fmt' to fix formatting."; exit 1; }
cargo clippy -- -D warnings || { echo "Fix clippy warnings before committing."; exit 1; }
EOF
    chmod +x "$HOOK_PATH"
    echo "Installed pre-commit hook (fmt + clippy)."
fi

echo "Setup complete. Run 'cargo build' to compile."