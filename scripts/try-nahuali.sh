#!/usr/bin/env bash
# try-nahuali.sh — de cero a ver "el recibo" en un comando.
#
# Levanta el stack local, compila la CLI, siembra memoria sintetica y corre el
# loop diario. Al final veras, en el mismo store, un claim CERTIFICADO por su
# evidencia y a la vez un AVISO sobre un hecho sin fuente: el momento que
# diferencia a Nahuali de una memoria de solo-recall.
#
# Uso:  bash scripts/try-nahuali.sh
# Requisitos: docker (o un stack SurrealDB+Qdrant ya arriba), cargo, jq.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

step() { printf '\n\033[1m==> %s\033[0m\n' "$*"; }

step "1/4 · Levantando el stack local (SurrealDB + Qdrant)"
bash scripts/ensure-dev-stack.sh

step "2/4 · Compilando la CLI (la primera vez tarda unos minutos)"
cargo build -q -p nahuali-cli

step "3/4 · Sembrando memoria sintetica y corriendo el loop diario"
NAHUALI_BIN="$ROOT/target/debug/nahuali" bash scripts/demo-daily-driver-loop.sh

step "4/4 · Que acabas de ver"
cat <<'EOF'
  En el bloque "3. Evidence-backed recall" de arriba:
    - el claim con fuente se CERTIFICA        (trust.can_trust: true,  score 1.0)
    - el store AVISA de un hecho sin fuente    (authority.can_trust: false, score 0.5)
  Eso es el recibo: memoria util y, en la misma respuesta, por que fiarte o no.

  Sigue explorando contra la misma base de datos:
    target/debug/nahuali --database <db-del-demo> inspect --json
    target/debug/nahuali --database <db-del-demo> self-inspect --json
    target/debug/nahuali --database <db-del-demo> review --json
EOF
