#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

OUTPUT=""

usage() {
  cat <<'USAGE'
Usage: bash scripts/export-sanitized-main-bundle.sh [--output PATH]

Creates a verified main-only git bundle from the current sanitized repository
state. The bundle is intended as an input for recreating a repository from the
current main branch without carrying unrelated hidden refs.

Options:
  --output PATH  Bundle output path. Default: .private/repo-transfer/nahuali-main-<sha>.bundle
  -h, --help     Show this help.
USAGE
}

require_command() {
  local command_name="$1"

  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "$command_name is required to export a sanitized main bundle" >&2
    exit 1
  fi
}

absolute_path() {
  local path="$1"
  local dir
  local base

  dir="$(dirname "$path")"
  base="$(basename "$path")"
  mkdir -p "$dir"
  dir="$(cd "$dir" && pwd -P)"
  printf '%s/%s\n' "$dir" "$base"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --output)
      if [[ $# -lt 2 ]]; then
        echo "--output requires a path" >&2
        exit 1
      fi
      OUTPUT="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

require_command git
require_command rg

if [[ -n "$(git status --porcelain)" ]]; then
  echo "sanitized bundle export requires a clean public working tree" >&2
  git status --short >&2
  exit 1
fi

if ! git show-ref --verify --quiet refs/heads/main; then
  echo "local refs/heads/main is required" >&2
  exit 1
fi

current_head="$(git rev-parse HEAD)"
main_head="$(git rev-parse refs/heads/main)"
if [[ "$current_head" != "$main_head" ]]; then
  echo "checkout must be at refs/heads/main before exporting" >&2
  echo "HEAD=$current_head" >&2
  echo "main=$main_head" >&2
  exit 1
fi

tracked_private_paths="$(
  git ls-files | rg '(^|/)(\.private|\.local|\.runs|\.nahuali-oss|\.release-dry-run|\.dev-bin|docs)(/|$)|(^|/)\.nahuali-demo$|\.snapshot\.json$|\.backup\.json$|\.interchange\.json$' || true
)"
if [[ -n "$tracked_private_paths" ]]; then
  echo "tracked local-only artifacts block sanitized export:" >&2
  echo "$tracked_private_paths" >&2
  exit 1
fi

tree_private_paths="$(
  git ls-tree -r --name-only "$current_head" \
    | rg '(^|/)(\.private|\.local|\.runs|\.nahuali-oss|\.release-dry-run|\.dev-bin|docs)(/|$)|(^|/)\.nahuali-demo$|\.snapshot\.json$|\.backup\.json$|\.interchange\.json$' || true
)"
if [[ -n "$tree_private_paths" ]]; then
  echo "current main tree contains local-only artifacts:" >&2
  echo "$tree_private_paths" >&2
  exit 1
fi

short_head="$(git rev-parse --short=12 "$current_head")"
if [[ -z "$OUTPUT" ]]; then
  OUTPUT=".private/repo-transfer/nahuali-main-${short_head}.bundle"
fi

output_abs="$(absolute_path "$OUTPUT")"
root_abs="$(pwd -P)"
case "$output_abs" in
  "$root_abs/.private/"*) ;;
  "$root_abs"/*)
    echo "bundle output inside the repository must be under ignored .private/" >&2
    echo "output=$output_abs" >&2
    exit 1
    ;;
esac

tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

rm -f "$output_abs"
git bundle create "$output_abs" refs/heads/main >/dev/null
git bundle verify "$output_abs" >/dev/null

clone_dir="$tmp_dir/clone"
git clone --quiet --branch main "$output_abs" "$clone_dir"

clone_head="$(git -C "$clone_dir" rev-parse HEAD)"
if [[ "$clone_head" != "$current_head" ]]; then
  echo "bundle clone head mismatch" >&2
  echo "expected=$current_head" >&2
  echo "actual=$clone_head" >&2
  exit 1
fi

clone_private_paths="$(
  git -C "$clone_dir" ls-files \
    | rg '(^|/)(\.private|\.local|\.runs|\.nahuali-oss|\.release-dry-run|\.dev-bin|docs)(/|$)|(^|/)\.nahuali-demo$|\.snapshot\.json$|\.backup\.json$|\.interchange\.json$' || true
)"
if [[ -n "$clone_private_paths" ]]; then
  echo "bundle clone contains local-only artifacts:" >&2
  echo "$clone_private_paths" >&2
  exit 1
fi

echo "sanitized main bundle exported"
echo "bundle=$output_abs"
echo "head=$current_head"
