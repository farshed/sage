#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"

touch .env
cargo build --release --locked --workspace
./target/release/sage download-models

echo "Setup complete. Configure your LLM in .env, then run ./target/release/sage."
