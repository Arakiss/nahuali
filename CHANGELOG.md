# Changelog

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
