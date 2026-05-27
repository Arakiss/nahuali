# Nahuali

<p align="center">
  <img src="assets/nahuali-cover.webp" alt="Nahuali memory graph: event stream, projection core, and inspected knowledge network" width="100%" />
</p>

<p align="center"><sub><em>Operational memory that inspects itself before it is trusted.</em></sub></p>

<p align="center">
  <a href="https://github.com/Arakiss/nahuali/actions/workflows/ci.yml"><img src="https://github.com/Arakiss/nahuali/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-yellow.svg" alt="License: MIT"></a>
  <a href="rust-toolchain.toml"><img src="https://img.shields.io/badge/rust-stable-orange.svg" alt="Rust stable"></a>
  <a href="docker-compose.yml"><img src="https://img.shields.io/badge/self--hostable-core-brightgreen.svg" alt="Self-hostable core"></a>
  <a href="#why"><img src="https://img.shields.io/badge/memory-self--inspecting-blue.svg" alt="Self-inspecting memory"></a>
  <a href="fixtures/knowledge-health-regression.json"><img src="https://img.shields.io/badge/regression-fixtured-brightgreen.svg" alt="Regression fixtures"></a>
</p>

> _Memory should know when it is not safe to be believed._

**Self-inspecting memory for AI agents. Evidence-backed recall.
Knowledge-health signals. Explainable semantic recall.**

Nahuali is a Rust memory engine for agents that need more than a bag of
recalled text. It maintains an auditable record ledger, graph-shaped projection,
explainable retrieval, and governance signals behind a narrow core API. The
engine retrieves memory with evidence and reports when current knowledge is
unsupported, contradictory, stale, weak, or incomplete.

The long-term target is larger than prompt recall: Nahuali should be able to map
a life, a project, or an organization as an evolving memory graph. Like a brain
during rest, it should consolidate what happened, surface contradictions, mark
stale knowledge, expose blind spots, and propose what needs attention next.
That makes Nahuali a candidate cognitive memory substrate for agents, operating
systems, developer tools, personal knowledge systems, and organization-scale
knowledge infrastructure.

The premise is simple: memory should not only answer "what do we know?" It
should also answer "how safe is it to rely on this, and what are we missing?"

## Origin

Nahuali started as an internal prototype before this Rust OSS foundation. That
earlier work shaped the product thesis: agent memory must inspect itself, not
only retrieve context. This repository is the clean Rust OSS foundation for that
idea, not a publication of every internal experiment or implementation detail.

## Why

Most agent memory systems optimize for recall volume. That is not enough for
long-running agents. A memory can be retrievable and still be unsafe: detached
from evidence, contradicted by newer observations, stale, low-confidence, or
silent about blind spots.

Nahuali takes the opposite stance:

- **Records first.** Observed episodes are the auditable ground truth.
- **Derived memory cites evidence.** Claims, links, procedures, and intentions
  can point back to the episode that supports them.
- **Source coverage is inspectable.** Episodes and derived memory are checked
  for missing provenance instead of being treated as equally trustworthy.
- **Projection is deterministic.** The current memory state is rebuilt from the
  validated record ledger.
- **Recall is explainable.** Results include kind, score, excerpt, matched
  terms, and evidence IDs when available.
- **Context is explicit.** Personal, project, organization, and custom scopes
  let one memory engine keep boundaries inspectable without pretending they are
  permissions.
- **Inspection is part of the core.** Knowledge health is a first-class output,
  not an afterthought.
- **Self-inspecting by default.** The engine, CLI, and MCP server are designed
  around knowledge-health checks before trust.

## Status

This repository is pre-release while the OSS foundation hardens. The current
release surface is intentionally narrow:

- `nahuali-core`: canonical Rust memory engine
- `nahuali`: installed CLI for humans, scripts, and agents
- `nahuali-mcp`: local MCP stdio server over the same core
- `nahuali-api`: local HTTP API beta surface over the same core
- `nahuali-regression`: internal fixture runner for release-gate regression checks

The project is not yet published to crates.io. Source installation is the
supported path until the public binary and registry release strategy is
finalized.

Python bindings are deferred from the first public release. The JavaScript
package is an unpublished beta client for the local HTTP API and is not
published to npm.

New Rust integrations should use `nahuali_core::MemoryEngine`. `LocalMemory`,
`Fact`, `Relation`, `add_fact`, and `relate` remain compatibility names for
pre-release callers and migration bridges.

