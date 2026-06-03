//! Composed, non-mutating memory trust report.
//!
//! The report answers the four questions the engine's north star asks of memory
//! before a caller relies on it, in one artifact: what do we know (knowledge
//! counts), why should we trust it (authority and ledger integrity), what is
//! missing or contradictory (knowledge health), and has the recorded history
//! been altered (integrity, plus an optional signed-checkpoint verdict). It
//! composes existing primitives; it does not introduce a new trust judgment.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[cfg(feature = "attestation")]
use crate::AttestedCheckpointVerdict;
#[cfg(feature = "attestation")]
use crate::LedgerAttestation;
use crate::{AuthorityDecision, KnowledgeHealth, LedgerAuditOptions, store::MemoryEngine};

/// Current memory trust report format version.
pub const MEMORY_TRUST_REPORT_VERSION: u32 = 1;

/// Options for building a memory trust report.
#[derive(Clone, Debug, Default)]
pub struct TrustReportOptions {
    /// A signed attestation receipt to verify against this ledger's history and
    /// fold into the report as the "has history been altered" evidence.
    #[cfg(feature = "attestation")]
    pub attestation: Option<LedgerAttestation>,
}

/// Knowledge counts: what the memory currently holds.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct TrustKnowledge {
    /// Total ledger events.
    pub event_count: usize,
    /// Registered provenance sources.
    pub source_count: usize,
    /// Projected entities.
    pub entity_count: usize,
    /// Ground-truth episodes.
    pub episode_count: usize,
    /// Evidence-backed claims.
    pub claim_count: usize,
    /// Typed links.
    pub link_count: usize,
    /// Procedures and preferences.
    pub procedure_count: usize,
    /// Intentions.
    pub intention_count: usize,
}

/// Restated ledger integrity: whether the recorded history is intact.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct TrustIntegrity {
    /// Whether the ledger passes every integrity check this build can run.
    pub ledger_verified: bool,
    /// Every event passes its per-event checksum.
    pub checksums_valid: bool,
    /// Sequences are contiguous and ordered from one.
    pub sequence_contiguous: bool,
    /// The tamper-evident hash chain is intact.
    #[cfg(feature = "tamper-evidence")]
    pub chain_intact: bool,
    /// The current chain tip, when the ledger is chained.
    #[cfg(feature = "tamper-evidence")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain_tip: Option<String>,
}

/// A composed, non-mutating snapshot of how trustworthy the memory is.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct MemoryTrustReport {
    /// Report format version.
    pub version: u32,
    /// Timestamp in milliseconds when the report was generated.
    pub generated_at_ms: u64,
    /// What the memory holds.
    pub knowledge: TrustKnowledge,
    /// Projection-level authority decision.
    pub authority: AuthorityDecision,
    /// Whether the recorded history is intact.
    pub integrity: TrustIntegrity,
    /// What is missing, stale, unsupported, or contradictory.
    pub health: KnowledgeHealth,
    /// Verdict for a supplied signed-checkpoint receipt, when given.
    #[cfg(feature = "attestation")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attestation: Option<AttestedCheckpointVerdict>,
    /// Composite verdict: the ledger is intact and authority certifies the
    /// memory (and any supplied checkpoint is anchored).
    pub trustworthy: bool,
    /// Human-readable reasons behind the verdict.
    pub verdict_reasons: Vec<String>,
}

impl MemoryEngine {
    /// Build a memory trust report with default options.
    pub fn trust_report(&self) -> MemoryTrustReport {
        self.trust_report_with_options(TrustReportOptions::default())
    }

    /// Build a memory trust report, optionally verifying a supplied attestation.
    #[cfg_attr(not(feature = "attestation"), allow(unused_variables))]
    pub fn trust_report_with_options(&self, options: TrustReportOptions) -> MemoryTrustReport {
        let data = self.data();
        let knowledge = TrustKnowledge {
            event_count: data.event_count,
            source_count: data.sources.len(),
            entity_count: data.entities.len(),
            episode_count: data.episodes.len(),
            claim_count: data.claims.len(),
            link_count: data.links.len(),
            procedure_count: data.procedures.len(),
            intention_count: data.intentions.len(),
        };

        let health = self.inspect();
        let authority = self.authority();
        let audit = self.audit_ledger(&LedgerAuditOptions::default());
        let integrity = TrustIntegrity {
            ledger_verified: audit.integrity.verified,
            checksums_valid: audit.integrity.checksums_valid,
            sequence_contiguous: audit.integrity.sequence_contiguous,
            #[cfg(feature = "tamper-evidence")]
            chain_intact: audit.integrity.chain_intact,
            #[cfg(feature = "tamper-evidence")]
            chain_tip: self.chain_tip(),
        };

        #[cfg(feature = "attestation")]
        let attestation = options
            .attestation
            .as_ref()
            .and_then(|receipt| self.verify_attested_checkpoint(receipt).ok());

        assemble_trust_report(
            knowledge,
            authority,
            health,
            integrity,
            #[cfg(feature = "attestation")]
            attestation,
            now_ms(),
        )
    }
}

