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

Nahuali provides some components that can contribute to an audit-trail design,
but it does not implement the SEC alternative by itself. It does not freeze bytes
on WORM media. Its hash chain can detect a rewritten non-tip event when the next
stored link is left unchanged, and its audit path reports append history between
retained bounds. Recreating an original after overwrite or deletion still
requires the firm to retain the original bytes or a suitable backup outside the
modified store. The full recordkeeping arrangement, including that preservation,
remains the firm's responsibility.

## Control Mapping

| Obligation | Nahuali implementation | Evidence | Gap or counsel note |
|---|---|---|---|
| SEC 17a-4 audit-trail alternative: recreate an original record if it is modified or deleted. | Every ordinary Nahuali memory write appends a typed `EventEnvelope`, and the audit path can diff retained ledger bounds. | `crates/nahuali-core/src/event.rs:25-52`; `crates/nahuali-core/src/store/services.rs:833-883`; `crates/nahuali-core/src/audit.rs:153-264` | This does **not** recreate bytes removed from or overwritten in the only store. Original records, suitable backups, classification of required records, and the complete 17a-4 arrangement must be provided externally. |
| Tamper-evidence for the retained record. | With `tamper-evidence` enabled, each event binds the previous event's chain hash into `prev_hash`; validation detects a non-tip in-place rewrite when the following link is not recomputed. | `crates/nahuali-core/src/event.rs:106-197`; `crates/nahuali-core/src/validation.rs:240-273` | A rewritten last event, truncation or rollback, and a fully recomputed suffix require comparison with an externally retained, authorized checkpoint. |
| Inclusion evidence for a specific record. | The `audit --inclusion-proof <SEQUENCE>` path emits a Merkle inclusion proof relative to the root in that audit result. | `crates/nahuali-core/src/merkle.rs:54-182`; `crates/nahuali-cli/src/cli.rs:865-869`; `crates/nahuali-cli/src/commands/audit.rs:66-120` | The proof establishes membership only under the supplied root. Third-party reliance requires an independently retained and authorized checkpoint for that root; it does not establish content truth or SEC preservation. |
| Independent verification anchor for the preserved series. | Version 2 Ed25519 checkpoints bind ledger lineage, tree size, Merkle root, and chain tip; verification requires a separately held operator policy. | `crates/nahuali-core/src/checkpoint.rs:125-220`; `crates/nahuali-cli/README.md:363-384` | This is an operator-controlled cryptographic anchor. It is **not** the designated-third-party (D3P) or designated executive officer undertaking that 17a-4 contemplates. Nahuali does not provide or act as a D3P. |
| FINRA 3110 supervision: reviewable, evidence-linked record of activity. | Health inspection surfaces unsupported, low-confidence, contradictory, superseded, and stale memory with evidence IDs; the trust report composes health, authority, and ledger integrity into one non-mutating verdict; recall can require a concrete evidence identifier. | `crates/nahuali-core/src/inspection.rs:181-338`; `crates/nahuali-core/src/recall.rs:9-27`; `crates/nahuali-core/src/trust_report.rs:119-222` | This is memory-governance signal for a reviewer, not a supervisory system. FINRA 3110 written supervisory procedures, designation of principals, and review workflows sit with the firm. |
| FINRA 4511 / 17a-4 retention (preserve for the required period; where unspecified, at least six years). | The core append path retains typed events in the configured ledger, validation can rescan checksum, sequence, and chain consistency, and recall applies inclusive point-in-time (`as_of_ms`) and lower-bound (`since_ms`) windows over retained items. | `crates/nahuali-core/src/event.rs:25-53`; `crates/nahuali-core/src/store/services.rs:833-883`; `crates/nahuali-core/src/validation.rs:142-280`; `crates/nahuali-core/src/recall.rs:394-404` | **Retention is not enforced.** No six-year retention timer, legal hold, or scheduled disposition exists. The operator controls the store lifetime. |
| Operator access to the preserved records and their integrity. | The CLI exposes `validate`, `audit`, `trust-report`, and checkpoint verification. The local API exposes memory health, audit, and trust reports, but it has no ledger-validation or checkpoint-verification endpoint. | `crates/nahuali-cli/src/cli.rs:829-889`; `crates/nahuali-cli/src/cli.rs:915-962`; `crates/nahuali-api/README.md:47-59` | Local, operator-run access. There is no regulator portal, non-rewriteable export bundle for examiners, or role-scoped access-control layer. |

## What Nahuali's ledger provides today

- **Append-only, hash-chained events** with per-event checksums and an optional
  SHA-256 chain link (`crates/nahuali-core/src/event.rs:25-52`,
  `crates/nahuali-core/src/event.rs:106-159`).
- **Merkle commitments and portable inclusion proofs** over the chained ledger,
  reachable from the CLI audit path
  (`crates/nahuali-core/src/merkle.rs:54-182`,
  `crates/nahuali-cli/src/cli.rs:865-869`,
  `crates/nahuali-cli/src/commands/audit.rs:66-120`).
- **Authorized Ed25519 checkpoints** under a separately held operator policy, as
  an external comparison point for rollback or a full re-chain
  (`crates/nahuali-core/src/checkpoint.rs:125-220`).
- **Non-mutating audit and trust reporting** that restate checksum, sequence,
  chain, and Merkle-root integrity and diff the ledger between two points
  (`crates/nahuali-core/src/audit.rs:129-184`,
  `crates/nahuali-core/src/audit.rs:195-310`).
- **Point-in-time recall** for filtering memory to records created at or before a
  supplied timestamp (`crates/nahuali-core/src/recall.rs:394-404`).

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
- **No promise that recorded content is true or permanently unaltered.** Local
  validation detects specified integrity failures. Rollback, last-event rewrite,
  or a full re-chain require an authorized external checkpoint, and none of
  these controls establish that the underlying assertion is correct.

## Current limits

For a firm evaluating Nahuali behind agent activity records, the engine offers
append-oriented capture, bounded integrity checks, Merkle membership evidence,
and operator-authorized checkpoints. Those are potential components of a wider
recordkeeping system, not proof of SEC or FINRA compliance. Nahuali does not
recreate an original whose only bytes were deleted or overwritten. Record
classification, original-record preservation, retention and legal holds, the
third-party or executive-officer undertaking, supervisory procedures under
FINRA 3110, and examiner access all remain the firm's responsibility and require
counsel. Quote any ledger claim with the commit, command, retained checkpoint,
and resulting report JSON.

Last reviewed: 2026-07-17.