The public storage contract is deliberately narrow in v1: SurrealDB stores the
append-only `memory_record` ledger, Rust projection materializes the graph-shaped
memory view, SurrealDB graph tables are rebuildable derived projection state, and
Qdrant stores a rebuildable derived semantic index. Internal design notes, beta
planning, and migration strategy documents remain private during pre-release
development.

The beta boundary is also narrow: this repository targets a self-hosted OSS
memory engine, not a hosted commercial service. The ledger remains
authoritative; derived graph and vector tiers must be rebuildable from the
ledger before they are trusted by release gates.

This repository keeps a non-loss migration rule: do not treat the OSS engine as
a drop-in replacement for any existing workflow until each capability has an OSS
equivalent or an explicit deferral.

## Install From Source

During pre-release development, avoid overwriting an existing global `nahuali`
command. Use `cargo run` or install into an isolated root until the CLI cutover
is intentional:

```bash
docker compose up -d
cargo run -p nahuali-cli -- --database .nahuali-demo validate
cargo run -p nahuali-mcp -- --database .nahuali-demo
cargo run -p nahuali-api -- --database .nahuali-demo --listen 127.0.0.1:7070
```

After cutover, install the local binaries from the repository root:

```bash
cargo install --path crates/nahuali-cli --locked
cargo install --path crates/nahuali-mcp --locked
cargo install --path crates/nahuali-api --locked
nahuali --version
nahuali-mcp --version
nahuali-api --version
```

The release gate smoke-tests this exact path in an isolated temporary install
root.

For transition-safe dogfooding, keep the existing global `nahuali` command
unchanged and use isolated source runs until the Rust CLI cutover is
intentional. The coexistence gate verifies that the Rust CLI can be used without
changing the global command:

```bash
bash scripts/verify-cli-coexistence.sh
```

The dogfood migration rehearsal validates the source-preserving import, recall,
inspection, backup, drill, and restore path with synthetic memory before any
real cutover:

```bash
bash scripts/verify-dogfood-migration.sh
```

For a sensitive export, use the private dry-run wrapper instead of hand-running
migration commands. It keeps derived artifacts in an ignored path or outside the
repository, does not copy the original input, and writes both a human summary
and a machine-readable `summary.json` with aggregate counts:

```bash
scripts/private-memory-dry-run.sh --input "$PRIVATE_EXPORT" --input-kind legacy
```

Prerelease automation exists for versioned binary archives, but the validated
day-to-day developer path remains `cargo run` and isolated source installs until
the public cutover is intentional.

## Quickstart

Start the local runtime first:

```bash
docker compose up -d
```

Use a demo database:

```bash
nahuali --database .nahuali-demo remember "Lena owns the release notes" --tag product --mention Lena --mention "Release Notes"
nahuali --database .nahuali-demo ingest examples/ingest-conversation.json --dry-run --json
nahuali --database .nahuali-demo ingest examples/ingest-conversation.json --json
nahuali --database .nahuali-demo claim Lena owns "release notes" --confidence 0.92 --source-last
nahuali --database .nahuali-demo link Lena owns "release notes" --confidence 0.9 --source-last
nahuali --database .nahuali-demo remember "Release notes belong to the Nahuali project" --scope project:Nahuali
nahuali --database .nahuali-demo recall "release notes" --scope project:Nahuali --json
nahuali --database .nahuali-demo preference "Release notes" "Keep release notes concise" --source-last
nahuali --database .nahuali-demo intention "Ship release notes" --priority high --source-last
nahuali --database .nahuali-demo intention-update <intention_id> --deadline-at-ms 1777939200000 --progress 25
nahuali --database .nahuali-demo intention-complete <intention_id> --reason "Release notes shipped"
nahuali --database .nahuali-demo reconcile-intentions --json
nahuali --database .nahuali-demo goal-progress --json
nahuali --database .nahuali-demo proactive --json
nahuali --database .nahuali-demo deadlines --json
nahuali --database .nahuali-demo anomalies --json
nahuali --database .nahuali-demo anomaly-acknowledge <anomaly_id> --note "Operator reviewed this alert" --dry-run --json
nahuali --database .nahuali-demo briefing --json
nahuali --database .nahuali-demo hook pre-prompt --input "Who owns release notes?" --json
nahuali --database .nahuali-demo sleep --json
nahuali --database .nahuali-demo consolidation-plan --json
nahuali --database .nahuali-demo recall "Lena release"
nahuali --database .nahuali-demo graph "Lena" --depth 2 --json
nahuali --database .nahuali-demo project "Lena"
nahuali --database .nahuali-demo semantic-rebuild
nahuali --database .nahuali-demo semantic-status --json
nahuali --database .nahuali-demo recall "Lena release" --semantic --json
nahuali --database .nahuali-demo inspect --json
nahuali --database .nahuali-demo self-inspect --json
nahuali --database .nahuali-demo reflect --json
nahuali --database .nahuali-demo review --json
nahuali --database .nahuali-demo review-resolve <review_id> --note "Operator reviewed this item" --dry-run --json
nahuali --database .nahuali-demo validate --json
nahuali --database .nahuali-demo maintenance
nahuali --database .nahuali-demo snapshot --output .nahuali-demo.snapshot.json --dry-run
nahuali --database .nahuali-demo snapshot --output .nahuali-demo.snapshot.json
nahuali --database .nahuali-demo snapshot-validate .nahuali-demo.snapshot.json --json
nahuali --database .nahuali-demo backup --output .nahuali-demo.backup.json --dry-run --json
nahuali --database .nahuali-demo backup --output .nahuali-demo.backup.json
nahuali backup-validate .nahuali-demo.backup.json --json
nahuali backup-drill .nahuali-demo.backup.json --target-database .nahuali-demo-restored --json
nahuali restore .nahuali-demo.backup.json --target-database .nahuali-demo-restored --dry-run --json
nahuali --database .nahuali-demo export --output .nahuali-demo.interchange.json
nahuali --database .nahuali-oss/imported import .nahuali-demo.interchange.json --dry-run --json
```

