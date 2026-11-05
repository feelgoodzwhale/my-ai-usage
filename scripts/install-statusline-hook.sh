#!/bin/bash
set -euo pipefail

TARGET_SCRIPT="$HOME/.claude/statusline-command.sh"
DATA_DIR="$HOME/.local/share/my-ai-usage"
mkdir -p "$HOME/.claude" "$DATA_DIR"

if [ -f "$TARGET_SCRIPT" ] && ! grep -qE "my-(ai-)?usage statusline snapshot" "$TARGET_SCRIPT"; then
  cp "$TARGET_SCRIPT" "${TARGET_SCRIPT}.my-ai-usage.bak"
fi

cat > "$TARGET_SCRIPT" << 'EOF'
#!/bin/bash
input=$(cat)

# --- my-ai-usage statusline snapshot (non-blocking atomic write) ---
{
  DATA_DIR="$HOME/.local/share/my-ai-usage"
  mkdir -p "$DATA_DIR" 2>/dev/null
  NOW_TS=$(date +%s)
  if echo "$input" | jq -e '.agent_state != null or .plan_tier != null or .cli_version != null' >/dev/null 2>&1; then
    echo "$input" | jq -c --argjson ts "$NOW_TS" '. + {_captured_at: $ts}' > "$DATA_DIR/agy-status.json.tmp" 2>/dev/null \
      && mv -f "$DATA_DIR/agy-status.json.tmp" "$DATA_DIR/agy-status.json" 2>/dev/null
  elif echo "$input" | jq -e '.rate_limits != null or .session_id != null or .cost != null' >/dev/null 2>&1; then
    echo "$input" | jq -c --argjson ts "$NOW_TS" '. + {_captured_at: $ts}' > "$DATA_DIR/claude-status.json.tmp" 2>/dev/null \
      && mv -f "$DATA_DIR/claude-status.json.tmp" "$DATA_DIR/claude-status.json" 2>/dev/null
  fi
} >/dev/null 2>&1 &
# --- end my-ai-usage statusline snapshot ---

model=$(echo "$input" | jq -r '.model.display_name // .model.id // "Claude"')
effort=$(echo "$input" | jq -r '.effort.level // empty')
cwd=$(echo "$input" | jq -r '.workspace.current_dir // .cwd // empty')
branch=$(git -C "${cwd:-.}" rev-parse --abbrev-ref HEAD 2>/dev/null)

out=""
[ -n "$branch" ] && out="\033[2;32mBranch: ${branch}\033[0m | "
out="${out}\033[2;36m${model}\033[0m"
[ -n "$effort" ] && out="$out \033[2;34m(${effort})\033[0m"

# usage <jq key> <label> <color> <date fmt>; resets_at is epoch seconds
usage() {
  local used reset
  used=$(echo "$input" | jq -r ".rate_limits.$1.used_percentage // empty")
  [ -z "$used" ] && return
  reset=$(echo "$input" | jq -r ".rate_limits.$1.resets_at // empty")
  out="$out | \033[2;$3m$2: $(printf '%.0f' "$used")%"
  [ -n "$reset" ] && out="$out (reset $(date -r "$reset" "+$4"))"
  out="$out\033[0m"
}

usage five_hour 5h 33 "%H:%M"
usage seven_day Weekly 35 "%a %d %H:%M"

printf "%b" "$out"
EOF

chmod +x "$TARGET_SCRIPT"

# Ensure ~/.claude/settings.json has statusLine configured if jq is available and statusLine is missing
if command -v jq >/dev/null 2>&1; then
  CLAUDE_SETTINGS="$HOME/.claude/settings.json"
  if [ -f "$CLAUDE_SETTINGS" ]; then
    if ! jq -e '.statusLine != null' "$CLAUDE_SETTINGS" >/dev/null 2>&1; then
      jq '. + {statusLine: {type: "command", command: "bash ~/.claude/statusline-command.sh"}}' "$CLAUDE_SETTINGS" > "${CLAUDE_SETTINGS}.tmp" \
        && mv -f "${CLAUDE_SETTINGS}.tmp" "$CLAUDE_SETTINGS"
    fi
  fi

  AGY_SETTINGS="$HOME/.gemini/antigravity-cli/settings.json"
  if [ -f "$AGY_SETTINGS" ]; then
    if ! jq -e '.statusLine != null' "$AGY_SETTINGS" >/dev/null 2>&1; then
      jq '. + {statusLine: {type: "command", command: "bash ~/.claude/statusline-command.sh", enabled: true}}' "$AGY_SETTINGS" > "${AGY_SETTINGS}.tmp" \
        && mv -f "${AGY_SETTINGS}.tmp" "$AGY_SETTINGS"
    fi
  fi
fi

echo "Updated $TARGET_SCRIPT"
