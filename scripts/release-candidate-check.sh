#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

REMOTE_REPO="${NAHUALI_GITHUB_REPO:-Arakiss/nahuali}"
GIT_REMOTE="${NAHUALI_GIT_REMOTE:-origin}"

require_command() {
  local command_name="$1"

  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "$command_name is required for the release-candidate gate" >&2
    exit 1
  fi
}

require_command git
require_command gh
require_command rg

if [[ -n "$(git status --porcelain)" ]]; then
  echo "release-candidate gate requires a clean working tree" >&2
  git status --short >&2
  exit 1
fi

expected_visibility="${NAHUALI_RELEASE_CANDIDATE_EXPECT_VISIBILITY:-PUBLIC}"
visibility="$(gh repo view "$REMOTE_REPO" --json visibility --jq '.visibility')"
is_private="$(gh repo view "$REMOTE_REPO" --json isPrivate --jq '.isPrivate')"
case "$expected_visibility" in
  PUBLIC)
    if [[ "$visibility" != "PUBLIC" || "$is_private" != "false" ]]; then
      echo "repository must be public for the current release-candidate gate" >&2
      echo "visibility=$visibility is_private=$is_private" >&2
      exit 1
    fi
    ;;
  PRIVATE)
    if [[ "$visibility" != "PRIVATE" || "$is_private" != "true" ]]; then
      echo "repository must be private for this release-candidate gate" >&2
      echo "visibility=$visibility is_private=$is_private" >&2
      exit 1
    fi
    ;;
  ANY)
    ;;
  *)
    echo "unsupported NAHUALI_RELEASE_CANDIDATE_EXPECT_VISIBILITY=$expected_visibility" >&2
    echo "expected PUBLIC, PRIVATE, or ANY" >&2
    exit 1
    ;;
esac

remote_heads="$(git ls-remote --heads "$GIT_REMOTE" | awk '{print $2}')"
unexpected_remote_heads="$(
  printf '%s\n' "$remote_heads" \
    | grep -Ev '^refs/heads/(main|release-please--branches--main)$' || true
)"
if ! printf '%s\n' "$remote_heads" | grep -qx 'refs/heads/main' \
  || [[ -n "$unexpected_remote_heads" ]]; then
  echo "$GIT_REMOTE must expose only refs/heads/main and the release-please branch while a release PR is open" >&2
  echo "$remote_heads" >&2
  exit 1
fi

remote_pull_refs="$(git ls-remote "$GIT_REMOTE" 'refs/pull/*' || true)"
if [[ -n "$remote_pull_refs" ]]; then
  remote_main_sha="$(git ls-remote "$GIT_REMOTE" refs/heads/main | awk '{print $1}')"
  unsafe_pull_refs=""

  while read -r pull_sha pull_ref; do
    [[ -n "$pull_sha" ]] || continue

    merge_base_sha="$(
      gh api "repos/$REMOTE_REPO/compare/$remote_main_sha...$pull_sha" \
        --jq '.merge_base_commit.sha' 2>/dev/null || true
    )"
    if [[ ! "$merge_base_sha" =~ ^[0-9a-f]{40}$ ]] \
      || ! git merge-base --is-ancestor "$merge_base_sha" HEAD; then
      unsafe_pull_refs+="$pull_sha $pull_ref (not based on sanitized main history)"$'\n'
      continue
    fi

    docs_paths="$(
      gh api "repos/$REMOTE_REPO/git/trees/$pull_sha?recursive=1" \
        --jq '.tree[].path' 2>/dev/null \
        | rg '^docs(/|$)' || true
    )"
    if [[ -n "$docs_paths" ]]; then
      unsafe_pull_refs+="$pull_sha $pull_ref (head tree exposes docs/)"$'\n'
    fi
  done <<<"$remote_pull_refs"

  if [[ -n "$unsafe_pull_refs" ]]; then
    echo "$GIT_REMOTE exposes GitHub pull refs that can retain pre-sanitized history" >&2
    echo "recreate the private repository or request a GitHub purge before public release approval" >&2
    echo "sanitized main bundle helper: bash scripts/export-sanitized-main-bundle.sh" >&2
    echo "$unsafe_pull_refs" >&2
    exit 1
  fi
fi

remote_tags="$(git ls-remote --tags "$GIT_REMOTE")"
if [[ -n "$remote_tags" ]]; then
  unexpected_tags="$(
    printf '%s\n' "$remote_tags" \
      | awk '{print $2}' \
      | grep -Ev '^refs/tags/nahuali-(cli|core|mcp|api)-v.*(\^\{\})?$' || true
  )"
  if [[ -n "$unexpected_tags" ]]; then
    echo "remote tags outside the private Nahuali release stream are not allowed" >&2
    echo "$unexpected_tags" >&2
    exit 1
  fi
