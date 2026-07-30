#!/bin/sh
set -eu

repo_root="${NAHUALI_WORKSPACE_ROOT:-}"
if [ -z "$repo_root" ]; then
  repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
fi

cd "$repo_root"

failures_file="$(mktemp)"
contract_failures_file="$(mktemp)"
first_contact_file="$(mktemp)"
trap 'rm -f "$failures_file" "$contract_failures_file" "$first_contact_file"' EXIT

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

for path in README.md BETA.md RELEASE_VERIFICATION.md crates/nahuali-core/README.md crates/nahuali-cli/README.md \
  crates/nahuali-mcp/ONBOARDING.md scripts .github/workflows; do
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

  grep -nE '(NAHUALI_VERSION=|--tag |releases/(download|tag)/)v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?' "$file" 2>/dev/null \
    | while IFS=: read -r line_number line; do
        printf '%s:%s: hardcoded concrete product release tag in %s\n' \
          "$relative" "$line_number" "$line" >>"$failures_file"
      done
done

if [ -s "$failures_file" ]; then
  echo "living release text must not pin concrete product release tags:" >&2
  sed 's/^/  - /' "$failures_file" >&2
  echo >&2
  echo "Use placeholder tags such as vX.Y.Z-beta.N in examples." >&2
  exit 1
fi

for path in README.md BETA.md RELEASE_VERIFICATION.md SECURITY.md SELF_REPAIR.md DISTRIBUTION_READINESS.md \
  ROADMAP.md TRUST_MODEL.md MEMORY_GOVERNANCE_LANDSCAPE.md \
  crates/nahuali-core/README.md crates/nahuali-cli/README.md \
  crates/nahuali-mcp/README.md crates/nahuali-mcp/ONBOARDING.md \
  crates/nahuali-api/README.md compliance examples \
  skills/nahuali; do
  scan_release_tags "$path"
done | sort | while IFS= read -r file; do
  relative="${file#./}"

  case "$relative" in
    *.avif|*.gif|*.ico|*.jpeg|*.jpg|*.json|*.pdf|*.png|*.webp|*.woff|*.woff2)
      continue
      ;;
  esac

  grep -niE \
    "world'?s first|industry[- ]first|the first (reliable|governed|trustworthy|tamper[- ]evident|agent) memory|best (agent )?memory|leading (agent )?memory|state[- ]of[- ]the[- ]art memory|production[- ]ready|detect(s|ed|ing)? (any|every) historical record rewrite|whether its recorded history was rewritten|your data is safe|exactly what the SEC requires|Nahuali (is|ships as) (an? )?open[- ]source|nothing was silently rewritten|safe to use as a supported memory claim|proving the history (is intact|was not altered)" \
    "$file" 2>/dev/null \
    | while IFS=: read -r line_number line; do
        printf '%s:%s: unsupported or inflated public claim in %s\n' \
          "$relative" "$line_number" "$line" >>"$contract_failures_file"
      done
done

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
# Literal Markdown code spans; shell expansion is intentionally disabled.
# shellcheck disable=SC2016
require_file_contains README.md '`MEMORY`'
# shellcheck disable=SC2016
require_file_contains README.md '`HISTORY`'
# shellcheck disable=SC2016
require_file_contains README.md '`EXTERNAL`'
require_file_contains README.md 'source-available under FSL-1\.1-MIT'
require_file_contains README.md 'independent reimplementation'
require_file_contains README.md 'NAHUALI_REQUIRE_SIGSTORE=1'
require_file_contains README.md 'GOVERNANCE_BENCHMARKS\.md'
require_file_contains README.md 'SELF_REPAIR\.md'
require_file_contains README.md 'BETA\.md'
require_file_contains README.md 'crates/nahuali-mcp/ONBOARDING\.md'
require_file_contains README.md 'scripts/verify-controlled-beta\.sh'
require_file_contains README.md '^## Build from source$'
require_file_contains scripts/install.sh '#build-from-source'
if grep -q '#install-from-source' scripts/install.sh; then
  printf '%s: references the removed #install-from-source README fragment\n' \
    scripts/install.sh >>"$contract_failures_file"
fi
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

sed -n '1,/^## What Nahuali checks$/p' README.md >"$first_contact_file"
if grep -niE 'merkle|ledger|checksum|sequence gap|hash chain|checkpoint|policy|signature|anchor|score' \
  "$first_contact_file" >>"$contract_failures_file"; then
  printf '%s\n' \
    'README.md: first-contact copy exposes implementation vocabulary before the product story' \
    >>"$contract_failures_file"
fi

python3 - "$contract_failures_file" <<'PY'
import pathlib
import re
import sys

failure_path = pathlib.Path(sys.argv[1])
roots = [
    pathlib.Path(name)
    for name in (
        "README.md",
        "BETA.md",
        "RELEASE_VERIFICATION.md",
        "SECURITY.md",
        "SELF_REPAIR.md",
        "DISTRIBUTION_READINESS.md",
        "ROADMAP.md",
        "TRUST_MODEL.md",
        "MEMORY_GOVERNANCE_LANDSCAPE.md",
        "compliance",
        "examples",
        "skills/nahuali",
        "crates/nahuali-core/README.md",
        "crates/nahuali-cli/README.md",
        "crates/nahuali-mcp/README.md",
        "crates/nahuali-mcp/ONBOARDING.md",
        "crates/nahuali-api/README.md",
    )
]
reference = re.compile(
    r"`((?:[A-Za-z0-9_.-]+/)+[A-Za-z0-9_.-]+|[A-Za-z0-9_.-]+\.(?:md|rs|yml|yaml|toml|json|sh|surql)):(\d+)(?:-(\d+))?`"
)
failures = []
unsafe_installer_pipe = re.compile(
    r"install\.sh[^\n]*(?:\\\n[^\n]*)?\|\s*(?:[A-Z_][A-Z0-9_]*=[^\s]+\s+)*sh"
)
for root in roots:
    files = root.rglob("*.md") if root.is_dir() else [root]
    for document in files:
        if not document.is_file():
            continue
        body = document.read_text(encoding="utf-8")
        unsafe_match = unsafe_installer_pipe.search(body)
        if unsafe_match:
            line = body.count("\n", 0, unsafe_match.start()) + 1
            failures.append(
                f"{document}:{line}: installer is piped directly into a shell"
            )
        for document_line, text in enumerate(body.splitlines(), 1):
            for match in reference.finditer(text):
                target = pathlib.Path(match.group(1))
                start = int(match.group(2))
                end = int(match.group(3) or start)
                if not target.is_file():
                    failures.append(
                        f"{document}:{document_line}: cited file does not exist: {target}"
                    )
                    continue
                line_count = sum(1 for _ in target.open(encoding="utf-8"))
                if start < 1 or end < start or end > line_count:
                    failures.append(
                        f"{document}:{document_line}: citation {target}:{start}-{end} exceeds {line_count} lines"
                    )
if failures:
    with failure_path.open("a", encoding="utf-8") as handle:
        for failure in failures:
            handle.write(failure + "\n")
PY

if [ -s "$contract_failures_file" ]; then
  echo "public contract references drifted:" >&2
  sed 's/^/  - /' "$contract_failures_file" >&2
  exit 1
fi

echo "living release text avoids concrete product release tags"
echo "public landing-page links and crate contracts are present"
