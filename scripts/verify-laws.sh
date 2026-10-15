#!/usr/bin/env bash
# Fail-closed gate for the invariants documented in LAWS.md.
#
# Fails when a Kani harness or law property test fails, is skipped, is
# ignored, or when the harnesses/tests in source drift from the ones LAWS.md
# declares. Requires `cargo kani` (see LAWS.md for the pinned version).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

LAWS_DOC="LAWS.md"
LAWS_SRC="crates/nahuali-core/src/laws.rs"
LOG_DIR="${NAHUALI_LAWS_LOG_DIR:-${RUNNER_TEMP:-/tmp}/nahuali-laws}"
mkdir -p "$LOG_DIR"

fail() {
  echo "laws gate: $*" >&2
  exit 1
}

sorted_unique() {
  sort -u | sed '/^$/d'
}

# Harnesses: every `#[kani::proof]` in the crate must live in laws.rs, and the
# function names under it must match the `kani:` entries in LAWS.md exactly.
proof_attributes="$(grep -rn '#\[kani::proof\]' crates/nahuali-core/src | wc -l | tr -d ' ')"
source_harnesses="$(awk '/#\[kani::proof\]/ { want = 1; next } want && /fn [a-z_0-9]+/ { match($0, /fn [a-z_0-9]+/); print substr($0, RSTART + 3, RLENGTH - 3); want = 0 }' "$LAWS_SRC" | sorted_unique)"
doc_harnesses="$(grep -oE 'kani: `[a-z_0-9]+`' "$LAWS_DOC" | sed -E 's/kani: `([a-z_0-9]+)`/\1/' | sorted_unique)"
harness_count="$(printf '%s\n' "$source_harnesses" | sed '/^$/d' | wc -l | tr -d ' ')"

[[ "$harness_count" -gt 0 ]] || fail "no Kani harnesses found in $LAWS_SRC"
[[ "$proof_attributes" == "$harness_count" ]] \
  || fail "found $proof_attributes #[kani::proof] attributes in the crate but $harness_count in $LAWS_SRC"
if [[ "$source_harnesses" != "$doc_harnesses" ]]; then
  diff <(printf '%s\n' "$doc_harnesses") <(printf '%s\n' "$source_harnesses") >&2 || true
  fail "Kani harnesses in $LAWS_SRC do not match the kani: entries in $LAWS_DOC (< doc, > source)"
fi

# Property tests: `test:` entries in LAWS.md must match the #[test] functions in
# laws.rs, and every one must run and pass, with none ignored.
source_tests="$(awk '/#\[test\]/ { want = 1; next } want && /fn [a-z_0-9]+/ { match($0, /fn [a-z_0-9]+/); print substr($0, RSTART + 3, RLENGTH - 3); want = 0 }' "$LAWS_SRC" | sorted_unique)"
doc_tests="$(grep -oE 'test: `[a-z_0-9]+`' "$LAWS_DOC" | sed -E 's/test: `([a-z_0-9]+)`/\1/' | sorted_unique)"
test_count="$(printf '%s\n' "$source_tests" | sed '/^$/d' | wc -l | tr -d ' ')"
if [[ "$source_tests" != "$doc_tests" ]]; then
  diff <(printf '%s\n' "$doc_tests") <(printf '%s\n' "$source_tests") >&2 || true
  fail "law tests in $LAWS_SRC do not match the test: entries in $LAWS_DOC (< doc, > source)"
fi
if grep -nE '#\[ignore' "$LAWS_SRC"; then
  fail "law tests must not be ignored"
fi

test_log="$LOG_DIR/law-tests.log"
cargo test --locked -p nahuali-core --lib laws:: -- --include-ignored 2>&1 | tee "$test_log"
grep -qE "^test result: ok\. ${test_count} passed; 0 failed; 0 ignored;" "$test_log" \
  || fail "expected exactly ${test_count} law tests to pass with 0 failed and 0 ignored"

# Kani: run every harness. Kani exits non-zero on any failure; the summary line
# must additionally account for every expected harness, so a harness that was
# filtered, cfg'd out, or silently skipped also fails the gate.
kani_log="$LOG_DIR/kani.log"
set +e
cargo kani -p nahuali-core --lib --output-format terse 2>&1 | tee "$kani_log"
kani_status="${PIPESTATUS[0]}"
set -e
[[ "$kani_status" == "0" ]] || fail "cargo kani exited with status $kani_status"
grep -qE "^Complete - ${harness_count} successfully verified harnesses, 0 failures, ${harness_count} total\.$" "$kani_log" \
  || fail "expected ${harness_count} successfully verified harnesses with 0 failures"
while IFS= read -r harness; do
  grep -qE "^(Thread [0-9]+: )?Checking harness laws::proofs::${harness}\.\.\.$" "$kani_log" \
    || fail "Kani did not check harness ${harness}"
done <<<"$source_harnesses"

echo "laws gate: ${harness_count} Kani harnesses verified, ${test_count} law tests passed"
