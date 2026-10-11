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

### Current state (shipped): `nahuali forget`

Since 0.8.0-beta.11 the CLI ships an explicit, operator-invoked erasure command
that follows the "versioned redaction" design:

```bash
nahuali forget episode_123 --reason "GDPR art. 17 request #42" --requested-by dpo --dry-run
nahuali forget episode_123 --reason "GDPR art. 17 request #42" --requested-by dpo --rebuild-semantic
```

- **What is erased.** Each target id and every memory derived from it: a source
  takes the episodes recorded from it; an episode takes the claims, links,
  procedures, intentions (and their updates), repairs, and review decisions
  that cite it. `--dry-run` lists the full cascade without writing anything.
- **How.** In one database transaction, every ledger record that held that
  content is replaced by a `redacted` stub, and a `memory_forgotten` record is
  appended. A stub keeps only the record's sequence, envelope id, checksum,
  previous-hash link, chained hash, the original event kind, and the id of the
  authorizing forget record. Payload text, tags, mentions, titles, locators,
  and metadata are gone from the ledger.
- **Auditable trace.** The `memory_forgotten` record holds who requested the
  erasure, when, the reason, the targets, the cascaded ids, and the sequence,
  kind, and chained hash of every redacted record. It is visible in
  `nahuali audit`, `nahuali data --json` (`forgotten`), and validation output.
  The reason is stored verbatim, so it must not repeat the erased content.
- **Integrity stays verifiable.** Because each stub keeps its chained hash, the
  hash chain, Merkle roots, inclusion and consistency proofs, and previously
  signed checkpoints remain valid. The tamper-evident history still shows that
  a record of a given kind existed at that position and was redacted, which is
  the intended trade-off.
- **Fails closed.** A stub that is not listed by a later `memory_forgotten`
  record, or a listed record that still holds content, makes reads, `validate`,
  and `backup-validate` fail. Redaction cannot be used to silently rewrite
  history.
- **Derived tiers.** Recall, inspection, review, and the SurrealDB graph
  projection stop surfacing the content immediately. The Qdrant semantic index
  is cleared of it only by `--rebuild-semantic` or `nahuali semantic-rebuild`.
- **Interface.** Erasure is CLI-only. It is deliberately not exposed through
  MCP or the HTTP API, so an agent cannot erase memory through its tool
  surface.

### What `forget` does not erase

`forget` is a building block for Article 17 handling, not proof that every copy
is gone. The operator remains responsible for:

- backups, snapshots, exports, and claim receipts written before the erasure;
- the Qdrant semantic index until it is rebuilt;
- physical bytes the storage engine (SurrealKV or a remote SurrealDB) keeps
  until it compacts or its own retention expires;
- terminal history, shell logs, and agent transcripts that contained the
  original command or output;
- re-importing an older export or restoring an older backup, which brings the
  content back.

Remaining candidate work: crypto-shredding via per-subject keys would also cover
copies the operator cannot reach, at the cost of key management and a precise
subject boundary. It is not built, and no delivery date is implied.

### Operational mitigations available today

Alongside `forget`, a deployer can reduce exposure with existing controls,
understanding each is a blunt instrument, not Article 17 conformance:

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
  (`nahuali backup --help`). Because erasure must reach backups
  too, keep backup retention short and inventoried for any store holding personal
  data.
- **Store-level disposal as a blunt deployment action.** An operator can dispose
  of an entire store together with its backups and derived indexes. Nahuali does
  not provide a command or verification report that proves all copies were
  erased, and restoring selected data into a fresh database can reintroduce
  personal data. Treat this as an operator procedure, not a shipped Article 17
  mechanism.

## Current limits

`nahuali forget` removes content from the authoritative ledger and the derived
SurrealDB tiers while keeping the history verifiable, but there is no complete
subject index and no report proving that every operator-controlled copy was
deleted. Finding all memory about a person still depends on searching recall,
scopes, and entities. A deployer with GDPR obligations should keep
special-category and irreplaceable personal data out of the store, minimize and
inventory what is recorded, apply explicit backup retention, rebuild the
semantic index after erasure, and involve counsel before relying on Nahuali for
personal-data processing.

Last reviewed: 2026-10-11.
