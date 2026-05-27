#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

REMOTE_REPO="${NAHUALI_GITHUB_REPO:-Arakiss/nahuali}"
GIT_REMOTE="${NAHUALI_GIT_REMOTE:-origin}"
PRIVATE_DENYLIST="${NAHUALI_PRIVATE_DENYLIST:-.git/info/nahuali-private-denylist}"
failures_file="$(mktemp)"

cleanup() {
  rm -f "$failures_file"
}
trap cleanup EXIT

record_failure() {
  {
    echo "$1"
    if [[ $# -gt 1 && -n "$2" ]]; then
      echo "$2"
    fi
    echo
  } >>"$failures_file"
}

require_command() {
  local command_name="$1"

  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "$command_name is required for the public-opening audit" >&2
    exit 1
  fi
}

require_command git
require_command rg
require_command gh

if [[ -n "$(git status --porcelain)" ]]; then
  echo "public-opening audit requires a clean working tree" >&2
  git status --short >&2
  exit 1
fi

if [[ "${NAHUALI_GO_PUBLIC_REQUIRE_PRIVATE_DENYLIST:-1}" == "1" && ! -f "$PRIVATE_DENYLIST" ]]; then
  echo "private denylist is required for this local public-opening audit" >&2
  echo "expected=$PRIVATE_DENYLIST" >&2
  exit 1
fi

bash scripts/security-supply-chain-check.sh

bad_local_identity="$(
  {
    printf 'user.name\t%s\n' "$(git config --get user.name || true)"
    printf 'user.email\t%s\n' "$(git config --get user.email || true)"
  } | rg -n -i 'gmail|hotmail|outlook|icloud|yahoo|proton|legal name|personal email' || true
)"
if [[ -n "$bad_local_identity" ]]; then
  echo "local git identity is not public-safe:" >&2
  echo "$bad_local_identity" >&2
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
  echo "git history contains non-noreply author or committer emails:" >&2
  echo "$bad_git_emails" >&2
  exit 1
fi

tracked_emails="$(
  git grep -n -I -E '[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}' -- \
    ':!Cargo.lock' ':!target/**' ':!*.svg' ':!*.lock' || true
)"
if [[ -n "$tracked_emails" ]]; then
  echo "tracked files contain email-like strings that need explicit review:" >&2
  echo "$tracked_emails" >&2
  exit 1
fi

remote_heads="$(git ls-remote --heads "$GIT_REMOTE" | awk '{print $2}')"
unexpected_remote_heads="$(
  printf '%s\n' "$remote_heads" \
    | grep -Ev '^refs/heads/(main|release-please--branches--main)$' || true
)"
if ! printf '%s\n' "$remote_heads" | grep -qx 'refs/heads/main' \
  || [[ -n "$unexpected_remote_heads" ]]; then
  record_failure \
    "$GIT_REMOTE exposes unexpected branches for a public opening:" \
    "$remote_heads"
fi

remote_pull_refs="$(git ls-remote "$GIT_REMOTE" 'refs/pull/*' || true)"
if [[ -n "$remote_pull_refs" ]]; then
  record_failure \
    "$GIT_REMOTE exposes pull refs that can retain pre-opening history:" \
    "$remote_pull_refs"$'\n'"Use a sanitized main-only bundle or get the hidden refs purged before making the repository public."
fi

open_prs="$(
  gh pr list --repo "$REMOTE_REPO" --state open --json number,title,headRefName \
    --jq '.[] | "#\(.number) \(.headRefName): \(.title)"' || true
)"
if [[ -n "$open_prs" ]]; then
  record_failure \
    "open pull requests must be closed or recreated after the public-safe history is ready:" \
    "$open_prs"
fi

remote_tags="$(
  git ls-remote --tags "$GIT_REMOTE" \
    | awk '{print $2}' \
    | grep -Ev '\^\{\}$' || true
)"
unexpected_tags="$(
  printf '%s\n' "$remote_tags" \
    | grep -Ev '^refs/tags/nahuali-(cli|core|mcp|api)-v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$' || true
)"
if [[ -n "$unexpected_tags" ]]; then
  record_failure \
    "remote tags outside the Nahuali release stream need review:" \
    "$unexpected_tags"
fi

release_target_failures="$(
  gh api "repos/$REMOTE_REPO/releases" \
    --jq '.[] | "\(.tag_name)\t\(.target_commitish)"' \
    | while IFS=$'\t' read -r tag target; do
        [[ -n "$tag" && -n "$target" ]] || continue

        if ! git cat-file -e "$target^{commit}" 2>/dev/null; then
          printf '%s targets commit outside sanitized local history: %s\n' "$tag" "$target"
        fi
      done
)"
if [[ -n "$release_target_failures" ]]; then
  record_failure \
    "GitHub releases target commits that are not in the public-safe history:" \
    "$release_target_failures"$'\n'"Delete and recreate those prereleases after the public-safe history is final."
fi

if [[ -s "$failures_file" ]]; then
  echo "public-opening audit failed:" >&2
  sed 's/^/  /' "$failures_file" >&2
  exit 1
fi

repo_state="$(
  gh repo view "$REMOTE_REPO" --json visibility,isPrivate --jq '"visibility=\(.visibility) is_private=\(.isPrivate)"'
)"

echo "public-opening audit passed"
echo "$repo_state"
