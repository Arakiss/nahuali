#!/usr/bin/env bash
set -euo pipefail

repo="${NAHUALI_GITHUB_REPO:-Arakiss/nahuali}"
tag="${NAHUALI_VERSION:-latest}"
asset="${NAHUALI_RELEASE_ASSET:-auto}"
json="false"
require_sbom="false"
require_provenance="false"
download_dir=""

usage() {
  cat <<'USAGE'
Usage: bash scripts/verify-release.sh [options]

Options:
  --tag TAG, --version TAG    Release tag to verify. Default: latest nahuali-cli release.
  --repo OWNER/NAME           GitHub repository. Default: Arakiss/nahuali.
  --asset NAME                Archive asset to verify. Default: current OS/arch.
  --dir DIR                   Download assets into DIR instead of a temp directory.
  --json                      Emit machine-readable JSON.
  --require-sbom              Fail if nahuali-<tag>.cdx.json is missing.
  --require-provenance        Fail if GitHub artifact attestation verification is missing.
  --require-attestation       Alias for --require-provenance.
  -h, --help                  Show this help.

The verifier downloads the current-platform release archive, checksum, and
Sigstore bundle. It verifies SHA-256, Cosign/Sigstore identity, and runs the
repository install smoke against the extracted nahuali, nahuali-mcp, and
nahuali-api binaries. SBOM and provenance checks are warnings unless required.
USAGE
}

die() {
  echo "verify-release: $*" >&2
  exit 2
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
    --asset)
      asset="${2:-}"
      shift 2
      ;;
    --dir)
      download_dir="${2:-}"
      shift 2
      ;;
    --json)
      json="true"
      shift
      ;;
    --require-sbom)
      require_sbom="true"
      shift
      ;;
    --require-provenance | --require-attestation)
      require_provenance="true"
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      echo "verify-release: unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

[[ -n "$tag" ]] || die "empty release tag"
command -v gh >/dev/null 2>&1 || die "required tool not found: gh"
command -v cosign >/dev/null 2>&1 || die "required tool not found: cosign"
command -v tar >/dev/null 2>&1 || die "required tool not found: tar"

if ! command -v shasum >/dev/null 2>&1 && ! command -v sha256sum >/dev/null 2>&1; then
  die "required tool not found: shasum or sha256sum"
fi

if [[ "$tag" == "latest" ]]; then
  tag="$(
    gh release list --repo "$repo" --limit 50 --json tagName,publishedAt \
      --jq '[.[] | select(.tagName | startswith("nahuali-cli-v"))] | sort_by(.publishedAt) | last | .tagName'
  )"
  if [[ -z "$tag" || "$tag" == "null" ]]; then
    echo "verify-release: no nahuali-cli release found in $repo" >&2
    exit 1
  fi
fi

version="${tag#nahuali-cli-v}"
if [[ "$version" == "$tag" || -z "$version" ]]; then
  die "unsupported nahuali-cli tag: $tag"
fi

detect_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$os:$arch" in
    Darwin:arm64 | Darwin:aarch64) echo "aarch64-apple-darwin" ;;
    Darwin:x86_64) echo "x86_64-apple-darwin" ;;
    Linux:arm64 | Linux:aarch64) echo "aarch64-unknown-linux-gnu" ;;
    Linux:x86_64) echo "x86_64-unknown-linux-gnu" ;;
    *) return 1 ;;
  esac
}

if [[ "$asset" == "auto" ]]; then
  target="$(detect_target)" || die "unsupported current platform for automatic asset selection: $(uname -s)/$(uname -m)"
  asset="nahuali-v${version}-${target}.tar.gz"
fi

expected_asset="false"
for target in \
  aarch64-apple-darwin \
  x86_64-apple-darwin \
  aarch64-unknown-linux-gnu \
  x86_64-unknown-linux-gnu
do
  if [[ "$asset" == "nahuali-v${version}-${target}.tar.gz" ]]; then
    expected_asset="true"
  fi
done
[[ "$expected_asset" == "true" ]] || die "unsupported release archive asset for $tag: $asset"

if [[ -n "$download_dir" ]]; then
  mkdir -p "$download_dir"
  tmp_dir="$download_dir"
  cleanup="false"
else
  tmp_dir="$(mktemp -d)"
  cleanup="true"
fi

if [[ "$cleanup" == "true" ]]; then
  trap 'rm -rf "$tmp_dir"' EXIT
fi

asset_names="$(gh release view "$tag" --repo "$repo" --json assets --jq '.assets[].name')"
sbom_asset="nahuali-$tag.cdx.json"
sbom_status="missing"
if printf '%s\n' "$asset_names" | grep -Fx "$sbom_asset" >/dev/null 2>&1; then
  sbom_status="pass"
fi

