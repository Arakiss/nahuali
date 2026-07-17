#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

NAHUALI_BIN="${NAHUALI_BIN:-$ROOT/target/debug/nahuali}"
SURREAL_CONTAINER="nahual-mictlan-surrealdb"
SURREAL_IMAGE="surrealdb/surrealdb:v3.0.5"
PROCESS_COUNT=16
DATABASE="projection_concurrency_$(date +%s)_$$"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/nahuali-projection-concurrency.XXXXXX")"

cleanup() {
  find "$WORK_DIR" -depth -delete 2>/dev/null || true
  if docker inspect "$SURREAL_CONTAINER" >/dev/null 2>&1; then
    printf 'REMOVE DATABASE %s;\n' "$DATABASE" | \
      docker exec -i "$SURREAL_CONTAINER" /surreal sql \
        --endpoint ws://localhost:8000 \
        --username root \
        --password root \
        --namespace nahuali \
        --hide-welcome >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

for command_name in docker jq; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "$command_name is required for the projection concurrency gate" >&2
    exit 1
  fi
done

if [[ ! -x "$NAHUALI_BIN" ]]; then
  echo "nahuali binary is missing or not executable: $NAHUALI_BIN" >&2
  exit 1
fi

observed_image="$(docker inspect --format='{{.Config.Image}}' "$SURREAL_CONTAINER" 2>/dev/null || true)"
if [[ "$observed_image" != "$SURREAL_IMAGE" ]]; then
  echo "the projection concurrency gate requires the running SurrealDB v3.0.5 dev container" >&2
  echo "run scripts/ensure-dev-stack.sh before this gate" >&2
  exit 1
fi

export NAHUALI_DB_URL="ws://127.0.0.1:18000"
export NAHUALI_DB_NAMESPACE="nahuali"
export NAHUALI_DB_USERNAME="root"
export NAHUALI_DB_PASSWORD="root"
export NO_COLOR=1

# Keep two mentions in this first append: it is the remote SurrealDB 3.0.5
# regression for inserting multiple rows into one fenced relation batch.
"$NAHUALI_BIN" --database "$DATABASE" remember \
  "Lena owns the release notes" \
  --tag product \
  --mention Lena \
  --mention "Release Notes" >/dev/null
"$NAHUALI_BIN" --database "$DATABASE" claim \
  Lena owns "release notes" \
  --confidence 0.92 \
  --source-last >/dev/null
"$NAHUALI_BIN" --database "$DATABASE" link \
  Lena owns "Release Notes" \
  --confidence 0.9 \
  --source-last >/dev/null

declare -a pids=()
declare -a outputs=()
declare -a errors=()
for ((index = 1; index <= PROCESS_COUNT; index += 1)); do
  output="$WORK_DIR/rebuild-$index.json"
  error="$WORK_DIR/rebuild-$index.stderr"
  outputs+=("$output")
  errors+=("$error")
  "$NAHUALI_BIN" --database "$DATABASE" projection-rebuild --json \
    >"$output" 2>"$error" &
  pids+=("$!")
done

successful=0
contended=0
unexpected=0
for ((index = 0; index < PROCESS_COUNT; index += 1)); do
  if wait "${pids[$index]}"; then
    successful=$((successful + 1))
    if ! jq -e '
      .report.status.in_sync == true and
      .report.status.projection_version == 2 and
      .report.status.checkpoint_projection_version == 2 and
      .report.status.checkpoint_manifest_algorithm == "sha256-canonical-json-v1" and
      (.report.status.actual_manifest_digest | test("^[0-9a-f]{64}$")) and
      .report.status.checkpoint_manifest_digest == .report.status.actual_manifest_digest
    ' "${outputs[$index]}" >/dev/null; then
      echo "successful projection rebuild $((index + 1)) returned an invalid or out-of-sync report" >&2
      unexpected=$((unexpected + 1))
    fi
  elif grep -Eq 'timed out after [0-9]+ms waiting for the graph projection rebuild lease|lost graph projection rebuild lease with fencing token [0-9]+' "${errors[$index]}"; then
    contended=$((contended + 1))
  else
    echo "projection rebuild $((index + 1)) failed for a reason other than explicit lease contention" >&2
    unexpected=$((unexpected + 1))
  fi
done

if ((successful == 0)); then
  echo "no concurrent projection rebuild completed successfully" >&2
  exit 1
fi
if ((unexpected != 0)); then
  echo "$unexpected concurrent projection rebuild result(s) violated the contract" >&2
  exit 1
fi

pre_mutation_validation="$WORK_DIR/pre-mutation-validation.json"
"$NAHUALI_BIN" --database "$DATABASE" projection-validate --json \
  >"$pre_mutation_validation"
jq -e '
  .validation.valid == true and
  .validation.status.in_sync == true and
  .validation.status.projection_version == 2 and
  .validation.status.checkpoint_projection_version == 2 and
  .validation.status.checkpoint_manifest_algorithm == "sha256-canonical-json-v1" and
  (.validation.status.actual_manifest_digest | test("^[0-9a-f]{64}$")) and
  .validation.status.checkpoint_manifest_digest == .validation.status.actual_manifest_digest and
  .validation.status.table_counts.claim == 1 and
  .validation.status.table_counts.mentions == 2 and
  .validation.status.table_counts.relates_to == 1 and
  .validation.status.table_counts.supports == 1
' "$pre_mutation_validation" >/dev/null

# Prove the CLI fails closed when counts and row identity stay constant but
# projected content no longer matches the checkpoint manifest.
printf "UPDATE claim SET object = 'tampered release notes';\n" | \
  docker exec -i "$SURREAL_CONTAINER" /surreal sql \
    --endpoint ws://localhost:8000 \
    --username root \
    --password root \
    --namespace nahuali \
    --database "$DATABASE" \
    --hide-welcome >/dev/null 2>&1

invalid_validation="$WORK_DIR/invalid-validation.json"
if "$NAHUALI_BIN" --database "$DATABASE" projection-validate --json \
  >"$invalid_validation" 2>"$WORK_DIR/invalid-validation.stderr"; then
  echo "projection-validate exited zero for manifest-corrupted content" >&2
  exit 1
fi
jq -e '
  .validation.valid == false and
  .validation.status.in_sync == false and
  .validation.status.table_counts.claim == 1 and
  any(.validation.issues[]; contains("claim"))
' "$invalid_validation" >/dev/null

"$NAHUALI_BIN" --database "$DATABASE" projection-rebuild --json \
  >"$WORK_DIR/restored-rebuild.json"
final_validation="$WORK_DIR/final-validation.json"
"$NAHUALI_BIN" --database "$DATABASE" projection-validate --json \
  >"$final_validation"
jq -e '
  .validation.valid == true and
  .validation.status.in_sync == true and
  .validation.status.projection_version == 2 and
  .validation.status.checkpoint_projection_version == 2 and
  (.validation.status.checkpoint_manifest_table_digests | length) == 17 and
  .validation.status.checkpoint_manifest_table_digests == .validation.status.actual_manifest_table_digests and
  .validation.status.checkpoint_manifest_digest == .validation.status.actual_manifest_digest
' "$final_validation" >/dev/null

printf 'projection concurrency gate passed: %s success, %s explicit contention\n' "$successful" "$contended"
