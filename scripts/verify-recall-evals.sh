#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ "${NAHUALI_VALIDATE_SKIP_DEV_STACK:-0}" != "1" ]]; then
  bash scripts/ensure-dev-stack.sh
fi

if ! command -v bun >/dev/null 2>&1; then
  echo "bun is required to run Promptfoo evals" >&2
  exit 1
fi

if ! command -v node >/dev/null 2>&1; then
  echo "node is required to run Promptfoo evals because Promptfoo uses Node-only native dependencies" >&2
  exit 1
fi

node_version="$(node --version | sed 's/^v//')"
IFS=. read -r node_major node_minor _ <<<"$node_version"
if [[ ! "$node_major" =~ ^[0-9]+$ || ! "$node_minor" =~ ^[0-9]+$ ]] \
  || ! (( (node_major == 20 && node_minor >= 20) || (node_major == 22 && node_minor >= 22) || node_major > 22 )); then
  echo "node $node_version is too old for Promptfoo; use Node ^20.20.0 or >=22.22.0" >&2
  exit 1
fi

cargo build -p nahuali-cli --quiet

TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
case "$TARGET_DIR" in
  /*) ;;
  *) TARGET_DIR="$ROOT/$TARGET_DIR" ;;
esac

NAHUALI_BIN="$TARGET_DIR/debug/nahuali"
if [[ ! -x "$NAHUALI_BIN" ]]; then
  echo "Rust nahuali binary is missing after cargo build" >&2
  echo "expected: $NAHUALI_BIN" >&2
  exit 1
fi

WORK_DIR="$(mktemp -d)"
cleanup() {
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT

prepare_bun_runtime() {
  rm -rf "$WORK_DIR/bun-cache" "$WORK_DIR/tmp"
  mkdir -p "$WORK_DIR/bun-cache" "$WORK_DIR/tmp"
}

prepare_bun_runtime

export NAHUALI_EVAL_NAHUALI_BIN="$NAHUALI_BIN"
export PROMPTFOO_CONFIG_DIR="$WORK_DIR/promptfoo"
export PROMPTFOO_DISABLE_TELEMETRY=1
export BUN_INSTALL_CACHE_DIR="$WORK_DIR/bun-cache"
export TMPDIR="$WORK_DIR/tmp"

PROMPTFOO_VERSION="${PROMPTFOO_VERSION:-0.121.12}"
if [[ -n "${PROMPTFOO_BIN:-}" ]]; then
  PROMPTFOO_COMMAND=("$PROMPTFOO_BIN")
else
  PROMPTFOO_COMMAND=(bunx "promptfoo@${PROMPTFOO_VERSION}")
fi

OUTPUT="$WORK_DIR/recall-evals.json"
run_promptfoo() {
  "${PROMPTFOO_COMMAND[@]}" eval \
    -c evals/promptfooconfig.yaml \
    --no-cache \
    --no-write \
    --no-progress-bar \
    --no-table \
    --max-concurrency 1 \
    --output "$OUTPUT"
}

if ! run_promptfoo; then
  echo "promptfoo eval failed; retrying once with a clean Bun runtime" >&2
  rm -f "$OUTPUT"
  prepare_bun_runtime
  run_promptfoo
fi

test -s "$OUTPUT"

echo "recall promptfoo evals passed"
