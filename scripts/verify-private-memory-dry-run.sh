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

require_json() {
  local file="$1"
  local query="$2"
  local expected="$3"
  local message="$4"

  local actual
  actual="$(jq -c "$query" "$file")"
  if [[ "$actual" != "$expected" ]]; then
    echo "$message" >&2
    echo "expected: $expected" >&2
    echo "actual:   $actual" >&2
    echo "--- $file ---" >&2
    cat "$file" >&2
    exit 1
  fi
}

require_text() {
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

LEGACY_EXPORT="$WORK_DIR/private-export.json"
OUTPUT_DIR="$WORK_DIR/output"
RUN_ID="synthetic_private_dry_run"
RUN_TOKEN="$(basename "$WORK_DIR" | tr -cd '[:alnum:]')"

cat >"$LEGACY_EXPORT" <<'JSON'
{
  "exportedAt": "2026-04-23T00:00:00.000Z",
  "data": {
    "entity": [
      {
        "id": "entity:lena",
        "name": "Lena",
        "type": "person",
        "createdAt": "2026-04-23T08:45:00.000Z",
        "aliases": ["L."],
        "attributes": {
          "role": "release owner",
          "team": "product"
        }
      }
    ],
    "episode": [
      {
        "id": "episode:release",
        "title": "Lena owns the release notes.",
        "body": "Release notes should stay concise and cite evidence.",
        "entityNames": ["Lena"],
        "tags": "product",
        "timestamp": "2026-04-23T08:45:30.125Z",
        "source": "conversation:release-review",
        "sourcePosition": 1,
        "operator": "release-chair"
      }
    ],
    "relates_to": [
      {
        "id": "relates_to:release",
        "fromEntity": "Lena",
        "toEntity": {"label": "Release Notes"},
        "relationType": "custom",
        "customType": "owns",
        "confidence": 0.91,
        "createdAt": "2026-04-23T08:45:00.000Z"
      }
    ],
    "procedure": [
      {
        "id": "procedure:release_notes",
        "name": "Release note style",
        "category": "writing",
        "description": "Keep release notes concise.",
        "rules": ["Cite evidence for shipped behavior."],
        "antiPatterns": ["Do not overpromise."],
        "priority": 80,
        "entityScope": ["Release Notes"],
        "contextScope": ["launch"],
        "createdAt": "2026-04-23T08:45:00.000Z"
      }
    ],
    "intention": [
      {
        "id": "intention:ship_release",
        "description": "Ship release notes",
        "type": "deadline",
        "status": "done",
        "importance": 0.95,
        "context": "Release readiness",
        "targetDate": "2026-04-24T09:00:00.000Z",
        "createdAt": "2026-04-23T08:45:00.000Z",
        "completedAt": "2026-04-24T09:00:00.000Z",
        "notes": ["Waiting for the final changelog."],
        "tags": ["release"],
        "entityNames": ["Lena"]
      }
    ]
  }
}
JSON

PRIVATE_DRY_RUN_OUTPUT="$WORK_DIR/private-memory-dry-run.stdout"
scripts/private-memory-dry-run.sh \
  --input "$LEGACY_EXPORT" \
  --input-kind legacy \
  --output-dir "$OUTPUT_DIR" \
  --scope project:Nahuali \
  --run-id "$RUN_ID" \
  --skip-gates >"$PRIVATE_DRY_RUN_OUTPUT"

SUMMARY="$OUTPUT_DIR/summary.txt"
SUMMARY_JSON="$OUTPUT_DIR/summary.json"
test -s "$SUMMARY"
test -s "$SUMMARY_JSON"
test -s "$OUTPUT_DIR/import-dry-run.json"
test -s "$OUTPUT_DIR/private-memory.interchange.json"

require_text "$PRIVATE_DRY_RUN_OUTPUT" 'private memory dry-run completed' "dry-run wrapper did not complete"
require_text "$SUMMARY" 'Input content copied: no' "summary did not record no input copy"
require_text "$SUMMARY" 'Summary JSON:' "summary did not point to the JSON artifact"

require_json "$SUMMARY_JSON" '.run_id' '"synthetic_private_dry_run"' "summary JSON did not record run id"
require_json "$SUMMARY_JSON" '.input_kind' '"legacy"' "summary JSON did not record input kind"
require_json "$SUMMARY_JSON" '.input_copied' 'false' "summary JSON must record that raw input was not copied"
require_json "$SUMMARY_JSON" '.synthetic_gates_before_run' '"skipped"' "summary JSON did not record skipped synthetic gates"
require_json "$SUMMARY_JSON" '.dry_run.valid' 'true' "summary JSON did not record a valid dry-run"
require_json "$SUMMARY_JSON" '.dry_run.imported_event_count' '0' "dry-run must remain non-mutating"
require_json "$SUMMARY_JSON" '.dry_run.appendable_event_count' '10' "summary JSON did not record appendable event count"
require_json "$SUMMARY_JSON" '.counts.sources' '1' "summary JSON did not record source count"
require_json "$SUMMARY_JSON" '.counts.episodes' '1' "summary JSON did not record episode count"
require_json "$SUMMARY_JSON" '.counts.claims' '4' "summary JSON did not record claim count"
require_json "$SUMMARY_JSON" '.counts.links' '1' "summary JSON did not record link count"
require_json "$SUMMARY_JSON" '.counts.procedures' '1' "summary JSON did not record procedure count"
require_json "$SUMMARY_JSON" '.counts.intentions' '1' "summary JSON did not record intention count"
require_json "$SUMMARY_JSON" '.conversion.issue_count' '0' "summary JSON did not record conversion issue count"
require_json "$SUMMARY_JSON" '.preflight.evidence_gap_count' '2' "summary JSON did not record evidence gaps"
require_json "$SUMMARY_JSON" '.preflight.unsourced_episode_count' '0' "summary JSON did not record sourced episode safety"
require_json "$SUMMARY_JSON" '.preflight.unscoped_record_count' '0' "summary JSON did not record scoped record safety"
require_json "$SUMMARY_JSON" '.preflight.scope_keys' '["project:nahuali"]' "summary JSON did not record scope keys"
require_json "$SUMMARY_JSON" '.readiness.review_item_count' '5' "summary JSON did not record review item pressure"
require_json "$SUMMARY_JSON" '.readiness.source_coverage_count' '1' "summary JSON did not record source coverage pressure"
require_json "$SUMMARY_JSON" '.readiness.automatic_write_back' 'false' "summary JSON must keep readiness non-mutating"
require_json "$SUMMARY_JSON" '.isolated_apply.ran' 'false' "summary JSON should not report isolated apply without --apply"
require_json "$SUMMARY_JSON" '.cutover_recommendation' '"no"' "summary JSON must default to no cutover"

APPLY_OUTPUT_DIR="$WORK_DIR/apply-output"
APPLY_RUN_ID="synthetic_private_apply_${RUN_TOKEN}"
PRIVATE_APPLY_OUTPUT="$WORK_DIR/private-memory-apply.stdout"
scripts/private-memory-dry-run.sh \
  --input "$LEGACY_EXPORT" \
  --input-kind legacy \
  --output-dir "$APPLY_OUTPUT_DIR" \
  --scope project:Nahuali \
  --run-id "$APPLY_RUN_ID" \
  --skip-gates \
  --apply >"$PRIVATE_APPLY_OUTPUT"

APPLY_SUMMARY="$APPLY_OUTPUT_DIR/summary.txt"
APPLY_SUMMARY_JSON="$APPLY_OUTPUT_DIR/summary.json"
test -s "$APPLY_SUMMARY"
test -s "$APPLY_SUMMARY_JSON"
test -s "$APPLY_OUTPUT_DIR/import.json"
test -s "$APPLY_OUTPUT_DIR/validate.json"
test -s "$APPLY_OUTPUT_DIR/projection-validate.json"
test -s "$APPLY_OUTPUT_DIR/semantic-rebuild.json"
test -s "$APPLY_OUTPUT_DIR/backup-validate.json"
test -s "$APPLY_OUTPUT_DIR/backup-drill.json"
test -s "$APPLY_OUTPUT_DIR/restore-dry-run.json"

require_text "$PRIVATE_APPLY_OUTPUT" 'private memory dry-run completed' "apply wrapper did not complete"
require_text "$APPLY_SUMMARY" 'Isolated apply: yes' "apply summary did not record isolated apply"
require_text "$APPLY_SUMMARY" 'Validate valid: true' "apply summary did not record validation success"
require_text "$APPLY_SUMMARY" 'Projection valid: true' "apply summary did not record projection success"
require_text "$APPLY_SUMMARY" 'Semantic rebuild source count: 10' "apply summary did not record semantic rebuild source count"
require_text "$APPLY_SUMMARY" 'Backup valid: true' "apply summary did not record backup validation"
require_text "$APPLY_SUMMARY" 'Backup drill valid: true' "apply summary did not record backup drill validation"
require_text "$APPLY_SUMMARY" 'Restore dry-run events: 0' "apply summary did not record restore dry-run safety"

require_json "$APPLY_SUMMARY_JSON" '.run_id' "\"$APPLY_RUN_ID\"" "apply summary JSON did not record run id"
require_json "$APPLY_SUMMARY_JSON" '.input_copied' 'false' "apply summary JSON must record no input copy"
require_json "$APPLY_SUMMARY_JSON" '.dry_run.valid' 'true' "apply summary JSON did not preserve dry-run validity"
require_json "$APPLY_SUMMARY_JSON" '.dry_run.imported_event_count' '0' "apply dry-run must remain non-mutating before isolated apply"
require_json "$APPLY_SUMMARY_JSON" '.isolated_apply.ran' 'true' "apply summary JSON did not record isolated apply"
require_json "$APPLY_SUMMARY_JSON" '.isolated_apply.validate_valid' 'true' "apply summary JSON did not record validation success"
require_json "$APPLY_SUMMARY_JSON" '.isolated_apply.projection_valid' 'true' "apply summary JSON did not record projection validation success"
require_json "$APPLY_SUMMARY_JSON" '.isolated_apply.semantic_rebuild_source_count' '10' "apply summary JSON did not record semantic rebuild source count"
require_json "$APPLY_SUMMARY_JSON" '.isolated_apply.backup_valid' 'true' "apply summary JSON did not record backup validation"
require_json "$APPLY_SUMMARY_JSON" '.isolated_apply.backup_drill_valid' 'true' "apply summary JSON did not record backup drill validation"
require_json "$APPLY_SUMMARY_JSON" '.isolated_apply.restore_dry_run_events' '0' "apply summary JSON did not record restore dry-run safety"
require_json "$APPLY_SUMMARY_JSON" '.cutover_recommendation' '"no"' "apply summary JSON must still default to no cutover"

echo "private memory dry-run summary check passed"
