# Your own Claude Code with service-arb removed (server entry and its token), so `claude mcp add` runs as for a new member; auth opens a Chromium with no sessions.
# Wiped every run; `--keep` resumes the last one.
s="$(git rev-parse --show-toplevel)/scripts/mcp-sandbox/tmp"
if [ "${1:-}" = "--keep" ]; then shift; else rm -rf "$s"; fi

if [ ! -d "$s" ]; then
  mkdir -p "$s/cfg" "$s/chrome" "$s/bin"
  for f in settings.json CLAUDE.md rules skills agents hooks plugins keybindings.json; do # not the daemon or session state, which the real install owns
    if [ -e ~/.claude/"$f" ]; then ln -s ~/.claude/"$f" "$s/cfg/$f"; fi
  done
  jq '.mcpOAuth |= with_entries(select(.key | startswith("service-arb|") | not))' ~/.claude/.credentials.json > "$s/cfg/.credentials.json"
  chmod 600 "$s/cfg/.credentials.json"
  jq --arg s "$s" 'del(.mcpServers["service-arb"]) | .projects |= map_values(del(.mcpServers["service-arb"])) | .projects[$s].hasTrustDialogAccepted = true' ~/.claude.json > "$s/cfg/.claude.json"
  cat > "$s/bin/xdg-open" <<SHIM
#!/bin/sh
exec chromium --user-data-dir="$s/chrome" --no-first-run --no-default-browser-check "\$@" >/dev/null 2>&1 &
SHIM
  chmod +x "$s/bin/xdg-open"
  ln -s xdg-open "$s/bin/open"
fi

export CLAUDE_CONFIG_DIR="$s/cfg" PATH="$s/bin:$PATH" BROWSER="$s/bin/xdg-open"
cd "$s"
if ! claude mcp get service-arb >/dev/null 2>&1; then
  claude mcp add --scope user --transport http service-arb https://sa.evinvest.ltd/playbook_mcp
fi
exec claude "$@"
