#!/bin/sh
# Nahuali installer.
#
# One-line install:
#   curl -fsSL https://raw.githubusercontent.com/Arakiss/nahuali/main/scripts/install.sh | sh
#
# Installs the prebuilt `nahuali`, `nahuali-mcp`, and `nahuali-api` binaries
# from the latest GitHub release into ~/.nahuali/bin. No Rust, Docker, or build
# toolchain is required to RUN this installer.
#
# The installer is idempotent and safe to re-run. It never edits your shell rc
# files; instead it prints the exact PATH line for you to add.
#
# The download is verified before anything is installed. The SHA-256 checksum is
# mandatory: a missing checksum asset aborts the install. When `cosign` is on
# PATH, the release's Sigstore signature is also verified against the GitHub
# Actions release identity; set NAHUALI_REQUIRE_SIGSTORE=1 to make that mandatory.
#
# Tunable via environment variables (all optional):
#   NAHUALI_GITHUB_REPO       GitHub repo to install from   (default: Arakiss/nahuali)
#   NAHUALI_VERSION           Release tag to install         (default: latest binary release)
#   NAHUALI_INSTALL_DIR       Install root                   (default: $HOME/.nahuali)
#   NAHUALI_NO_COLOR          Set to disable colored output  (default: color when TTY)
#   NAHUALI_REQUIRE_SIGSTORE  Set to 1 to require cosign signature verification
#
# This script targets POSIX sh and works under dash, ash (BusyBox), and bash.
set -eu

REPO="${NAHUALI_GITHUB_REPO:-Arakiss/nahuali}"
REQUESTED_VERSION="${NAHUALI_VERSION:-}"
INSTALL_DIR="${NAHUALI_INSTALL_DIR:-${HOME}/.nahuali}"
BIN_DIR="${INSTALL_DIR}/bin"
API_BASE="https://api.github.com/repos/${REPO}"
DOWNLOAD_BASE="https://github.com/${REPO}/releases/download"

# ---------------------------------------------------------------------------
# Presentation: color when attached to a terminal, plain otherwise.
# ---------------------------------------------------------------------------
if [ -t 1 ] && [ -t 2 ] && [ -z "${NAHUALI_NO_COLOR:-}" ] && [ "${TERM:-dumb}" != "dumb" ]; then
  C_RESET="$(printf '\033[0m')"
  C_BOLD="$(printf '\033[1m')"
  C_DIM="$(printf '\033[2m')"
  C_BLUE="$(printf '\033[34m')"
  C_GREEN="$(printf '\033[32m')"
  C_YELLOW="$(printf '\033[33m')"
  C_RED="$(printf '\033[31m')"
  C_CYAN="$(printf '\033[36m')"
else
  C_RESET="" C_BOLD="" C_DIM="" C_BLUE="" C_GREEN="" C_YELLOW="" C_RED="" C_CYAN=""
fi

step()  { printf '%s==>%s %s\n' "${C_BLUE}${C_BOLD}" "${C_RESET}" "$1"; }
info()  { printf '    %s\n' "$1"; }
ok()    { printf '%s  ok%s %s\n' "${C_GREEN}${C_BOLD}" "${C_RESET}" "$1"; }
warn()  { printf '%swarn%s %s\n' "${C_YELLOW}${C_BOLD}" "${C_RESET}" "$1" >&2; }

fail() {
  printf '\n%serror%s %s\n' "${C_RED}${C_BOLD}" "${C_RESET}" "$1" >&2
  if [ "$#" -gt 1 ]; then
    shift
    for line in "$@"; do
      printf '      %s\n' "$line" >&2
    done
  fi
  exit 1
}

# A scratch directory we always clean up, even on failure or interrupt.
TMP_DIR=""
cleanup() {
  [ -n "$TMP_DIR" ] && rm -rf "$TMP_DIR"
}
trap cleanup EXIT
trap 'cleanup; exit 130' INT
trap 'cleanup; exit 143' TERM

# ---------------------------------------------------------------------------
# Preflight: confirm the few tools we rely on are present.
# ---------------------------------------------------------------------------
have() { command -v "$1" >/dev/null 2>&1; }

DOWNLOADER=""
if have curl; then
  DOWNLOADER="curl"
elif have wget; then
  DOWNLOADER="wget"
