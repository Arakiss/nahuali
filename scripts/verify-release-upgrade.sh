#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

usage() {
  cat <<'USAGE'
Usage: bash scripts/verify-release-upgrade.sh

Download and verify the immediately preceding Nahuali beta release, create a
real ledger with that published binary, then prove the current binary can open,
read, extend, back up, and restore it with operational projections.

Environment:
  NAHUALI_CURRENT_BIN_DIR  Directory containing the current nahuali binary.
                           Default: target/release
  NAHUALI_PREVIOUS_TAG     Previous release override. Default: beta N-1.
  NAHUALI_GITHUB_REPO      Release repository. Default: Arakiss/nahuali
USAGE
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi
if [[ "$#" -ne 0 ]]; then
  echo "release-upgrade: unexpected arguments" >&2
  usage >&2
  exit 2
fi

for tool in gh cosign jq tar; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "release-upgrade: required tool not found: $tool" >&2
    exit 2
  }
done

current_version="$(sed -n 's/^version = "\([^"]*\)".*/\1/p' Cargo.toml | head -n 1)"
if [[ ! "$current_version" =~ ^([0-9]+\.[0-9]+\.[0-9]+)-beta\.([0-9]+)$ ]]; then
  echo "release-upgrade: current workspace version is not a supported beta: $current_version" >&2
  exit 2
fi
beta_base="${BASH_REMATCH[1]}"
beta_number="${BASH_REMATCH[2]}"
if (( beta_number == 0 )); then
  echo "release-upgrade: beta.0 has no immediately preceding beta release" >&2
  exit 2
fi

repo="${NAHUALI_GITHUB_REPO:-Arakiss/nahuali}"
previous_tag="${NAHUALI_PREVIOUS_TAG:-v${beta_base}-beta.$((beta_number - 1))}"
current_bin_dir="${NAHUALI_CURRENT_BIN_DIR:-target/release}"
case "$current_bin_dir" in
  /*) ;;
  *) current_bin_dir="$ROOT/$current_bin_dir" ;;
esac
current_bin="$current_bin_dir/nahuali"
[[ -x "$current_bin" ]] || {
  echo "release-upgrade: current binary is missing: $current_bin" >&2
  exit 2
}

work_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$work_dir"
}
trap cleanup EXIT

release_dir="$work_dir/release"
mkdir -p "$release_dir"
verification="$work_dir/previous-release-verification.json"
bash scripts/verify-release.sh \
  --repo "$repo" \
  --tag "$previous_tag" \
  --dir "$release_dir" \
  --json >"$verification"
jq -e '
  .status == "pass"
  and .checks.sha256 == "pass"
  and .checks.sigstore_bundle == "pass"
  and .checks.github_artifact_attestation == "pass"
' "$verification" >/dev/null

previous_bin="$(find "$release_dir/extracted" -type f -path '*/bin/nahuali' | head -n 1)"
[[ -x "$previous_bin" ]] || {
  echo "release-upgrade: verified previous release did not contain nahuali" >&2
  exit 1
}

previous_version="$($previous_bin --version)"
current_binary_version="$($current_bin --version)"
[[ "$previous_version" == "nahuali ${previous_tag#v}" ]] || {
  echo "release-upgrade: previous binary version mismatch: $previous_version" >&2
  exit 1
}
[[ "$current_binary_version" == "nahuali $current_version" ]] || {
  echo "release-upgrade: current binary version mismatch: $current_binary_version" >&2
  exit 1
}

export NAHUALI_HOME="$work_dir/home"
export NAHUALI_DB_URL="surrealkv://$work_dir/store"
store="upgrade_source_${beta_number}_$$"
restored_store="upgrade_restored_${beta_number}_$$"
backup="$work_dir/upgrade.backup.json"

"$previous_bin" --database "$store" episode \
  "Hrafn preserves evidence across the N-1 upgrade." \
  --tag upgrade \
  --mention Hrafn \
  --scope project:Upgrade >/dev/null
"$previous_bin" --database "$store" claim Hrafn preserves \
  "evidence across the N-1 upgrade" \
  --source-last \
  --confidence 0.97 \
  --scope project:Upgrade >/dev/null
"$previous_bin" --database "$store" intention \
  "Verify the next Nahuali release against this ledger" \
  --kind task \
  --priority high \
  --source-last \
  --scope project:Upgrade >/dev/null
"$previous_bin" --database "$store" validate --json >"$work_dir/previous-validate.json"
jq -e '.valid == true and .event_count == 3' "$work_dir/previous-validate.json" >/dev/null

"$current_bin" --database "$store" recall \
  "What does Hrafn preserve?" \
  --scope project:Upgrade \
  --require-evidence \
  --json >"$work_dir/current-recall.json"
jq -e '
  def results: if type == "array" then . else .results end;
  any(results[]?;
    .evidence_id != null
    and ((.excerpt // "") | ascii_downcase | contains("preserves evidence")))
' "$work_dir/current-recall.json" >/dev/null

"$current_bin" --database "$store" episode \
  "The current binary opened, recalled, and extended the N-1 ledger." \
  --tag upgrade \
  --mention Hrafn \
  --scope project:Upgrade >/dev/null
"$current_bin" --database "$store" projection-rebuild --json >"$work_dir/projection-rebuild.json"
"$current_bin" --database "$store" projection-validate --json >"$work_dir/projection-validate.json"
jq -e '.validation.valid == true' "$work_dir/projection-validate.json" >/dev/null

"$current_bin" --database "$store" backup --output "$backup" --json >"$work_dir/backup.json"
jq -e '.written == true and .summary.record_count == 4' "$work_dir/backup.json" >/dev/null
"$current_bin" backup-validate "$backup" --json >"$work_dir/backup-validate.json"
jq -e '.valid == true' "$work_dir/backup-validate.json" >/dev/null

"$current_bin" restore "$backup" \
  --target-database "$restored_store" \
  --rebuild-semantic \
  --json >"$work_dir/restore.json"
jq -e '
  .valid == true
  and .restored_event_count == 4
  and .graph_projection_rebuilt == true
  and .graph_projection_valid == true
  and .semantic_rebuild_completed == true
  and .semantic_index_current == true
  and .operationally_ready == true
' "$work_dir/restore.json" >/dev/null

"$current_bin" --database "$restored_store" validate --json >"$work_dir/restored-validate.json"
jq -e '.valid == true and .event_count == 4' "$work_dir/restored-validate.json" >/dev/null
"$current_bin" --database "$restored_store" recall \
  "What does Hrafn preserve?" \
  --scope project:Upgrade \
  --require-evidence \
  --json >"$work_dir/restored-recall.json"
jq -e '
  def results: if type == "array" then . else .results end;
  any(results[]?; .evidence_id != null)
' "$work_dir/restored-recall.json" >/dev/null

jq -n \
  --arg status pass \
  --arg previous_tag "$previous_tag" \
  --arg previous_version "$previous_version" \
  --arg current_version "$current_binary_version" \
  --argjson source_events 4 \
  --argjson restored_events 4 \
  '{
    status: $status,
    previous_release: {
      tag: $previous_tag,
      binary_version: $previous_version,
      checksum: "pass",
      sigstore_bundle: "pass",
      github_artifact_attestation: "pass"
    },
    current_binary_version: $current_version,
    source_event_count: $source_events,
    restored_event_count: $restored_events,
    evidence_recall_after_upgrade: "pass",
    graph_projection_after_upgrade: "pass",
    backup_restore_operational_readiness: "pass"
  }'
