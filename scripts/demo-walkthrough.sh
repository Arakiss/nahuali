#!/usr/bin/env bash
# demo-walkthrough.sh — Nahuali explained for HUMANS, step by step.
#
# Runs the REAL engine (the real CLI, the real ledger). Nothing is simulated:
# every result comes from the engine. But instead of dumping JSON, it narrates
# in plain language and explains why each step matters.
#
# Usage:        bash scripts/demo-walkthrough.sh
# With pauses:  NAHUALI_DEMO_PAUSE=1 bash scripts/demo-walkthrough.sh
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"; cd "$ROOT"
command -v jq >/dev/null || { echo "This walkthrough needs 'jq'."; exit 1; }

if   [[ -n "${NAHUALI_BIN:-}" ]];        then N="$NAHUALI_BIN"
elif [[ -x target/debug/nahuali ]];      then N="target/debug/nahuali"
elif [[ -x target/release/nahuali ]];    then N="target/release/nahuali"
else N=""; fi
DB="${NAHUALI_DEMO_DB:-walkthrough_$$}"
run(){ if [[ -n "$N" ]]; then "$N" --database "$DB" "$@"; else cargo run -q -p nahuali-cli -- --database "$DB" "$@"; fi; }

B(){ printf '\033[1m%s\033[0m\n' "$*"; }
dim(){ printf '\033[2m%s\033[0m\n' "$*"; }
ok(){ printf '   \033[1;32m✓ %s\033[0m\n' "$*"; }
warn(){ printf '   \033[1;33m⚠ %s\033[0m\n' "$*"; }
arrow(){ printf '   \033[36m→\033[0m %s\n' "$*"; }
cmd(){ dim "     \$ nahuali $*"; }
pause(){ [[ "${NAHUALI_DEMO_PAUSE:-0}" == "1" ]] && read -rp $'     (enter to continue)' _ ; printf '\n'; }

printf '\n'
B "═══════════════════════════════════════════════════════════════"
B "   Nahuali in 6 steps. No jargon. See what it actually does."
B "═══════════════════════════════════════════════════════════════"
cat <<'EOF'

   Your agent remembers things as it works. Some entries have a source
   record; others do not. This walkthrough shows how Nahuali keeps that
   distinction visible in recall and review.

   We will give it two memories -- one WITH a source, one WITHOUT --
   and watch it treat them differently.
EOF
pause

B "Step 1 · Memory starts empty"
cmd "validate"
V=$(run validate --json)
ok "Ledger valid: $(echo "$V" | jq -r '.valid'). Stored events: $(echo "$V" | jq -r '.event_count'). We start from zero."
pause

B "Step 2 · The agent OBSERVES something and stores it with its origin"
cmd 'remember "Ana said in the planning meeting that she owns the March release"'
run remember "Ana said in the planning meeting that she owns the March release." \
    --tag meeting --mention Ana --scope project:Demo >/dev/null
arrow "That becomes an EPISODE: the recorded source for the later claim."
pause

B "Step 3 · Turn the observation into a fact, citing that source"
cmd 'claim Ana owns "March release" --source-last'
run claim Ana owns "March release" --confidence 0.9 --source-last --scope project:Demo >/dev/null
ok "Now there is a FACT, and it points to the meeting as its evidence."
pause

B "Step 4 · The agent asserts ANOTHER fact... but with no source record"
cmd 'claim Beto owns "the roadmap"'
run claim Beto owns "the roadmap" --confidence 0.5 --scope project:Demo >/dev/null
arrow "Nahuali stores it and reports that it has no source reference."
pause

B "Step 5 · You ask the agent: who owns the March release?"
cmd 'recall "March release" --authority'
R=$(run recall "March release" --authority --json --scope project:Demo)
EXC=$(echo "$R" | jq -r '.results[0].excerpt // "—"')
TCAN=$(echo "$R" | jq -r '.results[0].trust.can_trust')
TMODE=$(echo "$R" | jq -r '.results[0].trust.mode')
TWHY=$(echo "$R" | jq -r '.results[0].trust.reasons[0] // ""')
ACAN=$(echo "$R" | jq -r '.authority.can_trust')
AMODE=$(echo "$R" | jq -r '.authority.mode')
AWHY=$(echo "$R" | jq -r '.authority.reasons[0] // ""')
echo "   Nahuali finds:  \"$EXC\""
if [[ "$TCAN" == "true" ]]; then
  ok "CERTIFIED ($TMODE). It meets the configured evidence gate."
  dim "       engine reason: $TWHY"
fi
if [[ "$ACAN" != "true" ]]; then
  warn "But it WARNS about the whole set ($AMODE). Not all of the store is trustworthy:"
  dim "       engine reason: $AWHY"
fi
arrow "The response keeps the sourced result and the store-level warning distinct."
pause

B "Step 6 · What should you review next?"
cmd "review"
RV=$(run review --json)
CNT=$(echo "$RV" | jq -r '.summary.item_count // (.items|length)')
T0=$(echo "$RV" | jq -r '.items[0].title // "—"')
G0=$(echo "$RV" | jq -r '.items[0].operator_guidance // ""')
echo "   Nahuali hands you a prioritized list of $CNT thing(s) to review. The first:"
warn "$T0"
dim "       what to do: $G0"
arrow "It does not fix it on its own. It flags it and leaves the decision to you."
pause

B "The aha"
cat <<'EOF'

   Nahuali returned the sourced claim with its evidence and flagged the
   unsupported claim for review. The CERTIFY verdict means the configured
   structural evidence checks passed; it does not prove the meeting statement
   was true or that an external action is safe.

EOF
dim "   (Demo memory in $DB · synthetic · disposable test store)"
printf '\n'
