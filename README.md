# Nahuali

<p align="center">
  <img src="assets/nahuali-cover.webp" alt="Nahuali memory graph: event stream, projection core, and inspected knowledge network" width="100%" />
</p>

<p align="center"><sub><em>Local memory that can inspect its own evidence before callers trust it.</em></sub></p>

<p align="center">
  <a href="https://github.com/Arakiss/nahuali/actions/workflows/ci.yml"><img src="https://github.com/Arakiss/nahuali/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-yellow.svg" alt="License: MIT"></a>
  <a href="rust-toolchain.toml"><img src="https://img.shields.io/badge/rust-stable-orange.svg" alt="Rust stable"></a>
  <a href="fixtures/knowledge-health-regression.json"><img src="https://img.shields.io/badge/regression-fixtured-brightgreen.svg" alt="Regression fixtures"></a>
</p>

Nahuali is a pre-release Rust memory engine for local agent and operator
workflows. It records observations into an append-only ledger, rebuilds
projected memory from that ledger, and exposes recall together with evidence
and knowledge-health signals.

The current project is intentionally small: a Rust core crate, a CLI, a local
MCP stdio server, a local HTTP API, fixtures, and release-gate scripts. It is
not a hosted product, not an accounts system, and not a general-purpose
database.

## What Exists Today

- `nahuali-core`: the canonical Rust engine.
- `nahuali`: a CLI for local memory recording, recall, inspection, backup, and
  migration rehearsals.
- `nahuali-mcp`: a local MCP stdio server over the same core.
- `nahuali-api`: a local HTTP API over the same core.
- `nahuali-regression`: a fixture runner used by release gates.
- `packages/js`: an unpublished beta TypeScript client for the local HTTP API.
- `packages/python`: a README-only placeholder; no Python package exists yet.

The repository is pre-1.0. The source tree is public, but the project should
still be treated as a beta foundation rather than a finished product.

## Storage Contract

Nahuali stores authoritative history in a SurrealDB `memory_record` ledger.
Opening a database validates record sequence order and event checksums before
projecting current state.

Current projected state is derived from the ledger:

- Rust projection materializes episodes, claims, links, procedures, intentions,
  sources, review state, and health signals.
- SurrealDB graph projection tables are rebuildable derived state.
- Qdrant stores a rebuildable derived semantic index.

The ledger is the source of truth. Snapshots, graph projection tables, and
semantic vectors are maintenance or retrieval artifacts that must be rebuildable
from the ledger.

Detailed architecture, commercial planning, migration strategy, and internal
design documents remain private during pre-release development. The public
contract is the code, schema files, crate READMEs, fixtures, and validation
scripts in this repository.

## Install From Source

Start the local services:

```bash
docker compose up -d
```

Run the CLI from source:

```bash
cargo run -p nahuali-cli -- --database .nahuali-demo validate
```

Install local binaries when you explicitly want them on `PATH`:

```bash
cargo install --path crates/nahuali-cli --locked
cargo install --path crates/nahuali-mcp --locked
cargo install --path crates/nahuali-api --locked
nahuali --version
nahuali-mcp --version
nahuali-api --version
```

During pre-release work, prefer `cargo run` or an isolated install root if you
already have another `nahuali` command on your machine.

## Quickstart

Record a source episode and cite it as evidence:

```bash
cargo run -p nahuali-cli -- --database .nahuali-demo remember \
  "Lena owns the release notes." \
  --tag product \
  --mention Lena

cargo run -p nahuali-cli -- --database .nahuali-demo claim \
  Lena owns "release notes" \
  --confidence 0.92 \
  --source-last

cargo run -p nahuali-cli -- --database .nahuali-demo recall \
  "Lena release" \
  --authority \
  --json
```

Inspect memory before relying on it:

```bash
cargo run -p nahuali-cli -- --database .nahuali-demo inspect --json
cargo run -p nahuali-cli -- --database .nahuali-demo self-inspect --json
cargo run -p nahuali-cli -- --database .nahuali-demo review --json
```

Use explicit scopes when a memory belongs to a project, organization, personal
context, or custom boundary:

```bash
cargo run -p nahuali-cli -- --database .nahuali-demo remember \
  "Release notes belong to the Nahuali project." \
  --scope project:Nahuali

cargo run -p nahuali-cli -- --database .nahuali-demo recall \
  "release notes" \
  --scope project:Nahuali \
  --json
```

