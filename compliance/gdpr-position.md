# GDPR Position

This document is the current written position on how Nahuali relates to the EU
General Data Protection Regulation (GDPR). It is an engineering alignment
document, not legal advice and not a claim of GDPR compliance. The hard part —
reconciling an append-only, tamper-evident ledger with the right to erasure — is
stated plainly rather than hidden.

Primary sources used for this position:

- GDPR (Regulation (EU) 2016/679), consolidated official text (EUR-Lex):
  https://eur-lex.europa.eu/eli/reg/2016/679/oj
- Article 17, Right to erasure ('right to be forgotten'):
  https://gdpr-info.eu/art-17-gdpr/
- Article 5(1)(e), Storage limitation:
  https://gdpr-info.eu/art-5-gdpr/

## What personal data can end up in memory records

Nahuali does not model "personal data" as a category, but personal data can
enter the ledger through ordinary writes, because the payloads carry free text
and identifiers:

- **Episodes** store natural-language `content`, free-text `tags`, and explicit
  entity `mentions` — any of which can name a person or contain personal data
  (`crates/nahuali-core/src/event.rs:268-292`).
- **Sources** carry a `title`, a `uri`/locator, and adapter-provided `metadata`
  preserved as provenance — these can reference or embed personal data
  (`crates/nahuali-core/src/event.rs:225-248`).
- **Facts and relations** store `subject`/`predicate`/`object` and
  `from`/`relation`/`to` triples that can encode statements about identifiable
  people (`crates/nahuali-core/src/event.rs:294-313`,
  `crates/nahuali-core/src/event.rs:315-334`).

The security guidance already states Nahuali is not a secret manager and that
credentials, tokens, and customer secrets do not belong in memory databases
(`SECURITY.md:29-30`). The same discipline should extend to special-category
personal data.

## Current deployment pattern

- **Local-first.** Nahuali is a local-first Rust memory engine
  (`README.md:25-27`). The authoritative store is a local SurrealDB `memory_record`
  ledger; the semantic index is a derived, rebuildable tier
  (`SECURITY.md:20-32`).
- **Single-operator.** The beta API has no accounts, tenants, API keys, or
  role-based access; scopes are memory labels, not permission boundaries
  (`crates/nahuali-api/README.md:18-20`).
- **EU-hostable.** Because the whole stack runs against operator-controlled
  local services, a deployer can host it entirely within an EU boundary of their
  choosing. Data residency is a deployment decision, not a hosted-service default
  Nahuali imposes.
- **Operator-controlled runtime data paths.** Nahuali does not provide a
  vendor-hosted memory service. The HTTP API binds to loopback by default;
  memory crosses a network boundary only when the operator selects remote
  SurrealDB or Qdrant endpoints (`crates/nahuali-core/src/database.rs:161-174`,
  `crates/nahuali-core/src/semantic/types.rs:16-77`,
  `crates/nahuali-api/src/main.rs:13-16`).

**Lawful basis, controller/processor roles, DPIAs, records of processing, and
data-subject request handling are the deployer's responsibility.** Nahuali is a
substrate the deployer runs; it does not determine purpose or means of processing
on the deployer's behalf, and this document does not assign those roles.

## The hard problem: append-only ledger vs Article 17 erasure

GDPR Article 17 gives a data subject the right, in defined circumstances, to
obtain erasure of personal data without undue delay
(https://gdpr-info.eu/art-17-gdpr/). Article 5(1)(e) requires that personal data
be kept in identifiable form no longer than necessary
(https://gdpr-info.eu/art-5-gdpr/).

Nahuali's integrity model is in direct tension with erasure by design. The ledger
is append-only, and each event under `tamper-evidence` binds the previous event's
chained hash into `prev_hash`, so removing or rewriting a historical record
breaks the chain at the next event (`crates/nahuali-core/src/event.rs:25-52`,
`crates/nahuali-core/src/event.rs:106-159`). The same linking that exposes some
historical rewrites also makes selective deletion difficult: removing one
person's record breaks the following link unless the suffix is recomputed. An
externally retained, authorized checkpoint is needed to distinguish that
recomputed history from the previously accepted ledger state.

### Current state (shipped)

**No erasure mechanism ships today.** There is no per-subject deletion, no
redaction event, no tombstone, and no retention timer in the engine. Recall
supports query-time windows (`as_of_ms`, `since_ms`) and inspection flags stale
memory, but neither removes data (`crates/nahuali-core/src/recall.rs:8-27`,
`crates/nahuali-core/src/inspection.rs:289-328`). Treat this as a known gap, not
a solved problem.

### Mechanism under design (candidate approaches, not yet built)

Two candidate designs are being evaluated. Both are described here so the gap is
transparent; neither is shipped, and no delivery date is implied.

- **Crypto-shredding via per-subject envelope keys.** Personal data for a subject
  is encrypted under a per-subject key; erasure becomes destroying that key,
  which renders the ciphertext unrecoverable while leaving the chain structurally
  intact. The trade-off is key-management complexity and defining the subject
  boundary precisely.
- **Versioned redaction events reconciled with the chain.** Erasure is recorded
  as a new, forward appended redaction event that supersedes the target, and the
  derived tiers (graph projection, semantic index) are rebuilt to drop the
  redacted content, while the original event's chain position is preserved as a
  tombstone. The trade-off is that the tamper-evident history still records that
  something existed and was redacted.

A chosen approach must also propagate erasure to every derived and backup copy —
the graph projection, the Qdrant semantic index, and any local backup artifacts —
or the erasure is incomplete.

### Operational mitigations available today

Until an erasure mechanism ships, a deployer can reduce exposure with existing
controls, understanding each is a blunt instrument, not Article 17 conformance:

- **Scope discipline.** An explicit `MemoryScope` can help queries locate records
  associated with a context (`crates/nahuali-core/src/recall.rs:8-27`). It is a
  label, not a subject index, authorization boundary, physical partition, or
  guarantee that every record about a person was classified consistently.
- **Data minimization at write time.** Keep special-category personal data and
  irreplaceable personal data out of memory databases entirely, consistent with
  the existing "not a secret manager" posture (`SECURITY.md:29-30`) and the beta
  rule to use only data you can recreate (`BETA.md:47-56`) and not to handle
  secrets or irreplaceable personal data (`BETA.md:109-120`).
- **Backup hygiene.** The `backup`, `backup-validate`, `backup-drill`, and
  `restore` commands let an operator take and verify local backups before changes
  (`crates/nahuali-cli/src/cli.rs:1022-1067`). Because erasure must reach backups
  too, keep backup retention short and inventoried for any store holding personal
  data.
- **Store-level disposal as a blunt deployment action.** An operator can dispose
  of an entire store together with its backups and derived indexes. Nahuali does
  not provide a command or verification report that proves all copies were
  erased, and restoring selected data into a fresh database can reintroduce
  personal data. Treat this as an operator procedure, not a shipped Article 17
  mechanism.

## Current limits

Nahuali's append-oriented ledger is in tension with selective erasure. Today
there is no fine-grained erasure mechanism, complete subject index, or proof that
all operator-controlled copies were deleted; the two candidate designs above are
not shipped. A deployer with GDPR obligations should keep special-category and
irreplaceable personal data out of the store, minimize and inventory what is
recorded, apply explicit backup retention, and involve counsel before relying on
Nahuali for personal-data processing. Store disposal is a coarse operator action,
not a verified erasure workflow.

Last reviewed: 2026-07-17.
