#!/usr/bin/env bash
# demo-walkthrough.sh — Nahuali explicado para HUMANOS, paso a paso.
#
# Corre el motor de VERDAD (la CLI real, el ledger real). No hay nada simulado:
# cada resultado sale del engine. Pero en vez de escupir JSON, te lo cuenta en
# lenguaje claro y te explica por que cada paso importa.
#
# Uso:        bash scripts/demo-walkthrough.sh
# Con pausas: NAHUALI_DEMO_PAUSE=1 bash scripts/demo-walkthrough.sh
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"; cd "$ROOT"
command -v jq >/dev/null || { echo "Este recorrido necesita 'jq'."; exit 1; }

if   [[ -n "${NAHUALI_BIN:-}" ]];        then N="$NAHUALI_BIN"
elif [[ -x target/debug/nahuali ]];      then N="target/debug/nahuali"
elif [[ -x target/release/nahuali ]];    then N="target/release/nahuali"
else N=""; fi
DB=".local/walkthrough-$$"
run(){ if [[ -n "$N" ]]; then "$N" --database "$DB" "$@"; else cargo run -q -p nahuali-cli -- --database "$DB" "$@"; fi; }

B(){ printf '\033[1m%s\033[0m\n' "$*"; }
dim(){ printf '\033[2m%s\033[0m\n' "$*"; }
ok(){ printf '   \033[1;32m✓ %s\033[0m\n' "$*"; }
warn(){ printf '   \033[1;33m⚠ %s\033[0m\n' "$*"; }
arrow(){ printf '   \033[36m→\033[0m %s\n' "$*"; }
cmd(){ dim "     \$ nahuali $*"; }
pause(){ [[ "${NAHUALI_DEMO_PAUSE:-0}" == "1" ]] && read -rp $'     (enter para seguir)' _ ; printf '\n'; }

bash scripts/ensure-dev-stack.sh >/dev/null 2>&1 || true

printf '\n'
B "═══════════════════════════════════════════════════════════════"
B "   Nahuali en 6 pasos. Sin jerga. Mira que hace de verdad."
B "═══════════════════════════════════════════════════════════════"
cat <<'EOF'

   Tu agente recuerda cosas mientras trabaja. Algunas con prueba de
   donde salieron, otras no. Una memoria normal las guarda todas por
   igual. Nahuali no: te dice de cuales puedes fiarte.

   Vamos a darle dos recuerdos -- uno CON fuente, otro SIN fuente --
   y a ver como los trata distinto.
EOF
pause

B "Paso 1 · La memoria empieza vacia"
cmd "validate"
V=$(run validate --json)
ok "Ledger valido: $(echo "$V" | jq -r '.valid'). Eventos guardados: $(echo "$V" | jq -r '.event_count'). Partimos de cero."
pause

B "Paso 2 · El agente OBSERVA algo y lo guarda con su origen"
cmd 'remember "Ana dijo en la reunion que ella lleva el release de marzo"'
run remember "Ana dijo en la reunion de planificacion que ella es la responsable del release de marzo." \
    --tag reunion --mention Ana --scope project:Demo >/dev/null
arrow "Eso queda como un EPISODIO. Es la fuente: la prueba original de donde salio la informacion."
pause

B "Paso 3 · Convierte la observacion en un hecho, citando esa fuente"
cmd 'claim Ana owns "release de marzo" --source-last'
run claim Ana owns "release de marzo" --confidence 0.9 --source-last --scope project:Demo >/dev/null
ok "Ahora hay un HECHO, y apunta a la reunion como su evidencia."
pause

B "Paso 4 · El agente afirma OTRO hecho... pero sin ninguna prueba"
cmd 'claim Beto owns "el roadmap"'
run claim Beto owns "el roadmap" --confidence 0.5 --scope project:Demo >/dev/null
arrow "Nahuali lo guarda, pero toma nota: esto nadie sabe de donde salio."
pause

B "Paso 5 · Le preguntas al agente: ¿quien lleva el release de marzo?"
cmd 'recall "release de marzo" --authority'
R=$(run recall "release de marzo" --authority --json --scope project:Demo)
EXC=$(echo "$R" | jq -r '.results[0].excerpt // "—"')
TCAN=$(echo "$R" | jq -r '.results[0].trust.can_trust')
TMODE=$(echo "$R" | jq -r '.results[0].trust.mode')
TWHY=$(echo "$R" | jq -r '.results[0].trust.reasons[0] // ""')
ACAN=$(echo "$R" | jq -r '.authority.can_trust')
AMODE=$(echo "$R" | jq -r '.authority.mode')
AWHY=$(echo "$R" | jq -r '.authority.reasons[0] // ""')
echo "   Nahuali encuentra:  \"$EXC\""
if [[ "$TCAN" == "true" ]]; then
  ok "CERTIFICADO ($TMODE). Tiene evidencia detras. Puedes actuar sobre esto."
  dim "       motivo del engine: $TWHY"
fi
if [[ "$ACAN" != "true" ]]; then
  warn "Pero AVISA sobre el conjunto ($AMODE). No todo el store es de fiar:"
  dim "       motivo del engine: $AWHY"
fi
arrow "Esto es lo que una memoria de solo-recall NO hace: te da la respuesta"
arrow "buena Y te avisa de lo dudoso, en la misma consulta."
pause

B "Paso 6 · ¿Que deberias revisar antes de fiarte del todo?"
cmd "review"
RV=$(run review --json)
CNT=$(echo "$RV" | jq -r '.summary.item_count // (.items|length)')
T0=$(echo "$RV" | jq -r '.items[0].title // "—"')
G0=$(echo "$RV" | jq -r '.items[0].operator_guidance // ""')
echo "   Nahuali te entrega una lista priorizada de $CNT cosa(s) a revisar. La primera:"
warn "$T0"
dim "       que hacer: $G0"
arrow "No lo arregla por su cuenta. Te lo señala y te deja decidir."
pause

B "El aja"
cat <<'EOF'

   Una memoria normal (Mem0, Zep) te habria devuelto las dos cosas
   como si ambas fueran ciertas. Nahuali te devolvio la buena CON su
   recibo y te marco la dudosa para revisar.

   No vende memoria. Vende poder fiarte de ella -- y dar la cara por
   lo que tu agente recordo e hizo.

EOF
dim "   (Memoria de demo en $DB · sintetica · puedes borrarla sin miedo)"
printf '\n'
