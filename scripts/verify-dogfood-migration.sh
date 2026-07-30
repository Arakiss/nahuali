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

LEGACY_EXPORT="$WORK_DIR/legacy-export.json"
INTERCHANGE="$WORK_DIR/projected-memory.interchange.json"
RUN_ID="$(basename "$WORK_DIR" | tr -cd '[:alnum:]')"
STORE="dogfood_migration_${RUN_ID}_source"
RESTORE_STORE="dogfood_migration_${RUN_ID}_restore"
DRILL_STORE="dogfood_migration_${RUN_ID}_drill"
BACKUP="$WORK_DIR/projected-memory.backup.json"

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
        "emotions": ["focused"],
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
        "triggers": [{"type": "keyword", "value": "release", "weight": 1}],
        "examples": [{"input": "A shipped change", "output": "A concise note"}],
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

CONVERT_OUTPUT="$WORK_DIR/convert.json"
"$NAHUALI_BIN" convert-legacy-export "$LEGACY_EXPORT" \
  --output "$INTERCHANGE" \
  --scope project:Nahuali \
  --json >"$CONVERT_OUTPUT"
test -s "$INTERCHANGE"
require_pattern "$INTERCHANGE" '"timestamp_ms"[[:space:]]*:[[:space:]]*1776933930125' "conversion did not preserve episode timestamp"
require_pattern "$INTERCHANGE" '"source_ref"[[:space:]]*:[[:space:]]*"source:conversation_release_review"' "conversion did not preserve episode source reference"
require_pattern "$INTERCHANGE" '"source_position"[[:space:]]*:[[:space:]]*1' "conversion did not preserve episode source position"
require_pattern "$INTERCHANGE" '"source_role"[[:space:]]*:[[:space:]]*"release-chair"' "conversion did not preserve episode source role"
require_pattern "$INTERCHANGE" '"status_timestamp_ms"[[:space:]]*:[[:space:]]*1777021200000' "conversion did not preserve intention status timestamp"
require_pattern "$CONVERT_OUTPUT" '"source_count"[[:space:]]*:[[:space:]]*1' "conversion did not produce one source"
require_pattern "$CONVERT_OUTPUT" '"episode_count"[[:space:]]*:[[:space:]]*1' "conversion did not produce one episode"
require_pattern "$CONVERT_OUTPUT" '"claim_count"[[:space:]]*:[[:space:]]*4' "conversion did not produce four claims"
require_pattern "$CONVERT_OUTPUT" '"link_count"[[:space:]]*:[[:space:]]*1' "conversion did not produce one link"
require_pattern "$CONVERT_OUTPUT" '"procedure_count"[[:space:]]*:[[:space:]]*1' "conversion did not produce one procedure"
require_pattern "$CONVERT_OUTPUT" '"intention_count"[[:space:]]*:[[:space:]]*1' "conversion did not produce one intention"
require_pattern "$CONVERT_OUTPUT" '"issue_count"[[:space:]]*:[[:space:]]*0' "conversion reported issues"

