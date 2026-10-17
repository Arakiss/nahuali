#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

version="${1:-$(tr -d '[:space:]' < version.txt)}"
tag="v${version}"

[[ "$version" =~ ^0\.[0-9]+\.[0-9]+-beta\.[0-9]+$ ]] || {
  echo "release-notes: expected a pre-1.0 beta version, got '$version'" >&2
  exit 1
}

section="$(
  awk -v heading="## [$version]" '
    index($0, heading) == 1 { active = 1; next }
    active && /^## \[/ { exit }
    active { print }
  ' CHANGELOG.md
)"

[[ -n "$section" ]] || {
  echo "release-notes: CHANGELOG.md has no [$version] section" >&2
  exit 1
}

# The release page prose lives in the version's own changelog entry, so it can
# never carry over from an earlier release. Print one `### <name>` subsection.
subsection() {
  awk -v heading="### $1" '
    $0 == heading { active = 1; next }
    active && /^#{2,3} / { exit }
    active { print }
  ' <<<"$section" | sed -e '/./,$!d'
}

why_upgrade="$(subsection "Why upgrade")"
upgrade_notes="$(subsection "Upgrade notes")"
for required in "Why upgrade:$why_upgrade" "Upgrade notes:$upgrade_notes"; do
  [[ -n "$(tr -d '[:space:]' <<<"${required#*:}")" ]] || {
    echo "release-notes: CHANGELOG.md [$version] needs a non-empty '### ${required%%:*}' subsection" >&2
    exit 1
  }
done

changes="$(
  awk '
    $0 == "### Why upgrade" || $0 == "### Upgrade notes" { skip = 1; next }
    /^### / { skip = 0 }
    !skip { print }
  ' <<<"$section"
)"

cat <<EOF
Nahuali v${version} is a prerelease of the local trust layer for agent memory.
Every recall can carry its evidence and a deterministic verdict, and the ledger
can detect specified integrity failures. Rollback and a fully re-chained history
require comparison with an externally retained, authorized checkpoint.

## Why upgrade

${why_upgrade}

## Changes

${changes}

## Breaking changes and migration

${upgrade_notes}

See CHANGELOG.md for the full product history.

## Install

\`\`\`bash
curl -fsSLo /tmp/nahuali-install.sh \\
  https://raw.githubusercontent.com/Arakiss/nahuali/${tag}/scripts/install.sh
NAHUALI_VERSION=${tag} sh /tmp/nahuali-install.sh
nahuali demo
nahuali explore
\`\`\`

To pin this release, set \`NAHUALI_VERSION=${tag}\` before running the installer.

## Verify the release

\`\`\`bash
bash scripts/check-release-assets.sh --tag ${tag} --require-sbom
bash scripts/verify-release.sh --tag ${tag} --require-sbom --require-provenance
\`\`\`

The release contains four platform archives, mandatory SHA-256 checksums,
Sigstore bundles, GitHub artifact attestations, and one CycloneDX SBOM.

## Beta limits

- No stable 1.0 API guarantee yet.
- No hosted service, accounts, teams, billing, managed sync, or managed uptime.
- Nahuali evaluates evidence and memory health. It does not claim that recalled
  information is objectively true.

## Full changelog

See [CHANGELOG.md](https://github.com/Arakiss/nahuali/blob/${tag}/CHANGELOG.md)
for the product history. Crate changelogs are technical appendices only.
EOF
