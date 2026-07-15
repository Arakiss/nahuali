# Changelog

This is the release history of Nahuali as one product. It covers the CLI, MCP
server, local HTTP API, Rust core, and the user-facing terminal interface.
Internal crate changelogs are technical appendices, not separate product
release histories.

## Unreleased

No user-facing changes yet.

## [0.8.0-beta.0] - 2026-07-15

### Why upgrade

This release makes the product's original promise usable without external
services: memory can be recorded, inspected, recalled with evidence, and given a
deterministic trust verdict from one local installation.

### New

- Embedded local storage for the default CLI, MCP, and HTTP workflows. Docker is
  no longer required to try or operate the core memory path.
- A trust-first `nahuali explore` terminal interface for browsing memory,
  evidence, store health, and ledger integrity.
- `nahuali demo`, `nahuali init`, evidence-required recall, self-inspection,
  review queues, trust reports, governed repair, snapshots, and migration tools.
- Official MCP Registry metadata and a multi-architecture MCP container image.

### Changed

- Tamper-evident SHA-256 chaining and Ed25519 tip attestation are enabled in
  normal builds. An unchained legacy build now requires an explicit opt-out.
- Strict validation rejects unchained or partially chained ledgers unless the
  caller deliberately selects the legacy-permissive path.
- Self-inspection distinguishes unsupported, stale, contradictory, and malformed
  memory more precisely and reports deduplicated affected-record counts.
- All shipped components now share one product version and one public release
  tag. The earlier `1.0.0-beta.0` and `1.1.0-beta.0` publications were premature
  automation errors and are superseded by this pre-1.0 release.

### Breaking changes and migration

- Strict validation now fails closed for unchained ledgers. Use
  `--allow-unchained` only while migrating a legacy store.
- New records use envelope version 2 and SHA-256 checksums. Existing version 1
  records remain readable and valid; they are not rewritten.
- The former `--require-chained` opt-in was replaced by the explicit
  `--allow-unchained` compatibility flag.

### Fixed

- Database names that would previously be normalized into a different name are
  rejected with a useful error.
- Configuration precedence is deterministic: command-line flag, environment,
  then built-in default.
- Same-observation multi-value facts are no longer misreported as contradictions.

### Security and integrity

- New ledger records use SHA-256 checksums and bind the preceding record hash.
- Install archives require SHA-256 verification and include Sigstore bundles and
  GitHub artifact attestations.
- Signed tip verification detects a fully recomputed ledger suffix that a hash
  chain alone cannot distinguish from a legitimate rewrite.

### Beta limits

- No stable 1.0 API guarantee yet.
- No hosted accounts, teams, billing, managed sync, or managed uptime.
- Nahuali evaluates evidence and memory health. It does not claim that recalled
  information is objectively true.

## [0.6.1-beta.0] - 2026-07-06

- Published verified macOS and Linux archives for x86_64 and arm64.
- Added mandatory checksums, Sigstore bundles, and release verification scripts.
- Curated the release page around evidence-backed recall, ledger integrity, and
  the supported local beta path.

## [0.6.0-beta.0] - 2026-06-20

- Added deterministic governed repair and the `nahuali repair` command.
- Added repair events to the append-only record model and surfaced repair needs
  without silently mutating memory.

## [0.5.0-beta.0] - 2026-06-14

- Added Merkle inclusion proofs to ledger audits.
- Made strict validation require chained records.
- Stabilized the CLI JSON output contract.

## [0.4.0-beta.0] - 2026-06-13

- Added the `explore` TUI, zero-service demo, harness initialization, ledger
  audit, trust report, temporal recall, archive recall, and reconciliation.
- Enabled tamper evidence by default for the CLI and added signed tip
  verification through a trusted keyring.
- Introduced the shared clay-on-coffee terminal presentation.

## [0.3.0-beta.0] - 2026-06-02

- Introduced the tamper-evident hash-chained ledger.
- Surfaced trust verdicts in normal CLI output.
- Added grouped help, shell completions, typed MCP results, and typed HTTP API
  response schemas.
- Changed the license to FSL-1.1-MIT. Each release converts to MIT two years
  after publication.

## [0.2.0-beta.0] - 2026-06-01

- Attached result-level trust to authority-ranked recall.
- Added Conventional Commit validation for release inputs.

## [0.1.0-beta.0] - 2026-06-01

- First public beta of the local Rust memory engine, CLI, MCP server, HTTP API,
  evidence-backed recall, knowledge-health inspection, and regression fixtures.

[0.8.0-beta.0]: https://github.com/Arakiss/nahuali/compare/nahuali-cli-v0.6.1-beta.0...v0.8.0-beta.0
[0.6.1-beta.0]: https://github.com/Arakiss/nahuali/compare/nahuali-cli-v0.6.0-beta.0...nahuali-cli-v0.6.1-beta.0
[0.6.0-beta.0]: https://github.com/Arakiss/nahuali/compare/nahuali-cli-v0.5.0-beta.0...nahuali-cli-v0.6.0-beta.0
[0.5.0-beta.0]: https://github.com/Arakiss/nahuali/compare/nahuali-cli-v0.4.0-beta.0...nahuali-cli-v0.5.0-beta.0
[0.4.0-beta.0]: https://github.com/Arakiss/nahuali/compare/nahuali-cli-v0.3.0-beta.0...nahuali-cli-v0.4.0-beta.0
[0.3.0-beta.0]: https://github.com/Arakiss/nahuali/compare/nahuali-cli-v0.2.0-beta.0...nahuali-cli-v0.3.0-beta.0
[0.2.0-beta.0]: https://github.com/Arakiss/nahuali/compare/nahuali-cli-v0.1.0-beta.0...nahuali-cli-v0.2.0-beta.0
[0.1.0-beta.0]: https://github.com/Arakiss/nahuali/releases/tag/nahuali-cli-v0.1.0-beta.0