else
  fail "need either curl or wget to download Nahuali, but found neither." \
    "Install curl (e.g. 'brew install curl' or 'apt-get install curl') and re-run."
fi

have tar || fail "need 'tar' to unpack the release archive, but it was not found." \
  "Install tar through your package manager and re-run."

# Pick a SHA-256 checker (GNU coreutils or BSD/macOS shasum).
SHA_CMD=""
if have sha256sum; then
  SHA_CMD="sha256sum"
elif have shasum; then
  SHA_CMD="shasum -a 256"
fi

# fetch_to URL DEST  -> download URL to DEST, returning non-zero on HTTP errors.
# Callers surface their own friendly message, so the downloader's own stderr is
# suppressed to keep output clean.
fetch_to() {
  _url="$1"
  _dest="$2"
  if [ "$DOWNLOADER" = "curl" ]; then
    curl -fsSL --proto '=https' --tlsv1.2 -o "$_dest" "$_url" 2>/dev/null
  else
    wget -q -O "$_dest" "$_url" 2>/dev/null
  fi
}

# fetch_stdout URL  -> print URL body to stdout.
fetch_stdout() {
  _url="$1"
  if [ "$DOWNLOADER" = "curl" ]; then
    curl -fsSL --proto '=https' --tlsv1.2 "$_url"
  else
    wget -q -O - "$_url"
  fi
}

# ---------------------------------------------------------------------------
# Detect OS + arch and map to a Rust release target triple.
# ---------------------------------------------------------------------------
detect_target() {
  _os="$(uname -s 2>/dev/null || echo unknown)"
  _arch="$(uname -m 2>/dev/null || echo unknown)"
  case "$_os" in
    Darwin) _os_part="apple-darwin" ;;
    Linux)  _os_part="unknown-linux-gnu" ;;
    *)
      fail "unsupported operating system: ${_os}." \
        "Nahuali ships prebuilt binaries for macOS and Linux only." \
        "On other systems, build from source: https://github.com/${REPO}#install-from-source"
      ;;
  esac
  case "$_arch" in
    x86_64 | amd64)        _arch_part="x86_64" ;;
    aarch64 | arm64)       _arch_part="aarch64" ;;
    *)
      fail "unsupported CPU architecture: ${_arch}." \
        "Nahuali ships prebuilt binaries for x86_64 and aarch64 only." \
        "On other architectures, build from source: https://github.com/${REPO}#install-from-source"
      ;;
  esac
  printf '%s-%s' "$_arch_part" "$_os_part"
}

# ---------------------------------------------------------------------------
# Resolve which release tag to install.
#
# The binary releases are tagged `vX.Y.Z-beta.N` and published as
# non-"latest" prereleases, so /releases/latest is not reliable. We scan the
# releases list (newest first) and pick the first product tag that actually
# carries a .tar.gz asset for the detected target. This deliberately skips
# releases (such as the workspace-root tag) that only ship an SBOM and no
# binaries.
# ---------------------------------------------------------------------------
resolve_tag() {
  _target="$1"
  if [ -n "$REQUESTED_VERSION" ]; then
    printf '%s' "$REQUESTED_VERSION"
    return 0
  fi

  _releases_json="$(fetch_stdout "${API_BASE}/releases?per_page=30")" || _releases_json=""
  if [ -z "$_releases_json" ]; then
    fail "could not reach the GitHub releases API for ${REPO}." \
      "Check your network connection and try again." \
      "You can also pin a version, e.g. NAHUALI_VERSION=vX.Y.Z-beta.N."
  fi

  # The API returns releases newest-first. Walk them in order and, per release,
  # check whether the asset for our target exists. The asset name embeds the
  # version (without the v prefix), so matching on the target triple
  # plus the .tar.gz suffix is unambiguous.
  _chosen=""
  _tag=""
  # Stream tag_name lines and asset name lines together, keeping the most recent
  # product tag that has seen a matching archive asset.
  _parsed="$(
    printf '%s\n' "$_releases_json" \
      | grep -E '"tag_name"|"name"[[:space:]]*:' \
      | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/TAG\t\1/p; s/.*"name"[[:space:]]*:[[:space:]]*"\([^"]*\.tar\.gz\)".*/ASSET\t\1/p'
  )"

  # Iterate: track the current tag; when we see an asset matching the target
  # under a product tag, that tag wins (first match = newest).
  _cur_tag=""
  # shellcheck disable=SC2034
  while IFS="$(printf '\t')" read -r _kind _val; do
    case "$_kind" in
      TAG)
        _cur_tag="$_val"
        ;;
      ASSET)
        case "$_cur_tag" in
          v*) ;;
          *) continue ;;
        esac
        case "$_val" in
          *"-${_target}.tar.gz")
            printf '%s' "$_cur_tag"
            return 0
            ;;
        esac
        ;;
    esac
  done <<EOF
