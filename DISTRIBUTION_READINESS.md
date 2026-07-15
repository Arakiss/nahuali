# Distribution Readiness

Nahuali is distributed as a local-first beta project. The supported channels are
source checkout workflows and GitHub prerelease binary archives produced by the
release workflow. Package registries and third-party package managers are not
supported distribution channels yet.

This page defines the non-destructive checks that make a commit ready for a
controlled beta or a prerelease decision. None of these commands publish crates,
create tags, upload release assets, or submit package-manager formulas.

## Current Channels

| Channel | Status | Evidence |
| --- | --- | --- |
| Source checkout | Supported beta path | `cargo run`, `cargo install --path`, demos, and controlled-beta gates |
| GitHub prerelease archives | Supported beta path after Release Please tag | `release.yml`, signed archives, SHA-256 files, release verification scripts |
| One-line installer | Supported only after a GitHub prerelease exists | `scripts/install.sh` resolves published GitHub release assets |
| Crates.io | Not supported yet | `cargo package` dry-runs only; no `cargo publish` path |
| Homebrew, npm, apt, containers, hosted service | Not supported | No formula, package, image, account, sync, billing, or hosted contract |

## Local Readiness Gate

Run the clean-tree gate before asking someone else to install or test a release
candidate:

```bash
bash scripts/validate-clean-tree.sh
```

The gate is intentionally non-publishing. It runs formatting, dependency-pin
checks, clippy, workspace tests, core docs, package dry-runs, release artifact
dry-runs, isolated install checks, CLI coexistence, dogfood workflows, regression
fixtures, benchmark checks, recall contract checks, and security checks.

For a faster distribution-only dry-run, build the local archive and checksum into
an ignored directory:

```bash
bash scripts/release-dry-run.sh --dist-dir .release-dry-run --keep
```

The generated `MANIFEST.txt` must report:

```text
publication=none
```

This proves the local archive shape without creating a tag, GitHub release,
registry package, or external upload.

## Remote Candidate Gate

After a candidate commit is pushed and CI is green, run:

```bash
bash scripts/release-candidate-check.sh
```

This checks public repository shape, branch and tag hygiene, prerelease-only
release state, issue triage, absence of tracked local artifacts, release-claim
language, security checks, documentation release references, and fresh-clone
validation. If a GitHub prerelease already exists for the current CLI version,
it also verifies the published release assets.

For repository settings that GitHub only exposes through authenticated API
calls, run:

```bash
NAHUALI_VERIFY_GITHUB_SETTINGS=1 bash scripts/security-supply-chain-check.sh
```

## Published Prerelease Verification

When Release Please creates a `vX.Y.Z-beta.N` product tag and the release
workflow uploads binaries, verify the release shape:

```bash
sh scripts/check-release-page.sh --tag vX.Y.Z-beta.N
sh scripts/check-release-assets.sh --tag vX.Y.Z-beta.N --require-sbom
bash scripts/verify-release.sh --tag vX.Y.Z-beta.N --require-sbom
```

The GitHub release page is a public product surface. The generated Release
Please changelog is only source material; it is not acceptable final copy. Each
beta release page must be curated before closeout with a product summary,
highlights, install instructions, verification commands, component versions,
explicit beta limits, and a changelog pointer. `check-release-page.sh` fails if
the page is empty, still looks generated, lacks the required sections, omits the
verification path, overpromises hosted service behavior, or has not yet received
the binary-channel assets.

The expected beta release shape is:

- four platform archives
- four `.sha256` checksum files
- four `.sigstore.json` Sigstore bundles
- one optional or required CycloneDX SBOM, depending on the command flag
- an install smoke test against the extracted `nahuali`, `nahuali-mcp`, and
  `nahuali-api` binaries

## Approval Boundary

These actions require explicit maintainer approval in the same release decision:

- merging a Release Please PR that cuts a prerelease tag
- manually dispatching `release.yml` for an existing tag
- creating, moving, or deleting release tags
- uploading, replacing, or deleting GitHub release assets
- publishing any crate to Crates.io
- submitting Homebrew, apt, container, npm, or other package-manager artifacts
- changing repository release settings, branch protection, or workflow
  permissions
- advertising a stable release, hosted service, sync, accounts, billing, or
  dashboard workflow

The default beta posture is conservative: validate locally, publish only through
the existing GitHub prerelease workflow, and do not add a new distribution
channel until the channel itself has a reviewed gate.

## Crates.io Preparation

Crates.io remains outside the supported distribution path. Before it can be
enabled, the repository needs a separate release decision that covers:

- crate ownership and token scope
- package metadata for every public crate
- registry dependency order, with `nahuali-core` published before crates that
  depend on it
- package contents review from `cargo package --list`
- a documented rollback story for a bad beta publish
- an explicit user-facing install path and version-support statement

Until that decision exists, crate packaging is only a local confidence check.
`cargo publish` is not part of the Nahuali beta gate.

## Stop Conditions

Do not proceed to a prerelease or wider tester handoff when any of these are
true:

- `main` CI is red for the candidate commit
- `bash scripts/validate-clean-tree.sh` fails
- the release dry-run does not produce an archive, checksum, and manifest with
  `publication=none`
- `bash scripts/release-candidate-check.sh` fails
- security or supply-chain checks fail
- public docs claim a distribution channel that is not implemented and verified
- local databases, backups, exports, private notes, or generated artifacts are
  tracked by Git

Readiness means the candidate can be verified and repeated. It does not mean a
new distribution channel is approved.
