#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

require_command() {
  local command_name="$1"

  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "$command_name is required to verify the recall contract" >&2
    exit 1
  fi
}

require_command cargo
require_command jq

if [[ "${NAHUALI_VALIDATE_SKIP_DEV_STACK:-0}" != "1" ]]; then
  bash scripts/ensure-dev-stack.sh
fi

cargo build -p nahuali-cli --quiet

TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
case "$TARGET_DIR" in
  /*) ;;
  *) TARGET_DIR="$ROOT/$TARGET_DIR" ;;
esac

NAHUALI_BIN="${NAHUALI_RECALL_CONTRACT_BIN:-$TARGET_DIR/debug/nahuali}"
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

database="recall_contract_$(date +%s)_$$"

run_nahuali() {
  "$NAHUALI_BIN" --database "$database" "$@"
}

assert_json() {
  local file="$1"
  local filter="$2"
  local message="$3"

  if ! jq -e "$filter" "$file" >/dev/null; then
    echo "recall contract failed: $message" >&2
    echo "output=$file" >&2
    jq . "$file" >&2 || cat "$file" >&2
    exit 1
  fi
}

run_nahuali remember \
  "Lena owns the release notes and keeps the changelog concise." \
  --scope project:Nahuali \
  --tag product \
  --mention Lena \
  --mention "Release Notes" >/dev/null
run_nahuali claim \
  Lena owns "release notes" \
  --scope project:Nahuali \
  --confidence 0.92 \
  --source-last >/dev/null
run_nahuali link \
  Lena owns "Release Notes" \
  --scope project:Nahuali \
  --confidence 0.9 \
  --source-last >/dev/null
run_nahuali preference \
  "Release Notes" \
  "Keep release notes concise and evidence-backed." \
  --scope project:Nahuali \
  --source-last >/dev/null
run_nahuali remember \
  "Mira owns the billing checklist." \
  --scope project:Billing \
  --tag billing \
  --mention Mira >/dev/null
run_nahuali claim \
  Mira owns "billing checklist" \
  --scope project:Billing \
  --confidence 0.88 \
  --source-last >/dev/null
run_nahuali link \
  Mira owns "Billing Checklist" \
  --scope project:Billing \
  --confidence 0.88 \
  --source-last >/dev/null

release_owner="$WORK_DIR/release-owner.json"
run_nahuali recall \
  "Who owns release notes?" \
  --scope project:Nahuali \
  --kind claim \
  --require-evidence \
  --authority \
  --json >"$release_owner"

assert_json "$release_owner" '
  (.authority.can_trust | type == "boolean")
  and any(.results[]?;
    .kind == "claim"
    and ((.excerpt // "") | ascii_downcase | contains("lena"))
    and ((.excerpt // "") | ascii_downcase | contains("release notes"))
    and ((.scope.key // "") | ascii_downcase) == "project:nahuali"
    and ((.evidence_id // "") | startswith("episode_"))
  )
' "evidence-backed scoped release owner recall must include the supported Lena claim"

billing_scope="$WORK_DIR/billing-scope.json"
run_nahuali recall \
  "Who owns release notes?" \
  --scope project:Billing \
  --kind claim \
  --require-evidence \
  --authority \
  --json >"$billing_scope"

assert_json "$billing_scope" '
  all(.results[]?;
    (((.scope.key // "") | ascii_downcase) != "project:nahuali")
    and (((.excerpt // "") | ascii_downcase | contains("lena")) | not)
    and (((.excerpt // "") | ascii_downcase | contains("release notes")) | not)
  )
' "project-scoped recall must not leak release ownership into project:Billing"

unknown_owner="$WORK_DIR/unknown-owner.json"
run_nahuali recall \
  "Who owns deployment keys?" \
  --scope project:Nahuali \
  --kind claim \
  --require-evidence \
  --authority \
  --json >"$unknown_owner"

assert_json "$unknown_owner" '
  all(.results[]?;
    (((.excerpt // "") | ascii_downcase | contains("deployment keys")) | not)
    and (((.excerpt // "") | ascii_downcase | contains("owns deployment")) | not)
  )
' "unknown scoped owner recall must not invent deployment-key ownership"

echo "recall contract passed"
