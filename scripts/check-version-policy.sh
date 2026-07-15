#!/usr/bin/env bash
set -euo pipefail

ROOT="${NAHUALI_WORKSPACE_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
cd "$ROOT"

fail() {
  echo "version-policy: $*" >&2
  exit 1
}

for command_name in cargo jq; do
  command -v "$command_name" >/dev/null 2>&1 \
    || fail "$command_name is required"
done

version="$(tr -d '[:space:]' < version.txt)"
[[ "$version" =~ ^0\.[0-9]+\.[0-9]+-beta\.[0-9]+$ ]] \
  || fail "version.txt must contain a pre-1.0 beta version, got '$version'"

allowed_major="$(jq -r '.allowed_major' release-policy.json)"
allowed_base="$(jq -r '.allowed_base_version' release-policy.json)"
prerelease="$(jq -r '.prerelease' release-policy.json)"
[[ "$allowed_major" == "0" && "$prerelease" == "beta" ]] \
  || fail "release-policy.json must keep the current train on 0.x beta"
[[ "${version%-beta.*}" == "$allowed_base" ]] \
  || fail "version $version leaves the explicitly approved $allowed_base beta train"

workspace_version="$(
  sed -n 's/^version = "\([^"]*\)".*/\1/p' Cargo.toml | head -n 1
)"
[[ "$workspace_version" == "$version" ]] \
  || fail "workspace version $workspace_version does not match version.txt $version"

internal_versions="$(
  sed -n 's/^nahuali-[a-z-]* = { version = "=\([^"]*\)", path = .*/\1/p' Cargo.toml
)"
[[ "$(printf '%s\n' "$internal_versions" | wc -l | tr -d '[:space:]')" == "2" ]] \
  || fail "workspace dependencies must keep exact publishable versions for nahuali-core and nahuali-ui"
while IFS= read -r internal_version; do
  [[ "$internal_version" == "$version" ]] \
    || fail "internal workspace dependency $internal_version does not match $version"
done <<< "$internal_versions"

manifest_version="$(jq -r '.["."]' .release-please-manifest.json)"
[[ "$manifest_version" == "$version" ]] \
  || fail "Release Please manifest $manifest_version does not match $version"

package_count="$(jq '.packages | length' release-please-config.json)"
root_package="$(jq -r '.packages | has(".")' release-please-config.json)"
component_tags="$(jq -r '."include-component-in-tag"' release-please-config.json)"
pre_major_guard="$(jq -r '."bump-minor-pre-major"' release-please-config.json)"
versioning_strategy="$(jq -r '.versioning' release-please-config.json)"
[[ "$package_count" == "1" && "$root_package" == "true" ]] \
  || fail "Release Please must model exactly one root product"
[[ "$component_tags" == "false" ]] \
  || fail "component names must not appear in public release tags"
[[ "$pre_major_guard" == "true" ]] \
  || fail "breaking changes before 1.0 must increment the minor version"
[[ "$versioning_strategy" == "prerelease" ]] \
  || fail "beta commits must increment beta.N until an explicit release decision"

metadata="$(mktemp)"
trap 'rm -f "$metadata"' EXIT
cargo metadata --locked --format-version 1 --no-deps > "$metadata"

mismatches="$(
  jq -r --arg version "$version" '
    .packages[]
    | select(.name | startswith("nahuali-"))
    | select(.version != $version)
    | "\(.name)=\(.version)"
  ' "$metadata"
)"
[[ -z "$mismatches" ]] \
  || fail "workspace package versions disagree: $mismatches"

server_version="$(jq -r '.version' server.json)"
server_image="$(jq -r '.packages[0].identifier' server.json)"
[[ "$server_version" == "$version" ]] \
  || fail "server.json version $server_version does not match $version"
[[ "$server_image" == "ghcr.io/arakiss/nahuali-mcp:$version" ]] \
  || fail "server.json image must use the product version"

grep -q "^## \[$version\]" CHANGELOG.md \
  || fail "CHANGELOG.md needs a curated [$version] product entry"
grep -Fq "release-${version//-/--}" README.md \
  || fail "README.md release badge does not match $version"

benchmark_result="benchmarks/agent-memory-trust/results/nahuali-${version}.json"
[[ -f "$benchmark_result" ]] \
  || fail "published trust benchmark result must match the product version"
benchmark_system_version="$(jq -r '.system.version' "$benchmark_result")"
[[ "$benchmark_system_version" == "nahuali $version" ]] \
  || fail "published trust benchmark reports $benchmark_system_version instead of nahuali $version"
grep -Fq "results/nahuali-${version}.json" benchmarks/agent-memory-trust/README.md \
  || fail "trust benchmark table must link the current product result"

echo "version-policy: $version is one coherent pre-1.0 product release"
