#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="${NAHUALI_DEMO_BIN_DIR:-$ROOT/target/debug}"
CLI="${NAHUALI_DEMO_CLI:-$BIN_DIR/nahuali}"
MCP="${NAHUALI_DEMO_MCP:-$BIN_DIR/nahuali-mcp}"
DEMO_HOME="${NAHUALI_DEMO_HOME:-${NAHUALI_HOME:-$HOME/.nahuali-launch-demo}}"
DEMO_DB="${NAHUALI_DEMO_DB:-launch_demo}"
SCOPE="project:Nahuali"

require_binary() {
  if [[ ! -x "$1" ]]; then
    printf 'Missing demo binary: %s\n' "$1" >&2
    printf 'Build the demo binaries with: cargo build -p nahuali-cli -p nahuali-mcp\n' >&2
    exit 1
  fi
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || {
    printf '%s is required for the launch demo\n' "$1" >&2
    exit 1
  }
}

run_cli() {
  env -u NAHUALI_DB_URL \
    NAHUALI_HOME="${NAHUALI_DEMO_HOME:-$DEMO_HOME}" \
    "$CLI" --database "${NAHUALI_DEMO_DB:-$DEMO_DB}" "$@"
}

seed() {
  run_cli remember \
    "The release review approved Tuesday as launch day." \
    --tag release --mention Launch --scope "$SCOPE" >/dev/null
  run_cli claim Launch day Tuesday \
    --source-last --confidence 0.94 --scope "$SCOPE" >/dev/null

  printf '\033[1;38;2;217;119;87mCLI · remember with evidence\033[0m\n'
  printf 'Observation  The release review approved Tuesday as launch day.\n'
  printf 'Claim        Launch day Tuesday\n'
  printf '\033[38;2;143;184;122mEvidence linked. The memory is ready for governed recall.\033[0m\n'
}

recall_cli() {
  local response
  response="$(run_cli recall "Launch day" --authority --json --scope "$SCOPE")"
  printf '\033[1;38;2;217;119;87mCLI · authority-aware recall\033[0m\n'
  jq -r '
    .results[]
    | select(.kind == "claim" and (.excerpt | contains("Tuesday")))
    | "\(.trust.mode | ascii_upcase)  \(.excerpt)\nEvidence  \(.evidence_id[0:18])…\nCan act   \(.trust.can_trust)"
  ' <<<"$response"
}

contradict() {
  run_cli claim Launch day Friday \
    --confidence 0.94 --scope "$SCOPE" >/dev/null

  printf '\033[1;38;2;217;119;87mCLI · another agent writes a competing claim\033[0m\n'
  printf 'Claim  Launch day Friday\n'
  printf 'Source none\n'
  printf '\033[38;2;224;177;94mThe new claim is stored, but it cannot silently replace sourced memory.\033[0m\n'
}

inspect_store() {
  local before after inspection
  before="$(run_cli audit --json | jq -r '.total_event_count')"
  inspection="$(run_cli self-inspect --json)"
  after="$(run_cli audit --json | jq -r '.total_event_count')"

  printf '\033[1;38;2;217;119;87mCLI · non-mutating self-inspection\033[0m\n'
  jq -r '
    "Contradictions  \(.summary.contradiction_count // .contradiction_count // 0)\nReview required  \(.write_back_policy.requires_operator_review)\nAutomatic write-back  \(.write_back_policy.automatic_write_back)"
  ' <<<"$inspection"
  printf 'Ledger records  %s before · %s after\n' "$before" "$after"
}

mcp_recall() {
  require_binary "$MCP"
  local tmp fifo pid response
  tmp="$(mktemp -d "${TMPDIR:-/tmp}/nahuali-launch-mcp.XXXXXX")"
  fifo="$tmp/mcp.stdin"
  mkfifo "$fifo"
  exec 3<>"$fifo"
  env -u NAHUALI_DB_URL \
    NAHUALI_HOME="${NAHUALI_DEMO_HOME:-$DEMO_HOME}" NO_COLOR=1 \
    "$MCP" --database "${NAHUALI_DEMO_DB:-$DEMO_DB}" \
    3>&- <"$fifo" >"$tmp/stdout" 2>"$tmp/stderr" &
  pid="$!"
  cleanup_mcp() {
    exec 3>&- 2>/dev/null || true
    if kill -0 "$pid" 2>/dev/null; then
      kill "$pid" 2>/dev/null || true
      wait "$pid" 2>/dev/null || true
    fi
    rm -r "$tmp"
  }
  trap cleanup_mcp RETURN

  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"nahuali-launch-demo","version":"1.0.0"}}}' >&3
  for _ in $(seq 1 100); do
    [[ "$(wc -l <"$tmp/stdout")" -ge 1 ]] && break
    kill -0 "$pid" 2>/dev/null || {
      sed -n '1,20p' "$tmp/stderr" >&2
      return 1
    }
    sleep 0.05
  done
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}' >&3
  printf '%s\n' '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"recall","arguments":{"query":"Launch day","scope":{"kind":"project","name":"Nahuali"},"kinds":["claim"],"requireEvidence":true}}}' >&3
  for _ in $(seq 1 100); do
    [[ "$(wc -l <"$tmp/stdout")" -ge 2 ]] && break
    sleep 0.05
  done
  response="$(sed -n '2p' "$tmp/stdout")"
  jq -e '.result.structuredContent.results | length > 0' <<<"$response" >/dev/null

  printf '\033[1;38;2;217;119;87mMCP · the agent asks before acting\033[0m\n'
  jq -r '
    .result.structuredContent as $memory
    | ($memory.results | map(.trust.mode | ascii_upcase) | unique | join(" / ")) as $modes
    | "Result verdicts  \($modes)\nStore authority  \($memory.authority.mode | ascii_upcase)\nCan act  \($memory.authority.can_trust)\nEvidence returned  \([$memory.results[].evidence_id] | unique | length)"
  ' <<<"$response"
}

verify() {
  local tmp clean compromised inspection
  tmp="$(mktemp -d "${TMPDIR:-/tmp}/nahuali-launch-demo.XXXXXX")"
  trap 'rm -r "$tmp"' RETURN
  export NAHUALI_DEMO_HOME="$tmp/home"
  export NAHUALI_DEMO_DB="launch_demo"

  seed >/dev/null
  clean="$(run_cli recall "Launch day" --authority --json --scope "$SCOPE")"
  jq -e '.results | any(.kind == "claim" and .trust.mode == "certify" and .trust.can_trust == true)' <<<"$clean" >/dev/null

  contradict >/dev/null
  compromised="$(run_cli recall "Launch day" --authority --json --scope "$SCOPE")"
  jq -e '.authority.mode == "block" and .authority.can_trust == false' <<<"$compromised" >/dev/null

  inspection="$(run_cli self-inspect --json)"
  jq -e '.write_back_policy.automatic_write_back == false and .write_back_policy.requires_operator_review == true' <<<"$inspection" >/dev/null
  mcp_recall >/dev/null
  printf 'launch demo contract passed\n'
}

require_binary "$CLI"
require_command jq

case "${1:-}" in
  seed) seed ;;
  recall) recall_cli ;;
  contradict) contradict ;;
  inspect) inspect_store ;;
  mcp-recall) mcp_recall ;;
  explore) run_cli explore ;;
  verify) verify ;;
  *)
    printf 'Usage: %s {seed|recall|contradict|inspect|mcp-recall|explore|verify}\n' "$0" >&2
    exit 2
    ;;
esac
