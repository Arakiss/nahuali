# Nahuali

<p align="center">
  <img src="assets/nahuali-cover.webp" alt="Nahuali memory graph: event stream, projection core, and inspected knowledge network" width="100%" />
</p>

<p align="center"><sub><em>Memory for AI agents that shows its evidence and can prove its history was not rewritten.</em></sub></p>

<p align="center">
  <a href="https://github.com/Arakiss/nahuali/actions/workflows/ci.yml"><img src="https://github.com/Arakiss/nahuali/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-FSL--1.1--MIT-yellow.svg" alt="License: FSL-1.1-MIT"></a>
  <a href="rust-toolchain.toml"><img src="https://img.shields.io/badge/rust-stable-orange.svg" alt="Rust stable"></a>
  <a href="#self-inspecting-memory"><img src="https://img.shields.io/badge/memory-self--inspecting-blue.svg" alt="Self-inspecting memory"></a>
  <a href="#tamper-evidence-and-attestation"><img src="https://img.shields.io/badge/ledger-tamper--evident-8a2be2.svg" alt="Tamper-evident ledger"></a>
  <a href="fixtures/knowledge-health-regression.json"><img src="https://img.shields.io/badge/regression-fixtured-brightgreen.svg" alt="Regression fixtures"></a>
</p>

