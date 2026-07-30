---
name: nahuali
description: >-
  Nahuali is evidence-aware memory for agents, with deterministic recall
  verdicts and explicit recorded-history checks. Load this skill
  whenever a task depends on context from earlier turns or sessions: recall
  before you answer (never assume), record what happened as it happens, assert
  durable facts only with evidence, and read the trust verdict on what you
  recall before you act on it. Recall results carry a trust decision (CERTIFY /
  ADVISORY / WARN / BLOCK) and any recorded evidence behind them. The default
  append-only, hash-chained history supports local integrity checks; rollback
  and fully re-chained replacement require an authorized checkpoint retained
  outside the store. Trigger phrases: "remember this", "what do we know about", "recall",
  "check memory", "is this trustworthy", "what's missing", "was the history
  altered", or any task that leans on prior context. The command is `nahuali`
  (a single Rust binary); the same operations are exposed over MCP for hosts
  without shell access.
---

# Nahuali — governed memory for AI agents

## What Nahuali provides (read this first)

Nahuali records observations, derived claims, relationships, procedures, and
intentions together with their available evidence. It keeps relevance separate
from a deterministic trust verdict:

- Every recall result carries a **trust verdict** — `CERTIFY` / `ADVISORY` /
  `WARN` / `BLOCK` — computed from the evidence behind it, not from how relevant
  it looks.
- The default history is an **append-only, hash-chained ledger**. `nahuali audit`
  and `nahuali validate` detect checksum, sequence, and broken-link failures.
  The live store alone cannot detect every possible replacement; rollback and
  fully re-chained history require an authorized checkpoint retained elsewhere.
- The engine **inspects its own health** (unsupported claims, contradictions,
  stale facts, isolated entities) and tells you before you lean on it.

