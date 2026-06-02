#!/usr/bin/env bash
# try-nahuali.sh — from zero to seeing "the receipt" in one command.
#
# Brings up the local stack, builds the CLI, seeds synthetic memory, and runs
# the daily loop. At the end you will see, in the same store, a claim CERTIFIED
# by its evidence and, at the same time, a WARNING about a fact with no source:
# the moment that sets Nahuali apart from a recall-only memory.
#
# Usage:  bash scripts/try-nahuali.sh
# Requires: docker (or an already-running SurrealDB+Qdrant stack), cargo, jq.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

step() { printf '\n\033[1m==> %s\033[0m\n' "$*"; }

DEMO_DB=".local/try-nahuali-demo"
export NAHUALI_DEMO_DB="$DEMO_DB"

step "1/4 · Bringing up the local stack (SurrealDB + Qdrant)"
bash scripts/ensure-dev-stack.sh

step "2/4 · Building the CLI (the first build takes a few minutes)"
cargo build -q -p nahuali-cli

step "3/4 · Seeding synthetic memory and running the daily loop"
NAHUALI_BIN="$ROOT/target/debug/nahuali" bash scripts/demo-daily-driver-loop.sh

step "4/4 · What you just saw"
cat <<EOF
  In the "3. Evidence-backed recall" block above:
    - the sourced claim is CERTIFIED       (trust.can_trust: true,  score 1.0)
    - the store WARNS about a fact with no source (authority.can_trust: false, score 0.5)
  That is the receipt: useful memory and, in the same response, why to trust it or not.

  Keep exploring against the same database:
    target/debug/nahuali --database $DEMO_DB inspect --json
    target/debug/nahuali --database $DEMO_DB self-inspect --json
    target/debug/nahuali --database $DEMO_DB review --json
EOF
