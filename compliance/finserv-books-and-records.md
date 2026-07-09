# FINRA / SEC Books-and-Records Mapping

This document maps Nahuali's shipped ledger controls to the recordkeeping and
supervision duties a US broker-dealer works under: FINRA Rule 3110
(supervision), FINRA Rule 4511 (general books-and-records requirements), and SEC
Rule 17a-4 (electronic records preservation). It is an engineering alignment
document, not legal advice, a compliance certification, or a designated
recordkeeping arrangement.

Nahuali is a local-first memory engine for AI agents. Where an agent's activity
log is itself a business record — a decision trail, a supervised communication,
or an audit artifact a firm must retain — the controls below describe what the
Nahuali ledger provides and what it deliberately does not.

Primary sources used for this mapping:

- FINRA Rule 3110, Supervision:
  https://www.finra.org/rules-guidance/rulebooks/finra-rules/3110
- FINRA Rule 4511, General Requirements (books and records):
  https://www.finra.org/rules-guidance/rulebooks/finra-rules/4511
- SEC Rule 17a-4, Records to be preserved by certain exchange members, brokers
  and dealers (17 CFR 240.17a-4), current text:
  https://www.ecfr.gov/current/title-17/chapter-II/part-240/section-240.17a-4
- SEC final rule, Electronic Recordkeeping Requirements for Broker-Dealers,
  Security-Based Swap Dealers, and Major Security-Based Swap Participants,
  Release No. 34-96034, adopted 2022-10-12, effective 2023-01-03, compliance
  date 2023-05-03: https://www.sec.gov/files/rules/final/2022/34-96034.pdf
- FINRA, Exchange Act Rule 17a-4 Amendments — Chart of Significant Changes
  (2022-12): https://www.finra.org/sites/default/files/2022-12/rule-17a-4-amendments.pdf

## The 2022 amendment matters for AI-agent logs

Before 2022, SEC Rule 17a-4(f) required covered electronic records to be
preserved exclusively in a non-rewriteable, non-erasable format, commonly called
WORM (write once, read many). Release No. 34-96034 (adopted 2022-10-12) added an
**audit-trail alternative**: a firm may instead use an electronic recordkeeping
system that maintains and preserves records in a manner that permits the
recreation of an original record if it is altered, overwritten, or erased. The
WORM option was retained; the audit-trail alternative was added alongside it.

Nahuali's integrity model is built around exactly this alternative. It does not
freeze bytes on WORM media; it keeps an append-only, hash-chained event ledger
whose validation detects any in-place alteration and whose audit path recreates
what changed between two points. A firm choosing the audit-trail alternative for
agent-generated records still owns the surrounding obligations (see Known
Limitations); Nahuali supplies the technical audit trail, not the arrangement.

## Control Mapping

| Obligation | Nahuali implementation | Evidence | Gap or counsel note |
|---|---|---|---|
| SEC 17a-4 audit-trail alternative: recreate an original record if it is modified or deleted. | Every memory write is an append-only, typed `EventEnvelope` (version, id, sequence, timestamp, checksum, optional chain link, payload). The audit path emits a non-mutating diff of what changed between two ledger points, restating integrity through the upper bound. | `crates/nahuali-core/src/event.rs:13-39`; `crates/nahuali-core/src/audit.rs:122-160`; `crates/nahuali-core/src/audit.rs:101-120` | Nahuali records its own memory events. It is not a firm-wide recordkeeping platform and does not classify which agent records are 17a-4 "required records". |
| Tamper-evidence for the retained record. | With `tamper-evidence` enabled, each event binds the previous event's chained hash into `prev_hash`; validation detects an in-place rewrite at the next event even if the per-event checksum is recomputed. | `crates/nahuali-core/src/event.rs:76-89`; `crates/nahuali-core/src/event.rs:13-39` | Detects alteration; does not by itself detect a full re-chain of the suffix by an actor who controls the whole store (see attestation below). |
| Reconstruction / inclusion evidence for a specific record. | Merkle roots and portable inclusion proofs can be derived over the chained ledger; the `audit --inclusion-proof <SEQUENCE>` path emits an inclusion proof under the audited root. | `crates/nahuali-core/src/merkle.rs:54-142`; `crates/nahuali-cli/src/cli.rs:824-828`; `crates/nahuali-core/src/audit.rs:101-120` | A proof shows a record was committed in a given order under a root; it does not attest to the truthfulness of the record's contents. |
| Independent third party / verification anchor for the preserved series. | Detached Ed25519 tip attestation signs the live chain tip with operator-held key material and stores the receipt outside the ledger; verification fails when the live ledger no longer matches the receipt. Keyrings model active and revoked keys. | `crates/nahuali-core/src/attestation.rs:1-20`; `crates/nahuali-core/src/attestation.rs:29-58`; `crates/nahuali-cli/src/cli.rs:845-868` | This is a cryptographic anchor an operator holds. It is **not** the designated-third-party (D3P) or designated executive officer undertaking that 17a-4 contemplates. Nahuali does not provide or act as a D3P. |
| FINRA 3110 supervision: reviewable, evidence-linked record of activity. | Health inspection surfaces unsupported, low-confidence, contradictory, superseded, and stale memory with evidence IDs; the trust report composes health, authority, and ledger integrity into one non-mutating verdict; recall can require a concrete evidence identifier. | `crates/nahuali-core/src/inspection.rs:289-328`; `crates/nahuali-core/src/recall.rs:8-27`; `crates/nahuali-core/src/audit.rs:122-160` | This is memory-governance signal for a reviewer, not a supervisory system. FINRA 3110 written supervisory procedures, designation of principals, and review workflows sit with the firm. |
| FINRA 4511 / 17a-4 retention (preserve for the required period; where unspecified, at least six years). | The append-only ledger retains events and validates their integrity over time; recall supports point-in-time (`as_of_ms`) and lower-bound (`since_ms`) windows for audit replay. | `crates/nahuali-core/src/recall.rs:8-27`; `crates/nahuali-core/src/event.rs:13-39` | **Retention is not enforced.** No six-year retention timer, legal hold, or scheduled disposition exists. The operator controls the store lifetime. |
| Operator access to the preserved records and their integrity. | `validate`, `audit`, `trust-report`, and (under `attestation`) `attest-verify` expose machine-readable integrity and trust status over the same core; the same paths are exposed over the local API. | `crates/nahuali-cli/src/cli.rs:84-86`; `crates/nahuali-cli/src/cli.rs:855-868`; `crates/nahuali-api/README.md:40-41` | Local, operator-run access. There is no regulator portal, immutable export bundle for examiners, or role-scoped access-control layer. |

