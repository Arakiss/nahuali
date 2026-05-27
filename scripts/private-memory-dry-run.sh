#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

usage() {
  cat <<'USAGE'
Usage:
  scripts/private-memory-dry-run.sh --input <path> [options]

Options:
  --input <path>          Private export or interchange file to rehearse.
  --input-kind <kind>     legacy or interchange. Default: legacy.
  --output-dir <path>     Private output directory. Default: .private/private-memory-dry-runs/<run-id>.
  --scope <scope>         Scope for legacy conversion. Default: project:Nahuali.
  --run-id <id>           Stable run id for repeatable private output paths.
  --apply                 Run an isolated throwaway apply rehearsal after the dry-run.
  --skip-gates            Skip synthetic preflight gates.
  -h, --help              Show this help.

The script never copies the original input export. Derived private artifacts are
written only to an ignored private path or to a directory outside the repository.
USAGE
}

abs_path() {
  local path="$1"
  local dir
  local base

  dir="$(dirname "$path")"
  base="$(basename "$path")"
  if [[ ! -d "$dir" ]]; then
    return 1
  fi
  dir="$(cd "$dir" && pwd -P)"
  printf '%s/%s\n' "$dir" "$base"
}

root_relative_path() {
  local path="$1"
  case "$path" in
    /*) printf '%s\n' "$path" ;;
    *) printf '%s/%s\n' "$ROOT" "$path" ;;
  esac
}

refuse_public_repo_path() {
  local path="$1"
  local label="$2"
  local abs
  abs="$(abs_path "$path")"

  case "$abs" in
    "$ROOT"/.private/*|"$ROOT"/.private) ;;
    "$ROOT"/*)
      echo "$label must be under .private/ or outside this repository: $path" >&2
      exit 1
      ;;
    *) ;;
  esac
}

jq_field() {
  local file="$1"
  local query="$2"
  local fallback="$3"

  if [[ ! -s "$file" ]]; then
    printf '%s\n' "$fallback"
    return
  fi
  jq -r "$query // \"$fallback\"" "$file" 2>/dev/null || printf '%s\n' "$fallback"
}

jq_json_field() {
  local file="$1"
  local query="$2"
  local fallback="$3"

  if [[ ! -s "$file" ]]; then
    printf '%s\n' "$fallback"
    return
  fi
  jq -c "($query) as \$value | if \$value == null then $fallback else \$value end" "$file" 2>/dev/null || printf '%s\n' "$fallback"
}

INPUT=""
INPUT_KIND="legacy"
SCOPE="project:Nahuali"
RUN_ID="$(date -u +%Y%m%dT%H%M%SZ)"
OUTPUT_DIR=""
APPLY=0
SKIP_GATES=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --input)
      INPUT="${2:-}"
      shift 2
      ;;
    --input-kind)
      INPUT_KIND="${2:-}"
      shift 2
      ;;
    --output-dir)
      OUTPUT_DIR="${2:-}"
      shift 2
      ;;
    --scope)
      SCOPE="${2:-}"
      shift 2
      ;;
    --run-id)
      RUN_ID="${2:-}"
      shift 2
      ;;
    --apply)
      APPLY=1
      shift
      ;;
    --skip-gates)
      SKIP_GATES=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ -z "$INPUT" ]]; then
  echo "--input is required" >&2
  usage >&2
  exit 2
fi

if [[ "$INPUT_KIND" != "legacy" && "$INPUT_KIND" != "interchange" ]]; then
  echo "--input-kind must be legacy or interchange" >&2
  exit 2
fi

if [[ ! "$RUN_ID" =~ ^[A-Za-z0-9._-]+$ ]]; then
  echo "--run-id may only contain letters, numbers, dots, underscores, or hyphens" >&2
  exit 2
fi

if [[ ! -f "$INPUT" ]]; then
  echo "input file does not exist: $INPUT" >&2
  exit 1
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "jq is required for private dry-run summaries" >&2
  exit 1
fi

INPUT_ABS="$(abs_path "$INPUT")"
refuse_public_repo_path "$INPUT" "input"

if [[ -z "$OUTPUT_DIR" ]]; then
  OUTPUT_DIR=".private/private-memory-dry-runs/$RUN_ID"
fi

if [[ "$OUTPUT_DIR" != /* ]]; then
  case "$OUTPUT_DIR" in
    .private|.private/*) ;;
    *)
      echo "output directory must be under .private/ or outside this repository: $OUTPUT_DIR" >&2
      exit 1
      ;;
  esac
fi
case "$OUTPUT_DIR" in
  "$ROOT"|"$ROOT"/*)
    case "$OUTPUT_DIR" in
      "$ROOT"/.private|"$ROOT"/.private/*) ;;
      *)
        echo "output directory must be under .private/ or outside this repository: $OUTPUT_DIR" >&2
        exit 1
        ;;
    esac
    ;;
esac
mkdir -p "$OUTPUT_DIR"
OUTPUT_ABS="$(abs_path "$OUTPUT_DIR")"
refuse_public_repo_path "$OUTPUT_DIR" "output directory"

if [[ "$SKIP_GATES" != "1" ]]; then
  bash scripts/verify-cli-coexistence.sh
  bash scripts/verify-dogfood-migration.sh
fi

GLOBAL_NAHUALI_BEFORE="$(command -v nahuali || true)"

if [[ -n "${NAHUALI_PRIVATE_DRY_RUN_BIN:-}" ]]; then
  NAHUALI_BIN="$(root_relative_path "$NAHUALI_PRIVATE_DRY_RUN_BIN")"
elif [[ -n "${NAHUALI_PRIVATE_DRY_RUN_BIN_DIR:-}" ]]; then
  NAHUALI_BIN="$(root_relative_path "$NAHUALI_PRIVATE_DRY_RUN_BIN_DIR")/nahuali"
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
  echo "Rust nahuali binary is missing or not executable" >&2
  echo "expected: $NAHUALI_BIN" >&2
  exit 1
fi

INTERCHANGE="$INPUT_ABS"
CONVERT_OUTPUT="$OUTPUT_ABS/convert.json"
if [[ "$INPUT_KIND" == "legacy" ]]; then
  INTERCHANGE="$OUTPUT_ABS/private-memory.interchange.json"
  "$NAHUALI_BIN" convert-legacy-export "$INPUT_ABS" \
    --output "$INTERCHANGE" \
    --scope "$SCOPE" \
    --json >"$CONVERT_OUTPUT"
fi

DRY_RUN_DB="private_migration_${RUN_ID}_dry_run"
DRY_RUN_OUTPUT="$OUTPUT_ABS/import-dry-run.json"
"$NAHUALI_BIN" --database "$DRY_RUN_DB" import "$INTERCHANGE" --dry-run --json >"$DRY_RUN_OUTPUT"

APPLY_DB="private_migration_${RUN_ID}_isolated"
DRILL_DB="private_migration_${RUN_ID}_drill"
RESTORE_DB="private_migration_${RUN_ID}_restore"
BACKUP="$OUTPUT_ABS/private-memory.backup.json"

if [[ "$APPLY" == "1" ]]; then
  "$NAHUALI_BIN" --database "$APPLY_DB" import "$INTERCHANGE" --json >"$OUTPUT_ABS/import.json"
  "$NAHUALI_BIN" --database "$APPLY_DB" validate --json >"$OUTPUT_ABS/validate.json"
  "$NAHUALI_BIN" --database "$APPLY_DB" projection-rebuild --json >"$OUTPUT_ABS/projection-rebuild.json"
  "$NAHUALI_BIN" --database "$APPLY_DB" projection-validate --json >"$OUTPUT_ABS/projection-validate.json"
  "$NAHUALI_BIN" --database "$APPLY_DB" semantic-rebuild --json >"$OUTPUT_ABS/semantic-rebuild.json"
  "$NAHUALI_BIN" --database "$APPLY_DB" backup --output "$BACKUP" --json >"$OUTPUT_ABS/backup.json"
  "$NAHUALI_BIN" backup-validate "$BACKUP" --json >"$OUTPUT_ABS/backup-validate.json"
  "$NAHUALI_BIN" backup-drill "$BACKUP" --target-database "$DRILL_DB" --json >"$OUTPUT_ABS/backup-drill.json"
  "$NAHUALI_BIN" restore "$BACKUP" --target-database "$RESTORE_DB" --dry-run --json >"$OUTPUT_ABS/restore-dry-run.json"
fi

SUMMARY="$OUTPUT_ABS/summary.txt"
SUMMARY_JSON="$OUTPUT_ABS/summary.json"

convert_summary_fallback() {
  local field="$1"
  jq_json_field "$CONVERT_OUTPUT" ".summary.$field" "null"
}

dry_run_valid="$(jq_json_field "$DRY_RUN_OUTPUT" '.report.valid' 'null')"
dry_run_imported_events="$(jq_json_field "$DRY_RUN_OUTPUT" '.report.imported_event_count' 'null')"
dry_run_appendable_events="$(jq_json_field "$DRY_RUN_OUTPUT" '.report.appendable_event_count' 'null')"
source_count="$(jq_json_field "$DRY_RUN_OUTPUT" '.report.counts.sources // .report.preflight.source_count' "$(convert_summary_fallback source_count)")"
episode_count="$(jq_json_field "$DRY_RUN_OUTPUT" '.report.counts.episodes' "$(convert_summary_fallback episode_count)")"
claim_count="$(jq_json_field "$DRY_RUN_OUTPUT" '.report.counts.claims' "$(convert_summary_fallback claim_count)")"
link_count="$(jq_json_field "$DRY_RUN_OUTPUT" '.report.counts.links' "$(convert_summary_fallback link_count)")"
procedure_count="$(jq_json_field "$DRY_RUN_OUTPUT" '.report.counts.procedures' "$(convert_summary_fallback procedure_count)")"
intention_count="$(jq_json_field "$DRY_RUN_OUTPUT" '.report.counts.intentions' "$(convert_summary_fallback intention_count)")"
conversion_issue_count="$(jq_json_field "$CONVERT_OUTPUT" '.summary.issue_count' 'null')"
conversion_issue_paths="$(jq_json_field "$CONVERT_OUTPUT" '[.issues[]?.path] | unique' '[]')"
evidence_gap_count="$(jq_json_field "$DRY_RUN_OUTPUT" '.report.preflight.evidence_gap_count' 'null')"
unsourced_episode_count="$(jq_json_field "$DRY_RUN_OUTPUT" '.report.preflight.unsourced_episode_count' 'null')"
unscoped_record_count="$(jq_json_field "$DRY_RUN_OUTPUT" '.report.preflight.unscoped_record_count' 'null')"
scope_keys="$(jq_json_field "$DRY_RUN_OUTPUT" '.report.preflight.scope_keys' '[]')"
review_item_count="$(jq_json_field "$DRY_RUN_OUTPUT" '.report.readiness.review_item_count' 'null')"
source_coverage_count="$(jq_json_field "$DRY_RUN_OUTPUT" '.report.readiness.self_inspection_summary.source_coverage_count' 'null')"
automatic_write_back="$(jq_json_field "$DRY_RUN_OUTPUT" '.report.readiness.write_back_policy.automatic_write_back' 'null')"

validate_valid="null"
projection_valid="null"
semantic_rebuild_source_count="null"
backup_valid="null"
backup_drill_valid="null"
restore_dry_run_events="null"
if [[ "$APPLY" == "1" ]]; then
  validate_valid="$(jq_json_field "$OUTPUT_ABS/validate.json" '.valid' 'null')"
  projection_valid="$(jq_json_field "$OUTPUT_ABS/projection-validate.json" '.validation.valid' 'null')"
  semantic_rebuild_source_count="$(jq_json_field "$OUTPUT_ABS/semantic-rebuild.json" '.report.source_event_count' 'null')"
  backup_valid="$(jq_json_field "$OUTPUT_ABS/backup-validate.json" '.valid' 'null')"
  backup_drill_valid="$(jq_json_field "$OUTPUT_ABS/backup-drill.json" '.valid' 'null')"
  restore_dry_run_events="$(jq_json_field "$OUTPUT_ABS/restore-dry-run.json" '.restored_event_count' 'null')"
fi

jq -n \
  --arg run_id "$RUN_ID" \
  --arg input_kind "$INPUT_KIND" \
  --arg output_directory "$OUTPUT_ABS" \
  --arg scope "$SCOPE" \
  --arg synthetic_gates_before_run "$([[ "$SKIP_GATES" == "1" ]] && echo skipped || echo passed)" \
  --arg cutover_recommendation "no" \
  --argjson input_copied false \
  --argjson dry_run_valid "$dry_run_valid" \
  --argjson dry_run_imported_events "$dry_run_imported_events" \
  --argjson dry_run_appendable_events "$dry_run_appendable_events" \
  --argjson source_count "$source_count" \
  --argjson episode_count "$episode_count" \
  --argjson claim_count "$claim_count" \
  --argjson relation_count "$link_count" \
  --argjson procedure_count "$procedure_count" \
  --argjson intention_count "$intention_count" \
  --argjson conversion_issue_count "$conversion_issue_count" \
  --argjson conversion_issue_paths "$conversion_issue_paths" \
  --argjson evidence_gap_count "$evidence_gap_count" \
  --argjson unsourced_episode_count "$unsourced_episode_count" \
  --argjson unscoped_record_count "$unscoped_record_count" \
  --argjson scope_keys "$scope_keys" \
  --argjson review_item_count "$review_item_count" \
  --argjson source_coverage_count "$source_coverage_count" \
  --argjson automatic_write_back "$automatic_write_back" \
  --argjson isolated_apply "$([[ "$APPLY" == "1" ]] && echo true || echo false)" \
  --argjson validate_valid "$validate_valid" \
  --argjson projection_valid "$projection_valid" \
  --argjson semantic_rebuild_source_count "$semantic_rebuild_source_count" \
  --argjson backup_valid "$backup_valid" \
  --argjson backup_drill_valid "$backup_drill_valid" \
  --argjson restore_dry_run_events "$restore_dry_run_events" \
  '{
    run_id: $run_id,
    input_kind: $input_kind,
    input_copied: $input_copied,
    output_directory: $output_directory,
    scope: $scope,
    synthetic_gates_before_run: $synthetic_gates_before_run,
    dry_run: {
      valid: $dry_run_valid,
      imported_event_count: $dry_run_imported_events,
      appendable_event_count: $dry_run_appendable_events
    },
    counts: {
      sources: $source_count,
      episodes: $episode_count,
      claims: $claim_count,
      links: $relation_count,
      procedures: $procedure_count,
      intentions: $intention_count
    },
    conversion: {
      issue_count: $conversion_issue_count,
      issue_paths: $conversion_issue_paths
    },
    preflight: {
      evidence_gap_count: $evidence_gap_count,
      unsourced_episode_count: $unsourced_episode_count,
      unscoped_record_count: $unscoped_record_count,
      scope_keys: $scope_keys
    },
    readiness: {
      review_item_count: $review_item_count,
      source_coverage_count: $source_coverage_count,
      automatic_write_back: $automatic_write_back
    },
    isolated_apply: {
      ran: $isolated_apply,
      validate_valid: $validate_valid,
      projection_valid: $projection_valid,
      semantic_rebuild_source_count: $semantic_rebuild_source_count,
      backup_valid: $backup_valid,
      backup_drill_valid: $backup_drill_valid,
      restore_dry_run_events: $restore_dry_run_events
    },
    cutover_recommendation: $cutover_recommendation
  }' >"$SUMMARY_JSON"

{
  echo "Run ID: $RUN_ID"
  echo "Input type: $INPUT_KIND"
  echo "Input content copied: no"
  echo "Output directory: $OUTPUT_ABS"
  echo "Scope: $SCOPE"
  echo "Synthetic gates before run: $([[ "$SKIP_GATES" == "1" ]] && echo skipped || echo passed)"
  echo "Dry-run valid: $(jq_field "$DRY_RUN_OUTPUT" '.report.valid' n/a)"
  echo "Dry-run imported events: $(jq_field "$DRY_RUN_OUTPUT" '.report.imported_event_count' n/a)"
  echo "Appendable events: $(jq_field "$DRY_RUN_OUTPUT" '.report.appendable_event_count' n/a)"
  echo "Source count: $(jq_field "$DRY_RUN_OUTPUT" '.report.counts.sources' "$(jq_field "$CONVERT_OUTPUT" '.summary.source_count' n/a)")"
  echo "Episode count: $(jq_field "$DRY_RUN_OUTPUT" '.report.counts.episodes' "$(jq_field "$CONVERT_OUTPUT" '.summary.episode_count' n/a)")"
  echo "Claim count: $(jq_field "$DRY_RUN_OUTPUT" '.report.counts.claims' "$(jq_field "$CONVERT_OUTPUT" '.summary.claim_count' n/a)")"
  echo "Link count: $(jq_field "$DRY_RUN_OUTPUT" '.report.counts.links' "$(jq_field "$CONVERT_OUTPUT" '.summary.link_count' n/a)")"
  echo "Procedure count: $(jq_field "$DRY_RUN_OUTPUT" '.report.counts.procedures' "$(jq_field "$CONVERT_OUTPUT" '.summary.procedure_count' n/a)")"
  echo "Intention count: $(jq_field "$DRY_RUN_OUTPUT" '.report.counts.intentions' "$(jq_field "$CONVERT_OUTPUT" '.summary.intention_count' n/a)")"
  echo "Evidence gaps: $(jq_field "$DRY_RUN_OUTPUT" '.report.preflight.evidence_gap_count' n/a)"
  echo "Review items: $(jq_field "$DRY_RUN_OUTPUT" '.report.readiness.review_item_count' n/a)"
  if [[ "$APPLY" == "1" ]]; then
    echo "Isolated apply: yes"
    echo "Validate valid: $(jq_field "$OUTPUT_ABS/validate.json" '.valid' n/a)"
    echo "Projection valid: $(jq_field "$OUTPUT_ABS/projection-validate.json" '.validation.valid' n/a)"
    echo "Semantic rebuild source count: $(jq_field "$OUTPUT_ABS/semantic-rebuild.json" '.report.source_event_count' n/a)"
    echo "Backup valid: $(jq_field "$OUTPUT_ABS/backup-validate.json" '.valid' n/a)"
    echo "Backup drill valid: $(jq_field "$OUTPUT_ABS/backup-drill.json" '.valid' n/a)"
    echo "Restore dry-run events: $(jq_field "$OUTPUT_ABS/restore-dry-run.json" '.restored_event_count' n/a)"
  else
    echo "Isolated apply: no"
  fi
  echo "Cutover recommendation: no"
  echo "Summary JSON: $SUMMARY_JSON"
} >"$SUMMARY"

GLOBAL_NAHUALI_AFTER="$(command -v nahuali || true)"
if [[ "$GLOBAL_NAHUALI_AFTER" != "$GLOBAL_NAHUALI_BEFORE" ]]; then
  echo "global nahuali command changed during private memory dry-run" >&2
  echo "before: ${GLOBAL_NAHUALI_BEFORE:-<missing>}" >&2
  echo "after: ${GLOBAL_NAHUALI_AFTER:-<missing>}" >&2
  exit 1
fi

echo "private memory dry-run completed"
echo "summary: $SUMMARY"
echo "summary_json: $SUMMARY_JSON"
