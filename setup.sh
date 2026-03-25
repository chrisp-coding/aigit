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

echo "Setup complete. Run 'cargo build' to compile."