DRY_RUN_OUTPUT="$WORK_DIR/import-dry-run.json"
"$NAHUALI_BIN" --database "$STORE" import "$INTERCHANGE" --dry-run --json >"$DRY_RUN_OUTPUT"
require_pattern "$DRY_RUN_OUTPUT" '"valid"[[:space:]]*:[[:space:]]*true' "import dry-run was not valid"
require_pattern "$DRY_RUN_OUTPUT" '"dry_run"[[:space:]]*:[[:space:]]*true' "import dry-run flag was not reported"
require_pattern "$DRY_RUN_OUTPUT" '"appendable_event_count"[[:space:]]*:[[:space:]]*10' "import dry-run did not plan ten events"
require_pattern "$DRY_RUN_OUTPUT" '"imported_event_count"[[:space:]]*:[[:space:]]*0' "import dry-run mutated the store"
require_pattern "$DRY_RUN_OUTPUT" '"source_count"[[:space:]]*:[[:space:]]*1' "import preflight did not count sources"
require_pattern "$DRY_RUN_OUTPUT" '"sourced_episode_count"[[:space:]]*:[[:space:]]*1' "import preflight did not count sourced episodes"
require_pattern "$DRY_RUN_OUTPUT" '"unsourced_episode_count"[[:space:]]*:[[:space:]]*0' "import preflight reported unsourced episodes"
require_pattern "$DRY_RUN_OUTPUT" '"derived_record_count"[[:space:]]*:[[:space:]]*7' "import preflight did not count derived records"
require_pattern "$DRY_RUN_OUTPUT" '"evidence_linked_record_count"[[:space:]]*:[[:space:]]*5' "import preflight did not count evidence-linked records"
require_pattern "$DRY_RUN_OUTPUT" '"evidence_gap_count"[[:space:]]*:[[:space:]]*2' "import preflight did not report expected evidence gaps"
require_pattern "$DRY_RUN_OUTPUT" '"referenced_episode_count"[[:space:]]*:[[:space:]]*1' "import preflight did not count referenced episodes"
require_pattern "$DRY_RUN_OUTPUT" '"unreferenced_episode_count"[[:space:]]*:[[:space:]]*0' "import preflight reported unreferenced episodes"
require_pattern "$DRY_RUN_OUTPUT" '"scoped_record_count"[[:space:]]*:[[:space:]]*9' "import preflight did not count scoped records"
require_pattern "$DRY_RUN_OUTPUT" '"unscoped_record_count"[[:space:]]*:[[:space:]]*0' "import preflight reported unscoped records"
require_pattern "$DRY_RUN_OUTPUT" '"readiness"' "import dry-run did not include migration readiness forecast"
require_pattern "$DRY_RUN_OUTPUT" '"source_coverage_count"[[:space:]]*:[[:space:]]*1' "import dry-run did not forecast source coverage review pressure"
require_pattern "$DRY_RUN_OUTPUT" '"review_item_count"[[:space:]]*:[[:space:]]*1' "import dry-run did not forecast review item count"
require_pattern "$DRY_RUN_OUTPUT" '"project:nahuali"' "import preflight did not preserve project scope key"

IMPORT_OUTPUT="$WORK_DIR/import.json"
"$NAHUALI_BIN" --database "$STORE" import "$INTERCHANGE" --json >"$IMPORT_OUTPUT"
require_pattern "$IMPORT_OUTPUT" '"valid"[[:space:]]*:[[:space:]]*true' "import was not valid"
require_pattern "$IMPORT_OUTPUT" '"imported_event_count"[[:space:]]*:[[:space:]]*10' "import did not write ten events"

POST_IMPORT_EXPORT="$WORK_DIR/post-import.interchange.json"
"$NAHUALI_BIN" --database "$STORE" export --output "$POST_IMPORT_EXPORT" --json >"$WORK_DIR/post-import-export.json"
require_pattern "$POST_IMPORT_EXPORT" '"timestamp_ms"[[:space:]]*:[[:space:]]*1776933930125' "import did not preserve episode timestamp"
require_pattern "$POST_IMPORT_EXPORT" '"source_ref"[[:space:]]*:[[:space:]]*"source_' "import did not preserve episode source reference"
require_pattern "$POST_IMPORT_EXPORT" '"source_position"[[:space:]]*:[[:space:]]*1' "import did not preserve episode source position"
require_pattern "$POST_IMPORT_EXPORT" '"source_role"[[:space:]]*:[[:space:]]*"release-chair"' "import did not preserve episode source role"
require_pattern "$POST_IMPORT_EXPORT" '"status_timestamp_ms"[[:space:]]*:[[:space:]]*1777021200000' "import did not preserve intention status timestamp"

