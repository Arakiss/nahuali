#!/usr/bin/env bash
set -euo pipefail

cargo run --quiet --locked -p nahuali-core \
  --example refresh_performance \
  --features regression-fixtures
