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
can detect specified integrity failures. Rollback and a fully re-chained history
require comparison with an externally retained, authorized checkpoint.

## Why upgrade

This beta fixes friction reported from daily use. Superseded claims stay marked
in recall after review, ranking explains itself and favors recent, confident,
evidence-backed memory, parallel commands queue on the embedded store lock, and
claim values no longer appear as isolated entities. \`nahuali forget\` adds an
explicit, auditable erasure path that keeps the ledger verifiable.

## Changes

${section}

## Breaking changes and migration

Existing stores need no migration: the ledger is unchanged and the projection
(now version 8) is re-derived on read. Run \`nahuali reconcile\` to rebuild the
graph projection and semantic index right away, and take new snapshots, since
older snapshots fail \`snapshot-validate\` on the projection version.

\`nahuali forget\` writes two new record types. Releases before this beta refuse
to open a store after \`forget\` has run, so upgrade every CLI, \`nahuali-mcp\`,
and \`nahuali-api\` process first. JSON output changes are additive; recall
scores now include a recency component. After upgrading, restart every
long-lived \`nahuali-mcp\` host before using the CLI against the same local
store. See CHANGELOG.md for the full migration notes.

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
