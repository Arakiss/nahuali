//! Composed, non-mutating memory trust report.
//!
//! The report answers the four questions the engine's north star asks of memory
//! before a caller relies on it, in one artifact: what do we know (knowledge
//! counts), why should we trust it (authority and internal history checks), what
//! is missing or contradictory (knowledge health), and was the current state
//! compared with an authorized external checkpoint. It composes existing
//! primitives; it does not introduce a new trust judgment.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[cfg(feature = "tamper-evidence")]
use crate::LedgerChainStatus;
#[cfg(feature = "attestation")]
use crate::{AttestationKeyring, LedgerAttestation};
use crate::{AuthorityDecision, KnowledgeHealth, LedgerAuditOptions, store::MemoryEngine};

/// Current memory trust report format version.
pub const MEMORY_TRUST_REPORT_VERSION: u32 = 2;

/// Stable identifier for the legacy, self-signed chain-tip receipt.
#[cfg(feature = "attestation")]
pub const TRUST_ATTESTATION_FORMAT_V1: &str = "self_signed_tip_v1";

/// Options for building a memory trust report.
#[derive(Clone, Debug, Default)]
pub struct TrustReportOptions {
    /// A signed attestation receipt to verify against this ledger's history and
    /// fold into the report as an external comparison for the current history.
    #[cfg(feature = "attestation")]
    pub attestation: Option<LedgerAttestation>,
    /// Operator-held keys that are allowed to establish external signer trust.
    /// Without a keyring, a valid v1 receipt remains self-signed evidence and
    /// cannot satisfy a supplied-attestation trust gate.
    #[cfg(feature = "attestation")]
    pub keyring: Option<AttestationKeyring>,
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

/// Restated internal checks over the history available to this store.
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
    /// Whether the chain is fully verified, legacy-compatible, or broken.
    #[cfg(feature = "tamper-evidence")]
    pub chain_status: LedgerChainStatus,
    /// The current chain tip, when the ledger is chained.
    #[cfg(feature = "tamper-evidence")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain_tip: Option<String>,
    /// Merkle commitment over the full chained ledger: one root for anchoring
    /// and inclusion proofs, distinct from the linear `chain_tip`. A commitment,
    /// not a proof, and not a trust gate. `None` when the ledger is unchained.
    #[cfg(feature = "tamper-evidence")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merkle_root: Option<String>,
}

/// Fail-closed evaluation of a supplied legacy attestation.
///
/// A v1 receipt proves that the private key corresponding to its embedded public
/// key signed a chain tip. It becomes a trusted external anchor only when that key is
/// active in an operator-held keyring.
#[cfg(feature = "attestation")]
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct TrustAttestationEvaluation {
    /// Receipt format. V1 remains readable for compatibility.
    pub format: String,
    /// Sequence claimed by the receipt.
    pub sequence: u64,
    /// Whether the detached signature is valid under the embedded public key.
    pub signature_valid: bool,
    /// Whether the signed tip matches this ledger's history at `sequence`.
    pub matches_history: bool,
    /// Whether the signing key is active in the supplied operator keyring.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signer_authorized: Option<bool>,
    /// Whether the signing key is explicitly revoked in the supplied keyring.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signer_revoked: Option<bool>,
    /// Whether signature, history, and external signer authorization all pass.
    pub trusted: bool,
    /// Cryptographic or format rejection that was supplied and must not be
    /// confused with the absence of a receipt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection: Option<String>,
}

