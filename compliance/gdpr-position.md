# GDPR Position

This document is an honest written position on how Nahuali relates to the EU
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
  (`crates/nahuali-core/src/event.rs:238-262`).
- **Sources** carry a `title`, a `uri`/locator, and adapter-provided `metadata`
  preserved as provenance — these can reference or embed personal data
  (`crates/nahuali-core/src/event.rs:195-218`).
- **Facts and relations** store `subject`/`predicate`/`object` and
  `from`/`relation`/`to` triples that can encode statements about identifiable
  people (`crates/nahuali-core/src/event.rs:264-283`,
  `crates/nahuali-core/src/event.rs:285-304`).

The security guidance already states Nahuali is not a secret manager and that
credentials, tokens, and customer secrets do not belong in memory databases
(`SECURITY.md:24-25`). The same discipline should extend to special-category
personal data.

## Current deployment pattern

- **Local-first.** Nahuali is a local-first Rust memory engine
  (`README.md:18`). The authoritative store is a local SurrealDB `memory_record`
  ledger; the semantic index is a derived, rebuildable tier
  (`SECURITY.md:17-27`).
- **Single-operator.** The beta API has no accounts, tenants, API keys, or
  role-based access; scopes are memory labels, not permission boundaries
  (`crates/nahuali-api/README.md:18-20`).
- **EU-hostable.** Because the whole stack runs against operator-controlled
  local services, a deployer can host it entirely within an EU boundary of their
  choosing. Data residency is a deployment decision, not a hosted-service default
  Nahuali imposes.
- **No telemetry.** The crates ship no analytics, telemetry, or phone-home code;
  the core attestation module states it never touches the network
  (`crates/nahuali-core/src/attestation.rs:19-20`). The only network endpoints
  are the operator's own configured SurrealDB and Qdrant services.

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
breaks the chain at the next event (`crates/nahuali-core/src/event.rs:13-39`,
`crates/nahuali-core/src/event.rs:76-89`). The property that makes the ledger
trustworthy for an auditor is the same property that makes selective deletion
hard: you cannot quietly excise one person's records and still present an intact
chain.

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

- **Scope discipline.** Record a subject's data under an explicit `MemoryScope`
  so it can be located and, at worst, isolated for wholesale deletion
  (`crates/nahuali-core/src/recall.rs:8-27`). Note that scopes are labels, not
  authorization boundaries (see `compliance/threat-model.md`).
- **Data minimization at write time.** Keep special-category personal data and
  irreplaceable personal data out of memory databases entirely, consistent with
  the existing "not a secret manager" posture (`SECURITY.md:24-25`) and the beta
  rule to use only data you can recreate (`BETA.md:38`).
- **Backup hygiene.** The `backup`, `backup-validate`, `backup-drill`, and
  `restore` commands let an operator take and verify local backups before changes
  (`crates/nahuali-cli/src/cli.rs:93-96`). Because erasure must reach backups
  too, keep backup retention short and inventoried for any store holding personal
  data.
- **Store-level deletion as the blunt instrument.** The only complete "erasure"
  available today is deleting an entire store (and its backups and derived
  indexes) and, where needed, restoring the remainder into a fresh database
  (`crates/nahuali-cli/src/cli.rs:919-929`). This erases the target subject but
  also everything co-resident in that store, so it is only practical when a store
  is scoped narrowly.

## Honest Position

Nahuali is honest about the tension: its tamper-evident, append-only ledger is
built to prevent silent deletion, which is exactly what a naive Article 17
erasure would require. Today there is no fine-grained erasure mechanism; there are
two candidate designs and a set of operational mitigations that are coarse. A
deployer with GDPR obligations over data that will enter Nahuali should keep
special-category and irreplaceable personal data out of the store, scope
aggressively, keep backups short and inventoried, and treat store-level deletion
as the current erasure path — and should involve counsel before relying on
Nahuali for any processing of personal data.

Last reviewed: 2026-07-10.
