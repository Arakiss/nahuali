#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ "${NAHUALI_VALIDATE_SKIP_DEV_STACK:-0}" != "1" ]]; then
  bash scripts/ensure-dev-stack.sh
fi

WORK_DIR="$(mktemp -d)"

cleanup() {
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT

require_pattern() {
  local file="$1"
  local pattern="$2"
  local message="$3"

  if ! grep -Eq "$pattern" "$file"; then
    echo "$message" >&2
    echo "--- $file ---" >&2
    cat "$file" >&2
    exit 1
  fi
}

GLOBAL_NAHUALI_BEFORE="$(command -v nahuali || true)"

if [[ -n "${NAHUALI_DOGFOOD_BIN:-}" ]]; then
  NAHUALI_BIN="$NAHUALI_DOGFOOD_BIN"
elif [[ -n "${NAHUALI_DOGFOOD_BIN_DIR:-}" ]]; then
  NAHUALI_BIN="${NAHUALI_DOGFOOD_BIN_DIR%/}/nahuali"
else
  cargo build -p nahuali-cli --quiet
  TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
  case "$TARGET_DIR" in
    /*) ;;
    *) TARGET_DIR="$ROOT/$TARGET_DIR" ;;
  esac
  NAHUALI_BIN="$TARGET_DIR/debug/nahuali"
fi
if [[ ! -x "$NAHUALI_BIN" ]]; then
  echo "Rust nahuali binary is missing" >&2
  echo "expected: $NAHUALI_BIN" >&2
  exit 1
fi

RUN_ID="$(basename "$WORK_DIR" | tr -cd '[:alnum:]')"
STORE="dogfood_daily_${RUN_ID}_source"
RESTORE_STORE="dogfood_daily_${RUN_ID}_restore"
DRILL_STORE="dogfood_daily_${RUN_ID}_drill"
BACKUP="$WORK_DIR/daily-memory.backup.json"
INGEST_DOC="$WORK_DIR/scoped-ingest.json"

cat >"$INGEST_DOC" <<'JSON'
{
  "version": 1,
  "source": {
    "kind": "conversation",
    "title": "Daily dogfood preflight",
    "uri": "fixture://daily-dogfood-preflight",
    "metadata": {
      "origin": "dogfood-gate"
    },
    "scope": {
      "kind": "project",
      "name": "Nahuali",
      "key": "project:nahuali"
    }
  },
  "episodes": [
    {
      "ref": "message-1",
      "content": "Lena owns release notes for the scoped dogfood gate.",
      "tags": ["product"],
      "mentions": ["Lena", "Release Notes"],
      "source_position": 1,
      "source_role": "operator"
    }
  ],
  "claims": [
    {
      "subject": "Lena",
      "predicate": "owns",
      "object": "release notes",
      "source_episode_ref": "message-1",
      "confidence": 0.92
    }
  ],
  "links": [
    {
      "from": "Lena",
      "relation": "owns",
      "to": "Release Notes",
      "source_episode_ref": "message-1",
      "confidence": 0.9
    }
  ]
}
JSON

INGEST_DRY_RUN_OUTPUT="$WORK_DIR/scoped-ingest-dry-run.json"
"$NAHUALI_BIN" --database "$STORE" ingest "$INGEST_DOC" --dry-run --json >"$INGEST_DRY_RUN_OUTPUT"
require_pattern "$INGEST_DRY_RUN_OUTPUT" '"valid"[[:space:]]*:[[:space:]]*true' "scoped ingest preflight was not valid"
require_pattern "$INGEST_DRY_RUN_OUTPUT" '"dry_run"[[:space:]]*:[[:space:]]*true' "scoped ingest preflight did not report dry-run"
require_pattern "$INGEST_DRY_RUN_OUTPUT" '"ingested_event_count"[[:space:]]*:[[:space:]]*0' "scoped ingest preflight mutated the store"
require_pattern "$INGEST_DRY_RUN_OUTPUT" '"source_scoped"[[:space:]]*:[[:space:]]*true' "scoped ingest preflight did not preserve scope"
require_pattern "$INGEST_DRY_RUN_OUTPUT" '"key"[[:space:]]*:[[:space:]]*"project:nahuali"' "scoped ingest preflight did not report scope key"
require_pattern "$INGEST_DRY_RUN_OUTPUT" '"derived_record_count"[[:space:]]*:[[:space:]]*2' "scoped ingest preflight did not count derived records"
require_pattern "$INGEST_DRY_RUN_OUTPUT" '"evidence_linked_record_count"[[:space:]]*:[[:space:]]*2' "scoped ingest preflight did not count evidence-linked records"
require_pattern "$INGEST_DRY_RUN_OUTPUT" '"evidence_gap_count"[[:space:]]*:[[:space:]]*0' "scoped ingest preflight reported evidence gaps"
require_pattern "$INGEST_DRY_RUN_OUTPUT" '"referenced_episode_count"[[:space:]]*:[[:space:]]*1' "scoped ingest preflight did not count referenced episodes"
require_pattern "$INGEST_DRY_RUN_OUTPUT" '"unreferenced_episode_count"[[:space:]]*:[[:space:]]*0' "scoped ingest preflight did not count unreferenced episodes"

EMPTY_AFTER_PREFLIGHT_OUTPUT="$WORK_DIR/empty-after-preflight.json"
"$NAHUALI_BIN" --database "$STORE" validate --json >"$EMPTY_AFTER_PREFLIGHT_OUTPUT"
require_pattern "$EMPTY_AFTER_PREFLIGHT_OUTPUT" '"event_count"[[:space:]]*:[[:space:]]*0' "scoped ingest preflight wrote records"

"$NAHUALI_BIN" --database "$STORE" remember \
  "Lena owns the release notes and keeps the changelog concise." \
  --scope project:Nahuali \
  --tag product \
  --mention Lena \
  --mention "Release Notes" >/dev/null
"$NAHUALI_BIN" --database "$STORE" claim \
  Lena owns "release notes" \
  --scope project:Nahuali \
  --confidence 0.92 \
  --source-last >/dev/null
"$NAHUALI_BIN" --database "$STORE" claim \
  Lena drafts "release notes" \
  --scope project:Nahuali \
  --confidence 0.72 >/dev/null
"$NAHUALI_BIN" --database "$STORE" link \
  Lena owns "Release Notes" \
  --scope project:Nahuali \
  --confidence 0.9 \
  --source-last >/dev/null
"$NAHUALI_BIN" --database "$STORE" preference \
  "Release notes" \
  "Keep release notes concise and evidence-backed." \
  --scope project:Nahuali \
  --source-last >/dev/null
"$NAHUALI_BIN" --database "$STORE" intention \
  "Ship release notes" \
  --scope project:Nahuali \
  --priority high \
  --source-last >/dev/null

VALIDATE_OUTPUT="$WORK_DIR/validate.json"
"$NAHUALI_BIN" --database "$STORE" validate --json >"$VALIDATE_OUTPUT"
require_pattern "$VALIDATE_OUTPUT" '"valid"[[:space:]]*:[[:space:]]*true' "daily store validation failed"
require_pattern "$VALIDATE_OUTPUT" '"event_count"[[:space:]]*:[[:space:]]*6' "daily store did not contain six events"

STATUS_OUTPUT="$WORK_DIR/status.json"
"$NAHUALI_BIN" --database "$STORE" status --json >"$STATUS_OUTPUT"
require_pattern "$STATUS_OUTPUT" '"event_count"[[:space:]]*:[[:space:]]*6' "status did not report six events"
require_pattern "$STATUS_OUTPUT" '"semantic_index_role"[[:space:]]*:[[:space:]]*"derived"' "status did not report derived semantic index role"
require_pattern "$STATUS_OUTPUT" '"surrealdb_graph_projection"' "status did not include graph projection validation"
require_pattern "$STATUS_OUTPUT" '"valid"[[:space:]]*:[[:space:]]*true' "status did not report a valid graph projection"

PROJECTION_STATUS_OUTPUT="$WORK_DIR/projection-status.json"
"$NAHUALI_BIN" --database "$STORE" projection-status --json >"$PROJECTION_STATUS_OUTPUT"
require_pattern "$PROJECTION_STATUS_OUTPUT" '"projection_role"[[:space:]]*:[[:space:]]*"derived_from_memory_record"' "projection status did not report derived role"
require_pattern "$PROJECTION_STATUS_OUTPUT" '"ledger_event_count"[[:space:]]*:[[:space:]]*6' "projection status did not report six ledger events"
require_pattern "$PROJECTION_STATUS_OUTPUT" '"in_sync"[[:space:]]*:[[:space:]]*true' "projection status was not in sync"

PROJECTION_VALIDATE_OUTPUT="$WORK_DIR/projection-validate.json"
"$NAHUALI_BIN" --database "$STORE" projection-validate --json >"$PROJECTION_VALIDATE_OUTPUT"
require_pattern "$PROJECTION_VALIDATE_OUTPUT" '"valid"[[:space:]]*:[[:space:]]*true' "projection validation failed"
require_pattern "$PROJECTION_VALIDATE_OUTPUT" '"in_sync"[[:space:]]*:[[:space:]]*true' "projection validation was not in sync"

PROJECTION_REBUILD_OUTPUT="$WORK_DIR/projection-rebuild.json"
"$NAHUALI_BIN" --database "$STORE" projection-rebuild --json >"$PROJECTION_REBUILD_OUTPUT"
require_pattern "$PROJECTION_REBUILD_OUTPUT" '"projection_role"[[:space:]]*:[[:space:]]*"derived_from_memory_record"' "projection rebuild did not report derived role"
require_pattern "$PROJECTION_REBUILD_OUTPUT" '"in_sync"[[:space:]]*:[[:space:]]*true' "projection rebuild did not leave the projection in sync"

BRIEFING_OUTPUT="$WORK_DIR/briefing.json"
"$NAHUALI_BIN" --database "$STORE" briefing --json >"$BRIEFING_OUTPUT"
require_pattern "$BRIEFING_OUTPUT" '"event_count"[[:space:]]*:[[:space:]]*6' "briefing did not report six events"
require_pattern "$BRIEFING_OUTPUT" '"active_intentions"' "briefing did not include active intentions"

SESSION_RESUME_OUTPUT="$WORK_DIR/session-resume.json"
"$NAHUALI_BIN" --database "$STORE" session-resume --json >"$SESSION_RESUME_OUTPUT"
require_pattern "$SESSION_RESUME_OUTPUT" '"source_projection"[[:space:]]*:[[:space:]]*"rust"' "session resume did not report Rust projection"
require_pattern "$SESSION_RESUME_OUTPUT" '"event_count"[[:space:]]*:[[:space:]]*6' "session resume did not report six events"
require_pattern "$SESSION_RESUME_OUTPUT" '"active_intentions"' "session resume did not include active intentions"

HOOK_START_OUTPUT="$WORK_DIR/hook-session-start.json"
"$NAHUALI_BIN" --database "$STORE" hook session-start --json >"$HOOK_START_OUTPUT"
require_pattern "$HOOK_START_OUTPUT" '"kind"[[:space:]]*:[[:space:]]*"session_start"' "session-start hook did not report its kind"
require_pattern "$HOOK_START_OUTPUT" '"event_count"[[:space:]]*:[[:space:]]*6' "session-start hook did not report six events"

HOOK_PRE_PROMPT_OUTPUT="$WORK_DIR/hook-pre-prompt.json"
"$NAHUALI_BIN" --database "$STORE" hook pre-prompt \
  --input "Who owns release notes?" \
  --json >"$HOOK_PRE_PROMPT_OUTPUT"
require_pattern "$HOOK_PRE_PROMPT_OUTPUT" '"kind"[[:space:]]*:[[:space:]]*"pre_prompt"' "pre-prompt hook did not report its kind"
require_pattern "$HOOK_PRE_PROMPT_OUTPUT" '"recall"' "pre-prompt hook did not include recall context"

HOOK_POST_ACTION_OUTPUT="$WORK_DIR/hook-post-action.json"
"$NAHUALI_BIN" --database "$STORE" hook post-action \
  --input "Checked release note ownership." \
  --json >"$HOOK_POST_ACTION_OUTPUT"
require_pattern "$HOOK_POST_ACTION_OUTPUT" '"kind"[[:space:]]*:[[:space:]]*"post_action"' "post-action hook did not report its kind"

HOOK_SLEEP_OUTPUT="$WORK_DIR/hook-sleep-cycle.json"
"$NAHUALI_BIN" --database "$STORE" hook sleep-cycle --json >"$HOOK_SLEEP_OUTPUT"
require_pattern "$HOOK_SLEEP_OUTPUT" '"kind"[[:space:]]*:[[:space:]]*"sleep_cycle"' "sleep-cycle hook did not report its kind"
require_pattern "$HOOK_SLEEP_OUTPUT" '"automatic_write_back"[[:space:]]*:[[:space:]]*false' "sleep-cycle hook must remain non-mutating"

HOOK_CLOSE_OUTPUT="$WORK_DIR/hook-session-close.json"
"$NAHUALI_BIN" --database "$STORE" hook session-close --json >"$HOOK_CLOSE_OUTPUT"
require_pattern "$HOOK_CLOSE_OUTPUT" '"kind"[[:space:]]*:[[:space:]]*"session_close"' "session-close hook did not report its kind"

RECALL_OUTPUT="$WORK_DIR/recall.json"
"$NAHUALI_BIN" --database "$STORE" recall \
  "release notes" \
  --scope project:Nahuali \
  --kind claim \
  --require-evidence \
  --authority \
  --json >"$RECALL_OUTPUT"
require_pattern "$RECALL_OUTPUT" 'release notes' "scoped recall did not return synthetic content"
require_pattern "$RECALL_OUTPUT" '"kind"[[:space:]]*:[[:space:]]*"claim"' "scoped recall did not filter by claim kind"
require_pattern "$RECALL_OUTPUT" '"evidence_id"[[:space:]]*:[[:space:]]*"episode_' "scoped recall did not require evidence"
require_pattern "$RECALL_OUTPUT" '"key"[[:space:]]*:[[:space:]]*"project:nahuali"' "scoped recall did not preserve project scope"
require_pattern "$RECALL_OUTPUT" '"can_trust"' "authority recall did not include trust context"

GRAPH_OUTPUT="$WORK_DIR/graph.json"
"$NAHUALI_BIN" --database "$STORE" graph "Lena" --depth 2 --json >"$GRAPH_OUTPUT"
require_pattern "$GRAPH_OUTPUT" '"nodes"' "graph output did not include nodes"
require_pattern "$GRAPH_OUTPUT" '"edges"' "graph output did not include edges"

PROJECT_OUTPUT="$WORK_DIR/project.json"
"$NAHUALI_BIN" --database "$STORE" project "Lena" --json >"$PROJECT_OUTPUT"
require_pattern "$PROJECT_OUTPUT" '"source_projection"[[:space:]]*:[[:space:]]*"rust"' "project view did not report Rust projection"
require_pattern "$PROJECT_OUTPUT" '"matched_entity"[[:space:]]*:[[:space:]]*true' "project view did not match Lena"
require_pattern "$PROJECT_OUTPUT" '"claim_count"[[:space:]]*:[[:space:]]*2' "project view did not include both claims"
require_pattern "$PROJECT_OUTPUT" '"link_count"[[:space:]]*:[[:space:]]*1' "project view did not include the release link"
require_pattern "$PROJECT_OUTPUT" '"procedure_count"[[:space:]]*:[[:space:]]*1' "project view did not include the preference"
require_pattern "$PROJECT_OUTPUT" '"intention_count"[[:space:]]*:[[:space:]]*1' "project view did not include the intention"
require_pattern "$PROJECT_OUTPUT" '"review_item_count"' "project view did not include review context"

TIMELINE_OUTPUT="$WORK_DIR/timeline.json"
"$NAHUALI_BIN" --database "$STORE" timeline --json >"$TIMELINE_OUTPUT"
require_pattern "$TIMELINE_OUTPUT" '"projection_role"[[:space:]]*:[[:space:]]*"derived_from_memory_record"' "timeline did not report derived projection role"
require_pattern "$TIMELINE_OUTPUT" '"episodes"' "timeline did not include episodes"
require_pattern "$TIMELINE_OUTPUT" 'Lena owns the release notes' "timeline did not include the daily episode"

PENDING_OUTPUT="$WORK_DIR/pending.json"
"$NAHUALI_BIN" --database "$STORE" pending --json >"$PENDING_OUTPUT"
require_pattern "$PENDING_OUTPUT" '"projection_role"[[:space:]]*:[[:space:]]*"derived_from_memory_record"' "pending did not report derived projection role"
require_pattern "$PENDING_OUTPUT" '"intentions"' "pending did not include intentions"
require_pattern "$PENDING_OUTPUT" 'Ship release notes' "pending did not include the active intention"

PROACTIVE_OUTPUT="$WORK_DIR/proactive.json"
"$NAHUALI_BIN" --database "$STORE" proactive --json >"$PROACTIVE_OUTPUT"
require_pattern "$PROACTIVE_OUTPUT" '"source_projection"[[:space:]]*:[[:space:]]*"rust"' "proactive report did not report Rust projection"
require_pattern "$PROACTIVE_OUTPUT" '"automatic_write_back"[[:space:]]*:[[:space:]]*false' "proactive report must remain non-mutating"
require_pattern "$PROACTIVE_OUTPUT" '"capture_opportunities"' "proactive report did not include capture opportunities"

DEADLINES_OUTPUT="$WORK_DIR/deadlines.json"
"$NAHUALI_BIN" --database "$STORE" deadlines --json >"$DEADLINES_OUTPUT"
require_pattern "$DEADLINES_OUTPUT" '"source_projection"[[:space:]]*:[[:space:]]*"rust"' "deadline report did not report Rust projection"
require_pattern "$DEADLINES_OUTPUT" '"deadline_count"' "deadline report did not include summary counts"

ANOMALIES_OUTPUT="$WORK_DIR/anomalies.json"
"$NAHUALI_BIN" --database "$STORE" anomalies --json >"$ANOMALIES_OUTPUT"
require_pattern "$ANOMALIES_OUTPUT" '"source_projection"[[:space:]]*:[[:space:]]*"rust"' "anomaly report did not report Rust projection"
require_pattern "$ANOMALIES_OUTPUT" '"unsupported_memory"' "anomaly report did not include the unsupported claim alert"

SEMANTIC_REBUILD_OUTPUT="$WORK_DIR/semantic-rebuild.json"
"$NAHUALI_BIN" --database "$STORE" semantic-rebuild --json >"$SEMANTIC_REBUILD_OUTPUT"
require_pattern "$SEMANTIC_REBUILD_OUTPUT" '"source_event_count"[[:space:]]*:[[:space:]]*6' "semantic rebuild did not index the daily store"
require_pattern "$SEMANTIC_REBUILD_OUTPUT" '"indexed_point_count"' "semantic rebuild did not report indexed points"

SEMANTIC_STATUS_OUTPUT="$WORK_DIR/semantic-status.json"
"$NAHUALI_BIN" --database "$STORE" semantic-status --json >"$SEMANTIC_STATUS_OUTPUT"
require_pattern "$SEMANTIC_STATUS_OUTPUT" '"collection_exists"[[:space:]]*:[[:space:]]*true' "semantic status did not find the rebuilt collection"
require_pattern "$SEMANTIC_STATUS_OUTPUT" '"is_current"[[:space:]]*:[[:space:]]*true' "semantic status did not certify a current derived index"
require_pattern "$SEMANTIC_STATUS_OUTPUT" '"missing_point_count"[[:space:]]*:[[:space:]]*0' "semantic status found missing points after rebuild"
require_pattern "$SEMANTIC_STATUS_OUTPUT" '"orphan_point_count"[[:space:]]*:[[:space:]]*0' "semantic status found orphan points after rebuild"
require_pattern "$SEMANTIC_STATUS_OUTPUT" '"stale_point_count"[[:space:]]*:[[:space:]]*0' "semantic status found stale points after rebuild"

SEMANTIC_RECALL_OUTPUT="$WORK_DIR/semantic-recall.json"
"$NAHUALI_BIN" --database "$STORE" recall \
  "release notes" \
  --semantic \
  --json >"$SEMANTIC_RECALL_OUTPUT"
require_pattern "$SEMANTIC_RECALL_OUTPUT" '"semantic_results"' "semantic recall did not include semantic results"
require_pattern "$SEMANTIC_RECALL_OUTPUT" '"semantic_score"' "semantic recall did not include semantic scoring"

INSPECT_OUTPUT="$WORK_DIR/inspect.json"
"$NAHUALI_BIN" --database "$STORE" inspect --json >"$INSPECT_OUTPUT"
require_pattern "$INSPECT_OUTPUT" '"episode_count"[[:space:]]*:[[:space:]]*1' "inspect did not report one episode"
require_pattern "$INSPECT_OUTPUT" '"supported_fact_count"[[:space:]]*:[[:space:]]*1' "inspect did not report the supported claim"
require_pattern "$INSPECT_OUTPUT" '"unsupported_fact_count"[[:space:]]*:[[:space:]]*1' "inspect did not report the unsupported claim"

SELF_INSPECT_OUTPUT="$WORK_DIR/self-inspect.json"
"$NAHUALI_BIN" --database "$STORE" self-inspect --json >"$SELF_INSPECT_OUTPUT"
require_pattern "$SELF_INSPECT_OUTPUT" '"automatic_write_back"[[:space:]]*:[[:space:]]*false' "self-inspection must remain non-mutating"
require_pattern "$SELF_INSPECT_OUTPUT" '"requires_operator_review"[[:space:]]*:[[:space:]]*true' "self-inspection must require operator review"
require_pattern "$SELF_INSPECT_OUTPUT" '"source_coverage_count"[[:space:]]*:[[:space:]]*1' "self-inspection did not report the source coverage finding"
require_pattern "$SELF_INSPECT_OUTPUT" '"kind"[[:space:]]*:[[:space:]]*"source_coverage"' "self-inspection did not expose the source coverage finding kind"

SLEEP_OUTPUT="$WORK_DIR/sleep.json"
"$NAHUALI_BIN" --database "$STORE" sleep --json >"$SLEEP_OUTPUT"
require_pattern "$SLEEP_OUTPUT" '"automatic_write_back"[[:space:]]*:[[:space:]]*false' "sleep mode must remain non-mutating"
require_pattern "$SLEEP_OUTPUT" '"consolidation_candidates"' "sleep mode did not include consolidation candidates"

PLAN_OUTPUT="$WORK_DIR/consolidation-plan.json"
"$NAHUALI_BIN" --database "$STORE" consolidation-plan --json >"$PLAN_OUTPUT"
require_pattern "$PLAN_OUTPUT" '"operations"' "consolidation plan did not include operations"
require_pattern "$PLAN_OUTPUT" '"automatic_write_back"[[:space:]]*:[[:space:]]*false' "consolidation plan must remain non-mutating"

REFLECT_OUTPUT="$WORK_DIR/reflect.json"
"$NAHUALI_BIN" --database "$STORE" reflect --json >"$REFLECT_OUTPUT"
require_pattern "$REFLECT_OUTPUT" '"automatic_write_back"[[:space:]]*:[[:space:]]*false' "reflection must remain non-mutating"

REVIEW_OUTPUT="$WORK_DIR/review.json"
"$NAHUALI_BIN" --database "$STORE" review --action capture-evidence --json >"$REVIEW_OUTPUT"
require_pattern "$REVIEW_OUTPUT" '"items"' "review output did not include items"
require_pattern "$REVIEW_OUTPUT" '"action"[[:space:]]*:[[:space:]]*"capture_evidence"' "review output did not filter by action"

BACKUP_DRY_RUN_OUTPUT="$WORK_DIR/backup-dry-run.json"
"$NAHUALI_BIN" --database "$STORE" backup --output "$BACKUP" --dry-run --json >"$BACKUP_DRY_RUN_OUTPUT"
require_pattern "$BACKUP_DRY_RUN_OUTPUT" '"dry_run"[[:space:]]*:[[:space:]]*true' "backup dry-run flag was not reported"
require_pattern "$BACKUP_DRY_RUN_OUTPUT" '"written"[[:space:]]*:[[:space:]]*false' "backup dry-run wrote output"

BACKUP_OUTPUT="$WORK_DIR/backup.json"
"$NAHUALI_BIN" --database "$STORE" backup --output "$BACKUP" --json >"$BACKUP_OUTPUT"
test -s "$BACKUP"
require_pattern "$BACKUP_OUTPUT" '"written"[[:space:]]*:[[:space:]]*true' "backup was not written"
require_pattern "$BACKUP_OUTPUT" '"record_count"[[:space:]]*:[[:space:]]*6' "backup did not include six records"

BACKUP_VALIDATE_OUTPUT="$WORK_DIR/backup-validate.json"
"$NAHUALI_BIN" backup-validate "$BACKUP" --json >"$BACKUP_VALIDATE_OUTPUT"
require_pattern "$BACKUP_VALIDATE_OUTPUT" '"valid"[[:space:]]*:[[:space:]]*true' "backup validation failed"

BACKUP_DRILL_OUTPUT="$WORK_DIR/backup-drill.json"
"$NAHUALI_BIN" backup-drill "$BACKUP" --target-database "$DRILL_STORE" --json >"$BACKUP_DRILL_OUTPUT"
require_pattern "$BACKUP_DRILL_OUTPUT" '"valid"[[:space:]]*:[[:space:]]*true' "backup drill failed"
require_pattern "$BACKUP_DRILL_OUTPUT" '"target_was_empty"[[:space:]]*:[[:space:]]*true' "backup drill did not use an empty target"

RESTORE_DRY_RUN_OUTPUT="$WORK_DIR/restore-dry-run.json"
"$NAHUALI_BIN" restore "$BACKUP" --target-database "$RESTORE_STORE" --dry-run --json >"$RESTORE_DRY_RUN_OUTPUT"
require_pattern "$RESTORE_DRY_RUN_OUTPUT" '"valid"[[:space:]]*:[[:space:]]*true' "restore dry-run failed"
require_pattern "$RESTORE_DRY_RUN_OUTPUT" '"restored_event_count"[[:space:]]*:[[:space:]]*0' "restore dry-run wrote records"

RESTORE_OUTPUT="$WORK_DIR/restore.json"
"$NAHUALI_BIN" restore "$BACKUP" --target-database "$RESTORE_STORE" --json >"$RESTORE_OUTPUT"
require_pattern "$RESTORE_OUTPUT" '"valid"[[:space:]]*:[[:space:]]*true' "restore failed"
require_pattern "$RESTORE_OUTPUT" '"restored_event_count"[[:space:]]*:[[:space:]]*6' "restore did not write six records"

RESTORED_VALIDATE_OUTPUT="$WORK_DIR/restored-validate.json"
"$NAHUALI_BIN" --database "$RESTORE_STORE" validate --json >"$RESTORED_VALIDATE_OUTPUT"
require_pattern "$RESTORED_VALIDATE_OUTPUT" '"valid"[[:space:]]*:[[:space:]]*true' "restored store validation failed"
require_pattern "$RESTORED_VALIDATE_OUTPUT" '"event_count"[[:space:]]*:[[:space:]]*6' "restored store did not preserve six events"

RESTORED_SEMANTIC_OUTPUT="$WORK_DIR/restored-semantic-rebuild.json"
"$NAHUALI_BIN" --database "$RESTORE_STORE" semantic-rebuild --json >"$RESTORED_SEMANTIC_OUTPUT"
require_pattern "$RESTORED_SEMANTIC_OUTPUT" '"source_event_count"[[:space:]]*:[[:space:]]*6' "restored semantic rebuild did not index six events"

GLOBAL_NAHUALI_AFTER="$(command -v nahuali || true)"
if [[ "$GLOBAL_NAHUALI_AFTER" != "$GLOBAL_NAHUALI_BEFORE" ]]; then
  echo "global nahuali command changed during daily dogfood verification" >&2
  echo "before: ${GLOBAL_NAHUALI_BEFORE:-<missing>}" >&2
  echo "after: ${GLOBAL_NAHUALI_AFTER:-<missing>}" >&2
  exit 1
fi

echo "dogfood daily workflow check passed"
