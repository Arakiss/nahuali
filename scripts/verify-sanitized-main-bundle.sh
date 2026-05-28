#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PRIVATE_DENYLIST="${NAHUALI_PRIVATE_DENYLIST:-$ROOT/.git/info/nahuali-private-denylist}"

usage() {
  cat <<'USAGE'
Usage: bash scripts/verify-sanitized-main-bundle.sh <bundle>

Verifies that a main-only bundle can seed a fresh public repository without
carrying hidden refs, local-only paths, non-noreply git identities, tracked
email strings, or private denylist matches.
USAGE
}

require_command() {
  local command_name="$1"

  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "$command_name is required to verify a sanitized main bundle" >&2
    exit 1
  fi
}

absolute_existing_file() {
  local path="$1"
  local dir
  local base

  dir="$(dirname "$path")"
  base="$(basename "$path")"
  if [[ ! -f "$dir/$base" ]]; then
    echo "bundle does not exist: $path" >&2
    exit 1
  fi
  dir="$(cd "$dir" && pwd -P)"
  printf '%s/%s\n' "$dir" "$base"
}

if [[ $# -ne 1 || "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit $([[ $# -eq 1 ]] && 0 || 2)
fi

require_command git
require_command rg
require_command cargo

bundle="$(absolute_existing_file "$1")"
if [[ "${NAHUALI_VERIFY_BUNDLE_REQUIRE_PRIVATE_DENYLIST:-1}" == "1" && ! -f "$PRIVATE_DENYLIST" ]]; then
  echo "private denylist is required for bundle verification" >&2
  echo "expected=$PRIVATE_DENYLIST" >&2
  exit 1
fi

tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

git bundle verify "$bundle" >/dev/null

clone_dir="$tmp_dir/nahuali"
git clone --quiet --branch main "$bundle" "$clone_dir"

cd "$clone_dir"

if [[ "$(git branch --format='%(refname:short)')" != "main" ]]; then
  echo "bundle clone must expose only the main branch" >&2
  git branch --format='%(refname:short)' >&2
  exit 1
fi

hidden_refs="$(
  git for-each-ref --format='%(refname)' refs/pull refs/changes refs/merge-requests 2>/dev/null || true
)"
if [[ -n "$hidden_refs" ]]; then
  echo "bundle clone contains hidden review refs:" >&2
  echo "$hidden_refs" >&2
  exit 1
fi

tracked_local_artifacts="$(
  git ls-files | rg '(^|/)(\.private|\.local|\.runs|\.nahuali-oss|\.nahual-rust|\.release-dry-run|\.dev-bin|docs)(/|$)|(^|/)\.nahuali-demo$|\.snapshot\.json$|\.backup\.json$|\.interchange\.json$' || true
)"
if [[ -n "$tracked_local_artifacts" ]]; then
  echo "bundle clone tracks local-only artifacts:" >&2
  echo "$tracked_local_artifacts" >&2
  exit 1
fi

bad_git_emails="$(
  git log --all --format='%H%x09%ae%x09%ce' \
    | awk -F '\t' '
        $2 !~ /(^[^@]+@users\.noreply\.github\.com$|^noreply@github\.com$)/ ||
        $3 !~ /(^[^@]+@users\.noreply\.github\.com$|^noreply@github\.com$)/ {
          print
        }
      '
)"
if [[ -n "$bad_git_emails" ]]; then
  echo "bundle history contains non-noreply author or committer emails:" >&2
  echo "$bad_git_emails" >&2
  exit 1
fi

tracked_emails="$(
  git grep -n -I -E '[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}' -- \
    ':!Cargo.lock' ':!target/**' ':!*.svg' ':!*.lock' || true
)"
if [[ -n "$tracked_emails" ]]; then
  echo "bundle clone contains email-like strings that need review:" >&2
  echo "$tracked_emails" >&2
  exit 1
fi

if [[ -f "$PRIVATE_DENYLIST" ]]; then
  NAHUALI_PRIVATE_DENYLIST="$PRIVATE_DENYLIST" bash scripts/security-supply-chain-check.sh
else
  bash scripts/security-supply-chain-check.sh
fi

head_sha="$(git rev-parse HEAD)"
echo "sanitized main bundle verification passed"
echo "bundle=$bundle"
echo "head=$head_sha"