The local HTTP API uses the same Rust core and storage contract:

```bash
cargo run -p nahuali-api -- --database .nahuali-demo --listen 127.0.0.1:7070
curl http://127.0.0.1:7070/v1/status
curl -X POST http://127.0.0.1:7070/v1/recall \
  -H 'content-type: application/json' \
  -d '{"query":"Lena release","limit":10}'
```

This API is a local beta surface. It is not an accounts, tenant-management, or
hosted-control-plane layer.

The selected database is a SurrealDB database name. Path-like values such as
`.nahuali-demo` are accepted for operator convenience and normalized before
SurrealDB selection. That database stores the authoritative `memory_record`
ledger. State is rebuilt by projecting records in Rust, not by trusting a
mutable snapshot or by reading private-product graph tables.

Ingestion documents are adapter contracts for source material. They register
source provenance, append source episodes, and write only the explicit derived
records included in the document. Dry-run ingestion validates the whole document
without mutating the ledger, including a preflight summary of scope, source
size, evidence gaps, and episode coverage.

`briefing --json` is the default pre-work surface for agents and operators. It
returns authority, health, recent episodes, active intentions, high-priority
review work, and graph seeds without writing memory.

`hook <kind> --json` is the host contract for deterministic memory context at
runtime. Hosts can call `session-start`, `pre-prompt`, `post-action`,
`session-close`, or `sleep-cycle` hooks so memory recall, authority, review
state, and consolidation guidance are loaded deliberately instead of depending
on the model to remember to ask.

`sleep --json` runs a non-mutating rest pass over recent memory. It replays
recent episodes, inspects health, proposes consolidation candidates, and keeps
every write behind explicit operator approval.

`consolidation-plan --json` turns rest, reflection, and review signals into an
explicit pipeline: replay evidence, extract candidates, reconcile them, gate
operator review, and report commit eligibility. It is still non-mutating and
keeps automatic write-back disabled.

`reflect --json` is the non-mutating consolidation planning surface. It groups
self-inspection findings into operator-approved cycles, reports source/evidence
coverage, and keeps write-back explicit.

`self-inspect --json` also reports source-coverage gaps when episodes lack
source records or derived memory lacks source episode evidence. These gaps are
review work, not automatic repair work.

The OSS local runtime intentionally uses non-default host ports so it can run
beside other local services without stealing their defaults.

Interchange documents are source-neutral import/export documents. They are not
record ledgers or snapshots, and imports append new events after validating the
whole document. They can carry source records, sourced episode positions, and
evidence links. Import dry-runs include preflight counts for source coverage,
scope, evidence coverage, and a self-inspection readiness forecast before
migrated memory is applied.

Historical exports can be bridged into interchange with
`convert-legacy-export`, which accepts structured export documents and
deterministic SurrealQL export bundles. This is a bridge into the OSS
interchange contract, not a promise of legacy TypeScript schema parity.
`convert-projection-export` remains available for projected-memory dump shapes
and conservative envelope aliases.

