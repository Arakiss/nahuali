#!/usr/bin/env bash
set -euo pipefail

# Run Nahuali's governance benchmark suite as one gate and report pass/fail per
# benchmark, exiting non-zero if any did not pass.
#
# Two integrity-side measures are computed from the library (LIVR, ARP) and need
# no services. The store-backed benchmarks (PCR, CDR, TVS, plus the
# knowledge-health and recall regressions) use an isolated embedded SurrealKV
# store so they never contend with or mutate the operator's memory.

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "$1 is required for the governance benchmark gate" >&2
    exit 1
  fi
}

require_command cargo

benchmark_home="$(mktemp -d "${TMPDIR:-/tmp}/nahuali-governance.XXXXXX")"
cleanup() {
  rm -r "$benchmark_home"
}
trap cleanup EXIT
export NAHUALI_HOME="$benchmark_home"
unset NAHUALI_DB_URL

# Build the regression runner once with attestation so --livr and --arp exist.
cargo build -p nahuali-regression --features attestation --quiet

TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
case "$TARGET_DIR" in
  /*) ;;
  *) TARGET_DIR="$ROOT/$TARGET_DIR" ;;
esac
RUNNER="$TARGET_DIR/debug/nahuali-regression"
if [[ ! -x "$RUNNER" ]]; then
  echo "nahuali-regression binary is missing after build" >&2
  echo "expected: $RUNNER" >&2
  exit 1
fi

failures=0
run_benchmark() {
  local label="$1"
  shift
  printf '==> %s\n' "$label"
  if "$@" >/dev/null; then
    printf 'PASS: %s\n' "$label"
  else
    printf 'FAIL: %s\n' "$label"
    failures=$((failures + 1))
  fi
}

run_benchmark "LIVR  ledger integrity verification rate" "$RUNNER" --livr
run_benchmark "ARP   attestation recovery profile" "$RUNNER" --arp
run_benchmark "PCR   provenance coverage rate" \
  "$RUNNER" --fixtures fixtures/provenance-coverage-regression.json
run_benchmark "CDR   contradiction & staleness detection rate" \
  "$RUNNER" --fixtures fixtures/contradiction-staleness-regression.json
run_benchmark "TVS   trust verdict soundness" \
  "$RUNNER" --fixtures fixtures/trust-verdict-soundness-regression.json
run_benchmark "knowledge-health regression" \
  "$RUNNER" --fixtures fixtures/knowledge-health-regression.json
run_benchmark "recall regression" \
  "$RUNNER" --fixtures fixtures/recall-regression.json

echo
if [[ "$failures" -gt 0 ]]; then
  echo "governance benchmark gate FAILED: $failures benchmark(s) did not pass" >&2
  exit 1
fi
echo "governance benchmark gate passed: all benchmarks clean"
