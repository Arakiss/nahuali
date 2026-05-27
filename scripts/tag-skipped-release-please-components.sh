#!/bin/sh
set -eu

mode="write"
remote="${NAHUALI_TAG_REMOTE:-origin}"
target="${NAHUALI_TAG_TARGET:-${GITHUB_SHA:-HEAD}}"
config_file="${NAHUALI_RELEASE_PLEASE_CONFIG:-release-please-config.json}"
manifest_file="${NAHUALI_RELEASE_PLEASE_MANIFEST:-.release-please-manifest.json}"

usage() {
  cat <<'EOF'
Usage: sh scripts/tag-skipped-release-please-components.sh [--dry-run|--check] [--target <ref>] [--remote <name>]

Create lightweight git tags for release-please packages that set
skip-github-release=true. These tags are intentionally not GitHub Releases:
they give release-please a stable previous-release boundary while keeping the
public Releases tab focused on the installable Nahuali product.

--dry-run       print the missing tags without creating them
--check         fail if any expected tag is missing
--target <ref>  tag this git ref instead of GITHUB_SHA or HEAD
--remote <name> push/fetch tags from this remote (default: origin)
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --dry-run)
      mode="dry-run"
      shift
      ;;
    --check)
      mode="check"
      shift
      ;;
    --target)
      if [ "$#" -lt 2 ]; then
        echo "missing value for --target" >&2
        exit 2
      fi
      target="$2"
      shift 2
      ;;
    --remote)
      if [ "$#" -lt 2 ]; then
        echo "missing value for --remote" >&2
        exit 2
      fi
      remote="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      usage >&2
      exit 2
      ;;
  esac
done

repo_root="${NAHUALI_WORKSPACE_ROOT:-}"
if [ -z "$repo_root" ]; then
  repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
fi

cd "$repo_root"

tags_file="$(mktemp)"
trap 'rm -f "$tags_file"' EXIT

NAHUALI_RELEASE_PLEASE_CONFIG="$config_file" \
NAHUALI_RELEASE_PLEASE_MANIFEST="$manifest_file" \
ruby > "$tags_file" <<'RUBY'
require "json"

config_path = ENV.fetch("NAHUALI_RELEASE_PLEASE_CONFIG")
manifest_path = ENV.fetch("NAHUALI_RELEASE_PLEASE_MANIFEST")
config = JSON.parse(File.read(config_path))
manifest = JSON.parse(File.read(manifest_path))

global_component_in_tag = config.fetch("include-component-in-tag", true)
global_v_in_tag = config.fetch("include-v-in-tag", true)
global_separator = config.fetch("tag-separator", "-")

config.fetch("packages", {}).each do |path, package|
  next unless package["skip-github-release"]

  version = manifest[path]
  next unless version

  component = package["component"] || package["package-name"] || path
  component_in_tag = package.fetch("include-component-in-tag", global_component_in_tag)
  v_in_tag = package.fetch("include-v-in-tag", global_v_in_tag)
  separator = package.fetch("tag-separator", global_separator)
  version_part = "#{v_in_tag ? "v" : ""}#{version}"

  tag = component_in_tag ? "#{component}#{separator}#{version_part}" : version_part
  puts tag
end
RUBY

if [ ! -s "$tags_file" ]; then
  echo "no skipped release-please components need internal tags"
  exit 0
fi

git fetch --quiet --tags "$remote"

missing=0
while IFS= read -r tag; do
  [ -n "$tag" ] || continue

  if git rev-parse -q --verify "refs/tags/$tag" >/dev/null; then
    echo "ok internal tag exists: $tag"
    continue
  fi

  missing=1
  case "$mode" in
    check)
      echo "missing internal release tag: $tag" >&2
      ;;
    dry-run)
      echo "plan internal tag: $tag -> $target"
      ;;
    write)
      git tag "$tag" "$target"
      git push "$remote" "refs/tags/$tag"
      echo "ok internal tag created: $tag -> $target"
      ;;
  esac
done < "$tags_file"

if [ "$mode" = "check" ] && [ "$missing" -ne 0 ]; then
  exit 1
fi
