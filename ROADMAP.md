# Nahuali Roadmap

This roadmap is directional. It explains where Nahuali is intended to go, not
what the current beta already guarantees. Tagged releases, tests, schemas, and
crate documentation are the authority for shipped behavior.

## North Star

Nahuali aims to become a self-inspecting memory substrate for long-running
agents and operator workflows.

The goal is not just to retrieve more context. The goal is to make memory answer
three questions before callers rely on it:

1. What do we know?
2. Why should we trust it?
3. What is missing, stale, unsupported, or contradictory?

The public OSS engine should stay focused on that foundation: local persistence,
ledger replay, evidence-backed recall, knowledge-health inspection, explicit
review paths, and agent-friendly interfaces over the same core.

## Product Principles

- **Ledger first.** The append-only `memory_record` ledger is the source of
  truth.
- **Derived tiers stay rebuildable.** Graph projection and the Qdrant semantic
  index must remain derived from the ledger.
- **Trust is inspectable.** Recall should expose evidence, health signals, and
  authority context instead of returning opaque text.
- **Inspection is non-mutating by default.** Reports can recommend review work,
  but memory writes should remain explicit.
- **Local-first engine.** The OSS repository should be useful as a self-hosted
  engine before any higher-level product layer exists.
- **Small public contract.** Public claims should map to code, tests, fixtures,
  or release gates.

## Current Beta Foundation

The current public foundation includes:

- Rust engine crate: `nahuali-core`
- CLI: `nahuali`
- local MCP stdio server: `nahuali-mcp`
- local HTTP API: `nahuali-api`
- SurrealDB-backed record ledger
- rebuildable SurrealDB graph projection
- rebuildable Qdrant semantic index
- evidence-backed lexical and hybrid recall
- knowledge-health inspection
- authority-aware recall
- scoped memory labels
- source ingestion and source-neutral interchange
- local backup, restore, and backup-drill flows
- non-mutating self-inspection, reflection, sleep, consolidation, review, and
  proactive reports
- synthetic regression fixtures and release-gate scripts

## Near-Term: Public Beta Hardening

The next milestone is a credible public beta for technical users who are
comfortable running a local Rust project.

Focus areas:

- keep installation and source-run instructions accurate
- keep the public README and crate READMEs aligned with shipped behavior
- cut reproducible prerelease artifacts from tagged commits
- keep the release gate runnable from a clean checkout
- make errors and JSON output stable enough for scripts
- tighten examples around evidence-backed recall and self-inspection
- document the exact boundaries of the local API and MCP surfaces
- collect issues that come from real local usage and convert them into fixtures

Exit criteria:

- a new technical user can clone the repository, start local services, run the
  CLI, record memory, recall it with authority context, inspect health, and run
  the documented validation commands
- public claims in README and ROADMAP are either implemented, explicitly marked
  as future work, or removed
- release tags and release notes describe what actually shipped

## Mid-Term: Stronger Memory Operations

After the beta foundation is stable, the engine should become better at
operating memory over time.

Likely areas:

- richer review workflows for weak evidence, contradictions, and stale memory
- safer migration rehearsals from existing exports into source-neutral
  interchange
- clearer graph exploration commands and API shapes
- better semantic recall diagnostics
- stronger benchmark fixtures for recall grounding and knowledge health
- improved backup and restore ergonomics
- explicit versioning policy for schemas, events, and public JSON contracts
- more examples that demonstrate real operator loops without depending on
  private data

These items should land only when they can be validated with fixtures, tests, or
documented local workflows.

## Later: Integrations And Wider Adoption

Longer-term work may expand the engine into more environments while keeping the
same core contract.

Possible directions:

- published Rust crates after API stability improves
- more MCP host examples
- import adapters for common local knowledge formats
- stronger release verification and package provenance
- optional server or service layers that remain thin wrappers over the local
  core

Any higher-level product or managed-service work should remain outside this OSS
engine unless it has a clear open-source contract and does not weaken the local
ledger-first model.

## Explicit Non-Goals For This Repository

This repository is not trying to ship everything at once.

Not part of the current OSS engine:

- hosted accounts or team administration
- payment or subscription management
- managed uptime promises
- secret storage
- browser dashboard as a beta requirement
- automatic memory repair without explicit review
- a claim that memory contents are true
- stable 1.0 API guarantees before the contract has matured

Nahuali can report evidence, confidence, authority, and health. Callers still
decide how much trust to give memory.

## How Roadmap Items Graduate

A roadmap item should move into the public contract only when it has:

- a clear command, API, crate function, or file format
- tests or fixtures that prove the behavior
- documented limits
- validation in the release gate if it affects public trust
- release notes that distinguish shipped behavior from future direction

Until then, it remains direction, not a promise.
