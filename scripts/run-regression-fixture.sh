#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ $# -ne 1 ]]; then
  echo "usage: bash scripts/run-regression-fixture.sh FIXTURE" >&2
  exit 1
fi

fixture="$1"
if [[ ! -f "$fixture" ]]; then
  echo "regression fixture does not exist: $fixture" >&2
  exit 1
fi

if [[ -n "${NAHUALI_REGRESSION_BIN:-}" ]]; then
  runner="$NAHUALI_REGRESSION_BIN"
elif [[ -n "${NAHUALI_REGRESSION_BIN_DIR:-}" ]]; then
  runner="${NAHUALI_REGRESSION_BIN_DIR%/}/nahuali-regression"
else
  exec cargo run -p nahuali-regression -- --fixtures "$fixture"
fi

if [[ ! -x "$runner" ]]; then
  echo "nahuali-regression binary is not executable: $runner" >&2
  exit 1
fi

exec "$runner" --fixtures "$fixture"