RECALL_OUTPUT="$WORK_DIR/recall.json"
"$NAHUALI_BIN" --database "$STORE" recall "release notes" --scope project:Nahuali --json >"$RECALL_OUTPUT"
require_pattern "$RECALL_OUTPUT" 'release notes' "scoped recall did not return the migrated content"
require_pattern "$RECALL_OUTPUT" '"key"[[:space:]]*:[[:space:]]*"project:nahuali"' "scoped recall did not preserve project scope"

INSPECT_OUTPUT="$WORK_DIR/inspect.json"
"$NAHUALI_BIN" --database "$STORE" inspect --json >"$INSPECT_OUTPUT"
require_pattern "$INSPECT_OUTPUT" '"episode_count"[[:space:]]*:[[:space:]]*1' "inspect did not report one episode"
require_pattern "$INSPECT_OUTPUT" '"supported_fact_count"[[:space:]]*:[[:space:]]*4' "inspect did not preserve supported claims"

SELF_INSPECT_OUTPUT="$WORK_DIR/self-inspect.json"
"$NAHUALI_BIN" --database "$STORE" self-inspect --json >"$SELF_INSPECT_OUTPUT"
require_pattern "$SELF_INSPECT_OUTPUT" '"automatic_write_back"[[:space:]]*:[[:space:]]*false' "self-inspection must remain non-mutating"
require_pattern "$SELF_INSPECT_OUTPUT" '"source_coverage_count"[[:space:]]*:[[:space:]]*1' "self-inspection did not report remaining derived evidence gaps"
require_pattern "$SELF_INSPECT_OUTPUT" '0 episode\(s\) lack source records and 2 derived memory item\(s\) lack source episode evidence' "self-inspection did not keep migrated episodes source-covered while surfacing derived gaps"

BACKUP_DRY_RUN_OUTPUT="$WORK_DIR/backup-dry-run.json"
"$NAHUALI_BIN" --database "$STORE" backup --output "$BACKUP" --dry-run --json >"$BACKUP_DRY_RUN_OUTPUT"
require_pattern "$BACKUP_DRY_RUN_OUTPUT" '"dry_run"[[:space:]]*:[[:space:]]*true' "backup dry-run flag was not reported"
require_pattern "$BACKUP_DRY_RUN_OUTPUT" '"written"[[:space:]]*:[[:space:]]*false' "backup dry-run wrote output"

BACKUP_OUTPUT="$WORK_DIR/backup.json"
"$NAHUALI_BIN" --database "$STORE" backup --output "$BACKUP" --json >"$BACKUP_OUTPUT"
test -s "$BACKUP"
require_pattern "$BACKUP_OUTPUT" '"written"[[:space:]]*:[[:space:]]*true' "backup was not written"
require_pattern "$BACKUP_OUTPUT" '"record_count"[[:space:]]*:[[:space:]]*10' "backup did not include ten records"

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
require_pattern "$RESTORE_OUTPUT" '"restored_event_count"[[:space:]]*:[[:space:]]*10' "restore did not write ten records"

RESTORED_VALIDATE_OUTPUT="$WORK_DIR/restored-validate.json"
"$NAHUALI_BIN" --database "$RESTORE_STORE" validate --json >"$RESTORED_VALIDATE_OUTPUT"
require_pattern "$RESTORED_VALIDATE_OUTPUT" '"valid"[[:space:]]*:[[:space:]]*true' "restored store validation failed"
require_pattern "$RESTORED_VALIDATE_OUTPUT" '"event_count"[[:space:]]*:[[:space:]]*10' "restored store did not preserve ten events"

GLOBAL_NAHUALI_AFTER="$(command -v nahuali || true)"
if [[ "$GLOBAL_NAHUALI_AFTER" != "$GLOBAL_NAHUALI_BEFORE" ]]; then
  echo "global nahuali command changed during dogfood migration verification" >&2
  echo "before: ${GLOBAL_NAHUALI_BEFORE:-<missing>}" >&2
  echo "after: ${GLOBAL_NAHUALI_AFTER:-<missing>}" >&2
  exit 1
fi

echo "dogfood migration check passed"
