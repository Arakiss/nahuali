# Nahuali

<p align="center">
  <img src="assets/nahuali-cover.webp" alt="Nahuali memory graph: event stream, projection core, and inspected knowledge network" width="100%" />
</p>

<p align="center"><sub><em>Auditable, tamper-evident memory for AI agents — it shows you the evidence before you trust it.</em></sub></p>

<p align="center">
  <a href="https://github.com/Arakiss/nahuali/actions/workflows/ci.yml"><img src="https://github.com/Arakiss/nahuali/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-FSL--1.1--MIT-yellow.svg" alt="License: FSL-1.1-MIT"></a>
  <a href="rust-toolchain.toml"><img src="https://img.shields.io/badge/rust-stable-orange.svg" alt="Rust stable"></a>
  <a href="#self-inspecting-memory"><img src="https://img.shields.io/badge/memory-self--inspecting-blue.svg" alt="Self-inspecting memory"></a>
  <a href="#tamper-evidence-and-attestation"><img src="https://img.shields.io/badge/ledger-tamper--evident-8a2be2.svg" alt="Tamper-evident ledger"></a>
  <a href="fixtures/knowledge-health-regression.json"><img src="https://img.shields.io/badge/regression-fixtured-brightgreen.svg" alt="Regression fixtures"></a>
</p>

Nahuali is a local-first, pre-release Rust memory engine for AI agents and
operator workflows. An agent that remembers across sessions accumulates a store
you eventually have to trust — and most memory layers give you no way to tell
which parts are supported, stale, contradictory, or quietly rewritten. Nahuali
treats memory as something you audit, not something you assume.

It does that two ways. **Self-inspecting memory** surfaces the evidence, health
signals, and authority decision behind a recall, so a caller can see *why* a
piece of memory should or should not be trusted. An optional **tamper-evident
ledger** binds the append-only history into an Ed25519-signable hash chain, so
you can prove the recorded past was not rewritten underneath you.

The current project is intentionally small: a Rust core crate, a CLI, a local
MCP stdio server, a local HTTP API, fixtures, and release-gate scripts. It is
not a hosted product, not an accounts system, and not a general-purpose
database.

For where the project is going next, see [ROADMAP.md](ROADMAP.md). The roadmap
is directional; this README describes the current public surface.

## Quickstart

Install the CLI, then see the trust story in about 30 seconds — no Docker, no
setup:

```bash
curl -fsSL https://raw.githubusercontent.com/Arakiss/nahuali/main/scripts/install.sh | sh
nahuali demo          # builds a tiny ledger, tampers with it, shows Nahuali catch it
nahuali init          # wire your agent harness to use Nahuali
```

`nahuali demo` runs the tamper-evidence story entirely in memory, with zero
dependencies. `nahuali init` installs the [Claude Code skill](skills/nahuali/)
and prints the MCP server config, so your agent uses governed memory as a
protocol — recall before assuming, assert only with evidence, read the per-result
trust decision. To make it binding for any harness, see the cross-harness
[protocol](skills/nahuali/protocol.md). Building from source instead:
`cargo install --path crates/nahuali-cli` (add `--features attestation` for the
signed-checkpoint and `demo` paths).

## How Memory Earns Trust

Most of Nahuali's surface exists to answer one question: should you trust this
memory right now? Four mechanisms build that answer, each inspectable on its
own.

- **Self-inspection.** Recall comes with health counts, trust signals, scores,
  and the authority decision for the store. Memory reports what is unsupported,
  contradictory, stale, isolated, low-confidence, or short on source coverage.
- **Provenance.** Derived claims and links cite the episodes they came from, and
  recall can return evidence IDs and require evidence, so an answer is traceable
  back to the observation that produced it.
- **Tamper-evident ledger** (opt-in). Each recorded event chains the previous
  event's hash, so rewriting any historical record breaks the chain at the next
  one and ledger replay detects it.
- **Tip attestation** (opt-in). The chain tip can be signed with an Ed25519 key,
  so even a full re-chain of the history — which repairs every internal link —
  fails verification against a receipt the attacker cannot forge.

The `trust-report` command (and the `trust_report` tool / `GET /v1/trust-report`)
composes these into one non-mutating verdict: knowledge counts, authority,
restated ledger integrity, knowledge health, and an overall `trustworthy` flag
with the reasons behind it.

None of this claims remembered information is *true*. It makes the current basis
for trust inspectable and the recorded history verifiable, and leaves the
decision to act with the operator.

## What Exists Today

- `nahuali-core`: the canonical Rust engine.
- `nahuali`: an agent-first CLI for local memory recording, recall, inspection,
  review, proactive signals, backup, and migration rehearsals.
- `nahuali-mcp`: a local MCP stdio server over the same core.
- `nahuali-api`: a local HTTP API over the same core.
- `nahuali-regression`: a fixture runner used by release gates.

