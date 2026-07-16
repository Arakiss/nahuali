use nahuali_core::{MemoryTrustReport, TrustIntegrity, TrustKnowledge};
use rmcp::schemars;
use serde::Serialize;

#[cfg(feature = "tamper-evidence")]
use super::LedgerChainStatusView;
use super::{AuthorityDecisionView, HealthView};

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct TrustReportResult {
    version: u32,
    generated_at_ms: u64,
    knowledge: TrustKnowledgeView,
    authority: AuthorityDecisionView,
    integrity: TrustIntegrityView,
    health: HealthView,
    trustworthy: bool,
    verdict_reasons: Vec<String>,
}

impl From<MemoryTrustReport> for TrustReportResult {
    fn from(report: MemoryTrustReport) -> Self {
        Self {
            version: report.version,
            generated_at_ms: report.generated_at_ms,
            knowledge: report.knowledge.into(),
            authority: report.authority.into(),
            integrity: report.integrity.into(),
            health: report.health.into(),
            trustworthy: report.trustworthy,
            verdict_reasons: report.verdict_reasons,
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct TrustKnowledgeView {
    event_count: usize,
    source_count: usize,
    entity_count: usize,
    episode_count: usize,
    claim_count: usize,
    link_count: usize,
    procedure_count: usize,
    intention_count: usize,
}

impl From<TrustKnowledge> for TrustKnowledgeView {
    fn from(knowledge: TrustKnowledge) -> Self {
        Self {
            event_count: knowledge.event_count,
            source_count: knowledge.source_count,
            entity_count: knowledge.entity_count,
            episode_count: knowledge.episode_count,
            claim_count: knowledge.claim_count,
            link_count: knowledge.link_count,
            procedure_count: knowledge.procedure_count,
            intention_count: knowledge.intention_count,
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct TrustIntegrityView {
    ledger_verified: bool,
    checksums_valid: bool,
    sequence_contiguous: bool,
    #[cfg(feature = "tamper-evidence")]
    chain_intact: bool,
    #[cfg(feature = "tamper-evidence")]
    chain_status: LedgerChainStatusView,
    #[cfg(feature = "tamper-evidence")]
    #[serde(skip_serializing_if = "Option::is_none")]
    chain_tip: Option<String>,
    #[cfg(feature = "tamper-evidence")]
    #[serde(skip_serializing_if = "Option::is_none")]
    merkle_root: Option<String>,
}

impl From<TrustIntegrity> for TrustIntegrityView {
    fn from(integrity: TrustIntegrity) -> Self {
        Self {
            ledger_verified: integrity.ledger_verified,
            checksums_valid: integrity.checksums_valid,
            sequence_contiguous: integrity.sequence_contiguous,
            #[cfg(feature = "tamper-evidence")]
            chain_intact: integrity.chain_intact,
            #[cfg(feature = "tamper-evidence")]
            chain_status: integrity.chain_status.into(),
            #[cfg(feature = "tamper-evidence")]
            chain_tip: integrity.chain_tip,
            #[cfg(feature = "tamper-evidence")]
            merkle_root: integrity.merkle_root,
        }
    }
}
