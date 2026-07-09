use nahuali_core::{
    AuthorityDecision, HealthSignal, KnowledgeHealth, SelfInspectionWriteBackPolicy,
};
use rmcp::schemars;
use serde::Serialize;

use super::json_string;

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct AuthorityDecisionView {
    mode: String,
    score: f32,
    can_trust: bool,
    reasons: Vec<String>,
    signal_kinds: Vec<String>,
}

impl From<AuthorityDecision> for AuthorityDecisionView {
    fn from(authority: AuthorityDecision) -> Self {
        let mode = json_string(&authority.mode);
        let signal_kinds = authority.signal_kinds.iter().map(json_string).collect();
        Self {
            mode,
            score: authority.score,
            can_trust: authority.can_trust,
            reasons: authority.reasons,
            signal_kinds,
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct HealthView {
    event_count: usize,
    episode_count: usize,
    fact_count: usize,
    relation_count: usize,
    entity_count: usize,
    supported_fact_count: usize,
    unsupported_fact_count: usize,
    low_confidence_fact_count: usize,
    conflicting_fact_count: usize,
    stale_fact_count: usize,
    isolated_entity_count: usize,
    blind_spot_count: usize,
    signal_count: usize,
    average_fact_confidence: f32,
    signals: Vec<HealthSignalView>,
    warnings: Vec<String>,
}

impl From<KnowledgeHealth> for HealthView {
    fn from(health: KnowledgeHealth) -> Self {
        Self {
            event_count: health.event_count,
            episode_count: health.episode_count,
            fact_count: health.fact_count,
            relation_count: health.relation_count,
            entity_count: health.entity_count,
            supported_fact_count: health.supported_fact_count,
            unsupported_fact_count: health.unsupported_fact_count,
            low_confidence_fact_count: health.low_confidence_fact_count,
            conflicting_fact_count: health.conflicting_fact_count,
            stale_fact_count: health.stale_fact_count,
            isolated_entity_count: health.isolated_entity_count,
            blind_spot_count: health.blind_spot_count,
            signal_count: health.signal_count,
            average_fact_confidence: health.average_fact_confidence,
            signals: health
                .signals
                .into_iter()
                .map(HealthSignalView::from)
                .collect(),
            warnings: health.warnings,
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct HealthSignalView {
    kind: String,
    dimensions: Vec<String>,
    severity: String,
    message: String,
    evidence_ids: Vec<String>,
}

impl From<HealthSignal> for HealthSignalView {
    fn from(signal: HealthSignal) -> Self {
        let kind = json_string(&signal.kind);
        let dimensions = signal.dimensions.iter().map(json_string).collect();
        let severity = json_string(&signal.severity);
        Self {
            kind,
            dimensions,
            severity,
            message: signal.message,
            evidence_ids: signal.evidence_ids,
        }
    }
}

/// Explicit write-back policy mirrored from the core self-inspection report.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct WriteBackPolicyView {
    automatic_write_back: bool,
    requires_operator_review: bool,
    message: String,
}

impl From<SelfInspectionWriteBackPolicy> for WriteBackPolicyView {
    fn from(policy: SelfInspectionWriteBackPolicy) -> Self {
        Self {
            automatic_write_back: policy.automatic_write_back,
            requires_operator_review: policy.requires_operator_review,
            message: policy.message,
        }
    }
}