Nahuali is a local-first, pre-release Rust memory engine for AI agents and
operator workflows, built around an uncomfortable fact: an agent that remembers
across sessions accumulates a store you eventually have to trust blindly, and
agent memory has become an attack surface. OWASP's agentic Top 10 lists memory
poisoning ([ASI06](https://owasp.org/www-project-agent-memory-guard/)) as a
first-class risk, the EU AI
Act's [Article 12](https://ai-act-service-desk.ec.europa.eu/en/ai-act/article-12)
requires automatic event logs from high-risk systems, and most memory layers still
cannot tell you which parts of what they return are supported, stale,
contradictory, or quietly rewritten. Nahuali treats memory as something you
audit, not something you assume.

Three mechanisms make that concrete:

- **A trust verdict on every recall.** Results carry evidence IDs, health
  signals, and an authority decision: certify, advisory, warn, or block, so a
  caller sees *why* a memory should or should not be trusted before acting on
  it.
- **A tamper-evident history.** Every default build hash-chains each recorded
  event and can sign the chain tip with Ed25519 — both on by default, so the
  claim is true out of the box. A full re-chain of the past still fails against a
  signed receipt. You can prove the recorded history was not rewritten underneath
  you.
- **Governance benchmarks you can re-run.** Tamper detection, provenance
  coverage, contradiction detection, key lifecycle, and verdict calibration are
  measured by reproducible fixtures in this repository, not asserted in
  marketing copy. It is the first reproducible agent-memory governance suite we
  have found in the current external claim review.

The current project is intentionally small: a Rust core crate, a CLI with an
interactive governance cockpit, a local MCP stdio server, a local HTTP API,
fixtures, and release-gate scripts. It is not a hosted product, not an
accounts system, and not a general-purpose database.

An LLM can also propose repairs to that governed memory, consolidating repeated
observations into a claim, or linking entities, while the deterministic engine
validates, classifies, gates, and records each one as an audited, reversible
event. The core never calls an LLM. See the
[Self-Repair Contract](SELF_REPAIR.md).

For where the project is going next, see [ROADMAP.md](ROADMAP.md). The roadmap
is directional; this README describes the current public surface. For the
prior-art, security, regulatory, and benchmark context behind the dated
comparison claim, see [Agent-Memory Governance
Landscape](MEMORY_GOVERNANCE_LANDSCAPE.md).

External comparison claims are not evergreen. The current review date is
2026-06-14; treat the competitive claim as stale after 2026-09-14, or before
quoting it in a new public release, launch post, benchmark report, investor
note, or customer-facing document, unless
[`MEMORY_GOVERNANCE_LANDSCAPE.md`](MEMORY_GOVERNANCE_LANDSCAPE.md) has been
refreshed.

## Quickstart

Install the CLI, then see the trust story in about 30 seconds with no Docker and
no setup:

```bash
curl -fsSL https://raw.githubusercontent.com/Arakiss/nahuali/main/scripts/install.sh | sh
nahuali demo          # builds a tiny ledger, tampers with it, shows Nahuali catch it
nahuali init          # wire your agent harness to use Nahuali
```

`nahuali demo` runs the tamper-evidence story entirely in memory, with zero
dependencies. `nahuali init` installs the [Claude Code skill](skills/nahuali/)
and prints the MCP server config, so your agent uses governed memory as a
protocol: recall before assuming, assert only with evidence, read the per-result
trust decision. To make it binding for any harness, see the cross-harness
[protocol](skills/nahuali/protocol.md). Building from source instead:
`cargo install --path crates/nahuali-cli` — the default build already includes
the signed-checkpoint and `demo` paths (attestation is a default feature).

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
- **Tamper-evident ledger** (default-on in the CLI, MCP, and API builds;
  feature-gated in the core library). Each recorded event chains the previous
  event's hash, so
  rewriting any historical record breaks the chain at the next one and ledger
  replay detects it.
- **Tip attestation** (on by default). The chain tip can be signed with an
  Ed25519 key, so even a full re-chain of the history, which repairs every
  internal link, fails verification against a receipt the attacker cannot forge.

The `trust-report` command (and the `trust_report` tool / `GET /v1/trust-report`)
composes these into one non-mutating verdict: knowledge counts, authority,
restated ledger integrity, knowledge health, and an overall `trustworthy` flag
with the reasons behind it.

None of this claims remembered information is *true*. It makes the current basis
for trust inspectable and the recorded history verifiable, and leaves the
decision to act with the operator.

## Governance Benchmarks

Trust should be a number you can recompute, not a word in a README. Nahuali
defines its own governance benchmarks: each injects a fixed, labeled corpus,
runs the real engine validators over it, and computes a rate. The measurement
lives in the library and the release gate, so anyone can rerun it; the same
reproducibility the engine asks of memory, applied to its own numbers. The
established agent-memory benchmarks (LOCOMO, LongMemEval, BEAM) measure recall
accuracy only; as of the 2026-06-14 external claim review, we know of no other
reproducible governance suite for agent memory, which is why Nahuali defines
and gates its own. See
[Governance Benchmark Methodology](GOVERNANCE_BENCHMARKS.md) for the corpus,
formula, commands, and limits behind these numbers, and
[Agent-Memory Governance Landscape](MEMORY_GOVERNANCE_LANDSCAPE.md#benchmark-gap)
for why this is a separate axis from recall benchmarks.

**Ledger Integrity Verification Rate (LIVR).** Detection rate `TP / (TP + FN)`
of ledger tampering, reported per detector tier over a nine-class synthetic
attack corpus, with the per-tier blind spot made explicit rather than averaged
away:

| Detector tier | Detection rate | What it adds |
|---|---|---|
| checksum-only | 0.22 | the naive baseline: only a stale or corrupted per-event checksum |
| replay-chain | 0.78 | + in-place rewrites, timestamp skew, sequence gaps, cross-ledger grafts, stripped chain links |
| attestation-tip | 1.00 | + fully re-chained suffixes, with zero false positives on the clean chained control |

```bash
cargo run -p nahuali-regression --features attestation -- --livr
```

**Provenance Coverage Rate (PCR).** The fraction of assertional memory that is
traceable to a source episode (`evidence_backed / total`), with its inverse, the
overconfidence rate (high-confidence-but-unsourced). The labeled fixture seeds a
known mix and the engine recovers it exactly: 0.75 coverage and 0.25
overconfidence over eight claims, with an `insufficient_samples` guard so a
small store cannot report a misleading rate:

```bash
cargo run -p nahuali-regression -- --fixtures fixtures/provenance-coverage-regression.json
```

**Contradiction & Staleness Detection Rate (CDR).** Detection of seeded
knowledge-health defects: same-observation contradictions, recency-resolved
supersessions, and time-stale facts, over a labeled corpus, paired with a clean
control that must produce zero false positives. The engine detects all six
seeded defects and stays silent on the consistent store:

```bash
cargo run -p nahuali-regression -- --fixtures fixtures/contradiction-staleness-regression.json
```

**Attestation Recovery Profile (ARP).** A coverage check over the attestation
key lifecycle: a live receipt is honored, a re-chained suffix voids the old
receipt, a rotated key's receipt is honored, a revoked key's cryptographically
valid receipt is rejected, and a receipt over a different ledger's tip is
rejected. Reported as a pass/fail profile across the matrix, not a single rate:

```bash
cargo run -p nahuali-regression --features attestation -- --arp
```

**Trust Verdict Soundness (TVS).** The recall-side calibration check: one labeled
store per authority mode, asserting the gate spans the full range correctly: a
clean connected store certifies, an isolated-entity store degrades to advisory,
an unsupported assertion warns, and a same-observation contradiction blocks. It
shows the trust gate is calibrated, not merely present:

```bash
cargo run -p nahuali-regression -- --fixtures fixtures/trust-verdict-soundness-regression.json
```

Honest limits: LIVR measures detection against a fixed synthetic injection
method, so a passing rate proves self-consistency detection, not the absence of
all tampering. PCR is an evidence-*presence* audit, not a calibration or
accuracy metric. It says a claim cites an observation, not that the claim is
true. CDR measures detection of the defect classes the health pipeline models,
not every possible inconsistency. ARP measures the keyring's behavior across a
fixed lifecycle matrix, not the strength of the underlying signature scheme. TVS
asserts the gate's verdict on labeled stores, not the correctness of the
underlying facts.

## How Nahuali Compares

This comparison is dated. It reflects the external claim review recorded on
2026-06-14 in
[`MEMORY_GOVERNANCE_LANDSCAPE.md`](MEMORY_GOVERNANCE_LANDSCAPE.md). Refresh
that document before treating the comparison as current after 2026-09-14 or
before quoting it externally.

Most public-code agent-memory engines (Mem0, Zep/Graphiti, Letta, Cognee)
optimize for **recall accuracy**, extracting and retrieving the right context,
and publish their LOCOMO and LongMemEval scores. Nahuali optimizes for a
different axis: **whether you can trust and verify** what memory returns. The two
are complementary, and the comparison cuts both ways:

| Axis | Nahuali | Recall-first engines |
|---|---|---|
| Tamper-evidence over the memory log | hash chain + Merkle proofs + Ed25519 tip attestation | none in the public layer |
| Confidence-vs-provenance recall trust | flags overconfident unsourced memory, gates recall | stores asserted facts without a provenance audit |
| Deterministic core (no LLM in recall/ingest) | yes, reviewable, reproducible | LLM-driven extraction (non-deterministic) |
| Point-in-time / bi-temporal recall | created-time filtering (`--as-of-ms`, `--max-age-days`); not yet a full valid/invalid interval model | Zep's bi-temporal model still leads |
| Raw recall accuracy (LOCOMO/LongMemEval) | not the goal; a credible floor, not the lead | strong published numbers |
| Ecosystem, integrations, traction | pre-release, narrow surface | large communities and framework integrations |

As of the 2026-06-14 external claim review, we have not found another
publicly inspectable agent-memory engine that combines all three of a hash-chained
Merkle-proofed ledger, detached Ed25519 tip attestation, and a per-recall
confidence-vs-provenance trust verdict over its memory ledger.
The closest prior art ships subsets:
[SuperLocalMemory](https://arxiv.org/abs/2603.02240) hash-chains compliance
events and scores writer trust against memory poisoning, but carries no
evidence-or-freshness verdict on the recall path; MentisDB stores hash-chained
entries without a governance benchmark suite or recall-path trust verdict;
OpenFang Merkle-chains agent *actions*, not a memory store; and the "Right to
History" prototype ([arXiv 2602.20214](https://arxiv.org/abs/2602.20214))
explores RFC 6962 audit logs for agents in research form. If you only need
maximum recall accuracy, a recall-first engine is the better fit; Nahuali is
for when you also have to defend what the memory says and prove it was not
rewritten. The fuller landscape and source-quality caveats are maintained in
[Agent-Memory Governance Landscape](MEMORY_GOVERNANCE_LANDSCAPE.md).

## What Exists Today

- `nahuali-core`: the canonical Rust engine.
- `nahuali`: an agent-first CLI for local memory recording, recall, inspection,
  review, governed repair, proactive signals, backup, and migration rehearsals,
  plus an interactive `explore` governance cockpit and read-only federated recall
  over an archive store (`recall --archive`).
- `nahuali-mcp`: a local MCP stdio server over the same core.
- `nahuali-api`: a local HTTP API over the same core.
- `nahuali-ui`: the shared terminal presentation crate (palette, tables, the
  cockpit widgets).
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
source coverage gaps. Repair is governed and append-only: an LLM can propose a
consolidation or link, the engine validates and gates each one, resolving a
contradiction stays explicit operator work, and memory is never rewritten or
deleted in place.

## Tamper-Evidence And Attestation

The history matters as much as the answer. At the base, Nahuali validates each
record's sequence and a per-event SHA-256 integrity checksum on open. That
catches accidental corruption, but a determined editor who rewrites a record and
recomputes its checksum would pass. The `tamper-evidence` feature closes that gap
with a hash chain, and the `attestation` feature adds Ed25519 tip signing on top.
Both are **on by default** across the whole workspace — a plain `cargo build` or
`cargo install` produces SHA-256 chaining plus a signable tip. They stay named
opt-OUTs: build with `--no-default-features` for a minimal, legacy-unchained
build (records then carry no chain link).

```bash
# The default builds chain records and are attestation-ready: validate detects
# an in-place rewrite out of the box.
cargo build -p nahuali-cli
cargo build -p nahuali-mcp
cargo build -p nahuali-api

# Drop the chain and signing surface for a minimal / legacy build.
cargo build -p nahuali-cli --no-default-features
```

With the chain enabled, every recorded event binds the previous event's chained
hash. Rewriting a historical record breaks the link at the next record, and
`validate` reports a broken chain even if the attacker forged a fresh per-record
checksum. Validation is **fail-closed by default**: a chain-stripped or partially
chained ledger is rejected. For a legacy ledger written before the chain existed,
use `validate --allow-unchained` — a loud, legacy-permissive escape hatch that
accepts unchained records. Legacy FNV-checksummed records stay valid on read (the
ledger is append-only); the report counts how many it accepted.

With `attestation`, sign the current tip and keep the receipt outside the store.
Nahuali never generates keys or touches the network. Supply a 32-byte Ed25519
seed you control (for example `openssl rand -hex 32 > ledger.key`):

```bash
nahuali --database .nahuali-demo attest-sign --key-file ledger.key -o tip.json
nahuali --database .nahuali-demo attest-verify tip.json
```

`attest-verify` exits non-zero when the receipt no longer vouches for the live
ledger, so it can gate a script or CI. A full re-chain of the history changes
the tip, so the signed receipt stops verifying and forging a new one needs the
private key.

A bare self-attesting receipt trusts whatever key it carries, so a leaked seed
would keep verifying forever. Pass a trusted-key ring to close that: a JSON file
listing the keys you authorize, each `active` or `revoked`. Rotation is adding a
new active key and re-attesting; revocation is flipping the old key to `revoked`.

```bash
nahuali --database .nahuali-demo attest-verify tip.json --keyring keyring.json
```

With `--keyring`, a receipt is honored only when it matches the live tip, its
signature verifies, and its signing key is active in the ring; a revoked or
unknown key is rejected even when the signature itself is valid, and the command
exits non-zero. The keyring is operator-held config kept outside the store, like
the receipts.

Attestation verifies the receipt you present. It cannot know whether that
receipt is the newest one an operator has ever issued. A rollback to an older
still-signed checkpoint is detectable only when automation verifies against the
latest receipt kept outside the store, or against another operator-controlled
freshness floor such as a known minimum sequence. Treat old valid receipts as
historical checkpoints, not as proof that the live store is current.

A second freshness gap sits at the head of the ledger: events appended after
your last signature are not yet under a signed tip. The hash chain still covers
them, so an in-place rewrite of any event, attested or not, breaks the link at
the next record and `validate` catches it with no receipt involved. What the
unsigned tail lacks is full-re-chain protection, which rests on a signed tip the
attacker cannot reproduce. Signing every append would not close this window: it
would put the signing key next to the store writer, the exact party a full
re-chain assumes control of, which is why the key stays off the write path. The
practical answer is to sign checkpoints often (signing is offline, key-free in
the core, and cheap) and to keep the unsigned tail auditable rather than
trusted. `audit --from-attestation` diffs exactly what was appended since the
last verified checkpoint, so the tail is reviewed against a signed anchor instead
of assumed current.

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
When scripts need a portable proof that one specific event is committed under
the audited Merkle root, ask `audit` for an inclusion proof:

```bash
nahuali --database .nahuali-demo audit --inclusion-proof 2 --json
```

The JSON response keeps the normal audit report and adds `inclusion_proof` with
the event sequence, leaf index, event id, leaf chain hash, Merkle root, sibling
path, and a local verification verdict.

Run the narrated walkthrough end to end:

```bash
cargo run -p nahuali-core --example tamper_evidence --features attestation
```

It shows a recomputed-checksum in-place rewrite being caught by the chain, and a
full suffix re-chain, which the chain alone cannot see, being caught by the
signed tip.

## Governed Self-Repair

Detecting what memory needs fixing is not the same as fixing it. Self-repair
closes that loop without giving up the trust model: an LLM proposes a repair as
JSON, and the deterministic engine validates, classifies, gates, and records it.
The binary never calls an LLM.

```bash
nahuali repair --proposal proposal.json --dry-run   # preview the verdict
nahuali repair --proposal proposal.json             # apply (or pipe JSON on stdin)
nahuali repair --proposal proposal.json --approve   # approve a queued repair
```

A proposal is evidence-anchored, so a fabricated citation is rejected rather than
minted into evidence-backed memory, and it is append-only, so a bad repair is
reversed by a later observation instead of a mutation. The engine assigns one of
three autonomy levels deterministically: a homogeneous, evidence-backed
consolidation or a link between two entities already present is applied
automatically; an ambiguous one is queued for operator approval; and a repair
that contradicts an existing claim is refused even with `--approve` and raised to
the operator. Each applied repair is a single audited event in the tamper-evident
ledger, and `self-inspect` surfaces how many repair candidates the engine already
detects without writing anything.

See the [Self-Repair Contract](SELF_REPAIR.md) for the six rules, the autonomy
gradient, and the proposal format.

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

Keep the semantic index current with `semantic-sync`: a non-destructive,
idempotent upsert that never drops the collection, safe to run after each batch
of writes so recall does not gap. `semantic-rebuild` is the destructive
counterpart, drop and recreate, needed when the embedder (and so the vector
space) changes.

Semantic vectors come from a deterministic local embedder by default. An
optional `local-embeddings` build feature swaps in a static model2vec model for
stronger semantic recall while staying fully local, offline, and deterministic;
no LLM is introduced into the core. Changing the embedder changes the vector
space, so rebuild the index with `semantic-rebuild` afterwards.

### Optional: stronger semantic recall with a local model

The default deterministic embedder hashes character n-grams, so phrasings that
share word shapes: morphology, common roots, substrings, typos, land near each
other (`release`/`releasing`, `product`/`products`). What it cannot do is bridge
true synonyms with no shared characters: `car` and `automobile` still look
unrelated to it. A static [model2vec](https://github.com/MinishLab/model2vec)
model places those synonyms close instead. Nahuali never downloads models. You
point it at a directory you control, so the core stays offline by construction.

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
commutes by automobile each morning", no shared words, ranked by meaning. The
deterministic embedder scores that episode the same as an unrelated one; the
model separates them (see `local_model_separates_meaning_where_deterministic_cannot`
in `nahuali-core`).

Detailed architecture, commercial planning, migration strategy, and internal
design documents remain private during pre-release development. The public
contract is the code, schema files, crate READMEs, fixtures, and validation
scripts in this repository.

## Resilience And Recovery

The CLI talks to a SurrealDB server (and an optional Qdrant). A natural question
for any operator: what happens when those services are down, and can data be
lost?

**Your data is not lost when a service is down.** The authoritative
`memory_record` ledger lives in SurrealDB's own durable volume. A stopped or
unreachable service means the CLI cannot *connect*, never that records were
deleted. When the service returns, the history is intact and re-verifies.

**While a service is unreachable, writes fail rather than queue.** A
`remember`/`claim`/… that cannot reach the store returns an error and records
nothing (there is no offline buffer yet); re-run it once the store is back. The
CLI makes this calm and actionable: it states that data is safe, best-effort
starts the local stack if its containers are merely stopped, and otherwise
prints the exact command to bring it up:

```text
✗ Cannot reach the Nahuali store at ws://localhost:18000.
  Your data is safe: nothing was lost or deleted; the database service
  is just unreachable (the append-only ledger lives in its own volume).
  Start the local stack and retry:
      docker compose up -d
```

**Qdrant is optional and derived.** Lexical recall, capture, briefing, audit,
and trust verdicts all work without Qdrant; only `recall --semantic` and the
`semantic-*` commands need it.

**Recovering after downtime is one command.** The ledger is ground truth and
needs no reconciliation; the derived tiers (the SurrealDB graph projection and
the Qdrant semantic index) can drift if a service was down while writes landed.
`nahuali reconcile` re-verifies the ledger and rebuilds both derived tiers from
it, reporting each, and skipping the semantic rebuild gracefully if Qdrant is
still unreachable:

```text
Reconcile · memory
  ledger    verified · chain intact · merkle 36628bbb4e…
  graph     rebuilt · 11 nodes · 0 relations
  semantic  synced · 7 points
```

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

Install local binaries when you explicitly want them on `PATH`. These default
source installs write hash-chained records; add `--no-default-features` only
when you intentionally need the legacy unchained format.

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

## Hands-On Tour (From Source)

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
The detailed command contract in [`crates/nahuali-cli/README.md`](crates/nahuali-cli/README.md#json-output-contract)
defines which JSON commands return direct payloads and which use metadata
envelopes.

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
  `projection_validate`, `semantic_status`, `semantic_rebuild`, and
  `semantic_sync`
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

cargo run -p nahuali-cli -- backup-validate .nahuali-demo.backup.json \
  --json

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
bash scripts/verify-governance-benchmarks.sh
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
- Release Please output is not final public copy. Before closeout, the GitHub
  release page must be edited into product-facing notes with highlights,
  install instructions, verification commands, component versions, beta limits,
  and a changelog pointer. Validate it with:
  `sh scripts/check-release-page.sh --tag nahuali-cli-vX.Y.Z-beta.N`.
- Repository workflow permissions stay read-only by default; Release Please
  requires the repository setting that allows GitHub Actions to create pull
  requests, while write scopes remain limited to the release job. Verify that
  setting with:
  `NAHUALI_VERIFY_GITHUB_SETTINGS=1 bash scripts/security-supply-chain-check.sh`.

The release train stays prerelease-only while the project is in beta.

See [DISTRIBUTION_READINESS.md](DISTRIBUTION_READINESS.md) for the
non-destructive readiness gates, supported beta channels, release verification
commands, and approval boundary for package registries or other external
distribution channels.

## Current Limits

- No hosted service.
- No accounts, teams, tenants, API keys, billing, sync, or dashboards.
- No unattended repair. Self-repair is opt-in and explicitly invoked
  (`nahuali repair`); it never resolves contradictions on its own, and there is
  no automatic consolidation pass inside a sleep cycle. See the
  [Self-Repair Contract](SELF_REPAIR.md).
- No guarantee that remembered information is true; Nahuali reports evidence,
  confidence, and health signals so callers can decide whether to trust it.
- Hash chaining and tip attestation are on by default across the whole
  workspace, so source installs and published binaries share the same ledger
  protection out of the box. The core library keeps both feature-gated as named
  opt-OUTs. Use `--no-default-features` only for an intentional legacy
  unchained build.
- No stable 1.0 API guarantee yet.

See [ROADMAP.md](ROADMAP.md) for the longer-term direction. Roadmap items are
not release guarantees until they appear in code, tests, and tagged releases.

## Repository Layout

```text
crates/nahuali-core        Rust memory engine
crates/nahuali-cli         CLI crate; installs the nahuali command
crates/nahuali-mcp         MCP stdio server
crates/nahuali-api         Local HTTP API
crates/nahuali-ui          Terminal presentation layer (palette, tables, cockpit)
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
