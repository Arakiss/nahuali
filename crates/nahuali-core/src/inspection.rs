use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::model::{Fact, MemoryData, Relation, ReviewDecisionOutcome};

const STALE_AFTER_DAYS: u64 = 90;
const DAY_MS: u64 = 24 * 60 * 60 * 1000;

/// Structured health report for a projected memory store.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct KnowledgeHealth {
    /// Number of events included in the inspected projection.
    pub event_count: usize,
    /// Number of projected episodes.
    pub episode_count: usize,
    /// Number of projected facts.
    pub fact_count: usize,
    /// Number of projected relations.
    pub relation_count: usize,
    /// Number of entities discovered from facts and relations.
    pub entity_count: usize,
    /// Number of facts with a source episode.
    pub supported_fact_count: usize,
    /// Number of facts without a source episode.
    pub unsupported_fact_count: usize,
    /// Number of facts below the confidence threshold.
    pub low_confidence_fact_count: usize,
    /// Number of subject/predicate groups with conflicting values.
    pub conflicting_fact_count: usize,
    /// Number of facts older than the staleness threshold.
    pub stale_fact_count: usize,
    /// Number of entities with no relation edges.
    pub isolated_entity_count: usize,
    /// Number of health signals requiring caller attention.
    pub blind_spot_count: usize,
    /// Mean confidence for projected facts, rounded to two decimals.
    pub average_fact_confidence: f32,
    /// Structured signals behind the aggregate counts.
    pub signals: Vec<HealthSignal>,
    /// Human-readable warning messages derived from the signals.
    pub warnings: Vec<String>,
}

/// Single knowledge-health issue or observation.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct HealthSignal {
    /// Signal category.
    pub kind: HealthSignalKind,
    /// Higher-level health dimensions affected by this signal.
    pub dimensions: Vec<HealthDimension>,
    /// Signal severity.
    pub severity: HealthSeverity,
    /// Human-readable explanation.
    pub message: String,
    /// Event or memory identifiers that support the signal.
    pub evidence_ids: Vec<String>,
}

/// Higher-level knowledge-health dimension.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HealthDimension {
    /// Coverage and completeness of available memory.
    Completeness,
    /// Confidence calibration for derived memory.
    Confidence,
    /// Recency and freshness of available memory.
    Freshness,
    /// Graph connectivity around known entities.
    Connectivity,
    /// Contradictory claims or links.
    Contradiction,
    /// Explicit staleness risk.
    Staleness,
    /// Memory detached from evidence.
    UnsupportedMemory,
    /// Caller-visible blind spot.
    BlindSpot,
}

/// Category for a knowledge-health signal.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HealthSignalKind {
    /// No episodes exist in the projection.
    NoEpisodes,
    /// A fact has no source episode.
    UnsupportedFact,
    /// A fact is below the confidence threshold.
    LowConfidenceFact,
    /// Facts with the same subject and predicate disagree on the object.
    ConflictingFact,
    /// A fact is older than the staleness threshold.
    StaleFact,
    /// An entity appears in facts but has no relation edges.
    IsolatedEntity,
}

/// Severity level for a knowledge-health signal.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum HealthSeverity {
    /// High-priority signal that should usually block blind trust.
    High,
    /// Medium-priority signal that should be surfaced to callers.
    Medium,
    /// Low-priority signal that may still matter for context quality.
    Low,
}

impl KnowledgeHealth {
    /// Inspect the projection using the current system time for staleness.
    pub fn inspect(data: &MemoryData) -> Self {
        Self::inspect_at(data, now_ms())
    }

