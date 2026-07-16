#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

binary="${NAHUALI_RETRIEVAL_BIN:-target/release/nahuali}"
if [[ ! -x "$binary" ]]; then
  cargo build --locked --release -p nahuali-cli
fi

source_revision="${NAHUALI_RETRIEVAL_SOURCE_REVISION:-$(git rev-parse HEAD)}"
result="$(mktemp "${TMPDIR:-/tmp}/nahuali-retrieval.XXXXXX.json")"
trap 'rm -f "$result"' EXIT

python3 -m unittest discover -s benchmarks/agent-memory-retrieval/tests
python3 benchmarks/agent-memory-retrieval/adapters/nahuali.py \
  --binary "$binary" \
  --source-revision "$source_revision" \
  --output "$result"
python3 benchmarks/agent-memory-retrieval/score.py "$result"
