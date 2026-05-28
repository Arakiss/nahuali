# Nahuali

<p align="center">
  <img src="assets/nahuali-cover.webp" alt="Nahuali memory graph: event stream, projection core, and inspected knowledge network" width="100%" />
</p>

<p align="center"><sub><em>Local memory that can inspect its own evidence before callers trust it.</em></sub></p>

<p align="center">
  <a href="https://github.com/Arakiss/nahuali/actions/workflows/ci.yml"><img src="https://github.com/Arakiss/nahuali/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-yellow.svg" alt="License: MIT"></a>
  <a href="rust-toolchain.toml"><img src="https://img.shields.io/badge/rust-stable-orange.svg" alt="Rust stable"></a>
  <a href="#self-inspecting-memory"><img src="https://img.shields.io/badge/memory-self--inspecting-blue.svg" alt="Self-inspecting memory"></a>
  <a href="fixtures/knowledge-health-regression.json"><img src="https://img.shields.io/badge/regression-fixtured-brightgreen.svg" alt="Regression fixtures"></a>
</p>

Nahuali is a pre-release Rust memory engine for local agent and operator
workflows. Its core idea is **self-inspecting memory**: memory should expose the
evidence, health signals, and authority decision behind recall before a caller
trusts it.

The engine records observations into an append-only ledger, rebuilds projected
memory from that ledger, and reports when projected knowledge is unsupported,
contradictory, stale, isolated, low-confidence, or missing source coverage.

The current project is intentionally small: a Rust core crate, a CLI, a local
MCP stdio server, a local HTTP API, fixtures, and release-gate scripts. It is
not a hosted product, not an accounts system, and not a general-purpose
database.

For where the project is going next, see [ROADMAP.md](ROADMAP.md). The roadmap
is directional; this README describes the current public surface.

## Origin

Nahuali started as a private internal prototype before this Rust OSS
foundation. That earlier work shaped the product thesis: long-running agent
memory should inspect its own evidence and health, not only retrieve more
context.

This repository is the clean public Rust line for that idea. It is not a
publication of every earlier experiment, private workflow, migration note, or
implementation detail. The public contract is intentionally limited to the
code, schemas, fixtures, examples, crate READMEs, and validation scripts in
this repository.

## What Exists Today

- `nahuali-core`: the canonical Rust engine.
- `nahuali`: a CLI for local memory recording, recall, inspection, backup, and
  migration rehearsals.
- `nahuali-mcp`: a local MCP stdio server over the same core.
- `nahuali-api`: a local HTTP API over the same core.
- `nahuali-regression`: a fixture runner used by release gates.

The repository is pre-1.0. The source tree is public, but the project should
still be treated as a beta foundation rather than a finished product.

## Self-Inspecting Memory

In this repository, self-inspection means concrete, local reports over the
current memory projection:

- `inspect` reports knowledge-health counts and signals.
- `recall --authority --json` returns recall results with the authority mode,
  trust flag, score, health report, and evidence IDs when available.
- `self-inspect` turns health and authority signals into proposed review work.
- `review` exposes a prioritized operator queue.
- `reflect`, `sleep`, `consolidation-plan`, and `proactive` plan follow-up work
  without writing memory automatically.

This is not a claim that Nahuali can prove remembered information is true. It is
a claim that Nahuali makes its current basis for trust inspectable: what
evidence exists, what is unsupported, what conflicts, what looks stale, and
what should be reviewed before acting on memory.

Practically, self-inspection means Nahuali can return useful memory while also
showing why the current store should or should not be trusted. A supported
answer can still come with a warning when the same store contains unrelated
unsupported claims, isolated entities, stale facts, contradictions, or source
coverage gaps. Review and repair remain explicit operator work; Nahuali does
not silently rewrite memory.

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

The local dev containers use project-specific names so they can coexist with
other Nahuali experiments on the same machine:

- `nahual-mictlan-surrealdb` stores the local ledger and graph projection.
- `nahual-tonalli-qdrant` stores the derived semantic index.

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

Start with the CLI. It is the fastest way to see the engine, the ledger, recall,
health inspection, and the review queue working together.

Run a synthetic end-to-end demo:

```bash
bash scripts/demo-self-inspecting-memory.sh
```

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

The HTTP API is not required for the first hands-on test. Use it when another
local program, service, or UI needs to call the same engine without shelling out
to the CLI.

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
  `intention`, `intention_update`, `intention_status`, and compatibility
  `fact`/`relate`
- recall and context: `recall`, `briefing`, `memory_hook`, `graph`, and
  `inspect`
- operator reports: `reconcile_intentions`, `goal_progress`, `proactive`,
  `deadlines`, `anomalies`, `anomaly_acknowledge`, `self_inspect`, `reflect`,
  `consolidation_plan`, `review`, and `review_resolve`
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

## Validation

Useful local checks:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo doc -p nahuali-core --no-deps
bash scripts/security-supply-chain-check.sh
bash scripts/verify-dogfood-daily-workflow.sh
bash scripts/verify-dogfood-migration.sh
bash scripts/verify-recall-contract.sh
```

The larger release gate is:

```bash
bash scripts/validate-clean-tree.sh
```

That gate runs formatting, clippy, workspace tests, documentation generation,
package and release dry-runs, install/coexistence checks, dogfood workflows,
regression fixtures, the recall contract smoke, and security checks.

## Current Limits

- No hosted service.
- No accounts, teams, tenants, API keys, billing, sync, or dashboards.
- No automatic memory repair or automatic consolidation write-back.
- No guarantee that remembered information is true; Nahuali reports evidence,
  confidence, and health signals so callers can decide whether to trust it.
- No stable 1.0 API guarantee yet.

See [ROADMAP.md](ROADMAP.md) for the longer-term direction. Roadmap items are
not release guarantees until they appear in code, tests, and tagged releases.

## Repository Layout

```text
crates/nahuali-core        Rust memory engine
crates/nahuali-cli         CLI crate; installs the nahuali command
crates/nahuali-mcp         MCP stdio server
crates/nahuali-api         Local HTTP API
crates/nahuali-regression  Regression fixture runner
fixtures                   Synthetic regression fixtures
examples                   Synthetic example inputs
scripts                    Release, validation, and safety checks
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT. See [LICENSE](LICENSE).