/// Compose the verdict and reasons from already-computed pieces. Pure over its
/// inputs so the verdict logic can be exercised in tests without a live engine.
fn assemble_trust_report(
    knowledge: TrustKnowledge,
    authority: AuthorityDecision,
    health: KnowledgeHealth,
    integrity: TrustIntegrity,
    #[cfg(feature = "attestation")] attestation: Option<AttestedCheckpointVerdict>,
    generated_at_ms: u64,
) -> MemoryTrustReport {
    #[cfg(feature = "attestation")]
    let trustworthy = integrity.ledger_verified
        && authority.can_trust
        && attestation.as_ref().is_none_or(|verdict| verdict.anchored);
    #[cfg(not(feature = "attestation"))]
    let trustworthy = integrity.ledger_verified && authority.can_trust;

    let mut reasons = Vec::new();
    reasons.push(if integrity.ledger_verified {
        "ledger integrity verified".to_string()
    } else {
        "ledger integrity failed".to_string()
    });
    reasons.push(format!(
        "authority {} (score {:.2})",
        token(&authority.mode),
        authority.score
    ));
    if !authority.can_trust {
        reasons.push("authority does not certify the memory".to_string());
    }
    if health.unsupported_fact_count > 0 {
        reasons.push(format!(
            "{} unsupported fact(s)",
            health.unsupported_fact_count
        ));
    }
    if health.conflicting_fact_count > 0 {
        reasons.push(format!(
            "{} conflicting fact(s)",
            health.conflicting_fact_count
        ));
    }
    if health.blind_spot_count > 0 {
        reasons.push(format!("{} blind spot(s)", health.blind_spot_count));
    }

    #[cfg(feature = "attestation")]
    if let Some(verdict) = &attestation {
        reasons.push(if verdict.anchored {
            format!(
                "attested checkpoint at sequence {} is anchored",
                verdict.sequence
            )
        } else {
            "supplied attestation does not anchor a verified checkpoint".to_string()
        });
    }

    MemoryTrustReport {
        version: MEMORY_TRUST_REPORT_VERSION,
        generated_at_ms,
        knowledge,
        authority,
        integrity,
        health,
        #[cfg(feature = "attestation")]
        attestation,
        trustworthy,
        verdict_reasons: reasons,
    }
}

/// Render a serde enum as its serialized string token without coupling to its
/// variants (e.g. `AuthorityMode::Advisory` -> `"advisory"`).
fn token<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemoryData;

    fn empty_knowledge() -> TrustKnowledge {
        TrustKnowledge {
            event_count: 0,
            source_count: 0,
            entity_count: 0,
            episode_count: 0,
            claim_count: 0,
            link_count: 0,
            procedure_count: 0,
            intention_count: 0,
        }
    }

    fn intact_integrity() -> TrustIntegrity {
        TrustIntegrity {
            ledger_verified: true,
            checksums_valid: true,
            sequence_contiguous: true,
            #[cfg(feature = "tamper-evidence")]
            chain_intact: true,
            #[cfg(feature = "tamper-evidence")]
            chain_tip: None,
        }
    }

    #[test]
    fn verdict_tracks_integrity_and_authority() {
        let data = MemoryData::default();
        let health = KnowledgeHealth::inspect_at(&data, 1_000);
        let authority = AuthorityDecision::evaluate(&health);

        let report = assemble_trust_report(
            empty_knowledge(),
            authority.clone(),
            health,
            intact_integrity(),
            #[cfg(feature = "attestation")]
            None,
            1_000,
        );

        assert_eq!(report.version, MEMORY_TRUST_REPORT_VERSION);
        assert_eq!(report.generated_at_ms, 1_000);
        assert_eq!(report.trustworthy, authority.can_trust);
        assert!(report.integrity.ledger_verified);
        assert!(
            report
                .verdict_reasons
                .iter()
                .any(|reason| reason.contains("ledger integrity verified"))
        );
    }

    #[test]
    fn broken_ledger_is_never_trustworthy() {
        let data = MemoryData::default();
        let health = KnowledgeHealth::inspect_at(&data, 1_000);
        let authority = AuthorityDecision::evaluate(&health);
        let integrity = TrustIntegrity {
            ledger_verified: false,
            checksums_valid: false,
            sequence_contiguous: true,
            #[cfg(feature = "tamper-evidence")]
            chain_intact: true,
            #[cfg(feature = "tamper-evidence")]
            chain_tip: None,
        };

        let report = assemble_trust_report(
            empty_knowledge(),
            authority,
            health,
            integrity,
            #[cfg(feature = "attestation")]
            None,
            1_000,
        );

        assert!(!report.trustworthy);
        assert!(
            report
                .verdict_reasons
                .iter()
                .any(|reason| reason.contains("ledger integrity failed"))
        );
    }
}