`nahuali` is a single Rust binary and the primary surface. The identical
operations are available over an MCP server (`nahuali-mcp`) for hosts without a
shell — see [Two ways to call it](#two-ways-to-call-it). Everything below is
verified against `nahuali <command> --help`; when in doubt, that help is the
authority, not this file.

## The one-minute mental model

1. **Recall before you assume.** Start with `briefing`, then `recall` specifics.
2. **Record what happened as an episode.** Episodes preserve observations and the
   evidence everything else cites. Record them freely — that is the point, not a
   cost.
3. **Graduate a durable fact into a claim, anchored to its episode.** A claim or
   link without a source episode is weaker by design and gets flagged.
4. **Read the trust verdict, not just the score.** Relevance and trust are
   different axes; a highly relevant result can still be `ADVISORY` or `WARN`.
5. **Check health before betting on a lot of memory** (`trust-report` /
   `inspect`), and **never rewrite history silently** — the ledger is appended
   to, never edited in place.

## The daily loop (copy-paste ready)

```bash
# 1 · Session start — non-mutating context surface. Run before meaningful work.
nahuali briefing

# 2 · Observe. Record episodes as things happen. Name the project, mention entities.
nahuali remember "Fixed the recall grouping bug in the CLI" --tag fix --mention nahuali
nahuali remember "Chose SurrealDB as the ledger backend" --scope project:nahuali
#    `-t/--tag` and `-m/--mention` are repeatable; `episode` is an accepted alias of `remember`.

# 3 · Graduate a durable, cross-session fact into an evidence-backed claim.
#     --source-last cites the episode you just recorded (this is what earns trust).
nahuali claim nahuali backend SurrealDB --source-last --confidence 0.9

# 4 · Relate entities when you are actually reasoning about the graph.
nahuali link nahuali depends_on SurrealDB --source-last

# 5 · Prospective work — things to do later, with a lifecycle.
nahuali intention "Ship shell completions" --priority high
nahuali pending                                  # list open intentions
nahuali intention-complete <intention-id> --reason "landed in 0.6.1"

# 6 · Recall — trust-tiered. Read the verdict on each result before acting.
nahuali recall "ledger backend" --authority       # rank by governance authority
nahuali recall "who owns billing" --semantic       # fuzzy match by meaning (needs the semantic index)
nahuali recall roadmap --kind intention --require-evidence --limit 25
```

Add `--json` to any command for clean machine output (colour and extras are
human-only and never pollute the JSON). Every command accepts `--database
<name>` to target a store other than the default.

## The four memory types — and *when* to write each

| Type | Record with | Write it when… |
|---|---|---|
| **Episodic** (default) | `remember` (alias `episode`) | Something happened — a decision, a fix, an event, an observation. This is the default; record episodes freely. They are the evidence everything else cites. |
| **Semantic** | `claim` · `link` (compat aliases: `fact` · `relate`) | A genuinely durable, cross-session fact is worth asserting (`claim <subject> <predicate> <object>`) or two entities relate (`link <from> <relation> <to>`). **Always anchor with `--source-last` or `--source-episode <id>`.** |
| **Procedural** | `procedure` · `preference` | A reusable how-to (`procedure`) or a stated rule/convention/default (`preference`) emerges and should be recalled later. Anchor these to an episode too. |
| **Prospective** | `intention` (+ `intention-complete` / `intention-block` / `intention-defer` / `intention-status` / `intention-update`) | There is future work: a task, goal, or reminder. `--kind task\|goal\|reminder`, `--priority low\|medium\|high\|critical`. |

The judgment that matters: **record episodes freely; assert claims and links
sparingly and only with evidence.** A store that is all episodes is healthy and
can still certify well-sourced results. A store full of unsourced claims is a
degrading, journal-like store that recall cannot trust. Do not force a knowledge
graph the session will not maintain.

## Reading recall trust (the part agents get wrong)

Each `recall` result prints a trust line. Branch on it:

- **`CERTIFY`** (`can_trust=true`, score 1.00) — backed by source evidence, no
  result-local weakening signal. State only what the recorded evidence supports,
  cite the `evidence:` id, and verify load-bearing facts against their current
  source.
- **`ADVISORY`** (score 0.75) — observed but not an evidence-backed assertion on
  its own (e.g. a bare entity). State it as a lead ("memory suggests…"), not a
  settled fact.
- **`WARN`** (score 0.50) — relevant but missing support, stale, or superseded.
  Do not act through it without confirming against the repo/git/tests.
- **`BLOCK`** (score 0.00) — a high-risk signal such as a contradiction. Do not
  rely on it.

Recall also prints a **store-level authority** header (`Store trust: …`). A good
single answer can still come from a store that holds unsupported or conflicting
material — the header says so. When it is `WARN` or `BLOCK`, slow down and run
`inspect` / `trust-report` before betting on memory.

The rule across the board: **recalled memory is evidence to verify, never
authority.** Confirm anything load-bearing against the current code, git, and
tests before acting.

## Governance & integrity

| Command | Answers |
|---|---|
| `nahuali trust-report` | One composed report: what is stored, the authority verdict, internal history checks, health counts, and whether the available checks support use. A supplied authorized checkpoint adds an external comparison. (`--html <path>` writes a dossier.) |
| `nahuali inspect` | Health snapshot: unsupported / conflicting / stale facts, isolated entities, blind spots. Read-only; proposes no fixes. |
| `nahuali self-inspect` | A prioritized review queue + consolidation candidates with evidence ids. Treat it as an **inspection packet to interpret against code/git/tests**, not an autonomous report. |
| `nahuali audit` | Recorded-history checks — `Integrity: verified (checksums ok, sequence contiguous, chain intact)` plus the Merkle root and chain tip. `✗ unverified` means at least one internal check failed; detecting rollback or a fully replaced and re-chained store requires a previously retained authorized checkpoint. Bound with `--from/--to` (sequence) or `--since/--until` (ms); `--inclusion-proof <seq>` emits membership evidence under the reported root. |
| `nahuali validate` | Validate the ledger without mutating it; strict chain validation is the default. `--allow-unchained` is the explicit compatibility exception for legacy records. |
| `nahuali reconcile` | Re-verify the ledger and rebuild the derived tiers (graph + semantic) from it. Run after a service was down. |
| `nahuali explore` (alias `tui`) | Interactive cockpit: `MEMORY` usability, `HISTORY` self-checks, optional `EXTERNAL` comparison, memory by kind, and review signals. |

**Repairs are explicit and conservative.** Apply only safe, evidence-supported
corrections surfaced by `self-inspect`; never invent facts, delete memory, or
rewrite history. The ledger is appended to, never edited in place.

## Data safety (non-negotiable)

- **An unreachable service proves only that the request failed.** The failed
  attempt does not write or delete a record, but the endpoint's durability
  cannot be inferred while it is unavailable. Restore connectivity, inspect
  the store, and verify backups before making a data-safety claim.
- **Writes fail rather than queue** while the service is unreachable — re-run
  when it is back up.
- **The semantic index is derived and optional** — lexical recall, capture,
  briefing, audit, and verdicts all work without it. Only `recall --semantic`
  and the `semantic-*` commands need it. Rebuild it with `nahuali
  semantic-rebuild` (or `semantic-sync` for a non-destructive upsert).
- **Never wipe or reinitialize a store without exporting first.** Use `nahuali
  backup --output <path>` (record-ledger backup) or `nahuali export --output
  <path>` (source-neutral interchange) before any destructive operation.
  `nahuali import <path>` and `nahuali restore` bring it back. Losing memory in a
  reinit is the one failure this product must not have.

## Anti-examples — the failure modes this skill exists to prevent

- **Inventing flags.** If you did not read it in `nahuali <command> --help`, it
  does not exist. There is no `--entities`, no `--tags` (it is `--tag`, singular
  and repeatable), no `--since` on `recall` (that flag is on `audit`; recall uses
  `--as-of-ms` / `--max-age-days`). Verify, then type.
- **Journaling into claims.** Do not assert `claim` after `claim` with no source
  episode. Unsourced claims degrade the store's trust to `WARN`. Record the
  episode first, then `claim … --source-last`.
- **Forcing a knowledge graph.** Do not manufacture `link`s the session will not
  maintain just to look structured. Link entities only when you are actually
  reasoning about how they relate.
- **Treating recall as truth.** A result is evidence with a verdict, not an
  authority. `ADVISORY` is a lead; `WARN`/`BLOCK` should not be acted through;
  even `CERTIFY` gets confirmed against the code when it is load-bearing.

## Two ways to call it

This skill documents the **CLI** (`nahuali`), the primary and most direct
surface. The identical capabilities are exposed by the MCP server `nahuali-mcp`
for hosts without shell access. The mapping is mechanical:

- Multi-word command names use underscores: `trust-report` → `trust_report`,
  `self-inspect` → `self_inspect`, `intention-status` → `intention_status`.
- Flags become named parameters in camelCase: `--source-last` → `sourceLast:
  true`, `--source-episode <id>` → `sourceEpisodeId`, `--require-evidence` →
  `requireEvidence`, `--kind` → `kinds`.

The MCP tool set is the same governed surface (`remember`, `recall`, `claim`,
`link`, `procedure`, `preference`, `intention*`, `inspect`, `trust_report`,
`audit`, `validate`, `reconcile`, `graph`, `semantic_*`, `ingest*`). Wire it up
and read the cross-harness protocol with `nahuali init`, which installs this
skill and prints the MCP config. The harness-agnostic version of these rules
lives in `skills/nahuali/protocol.md` in the repo.

## Ground truth

`nahuali --help` and `nahuali <command> --help` are always authoritative and
current. When this file and the binary disagree, the binary wins — file a fix
against this skill. Try the trust story with zero setup: `nahuali demo`.