Scopes are labels for memory context. They are not authentication or
authorization boundaries.

## Local API

Run the local HTTP API:

```bash
cargo run -p nahuali-api -- --database .nahuali-demo --listen 127.0.0.1:7070
```

Query status and recall:

```bash
curl http://127.0.0.1:7070/v1/status

curl -X POST http://127.0.0.1:7070/v1/recall \
  -H 'content-type: application/json' \
  -d '{"query":"Lena release","limit":10}'
```

The HTTP API is a local beta surface. It does not include authentication,
tenants, billing, sync, dashboards, or hosted operations.

## MCP Stdio Server

Run the local MCP server:

```bash
cargo run -p nahuali-mcp -- --database .nahuali-demo
```

The MCP server exposes structured tools and read-only resources for local
clients. Tool names include:

- memory writes: `remember`, `claim`, `link`, `procedure`, `preference`,
  `intention`, `intention_update`, and `intention_status`
- recall and context: `recall`, `briefing`, `memory_hook`, `graph`, and
  `inspect`
- operator reports: `goal_progress`, `proactive`, `deadlines`, `anomalies`,
  `anomaly_acknowledge`, `self_inspect`, `reflect`, `review`, and
  `review_resolve`
- derived-tier maintenance: `projection_status`, `projection_rebuild`,
  `projection_validate`, `semantic_status`, and `semantic_rebuild`
- validation and ingestion: `validate`, `ingest`, and `ingest_text`

MCP responses are structured so clients do not need to scrape human CLI output.

## What Nahuali Checks

Nahuali is designed to make memory quality inspectable:

- unsupported derived memory
- low-confidence claims or links
- contradictions
- stale assertions
- isolated entities
- missing source coverage
- evidence gaps
- overdue or blocked intentions

Inspection, reflection, sleep, consolidation, and proactive reports are
non-mutating by default. Memory changes require explicit commands or tool calls.

## Backups And Migration Rehearsals

Local backups preserve the record ledger and restore only into an empty target
database:

```bash
cargo run -p nahuali-cli -- --database .nahuali-demo backup \
  --output .nahuali-demo.backup.json \
  --dry-run \
  --json

cargo run -p nahuali-cli -- backup-validate .nahuali-demo.backup.json --json

cargo run -p nahuali-cli -- backup-drill \
  .nahuali-demo.backup.json \
  --target-database .nahuali-demo-restored \
  --json
```

Interchange import/export is a separate source-neutral format. It is useful for
rehearsing migrations without treating old projection dumps as authoritative
record ledgers.

## JavaScript And Python

The JavaScript package is an unpublished beta client for `nahuali-api`:

```bash
bun test --cwd packages/js
```

It is marked private and is not published to npm.

Python bindings are deferred. `packages/python` is intentionally README-only.

## Validation

Useful local checks:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
bun test --cwd packages/js
cargo doc -p nahuali-core --no-deps
bash scripts/security-supply-chain-check.sh
bash scripts/verify-dogfood-daily-workflow.sh
bash scripts/verify-dogfood-migration.sh
bash scripts/verify-recall-evals.sh
```

The larger release gate is:

```bash
NAHUALI_VALIDATE_RUN_PROMPTFOO_EVALS=1 bash scripts/validate-clean-tree.sh
```

That gate runs formatting, clippy, workspace tests, documentation generation,
package and release dry-runs, install/coexistence checks, dogfood workflows,
regression fixtures, recall evals, and security checks.

## Current Limits

- No hosted service.
- No accounts, teams, tenants, API keys, billing, sync, or dashboards.
- No Python package.
- No npm publication.
- No automatic memory repair or automatic consolidation write-back.
- No guarantee that remembered information is true; Nahuali reports evidence,
  confidence, and health signals so callers can decide whether to trust it.
- No stable 1.0 API guarantee yet.

## Repository Layout

```text
crates/nahuali-core        Rust memory engine
crates/nahuali-cli         CLI crate; installs the nahuali command
crates/nahuali-mcp         MCP stdio server
crates/nahuali-api         Local HTTP API
crates/nahuali-regression  Regression fixture runner
packages/js                Unpublished TypeScript HTTP client
packages/python            Deferred Python placeholder
fixtures                   Synthetic regression fixtures
examples                   Synthetic example inputs
scripts                    Release, validation, and safety checks
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT. See [LICENSE](LICENSE).
