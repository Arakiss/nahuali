#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
API_BIN="${NAHUALI_API_BIN:-$ROOT/target/debug/nahuali-api}"

for command_name in python3 bun curl; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    printf 'error: %s is required to verify the HTTP client examples\n' "$command_name" >&2
    exit 1
  fi
done

if [[ ! -x "$API_BIN" ]]; then
  printf 'error: nahuali-api binary is missing or not executable: %s\n' "$API_BIN" >&2
  printf 'build it with: cargo build -p nahuali-api\n' >&2
  exit 1
fi

tmp="$(mktemp -d "${TMPDIR:-/tmp}/nahuali-http-examples.XXXXXX")"
server_pid=""

stop_server() {
  if [[ -n "$server_pid" ]] && kill -0 "$server_pid" 2>/dev/null; then
    kill "$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
  fi
  server_pid=""
}

cleanup() {
  stop_server
  rm -r "$tmp"
}
trap cleanup EXIT

pick_loopback_port() {
  python3 - <<'PY'
import socket

with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as listener:
    listener.bind(("127.0.0.1", 0))
    print(listener.getsockname()[1])
PY
}

wait_until_live() {
  local base_url="$1"
  local log_file="$2"

  for _ in $(seq 1 150); do
    if curl --fail --silent --show-error "$base_url/v1/health" >/dev/null 2>&1; then
      return 0
    fi
    if ! kill -0 "$server_pid" 2>/dev/null; then
      printf 'error: nahuali-api exited before becoming ready\n' >&2
      cat "$log_file" >&2
      return 1
    fi
    sleep 0.1
  done

  printf 'error: nahuali-api did not become live at %s\n' "$base_url" >&2
  cat "$log_file" >&2
  return 1
}

wait_until_ready() {
  local base_url="$1"
  local log_file="$2"

  for _ in $(seq 1 50); do
    if curl --fail --silent --show-error "$base_url/v1/ready" >/dev/null 2>&1; then
      return 0
    fi
    if ! kill -0 "$server_pid" 2>/dev/null; then
      printf 'error: nahuali-api exited before becoming ready\n' >&2
      cat "$log_file" >&2
      return 1
    fi
    sleep 0.1
  done

  printf 'error: nahuali-api did not become ready at %s\n' "$base_url" >&2
  cat "$log_file" >&2
  return 1
}

run_client() {
  local label="$1"
  shift
  local port
  local database
  local base_url
  local store
  local log_file

  port="$(pick_loopback_port)"
  database="nahuali_http_${label}_$$_$(date +%s)"
  base_url="http://127.0.0.1:$port"
  store="$tmp/$label/store"
  log_file="$tmp/$label/server.log"
  mkdir -p "$(dirname "$store")"

  env \
    NAHUALI_DB_URL="surrealkv://$store" \
    NAHUALI_DB_NAMESPACE="nahuali_http_examples" \
    "$API_BIN" --database "$database" --listen "127.0.0.1:$port" \
    >"$log_file" 2>&1 &
  server_pid="$!"

  wait_until_live "$base_url" "$log_file"
  curl --fail --silent --show-error --request POST \
    "$base_url/v1/projection/rebuild" >/dev/null
  wait_until_ready "$base_url" "$log_file"
  env \
    NAHUALI_API_URL="$base_url" \
    NAHUALI_EXAMPLE_RUN_ID="$database" \
    "$@"
  stop_server
}

cd "$ROOT"
run_client python python3 examples/http/python_client.py
run_client typescript bun examples/http/typescript_client.ts

printf 'HTTP client examples verified with isolated disposable stores.\n'
