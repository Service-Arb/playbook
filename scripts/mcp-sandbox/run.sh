# What a new member sees adding the MCP: a Claude config with your Claude login but no MCP tokens, and a Chrome with no sessions.
# Wiped every run; `--keep` resumes the last one.
s="$(git rev-parse --show-toplevel)/scripts/mcp-sandbox/tmp"
if [ "${1:-}" = "--keep" ]; then shift; else rm -rf "$s"; fi
mkdir -p "$s/cfg" "$s/chrome" "$s/bin"

if [ ! -f "$s/cfg/.credentials.json" ]; then
  jq 'del(.mcpOAuth)' ~/.claude/.credentials.json > "$s/cfg/.credentials.json"
  chmod 600 "$s/cfg/.credentials.json"
fi

cat > "$s/bin/xdg-open" <<SHIM
#!/bin/sh
exec chromium --user-data-dir="$s/chrome" --no-first-run --no-default-browser-check "\$@" >/dev/null 2>&1 &
SHIM
chmod +x "$s/bin/xdg-open"
ln -sf xdg-open "$s/bin/open"

export CLAUDE_CONFIG_DIR="$s/cfg" PATH="$s/bin:$PATH" BROWSER="$s/bin/xdg-open"
cd "$s"
if ! claude mcp get service-arb >/dev/null 2>&1; then
  claude mcp add --scope user --transport http service-arb https://sa.evinvest.ltd/playbook_mcp
fi
exec claude "$@"
