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
| Provenance fields on supported record kinds | Source records carry source kind, locator, checksum, metadata, and scope. Episodes carry source id, source position, source role, and scope. Claims and relations can link back to a source episode. | `crates/nahuali-core/src/event.rs:225-334` | A reference proves that the cited record exists, not that it actually supports the claim or is true, relevant, independent, or authentic. |
| Reject or flag unsupported derived memory | Ingestion validates source references before writing. Repair and direct write paths reject fabricated evidence references. Recall trust warns or blocks unsupported or contradictory memory instead of silently certifying it. | `crates/nahuali-core/src/ingestion.rs:371-520`; `crates/nahuali-core/src/store/records.rs:418-447`; `crates/nahuali-core/src/store/records.rs:538-567`; `crates/nahuali-core/src/store/records.rs:875-894`; `crates/nahuali-core/src/self_repair.rs:358-390`; `crates/nahuali-core/src/recall.rs:407-555` | Unsupported memory can still exist if an operator intentionally records it. The system makes that visible rather than impossible. |
| Tenancy separation | `MemoryScope` labels memory by context boundary; recall filters by scope; semantic matches carry `scope_key`; health grouping includes scope so one project's facts do not conflict with another's. | `crates/nahuali-core/src/model.rs:66-79`; `crates/nahuali-core/src/recall.rs:13-18`; `crates/nahuali-core/src/semantic/types.rs:267-284`; `crates/nahuali-core/src/inspection.rs:447-479` | This is context separation, not hosted multi-tenant authorization. The beta API has no accounts, tenants, API keys, or RBAC (`crates/nahuali-api/README.md:18-20`). |
| Expiry or quarantine of unverified data | Recall can require evidence and can filter by query-time windows (`as_of_ms`, `since_ms`). Inspection flags unsupported, low-confidence, stale, superseded, and contradictory memory. | `crates/nahuali-core/src/recall.rs:17-26`; `crates/nahuali-core/src/inspection.rs:10-46`; `crates/nahuali-core/src/inspection.rs:183-328` | Automatic expiry, quarantine, and deletion of unverified memory are not implemented. Current behavior is visibility and query-time filtering. |
| Periodic evaluation against ground truth | Governance benchmark fixtures are fixed, reproducible release gates over ledger integrity, provenance coverage, contradiction and staleness, attestation recovery, and trust verdict soundness. | `GOVERNANCE_BENCHMARKS.md:35-51`; `GOVERNANCE_BENCHMARKS.md:63-76`; `GOVERNANCE_BENCHMARKS.md:216-230` | The benchmarks are first-party and synthetic, not independent certification. Quote results with commit, command, and report JSON. |
| Cryptographic logging and integrity-evident audit trail | Each event can bind the previous event hash; Merkle roots and inclusion proofs can be generated; audit reports restate checksum, sequence, chain, Merkle root, and verification status. | `crates/nahuali-core/src/event.rs:106-197`; `crates/nahuali-core/src/merkle.rs:54-142`; `crates/nahuali-core/src/audit.rs:129-184`; `crates/nahuali-core/src/audit.rs:267-310` | The next link detects a non-tip rewrite when the suffix is not recomputed. A last-event rewrite, truncation, rollback, or full re-chain requires an externally retained, authorized checkpoint. |
| Detached trust anchor for rollback and re-chain detection | Version 2 Ed25519 checkpoints bind ledger lineage, tree size, root, and chain tip; verification requires a separately held operator policy that excludes unknown and revoked keys. | `crates/nahuali-core/src/checkpoint.rs:125-220`; `crates/nahuali-core/src/checkpoint.rs:375-709` | Checkpoint freshness is external. A valid old checkpoint is historical unless automation verifies a retained freshness floor or the expected latest checkpoint. |
| Avoid automatic re-ingestion or autonomous mutation of poisoned context | Self-inspection, reflection, consolidation planning, and review plan or recommend work without writing automatically. Governed repair validates and gates explicit proposals before appending an event. | `README.md:221-229`; `SELF_REPAIR.md`; `crates/nahuali-mcp/src/tools.rs:744-850` | There is no complete malicious-content classifier. Nahuali controls memory writes and trust signals; it does not sanitize every upstream document. |

## ASI06 Position

Nahuali implements controls relevant to ASI06: persistent memory is inspectable,
provenance-aware, scope-filtered, replayable, and checked for specified integrity
failures. These controls include structural evidence links, recall-side trust
verdicts, health signals, hash-chain validation, Merkle membership evidence, and
operator-authorized Ed25519 checkpoints. They reduce and expose some poisoning
risk; they do not certify prevention.

The remaining gaps are important: no hosted tenant authorization, no automatic
expiry of unverified data, no encryption at rest, no complete content-safety
classifier, and no guarantee that stored claims are true. These gaps should be
kept attached to any buyer or auditor discussion.

Last reviewed: 2026-07-17.
