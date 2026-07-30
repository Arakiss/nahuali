# Controlled Beta

Nahuali is a public beta. This page defines the data-safety boundary for
technical testers while the storage and API contracts are still pre-1.0.

The beta goal is narrow: verify that the local CLI-first memory loop can run
from a clean install without risking existing data.

## What The Beta Covers

The controlled beta gate verifies that a checkout can:

- open the embedded persistent store without Docker or an external database
- record synthetic project memory into an isolated database
- resume a session from persisted ledger state
- recall scoped memory with evidence IDs, trust signals, and authority context
- inspect memory health and expose review work
- run self-inspection, review, reflection, sleep, consolidation, and proactive
  reports without implicit memory writes
- create, validate, drill, restore, and re-validate a local backup
- rebuild and query the derived semantic index
- race 16 graph-projection rebuilds against SurrealDB 3.0.5, require every
  successful result to match the ledger content manifest, detect same-count
  row tampering, and repair the projection back to a validated state
- create and verify a version 2 signed checkpoint under an external policy
- distinguish current checkpoints from verified historical prefixes
- export and verify a compact evidence-backed claim receipt without opening a
  database
- avoid changing any globally installed `nahuali` command
- pass the public security and supply-chain hygiene checks

Semantic-index checks still use the optional Qdrant development service. The
default lexical workflow and all ledger-integrity checks do not require it.

Run the gate:

```bash
bash scripts/verify-controlled-beta.sh
```

The script prints each validation step and exits non-zero on the first blocker.
A non-zero exit means the current checkout is not ready for controlled beta
testing.

## Tester Rules

Use Nahuali only with data you can recreate.

- Use a new `--database` value per test project or person.
- Start with synthetic or non-sensitive project notes.
- Do not import private exports until the dry-run and backup drill commands pass.
- Do not treat recall as truth. Treat it as memory plus inspectable evidence and
  warnings.
- Do not rely on automatic repair. Review, consolidation, and sleep reports are
  non-mutating unless a separate explicit write command is run.
- Keep local backups before experimenting with data you care about.
- Keep checkpoint keys, policies, and the latest accepted checkpoint outside the
  memory database. A valid old checkpoint is not proof that no newer checkpoint
  exists.

## First Commands

Run the full beta gate first:

```bash
bash scripts/verify-controlled-beta.sh
```

Then run the two human-readable demos:

```bash
bash scripts/demo-self-inspecting-memory.sh
bash scripts/demo-daily-driver-loop.sh
```

The demos use ignored `.local/` databases. Their output is synthetic and safe to
discard.

## Passing Criteria

A commit can be considered controlled-beta-ready only when all of these are
freshly true:

- `bash scripts/verify-controlled-beta.sh` passes.
- `bash scripts/security-supply-chain-check.sh` passes.
- `cargo test --workspace` passes or the current CI run for the commit is green.
- Public docs describe only shipped local behavior and explicit non-goals.
- No private notes, local databases, exports, backups, or workflow artifacts are
  tracked by Git.

## Blockers

Do not ask another person to test the beta when any of these are true:

- the dev stack cannot start with an actionable error
- the daily-driver workflow gate fails
- scoped recall returns unsupported memory when evidence is required
- self-inspection, sleep, reflection, or consolidation writes memory implicitly
- backup validation, backup drill, restore dry-run, or restore validation fails
- a supplied checkpoint is not authorized by the external policy, does not
  match the expected lineage, or fails the requested current/historical mode
- a claim receipt cannot verify every selected envelope, Merkle proof, and
  provenance link under its checkpoint
- graph projection validation reports an active rebuild, a version mismatch, a
  ledger-tip mismatch, or any count or content-manifest mismatch
- security and supply-chain checks fail
- CI is red for the commit being shared

## Explicit Non-Goals

The controlled beta does not include:

- hosted accounts, tenants, billing, or sync
- browser dashboard workflows
- stable 1.0 API guarantees
- automatic memory repair
- a promise that remembered content is true
- independent checkpoint witnesses, gossip, public anchoring, or blockchain
  consensus
- safe handling of secrets or irreplaceable personal data

Nahuali should expose why memory can or cannot be trusted. The operator still
decides whether to act on that memory.