fi

release_check="$(
  gh api "repos/$REMOTE_REPO/releases" \
    --jq '[.[] | select(.prerelease != true)] | length'
)"
if [[ "$release_check" != "0" ]]; then
  echo "only private prereleases are allowed before explicit public release approval" >&2
  gh release list --repo "$REMOTE_REPO" --limit 10 >&2
  exit 1
fi

open_issue_count="$(gh issue list --repo "$REMOTE_REPO" --state open --json number --jq 'length')"
if [[ "$open_issue_count" != "0" ]]; then
  untriaged_count="$(
    gh issue list \
      --repo "$REMOTE_REPO" \
      --state open \
      --json number,labels \
      --jq '[.[] | select(([.labels[].name] | any(. == "release-blocking" or . == "post-release")) | not)] | length'
  )"
  if [[ "$untriaged_count" != "0" ]]; then
    echo "open issues must be labeled release-blocking or post-release" >&2
    gh issue list --repo "$REMOTE_REPO" --state open --json number,title,labels --limit 100 >&2
    exit 1
  fi
fi

tracked_local_artifacts="$(
  git ls-files | rg '(^|/)(\.private|\.local|\.runs|\.nahuali-oss|\.release-dry-run|\.dev-bin|docs)(/|$)|(^|/)\.nahuali-demo\$|\.snapshot\.json$|\.backup\.json$|\.interchange\.json$' || true
)"
if [[ -n "$tracked_local_artifacts" ]]; then
  echo "local-only artifacts are tracked:" >&2
  echo "$tracked_local_artifacts" >&2
  exit 1
fi

public_claim_pattern='(?i)(Nahuali Cloud|public[[:space:]]+release[[:space:]]+(approved|ready)|ships[[:space:]]+with[[:space:]]+hosted|ships[[:space:]]+hosted|includes[[:space:]]+hosted[[:space:]]+operations|includes[[:space:]]+a[[:space:]]+hosted[[:space:]]+service|offers[^.\n]*(hosted|managed|accounts|teams|billing|sync|dashboards)|provides[^.\n]*(hosted|managed|accounts|teams|billing|sync|dashboards)|hosted[[:space:]]+control[[:space:]]+plane[[:space:]]+is[[:space:]]+part[[:space:]]+of|managed[[:space:]]+deployment[[:space:]]+is[[:space:]]+part[[:space:]]+of|accounts[[:space:]]+are[[:space:]]+part[[:space:]]+of|billing[[:space:]]+is[[:space:]]+part[[:space:]]+of|managed[[:space:]]+backup[[:space:]]+automation[[:space:]]+is[[:space:]]+included|point-in-time[[:space:]]+restore[[:space:]]+is[[:space:]]+included|SLA-backed[[:space:]]+recovery[[:space:]]+is[[:space:]]+included)'
if rg -n "$public_claim_pattern" README.md crates packages; then
  echo "public text contains release or hosted-operation claims that need review" >&2
  exit 1
fi

cli_version="$(sed -n 's/^version = "\(.*\)"/\1/p' crates/nahuali-cli/Cargo.toml | head -n 1)"
cli_tag="nahuali-cli-v${cli_version}"
if gh release view "$cli_tag" --repo "$REMOTE_REPO" >/dev/null 2>&1; then
  sh scripts/check-release-assets.sh --repo "$REMOTE_REPO" --tag "$cli_tag"
  bash scripts/verify-release.sh --repo "$REMOTE_REPO" --tag "$cli_tag"

  release_target="$(gh release view "$cli_tag" --repo "$REMOTE_REPO" --json targetCommitish --jq '.targetCommitish')"
  current_head="$(git rev-parse HEAD)"
  if [[ "$release_target" != "$current_head" ]]; then
    ahead_count="$(git rev-list --count "${release_target}..HEAD" 2>/dev/null || echo "unknown")"
    message="release $cli_tag points at $release_target while current HEAD is $current_head ($ahead_count commits ahead)"
    if [[ "${NAHUALI_RELEASE_CANDIDATE_REQUIRE_CURRENT_RELEASE:-0}" == "1" ]]; then
      echo "$message" >&2
      echo "cut a new signed prerelease or unset NAHUALI_RELEASE_CANDIDATE_REQUIRE_CURRENT_RELEASE for local-only RC checks" >&2
      exit 1
    fi
    echo "warning: $message" >&2
  fi
else
  echo "release asset verification skipped; no GitHub release exists for $cli_tag"
fi

bash scripts/security-supply-chain-check.sh
bash scripts/check-doc-release-refs.sh
bash scripts/fresh-clone-validate.sh

echo "release-candidate gate passed"
