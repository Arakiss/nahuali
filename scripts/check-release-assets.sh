#!/usr/bin/env sh
set -eu

repo="${NAHUALI_GITHUB_REPO:-Arakiss/nahuali}"
tag="${NAHUALI_VERSION:-latest}"
json="false"

usage() {
  cat <<'USAGE'
Usage: sh scripts/check-release-assets.sh [options]

Options:
  --tag TAG, --version TAG  Release tag to inspect. Default: latest product release.
  --repo OWNER/NAME         GitHub repository. Default: Arakiss/nahuali.
  --json                    Emit machine-readable JSON.
  --require-sbom            Compatibility flag; the SBOM is always required.
  -h, --help                Show this help.

The check expects the current beta release channel shape:
  - 4 platform archives
  - 4 .sha256 checksum files
  - 4 .sigstore.json Sigstore bundles
  - required CycloneDX SBOM: nahuali-<tag>.cdx.json

Unknown extra assets are reported as warnings but do not fail the check.
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
    --require-sbom)
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      echo "release-assets: unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

command -v gh >/dev/null 2>&1 || {
  echo "release-assets: required tool not found: gh" >&2
  exit 2
}

if [ -z "$tag" ]; then
  echo "release-assets: empty release tag" >&2
  exit 2
fi

if [ "$tag" = "latest" ]; then
  tag="$(
    gh release list --repo "$repo" --limit 50 --json tagName,publishedAt \
      --jq '[.[] | select(.tagName | startswith("v"))] | sort_by(.publishedAt) | last | .tagName'
  )"
  if [ -z "$tag" ] || [ "$tag" = "null" ]; then
    echo "release-assets: no Nahuali product release found in $repo" >&2
    exit 1
  fi
fi

version="${tag#v}"
if [ "$version" = "$tag" ] || [ -z "$version" ]; then
  echo "release-assets: unsupported product tag: $tag" >&2
  exit 2
fi

asset_names="$(gh release view "$tag" --repo "$repo" --json assets --jq '.assets[].name')"
asset_count="$(printf '%s\n' "$asset_names" | awk 'NF { count += 1 } END { print count + 0 }')"
archive_count="$(printf '%s\n' "$asset_names" | awk '/^nahuali-v[^[:space:]]+-(aarch64-apple-darwin|x86_64-apple-darwin|aarch64-unknown-linux-gnu|x86_64-unknown-linux-gnu)\.tar\.gz$/ { count += 1 } END { print count + 0 }')"
checksum_count="$(printf '%s\n' "$asset_names" | awk '/^nahuali-v[^[:space:]]+-(aarch64-apple-darwin|x86_64-apple-darwin|aarch64-unknown-linux-gnu|x86_64-unknown-linux-gnu)\.tar\.gz\.sha256$/ { count += 1 } END { print count + 0 }')"
sigstore_count="$(printf '%s\n' "$asset_names" | awk '/^nahuali-v[^[:space:]]+-(aarch64-apple-darwin|x86_64-apple-darwin|aarch64-unknown-linux-gnu|x86_64-unknown-linux-gnu)\.tar\.gz\.sigstore\.json$/ { count += 1 } END { print count + 0 }')"
sbom_asset="nahuali-$tag.cdx.json"
if printf '%s\n' "$asset_names" | grep -Fx "$sbom_asset" >/dev/null 2>&1; then
  sbom_count=1
else
  sbom_count=0
fi

expected_assets=""
for target in \
  aarch64-apple-darwin \
  x86_64-apple-darwin \
  aarch64-unknown-linux-gnu \
  x86_64-unknown-linux-gnu
do
  archive="nahuali-v${version}-${target}.tar.gz"
  expected_assets="${expected_assets}${archive}
${archive}.sha256
${archive}.sigstore.json
"
done

missing=""
for expected in $expected_assets; do
  if ! printf '%s\n' "$asset_names" | grep -Fx "$expected" >/dev/null 2>&1; then
    missing="${missing}${missing:+
}$expected"
  fi
done
if [ "$sbom_count" -ne 1 ]; then
  missing="${missing}${missing:+
}$sbom_asset"
fi

unexpected=""
for asset in $asset_names; do
  if ! printf '%s\n' "$expected_assets" | grep -Fx "$asset" >/dev/null 2>&1 \
    && [ "$asset" != "$sbom_asset" ]; then
    unexpected="${unexpected}${unexpected:+
}$asset"
  fi
done

status="pass"
if [ -n "$missing" ] || [ "$archive_count" -ne 4 ] || [ "$checksum_count" -ne 4 ] || [ "$sigstore_count" -ne 4 ] || [ "$sbom_count" -ne 1 ]; then
  status="fail"
elif [ -n "$unexpected" ]; then
  status="warn"
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

if [ "$json" = "true" ]; then
  missing_json="$(printf '%s\n' "$missing" | json_string_array)"
  unexpected_json="$(printf '%s\n' "$unexpected" | json_string_array)"
  release_summary="$(
    gh release view "$tag" --repo "$repo" \
      --json tagName,name,isPrerelease,publishedAt,targetCommitish,url \
      --jq '{tagName,name,isPrerelease,publishedAt,targetCommitish,url}'
  )"
  printf '{\n'
  printf '  "status": "%s",\n' "$status"
  printf '  "release": %s,\n' "$release_summary"
  printf '  "counts": {\n'
  printf '    "assets": %s,\n' "$asset_count"
  printf '    "archives": %s,\n' "$archive_count"
  printf '    "checksums": %s,\n' "$checksum_count"
  printf '    "sigstore_bundles": %s,\n' "$sigstore_count"
  printf '    "cyclonedx_sbom": %s\n' "$sbom_count"
  printf '  },\n'
  printf '  "expected": {\n'
  printf '    "required_assets": 13,\n'
  printf '    "archives": 4,\n'
  printf '    "checksums": 4,\n'
  printf '    "sigstore_bundles": 4,\n'
  printf '    "cyclonedx_sbom": "required"\n'
  printf '  },\n'
  printf '  "missing": %s,\n' "$missing_json"
  printf '  "unexpected": %s\n' "$unexpected_json"
  printf '}\n'
else
  release_name="$(gh release view "$tag" --repo "$repo" --json name --jq '.name')"
  release_url="$(gh release view "$tag" --repo "$repo" --json url --jq '.url')"
  release_target="$(gh release view "$tag" --repo "$repo" --json targetCommitish --jq '.targetCommitish')"
  echo "release assets: $status"
  echo "tag: $tag"
  echo "name: $release_name"
  echo "target: $release_target"
  echo "assets: $asset_count total, $archive_count archives, $checksum_count checksums, $sigstore_count sigstore bundles, $sbom_count CycloneDX SBOM"
  echo "url: $release_url"
  if [ "$sbom_count" -ne 1 ]; then
    echo "missing required CycloneDX SBOM: $sbom_asset"
  fi
  if [ -n "$missing" ]; then
    echo "missing:"
    printf '%s\n' "$missing" | sed 's/^/- /'
  fi
  if [ -n "$unexpected" ]; then
    echo "unexpected:"
    printf '%s\n' "$unexpected" | sed 's/^/- /'
  fi
fi

case "$status" in
  fail) exit 1 ;;
  *) exit 0 ;;
esac
