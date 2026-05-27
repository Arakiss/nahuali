#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

repo="${NAHUALI_GITHUB_REPO:-Arakiss/nahuali}"
target_branch="${NAHUALI_RELEASE_PLEASE_BRANCH:-}"
expected_version="${NAHUALI_RELEASE_PLEASE_EXPECTED_VERSION:-}"

usage() {
  cat <<'USAGE'
Usage: bash scripts/verify-release-please-dry-run.sh [options]

Options:
  --repo OWNER/NAME           GitHub repository. Default: Arakiss/nahuali.
  --target-branch BRANCH      Branch to inspect. Default: current branch.
  --expected-version VERSION  Require a nahuali-cli candidate for this version.
  -h, --help                  Show this help.

Runs Release Please in dry-run mode against a temporary local clone so the
current worktree is never reset by release-please. The command requires an
authenticated gh CLI session because private repositories need a GitHub token.
USAGE
}

die() {
  echo "release-please-dry-run: $*" >&2
  exit 2
}

while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --repo)
      repo="${2:-}"
      shift 2
      ;;
    --target-branch)
      target_branch="${2:-}"
      shift 2
      ;;
    --expected-version)
      expected_version="${2:-}"
      shift 2
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      echo "release-please-dry-run: unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

[[ -n "$repo" ]] || die "empty GitHub repository"
command -v bun >/dev/null 2>&1 || die "required tool not found: bun"
command -v gh >/dev/null 2>&1 || die "required tool not found: gh"
command -v git >/dev/null 2>&1 || die "required tool not found: git"

if [[ -z "$target_branch" ]]; then
  target_branch="$(git branch --show-current)"
fi
[[ -n "$target_branch" ]] || die "could not determine current branch; pass --target-branch"

dirty="$(git status --porcelain --untracked-files=no)"
if [[ -n "$dirty" ]]; then
  echo "release-please-dry-run: tracked worktree changes are not included in the temporary clone:" >&2
  echo "$dirty" >&2
  exit 1
fi

tmp_root="$(mktemp -d)"
tmp_cache="$(mktemp -d)"
tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp_root" "$tmp_cache" "$tmp_dir"
}
trap cleanup EXIT

git clone --quiet "$ROOT" "$tmp_root/repo"
token="$(gh auth token)"
output="$tmp_root/release-please-dry-run.log"

if ! BUN_INSTALL_CACHE_DIR="$tmp_cache" TMPDIR="$tmp_dir" bunx release-please release-pr \
  --repo-url "$repo" \
  --target-branch "$target_branch" \
  --config-file release-please-config.json \
  --manifest-file .release-please-manifest.json \
  --local \
  --local-path "$tmp_root/repo" \
  --dry-run \
  --debug \
  --token "$token" >"$output" 2>&1; then
  sed -n '1,220p' "$output" >&2
  exit 1
fi

if ! grep -Eq '^Would open [1-9][0-9]* pull requests$' "$output"; then
  echo "release-please-dry-run: expected at least one Release Please PR candidate" >&2
  sed -n '/Building candidate release pull request for path: crates\/nahuali-cli/,$p' "$output" | tail -n 120 >&2
  exit 1
fi

if ! grep -Fq 'nahuali-cli:' "$output"; then
  echo "release-please-dry-run: expected a nahuali-cli release candidate" >&2
  sed -n '/Would open /,$p' "$output" >&2
  exit 1
fi

if [[ -n "$expected_version" ]] \
  && ! grep -Eq "(nahuali-cli: ${expected_version}|version: ${expected_version} from release-please)" "$output"; then
  echo "release-please-dry-run: expected nahuali-cli $expected_version" >&2
  sed -n '/Would open /,$p' "$output" >&2
  exit 1
fi

grep -E '^(Would open|title:|branch:|draft:)|nahuali-cli:' "$output" || true
echo "release-please dry-run passed"
