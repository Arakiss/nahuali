use serde::{Deserialize, Serialize};

use crate::inspection::{HealthSeverity, HealthSignalKind, KnowledgeHealth};
use crate::model::RecallResult;

/// Authority mode assigned to a memory answer.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityMode {
    /// Memory is usable but callers should still inspect the health report.
    Advisory,
    /// Memory is usable only with explicit uncertainty.
    Warn,
    /// Memory should not be trusted without more evidence.
    Block,
    /// Memory is currently supported by the available health checks.
    Certify,
}

/// Deterministic trust decision derived from knowledge health.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct AuthorityDecision {
    /// Chosen authority mode.
    pub mode: AuthorityMode,
    /// Numeric trust score in the `0.0..=1.0` range.
    pub score: f32,
    /// Whether callers may rely on memory without adding uncertainty.
    pub can_trust: bool,
    /// Human-readable reasons for the decision.
    pub reasons: Vec<String>,
    /// Health signal kinds that drove the decision.
    pub signal_kinds: Vec<HealthSignalKind>,
}

impl AuthorityDecision {
    /// Evaluate a health report into a deterministic authority decision.
    pub fn evaluate(health: &KnowledgeHealth) -> Self {
        let mode = if health
            .signals
            .iter()
            .any(|signal| signal.severity == HealthSeverity::High)
        {
            AuthorityMode::Block
        } else if health
            .signals
            .iter()
            .any(|signal| signal.severity == HealthSeverity::Medium)
        {
            AuthorityMode::Warn
        } else if health
            .signals
            .iter()
            .any(|signal| signal.severity == HealthSeverity::Low)
        {
            AuthorityMode::Advisory
        } else {
            AuthorityMode::Certify
        };
        let score = match mode {
            AuthorityMode::Certify => 1.0,
            AuthorityMode::Advisory => 0.75,
            AuthorityMode::Warn => 0.5,
            AuthorityMode::Block => 0.0,
        };
        let reasons = if health.signals.is_empty() {
            vec!["No health signals require attention.".to_string()]
        } else {
            health
                .signals
                .iter()
                .map(|signal| signal.message.clone())
                .collect()
        };
        let mut signal_kinds = Vec::new();
        for signal in &health.signals {
            if !signal_kinds.contains(&signal.kind) {
                signal_kinds.push(signal.kind.clone());
            }
        }

        Self {
            can_trust: mode == AuthorityMode::Certify,
            mode,
            score,
            reasons,
            signal_kinds,
        }
    }
}

/// Recall result paired with health and authority context.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct AuthorityRecall {
    /// Scored recall candidates.
    pub results: Vec<RecallResult>,
    /// Authority decision for the current projected memory.
    pub authority: AuthorityDecision,
    /// Health report used to produce the authority decision.
    pub health: KnowledgeHealth,
}

#[cfg(test)]
mod tests {
    use crate::{HealthDimension, HealthSeverity, HealthSignal, HealthSignalKind, KnowledgeHealth};

    use super::{AuthorityDecision, AuthorityMode};

    #[test]
    fn certifies_memory_without_health_signals() {
        let health = health_with_signals(Vec::new());

        let authority = AuthorityDecision::evaluate(&health);

        assert_eq!(authority.mode, AuthorityMode::Certify);
        assert!(authority.can_trust);
        assert_eq!(authority.score, 1.0);
    }

    #[test]
    fn blocks_high_severity_memory() {
        let health = health_with_signals(vec![signal(
            HealthSignalKind::ConflictingFact,
            HealthSeverity::High,
        )]);

        let authority = AuthorityDecision::evaluate(&health);

        assert_eq!(authority.mode, AuthorityMode::Block);
        assert!(!authority.can_trust);
        assert_eq!(authority.score, 0.0);
        assert_eq!(
            authority.signal_kinds,
            vec![HealthSignalKind::ConflictingFact]
        );
    }

    #[test]
    fn warns_for_medium_severity_memory() {
        let health = health_with_signals(vec![signal(
            HealthSignalKind::UnsupportedFact,
            HealthSeverity::Medium,
        )]);

        let authority = AuthorityDecision::evaluate(&health);

        assert_eq!(authority.mode, AuthorityMode::Warn);
        assert!(!authority.can_trust);
    }

    #[test]
    fn uses_advisory_for_low_severity_memory() {
        let health = health_with_signals(vec![signal(
            HealthSignalKind::IsolatedEntity,
            HealthSeverity::Low,
        )]);

        let authority = AuthorityDecision::evaluate(&health);

        assert_eq!(authority.mode, AuthorityMode::Advisory);
        assert!(!authority.can_trust);
    }

    #[test]
    fn warns_for_recency_resolved_supersession() {
        let health = health_with_signals(vec![signal(
            HealthSignalKind::SupersededFact,
            HealthSeverity::Medium,
        )]);

        let authority = AuthorityDecision::evaluate(&health);

        assert_eq!(authority.mode, AuthorityMode::Warn);
        assert!(!authority.can_trust);
        assert_eq!(authority.score, 0.5);
    }

    #[test]
    fn deduplicates_signal_kinds_in_first_seen_order() {
        let health = health_with_signals(vec![
            signal(HealthSignalKind::UnsupportedFact, HealthSeverity::Medium),
            signal(HealthSignalKind::UnsupportedFact, HealthSeverity::Medium),
            signal(HealthSignalKind::LowConfidenceFact, HealthSeverity::Medium),
        ]);

        let authority = AuthorityDecision::evaluate(&health);

        assert_eq!(
            authority.signal_kinds,
            vec![
                HealthSignalKind::UnsupportedFact,
                HealthSignalKind::LowConfidenceFact
            ]
        );
    }

    fn health_with_signals(signals: Vec<HealthSignal>) -> KnowledgeHealth {
        KnowledgeHealth {
            event_count: 0,
            episode_count: 0,
            fact_count: 0,
            relation_count: 0,
            entity_count: 0,
            supported_fact_count: 0,
            unsupported_fact_count: 0,
            low_confidence_fact_count: 0,
            conflicting_fact_count: 0,
            stale_fact_count: 0,
            superseded_fact_count: 0,
            isolated_entity_count: 0,
            blind_spot_count: signals
                .iter()
                .filter(|signal| {
                    matches!(
                        signal.kind,
                        HealthSignalKind::NoEpisodes | HealthSignalKind::IsolatedEntity
                    )
                })
                .count(),
            signal_count: signals.len(),
            average_fact_confidence: 0.0,
            warnings: signals
                .iter()
                .map(|signal| signal.message.clone())
                .collect(),
            signals,
        }
    }

    fn signal(kind: HealthSignalKind, severity: HealthSeverity) -> HealthSignal {
        HealthSignal {
            kind,
            dimensions: vec![HealthDimension::BlindSpot],
            severity,
            message: "memory requires attention".to_string(),
            evidence_ids: Vec::new(),
        }
    }
}
