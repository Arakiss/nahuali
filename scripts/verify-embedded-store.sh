#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="${NAHUALI_VERIFY_BIN_DIR:-$ROOT/target/debug}"
CLI="${NAHUALI_VERIFY_CLI:-$BIN_DIR/nahuali}"
MCP="${NAHUALI_VERIFY_MCP:-$BIN_DIR/nahuali-mcp}"

for binary in "$CLI" "$MCP"; do
  if [[ ! -x "$binary" ]]; then
    printf 'error: missing executable %s\n' "$binary" >&2
    printf 'build both binaries with: cargo build -p nahuali-cli -p nahuali-mcp\n' >&2
    exit 1
  fi
done
command -v jq >/dev/null 2>&1 || {
  printf 'error: jq is required\n' >&2
  exit 1
}

tmp="$(mktemp -d "${TMPDIR:-/tmp}/nahuali-embedded-verify.XXXXXX")"
mcp_pid=""
cleanup() {
  exec 3>&- 2>/dev/null || true
  if [[ -n "$mcp_pid" ]] && kill -0 "$mcp_pid" 2>/dev/null; then
    kill "$mcp_pid" 2>/dev/null || true
    wait "$mcp_pid" 2>/dev/null || true
  fi
  rm -r "$tmp"
}
trap cleanup EXIT

home="$tmp/home"
operator_home="$tmp/operator-home"
mkdir -p "$operator_home/.claude"
env HOME="$operator_home" NAHUALI_HOME="$home" NO_COLOR=1 \
  "$CLI" init >"$tmp/init-first.txt"
cp "$operator_home/.claude/skills/nahuali/SKILL.md" "$tmp/installed-skill.md"
env HOME="$operator_home" NAHUALI_HOME="$home" NO_COLOR=1 \
  "$CLI" init >"$tmp/init-second.txt"
cmp "$tmp/installed-skill.md" "$operator_home/.claude/skills/nahuali/SKILL.md"
grep -F 'already installed' "$tmp/init-second.txt" >/dev/null
grep -F '"command": "nahuali-mcp"' "$tmp/init-first.txt" >/dev/null
grep -F '["--database", "memory"]' "$tmp/init-first.txt" >/dev/null

env -u NAHUALI_DB_URL NAHUALI_HOME="$home" NO_COLOR=1 \
  "$CLI" remember "Lena owns the release notes" --mention Lena --tag product --json \
  >"$tmp/remember.json"
env -u NAHUALI_DB_URL NAHUALI_HOME="$home" NO_COLOR=1 \
  "$CLI" claim Lena owns "release notes" --source-last --confidence 0.92 --json \
  >"$tmp/claim.json"
env -u NAHUALI_DB_URL NAHUALI_HOME="$home" NO_COLOR=1 \
  "$CLI" recall "Lena release notes" --authority --json \
  >"$tmp/recall.json"
jq -e '.results | any(.kind == "claim" and .trust.can_trust == true)' \
  "$tmp/recall.json" >/dev/null

# Reopen the embedded store repeatedly to catch shutdown or lock-release errors.
for _ in 1 2 3 4 5; do
  env -u NAHUALI_DB_URL NAHUALI_HOME="$home" NO_COLOR=1 \
    "$CLI" recall "Lena release notes" --authority --json >/dev/null
done

fifo="$tmp/mcp.stdin"
mkfifo "$fifo"
exec 3<>"$fifo"
env -u NAHUALI_DB_URL NAHUALI_HOME="$home" NO_COLOR=1 \
  "$MCP" 3>&- <"$fifo" >"$tmp/mcp.stdout" 2>"$tmp/mcp.stderr" &
mcp_pid="$!"

printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"embedded-store-verifier","version":"1.0.0"}}}' >&3

initialized=""
for _ in $(seq 1 100); do
  if [[ -s "$tmp/mcp.stdout" ]]; then
    initialized="$(sed -n '1p' "$tmp/mcp.stdout")"
    break
  fi
  if ! kill -0 "$mcp_pid" 2>/dev/null; then
    printf 'error: MCP server exited before initialization\n' >&2
    cat "$tmp/mcp.stderr" >&2
    exit 1
  fi
  sleep 0.1
done
printf '%s' "$initialized" | jq -e \
  '.result.serverInfo.name == "nahuali" and (.result.capabilities.tools | type == "object")' \
  >/dev/null

if env -u NAHUALI_DB_URL NAHUALI_HOME="$home" NO_COLOR=1 \
  "$CLI" recall "Lena release notes" --json \
  >"$tmp/contended.stdout" 2>"$tmp/contended.stderr"; then
  printf 'error: a second process opened the embedded store while MCP owned it\n' >&2
  exit 1
fi
grep -F 'Another Nahuali process may be using it' "$tmp/contended.stderr" >/dev/null

# Closing the last writer sends EOF to the MCP server. The CLI must regain the
# store immediately and recover the same memory after the server exits.
exec 3>&-
wait "$mcp_pid"
mcp_pid=""
env -u NAHUALI_DB_URL NAHUALI_HOME="$home" NO_COLOR=1 \
  "$CLI" recall "Lena release notes" --authority --json \
  | jq -e '.results | any(.kind == "claim" and .trust.can_trust == true)' >/dev/null

printf 'embedded store verification passed\n'