$_parsed
EOF

  fail "no published release contains a Nahuali binary for ${_target}." \
    "The latest binary release may not cover this platform yet." \
    "See available assets: https://github.com/${REPO}/releases"
}

# ---------------------------------------------------------------------------
# Main flow.
# ---------------------------------------------------------------------------
printf '\n%s%sNahuali%s installer\n' "${C_BOLD}" "${C_CYAN}" "${C_RESET}"
printf '%sGoverned memory for AI agents. Prove what you know and that it was not altered.%s\n\n' "${C_DIM}" "${C_RESET}"

step "Detecting platform"
TARGET="$(detect_target)"
ok "${TARGET}"

step "Resolving release"
TAG="$(resolve_tag "$TARGET")"
VERSION="${TAG#v}"
if [ "$VERSION" = "$TAG" ] || [ -z "$VERSION" ]; then
  fail "release tag '${TAG}' is not a recognized Nahuali product version." \
    "Expected the form vX.Y.Z-beta.N."
fi
ok "${TAG}  (v${VERSION})"

ARCHIVE="nahuali-v${VERSION}-${TARGET}.tar.gz"
ARCHIVE_URL="${DOWNLOAD_BASE}/${TAG}/${ARCHIVE}"
CHECKSUM_URL="${ARCHIVE_URL}.sha256"
BUNDLE_URL="${ARCHIVE_URL}.sigstore.json"
# The release workflow signs each artifact from this exact GitHub Actions
# identity (see .github/workflows/release.yml: SIGNER_IDENTITY), so we pin to it.
SIGNER_IDENTITY="https://github.com/${REPO}/.github/workflows/release.yml@refs/tags/${TAG}"
OIDC_ISSUER="https://token.actions.githubusercontent.com"

TMP_DIR="$(mktemp -d 2>/dev/null || mktemp -d -t nahuali-install)"
ARCHIVE_PATH="${TMP_DIR}/${ARCHIVE}"
CHECKSUM_PATH="${ARCHIVE_PATH}.sha256"
BUNDLE_PATH="${ARCHIVE_PATH}.sigstore.json"

step "Downloading"
info "${ARCHIVE_URL}"
if ! fetch_to "$ARCHIVE_URL" "$ARCHIVE_PATH"; then
  fail "failed to download the release archive." \
    "URL: ${ARCHIVE_URL}" \
    "Confirm the release exists and that your network allows github.com."
fi
ok "${ARCHIVE}"

step "Verifying checksum"
# Checksum verification is mandatory. Without a SHA tool we cannot verify, and
# without a published checksum asset the release is unverifiable; both abort
# rather than install unverified binaries.
if [ -z "$SHA_CMD" ]; then
  fail "cannot verify the download: neither sha256sum nor shasum is available." \
    "Checksum verification is mandatory. Install coreutils (Linux: 'apt-get install coreutils';" \
    "macOS ships shasum) and re-run."
fi
if ! fetch_to "$CHECKSUM_URL" "$CHECKSUM_PATH" 2>/dev/null || [ ! -s "$CHECKSUM_PATH" ]; then
  fail "no SHA-256 checksum was published for this release. Refusing to install unverified binaries." \
    "URL: ${CHECKSUM_URL}" \
    "This release cannot be verified; do not install it. Report it upstream or pin a" \
    "verified release with NAHUALI_VERSION."
fi
EXPECTED_SHA="$(awk '{print $1; exit}' "$CHECKSUM_PATH")"
ACTUAL_SHA="$($SHA_CMD "$ARCHIVE_PATH" | awk '{print $1; exit}')"
if [ -z "$EXPECTED_SHA" ] || [ -z "$ACTUAL_SHA" ]; then
  fail "could not compute or read the SHA-256 checksum for verification."