Backups are authoritative record-ledger manifests for local durability. Restore
requires an empty target database and preserves event envelopes exactly.
Run `backup-drill` before restore to validate the backup and dry-run the target.
Derived retrieval indexes are treated as rebuildable state. Run
`semantic-rebuild` after restore to recreate the semantic index from the
restored records.

`--source-last` cites the most recently recorded episode as evidence for the
fact or relation being added. That keeps the quickstart copy-pasteable while
still producing supported memory instead of detached assertions.

`proactive --json` composes deadline signals, anomaly alerts, evidence capture
opportunities, and high-risk review work into a non-mutating operator report.
Use `deadlines --json` and `anomalies --json` for narrower scriptable views.
`anomaly-acknowledge <anomaly_id> --note ...` is the explicit audit path for
acknowledging an alert; `--dry-run --json` previews the append-only review
decision before writing it.

`--scope kind:name` records or recalls memory inside an explicit context
boundary such as `personal:Operator`, `project:Nahuali`, or
`organization:ExampleCo`. Scoped recall returns only records in that exact
scope; unscoped recall remains the broad compatibility path.

## Scripted CLI Use

Primary commands support `--json` when a script or agent needs structured output
instead of human text:

```bash
nahuali --database .nahuali-demo remember "Lena owns the release notes" --tag product --mention Lena --json
nahuali --database .nahuali-demo ingest examples/ingest-conversation.json --dry-run --json
nahuali --database .nahuali-demo claim Lena owns "release notes" --confidence 0.92 --source-last --json
nahuali --database .nahuali-demo link Lena owns "release notes" --confidence 0.9 --source-last --json
nahuali --database .nahuali-demo preference "Release notes" "Keep release notes concise" --source-last --json
nahuali --database .nahuali-demo intention "Ship release notes" --priority high --source-last --json
nahuali --database .nahuali-demo intention-update <intention_id> --goal <goal_id> --depends-on <dependency_id> --progress 25 --json
nahuali --database .nahuali-demo reconcile-intentions --now-ms 1777939200000 --json
nahuali --database .nahuali-demo goal-progress --json
nahuali --database .nahuali-demo proactive --json
nahuali --database .nahuali-demo deadlines --json
nahuali --database .nahuali-demo anomalies --json
nahuali --database .nahuali-demo anomaly-acknowledge <anomaly_id> --note "Operator reviewed this alert" --json
nahuali --database .nahuali-demo briefing --json
nahuali --database .nahuali-demo hook pre-prompt --input "Who owns release notes?" --json
nahuali --database .nahuali-demo sleep --json
nahuali --database .nahuali-demo consolidation-plan --json
nahuali --database .nahuali-demo data --json
nahuali --database .nahuali-demo recall "Lena release" --json
nahuali --database .nahuali-demo graph "Lena" --depth 2 --limit 20 --json
nahuali --database .nahuali-demo project "Lena" --json
nahuali --database .nahuali-demo semantic-rebuild --json
nahuali --database .nahuali-demo recall "Lena release" --semantic --json
nahuali --database .nahuali-demo reflect --json
nahuali --database .nahuali-demo review --limit 5 --min-priority high --json
nahuali --database .nahuali-demo review-resolve <review_id> --note "Operator reviewed this item" --json
```

JSON mode writes valid JSON to stdout with no surrounding prose. Human-readable
output remains the default.

## MCP Stdio Server

Nahuali ships a local MCP stdio server for agent clients:

```bash
nahuali-mcp --database .nahuali-demo
```

The server follows the MCP `2025-11-25` baseline through the official Rust SDK.
It exposes:

- tools for `remember`, `claim`, `fact`, `link`, `relate`, `procedure`,
  `preference`, `intention`, `intention_update`, `intention_status`,
  `reconcile_intentions`, `goal_progress`, `proactive`, `deadlines`,
  `anomalies`, `anomaly_acknowledge`, `ingest`, `ingest_text`, `briefing`,
  `memory_hook`, `recall`, `graph`, `inspect`, `self_inspect`, `reflect`,
  `consolidation_plan`, `review`, `review_resolve`, `projection_status`,
  `projection_rebuild`, `projection_validate`, `semantic_status`,
  `semantic_rebuild`, and `validate`; recall responses include authority data
  in MCP, and the CLI can request the same shape with
  `recall --authority --json`
