#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ "${NAHUALI_VALIDATE_SKIP_DEV_STACK:-0}" != "1" ]]; then
  bash scripts/ensure-dev-stack.sh
fi

INSTALL_ROOT="$(mktemp -d)"
STORE_DIR="$(mktemp -d)"

cleanup() {
  rm -rf "$INSTALL_ROOT" "$STORE_DIR"
}
trap cleanup EXIT

json_matches() {
  local output="$1"
  local pattern="$2"
  printf '%s\n' "$output" | grep -Eq "$pattern"
}

if [[ -n "${NAHUALI_VERIFY_INSTALL_BIN_DIR:-}" ]]; then
  SOURCE_BIN_DIR="$NAHUALI_VERIFY_INSTALL_BIN_DIR"
  case "$SOURCE_BIN_DIR" in
    /*) ;;
    *) SOURCE_BIN_DIR="$ROOT/$SOURCE_BIN_DIR" ;;
  esac

  mkdir -p "$INSTALL_ROOT/bin"
  for binary in nahuali nahuali-mcp nahuali-api; do
    if [[ ! -x "$SOURCE_BIN_DIR/$binary" ]]; then
      echo "source release binary is missing or not executable: $SOURCE_BIN_DIR/$binary" >&2
      exit 1
    fi
    cp "$SOURCE_BIN_DIR/$binary" "$INSTALL_ROOT/bin/$binary"
    chmod +x "$INSTALL_ROOT/bin/$binary"
  done
else
  cargo install --path crates/nahuali-cli --locked --debug --root "$INSTALL_ROOT" --force --quiet
  cargo install --path crates/nahuali-mcp --locked --debug --root "$INSTALL_ROOT" --force --quiet
  cargo install --path crates/nahuali-api --locked --debug --root "$INSTALL_ROOT" --force --quiet
fi

NAHUALI="$INSTALL_ROOT/bin/nahuali"
NAHUALI_MCP="$INSTALL_ROOT/bin/nahuali-mcp"
NAHUALI_API="$INSTALL_ROOT/bin/nahuali-api"
# The CLI refuses a path-like --database name, so derive a clean, unique
# SurrealDB identifier from the temp dir instead of passing the path itself.
RUN_ID="$(basename "$STORE_DIR" | tr -cd '[:alnum:]')"
STORE="verify_install_${RUN_ID}"

if [[ ! -x "$NAHUALI" ]]; then
  echo "installed nahuali binary is missing or not executable" >&2
  exit 1
fi

if [[ ! -x "$NAHUALI_MCP" ]]; then
  echo "installed nahuali-mcp binary is missing or not executable" >&2
  exit 1
fi

if [[ ! -x "$NAHUALI_API" ]]; then
  echo "installed nahuali-api binary is missing or not executable" >&2
  exit 1
fi

cli_version="$("$NAHUALI" --version)"
if [[ "$cli_version" != nahuali\ * ]]; then
  echo "unexpected nahuali version output: $cli_version" >&2
  exit 1
fi

mcp_version="$("$NAHUALI_MCP" --version)"
if [[ "$mcp_version" != nahuali-mcp\ * ]]; then
  echo "unexpected nahuali-mcp version output: $mcp_version" >&2
  exit 1
fi

api_version="$("$NAHUALI_API" --version)"
if [[ "$api_version" != nahuali-api\ * ]]; then
  echo "unexpected nahuali-api version output: $api_version" >&2
  exit 1
fi

"$NAHUALI" --database "$STORE" remember "Lena owns the release notes" --tag product >/dev/null
"$NAHUALI" --database "$STORE" fact Lena owns "release notes" --confidence 0.92 --source-last >/dev/null
"$NAHUALI" --database "$STORE" relate Lena owns "release notes" --confidence 0.9 --source-last >/dev/null
"$NAHUALI" --database "$STORE" claim Lena prefers "concise release notes" --confidence 0.93 --source-last >/dev/null
"$NAHUALI" --database "$STORE" link Lena prefers "release notes" --confidence 0.91 --source-last >/dev/null
"$NAHUALI" --database "$STORE" preference "Release notes" "Keep release notes concise" --source-last >/dev/null
intention_output="$("$NAHUALI" --database "$STORE" intention "Ship release notes" --priority high --source-last --json)"
intention_id="$(printf '%s\n' "$intention_output" | sed -n 's/^[[:space:]]*"id"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)"
if [[ -z "$intention_id" ]]; then
  echo "installed nahuali intention output did not include an id" >&2
  echo "$intention_output" >&2
  exit 1
fi
"$NAHUALI" --database "$STORE" intention-status "$intention_id" completed --reason done >/dev/null

recall_output="$("$NAHUALI" --database "$STORE" recall "Lena release")"
if [[ "$recall_output" != *"evidence: episode_"* ]]; then
  echo "installed nahuali recall output did not include evidence" >&2
  echo "$recall_output" >&2
  exit 1
fi

authority_output="$("$NAHUALI" --database "$STORE" recall "Lena release" --authority --json)"
if [[ "$authority_output" != *'"mode": "advisory"'* && "$authority_output" != *'"mode": "certify"'* ]]; then
  echo "installed nahuali authority recall output did not include an authority mode" >&2
  echo "$authority_output" >&2
  exit 1
fi

inspect_output="$("$NAHUALI" --database "$STORE" inspect --json)"
if [[ "$inspect_output" != *'"supported_fact_count": 2'* ]]; then
  echo "installed nahuali inspect output did not report supported memory" >&2
  echo "$inspect_output" >&2
  exit 1
fi

validate_output="$("$NAHUALI" --database "$STORE" validate --json)"
if ! json_matches "$validate_output" '"valid"[[:space:]]*:[[:space:]]*true'; then
  echo "installed nahuali validate output did not report a valid store" >&2
  echo "$validate_output" >&2
  exit 1
fi
if ! json_matches "$validate_output" '"procedure_count"[[:space:]]*:[[:space:]]*1' \
  || ! json_matches "$validate_output" '"intention_count"[[:space:]]*:[[:space:]]*1'; then
  echo "installed nahuali validate output did not include expanded memory counts" >&2
  echo "$validate_output" >&2
  exit 1
fi

INTERCHANGE="$STORE_DIR/memory.interchange.json"
IMPORTED_STORE="verify_install_imported_${RUN_ID}"
export_output="$("$NAHUALI" --database "$STORE" export --output "$INTERCHANGE" --json)"
if ! json_matches "$export_output" '"episode_count"[[:space:]]*:[[:space:]]*1' \
  || ! json_matches "$export_output" '"claim_count"[[:space:]]*:[[:space:]]*2' \
  || ! json_matches "$export_output" '"link_count"[[:space:]]*:[[:space:]]*2'; then
  echo "installed nahuali export output did not report the expected interchange counts" >&2
  echo "$export_output" >&2
  exit 1
fi
if [[ ! -s "$INTERCHANGE" ]]; then
  echo "installed nahuali export did not write an interchange document" >&2
  exit 1
fi

import_dry_run="$("$NAHUALI" --database "$IMPORTED_STORE" import "$INTERCHANGE" --dry-run --json)"
if ! json_matches "$import_dry_run" '"valid"[[:space:]]*:[[:space:]]*true' \
  || ! json_matches "$import_dry_run" '"dry_run"[[:space:]]*:[[:space:]]*true' \
  || ! json_matches "$import_dry_run" '"imported_event_count"[[:space:]]*:[[:space:]]*0'; then
  echo "installed nahuali import dry-run output was not scriptable" >&2
  echo "$import_dry_run" >&2
  exit 1
fi

import_output="$("$NAHUALI" --database "$IMPORTED_STORE" import "$INTERCHANGE" --json)"
if ! json_matches "$import_output" '"valid"[[:space:]]*:[[:space:]]*true' \
  || ! json_matches "$import_output" '"imported_event_count"[[:space:]]*:[[:space:]]*8'; then
  echo "installed nahuali import output did not report the expected event count" >&2
  echo "$import_output" >&2
  exit 1
fi

imported_validate_output="$("$NAHUALI" --database "$IMPORTED_STORE" validate --json)"
if ! json_matches "$imported_validate_output" '"valid"[[:space:]]*:[[:space:]]*true' \
  || ! json_matches "$imported_validate_output" '"event_count"[[:space:]]*:[[:space:]]*8'; then
  echo "installed nahuali imported store did not validate" >&2
  echo "$imported_validate_output" >&2
  exit 1
fi

maintenance_output="$("$NAHUALI" --database "$STORE" maintenance --json)"
if ! json_matches "$maintenance_output" '"snapshot_supported"[[:space:]]*:[[:space:]]*true' \
  || ! json_matches "$maintenance_output" '"compaction_supported"[[:space:]]*:[[:space:]]*false'; then
  echo "installed nahuali maintenance output did not report the expected policy" >&2
  echo "$maintenance_output" >&2
  exit 1
fi

SNAPSHOT="$STORE_DIR/memory.snapshot.json"
snapshot_dry_run="$("$NAHUALI" --database "$STORE" snapshot --output "$SNAPSHOT" --dry-run --json)"
if ! json_matches "$snapshot_dry_run" '"dry_run"[[:space:]]*:[[:space:]]*true' \
  || ! json_matches "$snapshot_dry_run" '"written"[[:space:]]*:[[:space:]]*false'; then
  echo "installed nahuali snapshot dry-run output was not scriptable" >&2
  echo "$snapshot_dry_run" >&2
  exit 1
fi
if [[ -e "$SNAPSHOT" ]]; then
  echo "installed nahuali snapshot dry-run wrote a snapshot" >&2
  exit 1
fi

snapshot_write="$("$NAHUALI" --database "$STORE" snapshot --output "$SNAPSHOT" --json)"
if ! json_matches "$snapshot_write" '"written"[[:space:]]*:[[:space:]]*true'; then
  echo "installed nahuali snapshot output did not report a write" >&2
  echo "$snapshot_write" >&2
  exit 1
fi

snapshot_validate="$("$NAHUALI" --database "$STORE" snapshot-validate "$SNAPSHOT" --json)"
if ! json_matches "$snapshot_validate" '"valid"[[:space:]]*:[[:space:]]*true' \
  || ! json_matches "$snapshot_validate" '"replay_equivalent"[[:space:]]*:[[:space:]]*true'; then
  echo "installed nahuali snapshot validation did not report replay equivalence" >&2
  echo "$snapshot_validate" >&2
  exit 1
fi

echo "install smoke passed"