fi
if [ "$EXPECTED_SHA" != "$ACTUAL_SHA" ]; then
  fail "checksum mismatch. Refusing to install a corrupted or tampered archive." \
    "expected: ${EXPECTED_SHA}" \
    "actual:   ${ACTUAL_SHA}" \
    "Delete any partial download and re-run; if it persists, report it upstream."
fi
ok "sha256 matches"

# ---------------------------------------------------------------------------
# Sigstore signature verification.
#
# The release pipeline signs each artifact with cosign (keyless, Sigstore) and
# publishes a .sigstore.json bundle next to the archive. When cosign is present
# (or NAHUALI_REQUIRE_SIGSTORE=1 forces it), verify the bundle against the exact
# GitHub Actions release identity that signed it. A verification failure is a
# hard abort; when cosign is absent and not required, we say so plainly and
# continue on the checksum alone.
# ---------------------------------------------------------------------------
step "Verifying signature"
REQUIRE_SIGSTORE="${NAHUALI_REQUIRE_SIGSTORE:-0}"
if ! have cosign; then
  if [ "$REQUIRE_SIGSTORE" = "1" ]; then
    fail "NAHUALI_REQUIRE_SIGSTORE=1 but 'cosign' was not found on PATH." \
      "Install cosign (https://docs.sigstore.dev/cosign/system_config/installation/) and re-run," \
      "or unset NAHUALI_REQUIRE_SIGSTORE to install with checksum-only verification."
  fi
  warn "cosign not found. SIGNATURE NOT VERIFIED (checksum verified). Install cosign and set NAHUALI_REQUIRE_SIGSTORE=1 to require it."
elif ! fetch_to "$BUNDLE_URL" "$BUNDLE_PATH" 2>/dev/null || [ ! -s "$BUNDLE_PATH" ]; then
  if [ "$REQUIRE_SIGSTORE" = "1" ]; then
    fail "NAHUALI_REQUIRE_SIGSTORE=1 but no Sigstore bundle was published for this release." \
      "URL: ${BUNDLE_URL}"
  fi
  warn "no Sigstore bundle published for this release. SIGNATURE NOT VERIFIED (checksum verified)."
elif cosign verify-blob "$ARCHIVE_PATH" \
  --bundle "$BUNDLE_PATH" \
  --certificate-identity "$SIGNER_IDENTITY" \
  --certificate-oidc-issuer "$OIDC_ISSUER" >/dev/null 2>&1; then
  ok "signature verified (${TAG})"
else
  fail "Sigstore signature verification FAILED. Refusing to install." \
    "The archive was not signed by the expected release identity, or it was altered in transit." \
    "Expected signer: ${SIGNER_IDENTITY}" \
    "Do not install this artifact; report it upstream."
fi

step "Unpacking"
EXTRACT_DIR="${TMP_DIR}/extract"
mkdir -p "$EXTRACT_DIR"
if ! tar -xzf "$ARCHIVE_PATH" -C "$EXTRACT_DIR"; then
  fail "failed to extract the release archive (it may be corrupted)." \
    "Re-run the installer; if it persists, report it upstream."
fi