/// A composed, non-mutating snapshot of the checks available for this memory.
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
    /// Internal checks over the recorded history available to this store.
    pub integrity: TrustIntegrity,
    /// What is missing, stale, unsupported, or contradictory.
    pub health: KnowledgeHealth,
    /// Verdict for a supplied signed-checkpoint receipt, when given.
    #[cfg(feature = "attestation")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attestation: Option<TrustAttestationEvaluation>,
    /// Composite verdict: internal checks pass and authority certifies the
    /// memory (and any supplied checkpoint is authorized and matches).
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
            chain_status: audit.integrity.chain_status,
            #[cfg(feature = "tamper-evidence")]
            chain_tip: (audit.integrity.chain_status == LedgerChainStatus::Verified)
                .then(|| self.chain_tip())
                .flatten(),
            // The default audit covers the whole ledger, so its Merkle root is
            // the full-ledger commitment; reuse it rather than recomputing.
            #[cfg(feature = "tamper-evidence")]
            merkle_root: audit.integrity.merkle_root.clone(),
        };

        #[cfg(feature = "attestation")]
        let attestation = options
            .attestation
            .as_ref()
            .map(|receipt| evaluate_attestation(self, receipt, options.keyring.as_ref()));

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
    #[cfg(feature = "attestation")] attestation: Option<TrustAttestationEvaluation>,
    generated_at_ms: u64,
) -> MemoryTrustReport {
    #[cfg(feature = "attestation")]
    let trustworthy = integrity.ledger_verified
        && authority.can_trust
        && attestation
            .as_ref()
            .is_none_or(|evaluation| evaluation.trusted);
    #[cfg(not(feature = "attestation"))]
    let trustworthy = integrity.ledger_verified && authority.can_trust;

    let mut reasons = Vec::new();
    reasons.push(if integrity.ledger_verified {
        "recorded-history checks passed".to_string()
    } else {
        "recorded-history checks failed".to_string()
    });
    #[cfg(feature = "tamper-evidence")]
    match integrity.chain_status {
        LedgerChainStatus::Empty => {
            reasons.push("ledger is empty; no recorded history exists to verify".to_string());
        }
        LedgerChainStatus::Verified => {}
        LedgerChainStatus::Legacy => reasons.push(
            "ledger contains legacy unchained history; cryptographic integrity is not verified"
                .to_string(),
        ),
        LedgerChainStatus::Broken => {
            reasons.push("ledger hash chain is broken".to_string());
        }
    }
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
    if let Some(evaluation) = &attestation {
        reasons.push(if evaluation.trusted {
            format!(
                "attested checkpoint at sequence {} is trusted under the operator keyring",
                evaluation.sequence
            )
        } else if let Some(rejection) = &evaluation.rejection {
            format!("supplied attestation was rejected: {rejection}")
        } else if evaluation.signer_revoked == Some(true) {
            "supplied attestation uses a revoked signing key".to_string()
        } else if evaluation.signer_authorized == Some(false) {
            "supplied attestation uses a signing key that is not authorized".to_string()
        } else if evaluation.signer_authorized.is_none()
            && evaluation.signature_valid
            && evaluation.matches_history
        {
            "self-signed v1 receipt matches history, but signer authority was not checked"
                .to_string()
        } else {
            "supplied attestation does not match a verified checkpoint".to_string()
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

#[cfg(feature = "attestation")]
fn evaluate_attestation(
    memory: &MemoryEngine,
    receipt: &LedgerAttestation,
    keyring: Option<&AttestationKeyring>,
) -> TrustAttestationEvaluation {
    if let Some(keyring) = keyring {
        return match memory.verify_attested_checkpoint_with_keyring(receipt, keyring) {
            Ok(verdict) => TrustAttestationEvaluation {
                format: TRUST_ATTESTATION_FORMAT_V1.to_string(),
                sequence: receipt.sequence,
                signature_valid: verdict.signature_valid,
                matches_history: verdict.matches_history,
                signer_authorized: Some(verdict.key_trusted),
                signer_revoked: Some(verdict.key_revoked),
                trusted: verdict.is_trusted(),
                rejection: None,
            },
            Err(error) => TrustAttestationEvaluation {
                format: TRUST_ATTESTATION_FORMAT_V1.to_string(),
                sequence: receipt.sequence,
                signature_valid: false,
                matches_history: false,
                signer_authorized: Some(keyring.authorizes(&receipt.public_key)),
                signer_revoked: Some(keyring.is_revoked(&receipt.public_key)),
                trusted: false,
                rejection: Some(error.to_string()),
            },
        };
    }

    match memory.verify_attested_checkpoint(receipt) {
        Ok(verdict) => TrustAttestationEvaluation {
            format: TRUST_ATTESTATION_FORMAT_V1.to_string(),
            sequence: receipt.sequence,
            signature_valid: verdict.signature_valid,
            matches_history: verdict.matches_history,
            signer_authorized: None,
            signer_revoked: None,
            trusted: false,
            rejection: None,
        },
        Err(error) => TrustAttestationEvaluation {
            format: TRUST_ATTESTATION_FORMAT_V1.to_string(),
            sequence: receipt.sequence,
            signature_valid: false,
            matches_history: false,
            signer_authorized: None,
            signer_revoked: None,
            trusted: false,
            rejection: Some(error.to_string()),
        },
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
    #[cfg(feature = "attestation")]
    use crate::{AttestationKey, AttestationKeyStatus};
    use crate::{AuthorityMode, MemoryData};

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
            chain_status: LedgerChainStatus::Verified,
            #[cfg(feature = "tamper-evidence")]
            chain_tip: None,
            #[cfg(feature = "tamper-evidence")]
            merkle_root: None,
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
                .any(|reason| reason.contains("recorded-history checks passed"))
        );
    }

    #[test]
    fn integrity_can_pass_while_the_composite_use_verdict_fails() {
        let data = MemoryData::default();
        let health = KnowledgeHealth::inspect_at(&data, 1_000);
        let authority = AuthorityDecision {
            mode: AuthorityMode::Block,
            score: 0.0,
            can_trust: false,
            reasons: vec!["The memory lacks usable evidence.".to_string()],
            signal_kinds: Vec::new(),
        };

        let report = assemble_trust_report(
            empty_knowledge(),
            authority,
            health,
            intact_integrity(),
            #[cfg(feature = "attestation")]
            None,
            1_000,
        );

        assert!(report.integrity.ledger_verified);
        assert!(!report.authority.can_trust);
        assert!(!report.trustworthy);
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
            chain_status: LedgerChainStatus::Verified,
            #[cfg(feature = "tamper-evidence")]
            chain_tip: None,
            #[cfg(feature = "tamper-evidence")]
            merkle_root: None,
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
                .any(|reason| reason.contains("recorded-history checks failed"))
        );
    }

    #[cfg(feature = "tamper-evidence")]
    #[test]
    fn legacy_chain_is_reported_without_calling_it_broken_or_verified() {
        let data = MemoryData::default();
        let health = KnowledgeHealth::inspect_at(&data, 1_000);
        let authority = AuthorityDecision::evaluate(&health);
        let integrity = TrustIntegrity {
            ledger_verified: false,
            checksums_valid: true,
            sequence_contiguous: true,
            chain_intact: false,
            chain_status: LedgerChainStatus::Legacy,
            chain_tip: None,
            merkle_root: None,
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
                .any(|reason| reason.contains("legacy unchained history"))
        );
        assert!(
            !report
                .verdict_reasons
                .iter()
                .any(|reason| reason.contains("hash chain is broken"))
        );
    }

    #[cfg(feature = "attestation")]
    #[test]
    fn a_supplied_self_signed_receipt_requires_an_operator_keyring() {
        let path = temp_store("trust_report_keyring");
        let _ = std::fs::remove_file(&path);
        let mut memory = MemoryEngine::open(&path).expect("open test store");
        memory
            .remember(
                "Hrafn retains the signed decision.",
                vec!["test".to_string()],
            )
            .expect("record test episode");
        let receipt = memory
            .attest_chain_tip(&"01".repeat(32))
            .expect("sign current tip")
            .expect("non-empty ledger has a tip");

        let self_signed = memory.trust_report_with_options(TrustReportOptions {
            attestation: Some(receipt.clone()),
            keyring: None,
        });
        let evaluation = self_signed.attestation.expect("receipt remains visible");
        assert!(evaluation.signature_valid);
        assert!(evaluation.matches_history);
        assert_eq!(evaluation.signer_authorized, None);
        assert!(!evaluation.trusted);

        let keyring = AttestationKeyring {
            keys: vec![AttestationKey {
                key_id: Some("test-primary".to_string()),
                public_key: receipt.public_key.clone(),
                status: AttestationKeyStatus::Active,
            }],
        };
        let authorized = memory.trust_report_with_options(TrustReportOptions {
            attestation: Some(receipt),
            keyring: Some(keyring),
        });
        assert!(
            authorized
                .attestation
                .expect("authorized receipt remains visible")
                .trusted
        );

        drop(memory);
        let _ = std::fs::remove_file(path);
    }

    #[cfg(feature = "attestation")]
    #[test]
    fn malformed_supplied_attestation_is_retained_as_a_rejection() {
        let path = temp_store("trust_report_rejected_attestation");
        let _ = std::fs::remove_file(&path);
        let memory = MemoryEngine::open(&path).expect("open test store");
        let receipt = LedgerAttestation {
            version: 1,
            algorithm: "unsupported".to_string(),
            sequence: 1,
            tip: "00".repeat(32),
            public_key: "00".repeat(32),
            signature: "00".repeat(64),
        };

        let report = memory.trust_report_with_options(TrustReportOptions {
            attestation: Some(receipt),
            keyring: None,
        });
        let evaluation = report.attestation.expect("rejection must not disappear");
        assert!(!evaluation.trusted);
        assert!(evaluation.rejection.is_some());
        assert!(
            report
                .verdict_reasons
                .iter()
                .any(|reason| reason.contains("supplied attestation was rejected"))
        );

        drop(memory);
        let _ = std::fs::remove_file(path);
    }

    #[cfg(feature = "attestation")]
    fn temp_store(label: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("nahuali_{label}_{}_{}", std::process::id(), nanos))
    }
}