- read-only JSON resources for database summary, sources, health, entities,
  episodes, claims, links, facts, relations, procedures, intentions, and records
- prompts for health-checked recall and evidence-backed claim recording

Tool calls return structured content so agents can consume memory results
without scraping prose. Resources are read-only context; memory mutation stays
behind explicit tool calls.

For host-managed rest cycles, MCP clients call `memory_hook` with
`kind=sleep_cycle`. The structured response includes the same Sleep Mode report
as the CLI command.

MCP clients can also call `consolidation_plan` when a host needs the explicit
replay, extraction, reconciliation, review-gate, and commit-eligibility plan
without invoking the CLI.

For operator-loop parity, MCP clients can update intention metadata, reconcile
intentions, inspect goal progress, read proactive/deadline/anomaly reports, and
acknowledge anomaly alerts without invoking the CLI. These reports are
non-mutating except for explicit status/update/acknowledgement tool calls.

## Core Contract

Nahuali's current public contract is deliberately small:

1. Opening a database validates record sequence order and event checksums before
   projection.
2. Every mutation records an envelope; projected state can be rebuilt from the
   record ledger.
3. SurrealDB stores the authoritative `memory_record` ledger and a rebuildable
   graph projection for entities, episodes, claims, relations, intentions,
   procedures, health signals, and review/audit state.
4. Source records preserve ingestion provenance for documents, transcripts, and
   conversations.
5. Episodes can cite source provenance and source-local position or role.
6. Ingestion validates an entire source-neutral document before appending
   source, episode, claim, link, procedure, or intention records.
7. Ingestion reports include preflight counts for scope, source size, derived
   records, evidence gaps, and episode coverage before writes are applied.
8. Claims, links, procedures, and intentions may cite a source episode. Facts
   and relations remain compatibility names for claims and links.
9. Lexical recall returns scored candidates with matched terms and evidence IDs.
10. Inspection reports support, contradictions, freshness/staleness,
   connectivity, isolated entities, low confidence, and blind spots.
11. Authority decisions classify memory as `certify`, `advisory`, `warn`, or
   `block` before agents trust recall.
12. Corrupt record ledgers fail closed instead of being silently repaired.
13. Snapshots are optional maintenance artifacts and must validate against a
   fresh record replay before use.
14. Local backups preserve the authoritative record ledger and restore only into
   an empty database before semantic indexes are rebuilt.
15. Backup drills validate a backup and dry-run restore into a target database
    without writing records.
16. SurrealDB graph projection state is derived from the record ledger and can
    be inspected, rebuilt, and validated with `projection-status`,
    `projection-rebuild`, and `projection-validate`.
17. Semantic indexes are derived from the current projection and can be rebuilt
    explicitly with `semantic-rebuild`.
18. Hybrid recall keeps lexical, semantic, evidence, and authority components
    visible instead of collapsing them into an opaque score.
19. Explicit scopes can label personal, project, organization, or custom
    contexts. Scoped recall is an exact filter, while unscoped memory remains
    readable through the compatibility path.
20. Graph traversal returns deterministic memory neighborhoods with nodes,
    edges, evidence IDs, depth, authority, and health/review overlays.
21. Project reports return a focused entity or project dashboard with the
    matched entity, graph neighborhood, recall results, evidence-backed memory,
    authority, health, and review context.
22. Briefing reports provide the default non-mutating pre-work context: health,
    authority, recent episodes, active intentions, review priorities, and graph
    seeds.
23. Self-inspection reports are non-mutating consolidation passes that convert
    health and authority signals into evidence-backed findings and proposed
    review queue items.
24. Reflection reports group self-inspection findings into non-mutating
    operator-approved cycles with source/evidence coverage.
25. Sleep Mode reports replay recent episodes, inspect memory health, and
    propose consolidation candidates without writing memory.
26. Consolidation-plan reports turn rest and review signals into explicit
    replay, extraction, reconciliation, review-gate, and commit-eligibility
    operations without writing memory.
27. Operator review reports turn self-inspection into a prioritized, filterable
    queue without automatically writing memory.
28. Review resolutions require an explicit operator note and append an audit
    decision instead of silently rewriting memory.
29. Proactive reports compose deadlines, anomaly alerts, capture opportunities,
    and high-risk review work without writing memory; anomaly acknowledgements
    require an explicit operator note and append an audit decision.
30. Interchange import/export uses a separate source-neutral format with
    source provenance so migration bridges do not become the record-ledger
    contract.

