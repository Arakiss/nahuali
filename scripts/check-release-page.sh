#!/usr/bin/env sh
set -eu

repo="${NAHUALI_GITHUB_REPO:-Arakiss/nahuali}"
tag="${NAHUALI_VERSION:-latest}"
json="false"
allow_missing_assets="false"

usage() {
  cat <<'USAGE'
Usage: sh scripts/check-release-page.sh [options]

Options:
  --tag TAG, --version TAG   Release tag to inspect. Default: latest nahuali-cli release.
  --repo OWNER/NAME          GitHub repository. Default: Arakiss/nahuali.
  --json                     Emit machine-readable JSON.
  --allow-missing-assets     Do not fail when binary assets are still uploading.
  -h, --help                 Show this help.

The check treats the GitHub release page as a public product surface, not a raw
Release Please changelog. A curated beta release page must include:
  - product title: "Nahuali vX.Y.Z-beta.N"
  - useful public body, not generated boilerplate
  - Highlights, Install, Verify the release, Component versions, Beta limits,
    and Changelog sections
  - install.sh, check-release-assets.sh, and verify-release.sh references
  - explicit beta limits, including no hosted service claim
  - 12 owned binary-channel assets unless --allow-missing-assets is passed
USAGE
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --tag | --version)
      tag="${2:-}"
      shift 2
      ;;
    --repo)
      repo="${2:-}"
      shift 2
      ;;
    --json)
      json="true"
      shift
      ;;
    --allow-missing-assets)
      allow_missing_assets="true"
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      echo "release-page: unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

command -v gh >/dev/null 2>&1 || {
  echo "release-page: required tool not found: gh" >&2
  exit 2
}

if [ -z "$tag" ]; then
  echo "release-page: empty release tag" >&2
  exit 2
fi

if [ "$tag" = "latest" ]; then
  tag="$(
    gh release list --repo "$repo" --limit 50 --json tagName,publishedAt \
      --jq '[.[] | select(.tagName | startswith("nahuali-cli-v"))] | sort_by(.publishedAt) | last | .tagName'
  )"
  if [ -z "$tag" ] || [ "$tag" = "null" ]; then
    echo "release-page: no nahuali-cli release found in $repo" >&2
    exit 1
  fi
fi

version="${tag#nahuali-cli-v}"
if [ "$version" = "$tag" ] || [ -z "$version" ]; then
  echo "release-page: unsupported nahuali-cli tag: $tag" >&2
  exit 2
fi

body_file="$(mktemp)"
trap 'rm -f "$body_file"' EXIT

name="$(gh release view "$tag" --repo "$repo" --json name --jq '.name // ""')"
url="$(gh release view "$tag" --repo "$repo" --json url --jq '.url // ""')"
is_draft="$(gh release view "$tag" --repo "$repo" --json isDraft --jq '.isDraft')"
is_prerelease="$(gh release view "$tag" --repo "$repo" --json isPrerelease --jq '.isPrerelease')"
asset_count="$(gh release view "$tag" --repo "$repo" --json assets --jq '.assets | length')"
gh release view "$tag" --repo "$repo" --json body --jq '.body // ""' >"$body_file"

failures=""

add_failure() {
  failures="${failures}${failures:+
}$1"
}

require_body_pattern() {
  pattern="$1"
  message="$2"
  if ! grep -Eq "$pattern" "$body_file"; then
    add_failure "$message"
  fi
}

reject_body_pattern() {
  pattern="$1"
  message="$2"
  if grep -Eiq "$pattern" "$body_file"; then
    add_failure "$message"
  fi
}

expected_name="Nahuali v${version}"
if [ "$name" != "$expected_name" ]; then
  add_failure "release title must be '$expected_name'"
fi

if [ "$is_draft" != "false" ]; then
  add_failure "release must be published, not draft"
fi

if [ "$is_prerelease" != "true" ]; then
  add_failure "beta release must be marked as prerelease"
fi

body_bytes="$(wc -c <"$body_file" | tr -d '[:space:]')"
if [ "$body_bytes" -lt 900 ]; then
  add_failure "release body is too short to be curated public copy"
fi

first_content_line="$(awk 'NF { print; exit }' "$body_file")"
case "$first_content_line" in
  "## ["*)
    add_failure "release body starts with a raw generated changelog heading"
    ;;
esac

reject_body_pattern ':robot:|beep boop|I have created a release|This PR was generated with' \
  "release body contains uncurated Release Please boilerplate"

require_body_pattern '^Nahuali v[0-9]+\.[0-9]+\.[0-9]+-beta\.[0-9]+ is a prerelease' \
  "release body must open with a product-focused prerelease summary"
require_body_pattern '^## Highlights$' "release body must include a Highlights section"
require_body_pattern '^## Install$' "release body must include an Install section"
require_body_pattern '^## Verify the release$' "release body must include a Verify the release section"
require_body_pattern '^## Component versions in this release$' "release body must include component versions"
require_body_pattern '^## Beta limits$' "release body must include beta limits"
require_body_pattern '^## Changelog$' "release body must include a changelog pointer"
require_body_pattern 'install\.sh' "release body must reference the installer"
require_body_pattern 'scripts/check-release-assets\.sh' "release body must reference the asset checker"
require_body_pattern 'scripts/verify-release\.sh' "release body must reference the release verifier"
require_body_pattern 'There is no hosted service|No hosted service|no hosted service' \
  "release body must explicitly avoid hosted-service claims"

if [ "$allow_missing_assets" != "true" ] && [ "$asset_count" -lt 12 ]; then
  add_failure "release must expose the 12 binary-channel assets before closeout"
fi

json_string_array() {
  awk '
    BEGIN { printf "["; first = 1 }
    NF {
      gsub(/\\/,"\\\\")
      gsub(/"/,"\\\"")
      if (!first) {
        printf ","
      }
      printf "\"%s\"", $0
      first = 0
    }
    END { printf "]" }
  '
}

status="pass"
if [ -n "$failures" ]; then
  status="fail"
fi

if [ "$json" = "true" ]; then
  failures_json="$(printf '%s\n' "$failures" | json_string_array)"
  printf '{\n'
  printf '  "status": "%s",\n' "$status"
  printf '  "repo": "%s",\n' "$repo"
  printf '  "tag": "%s",\n' "$tag"
  printf '  "name": "%s",\n' "$name"
  printf '  "url": "%s",\n' "$url"
  printf '  "body_bytes": %s,\n' "$body_bytes"
  printf '  "asset_count": %s,\n' "$asset_count"
  printf '  "failures": %s\n' "$failures_json"
  printf '}\n'
else
  echo "release page: $status"
  echo "tag: $tag"
  echo "name: $name"
  echo "assets: $asset_count"
  echo "body-bytes: $body_bytes"
  echo "url: $url"
  if [ -n "$failures" ]; then
    echo "failures:"
    printf '%s\n' "$failures" | sed 's/^/- /'
  fi
fi

case "$status" in
  fail) exit 1 ;;
  *) exit 0 ;;
esac
