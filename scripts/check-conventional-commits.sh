#!/usr/bin/env bash
# Enforce Conventional Commit subject lines.
#
# Usage:
#   scripts/check-conventional-commits.sh <git-range>   # validate a commit range
#   scripts/check-conventional-commits.sh --message FILE # validate one message file
#
# With no argument it validates the HEAD commit. Merge commits are skipped.
# Accepted types must stay in sync with CONTRIBUTING.md and Release Please.
set -euo pipefail

types="feat|fix|docs|style|test|refactor|perf|build|ci|chore|security|revert"
pattern="^(${types})(\([a-z0-9._/-]+\))?!?: .+"

validate_subject() {
  local subject="$1"
  case "$subject" in
    "Merge "*) return 0 ;;
    "Revert "*) return 0 ;;
  esac
  if printf '%s' "$subject" | grep -Eq "$pattern"; then
    return 0
  fi
  return 1
}

if [[ "${1:-}" == "--message" ]]; then
  message_file="${2:?--message requires a file path}"
  subject="$(head -n1 "$message_file")"
  if validate_subject "$subject"; then
    exit 0
  fi
  echo "Commit subject is not a Conventional Commit:" >&2
  echo "  ${subject}" >&2
  echo "Accepted types: ${types//|/, }." >&2
  echo "Example: feat(semantic): add provenance-backed recall scoring" >&2
  exit 1
fi

range="${1:-HEAD~1..HEAD}"
fail=0
while IFS=$'\t' read -r sha subject; do
  if ! validate_subject "$subject"; then
    echo "::error::non-conventional commit ${sha}: ${subject}" >&2
    fail=1
  fi
done < <(git log --no-merges --format='%h%x09%s' "$range")

if [[ "$fail" -ne 0 ]]; then
  echo "Commit subjects must follow Conventional Commits. Accepted types: ${types//|/, }." >&2
  exit 1
fi

echo "All commit subjects follow Conventional Commits."