The repository is pre-1.0. The source tree is public, but the project should
still be treated as a beta foundation rather than a finished product.

## Self-Inspecting Memory

In this repository, self-inspection means concrete, local reports over the
current memory projection:

- `inspect` reports knowledge-health counts and signals.
- `recall --authority --json` returns recall results with store-level
  authority, result-level trust, scores, health signals, and evidence IDs when
  available.
- `self-inspect` turns health and authority signals into proposed review work.
- `review` exposes a prioritized operator queue.
- `reflect`, `sleep`, `consolidation-plan`, and `proactive` plan follow-up work
  without writing memory automatically.

A supported answer can still come with a warning when the same store contains
unrelated unsupported claims, isolated entities, stale facts, contradictions, or
source coverage gaps. Review and repair stay explicit operator work; Nahuali
does not silently rewrite memory.

## Tamper-Evidence And Attestation

The history matters as much as the answer. By default Nahuali validates each
record's sequence and a self-contained checksum on open. That catches accidental
corruption, but a determined editor who rewrites a record and recomputes its
checksum would pass. Two opt-in build features close that gap and stay off by
default, so a default build writes byte-for-byte identical records.

Build the CLI with the audit chain, or with the chain plus signing:

```bash
# Hash-chained audit ledger: validate detects an in-place rewrite.
cargo build -p nahuali-cli --features tamper-evidence

# Chain plus Ed25519 tip signing (implies tamper-evidence).
cargo build -p nahuali-cli --features attestation
```

With `tamper-evidence`, every recorded event binds the previous event's chained
hash. Rewriting a historical record breaks the link at the next record, and
`validate` reports a broken chain even if the attacker forged a fresh per-record
checksum.

With `attestation`, sign the current tip and keep the receipt outside the store.
Nahuali never generates keys or touches the network — supply a 32-byte Ed25519
seed you control (for example `openssl rand -hex 32 > ledger.key`):

```bash
nahuali --database .nahuali-demo attest-sign --key-file ledger.key -o tip.json
nahuali --database .nahuali-demo attest-verify tip.json
```

`attest-verify` exits non-zero when the receipt no longer vouches for the live
ledger, so it can gate a script or CI. A full re-chain of the history changes
the tip, so the signed receipt stops verifying and forging a new one needs the
private key.

`audit` is a non-mutating diff of what the ledger recorded between two points,
with the integrity of that history restated next to it. It works in any build
(bounded by `--from`/`--to` sequence and `--since`/`--until` timestamp). On an
`attestation` build, anchor the lower bound on a signed receipt to diff exactly
what was appended since a verified checkpoint:

```bash
nahuali --database .nahuali-demo audit --from-attestation tip.json --json
```

It refuses to run when the receipt does not anchor a verified checkpoint in this
ledger, and exits non-zero when the audited history fails integrity
verification, so the diff can never claim to start from an unverified point.

Run the narrated walkthrough end to end:

```bash
cargo run -p nahuali-core --example tamper_evidence --features attestation
```

It shows a recomputed-checksum in-place rewrite being caught by the chain, and a
full suffix re-chain — which the chain alone cannot see — being caught by the
signed tip.

## Storage Contract

Nahuali stores authoritative history in a SurrealDB `memory_record` ledger.
Opening a database validates record sequence order and event checksums (and the
hash chain, when enabled) before projecting current state.

Current projected state is derived from the ledger:

- Rust projection materializes episodes, claims, links, procedures, intentions,
  sources, review state, and health signals.
- SurrealDB graph projection tables are rebuildable derived state.
- Qdrant stores a rebuildable derived semantic index.

The ledger is the source of truth. Snapshots, graph projection tables, and
semantic vectors are maintenance or retrieval artifacts that must be rebuildable
from the ledger.

Semantic vectors come from a deterministic local embedder by default. An
optional `local-embeddings` build feature swaps in a static model2vec model for
stronger semantic recall while staying fully local, offline, and deterministic;
no LLM is introduced into the core. Changing the embedder changes the vector
space, so rebuild the index with `semantic-rebuild` afterwards.

### Optional: stronger semantic recall with a local model

