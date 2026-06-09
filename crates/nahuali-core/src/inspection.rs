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
    /// Number of facts retired by a newer, evidence-backed value for the same
    /// `(scope, subject, predicate)`.
    pub superseded_fact_count: usize,
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
    /// The store's most recent memory is older than the staleness threshold —
    /// a dormant store that should be used with judgment rather than trusted as
    /// current. Keeps an old, untouched episode log from silently certifying
    /// (a clean, fresh log otherwise raises no signal).
    StaleEpisode,
    /// A fact was replaced by a newer, evidence-backed fact for the same
    /// `(scope, subject, predicate)`.
    ///
    /// Exact-match syntactic supersession: the retired value differs from the
    /// current one, the newest fact holds a unique maximum timestamp, and that
    /// newest fact carries a source episode distinct from the value it retires
    /// (a new observation, not a re-statement within one episode). Honest
    /// limits: it cannot collapse an orderly value chain
    /// (`draft` -> `reviewed` -> `published`) into a single signal, it is keyed
    /// on ledger ingestion order rather than wall-clock causality, and an
    /// unprovenanced newer value or two differing values from the same episode
    /// are deliberately left as a [`HealthSignalKind::ConflictingFact`] instead
    /// of a clean supersession.
    SupersededFact,
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

        let (raw_conflicts, supersessions) = classify_fact_groups(facts);
        let conflicts = raw_conflicts
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

        // A newer observation disagrees with an older one for the same
        // (scope, subject, predicate). We cannot prove this was a clean refresh
        // rather than a cross-source contradiction, so it warrants caller
        // uncertainty (Medium -> Warn): the disagreement is surfaced, never
        // silently resolved into Certify, and never escalated to the hard Block
        // of a same-observation `ConflictingFact` (High). Each retired value
        // links both its own event and the superseding event so callers can see
        // what replaced what.
        let mut superseded_fact_count = 0;
        for supersession in &supersessions {
            for retired in &supersession.retired {
                if evidence_reviewed(
                    &[
                        retired.event_id.as_str(),
                        supersession.current_event_id.as_str(),
                    ],
                    &resolved_review_evidence,
                ) {
                    continue;
                }
                superseded_fact_count += 1;
                signals.push(HealthSignal {
                    kind: HealthSignalKind::SupersededFact,
                    dimensions: vec![HealthDimension::Freshness, HealthDimension::Staleness],
                    severity: HealthSeverity::Medium,
                    message: format!(
                        "{} {} value '{}' was superseded by '{}'.",
                        supersession.subject,
                        supersession.predicate,
                        retired.value,
                        supersession.current_value
                    ),
                    evidence_ids: vec![
                        retired.event_id.clone(),
                        supersession.current_event_id.clone(),
                    ],
                });
            }
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

        // Freshness as a trust dimension: if the store's most recent memory is
        // older than the staleness threshold, the store is dormant — flag it so a
        // stale, untouched episode log does not silently certify. A clean, fresh
        // log raises no signal and still certifies. Low severity: an old episode
        // is true history, not wrong — "use with judgment", not "verify".
        if let Some(latest_ms) = data
            .episodes
            .iter()
            .map(|episode| episode.created_at_ms)
            .max()
            && latest_ms < stale_before_ms
        {
            signals.push(HealthSignal {
                kind: HealthSignalKind::StaleEpisode,
                dimensions: vec![HealthDimension::Freshness, HealthDimension::Staleness],
                severity: HealthSeverity::Low,
                message: format!(
                    "The most recent memory is over {STALE_AFTER_DAYS} days old; the store may be stale."
                ),
                evidence_ids: Vec::new(),
            });
        }

        let entity_graph = entity_graph(data);
        // In a store that is building no knowledge yet — no facts and no relations,
        // e.g. a pure episode log whose only entities come from `--mention` — every
        // entity is trivially "isolated". That is noise, not a blind spot: an
        // episode-first memory is legitimate, not a deficiency. Isolation is only a
        // real gap once the store has knowledge (facts or relations) that leaves an
        // entity disconnected. Without this gate, a clean episode log is wrongly
        // capped at ADVISORY by a single orphan-from-mention signal.
        let building_knowledge = !facts.is_empty() || !relations.is_empty();
        let isolated_entities = if building_knowledge {
            entity_graph
                .iter()
                .filter_map(|(entity, relation_count)| {
                    if *relation_count == 0 {
                        Some(entity.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
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
            superseded_fact_count,
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

#[derive(Debug)]
struct Supersession {
    subject: String,
    predicate: String,
    current_value: String,
    current_event_id: String,
    retired: Vec<RetiredFact>,
}

#[derive(Debug)]
struct RetiredFact {
    value: String,
    event_id: String,
}

/// Partition `(scope, subject, predicate)` groups that disagree on the object
/// into clean supersessions and genuine conflicts.
///
/// A group with at least two distinct object values is a **clean
/// supersession** when a single fact holds the maximum `created_at_ms` (no tie
/// at the newest timestamp) and that newest fact carries a source episode. Only
/// the older facts whose value differs from the surviving one *and* whose
/// source episode differs from the winner's are retired. Every other
/// disagreeing group stays a **conflict**: a tie at the newest timestamp, a
/// newest value with no provenance, or two differing values recorded against
/// the same episode keep the High-severity `ConflictingFact` instead of
/// silently reclassifying a contradiction as a benign refresh. Grouping
/// includes the scope key so a `project:alpha` fact never supersedes or
/// conflicts with a `project:beta` fact that shares the same subject and
/// predicate.
fn classify_fact_groups(facts: &[Fact]) -> (Vec<FactConflict>, Vec<Supersession>) {
    let mut groups: BTreeMap<(Option<&str>, &str, &str), Vec<&Fact>> = BTreeMap::new();
    for fact in facts {
        let scope_key = fact.scope.as_ref().map(|scope| scope.key.as_str());
        groups
            .entry((scope_key, fact.subject.as_str(), fact.predicate.as_str()))
            .or_default()
            .push(fact);
    }

    let mut conflicts = Vec::new();
    let mut supersessions = Vec::new();

    for ((_, subject, predicate), group) in groups {
        let values = group
            .iter()
            .map(|fact| fact.object.clone())
            .collect::<BTreeSet<_>>();
        if values.len() <= 1 {
            continue;
        }

        let max_ts = group
            .iter()
            .map(|fact| fact.created_at_ms)
            .max()
            .unwrap_or_default();
        let newest = group
            .iter()
            .filter(|fact| fact.created_at_ms == max_ts)
            .collect::<Vec<_>>();

        if newest.len() == 1 {
            let winner = newest[0];
            if let Some(winner_episode) = winner.source_episode_id.as_deref() {
                // A supersession is a *new observation* replacing an old claim,
                // so the retired value must come from a different source episode
                // than the winner. Two differing values recorded against the
                // same episode are a contradiction within one observation, not a
                // refresh, and stay a conflict below.
                let retired = group
                    .iter()
                    .filter(|fact| {
                        fact.event_id != winner.event_id
                            && fact.object != winner.object
                            && fact.source_episode_id.as_deref() != Some(winner_episode)
                    })
                    .map(|fact| RetiredFact {
                        value: fact.object.clone(),
                        event_id: fact.event_id.clone(),
                    })
                    .collect::<Vec<_>>();
                if !retired.is_empty() {
                    supersessions.push(Supersession {
                        subject: subject.to_string(),
                        predicate: predicate.to_string(),
                        current_value: winner.object.clone(),
                        current_event_id: winner.event_id.clone(),
                        retired,
                    });
                    continue;
                }
            }
        }

        let evidence_ids = group
            .iter()
            .map(|fact| fact.event_id.clone())
            .collect::<BTreeSet<_>>();
        conflicts.push(FactConflict {
            subject: subject.to_string(),
            predicate: predicate.to_string(),
            values,
            evidence_ids,
        });
    }

    (conflicts, supersessions)
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

pub(crate) fn resolved_review_evidence(data: &MemoryData) -> Vec<BTreeSet<String>> {
    data.review_decisions
        .iter()
        .filter(|decision| decision.outcome == ReviewDecisionOutcome::Resolved)
        .filter(|decision| !decision.evidence_ids.is_empty())
        .map(|decision| decision.evidence_ids.iter().cloned().collect())
        .collect()
}

pub(crate) fn evidence_reviewed(
    evidence_ids: &[&str],
    resolved_evidence: &[BTreeSet<String>],
) -> bool {
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
    use crate::model::{Entity, Episode, Fact, MemoryData, MemoryScope, MemoryScopeKind, Relation};

    use super::{HealthSeverity, HealthSignalKind, KnowledgeHealth};

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

    #[test]
    fn episode_only_store_does_not_flag_mentioned_entities_as_isolated() {
        // A pure episode log: entities exist only from `--mention`, with no facts
        // and no relations. Isolation is expected and must not be flagged — it
        // would otherwise wrongly cap a clean episode log at ADVISORY.
        let mentioned = |id: &str, name: &str| Entity {
            id: id.to_string(),
            name: name.to_string(),
            mention_count: 1,
            first_seen_at_ms: 1000,
            last_seen_at_ms: 1000,
            source_event_ids: vec!["event_1".to_string()],
            scope: None,
        };
        let data = MemoryData {
            event_count: 1,
            last_event_id: Some("event_1".to_string()),
            episodes: vec![Episode {
                id: "episode_1".to_string(),
                event_id: "event_1".to_string(),
                content: "Worked on Syrinx and Polaris.".to_string(),
                tags: Vec::new(),
                mentions: vec!["Syrinx".to_string(), "Polaris".to_string()],
                source_id: None,
                source_position: None,
                source_role: None,
                scope: None,
                created_at_ms: 1000,
            }],
            entities: vec![
                mentioned("entity_1", "Syrinx"),
                mentioned("entity_2", "Polaris"),
            ],
            ..MemoryData::default()
        };

        let health = KnowledgeHealth::inspect_at(&data, 1000);

        assert_eq!(health.entity_count, 2);
        assert_eq!(health.isolated_entity_count, 0);
        // A clean episode log with mentioned-but-unlinked entities raises no health
        // signal at all, so it certifies — episode-first memory is first-class.
        assert!(health.signals.is_empty());
    }

    #[test]
    fn dormant_episode_log_is_flagged_stale() {
        let day = super::DAY_MS;
        let data = MemoryData {
            event_count: 1,
            last_event_id: Some("event_1".to_string()),
            episodes: vec![Episode {
                id: "episode_1".to_string(),
                event_id: "event_1".to_string(),
                content: "An old observation, untouched for months.".to_string(),
                tags: Vec::new(),
                mentions: Vec::new(),
                source_id: None,
                source_position: None,
                source_role: None,
                scope: None,
                created_at_ms: 10 * day,
            }],
            ..MemoryData::default()
        };

        // "Now" is well past the staleness window since the only episode, so the
        // dormant store must not silently certify.
        let health = KnowledgeHealth::inspect_at(&data, 200 * day);

        assert!(
            health
                .signals
                .iter()
                .any(|signal| signal.kind == HealthSignalKind::StaleEpisode)
        );
    }

    #[test]
    fn reports_clean_supersession_for_evidence_backed_refresh() {
        let data = MemoryData {
            event_count: 2,
            last_event_id: Some("event_2".to_string()),
            facts: vec![
                sourced(
                    "fact_1",
                    "event_1",
                    "episode_1",
                    "Atlas",
                    "status",
                    "draft",
                    1000,
                    0.9,
                ),
                sourced(
                    "fact_2",
                    "event_2",
                    "episode_2",
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

        assert_eq!(health.superseded_fact_count, 1);
        assert_eq!(health.conflicting_fact_count, 0);
        let superseded = health
            .signals
            .iter()
            .find(|signal| signal.kind == HealthSignalKind::SupersededFact)
            .expect("superseded signal present");
        assert_eq!(superseded.severity, HealthSeverity::Medium);
        assert!(superseded.evidence_ids.contains(&"event_1".to_string()));
        assert!(superseded.evidence_ids.contains(&"event_2".to_string()));
        assert!(
            !health
                .signals
                .iter()
                .any(|signal| signal.kind == HealthSignalKind::ConflictingFact)
        );
    }

    #[test]
    fn supersession_and_staleness_are_independent() {
        let day = super::DAY_MS;
        let now = 200 * day;
        let data = MemoryData {
            event_count: 3,
            last_event_id: Some("event_3".to_string()),
            facts: vec![
                sourced(
                    "fact_1",
                    "event_1",
                    "episode_1",
                    "Atlas",
                    "status",
                    "draft",
                    5 * day,
                    0.9,
                ),
                sourced(
                    "fact_2",
                    "event_2",
                    "episode_2",
                    "Atlas",
                    "status",
                    "review",
                    10 * day,
                    0.9,
                ),
                sourced(
                    "fact_3",
                    "event_3",
                    "episode_3",
                    "Atlas",
                    "status",
                    "published",
                    120 * day,
                    0.9,
                ),
            ],
            ..MemoryData::default()
        };

        let health = KnowledgeHealth::inspect_at(&data, now);

        assert_eq!(health.superseded_fact_count, 2);
        assert_eq!(health.stale_fact_count, 2);
        assert_eq!(health.conflicting_fact_count, 0);
    }

    #[test]
    fn supersession_groups_by_scope() {
        let mut alpha = sourced(
            "fact_1",
            "event_1",
            "episode_1",
            "Atlas",
            "status",
            "draft",
            1000,
            0.9,
        );
        alpha.scope = Some(MemoryScope::new(MemoryScopeKind::Project, "alpha").expect("scope"));
        let mut beta = sourced(
            "fact_2",
            "event_2",
            "episode_2",
            "Atlas",
            "status",
            "published",
            1001,
            0.9,
        );
        beta.scope = Some(MemoryScope::new(MemoryScopeKind::Project, "beta").expect("scope"));

        let data = MemoryData {
            event_count: 2,
            last_event_id: Some("event_2".to_string()),
            facts: vec![alpha, beta],
            ..MemoryData::default()
        };

        let health = KnowledgeHealth::inspect_at(&data, 1001);

        assert_eq!(health.superseded_fact_count, 0);
        assert_eq!(health.conflicting_fact_count, 0);
    }

    #[test]
    fn single_old_fact_is_stale_not_superseded() {
        let now = 120 * super::DAY_MS;
        let data = MemoryData {
            event_count: 1,
            last_event_id: Some("event_1".to_string()),
            facts: vec![sourced(
                "fact_1",
                "event_1",
                "episode_1",
                "Lena",
                "owns",
                "Roadmap",
                1,
                0.8,
            )],
            ..MemoryData::default()
        };

        let health = KnowledgeHealth::inspect_at(&data, now);

        assert_eq!(health.stale_fact_count, 1);
        assert_eq!(health.superseded_fact_count, 0);
        assert!(
            !health
                .signals
                .iter()
                .any(|signal| signal.kind == HealthSignalKind::SupersededFact)
        );
    }

    #[test]
    fn unprovenanced_newer_value_stays_conflict() {
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
        assert_eq!(health.superseded_fact_count, 0);
        assert!(health.signals.iter().any(|signal| {
            signal.kind == HealthSignalKind::ConflictingFact
                && signal.severity == HealthSeverity::High
        }));
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

    #[allow(clippy::too_many_arguments)]
    fn sourced(
        id: &str,
        event_id: &str,
        source_episode_id: &str,
        subject: &str,
        predicate: &str,
        object: &str,
        created_at_ms: u64,
        confidence: f32,
    ) -> Fact {
        Fact {
            source_episode_id: Some(source_episode_id.to_string()),
            ..fact(
                id,
                event_id,
                subject,
                predicate,
                object,
                created_at_ms,
                confidence,
            )
        }
    }
}
