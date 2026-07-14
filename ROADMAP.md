# Nahuali Roadmap

This roadmap is deliberately narrow. It explains the current product direction
and the next beta hardening work without turning speculative ideas into a public
backlog. Tagged releases, tests, schemas, and crate documentation are the
authority for shipped behavior.

## North Star

Nahuali aims to become a self-inspecting memory substrate for long-running
agents and operator workflows.

The goal is not just to retrieve more context. The goal is to make memory answer
three questions before callers rely on it:

1. What do we know?
2. Why should we trust it?
3. What is missing, stale, unsupported, or contradictory?
4. Has the recorded history been altered?

The first three are answered by inspectable trust: evidence, health signals, and
authority context. The fourth is answered by a verifiable ledger: an opt-in hash
chain and a signed tip let a caller prove the recorded past was not rewritten.

The public source-available (FSL-1.1-MIT) engine should stay focused on that
foundation: local persistence, ledger replay, evidence-backed recall,
knowledge-health inspection, explicit review paths, and agent-friendly
interfaces over the same core.

## Product Principles

- **Ledger first.** The append-only `memory_record` ledger is the source of
  truth.
- **Derived tiers stay rebuildable.** Graph projection and the Qdrant semantic
  index must remain derived from the ledger.
- **Trust is inspectable.** Recall should expose evidence, health signals, and
  authority context instead of returning opaque text.
- **Inspection is non-mutating by default.** Reports can recommend review work,
  but memory writes should remain explicit.
- **Local-first engine.** The source-available (FSL-1.1-MIT) repository should
  be useful as a self-hosted engine before any higher-level product layer exists.
- **Small public contract.** Public claims should map to code, tests, fixtures,
  or release gates.

## Current Beta Foundation

The current public foundation includes:

- Rust engine crate: `nahuali-core`
- CLI: `nahuali`
- local MCP stdio server: `nahuali-mcp`
- local HTTP API: `nahuali-api`
- SurrealDB-backed record ledger
- tamper-evident hash-chained ledger (default-on across core, CLI, MCP, and API)
- Ed25519 chain-tip attestation (default-on build surface; signing remains an
  explicit operator action with an operator-held key)
- rebuildable SurrealDB graph projection
- rebuildable Qdrant semantic index
- optional local model2vec embedder for stronger semantic recall (off by default)
- evidence-backed lexical and hybrid recall
- knowledge-health inspection
- authority-aware recall
- scoped memory labels
- source ingestion and source-neutral interchange
- local backup, restore, and backup-drill flows
- non-mutating self-inspection, reflection, sleep, consolidation, review, and
  proactive reports
- governed self-repair: an LLM proposes a consolidation or link, the
  deterministic engine validates, classifies, gates, and records it as an
  audited, reversible event (`nahuali repair`); see the
  [Self-Repair Contract](SELF_REPAIR.md)
- non-mutating ledger audit/diff between two points, with integrity restated and
  optional anchoring on a signed checkpoint
- composed memory trust report that answers what we know, why to trust it, what
  is missing, and whether history was altered in one non-mutating verdict
- one-line installer, a zero-dependency `demo`, and a harness adoption skill and
  cross-harness protocol that `init` wires into the user's agent
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
- extend the opt-in tamper-evidence, attestation, and local-model surfaces from
  the CLI to the MCP and API binaries
- keep the agent-first CLI daily-driver loop demo passing from a clean checkout
- tighten validated examples around evidence-backed recall and self-inspection
- document the exact boundaries of the local API and MCP surfaces
- collect issues that come from real local usage and convert them into fixtures

Exit criteria:

- a new technical user can clone the repository, start local services, run the
  CLI, record memory, recall it with authority context, inspect health, and run
  the documented validation commands
- public claims in README and ROADMAP are implemented or removed
- release tags and release notes describe what actually shipped

## Explicit Non-Goals For This Repository

This repository is not trying to ship everything at once.

Not part of the current source-available (FSL-1.1-MIT) engine:

- hosted accounts or team administration
- payment or subscription management
- managed uptime promises
- secret storage
- browser dashboard as a beta requirement
- automatic resolution of contradictions, or any unattended repair that runs
  without an explicit `nahuali repair` invocation (self-repair step 2: an
  automatic consolidation pass inside a sleep/consolidate cycle is specified for
  later and intentionally not built)
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
- release notes that distinguish shipped behavior from non-goals

Until then, it should stay out of the public roadmap.
