#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ "${NAHUALI_VALIDATE_SKIP_DEV_STACK:-0}" != "1" ]]; then
  bash scripts/ensure-dev-stack.sh
fi

INSTALL_ROOT="$(mktemp -d)"
STORE_DIR="$(mktemp -d)"

cleanup() {
  rm -rf "$INSTALL_ROOT" "$STORE_DIR"
}
trap cleanup EXIT

GLOBAL_NAHUALI_BEFORE="$(command -v nahuali || true)"
CARGO_RUN_OUTPUT="$STORE_DIR/cargo-run-output.json"

if [[ -n "${NAHUALI_VERIFY_CLI_BIN_DIR:-}" ]]; then
  SOURCE_BIN_DIR="$NAHUALI_VERIFY_CLI_BIN_DIR"
  case "$SOURCE_BIN_DIR" in
    /*) ;;
    *) SOURCE_BIN_DIR="$ROOT/$SOURCE_BIN_DIR" ;;
  esac
  if [[ ! -x "$SOURCE_BIN_DIR/nahuali" ]]; then
    echo "source release nahuali binary is missing or not executable: $SOURCE_BIN_DIR/nahuali" >&2
    exit 1
  fi
  "$SOURCE_BIN_DIR/nahuali" --database "$STORE_DIR/cargo-run" validate --json >"$CARGO_RUN_OUTPUT"
else
  cargo run -p nahuali-cli -- --database "$STORE_DIR/cargo-run" validate --json >"$CARGO_RUN_OUTPUT"
fi
if ! grep -q '"valid":true' "$CARGO_RUN_OUTPUT"; then
  echo "cargo-run Rust CLI validation did not report a valid store" >&2
  cat "$CARGO_RUN_OUTPUT" >&2
  exit 1
fi

if [[ -n "${NAHUALI_VERIFY_CLI_BIN_DIR:-}" ]]; then
  mkdir -p "$INSTALL_ROOT/bin"
  cp "$SOURCE_BIN_DIR/nahuali" "$INSTALL_ROOT/bin/nahuali"
  chmod +x "$INSTALL_ROOT/bin/nahuali"
else
  cargo install --path crates/nahuali-cli --locked --debug --root "$INSTALL_ROOT" --force --quiet
fi

RUST_NAHUALI="$INSTALL_ROOT/bin/nahuali"
if [[ ! -x "$RUST_NAHUALI" ]]; then
  echo "isolated Rust nahuali binary is missing or not executable" >&2
  exit 1
fi

GLOBAL_NAHUALI_AFTER="$(command -v nahuali || true)"
if [[ "$GLOBAL_NAHUALI_AFTER" != "$GLOBAL_NAHUALI_BEFORE" ]]; then
  echo "global nahuali command changed during isolated install" >&2
  echo "before: ${GLOBAL_NAHUALI_BEFORE:-<missing>}" >&2
  echo "after: ${GLOBAL_NAHUALI_AFTER:-<missing>}" >&2
  exit 1
fi

if [[ -n "$GLOBAL_NAHUALI_BEFORE" && "$GLOBAL_NAHUALI_BEFORE" == "$RUST_NAHUALI" ]]; then
  echo "isolated Rust nahuali unexpectedly resolved as the global nahuali command" >&2
  exit 1
fi

STORE="$STORE_DIR/isolated-install"
"$RUST_NAHUALI" --database "$STORE" remember "Synthetic coexistence memory" --tag synthetic >/dev/null
"$RUST_NAHUALI" --database "$STORE" claim "Synthetic CLI" validates coexistence --source-last >/dev/null
validate_output="$("$RUST_NAHUALI" --database "$STORE" validate --json)"
if [[ "$validate_output" != *'"valid":true'* || "$validate_output" != *'"event_count":2'* ]]; then
  echo "isolated Rust nahuali validation did not report the expected store" >&2
  echo "$validate_output" >&2
  exit 1
fi

echo "cli coexistence check passed"
