#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

DIST_DIR=""
KEEP=0

usage() {
  cat <<'USAGE'
Usage: bash scripts/release-dry-run.sh [--dist-dir PATH] [--keep]

Builds local release-candidate artifacts without publishing crates, tags,
or GitHub releases. When --dist-dir is omitted, artifacts are written to a
temporary directory and removed on exit unless --keep is provided.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dist-dir)
      if [[ $# -lt 2 ]]; then
        echo "--dist-dir requires a path" >&2
        exit 1
      fi
      DIST_DIR="$2"
      shift 2
      ;;
    --keep)
      KEEP=1
      shift
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

if [[ -z "$DIST_DIR" ]]; then
  DIST_DIR="$(mktemp -d)"
  if [[ "$KEEP" -eq 0 ]]; then
    trap 'rm -rf "$DIST_DIR"' EXIT
  fi
else
  mkdir -p "$DIST_DIR"
fi

version="$(tr -d '[:space:]' < version.txt)"
if [[ -z "$version" ]]; then
  echo "failed to read the product version from version.txt" >&2
  exit 1
fi

host_triple="$(rustc -vV | sed -n 's/^host: //p')"
if [[ -z "$host_triple" ]]; then
  echo "failed to read host target from rustc" >&2
  exit 1
fi

package_order=(nahuali-core nahuali-cli nahuali-mcp nahuali-api)
package_status=()
for package in "${package_order[@]}"; do
  package_log="$DIST_DIR/${package}.cargo-package.log"
  if cargo package -p "$package" --allow-dirty --no-verify > "$package_log" 2>&1; then
    package_status+=("${package}=packaged")
    continue
  fi

  if [[ "$package" != "nahuali-core" ]] \
    && grep -Eq 'no matching package named `nahuali-(core|ui)` found' "$package_log"; then
    cargo package -p "$package" --allow-dirty --list > "$DIST_DIR/${package}.package-files.txt"
    package_status+=("${package}=blocked_until_nahuali-core_registry")
    continue
  fi

  cat "$package_log" >&2
  exit 1
done

# Mirror the release workflow's feature set so the dry-run proves the same
# artifacts the tag build will ship.
cargo build --release -p nahuali-cli -p nahuali-mcp -p nahuali-api -p nahuali-regression --features nahuali-cli/attestation,nahuali-mcp/tamper-evidence,nahuali-api/tamper-evidence

target_dir="${CARGO_TARGET_DIR:-target}"
release_dir="$target_dir/release"
artifact_name="nahuali-v${version}-${host_triple}"
artifact_dir="$DIST_DIR/$artifact_name"
archive_path="$DIST_DIR/${artifact_name}.tar.gz"
checksum_path="$archive_path.sha256"

rm -rf "$artifact_dir" "$archive_path" "$checksum_path"
mkdir -p "$artifact_dir/bin"
cp "$release_dir/nahuali" "$artifact_dir/bin/nahuali"
cp "$release_dir/nahuali-mcp" "$artifact_dir/bin/nahuali-mcp"
cp "$release_dir/nahuali-api" "$artifact_dir/bin/nahuali-api"
chmod +x "$artifact_dir/bin/nahuali" "$artifact_dir/bin/nahuali-mcp" "$artifact_dir/bin/nahuali-api"

{
  printf 'name=nahuali\n'
  printf 'version=%s\n' "$version"
  printf 'target=%s\n' "$host_triple"
  printf 'archive=%s\n' "${artifact_name}.tar.gz"
  printf 'binaries=nahuali,nahuali-mcp,nahuali-api\n'
  printf 'package_order=%s\n' "${package_order[*]}"
  printf 'package_status=%s\n' "${package_status[*]}"
  printf 'publication=none\n'
} > "$artifact_dir/MANIFEST.txt"

tar -czf "$archive_path" -C "$DIST_DIR" "$artifact_name"

if command -v sha256sum >/dev/null 2>&1; then
  sha256sum "$archive_path" > "$checksum_path"
else
  shasum -a 256 "$archive_path" > "$checksum_path"
fi

echo "release dry run passed"
echo "dist_dir=$DIST_DIR"
echo "archive=$archive_path"
echo "checksum=$checksum_path"
