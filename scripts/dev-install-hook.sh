#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release -p agent-loaf-hook
install -d -m 700 "$HOME/.agent-loaf" "$HOME/.agent-loaf/bin"
install -m 755 target/release/agent-loaf-hook "$HOME/.agent-loaf/bin/agent-loaf-hook"
echo "installed: $HOME/.agent-loaf/bin/agent-loaf-hook"
