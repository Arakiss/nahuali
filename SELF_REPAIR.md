# Self-Repair Contract

Nahuali can detect what memory needs fixing — repeated observations worth
consolidating, entities that should be linked, contradictions, stale or
unsupported claims. Self-repair closes that loop without giving up governance:
an LLM **proposes** a repair, and the deterministic engine **validates,
classifies, records, and gates** it.

The design goal is narrow: let an LLM propose a repair while keeping validation
and write-back deterministic and reviewable. A proposal can be rejected before
it is written, queued for an operator, or superseded by a later ledger event.
Those controls reduce unattended ledger mutation; they do not prove that a
proposal is true or relevant, and they cannot reverse actions an external caller
already took from a bad memory.

## The guarantee

The binary never calls an LLM. A proposal enters as structured JSON produced by
an agent at the edge (`nahuali repair --proposal <file>` or stdin). Everything
the engine does with it — validation, classification, write-back — is
deterministic, offline, and append-only. The trust kernel stays LLM-free.

## The six rules

1. **The LLM proposes; the deterministic engine validates and records.** The
   trust kernel uses no LLM.
2. **Always reference-anchored.** Without an existing source episode there is no
   repair. A nonexistent citation is rejected rather than minted into
   evidence-backed memory. An existing episode ID proves only that the cited
   record exists; it does not prove source truth, relevance, or independence.
3. **Additive inside the ledger.** Repairs append events. A later observation
   can supersede a bad repair in projected memory, but does not delete history or
   undo any external action already taken from it.
4. **The repair is itself an audited event.** It is recorded in the
   tamper-evident ledger like any other event, with its proposal provenance and
   the verdict it was applied under.
5. **A `Block` verdict gates the repair.** The authority decision is used beyond
   recall: `Block` prevents an otherwise automatic unattended write.
6. **The core stays LLM-free, offline, and deterministic.** Self-repair is an
   opt-in layer at the edge, not a change to the kernel.

## The autonomy gradient

The engine — never the LLM — assigns one of three autonomy levels to each
proposal, deterministically, from the current projection:

| Repair | Policy | Why |
|---|---|---|
| Consolidate episodes that share one scope and a common tag into a cited claim | `Auto` | deterministic, reference-anchored, and supersedable inside the ledger |
| Link two entities that are both already present in memory | `Auto` | both endpoints and a cited episode exist; this is a structural check, not proof of the relationship |
| Consolidate without a homogeneous tag and scope (ambiguous pattern) | `Queue` | needs operator judgment → requires `--approve` |
| Assert a claim that contradicts an existing one (same subject + predicate, different value) | `NeverAuto` | a trust engine does not mask contradictions; it is raised to the operator |

Rule 5 layers on top: only a store authority verdict of `Block` degrades an
otherwise `Auto` repair to `Queue`. `Advisory` and `Warn` remain visible in the
report but do not change the classifier's autonomy level.

Write-back is gated on the level:

- **`Auto`** — applied with no approval.
- **`Queue`** — applied only with operator `--approve`, recorded with an
  `operator_override` flag; otherwise the verdict is reported and nothing is
  written.
- **`NeverAuto`** — refused even with `--approve`. The report surfaces the
  contradicting claim and points to the manual resolution path (an explicit
  operator review).

## The proposal format

A `RepairProposal` is JSON. It carries the materializable operation, the
evidence episodes that anchor it, the model that proposed it, and a rationale.

Consolidate a claim:

```json
{
  "payload": {
    "kind": "consolidate_claim",
    "subject": "Lena",
    "predicate": "owns",
    "object": "release notes",
    "confidence": 0.9
  },
  "evidence_episode_ids": ["episode_...", "episode_..."],
  "proposed_by": "claude-opus-4-8",
  "rationale": "Two release-tagged episodes both attribute the release notes to Lena."
}
```

Link two entities:

```json
{
  "payload": {
    "kind": "link_entities",
    "from": "Lena",
    "relation": "owns",
    "to": "Release Notes",
    "confidence": 0.9
  },
  "evidence_episode_ids": ["episode_..."],
  "proposed_by": "claude-opus-4-8",
  "rationale": "Both entities co-occur in the cited episode."
}
```

The engine rejects a structurally invalid proposal before anything is written:
a missing `proposed_by`, no evidence, an evidence episode that does not exist, an
empty field, or a link whose endpoints are not present in the projection. The
materialized claim or link references the first cited episode, so it meets the
same structural evidence requirement as a directly written memory. That check
does not establish that the episode actually supports the proposal.

## Using it

```bash
# Preview the verdict without writing.
nahuali repair --proposal proposal.json --dry-run

# Apply an Auto repair (or read the proposal from stdin).
nahuali repair --proposal proposal.json
cat proposal.json | nahuali repair

# Approve a queued repair that needs operator judgment.
nahuali repair --proposal proposal.json --approve

# Machine-readable report.
nahuali repair --proposal proposal.json --json
```

The applied repair is a single append-only `RepairApplied` event that
materializes the claim or link **and** records its audit atomically. It shows up
in `nahuali audit` and `nahuali trust-report` like any other event, and
`nahuali validate` stays green: strict chain validation is the default.
`--allow-unchained` is the explicit compatibility exception for legacy records.

### Step 0: the deterministic nudge

`nahuali self-inspect` surfaces a deterministic repair-need signal — how many
consolidation and link candidates the engine already detects — and points at
`nahuali repair`. It only informs. It never writes, and it never enables
automatic write-back.

## Scope and limits

This is self-repair step 1: the contract, the deterministic nudge, and the first
real repair with a review queue. Deliberately out of scope:

- **No automatic resolution of contradictions.** `NeverAuto` is never relaxed.
  A contradiction is always raised to the operator.
- **No repair runs on every write.** `nahuali repair` is a deliberate,
  explicitly invoked command. The classifier marks eligible consolidations
  `Auto`, but nothing applies them on your behalf.
- **Step 2 is not wired.** An automatic consolidation pass inside a `sleep` /
  `consolidate` cycle is specified for later and intentionally not built here.

Nahuali reports evidence, confidence, authority, and health, and now lets an LLM
propose repairs against that governed substrate. Callers — and the deterministic
gate — still decide how much trust to give each one.
