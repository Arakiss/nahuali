#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

require_command() {
  local command_name="$1"

  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "$command_name is required for the self-inspection demo" >&2
    exit 1
  fi
}

require_command jq

RUN_ID="${NAHUALI_DEMO_RUN_ID:-$(date +%s)}"
DEMO_DB="${NAHUALI_DEMO_DB:-.local/demo-self-inspection-${RUN_ID}}"

if [[ -n "${NAHUALI_BIN:-}" ]]; then
  NAHUALI="$NAHUALI_BIN"
elif [[ -x "$ROOT/target/release/nahuali" ]]; then
  NAHUALI="$ROOT/target/release/nahuali"
else
  NAHUALI=""
fi

run_nahuali() {
  if [[ -n "$NAHUALI" ]]; then
    "$NAHUALI" --database "$DEMO_DB" "$@"
  else
    cargo run --quiet -p nahuali-cli -- --database "$DEMO_DB" "$@"
  fi
}

summarize() {
  local title="$1"
  local filter="$2"

  printf '\n## %s\n' "$title"
  jq "$filter"
}

mkdir -p .local
bash scripts/ensure-dev-stack.sh >/dev/null

printf 'Nahuali self-inspection demo\n'
printf 'database=%s\n' "$DEMO_DB"

run_nahuali validate --json \
  | summarize "Empty ledger is valid" \
      '{valid, event_count, record_ledger_table, projection}'

run_nahuali remember \
  "Lena owns the release notes for the beta launch." \
  --tag product \
  --mention Lena \
  --scope project:Nahuali >/dev/null

run_nahuali claim \
  Lena owns "release notes" \
  --confidence 0.92 \
  --source-last \
  --scope project:Nahuali >/dev/null

run_nahuali claim \
  Mateo owns "deployment keys" \
  --confidence 0.51 \
  --scope project:Nahuali >/dev/null

run_nahuali recall \
  "who owns release notes" \
  --authority \
  --json \
  --scope project:Nahuali \
  | summarize "Evidence-backed recall with store-level authority context" \
      '{
        top_result: .results[0],
        authority: .authority,
        health: {
          supported_fact_count: .health.supported_fact_count,
          unsupported_fact_count: .health.unsupported_fact_count,
          blind_spot_count: .health.blind_spot_count
        }
      }'

run_nahuali recall \
  "deployment keys owner" \
  --authority \
  --json \
  --scope project:Nahuali \
  | summarize "Weak memory is still visible but flagged" \
      '{
        top_result: .results[0],
        unsupported_result: (.results[] | select(.kind == "claim")),
        authority: .authority
      }'

run_nahuali inspect --json \
  | summarize "Knowledge-health inspection" \
      '{
        event_count,
        episode_count,
        supported_fact_count,
        unsupported_fact_count,
        blind_spot_count,
        warnings
      }'

run_nahuali review --json \
  | summarize "Operator review queue" \
      '{
        summary,
        top_items: [.items[0:3][] | {
          priority,
          action,
          title,
          detail,
          operator_guidance
        }]
      }'

printf '\nSelf-inspection is intentionally non-mutating: it reports review work but does not repair memory automatically.\n'
printf 'Demo complete. Re-run CLI commands against %s to inspect the same ledger.\n' "$DEMO_DB"
