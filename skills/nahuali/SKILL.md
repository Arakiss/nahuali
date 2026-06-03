---
name: nahuali
description: >-
  Use the Nahuali MCP server as governed, evidence-first memory for the agent.
  Load this skill whenever the task needs persistent context across turns or
  sessions: recall prior context before answering (never assume), remember
  people/facts/decisions/events/tasks as they happen, assert claims or links
  only with evidence, or check whether the current memory is trustworthy before
  acting. Nahuali records an append-only ledger, returns recall with a
  per-result trust decision, and reports when knowledge is unsupported,
  conflicting, stale, or isolated. Trigger phrases: "remember this", "what do
  we know about", "recall", "check memory", "is this trustworthy", "what's
  missing", "was the history altered", or any task that depends on prior
  sessions.
---

# Nahuali — governed memory for Claude Code

Nahuali is not a "store more context" memory. It is a governance layer over
memory: every recall comes back with the evidence behind it and a trust
decision, and the engine can inspect its own health (unsupported claims,
contradictions, stale facts, blind spots) before you act on what it returns.
Its history is an append-only ledger you can audit.

The difference that matters: **recall-more vs. trust-what-you-recall.** An agent
that confidently recalls a wrong answer is worse than one with no memory.
Nahuali lets you tell the difference and say so.

The MCP server name in this client is `nahuali`. All tools below are its MCP
tools.

## When to load this skill

Load it whenever the task leans on memory: resuming work, anything that depends
on prior sessions, capturing a decision or fact, or deciding whether what you
recalled can be trusted. If you are about to assume context instead of querying
it, load this first.

## The core loop

1. **Start by recalling, not assuming.** Call `briefing` at session start for
   the read-only pre-work surface (authority, health, recent episodes, active
   intentions, high-priority review items, graph seeds — it changes nothing).
   Then `recall` for specifics and read the per-result trust.
2. **Record observations as they happen.** Use `remember` for an episode (a raw
   observation — this becomes the evidence other memory cites). Keep it factual
   and specific; add tags and mentions. Record the episode *before* asserting
   anything derived from it.
3. **Assert facts only with evidence.** Use `claim` (subject/predicate/object)
   or `link` (typed relation between entities) and cite the source episode
   (pass `sourceLast: true` to cite the episode you just recorded). A claim
   without evidence is weaker by design.
4. **Check health before trusting at scale.** `inspect` for the database-wide
   health snapshot; `trust_report` for one composed verdict on whether the store
   can be trusted; `self_inspect` to turn weak spots into proposed review work;
   `review` for the operator queue.
5. **Never silently rewrite memory.** Repairs are explicit. Surface conflicts
   and staleness; do not paper over them. The only write-back path for a review
   item is `review_resolve`, with an operator note.

## Reading recall trust (the important part)

`recall` returns, per result, a **trust mode**, not just a relevance score. The
score says how relevant; the mode says whether you may treat it as true. They
are different axes — a highly relevant result can still be `Advisory` or `Warn`.

- `Certify` (`can_trust=true`) — backed by source evidence, no result-local
  signal weakens it. State it as known; cite its evidence id.
- `Advisory` — observable but not a supported assertion on its own. State it as
  a lead ("memory suggests…"), not a settled fact.
- `Warn` — relevant but missing support, or weakened by a medium-risk signal.
  Do not act through it without confirming.
- `Block` — affected by a high-risk signal. Do not rely on it.

Also read the **store-level authority** returned alongside the results
(`Certify` / `Advisory` / `Warn` / `Block`, with stable `mode`, `score`,
`can_trust`, `signal_kinds` fields). A single good answer can still come from a
store that holds unsupported or conflicting material, and Nahuali says so. When
authority is `Warn` or `Block`, be cautious and prefer `inspect` /
`self_inspect` before acting.

## The key MCP tools

- **`briefing`** — call this first, every session. Read-only pre-work surface.
- **`remember`** — record an episode (the evidence). Do this before any claim.
- **`recall`** — trust-scored retrieval: results with per-result trust, evidence
  ids, store authority, and the health report in one call. Accepts `kinds` and
  `requireEvidence` for a narrow, evidence-backed surface; accepts a `scope`.
- **`claim`** — evidence-backed subject-predicate-object assertion. Use
  `sourceLast: true` to cite the episode you just recorded. (`fact` is a
  deprecated alias.)
- **`link`** — evidence-backed typed connection between two entities.
  (`relate` is a deprecated alias.)
- **`inspect`** — database-wide health snapshot (supported vs unsupported,
  contradictions, stale facts, blind spots). Read-only; proposes no fixes.
- **`trust_report`** — one composed, non-mutating verdict: knowledge counts,
  authority, restated ledger integrity, knowledge health, and an overall
  `trustworthy` flag with reasons. Answers "what do we know / why trust it /
  what's missing / was the history altered" in a single call.
- **`audit`** — non-mutating diff of what the ledger recorded between two points
  (`from`/`to` sequence bounds, optional `since`/`until` timestamps), with that
  history's integrity restated. Reports per-kind counts, per-event entries, and
  whether the history through the upper bound verifies.

Other tools you will reach for: `intention` (+ `intention_update`,
`intention_status`) for cross-session tasks; `self_inspect` / `reflect` /
`consolidation_plan` for governance; `review` / `review_resolve` for the
explicit write-back; `graph` for neighborhood traversal; `validate`,
`projection_*` and `semantic_*` for tier maintenance; `ingest` / `ingest_text`
to import sources with provenance.

## Semantic recall (by meaning)

Lexical recall matches words; semantic recall matches meaning. After adding or
changing memories, run `semantic_rebuild` once, then recall semantically to find
things related in meaning even with no shared keywords ("who handles the
finances" finds "maintains the budget spreadsheet"). `semantic_status` shows the
index state.

## Proving the history is intact

`audit` shows what changed and that nothing was silently rewritten. With the
optional `tamper-evidence` build feature, events are chained by hash so an
in-place rewrite of a historical record is detectable even if its checksum was
recomputed. Chain-tip attestation (signing a checkpoint) is a CLI/operator
action; the MCP server exposes no signing tool — from Claude Code you read and
audit integrity, you do not sign.

## Principles to keep

- Evidence is the boundary between an observation and a fact. Preserve it.
- Prefer recalling and citing over guessing.
- Do not present `Advisory` / unsupported results as established facts.
- Memory health is a first-class signal — check it, report it, do not hide it.
- The ledger is append-only. Repairs are explicit and leave a record.
