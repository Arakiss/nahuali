#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CLONE_PARENT="$(mktemp -d)"
CLONE_DIR="$CLONE_PARENT/nahuali"

cleanup() {
  rm -rf "$CLONE_PARENT"
}
trap cleanup EXIT

git clone --local --no-hardlinks "$ROOT" "$CLONE_DIR" >/dev/null
cd "$CLONE_DIR"

if [[ -e .private || -e .local || -e .runs || -e docs ]]; then
  echo "fresh clone unexpectedly contains local run artifacts" >&2
  exit 1
fi

if [[ -n "$(git status --porcelain)" ]]; then
  echo "fresh clone is not clean" >&2
  git status --short >&2
  exit 1
fi

if command -v docker >/dev/null 2>&1 && [[ "${NAHUALI_FRESH_CLONE_USE_DOCKER:-1}" != "0" ]]; then
  bash "$ROOT/scripts/ensure-dev-stack.sh"
  docker run --rm \
    --add-host=host.docker.internal:host-gateway \
    -v "$CLONE_DIR":/work \
    -w /work \
    -e CARGO_TARGET_DIR=/tmp/nahuali-target \
    -e DEBIAN_FRONTEND=noninteractive \
    -e NAHUALI_VALIDATE_SKIP_DEV_STACK=1 \
    -e NAHUALI_DB_URL=host.docker.internal:18000 \
    -e NAHUALI_QDRANT_URL=http://host.docker.internal:16333 \
    rust:latest \
    sh -lc '
      set -e
      export PATH=/usr/local/cargo/bin:$PATH
      apt-get update >/dev/null
      apt-get install -y jq ripgrep ruby >/dev/null
      bash scripts/validate-clean-tree.sh
    '
else
  bash "$ROOT/scripts/ensure-dev-stack.sh"
  NAHUALI_VALIDATE_SKIP_DEV_STACK=1 bash scripts/validate-clean-tree.sh
fi

echo "fresh clone validation passed"
