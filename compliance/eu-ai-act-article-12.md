# EU AI Act Article 12 Mapping

This document maps Nahuali's shipped logging and trust controls to technical
obligations in EU AI Act Article 12 and related logging duties in Articles 19
and 26. It is an engineering alignment document, not legal advice or a
certification.

Primary legal text used for this mapping:

- Regulation (EU) 2024/1689, including Articles 12, 19, and 26:
  https://eur-lex.europa.eu/eli/reg/2024/1689/oj/eng

Timeline note: as of 2026-07-17, the Council's 2026-06-29 Digital Omnibus
announcement describes delayed high-risk rule application dates of 2027-12-02
for stand-alone high-risk AI systems and 2028-08-02 for high-risk AI systems
embedded in regulated products, while saying publication in the Official
Journal would follow shortly:
https://www.consilium.europa.eu/en/press/press-releases/2026/06/29/artificial-intelligence-council-gives-final-green-light-to-simplify-and-streamline-rules/.
Check the Official Journal text and counsel before relying on these dates.

## Applicability

Nahuali is a local memory substrate, not a complete high-risk AI system. The
mapping below describes controls a deployer or provider can use to support
traceability and logging. It does not decide whether a deployment is high-risk,
whether Nahuali is part of the regulated system boundary, or whether an
application satisfies the EU AI Act.

## Clause Mapping

| Obligation | Nahuali implementation | Evidence | Gap or counsel note |
|---|---|---|---|
| Article 12(1): high-risk AI systems must technically allow automatic recording of events over their lifetime. | Nahuali persists each memory write as a typed `EventEnvelope` in the append-oriented `memory_record` ledger. Opening a database validates sequence and checksum before projection. | `crates/nahuali-core/src/event.rs:25-52`; `crates/nahuali-core/src/store/services.rs:833-883`; `crates/nahuali-core/src/store/records.rs:49-68`; `crates/nahuali-core/src/store/ledger.rs:13-44` | Supports technical logging for Nahuali memory events. It is not a full-system event logger for every surrounding application action. |
| Article 12(2): logging must enable identification of situations that may result in risk or substantial modification. | Health inspection surfaces unsupported facts, low confidence, contradictions, staleness, supersessions, and isolated entities with evidence IDs. Trust reports combine health, authority, and ledger integrity in one non-mutating verdict. | `crates/nahuali-core/src/inspection.rs:10-53`; `crates/nahuali-core/src/inspection.rs:181-380`; `crates/nahuali-core/src/trust_report.rs:119-222` | This is memory-governance signal, not a complete AI Act risk-management system. Check with counsel before describing it as satisfying risk-management obligations. |
| Article 12(2): logging must support post-market monitoring. | Non-mutating audit reports restate ledger integrity, count event types, and can diff the ledger between points. Governance benchmarks provide reproducible release-gate checks over integrity, provenance, health, attestation, and trust verdict behavior. | `crates/nahuali-core/src/audit.rs:153-184`; `crates/nahuali-core/src/audit.rs:195-310`; `GOVERNANCE_BENCHMARKS.md:35-51` | Nahuali has no hosted post-market monitoring workflow, alerting service, or compliance dashboard. Deployers must wire reports into their own monitoring process. |
| Article 12(2): logging must support monitoring of operation under Article 26(5). | The CLI and MCP expose recall, inspection, ledger validation, audit, and trust reports. The API exposes recall, memory health, audit, and trust reports, but not a ledger-validation endpoint. These paths let an operator inspect memory health and available history checks without mutating memory. | `README.md:148-167`; `crates/nahuali-mcp/src/tools.rs:687-723`; `crates/nahuali-mcp/src/tools.rs:744-850`; `crates/nahuali-mcp/src/tools.rs:969-1028`; `crates/nahuali-api/README.md:47-59` | Local controls only. Article 26 monitoring duties remain with the deployer and surrounding system owner. |
| Article 12(3): for remote biometric identification systems listed in Annex III point 1(a), logs must include period of use, reference database checked, input data that led to a match, and natural persons verifying results. | Nahuali can record source metadata, source-scoped episodes, source roles, evidence links, and typed events. | `crates/nahuali-core/src/event.rs:199-334`; `crates/nahuali-core/src/ingestion.rs:41-59` | Nahuali does not ship a biometric-identification schema, verifier identity field, or reference-database log contract. A biometric deployment would need application-specific events. Check with counsel. |
| Article 19: providers of high-risk AI systems must keep automatically generated logs under their control for an appropriate period, at least six months unless another law says otherwise. | The append path retains typed events in the configured ledger, and validation can rescan checksum, sequence, and chain consistency. Optional snapshots remain maintenance artifacts and are never authoritative. | `crates/nahuali-core/src/event.rs:25-53`; `crates/nahuali-core/src/store/services.rs:833-883`; `crates/nahuali-core/src/validation.rs:142-280`; `crates/nahuali-core/src/maintenance.rs:13-17` | Retention is not enforced. No built-in six-month retention policy, legal hold, deletion workflow, or export retention scheduler exists yet. Roadmap item, check with counsel. |
| Article 26: deployers must keep logs under their control for an appropriate period, at least six months unless another law says otherwise. | Nahuali is local-first. The operator controls the configured SurrealDB ledger and the CLI validation, audit, backup, and trust-report commands. | `SECURITY.md:3-6`; `crates/nahuali-cli/src/cli.rs:829-889`; `crates/nahuali-cli/src/cli.rs:1022-1067` | Local control can help deployer access, but there is no role-based deployer portal, retention SLA, or legal evidence packaging. |
| Traceability of generated logs. | Events include sequence, timestamp, id, checksum, optional previous hash, and typed payload. Hash-chain validation and Merkle proofs support tamper-evidence and inclusion evidence. | `crates/nahuali-core/src/event.rs:25-52`; `crates/nahuali-core/src/event.rs:106-197`; `crates/nahuali-core/src/merkle.rs:54-142` | Chain validation proves ledger self-consistency, not truth of memory contents. |
| Access to logs and inspection by operators. | The CLI `validate`, `audit`, and `trust-report` paths expose machine-readable integrity and trust information. MCP exposes analogous tools; the API exposes `audit` and `trust-report` but no ledger-validation endpoint. | `crates/nahuali-cli/src/commands/preopen.rs:80-152`; `crates/nahuali-cli/src/commands/audit.rs:12-50`; `crates/nahuali-cli/src/commands/trust_report.rs:10-49`; `crates/nahuali-mcp/src/tools.rs:969-1028`; `crates/nahuali-api/README.md:47-59` | No built-in user management, access delegation, or immutable export bundle for regulators. |

## Current compliance position

Nahuali is aligned with the technical logging direction of Article 12: automatic
event recording, replayable history, validation, tamper-evidence, audit, and
operator-readable trust reports. It should not be described as AI Act compliant
without deployment-specific legal review, because classification, retention,
access, post-market monitoring, biometric-special logging, and organizational
process controls sit outside the shipped memory engine.
