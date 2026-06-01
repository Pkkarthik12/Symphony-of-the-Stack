#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
export SYMPHONY_WEB_DIR="${ROOT}/web"
echo "Symphony UI: http://localhost:8765"
exec cargo run -p symphony-bridge --release
