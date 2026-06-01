# Changelog

## Unreleased

### Changed

- License moved from MIT to the Functional Source License (**FSL-1.1-MIT**)
  while the project is still pre-1.0. The code stays source-available — free to
  read, audit, use, modify, and self-host — and the only restriction is offering
  a competing commercial product or service. Under FSL, each release becomes
  MIT-licensed two years after it ships; the earlier MIT-licensed beta remains
  under MIT.

### Added

- Optional local `model2vec` embedder behind the `local-embeddings` build
  feature for real semantic recall, kept fully local and deterministic. The
  default build remains the deterministic embedder and pulls in no new
  dependencies. The semantic index schema version is now 3; run
  `semantic-rebuild` after switching embedders.
- Conventional Commit subject enforcement on pull requests, with an opt-in local
  `commit-msg` hook.

## 0.1.0-beta.0

Initial public beta seed for the local Rust memory engine.

Included:

- `nahuali-core` as the canonical ledger-backed engine crate.
- `nahuali` CLI for local recording, recall, inspection, review, backup,
  restore, import, and migration rehearsals.
- `nahuali-mcp` local stdio server over the same core.
- `nahuali-api` local HTTP API over the same core.
- SurrealDB `memory_record` ledger validation with rebuildable graph
  projection.
- Rebuildable Qdrant semantic index and hybrid recall.
- Evidence-backed recall, authority context, knowledge-health inspection, and
  non-mutating operator reports.
- Synthetic regression fixtures and release-candidate validation scripts.

Release-candidate validation:

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo doc -p nahuali-core --no-deps`
- `cargo package -p nahuali-core --allow-dirty --no-verify`
- `bash scripts/release-dry-run.sh`
- install, CLI coexistence, private dry-run, dogfood, migration, recall
  contract, regression fixture, documentation, and supply-chain smokes through
  `bash scripts/release-candidate-check.sh`

Boundaries:

- No hosted accounts, tenants, billing, sync, dashboards, or managed uptime.
- No stable 1.0 API guarantee.
- No claim that remembered information is true; Nahuali reports evidence,
  conflicts, staleness, and health signals so callers can decide what to trust.