# The archive lays out <basename>/bin/{nahuali,nahuali-mcp,nahuali-api}.
SRC_BIN_DIR=""
for _candidate in \
  "${EXTRACT_DIR}/nahuali-v${VERSION}-${TARGET}/bin" \
  "${EXTRACT_DIR}"/*/bin
do
  if [ -d "$_candidate" ] && [ -f "${_candidate}/nahuali" ]; then
    SRC_BIN_DIR="$_candidate"
    break
  fi
done
[ -n "$SRC_BIN_DIR" ] || fail "the release archive did not contain the expected bin/ layout." \
  "This may indicate a malformed release; report it upstream."
ok "extracted"

step "Installing to ${BIN_DIR}"
mkdir -p "$BIN_DIR"
INSTALLED=""
for _bin in nahuali nahuali-mcp nahuali-api; do
  _src="${SRC_BIN_DIR}/${_bin}"
  if [ ! -f "$_src" ]; then
    fail "expected binary '${_bin}' is missing from the release archive." \
      "Report this upstream; the release may be incomplete."
  fi
  # Write to a temp name then move into place so a re-run never leaves a
  # half-written binary (idempotent and atomic per file).
  _dst="${BIN_DIR}/${_bin}"
  cp "$_src" "${_dst}.new"
  chmod +x "${_dst}.new"
  mv -f "${_dst}.new" "$_dst"
  INSTALLED="${INSTALLED} ${_bin}"
done
ok "installed:${INSTALLED}"

# ---------------------------------------------------------------------------
# Confirm the binary actually runs on this machine.
# ---------------------------------------------------------------------------
step "Verifying install"
NAHUALI_BIN="${BIN_DIR}/nahuali"
if VERSION_OUT="$("$NAHUALI_BIN" --version 2>/dev/null)"; then
  ok "${VERSION_OUT}"
else
  fail "the installed 'nahuali' binary did not run on this machine." \
    "Path: ${NAHUALI_BIN}" \
    "This usually means an OS/arch mismatch. Detected target: ${TARGET}." \
    "If wrong, report it; otherwise build from source: https://github.com/${REPO}#install-from-source"
fi

# ---------------------------------------------------------------------------
# PATH guidance. We never edit shell rc files for you.
# ---------------------------------------------------------------------------
ON_PATH="false"
case ":${PATH}:" in
  *":${BIN_DIR}:"*) ON_PATH="true" ;;
esac

printf '\n%s%sNahuali v%s is installed.%s\n' "${C_GREEN}${C_BOLD}" "" "${VERSION}" "${C_RESET}"

if [ "$ON_PATH" = "true" ]; then
  printf '%s%s is already on your PATH.%s\n' "${C_DIM}" "${BIN_DIR}" "${C_RESET}"
else
  # Suggest the rc file that matches the current shell, without touching it.
  _shell_name="$(basename "${SHELL:-sh}")"
  case "$_shell_name" in
    zsh)  _rc="${HOME}/.zshrc" ;;
    bash) _rc="${HOME}/.bashrc" ;;
    fish) _rc="${HOME}/.config/fish/config.fish" ;;
    *)    _rc="your shell profile" ;;
  esac
  printf '\n%sAdd Nahuali to your PATH%s. Paste this into %s:\n\n' "${C_BOLD}" "${C_RESET}" "$_rc"
  if [ "$_shell_name" = "fish" ]; then
    printf '    %sfish_add_path %s%s\n' "${C_CYAN}" "${BIN_DIR}" "${C_RESET}"
  else
    # The literal $PATH is intentional: it is the line the user pastes verbatim.
    # shellcheck disable=SC2016
    printf '    %sexport PATH="%s:$PATH"%s\n' "${C_CYAN}" "${BIN_DIR}" "${C_RESET}"
  fi
  printf '\n%sThen reload your shell: open a new terminal, or source the file above.%s\n' "${C_DIM}" "${C_RESET}"
  printf '%sFor this session only, you can also run the full path below.%s\n' "${C_DIM}" "${C_RESET}"
fi

# ---------------------------------------------------------------------------
# The magic next step.
# ---------------------------------------------------------------------------
printf '\n%sNext:%s see why memory is trusted, flagged, or blocked. No Docker, no setup:\n\n' "${C_BOLD}" "${C_RESET}"
if [ "$ON_PATH" = "true" ]; then
  printf '    %snahuali demo%s\n' "${C_CYAN}${C_BOLD}" "${C_RESET}"
else
  printf '    %s%s/nahuali demo%s\n' "${C_CYAN}${C_BOLD}" "${BIN_DIR}" "${C_RESET}"
fi
printf '\n%sIt recalls supported and unsupported claims, inspects its own health, and tests ledger tampering.%s\n\n' "${C_DIM}" "${C_RESET}"

printf '%sThen wire your agent harness to use Nahuali (installs the skill, prints the MCP config):%s\n\n' "${C_BOLD}" "${C_RESET}"
if [ "$ON_PATH" = "true" ]; then
  printf '    %snahuali init%s\n\n' "${C_CYAN}${C_BOLD}" "${C_RESET}"
else
  printf '    %s%s/nahuali init%s\n\n' "${C_CYAN}${C_BOLD}" "${BIN_DIR}" "${C_RESET}"
fi
