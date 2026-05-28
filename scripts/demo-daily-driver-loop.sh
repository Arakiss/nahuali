#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

require_command() {
  local command_name="$1"

  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "$command_name is required for the daily-driver demo" >&2
    exit 1
  fi
}

require_command jq

RUN_ID="${NAHUALI_DEMO_RUN_ID:-$(date +%s)}"
NOW_MS="${NAHUALI_DEMO_NOW_MS:-1779928800000}"
DEADLINE_MS="${NAHUALI_DEMO_DEADLINE_MS:-$((NOW_MS + 86400000))}"
DEMO_DB="${NAHUALI_DEMO_DB:-.local/demo-daily-driver-${RUN_ID}}"

if [[ -n "${NAHUALI_BIN:-}" ]]; then
  NAHUALI="$NAHUALI_BIN"
elif [[ -x "$ROOT/target/debug/nahuali" ]]; then
  NAHUALI="$ROOT/target/debug/nahuali"
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

printf 'Nahuali agent-first daily-driver CLI demo\n'
printf 'database=%s\n' "$DEMO_DB"
printf 'now_ms=%s\n' "$NOW_MS"

run_nahuali validate --json \
  | summarize "1. Preflight ledger validation" \
      '{valid, event_count, record_ledger_table, projection}'

run_nahuali remember \
  "Nahuali beta work needs an agent-first CLI loop with stable JSON, authority, review, and proactive signals." \
  --tag beta \
  --mention Nahuali \
  --mention CLI \
  --scope project:Nahuali >/dev/null

run_nahuali claim \
  Nahuali needs "agent-first CLI loop" \
  --confidence 0.94 \
  --source-last \
  --scope project:Nahuali >/dev/null

run_nahuali link \
  Nahuali uses CLI \
  --confidence 0.91 \
  --source-last \
  --scope project:Nahuali >/dev/null

run_nahuali claim \
  Pilot owns "feedback inbox" \
  --confidence 0.52 \
  --scope project:Nahuali >/dev/null

INTENTION_JSON="$(run_nahuali intention \
  "Validate the agent-first CLI loop before the beta cut" \
  --kind goal \
  --priority high \
  --source-last \
  --scope project:Nahuali \
  --json)"
INTENTION_ID="$(printf '%s\n' "$INTENTION_JSON" | jq -r '.id')"

run_nahuali intention-update "$INTENTION_ID" \
  --deadline-at-ms "$DEADLINE_MS" \
  --goal "Nahuali beta" \
  --progress 40 \
  --json >/dev/null

run_nahuali session-resume --json \
  | summarize "2. Session resume contract" \
      '{
        authority: .report.authority,
        summary: .report.summary,
        latest_episode: .report.recent_episodes[0],
        top_intention: .report.active_intentions[0],
        review_items: [.report.review_items[0:3][] | {
          priority,
          action,
          title,
          operator_guidance
        }]
      }'

run_nahuali recall \
  "agent-first CLI loop" \
  --authority \
  --json \
  --scope project:Nahuali \
  | summarize "3. Evidence-backed recall for agent planning" \
      '{
        top_result: .results[0],
        top_result_trust: .results[0].trust,
        store_authority: .authority,
        health_counts: {
          supported_fact_count: .health.supported_fact_count,
          unsupported_fact_count: .health.unsupported_fact_count,
          blind_spot_count: .health.blind_spot_count
        }
      }'

run_nahuali goal-progress --json \
  | summarize "4. Goal progress contract" \
      '{
        goal_count: .report.goal_count,
        goals: [.report.goals[] | {
          description,
          status,
          explicit_progress_percent,
          derived_progress_percent,
          active_count,
          completed_count,
          blocked_count
        }]
      }'

run_nahuali deadlines \
  --now-ms "$NOW_MS" \
  --horizon-ms 604800000 \
  --json \
  | summarize "5. Deadline signals" \
      '{
        summary: .report.summary,
        deadlines: [.report.deadlines[] | {
          description,
          priority,
          deadline_at_ms,
          state,
          guidance
        }]
      }'

run_nahuali proactive \
  --now-ms "$NOW_MS" \
  --json \
  | summarize "6. Proactive operator signals" \
      '{
        summary: .report.summary,
        deadlines: .report.deadlines.summary,
        anomalies: .report.anomalies.summary,
        capture_opportunities: [.report.capture_opportunities[0:3][] | {
          priority,
          title,
          suggested_action
        }]
      }'

run_nahuali inspect --json \
  | summarize "7. Self-inspection health" \
      '{
        authority: {
          unsupported_fact_count,
          low_confidence_fact_count,
          blind_spot_count,
          warnings
        }
      }'

run_nahuali review --json \
  | summarize "8. Operator review queue" \
      '{
        summary,
        top_items: [.items[0:4][] | {
          priority,
          action,
          title,
          detail,
          operator_guidance
        }]
      }'

run_nahuali session-resume --json \
  | jq -e '.report.summary.returned_episode_count >= 1 and .report.summary.returned_intention_count >= 1' >/dev/null
run_nahuali recall "agent-first CLI loop" --authority --json --scope project:Nahuali \
  | jq -e '.results[0].trust.can_trust == true' >/dev/null
run_nahuali deadlines --now-ms "$NOW_MS" --horizon-ms 604800000 --json \
  | jq -e '.report.summary.due_soon_count >= 1' >/dev/null
run_nahuali proactive --now-ms "$NOW_MS" --json \
  | jq -e '.report.summary.deadline_count >= 1 and .report.summary.capture_opportunity_count >= 1 and .report.summary.anomaly_count >= 1' >/dev/null

printf '\nAgent-first daily-driver loop passed.\n'
printf 'Re-run CLI commands against %s to inspect the same ledger.\n' "$DEMO_DB"
