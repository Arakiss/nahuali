# Agent-Memory and Governance Landscape

Last external claim review: 2026-07-17.

Next scheduled review: 2026-10-15, or earlier before this comparison is used
in a release, launch post, benchmark report, investor note, or customer-facing
document.

This review uses public primary sources: official repositories, official
documentation, and the authors' papers. Product and benchmark statements remain
the originating project's claims unless an independent reproduction is linked.
The review is representative rather than exhaustive.

## The Comparison Has Three Axes

Agent-memory products are often compared as though they answer one question.
They currently expose at least three separate evaluation axes:

1. **Answer quality:** can the system retain, retrieve, and apply the right
   context across sessions, updates, and long histories?
2. **Adoption and operation:** can a team integrate it through familiar SDKs
   and frameworks, then run it reliably without building the surrounding
   service itself?
3. **Governance and integrity:** can a caller inspect evidence, freshness,
   contradictions, authorization, and recorded-history integrity before acting
   on a memory?

Good performance on one axis does not imply good performance on the others.
Nahuali should therefore be described by the controls it implements and the
evidence it publishes, not by a category-wide ranking.

## Current Agent-Memory Systems

### Mem0

[Mem0](https://github.com/mem0ai/mem0) provides an Apache-2.0 memory library,
self-hosted server, managed platform, CLI, MCP integration, Python and
JavaScript interfaces, and integrations with common agent frameworks. Its
current open-source algorithm combines additive extraction, semantic and BM25
retrieval, entity linking, and temporal ranking. Mem0 publishes a
[research paper](https://arxiv.org/abs/2504.19413), evaluation code, and current
project-authored LoCoMo and LongMemEval results.

Mem0 lowers adoption cost through hosted and open-source deployment paths,
multiple SDKs, migration guides, framework examples, and recognizable
answer-quality evidence. The reviewed sources emphasize retrieval quality,
personalization, and managed operation; this review does not treat those sources
as evidence for Nahuali-style authorized checkpoints or portable claim receipts.

### Zep and Graphiti

[Graphiti](https://github.com/getzep/graphiti) is the Apache-2.0 temporal context
graph engine used by Zep. It models source episodes, entities, relationships,
and fact-validity intervals, and combines semantic, keyword, and graph
retrieval. [Zep](https://help.getzep.com/v2/quickstart) supplies the managed
operation around that model, including users, sessions, dashboards, and Python,
TypeScript, and Go SDKs. The team also publishes the
[Zep architecture paper](https://arxiv.org/abs/2501.13956) and benchmark claims.

Graphiti overlaps with Nahuali on episode provenance and temporal change instead
of treating memory as undifferentiated vector chunks. Zep's managed service and
SDK coverage address operational work that Nahuali currently leaves to the
operator. The reviewed sources do not establish a detached checkpoint policy or
offline claim-receipt contract.

### Letta

[Letta](https://github.com/letta-ai/letta), formerly MemGPT, is an Apache-2.0
platform for stateful agents. Its memory model combines always-visible,
agent-editable memory blocks with files, archival memory, and external retrieval
tools. The [context hierarchy](https://docs.letta.com/guides/core-concepts/memory/context-hierarchy)
makes those trade-offs explicit. Letta offers Python and TypeScript SDKs, a
managed service, a self-hosted server, a developer community, and a separate
[agent evaluation kit](https://github.com/letta-ai/letta-evals). Its underlying
memory-tier approach was introduced in the
[MemGPT paper](https://arxiv.org/abs/2310.08560).

Letta's adoption path is built around a complete stateful-agent runtime rather
than a standalone integrity layer. Its memory blocks also expose useful control
concepts such as read-only and shared blocks. The sources reviewed here do not
support treating that state model as an append-only, externally checkpointed
ledger.

### LangMem

[LangMem](https://github.com/langchain-ai/langmem) is an MIT-licensed toolkit for
forming and updating long-term memory. It supports semantic, episodic, and
procedural memory; memory formation in the request path or in background work;
storage-independent transformation functions; and native LangGraph storage and
platform integration. Its
[conceptual guide](https://langchain-ai.github.io/langmem/concepts/conceptual_guide/)
also states that useful memory systems are usually application-specific.

LangMem benefits from LangChain and LangGraph's existing integrations and user
base. Applications can adopt it without replacing their storage model. That
flexibility also means LangMem does not impose a tamper-evident ledger,
checkpoint-retention policy, or recall-authorization verdict on the underlying
store.

### Hindsight

[Hindsight](https://github.com/vectorize-io/hindsight) is an MIT-licensed memory
system organized around `retain`, `recall`, and `reflect`. It stores world facts,
agent experiences, and derived mental models in memory banks. It provides an LLM
wrapper, REST API, Python and Node clients, a CLI, self-hosted deployment, a
[managed cloud service](https://docs.hindsight.vectorize.io/), and public
[benchmark artifacts](https://github.com/vectorize-io/hindsight-benchmarks).

Hindsight documents answer-quality results alongside a short integration path
and managed operation. Its published benchmark numbers should still be treated
as project-authored unless a cited reproduction matches the dataset revision,
reader, judge, prompts, and run protocol. The reviewed sources do not establish
Nahuali's external checkpoint and receipt semantics.

### Supermemory

[Supermemory](https://github.com/supermemoryai/supermemory) is an MIT-licensed
memory and context service spanning conversation memory, user profiles, hybrid
search, document processing, data connectors, SDKs, MCP, and coding-agent
plugins. It publishes LoCoMo and LongMemEval claims and an open
[MemoryBench harness](https://github.com/supermemoryai/memorybench).

Supermemory packages memory, retrieval, user context, and external data
ingestion behind one managed interface. That is materially different from
Nahuali's local governance focus. Project-authored benchmark results remain
method-dependent, and the reviewed sources are not evidence of externally
authorized ledger checkpoints.

### MemMachine

[MemMachine](https://github.com/MemMachine/MemMachine) is an Apache-2.0 memory
layer with working, episodic, and profile memory; REST, Python, TypeScript, and
MCP interfaces; framework integrations; and self-hosted and managed deployment
paths. Its 2026
[paper](https://arxiv.org/abs/2604.04853) emphasizes preserving complete
conversation episodes to reduce information loss during extraction and reports
LoCoMo and LongMemEval results.

MemMachine is relevant because retaining full episodes is a practical answer to
provenance loss. Preserving source conversations, however, is not the same claim
as detecting a rewritten ledger or authorizing a retained checkpoint.

## Why These Systems Gain Adoption

Their public adoption paths share three patterns:

- **Answer-quality evidence is easy to understand.** LoCoMo and LongMemEval turn
  an abstract memory claim into a score tied to recognizable abilities. Open
  harnesses and saved outputs make the claim easier to inspect, even when the
  originating vendor ran the evaluation.
- **SDKs and integrations reduce switching cost.** Python and TypeScript SDKs,
  REST APIs, MCP servers, framework adapters, wrappers, migration guides, and
  runnable examples let a team test memory inside an existing application.
- **Managed operation and community reduce non-model work.** Hosted storage,
  authentication, tenant management, dashboards, monitoring, support, regular
  releases, and active contributor channels make the product usable beyond a
  local experiment.

Nahuali currently publishes more detail about evidence and recorded-history
limits than about those adoption concerns. This scope leaves product gaps:
governance controls do not replace answer-quality evidence, client libraries,
integrations, or managed operation.

## Security and Cryptographic Prior Art

The relevant building blocks already appear in memory and agent systems. They
should be treated as prior art, not as evidence that one project owns their
combination.

- [OWASP Agent Memory Guard](https://owasp.org/www-project-agent-memory-guard/)
  screens memory reads and writes for injection, secret leakage, protected-key
  changes, and other policy violations, and provides snapshots and rollback. It
  is complementary to a tamper-evident ledger: content screening and historical
  integrity answer different questions.
- [SuperLocalMemory](https://arxiv.org/abs/2603.02240) describes a local-first
  multi-agent memory system with per-agent provenance, trust scoring, adaptive
  ranking, and defenses aimed at memory poisoning.
- [MentisDB](https://docs.rs/mentisdb/latest/mentisdb/) is a Rust memory engine
  with typed thoughts, an append-only hash-chained log, hybrid retrieval,
  versioned skills, and signing support. It overlaps directly with durable and
  cryptographically verifiable memory.
- [OpenFang](https://github.com/RightNow-AI/openfang) applies a Merkle hash-chain
  audit trail to agent actions and signs agent manifests. Its primary object is
  execution history rather than a memory recall contract.
- [Right to History](https://arxiv.org/abs/2602.20214) applies Merkle audit logs
  and capability controls to verifiable agent execution. It addresses execution
  history rather than a general-purpose memory engine.

Memory poisoning is also an active research topic. For example,
[MPBench](https://arxiv.org/abs/2606.04329) studies write channels and structural
weaknesses that allow persistent poisoned content to influence later behavior.
Nahuali's provenance, health, and integrity checks can expose some risk signals;
they are not a complete input-screening or poisoning-defense product.

## Benchmark Landscape

### LoCoMo

[LoCoMo](https://github.com/snap-research/locomo), introduced in the
[ACL 2024 paper](https://arxiv.org/abs/2402.17753), releases ten long-running
conversations annotated for question answering and event summarization, with
multimodal dialogue-generation material. It tests whether a system can answer or
summarize from long conversational histories.

LoCoMo does not by itself test whether stored history was rewritten, whether a
retrieved statement has authorized provenance, or whether an operator retained
an external integrity checkpoint.

### LongMemEval

[LongMemEval](https://github.com/xiaowu0162/LongMemEval), accepted at ICLR 2025,
contains 500 questions covering information extraction, multi-session
reasoning, knowledge updates, temporal reasoning, and abstention. The official
repository publishes cleaned datasets and separate retrieval and answer
evaluation paths; the accompanying
[paper](https://arxiv.org/abs/2410.10813) defines the task.

LongMemEval is a relevant answer-quality target for Nahuali. A retrieval-only
adapter does not constitute a LongMemEval question-answering result: it must not
be reported as one without the reader, official answer evaluation, dataset
revision, prompts, model identifiers, and run outputs.

### LongMemEval-V2

[LongMemEval-V2](https://github.com/xiaowu0162/LongMemEval-V2) moves from chat
history to long histories of multimodal web-agent trajectories in customized
web and enterprise environments. Its 451 curated questions cover static state,
dynamic state, workflows, environment-specific failure modes, and premise
awareness. The largest released histories reach 115 million tokens, and the
evaluation combines answer accuracy with query latency. The official
[2026 paper](https://arxiv.org/abs/2605.12493) and repository define fixed
baseline and submission protocols.

V2 evaluates learned environment behavior rather than dialogue recall alone. It
still does not replace integrity tests: an accurate answer does not establish
that the retained history is untampered or that a checkpoint signer was
authorized.

### Rules for Quoting Memory Benchmarks

Do not compare headline numbers unless the report identifies:

- exact dataset and revision,
- ingestion and retrieval configuration,
- reader and judge models,
- prompts and context formatting,
- number of runs and aggregation method,
- latency boundary and hardware or service conditions, and
- raw or per-question outputs sufficient for review.

Vendor-authored harnesses and results are useful evidence when disclosed. They
are not independent certification. Retrieval metrics, answer accuracy, token
cost, and latency should remain separate rather than being collapsed into one
rank.

## Nahuali: Implemented Behavior and Current Gaps

The beta behavior reviewed here is documented in the [README](README.md), the
[trust model](TRUST_MODEL.md), and the project's
[release history](https://github.com/Arakiss/nahuali/releases). It implements:

- a local-first Rust engine with embedded SurrealKV by default and optional
  operator-controlled SurrealDB;
- CLI, TUI, stdio MCP, local HTTP, and Rust crate interfaces;
- evidence-linked episodes, claims, relationships, procedures, and intentions;
- deterministic recall verdicts based on evidence, freshness, contradictions,
  and store-health signals;
- an append-only hash-chained authoritative ledger, non-mutating audit, and
  Merkle inclusion and consistency proofs;
- Ed25519-signed ledger checkpoints verified against a separately supplied
  operator policy, including origin, ledger lineage, active or revoked keys, and
  signature threshold; and
- portable claim receipts that bind a selected claim, its evidence episode, an
  optional source event, their inclusion paths, and an authorized checkpoint.

The project also publishes synthetic
[governance regression suites](GOVERNANCE_BENCHMARKS.md). Those are
project-authored release gates, not a comparative answer-quality benchmark, an
external security audit, or a certification.

Current gaps and limits:

- The current beta line publishes no end-to-end LoCoMo, LongMemEval, or
  LongMemEval-V2 answer score. Development work on the
  [LongMemEval v1 adapter](benchmarks/longmemeval/README.md) is retrieval-only;
  it does not run a reader or produce the official question-answering score.
- It has no managed control plane, hosted synchronization, accounts, billing,
  tenant administration, or vendor-operated checkpoint-retention service.
- It does not yet offer the Python, TypeScript, and Go SDK coverage or breadth of
  framework and data-source integrations shown by several systems above.
- The local HTTP API is unauthenticated and is not intended for exposure to an
  untrusted network. Scope labels organize memory; they are not access-control
  boundaries.
- It does not independently witness checkpoints, publish them to a transparency
  service, or provide an external time guarantee.
- Evidence commitment does not prove factual truth, author identity, source
  authenticity, or the continued availability of source bytes.
- The product is in beta, and its API and storage behavior may change before
  1.0.

Nahuali is source-available under
[FSL-1.1-MIT](LICENSE), with a future MIT grant for each released version. That
license can add evaluation and adoption friction compared with Apache-2.0 or MIT
projects and should be described plainly. [Qdrant](https://github.com/qdrant/qdrant#license),
which Nahuali can use as an optional derived semantic index, is Apache-2.0. It is
not an FSL precedent and should not be presented as one.

## Internal History Checks vs External Checkpoints

This boundary must remain explicit in every public explanation.

### Checks performed against the store itself

Nahuali can recompute event checksums, sequence continuity, hash-chain links,
and the Merkle root for the ledger bytes it is currently reading. These checks
detect malformed or inconsistent current history. They cannot, on their own,
distinguish the original ledger from a fully rewritten and consistently
re-chained replacement.

### Checks against an externally retained authorized state

An operator can export a signed checkpoint and retain that checkpoint and its
authorization policy separately from the memory store. Later,
`checkpoint-verify` can compare the live ledger prefix with the retained tree
size, Merkle root, chain tip, lineage, origin, signer set, revocation state, and
signature threshold. This can detect rollback, truncation, or re-chaining
relative to the retained checkpoint.

The assurance depends on keeping the checkpoint, policy, and signing authority
outside the attacker's control. A valid checkpoint proves that authorized keys
committed to a ledger state; it does not prove an independently witnessed time.
If the attacker can replace the store and every retained trust artifact, local
verification has no independent reference. Witness co-signing or publication to
an external transparency service remains future work.

Portable claim-receipt verification has a narrower scope. It verifies selected
events, their provenance links and inclusion paths, and checkpoint
authorization. It does not replay the complete ledger prefix unless the verifier
also obtains the ledger and runs checkpoint verification.

## Responsible Public Positioning

A supportable summary is:

> Nahuali is a locally operated agent-memory engine with evidence-aware recall
> verdicts and tamper-evident recorded-history checks. Separately retained,
> operator-authorized checkpoints can detect specified rollback and re-chaining
> cases relative to a previously accepted ledger state.

Keep these qualifications attached:

- no claim of superior answer quality;
- no claim that the architecture or cryptographic primitives are exclusive;
- no claim of independent witnessing, factual truth, or authorship;
- no claim of legal compliance or independent certification; and
- no claim that governance controls replace content screening, access control,
  strong retrieval, integrations, or managed operation.

The comparison should be refreshed from the same primary-source standard before
each public use. If another system documents overlapping controls, record the
overlap directly and narrow the wording rather than defending a category claim.
