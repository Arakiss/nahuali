# AGENTS.md — Nahuali as your agent's memory protocol

This file tells any agent system (Claude Code, Codex, Cursor, an MCP-aware
harness, or your own loop) how and why to use Nahuali as its memory. It is
harness-agnostic: the rules below are the protocol, not a tool tour.

If your harness reads `AGENTS.md`, treat this as binding. If it reads a
different convention (a system prompt, a skill, a hook), copy the protocol
into that surface.

## What Nahuali provides

Nahuali records observations and derived memory with their available evidence.
Every recall carries a trust decision separate from relevance. The engine can
inspect unsupported claims, contradictions, stale facts, and blind spots before
you act. Its default append-only, hash-chained history supports local integrity
checks and can be compared with an authorized checkpoint retained outside the
store.

## The protocol (non-negotiable)

These six rules are the contract. Follow them in order, every session.

1. **Start by recalling, never by assuming.** Before answering anything that
   depends on prior context, call `briefing` for the session-wide picture, then
   `recall` for specifics. Do not reconstruct context from your own
   assumptions when memory can be queried.

2. **Record observations as episodes.** When the user states a decision, fact,
   event, or preference worth keeping, capture it with `remember` as an
   episode. Episodes are append-only observations and the evidence everything
   else cites. Keep them factual and specific. Record the episode *before* you
   assert anything derived from it.

3. **Assert claims and links only with evidence.** A claim (subject-predicate-
   object) or a link (typed relation between entities) must cite the episode
   that supports it. Record the episode first, then derive the claim or link
   from it. An unsupported assertion is weaker by design, and the health report
   will flag it.

4. **Read the per-result TRUST decision, not just the score.** Every `recall`
   result carries a trust mode, not only a relevance number. The decision
   evaluates provenance and memory health; it does not prove external truth:
   - `Certify` (`can_trust=true`) — backed by source evidence, no result-local
     signal weakens it. State only what the evidence supports, cite its evidence
     id, preserve any uncertainty in the source, and verify load-bearing facts
     against their current source.
   - `Advisory` — observable but not a supported assertion on its own. Treat as
     a lead, not a fact.
   - `Warn` — relevant but missing support, or weakened by a medium-risk signal.
   - `Block` — affected by a high-risk signal. Do not rely on it.
   Also read the **store-level authority** returned alongside results
   (`Certify` / `Advisory` / `Warn` / `Block`): a single good answer can still
   come from a store that holds unsupported or conflicting material, and
   Nahuali says so.

5. **Check health before trusting at scale.** Before you make a decision that
   leans on a lot of recalled memory, read the health signals: `inspect` for
   the database-wide snapshot, `trust_report` for one composed view of the
   checks represented in that report, and `self_inspect` to turn weak spots
   into proposed review work. When authority is `Warn` or `Block`, slow down
   and surface the gap instead of acting through it.

6. **Never silently rewrite memory.** Repairs are explicit and auditable.
   Surface conflicts and staleness; do not paper over them. The only write-back
   path for review items is `review_resolve`, with an operator note. The
   history is a ledger — appended to, never quietly edited.

## Reading recall correctly (the part agents get wrong)

The score tells you *how relevant* a result is. The trust mode tells you
*whether its evidence and health support using it*. They are different axes. A highly relevant
result can still be `Advisory` or `Warn`. Always branch on the trust mode:

- `Certify` → state what the evidence supports, cite the evidence id, and do not
  turn source confidence into a stronger claim of external truth.
- `Advisory` → state it as a lead ("memory suggests…"), not a settled fact.
- `Warn` / `Block` → do not act through it; say the support is missing and, if
  it matters, capture the evidence or open a review.

Same for the store-level authority decision: it is computed from the health
report, and `can_trust` is true only when the mode is `Certify`. When it is not,
prefer `inspect` / `self_inspect` before acting.

## Checking recorded history

`audit` returns a non-mutating diff of what the ledger recorded between two
points, together with checksum, sequence-contiguity, and default hash-chain
checks. A non-tip rewrite with a recomputed checksum is detected when the next
stored link no longer matches. A last-event rewrite, truncation, rollback, or a
fully re-chained replacement can remain internally consistent; detecting those
cases requires comparison with an authorized checkpoint retained outside the
store. Checkpoint signing is a CLI/operator action; the MCP server exposes no
signing tool.

## Adding the server (one-line MCP config)

Nahuali ships an MCP stdio server, `nahuali-mcp`. Register it with your client.
The server name agents reference is `nahuali`.

```json
{
  "mcpServers": {
    "nahuali": {
      "command": "nahuali-mcp",
      "args": ["--database", "my_project"]
    }
  }
}
```

Install the binary from source with
`cargo install --path crates/nahuali-mcp --locked`. The database value is a
SurrealDB identifier, not a path; choose a stable name for each memory context.

At the start of every session, call `briefing` first. It returns the read-only
pre-work surface (authority, health, recent episodes, active intentions,
high-priority review items, graph seeds) and changes nothing.

## Tool map (the ones the protocol uses)

Do not invent tools. These are the real ones.

- **Write (record):** `remember` (episode — the evidence), `claim`
  (subject-predicate-object, evidence-backed), `link` (typed relation, evidence-
  backed), `procedure`, `preference`, `intention` (+ `intention_update`,
  `intention_status`), `ingest` / `ingest_text` (import sources with provenance).
- **Read (recall):** `briefing` (session start), `recall` (trust-scored
  retrieval), `graph` (neighborhood traversal), `memory_hook` (governed context
  for a host execution point).
- **Govern (trust):** `inspect` (health snapshot), `trust_report` (one composed
  verdict), `self_inspect` (findings + proposed review), `reflect`,
  `consolidation_plan`, `review` / `review_resolve` (the explicit write-back
  path), `proactive`, `deadlines`, `anomalies` / `anomaly_acknowledge`,
  `reconcile_intentions`, `goal_progress`.
- **Check (integrity):** `audit` (recorded-history diff and checks), `validate`
  (ledger intact + migration needs).
- **Maintain (derived tiers):** `projection_status` / `projection_rebuild` /
  `projection_validate`, `semantic_status` / `semantic_rebuild`.

`fact` and `relate` are compatibility aliases for `claim` and `link` — prefer
the canonical tools. `preference` is **not** an alias: it is its own memory type
(a stated rule or convention), distinct from `procedure` (a repeatable how-to).

## Semantic recall (by meaning, not keywords)

Lexical recall matches words; semantic recall matches meaning. After adding or
changing memories, run `semantic_rebuild` once, then recall to find things
related in meaning even with no shared keywords ("who handles the finances"
finds "maintains the budget spreadsheet"). `semantic_status` shows index state.

## Principles to keep

- Evidence is the boundary between an observation and a fact. Preserve it.
- Prefer recalling and citing over guessing.
- Do not present `Advisory` / unsupported results as established facts.
- Memory health is a first-class signal. Check it, report it, do not hide it.
- The ledger is append-only. Repairs are explicit and leave a record.
