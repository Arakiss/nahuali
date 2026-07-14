#!/bin/sh
set -eu

repo_root="${NAHUALI_WORKSPACE_ROOT:-}"
if [ -z "$repo_root" ]; then
  repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
fi

cd "$repo_root"

failures_file="$(mktemp)"
contract_failures_file="$(mktemp)"
trap 'rm -f "$failures_file" "$contract_failures_file"' EXIT

scan_release_tags() {
  path="$1"

  if [ ! -e "$path" ]; then
    return
  fi

  if [ -d "$path" ]; then
    find "$path" -type f
  else
    printf '%s\n' "$path"
  fi
}

for path in README.md BETA.md crates/nahuali-core/README.md crates/nahuali-cli/README.md scripts .github/workflows; do
  scan_release_tags "$path"
done | sort | while IFS= read -r file; do
  relative="${file#./}"

  case "$relative" in
    CHANGELOG.md)
      continue
      ;;
    *.avif|*.gif|*.ico|*.jpeg|*.jpg|*.pdf|*.png|*.webp|*.woff|*.woff2)
      continue
      ;;
  esac

  grep -nE 'nahuali-cli-v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?' "$file" 2>/dev/null \
    | while IFS=: read -r line_number line; do
        printf '%s:%s: hardcoded concrete nahuali-cli release tag in %s\n' \
          "$relative" "$line_number" "$line" >>"$failures_file"
      done
done

if [ -s "$failures_file" ]; then
  echo "living release text must not pin concrete nahuali-cli release tags:" >&2
  sed 's/^/  - /' "$failures_file" >&2
  echo >&2
  echo "Use placeholder tags such as nahuali-cli-vX.Y.Z-beta.N in examples." >&2
  exit 1
fi

require_file_contains() {
  file="$1"
  pattern="$2"

  if [ ! -f "$file" ]; then
    printf '%s: missing required public contract artifact\n' "$file" >>"$contract_failures_file"
    return
  fi

  if ! grep -Eq "$pattern" "$file"; then
    printf '%s: missing %s\n' "$file" "$pattern" >>"$contract_failures_file"
  fi
}

require_file_contains crates/nahuali-core/schema/memory_record.surql 'DEFINE TABLE IF NOT EXISTS memory_record SCHEMALESS;'
require_file_contains crates/nahuali-core/schema/memory_record.surql 'DEFINE INDEX IF NOT EXISTS memory_record_sequence_idx ON TABLE memory_record COLUMNS sequence UNIQUE;'
require_file_contains README.md 'nahuali demo'
require_file_contains README.md 'GOVERNANCE_BENCHMARKS\.md'
require_file_contains README.md 'SELF_REPAIR\.md'
require_file_contains README.md 'BETA\.md'
require_file_contains README.md 'crates/nahuali-mcp/ONBOARDING\.md'
require_file_contains README.md 'scripts/verify-controlled-beta\.sh'
require_file_contains BETA.md 'scripts/verify-controlled-beta\.sh'
require_file_contains BETA.md 'self-inspection, review, reflection, sleep, consolidation, and proactive'
require_file_contains crates/nahuali-core/README.md 'memory_record'
require_file_contains crates/nahuali-core/README.md 'derived tiers'
require_file_contains crates/nahuali-cli/README.md 'memory_record'
require_file_contains crates/nahuali-mcp/README.md 'intention_update'
require_file_contains crates/nahuali-mcp/README.md 'goal_progress'
require_file_contains crates/nahuali-mcp/README.md 'anomaly_acknowledge'
require_file_contains crates/nahuali-mcp/README.md 'projection_validate'
require_file_contains crates/nahuali-mcp/README.md 'semantic_rebuild'
require_file_contains .gitignore '^docs/$'
require_file_contains .gitignore '^\.private/$'

if [ -s "$contract_failures_file" ]; then
  echo "public contract references drifted:" >&2
  sed 's/^/  - /' "$contract_failures_file" >&2
  exit 1
fi

echo "living release text avoids concrete nahuali-cli release tags"
echo "public landing-page links and crate contracts are present"