Detailed architecture, public API, compatibility, and self-hosting planning
documents remain private during pre-release development. The public contract in
this repository is the source code, crate READMEs, CLI/API JSON behavior,
schema files, fixtures, and validation scripts.

## OSS Boundary

Nahuali OSS is the self-inspecting memory engine: core crate, CLI, MCP server,
record-ledger contract, local validation, snapshots, regression fixtures, and
release gates. It does not include a hosted service, accounts, teams, billing,
sync, dashboards, or managed deployments.

The OSS durability baseline is local and verifiable: the record ledger is
authoritative, snapshots must validate against replay, backups preserve records
exactly, and import/export uses a source-neutral interchange format. Managed
backup scheduling, retention policies, off-site encrypted snapshots,
point-in-time restore, workspace policy, audit, and SLA-backed recovery belong
above the core in a hosted or commercial layer.

Technical extension points are the Rust API, public API compatibility policy,
CLI JSON output, MCP surface, record-ledger validation, and future bindings
after the core release-candidate freeze. Hosted or commercial products can be
built above the core, but the OSS repo does not promise free hosted operations.

## Regression Guarantee

Nahuali's release gate checks more than "it compiles":

- Rust formatting
- workspace tests
- clippy with warnings denied
- generated public docs for `nahuali-core`
- `nahuali-core` package smoke
- installed CLI/MCP/API source-install smoke
- source-neutral interchange and source/time-preserving projected-export migration rehearsal
- synthetic knowledge-health regression fixtures
- privacy and secret scans
- license, metadata, and supply-chain checks

The fixture suite covers supported memory, unsupported facts, contradictions,
staleness, deterministic authority modes, record-ledger integrity, recall
ranking, no-match recall, partial matches, procedure recall, intention recall,
and recall authority coupling.

App-backed evals cover higher-level behavior that should stay stable across
retrieval or agent-facing changes. The initial Promptfoo suite seeds synthetic
memory through the real CLI and checks scoped, evidence-backed recall:

```bash
bash scripts/verify-recall-evals.sh
```

## Workspace

```txt
crates/nahuali-core   Self-inspecting memory engine
crates/nahuali-cli    Native CLI crate; installs the nahuali command
crates/nahuali-mcp    MCP stdio server crate; installs the nahuali-mcp command
crates/nahuali-api    HTTP API crate; installs the nahuali-api command
crates/nahuali-regression  Internal regression fixture runner
packages/python       Deferred Python binding placeholder
packages/js           Private beta TypeScript HTTP client
fixtures              Reproducible release-gate fixtures
examples              Synthetic workflows
```

## Validation

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
bun test --cwd packages/js
cargo doc -p nahuali-core --no-deps
cargo package -p nahuali-core --allow-dirty --no-verify
bash scripts/release-dry-run.sh
bash scripts/verify-install.sh
bash scripts/verify-cli-coexistence.sh
bash scripts/security-supply-chain-check.sh
cargo run -p nahuali-regression -- --fixtures fixtures/knowledge-health-regression.json
bash scripts/verify-recall-evals.sh
bash scripts/validate-clean-tree.sh
```

`validate-clean-tree.sh` keeps the Promptfoo suite opt-in so normal CI does not
depend on first-run package download latency:

```bash
NAHUALI_VALIDATE_RUN_PROMPTFOO_EVALS=1 bash scripts/validate-clean-tree.sh
```

Release-candidate freeze:

```bash
bash scripts/release-candidate-check.sh
```

## Roadmap

Near-term:

- public release artifacts after the release gate is solid
- release-candidate hardening after the public API and binding freeze
- operator-reviewed self-inspection write-back hardening across CLI, MCP, and
  compatibility tests
- cooling-off review before any public visibility change

Later:

- Python bindings and deeper JavaScript bindings over `nahuali-core`
- versioned compaction over the record ledger
- hosted backup automation, retention, encrypted off-site storage, and
  point-in-time restore as managed-product surfaces
- richer scoring with explainability preserved
- optional sync/server layers that remain thin wrappers over the local core

## Not In Scope

Nahuali is an engine, not a hosted platform:

- not a generic retrieval database
- not a free hosted service
- not a managed control plane
- not a replacement for OS or process sandboxing
- not a secret store
- not a guarantee that remembered information is true

Nahuali reports the projected memory state, evidence links, confidence, and
health signals so callers can decide how much authority to give the memory.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT. See [LICENSE](LICENSE).
