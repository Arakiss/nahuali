#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

require_command() {
  local command_name="$1"

  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "$command_name is required for the controlled beta gate" >&2
    exit 1
  fi
}

run_step() {
  local label="$1"
  shift

  local started_at="$SECONDS"
  printf '==> %s\n' "$label"
  "$@"
  printf 'ok: %s (%ss)\n' "$label" "$((SECONDS - started_at))"
}

require_docker() {
  require_command docker

  if ! docker info >/dev/null 2>&1; then
    echo "Docker is required and the daemon is not reachable." >&2
    echo "Start Docker Desktop or your Docker daemon, then rerun this script." >&2
    exit 1
  fi

  if ! docker compose version >/dev/null 2>&1; then
    echo "docker compose is required for the local SurrealDB and Qdrant stack." >&2
    echo "Install Docker Compose v2 or use a Docker Desktop version that includes it." >&2
    exit 1
  fi
}

require_command cargo
require_command jq
require_command bun
require_docker

run_step "Public documentation contract" bash scripts/check-doc-release-refs.sh
run_step "Public security and supply-chain hygiene" bash scripts/security-supply-chain-check.sh
run_step "Service-backed dev stack" bash scripts/ensure-dev-stack.sh
run_step "Governance benchmark suite" bash scripts/verify-governance-benchmarks.sh

run_step "Build local binaries once for beta checks" cargo build -p nahuali-cli -p nahuali-mcp -p nahuali-api --quiet
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
case "$TARGET_DIR" in
  /*) ;;
  *) TARGET_DIR="$ROOT/$TARGET_DIR" ;;
esac
NAHUALI_BIN="$TARGET_DIR/debug/nahuali"
NAHUALI_API_BIN="$TARGET_DIR/debug/nahuali-api"
if [[ ! -x "$NAHUALI_BIN" ]]; then
  echo "Rust nahuali binary is missing after build" >&2
  echo "expected: $NAHUALI_BIN" >&2
  exit 1
fi
if [[ ! -x "$NAHUALI_API_BIN" ]]; then
  echo "Rust nahuali-api binary is missing after build" >&2
  echo "expected: $NAHUALI_API_BIN" >&2
  exit 1
fi

run_step "External-policy signed checkpoint operator path" \
  cargo test -p nahuali-cli --test operator_paths \
    checkpoint_v2_operator_path_enforces_external_threshold_policy_and_match_modes -- --exact

run_step "Private portable claim receipt operator path" \
  cargo test -p nahuali-cli --test operator_paths \
    claim_receipt_exports_privately_and_verifies_without_opening_a_store -- --exact

run_step "Cross-process graph projection fencing and manifest" \
  env NAHUALI_BIN="$NAHUALI_BIN" bash scripts/verify-projection-concurrency.sh

run_step "Embedded persistence, process ownership, and MCP handshake" \
  env NAHUALI_VERIFY_BIN_DIR="$TARGET_DIR/debug" bash scripts/verify-embedded-store.sh

export NAHUALI_VALIDATE_SKIP_DEV_STACK=1

run_step "Self-inspecting memory demo" \
  env NAHUALI_BIN="$NAHUALI_BIN" bash scripts/demo-self-inspecting-memory.sh

run_step "Agent-first daily-driver demo" \
  env NAHUALI_BIN="$NAHUALI_BIN" bash scripts/demo-daily-driver-loop.sh

run_step "Daily-driver reliability gate" \
  env NAHUALI_DOGFOOD_BIN="$NAHUALI_BIN" bash scripts/verify-dogfood-daily-workflow.sh

run_step "Evidence-backed recall contract" \
  env NAHUALI_RECALL_CONTRACT_BIN="$NAHUALI_BIN" bash scripts/verify-recall-contract.sh

run_step "Loopback HTTP client examples" \
  env NAHUALI_API_BIN="$NAHUALI_API_BIN" bash scripts/verify-http-client-examples.sh

run_step "LongMemEval retrieval adapter smoke" \
  env NAHUALI_LONGMEMEVAL_BIN="$NAHUALI_BIN" bash scripts/verify-longmemeval-adapter.sh

printf '\ncontrolled beta gate passed\n'
printf 'The checkout is ready for controlled synthetic/local testing.\n'
