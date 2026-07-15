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

for required in \
  "### Why upgrade" \
  "### Breaking changes and migration" \
  "### Beta limits"; do
  grep -Fq "$required" <<<"$section" || {
    echo "release-notes: missing '$required' in CHANGELOG.md" >&2
    exit 1
  }
done

cat <<EOF
Nahuali v${version} is a prerelease of the local trust layer for agent memory.
Every recall can carry its evidence and a deterministic verdict, and the ledger
can prove whether its recorded history was rewritten.

$(sed 's/^### /## /' <<<"$section")

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
bash scripts/verify-release.sh --tag ${tag}
\`\`\`

The release contains four platform archives, mandatory SHA-256 checksums,
Sigstore bundles, GitHub artifact attestations, and one CycloneDX SBOM.

## Full changelog

See [CHANGELOG.md](https://github.com/Arakiss/nahuali/blob/${tag}/CHANGELOG.md)
for the product history. Crate changelogs are technical appendices only.
EOF