## What Nahuali's ledger provides today

- **Append-only, hash-chained events** with per-event checksums and an optional
  SHA-256 chain link (`crates/nahuali-core/src/event.rs:13-39`,
  `crates/nahuali-core/src/event.rs:76-89`).
- **Merkle commitments and portable inclusion proofs** over the chained ledger,
  reachable from the CLI audit path
  (`crates/nahuali-core/src/merkle.rs:54-142`,
  `crates/nahuali-cli/src/cli.rs:824-828`).
- **Detached Ed25519 tip attestation** with keyring rotation and revocation, as
  an operator-held anchor against a full re-chain
  (`crates/nahuali-core/src/attestation.rs:1-20`,
  `crates/nahuali-core/src/attestation.rs:29-58`).
- **Non-mutating audit and trust reporting** that restate checksum, sequence,
  chain, and Merkle-root integrity and diff the ledger between two points
  (`crates/nahuali-core/src/audit.rs:101-120`,
  `crates/nahuali-core/src/audit.rs:122-160`).
- **Point-in-time recall** for reconstructing what memory held as of a checkpoint
  (`crates/nahuali-core/src/recall.rs:8-27`).

## What Nahuali does NOT provide

- **No compliance certification.** Nahuali is not certified against 17a-4, and
  this document is not a legal opinion.
- **No designated third party (D3P) or designated executive officer
  arrangement.** The 2022 amendment lets a firm designate an executive officer in
  lieu of a third party to make the required undertakings; Nahuali supplies
  neither the undertaking nor a D3P service.
- **No WORM storage guarantee.** Nahuali implements the audit-trail direction, not
  non-rewriteable, non-erasable media. Firms relying on the WORM option must
  supply that at the storage layer.
- **No enforced retention or legal hold.** Retention, six-year preservation, and
  disposition are operator-managed; nothing in the engine enforces or schedules
  them (`crates/nahuali-core/src/recall.rs:8-27`).
- **No hosted access control for examiners.** There is no accounts/tenant model
  or role-based access; the beta API has no authentication
  (`crates/nahuali-api/README.md:18-20`).
- **No promise that recorded content is true.** The chain proves the record was
  not altered after the fact, not that the underlying assertion is correct
  (`crates/nahuali-core/src/audit.rs:101-120`).

## Honest Position

For a firm evaluating Nahuali as the technical audit trail behind AI-agent
activity records, the engine aligns with the direction of the 2022 SEC 17a-4
audit-trail alternative: append-only capture, tamper-evidence, reconstruction
via audit and inclusion proofs, and an operator-held cryptographic anchor. It
does not, on its own, make a deployment 17a-4 or FINRA compliant. Record
classification, retention periods and legal holds, the third-party or executive-
officer undertaking, supervisory procedures under FINRA 3110, and examiner access
all remain the firm's responsibility and require counsel. Quote any ledger claim
with the commit, the command run, and the resulting report JSON.

Last reviewed: 2026-07-10.
