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

cat <<EOF
Nahuali v${version} is a prerelease of the local trust layer for agent memory.
Every recall can carry its evidence and a deterministic verdict, and the ledger
can prove whether its recorded history was rewritten.

## Why upgrade

This beta keeps the published product, its benchmark evidence, and the exact
source tag aligned. It also prevents old and new embedded engine versions from
opening the same local store without a clear recovery path.

## Changes

$(sed 's/^### /### /' <<<"$section")

## Breaking changes and migration

This beta introduces no new memory-envelope migration. Existing version 1 and
version 2 records remain readable. After upgrading, restart every long-lived
\`nahuali-mcp\` host before using the CLI against the same local store.

## Install

\`\`\`bash
curl -fsSL https://raw.githubusercontent.com/Arakiss/nahuali/main/scripts/install.sh | sh
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
