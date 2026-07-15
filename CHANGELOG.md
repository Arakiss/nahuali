# Changelog

This is the release history of Nahuali as one product. It covers the CLI, MCP
server, local HTTP API, Rust core, and the user-facing terminal interface.
Internal crate changelogs are technical appendices, not separate product
release histories.

## Unreleased

### Changed

- The installer now detects a running `nahuali-mcp` process and asks the user
  to restart its host after an upgrade, preventing old and new embedded engine
  versions from opening the same store.

## [0.8.0-beta.3](https://github.com/Arakiss/nahuali/compare/v0.8.0-beta.2...v0.8.0-beta.3) (2026-07-15)


### Fixed

* prevent release verification from contending with active stores ([777d617](https://github.com/Arakiss/nahuali/commit/777d61740ea975329fd6fedc9f5a9c8abfcb750d))

## [0.8.0-beta.2](https://github.com/Arakiss/nahuali/compare/v0.8.0-beta.1...v0.8.0-beta.2) (2026-07-15)


### New

* make governed recall visible end to end ([56fdb14](https://github.com/Arakiss/nahuali/commit/56fdb14dd2c145860f9a7a93491ef9329d1866f1))
* make the TUI mascot crisp and reliably visible ([94caaef](https://github.com/Arakiss/nahuali/commit/94caaef7ed02943702322c1d5fe9e54b9b96f38d))


### Fixed

* keep the mascot asset within release limits ([ab16d31](https://github.com/Arakiss/nahuali/commit/ab16d3171cef05c981e5d82e62f0fb276aacaa7a))
* render curated notes from generated changelogs ([84d8d8f](https://github.com/Arakiss/nahuali/commit/84d8d8fbe9cadc4781cf61397612d35db1632e05))

## [0.8.0-beta.1](https://github.com/Arakiss/nahuali/compare/v0.8.0-beta.0...v0.8.0-beta.1) (2026-07-15)


### Fixed

* bind benchmark evidence to product releases ([0968b1b](https://github.com/Arakiss/nahuali/commit/0968b1be2da221274b77fef8946ae9d0889f0d80))
* make required CI independent of trigger type ([acc3836](https://github.com/Arakiss/nahuali/commit/acc38364d8d1052eebad59a105ff8b878a791bc2))
* prevent mixed-engine store upgrades ([d742e66](https://github.com/Arakiss/nahuali/commit/d742e6601756e2dedd27d754c64f61424d63def3))
* separate release proposals from public evidence ([8e38aa2](https://github.com/Arakiss/nahuali/commit/8e38aa2d8ed7a667b28397ece3843a35dec7783e))

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
- Embedded storage uses SurrealDB 3.1.5, which fixes the upstream cold-start
  session race that could intermittently abort a fresh local database.

### Security and integrity

- New ledger records use SHA-256 checksums and bind the preceding record hash.
- Install archives require SHA-256 verification and include Sigstore bundles and
  GitHub artifact attestations.
- Signed tip verification detects a fully recomputed ledger suffix that a hash
  chain alone cannot distinguish from a legitimate rewrite.

### Beta limits

- No stable 1.0 API guarantee yet.
- No hosted service, accounts, teams, billing, managed sync, or managed uptime.
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
