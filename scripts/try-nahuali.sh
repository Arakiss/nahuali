#!/usr/bin/env bash
# try-nahuali.sh — from zero to an evidence-aware recall in one command.
#
# Builds the CLI against the embedded default store and runs a synthetic
# walkthrough. At the end you will see a sourced claim that meets the configured
# evidence gate and, in the same store, a warning about a claim with no source.
#
# Usage:  bash scripts/try-nahuali.sh
# Requires: cargo and jq. Docker is not required for the embedded default store.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

step() { printf '\n\033[1m==> %s\033[0m\n' "$*"; }

# A clean SurrealDB identifier: the CLI refuses a path-like --database name.
DEMO_DB="${NAHUALI_DEMO_DB:-try_nahuali_demo_$$}"
export NAHUALI_DEMO_DB="$DEMO_DB"

step "1/3 · Building the CLI (the initial build can take a few minutes)"
cargo build -q -p nahuali-cli

step "2/3 · Running the synthetic evidence walkthrough"
NAHUALI_BIN="$ROOT/target/debug/nahuali" bash scripts/demo-walkthrough.sh

step "3/3 · What you just saw"
cat <<EOF
  The sourced claim met the configured evidence gate, while the store warned
  about a claim with no source. That distinction is visible in one recall.

  Evidence linkage does not prove the source is true or sufficient. The caller
  still decides whether the information is appropriate for an external action.

  Keep exploring against the same database:
    target/debug/nahuali --database $DEMO_DB inspect --json
    target/debug/nahuali --database $DEMO_DB self-inspect --json
    target/debug/nahuali --database $DEMO_DB review --json
EOF
