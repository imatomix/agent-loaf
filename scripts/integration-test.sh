#!/usr/bin/env bash
# 本物の claude を使い、一時的な設定ディレクトリでプラグインを入れて外す。
# ANTHROPIC_API_KEY か CLAUDE_CODE_OAUTH_TOKEN があれば claude -p を 1 回動かしてイベントが届くことも確かめる
set -euo pipefail
cd "$(dirname "$0")/.."
./scripts/dev-install-hook.sh
cargo test -p agent-loaf-core --test integration_install -- --ignored --nocapture
