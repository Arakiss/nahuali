#!/usr/bin/env bash
# Hermetic check of the installer's signature reporting.
#
# Serves a fake release from a local directory through a stub `curl`, runs
# scripts/install.sh with a PATH that does (or does not) contain `cosign`, and
# asserts the installer says plainly whether the signature was verified, both
# when it happens and in the final summary.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

TAG="v0.0.0-beta.1"
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) TARGET="x86_64-unknown-linux-gnu" ;;
  Linux-aarch64) TARGET="aarch64-unknown-linux-gnu" ;;
  Darwin-x86_64) TARGET="x86_64-apple-darwin" ;;
  Darwin-arm64) TARGET="aarch64-apple-darwin" ;;
  *) echo "unsupported platform for this check" >&2; exit 1 ;;
esac
ARCHIVE="nahuali-${TAG}-${TARGET}.tar.gz"
RELEASE="$WORK/release"
mkdir -p "$RELEASE" "$WORK/pkg/nahuali-${TAG}-${TARGET}/bin"
for binary in nahuali nahuali-mcp nahuali-api; do
  printf '#!/bin/sh\necho "%s 0.0.0-beta.1"\n' "$binary" >"$WORK/pkg/nahuali-${TAG}-${TARGET}/bin/$binary"
  chmod +x "$WORK/pkg/nahuali-${TAG}-${TARGET}/bin/$binary"
done
tar -czf "$RELEASE/$ARCHIVE" -C "$WORK/pkg" "nahuali-${TAG}-${TARGET}"
if command -v sha256sum >/dev/null 2>&1; then
  (cd "$RELEASE" && sha256sum "$ARCHIVE" >"$ARCHIVE.sha256")
else
  (cd "$RELEASE" && shasum -a 256 "$ARCHIVE" >"$ARCHIVE.sha256")
fi
printf '{}\n' >"$RELEASE/$ARCHIVE.sigstore.json"

# A PATH with only the tools the installer needs, a stub curl, and no cosign.
TOOLS="$WORK/tools"
mkdir -p "$TOOLS"
for tool in sh awk basename cat chmod cp grep mkdir mktemp mv rm sed tar gzip uname sha256sum shasum pgrep; do
  if path="$(command -v "$tool" 2>/dev/null)"; then
    ln -sf "$path" "$TOOLS/$tool"
  fi
done
cat >"$TOOLS/curl" <<EOF
#!/bin/sh
dest=""
url=""
while [ "\$#" -gt 0 ]; do
  case "\$1" in
    -o) dest="\$2"; shift 2 ;;
    --proto|--tlsv1.2) [ "\$1" = "--proto" ] && shift 2 || shift ;;
    -*) shift ;;
    *) url="\$1"; shift ;;
  esac
done
file="$RELEASE/\${url##*/}"
[ -f "\$file" ] || exit 22
if [ -n "\$dest" ]; then cp "\$file" "\$dest"; else cat "\$file"; fi
EOF
chmod +x "$TOOLS/curl"

run_installer() {
  local name="$1"
  shift
  env -i HOME="$WORK/home-$name" PATH="$TOOLS" NAHUALI_VERSION="$TAG" \
    NAHUALI_INSTALL_DIR="$WORK/install-$name" NAHUALI_NO_COLOR=1 "$@" \
    sh "$ROOT/scripts/install.sh" >"$WORK/$name.out" 2>"$WORK/$name.err"
}

fail_check() {
  echo "installer signature notice: $1" >&2
  echo "--- stdout" >&2
  cat "$WORK/$2.out" >&2
  echo "--- stderr" >&2
  cat "$WORK/$2.err" >&2
  exit 1
}

# 1. cosign missing: the install proceeds on the checksum, and both the step and
#    the final summary say the signature was NOT verified and how to require it.
run_installer no-cosign || fail_check "install without cosign should succeed" no-cosign
grep -q "cosign not found. SIGNATURE NOT VERIFIED (checksum verified)" "$WORK/no-cosign.err" \
  || fail_check "missing the in-step warning" no-cosign
grep -q "^Signature: NOT verified (cosign not installed); only the SHA-256 checksum was checked\.$" "$WORK/no-cosign.out" \
  || fail_check "missing the summary line" no-cosign
grep -q "NAHUALI_REQUIRE_SIGSTORE=1" "$WORK/no-cosign.out" \
  || fail_check "summary does not say how to require verification" no-cosign

# 2. cosign required but missing: refuse to install.
if run_installer require-cosign NAHUALI_REQUIRE_SIGSTORE=1; then
  fail_check "NAHUALI_REQUIRE_SIGSTORE=1 without cosign must fail" require-cosign
fi
grep -q "NAHUALI_REQUIRE_SIGSTORE=1 but 'cosign' was not found on PATH" "$WORK/require-cosign.err" \
  || fail_check "missing the required-cosign error" require-cosign
[[ ! -e "$WORK/install-require-cosign/bin/nahuali" ]] \
  || fail_check "nothing may be installed when verification is required and impossible" require-cosign

# 3. cosign present and verification succeeds: the summary says so.
printf '#!/bin/sh\nexit 0\n' >"$TOOLS/cosign"
chmod +x "$TOOLS/cosign"
run_installer with-cosign || fail_check "install with cosign should succeed" with-cosign
grep -q "^Signature: verified (Sigstore, ${TAG})\.$" "$WORK/with-cosign.out" \
  || fail_check "summary does not report the verified signature" with-cosign

echo "installer signature notice: ok"
