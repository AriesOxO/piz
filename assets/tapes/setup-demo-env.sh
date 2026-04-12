#!/usr/bin/env bash
set -euo pipefail

HOME_PATH="${1:-$(dirname "$0")/.demo-home}"
PIZ_DIR="$HOME_PATH/.piz"
mkdir -p "$PIZ_DIR"

cat >"$PIZ_DIR/config.toml" <<'EOF'
default_backend = "openai"
cache_ttl_hours = 48
auto_confirm_safe = false
show_explanation = false
language = "en"
chat_history_size = 20
cache_max_entries = 1000

[openai]
api_key = "demo-key"
model = "demo-mock"
base_url = "http://127.0.0.1:18080"
EOF

cat >"$PIZ_DIR/last_exec.json" <<'EOF'
{
  "command": "python app.py",
  "exit_code": 1,
  "stdout": "",
  "stderr": "ModuleNotFoundError: No module named 'flask'",
  "timestamp": 1775900000
}
EOF

cat >"$PIZ_DIR/update_state.json" <<'EOF'
{
  "last_check": 1775900000,
  "latest_version": "0.3.4"
}
EOF