    /// Inspect the projection using an explicit timestamp for deterministic
    /// tests or reproducible evaluations.
    pub fn inspect_at(data: &MemoryData, now_ms: u64) -> Self {
        let facts = projected_facts(data);
        let relations = projected_relations(data);
        let resolved_review_evidence = resolved_review_evidence(data);
        let fact_count = facts.len();
        let supported_fact_count = facts
            .iter()
            .filter(|fact| fact.source_episode_id.is_some())
            .count();
        let unsupported_fact_count = facts
            .iter()
            .filter(|fact| {
                fact.source_episode_id.is_none()
                    && !evidence_reviewed(&[fact.event_id.as_str()], &resolved_review_evidence)
            })
            .count();
        let low_confidence_fact_count = facts
            .iter()
            .filter(|fact| {
                fact.confidence < 0.5
                    && !evidence_reviewed(&[fact.event_id.as_str()], &resolved_review_evidence)
            })
            .count();
        let average_fact_confidence = if fact_count == 0 {
            0.0
        } else {
            round2(facts.iter().map(|fact| fact.confidence).sum::<f32>() / fact_count as f32)
        };

        let mut signals = Vec::new();

        if data.episodes.is_empty() {
            signals.push(HealthSignal {
                kind: HealthSignalKind::NoEpisodes,
                dimensions: vec![HealthDimension::Completeness, HealthDimension::BlindSpot],
                severity: HealthSeverity::High,
                message: "No episodes stored yet.".to_string(),
                evidence_ids: Vec::new(),
            });
        }

        for fact in facts.iter().filter(|fact| {
            fact.source_episode_id.is_none()
                && !evidence_reviewed(&[fact.event_id.as_str()], &resolved_review_evidence)
        }) {
            signals.push(HealthSignal {
                kind: HealthSignalKind::UnsupportedFact,
                dimensions: vec![
                    HealthDimension::UnsupportedMemory,
                    HealthDimension::BlindSpot,
                ],
                severity: HealthSeverity::Medium,
                message: format!("Fact '{}' has no source episode.", fact_statement(fact)),
                evidence_ids: vec![fact.event_id.clone()],
            });
        }

        for fact in facts.iter().filter(|fact| {
            fact.confidence < 0.5
                && !evidence_reviewed(&[fact.event_id.as_str()], &resolved_review_evidence)
        }) {
            signals.push(HealthSignal {
                kind: HealthSignalKind::LowConfidenceFact,
                dimensions: vec![HealthDimension::Confidence],
                severity: HealthSeverity::Medium,
                message: format!(
                    "Fact '{}' is below confidence threshold.",
                    fact_statement(fact)
                ),
                evidence_ids: vec![fact.event_id.clone()],
            });
        }

        let conflicts = conflicting_facts(facts)
            .into_iter()
            .filter(|conflict| {
                !evidence_reviewed(
                    &conflict
                        .evidence_ids
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>(),
                    &resolved_review_evidence,
                )
            })
            .collect::<Vec<_>>();
        for conflict in &conflicts {
            signals.push(HealthSignal {
                kind: HealthSignalKind::ConflictingFact,
                dimensions: vec![HealthDimension::Contradiction, HealthDimension::BlindSpot],
                severity: HealthSeverity::High,
                message: format!(
                    "{} {} has conflicting values: {}",
                    conflict.subject,
                    conflict.predicate,
                    conflict
                        .values
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                evidence_ids: conflict.evidence_ids.iter().cloned().collect(),
            });
        }

        let stale_before_ms = now_ms.saturating_sub(STALE_AFTER_DAYS * DAY_MS);
        let stale_facts = facts
            .iter()
            .filter(|fact| {
                fact.created_at_ms < stale_before_ms
                    && !evidence_reviewed(&[fact.event_id.as_str()], &resolved_review_evidence)
            })
            .collect::<Vec<_>>();
        for fact in &stale_facts {
            signals.push(HealthSignal {
                kind: HealthSignalKind::StaleFact,
                dimensions: vec![HealthDimension::Freshness, HealthDimension::Staleness],
                severity: HealthSeverity::Medium,
                message: format!("Fact '{}' is stale.", fact_statement(fact)),
                evidence_ids: vec![fact.event_id.clone()],
            });
        }

        let entity_graph = entity_graph(data);
        let isolated_entities = entity_graph
            .iter()
            .filter_map(|(entity, relation_count)| {
                if *relation_count == 0 {
                    Some(entity.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        for entity in &isolated_entities {
            signals.push(HealthSignal {
                kind: HealthSignalKind::IsolatedEntity,
                dimensions: vec![HealthDimension::Connectivity, HealthDimension::BlindSpot],
                severity: HealthSeverity::Low,
                message: format!("Entity '{entity}' is not connected by any relation."),
                evidence_ids: Vec::new(),
            });
        }

        let warnings = signals
            .iter()
            .map(|signal| signal.message.clone())
            .collect::<Vec<_>>();

        Self {
            event_count: data.event_count,
            episode_count: data.episodes.len(),
            fact_count,
            relation_count: relations.len(),
            entity_count: entity_graph.len(),
            supported_fact_count,
            unsupported_fact_count,
            low_confidence_fact_count,
            conflicting_fact_count: conflicts.len(),
            stale_fact_count: stale_facts.len(),
            isolated_entity_count: isolated_entities.len(),
            blind_spot_count: signals.len(),
            average_fact_confidence,
            signals,
            warnings,
        }
    }
}

#[derive(Debug)]
struct FactConflict {
    subject: String,
    predicate: String,
    values: BTreeSet<String>,
    evidence_ids: BTreeSet<String>,
}

fn conflicting_facts(facts: &[Fact]) -> Vec<FactConflict> {
    let mut groups: BTreeMap<(&str, &str), Vec<&Fact>> = BTreeMap::new();
    for fact in facts {
        groups
            .entry((fact.subject.as_str(), fact.predicate.as_str()))
            .or_default()
            .push(fact);
    }

    groups
        .into_iter()
        .filter_map(|((subject, predicate), facts)| {
            let values = facts
                .iter()
                .map(|fact| fact.object.clone())
                .collect::<BTreeSet<_>>();
            if values.len() <= 1 {
                return None;
            }

            let evidence_ids = facts
                .iter()
                .map(|fact| fact.event_id.clone())
                .collect::<BTreeSet<_>>();

            Some(FactConflict {
                subject: subject.to_string(),
                predicate: predicate.to_string(),
                values,
                evidence_ids,
            })
        })
        .collect()
}

fn entity_graph(data: &MemoryData) -> BTreeMap<String, usize> {
    let mut entities = BTreeMap::new();
    let facts = projected_facts(data);
    let relations = projected_relations(data);

    for entity in &data.entities {
        entities.entry(entity_key(&entity.name)).or_insert(0);
    }

    for fact in facts {
        entities.entry(entity_key(&fact.subject)).or_insert(0);
        entities.entry(entity_key(&fact.object)).or_insert(0);
    }

    for relation in relations {
        *entities.entry(entity_key(&relation.from)).or_insert(0) += 1;
        *entities.entry(entity_key(&relation.to)).or_insert(0) += 1;
    }

    entities
}

fn entity_key(name: &str) -> String {
    name.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn projected_facts(data: &MemoryData) -> &[Fact] {
    if data.claims.is_empty() {
        &data.facts
    } else {
        &data.claims
    }
}

fn projected_relations(data: &MemoryData) -> &[Relation] {
    if data.links.is_empty() {
        &data.relations
    } else {
        &data.links
    }
}

fn fact_statement(fact: &Fact) -> String {
    format!("{} {} {}", fact.subject, fact.predicate, fact.object)
}

fn resolved_review_evidence(data: &MemoryData) -> Vec<BTreeSet<String>> {
    data.review_decisions
        .iter()
        .filter(|decision| decision.outcome == ReviewDecisionOutcome::Resolved)
        .filter(|decision| !decision.evidence_ids.is_empty())
        .map(|decision| decision.evidence_ids.iter().cloned().collect())
        .collect()
}

fn evidence_reviewed(evidence_ids: &[&str], resolved_evidence: &[BTreeSet<String>]) -> bool {
    !evidence_ids.is_empty()
        && resolved_evidence.iter().any(|reviewed| {
            evidence_ids
                .iter()
                .all(|evidence_id| reviewed.contains(*evidence_id))
        })
}

fn round2(value: f32) -> f32 {
    (value * 100.0).round() / 100.0
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use crate::model::{Fact, MemoryData, Relation};

    use super::{HealthSignalKind, KnowledgeHealth};

    #[test]
    fn reports_conflicting_fact_assertions() {
        let data = MemoryData {
            event_count: 2,
            last_event_id: Some("event_2".to_string()),
            facts: vec![
                fact("fact_1", "event_1", "Atlas", "status", "draft", 1000, 0.9),
                fact(
                    "fact_2",
                    "event_2",
                    "Atlas",
                    "status",
                    "published",
                    1001,
                    0.9,
                ),
            ],
            ..MemoryData::default()
        };

        let health = KnowledgeHealth::inspect_at(&data, 1001);

        assert_eq!(health.conflicting_fact_count, 1);
        assert!(
            health
                .signals
                .iter()
                .any(|signal| signal.kind == HealthSignalKind::ConflictingFact)
        );
    }

    #[test]
    fn reports_stale_and_isolated_entities() {
        let now = 120 * 24 * 60 * 60 * 1000;
        let data = MemoryData {
            event_count: 1,
            last_event_id: Some("event_1".to_string()),
            facts: vec![fact("fact_1", "event_1", "Lena", "owns", "Roadmap", 1, 0.8)],
            ..MemoryData::default()
        };

        let health = KnowledgeHealth::inspect_at(&data, now);

        assert_eq!(health.stale_fact_count, 1);
        assert_eq!(health.isolated_entity_count, 2);
        assert!(
            health
                .signals
                .iter()
                .any(|signal| signal.kind == HealthSignalKind::StaleFact)
        );
    }

    #[test]
    fn relations_connect_entities() {
        let data = MemoryData {
            event_count: 2,
            last_event_id: Some("event_2".to_string()),
            facts: vec![fact(
                "fact_1", "event_1", "Lena", "owns", "Roadmap", 1000, 0.8,
            )],
            relations: vec![Relation {
                id: "relation_1".to_string(),
                event_id: "event_2".to_string(),
                from: "Lena".to_string(),
                relation: "owns".to_string(),
                to: "Roadmap".to_string(),
                source_episode_id: Some("episode_1".to_string()),
                confidence: 0.9,
                scope: None,
                created_at_ms: 1001,
            }],
            ..MemoryData::default()
        };

        let health = KnowledgeHealth::inspect_at(&data, 1001);

        assert_eq!(health.entity_count, 2);
        assert_eq!(health.isolated_entity_count, 0);
    }

    fn fact(
        id: &str,
        event_id: &str,
        subject: &str,
        predicate: &str,
        object: &str,
        created_at_ms: u64,
        confidence: f32,
    ) -> Fact {
        Fact {
            id: id.to_string(),
            event_id: event_id.to_string(),
            subject: subject.to_string(),
            predicate: predicate.to_string(),
            object: object.to_string(),
            source_episode_id: None,
            confidence,
            scope: None,
            created_at_ms,
        }
    }
}