gh release download "$tag" \
  --repo "$repo" \
  --dir "$tmp_dir" \
  --clobber \
  --pattern "$asset" \
  --pattern "$asset.sha256" \
  --pattern "$asset.sigstore.json" >/dev/null

if [[ "$sbom_status" == "pass" ]]; then
  gh release download "$tag" \
    --repo "$repo" \
    --dir "$tmp_dir" \
    --clobber \
    --pattern "$sbom_asset" >/dev/null
fi

checksum_status="fail"
if command -v shasum >/dev/null 2>&1; then
  if (cd "$tmp_dir" && shasum -a 256 -c "$asset.sha256" >/dev/null 2>&1); then
    checksum_status="pass"
  fi
else
  if (cd "$tmp_dir" && sha256sum -c "$asset.sha256" >/dev/null 2>&1); then
    checksum_status="pass"
  fi
fi

identity="https://github.com/$repo/.github/workflows/release.yml@refs/tags/$tag"
sigstore_status="fail"
if cosign verify-blob "$tmp_dir/$asset" \
  --bundle "$tmp_dir/$asset.sigstore.json" \
  --certificate-identity "$identity" \
  --certificate-oidc-issuer "https://token.actions.githubusercontent.com" >/dev/null 2>&1; then
  sigstore_status="pass"
fi

provenance_status="missing"
if gh attestation verify "$tmp_dir/$asset" \
  --repo "$repo" \
  --cert-identity "$identity" \
  --cert-oidc-issuer "https://token.actions.githubusercontent.com" \
  --source-ref "refs/tags/$tag" >/dev/null 2>&1; then
  provenance_status="pass"
fi

extract_dir="$tmp_dir/extracted"
mkdir -p "$extract_dir"
install_smoke_status="fail"
if tar -xzf "$tmp_dir/$asset" -C "$extract_dir" >/dev/null 2>&1; then
  nahuali_bin="$(find "$extract_dir" -type f -path "*/bin/nahuali" | head -n 1)"
  if [[ -n "$nahuali_bin" ]]; then
    bin_dir="$(dirname "$nahuali_bin")"
    if env \
      NAHUALI_VALIDATE_SKIP_DEV_STACK=1 \
      NAHUALI_VERIFY_INSTALL_BIN_DIR="$bin_dir" \
      bash scripts/verify-install.sh >/dev/null 2>&1; then
      install_smoke_status="pass"
    fi
  fi
fi

status="pass"
if [[ "$checksum_status" != "pass" || "$sigstore_status" != "pass" || "$install_smoke_status" != "pass" ]]; then
  status="fail"
elif [[ "$require_sbom" == "true" && "$sbom_status" != "pass" ]]; then
  status="fail"
elif [[ "$require_provenance" == "true" && "$provenance_status" != "pass" ]]; then
  status="fail"
elif [[ "$sbom_status" != "pass" || "$provenance_status" != "pass" ]]; then
  status="warn"
fi

json_string() {
  printf '%s' "$1" | awk '
    BEGIN { printf "\"" }
    {
      gsub(/\\/,"\\\\")
      gsub(/"/,"\\\"")
      gsub(/\t/,"\\t")
      printf "%s", $0
    }
    END { printf "\"" }
  '
}

if [[ "$json" == "true" ]]; then
  printf '{\n'
  printf '  "status": "%s",\n' "$status"
  printf '  "repo": %s,\n' "$(json_string "$repo")"
  printf '  "tag": %s,\n' "$(json_string "$tag")"
  printf '  "asset": %s,\n' "$(json_string "$asset")"
  printf '  "download_dir": %s,\n' "$(json_string "$tmp_dir")"
  printf '  "checks": {\n'
  printf '    "sha256": "%s",\n' "$checksum_status"
  printf '    "sigstore_bundle": "%s",\n' "$sigstore_status"
  printf '    "install_smoke": "%s",\n' "$install_smoke_status"
  printf '    "cyclonedx_sbom": "%s",\n' "$sbom_status"
  printf '    "github_artifact_attestation": "%s"\n' "$provenance_status"
  printf '  },\n'
  printf '  "expected_identity": %s\n' "$(json_string "$identity")"
  printf '}\n'
else
  echo "release verification: $status"
  echo "repo: $repo"
  echo "tag: $tag"
  echo "asset: $asset"
  echo "sha256: $checksum_status"
  echo "sigstore bundle: $sigstore_status"
  echo "install smoke: $install_smoke_status"
  echo "CycloneDX SBOM: $sbom_status"
  echo "GitHub artifact attestation: $provenance_status"
  echo "expected identity: $identity"
  if [[ "$cleanup" == "false" ]]; then
    echo "download dir: $tmp_dir"
  fi
fi

if [[ "$status" == "fail" ]]; then
  exit 1
fi
