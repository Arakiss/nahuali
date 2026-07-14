#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

run_step() {
  local label="$1"
  shift

  local started_at="$SECONDS"
  if [[ "${GITHUB_ACTIONS:-}" == "true" ]]; then
    echo "::group::${label}"
  else
    echo "==> ${label}"
  fi

  "$@"

  local elapsed=$((SECONDS - started_at))
  if [[ "${GITHUB_ACTIONS:-}" == "true" ]]; then
    echo "::endgroup::"
  fi
  echo "ok: ${label} (${elapsed}s)"
}

run_quiet_step() {
  local label="$1"
  shift

  local output
  output="$(mktemp)"
  local started_at="$SECONDS"

  if [[ "${GITHUB_ACTIONS:-}" == "true" ]]; then
    echo "::group::${label}"
  else
    echo "==> ${label}"
  fi

  if "$@" >"$output" 2>&1; then
    rm -f "$output"
    local elapsed=$((SECONDS - started_at))
    if [[ "${GITHUB_ACTIONS:-}" == "true" ]]; then
      echo "::endgroup::"
    fi
    echo "ok: ${label} (${elapsed}s)"
    return 0
  fi

  cat "$output" >&2
  rm -f "$output"
  if [[ "${GITHUB_ACTIONS:-}" == "true" ]]; then
    echo "::endgroup::"
  fi
  return 1
}

if [[ "${NAHUALI_VALIDATE_SKIP_BASE_CHECKS:-0}" != "1" ]]; then
  run_step "Rust formatting" cargo fmt --check
  run_step "Workspace internal dependency pins" sh scripts/sync-workspace-internal-deps.sh --check
  run_step "MCP server release metadata" sh scripts/sync-mcp-server-metadata.sh --check
fi
run_step "Private memory dry-run helper interface" bash scripts/private-memory-dry-run.sh --help
run_step "Sanitized main bundle helper interface" bash scripts/export-sanitized-main-bundle.sh --help
if [[ "${NAHUALI_VALIDATE_SKIP_DEV_STACK:-0}" != "1" ]]; then
  run_step "Service-backed dev stack" bash scripts/ensure-dev-stack.sh
fi
export NAHUALI_VALIDATE_SKIP_DEV_STACK=1
if [[ "${NAHUALI_VALIDATE_SKIP_BASE_CHECKS:-0}" != "1" ]]; then
  run_step "Rust clippy" cargo clippy --workspace --all-targets -- -D warnings
  run_step "Rust workspace tests" cargo test --workspace
fi
run_step "Rust API documentation" cargo doc -p nahuali-core --no-deps
run_step "Core crate package dry-run" cargo package -p nahuali-core --allow-dirty --no-verify
run_step "Release artifact dry-run" bash scripts/release-dry-run.sh
release_bin_dir="${CARGO_TARGET_DIR:-target}/release"
run_step "Regression runner release binary" test -x "$release_bin_dir/nahuali-regression"
run_step "Isolated install smoke" env NAHUALI_VERIFY_INSTALL_BIN_DIR="$release_bin_dir" bash scripts/verify-install.sh
run_step "CLI coexistence smoke" env NAHUALI_VERIFY_CLI_BIN_DIR="$release_bin_dir" bash scripts/verify-cli-coexistence.sh
run_step "Private memory dry-run summary smoke" env NAHUALI_PRIVATE_DRY_RUN_BIN_DIR="$release_bin_dir" bash scripts/verify-private-memory-dry-run.sh
run_step "Daily dogfood workflow" env NAHUALI_DOGFOOD_BIN_DIR="$release_bin_dir" bash scripts/verify-dogfood-daily-workflow.sh
run_step "Dogfood migration workflow" env NAHUALI_DOGFOOD_BIN_DIR="$release_bin_dir" bash scripts/verify-dogfood-migration.sh
run_step "Documentation release refs" sh scripts/check-doc-release-refs.sh
for attempt in 1 2 3; do
  run_quiet_step "Knowledge-health regression fixture ${attempt}" env NAHUALI_REGRESSION_BIN_DIR="$release_bin_dir" bash scripts/run-regression-fixture.sh fixtures/knowledge-health-regression.json
done
run_quiet_step "Recall regression fixture" env NAHUALI_REGRESSION_BIN_DIR="$release_bin_dir" bash scripts/run-regression-fixture.sh fixtures/recall-regression.json
run_step "Recall contract smoke" env NAHUALI_RECALL_CONTRACT_BIN_DIR="$release_bin_dir" bash scripts/verify-recall-contract.sh
run_step "Security and supply-chain checks" bash scripts/security-supply-chain-check.sh