The default deterministic embedder hashes tokens, so two phrasings that share no
words look unrelated to it. A static [model2vec](https://github.com/MinishLab/model2vec)
model places synonyms close instead. Nahuali never downloads models — you point
it at a directory you control, so the core stays offline by construction.

Fetch a model once (any model2vec export works; the directory must hold
`tokenizer.json`, `model.safetensors`, and `config.json`):

```bash
mkdir -p models/potion-retrieval-32M
base="https://huggingface.co/minishlab/potion-retrieval-32M/resolve/main"
for f in tokenizer.json model.safetensors config.json; do
  curl -L -o "models/potion-retrieval-32M/$f" "$base/$f"
done
```

Build with the feature, point the environment at the model, then rebuild the
index and recall by meaning:

```bash
export NAHUALI_EMBEDDING_PROVIDER=model2vec
export NAHUALI_LOCAL_EMBEDDING_MODEL_PATH="$PWD/models/potion-retrieval-32M"

cargo run -p nahuali-cli --features local-embeddings -- \
  --database .nahuali-demo semantic-rebuild
cargo run -p nahuali-cli --features local-embeddings -- \
  --database .nahuali-demo recall --semantic "driving a car to work"
```

A query like `driving a car to work` then surfaces an episode such as "Lena
commutes by automobile each morning" — no shared words, ranked by meaning. The
deterministic embedder scores that episode the same as an unrelated one; the
model separates them (see `local_model_separates_meaning_where_deterministic_cannot`
in `nahuali-core`).

Detailed architecture, commercial planning, migration strategy, and internal
design documents remain private during pre-release development. The public
contract is the code, schema files, crate READMEs, fixtures, and validation
scripts in this repository.

## Install From Source

For the one-line binary install see [Quickstart](#quickstart); this section is
for building from source and running the full local stack.

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

Run the agent-first daily-driver loop demo:

```bash
bash scripts/demo-daily-driver-loop.sh
```

Run the controlled beta gate before asking another technical user to test a
checkout:

```bash
bash scripts/verify-controlled-beta.sh
```

See [BETA.md](BETA.md) for the current controlled testing boundary, tester
rules, passing criteria, and blockers.

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

## Agent-First CLI

The CLI is the canonical local interface for agents and operators during the
beta foundation phase. Agent-facing usage should prefer `--json`, explicit
flags, scoped databases, and non-mutating inspection commands. Human-readable
output is kept useful, but the machine contract comes first.

The expected local loop is:

1. `validate --json` before relying on a database.
2. `session-resume --json` or `briefing --json` before planning work.
3. `recall --authority --json` before using remembered context.
4. `inspect --json`, `self-inspect --json`, and `review --json` before
   repairing weak memory.
5. `goal-progress --json`, `deadlines --json`, `anomalies --json`, and
   `proactive --json` for forward-looking work.

MCP exposes the same core to agent hosts that prefer tool calls. The HTTP API
serves local integrations and UI/service experiments. Neither replaces the CLI
as the fastest way to inspect and validate the engine locally.

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

For one-line setup (the copy-paste `.mcp.json`, the trust value, and a first
session), see [the MCP onboarding guide](crates/nahuali-mcp/ONBOARDING.md), or
run `nahuali init`. Run the local MCP server directly:

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
- ledger inspection: `validate`, `audit`, and `trust_report`
- ingestion: `ingest` and `ingest_text`

MCP responses are structured so clients do not need to scrape human CLI output.
Each tool advertises a typed JSON Schema for its output in `tools/list` and
returns matching structured content, so a host can validate results against the
schema instead of parsing prose. An integration test freezes that surface
against drift.

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

Interchange import/export is a separate source-neutral format, useful for
rehearsing migrations without treating old projection dumps as authoritative
record ledgers. Imports are applied as a single batched ledger flush, so loading
a large history stays fast.

## Validation

Useful local checks:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo doc -p nahuali-core --no-deps
bash scripts/security-supply-chain-check.sh
bash scripts/verify-controlled-beta.sh
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

## Release Automation

Nahuali uses semantic commits and Release Please for the public beta release
train.

- Commits should use conventional prefixes such as `feat:`, `fix:`, `docs:`,
  `test:`, `ci:`, `chore:`, `refactor:`, `perf:`, or `security:`.
- Pushes to `main` run CI and update the Release Please PR when a releasable
  change exists.
- Merging the Release Please PR creates the version tag and GitHub prerelease.
- Binary artifacts are built only for `nahuali-cli-vX.Y.Z-beta.N` tags.
- Manual workflow dispatch remains available for release-pr and artifact
  reruns.
- Repository workflow permissions stay read-only by default; Release Please
  requires the repository setting that allows GitHub Actions to create pull
  requests, while write scopes remain limited to the release job. Verify that
  setting with:
  `NAHUALI_VERIFY_GITHUB_SETTINGS=1 bash scripts/security-supply-chain-check.sh`.

The release train stays prerelease-only while the project is in beta.

## Current Limits

- No hosted service.
- No accounts, teams, tenants, API keys, billing, sync, or dashboards.
- No automatic memory repair or automatic consolidation write-back.
- No guarantee that remembered information is true; Nahuali reports evidence,
  confidence, and health signals so callers can decide whether to trust it.
- Tamper-evidence and tip attestation are opt-in build features, off by default.
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

Nahuali is source-available under the Functional Source License, version 1.1,
with an MIT future grant (**FSL-1.1-MIT**). You may use, copy, modify, and
self-host it for any purpose other than offering a competing commercial product
or service. Two years after each version is published, that version also becomes
available to you under the MIT license. See [LICENSE](LICENSE).
