# OWASP ASI06 Memory And Context Poisoning Mapping

This document maps Nahuali's shipped controls to ASI06, Memory and Context
Poisoning, in the OWASP Top 10 for Agentic Applications:

- OWASP Top 10 for Agentic Applications, 2026:
  https://genai.owasp.org/resource/owasp-top-10-for-agentic-applications-for-2026/
- OWASP discussion of memory as an attack surface:
  https://genai.owasp.org/2026/05/13/memory-is-a-feature-it-is-also-an-attack-surface/

Concrete threat reference: CVE-2026-21852 and related agent
memory-poisoning style reports are treated here as the class of incident where a
trusted local agent context or memory file is poisoned and that poisoned context
persists across sessions. Verify exact CVE scope against NVD and vendor
advisories before using the CVE phrasing externally.

## Control Mapping

| ASI06 mitigation | Nahuali shipped behavior | Evidence | Gap or limit |
|---|---|---|---|
| Provenance metadata on every write | Source records carry source kind, locator, checksum, metadata, and scope. Episodes carry source id, source position, source role, and scope. Claims and relations can link back to the source episode that produced them. | `crates/nahuali-core/src/event.rs:195-218`; `crates/nahuali-core/src/event.rs:238-283`; `crates/nahuali-core/src/event.rs:285-304` | Provenance records that a memory has support. It does not prove the source itself is true. |
| Reject or flag unsupported derived memory | Ingestion validates source references before writing. Repair and direct write paths reject fabricated evidence references. Recall trust warns or blocks unsupported or contradictory memory instead of silently certifying it. | `crates/nahuali-core/src/ingestion.rs:371-520`; `crates/nahuali-core/src/store/records.rs:808-827`; `crates/nahuali-core/src/recall.rs:308-395` | Unsupported memory can still exist if an operator intentionally records it. The system makes that visible rather than impossible. |
| Tenancy separation | `MemoryScope` labels memory by context boundary; recall filters by scope; semantic matches carry `scope_key`; health grouping includes scope so one project's facts do not conflict with another's. | `crates/nahuali-core/src/model.rs:66-79`; `crates/nahuali-core/src/recall.rs:13-18`; `crates/nahuali-core/src/semantic/types.rs:267-284`; `crates/nahuali-core/src/inspection.rs:412-427` | This is context separation, not hosted multi-tenant authorization. The beta API has no accounts, tenants, API keys, or RBAC (`crates/nahuali-api/README.md:18-20`). |
| Expiry or quarantine of unverified data | Recall can require evidence and can filter by query-time windows (`as_of_ms`, `since_ms`). Inspection flags unsupported, low-confidence, stale, superseded, and contradictory memory. | `crates/nahuali-core/src/recall.rs:17-26`; `crates/nahuali-core/src/inspection.rs:10-46`; `crates/nahuali-core/src/inspection.rs:183-328` | Automatic expiry, quarantine, and deletion of unverified memory are not implemented. Current behavior is visibility and query-time filtering. |
| Periodic evaluation against ground truth | Governance benchmark fixtures are fixed, reproducible release gates over ledger integrity, provenance coverage, contradiction and staleness, attestation recovery, and trust verdict soundness. | `GOVERNANCE_BENCHMARKS.md:35-51`; `GOVERNANCE_BENCHMARKS.md:63-76`; `GOVERNANCE_BENCHMARKS.md:216-230` | The benchmarks are first-party and synthetic, not independent certification. Quote results with commit, command, and report JSON. |
| Cryptographic logging and immutable audit trail | Each event can bind the previous event hash; Merkle roots and inclusion proofs can be generated; audit reports restate checksum, sequence, chain, Merkle root, and verification status. | `crates/nahuali-core/src/event.rs:76-89`; `crates/nahuali-core/src/merkle.rs:54-142`; `crates/nahuali-core/src/audit.rs:101-120` | Full suffix re-chain protection depends on an external signed checkpoint. The hash chain alone cannot detect a complete re-chain by an attacker who controls the store. |
| Detached trust anchor for rollback and re-chain detection | Ed25519 tip attestation signs the live chain tip using operator-held key material; verification fails when the live ledger no longer matches the receipt. Keyring verification rejects revoked or unknown keys. | `crates/nahuali-core/src/attestation.rs:1-20`; `crates/nahuali-core/src/attestation.rs:95-144`; `crates/nahuali-core/src/attestation.rs:292-352` | Attestation freshness is external. A valid old receipt is a historical checkpoint unless automation verifies the latest receipt or a known freshness floor. |
| Avoid automatic re-ingestion or autonomous mutation of poisoned context | Self-inspection, reflection, sleep, consolidation planning, and proactive reports plan or recommend work without writing automatically. Governed repair validates and gates explicit proposals before appending an event. | `README.md:263-277`; `README.md:393-415`; `crates/nahuali-mcp/src/tools.rs:73-95` | There is no complete malicious-content classifier. Nahuali controls memory writes and trust signals; it does not sanitize every upstream document. |

## ASI06 Position

Nahuali addresses ASI06 by making persistent memory inspectable, provenance-aware,
scope-filtered, replayable, and tamper-evident. The current strongest controls
are deterministic evidence linking, recall-side trust verdicts, health signals,
hash-chain validation, Merkle inclusion evidence, and detached Ed25519
attestation.

The remaining gaps are important: no hosted tenant authorization, no automatic
expiry of unverified data, no encryption at rest, no complete content-safety
classifier, and no guarantee that stored claims are true. These gaps should be
kept attached to any buyer or auditor discussion.
