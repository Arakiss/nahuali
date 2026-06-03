use crate::inspection::{HealthSeverity, HealthSignal, HealthSignalKind, KnowledgeHealth};
use crate::model::{
    Claim, IntentionStatus, Link, MemoryData, MemoryKind, MemoryScope, RecallResult,
    RecallResultTrust, RecallResultTrustMode,
};
use serde::{Deserialize, Serialize};

/// Options for lexical recall over projected memory.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct RecallOptions {
    /// Maximum number of results returned after filtering and scoring.
    pub limit: usize,
    /// Optional exact memory scope boundary.
    pub scope: Option<MemoryScope>,
    /// Optional memory kinds to include. Empty means all kinds.
    pub kinds: Vec<MemoryKind>,
    /// Require a concrete evidence identifier on every returned result.
    pub require_evidence: bool,
}

impl Default for RecallOptions {
    fn default() -> Self {
        Self {
            limit: 10,
            scope: None,
            kinds: Vec::new(),
            require_evidence: false,
        }
    }
}

pub(crate) fn recall(data: &MemoryData, query: &str, limit: usize) -> Vec<RecallResult> {
    recall_with_options(
        data,
        query,
        RecallOptions {
            limit,
            ..RecallOptions::default()
        },
    )
}

#[cfg(test)]
fn recall_scoped(
    data: &MemoryData,
    query: &str,
    limit: usize,
    scope: &MemoryScope,
) -> Vec<RecallResult> {
    recall_with_options(
        data,
        query,
        RecallOptions {
            limit,
            scope: Some(scope.clone()),
            ..RecallOptions::default()
        },
    )
}

pub(crate) fn recall_with_options(
    data: &MemoryData,
    query: &str,
    options: RecallOptions,
) -> Vec<RecallResult> {
    let terms = tokenize(query);
    if terms.is_empty() {
        return Vec::new();
    }

    let mut results = Vec::new();

    for entity in &data.entities {
        if !scope_matches(entity.scope.as_ref(), options.scope.as_ref()) {
            continue;
        }
        let haystack = entity.name.clone();
        let matched_terms = matched_terms(&haystack, &terms);
        if !is_relevant_match(&matched_terms, &terms) {
            continue;
        }

        push_result(
            &mut results,
            RecallResult {
                kind: MemoryKind::Entity,
                id: entity.id.clone(),
                score: score(&haystack, &matched_terms) + entity.mention_count as f32 * 0.05,
                excerpt: entity.name.clone(),
                evidence_id: entity.source_event_ids.first().cloned(),
                matched_terms,
                scope: entity.scope.clone(),
                trust: None,
            },
            &options,
        );
    }

    for episode in &data.episodes {
        if !scope_matches(episode.scope.as_ref(), options.scope.as_ref()) {
            continue;
        }
        let haystack = format!("{} {}", episode.content, episode.tags.join(" "));
        let matched_terms = matched_terms(&haystack, &terms);
        if !is_relevant_match(&matched_terms, &terms) {
            continue;
        }

        push_result(
            &mut results,
            RecallResult {
                kind: MemoryKind::Episode,
                id: episode.id.clone(),
                score: score(&haystack, &matched_terms),
                excerpt: episode.content.clone(),
                evidence_id: Some(episode.id.clone()),
                matched_terms,
                scope: episode.scope.clone(),
                trust: None,
            },
            &options,
        );
    }

    for claim in projected_claims(data) {
        if !scope_matches(claim.scope.as_ref(), options.scope.as_ref()) {
            continue;
        }
        let haystack = format!("{} {} {}", claim.subject, claim.predicate, claim.object);
        let matched_terms = matched_terms(&haystack, &terms);
        if !is_relevant_match(&matched_terms, &terms) {
            continue;
        }

        let evidence_bonus = if claim.source_episode_id.is_some() {
            0.25
        } else {
            0.0
        };

        push_result(
            &mut results,
            RecallResult {
                kind: MemoryKind::Claim,
                id: claim.id.clone(),
                score: score(&haystack, &matched_terms) + claim.confidence + evidence_bonus,
                excerpt: format!("{} {} {}", claim.subject, claim.predicate, claim.object),
                evidence_id: claim.source_episode_id.clone(),
                matched_terms,
                scope: claim.scope.clone(),
                trust: None,
            },
            &options,
        );
    }

    for link in projected_links(data) {
        if !scope_matches(link.scope.as_ref(), options.scope.as_ref()) {
            continue;
        }
        let haystack = format!("{} {} {}", link.from, link.relation, link.to);
        let matched_terms = matched_terms(&haystack, &terms);
        if !is_relevant_match(&matched_terms, &terms) {
            continue;
        }

        let evidence_bonus = if link.source_episode_id.is_some() {
            0.25
        } else {
            0.0
        };

        push_result(
            &mut results,
            RecallResult {
                kind: MemoryKind::Link,
                id: link.id.clone(),
                score: score(&haystack, &matched_terms) + link.confidence + evidence_bonus,
                excerpt: format!("{} {} {}", link.from, link.relation, link.to),
                evidence_id: link.source_episode_id.clone(),
                matched_terms,
                scope: link.scope.clone(),
                trust: None,
            },
            &options,
        );
    }

    for procedure in &data.procedures {
        if !scope_matches(procedure.scope.as_ref(), options.scope.as_ref()) {
            continue;
        }
        let haystack = format!("{:?} {} {}", procedure.kind, procedure.name, procedure.body);
        let matched_terms = matched_terms(&haystack, &terms);
        if !is_relevant_match(&matched_terms, &terms) {
            continue;
        }

        let evidence_bonus = if procedure.source_episode_id.is_some() {
            0.25
        } else {
            0.0
        };

        push_result(
            &mut results,
            RecallResult {
                kind: MemoryKind::Procedure,
                id: procedure.id.clone(),
                score: score(&haystack, &matched_terms) + procedure.confidence + evidence_bonus,
                excerpt: format!("{}: {}", procedure.name, procedure.body),
                evidence_id: procedure.source_episode_id.clone(),
                matched_terms,
                scope: procedure.scope.clone(),
                trust: None,
            },
            &options,
        );
    }

    for intention in &data.intentions {
        if !scope_matches(intention.scope.as_ref(), options.scope.as_ref()) {
            continue;
        }
        let haystack = format!(
            "{:?} {:?} {:?} {}",
            intention.kind, intention.status, intention.priority, intention.description
        );
        let matched_terms = matched_terms(&haystack, &terms);
        if !is_relevant_match(&matched_terms, &terms) {
            continue;
        }

        let status_bonus = match intention.status {
            IntentionStatus::Active | IntentionStatus::Blocked => 0.5,
            IntentionStatus::Deferred => 0.25,
            IntentionStatus::Completed | IntentionStatus::Abandoned => 0.0,
        };

        push_result(
            &mut results,
            RecallResult {
                kind: MemoryKind::Intention,
                id: intention.id.clone(),
                score: score(&haystack, &matched_terms) + status_bonus,
                excerpt: intention.description.clone(),
                evidence_id: intention.source_episode_id.clone(),
                matched_terms,
                scope: intention.scope.clone(),
                trust: None,
            },
            &options,
        );
    }

    results.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.id.cmp(&right.id))
    });
    results.truncate(options.limit.max(1));
    results
}

pub(crate) fn attach_result_trust(
    data: &MemoryData,
    health: &KnowledgeHealth,
    results: &mut [RecallResult],
) {
    for result in results {
        result.trust = Some(result_trust(data, health, result));
    }
}

fn result_trust(
    data: &MemoryData,
    health: &KnowledgeHealth,
    result: &RecallResult,
) -> RecallResultTrust {
    let source_event_id = source_event_id_for_result(data, result);
    let local_signals = source_event_id
        .as_deref()
        .map(|event_id| {
            health
                .signals
                .iter()
                .filter(|signal| signal.evidence_ids.iter().any(|id| id == event_id))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if !local_signals.is_empty() {
        return trust_from_signals(local_signals);
    }

    match result.kind {
        MemoryKind::Episode => RecallResultTrust {
            mode: RecallResultTrustMode::Certify,
            score: 1.0,
            can_trust: true,
            reasons: vec!["Result is an observed episode and can cite itself as evidence.".into()],
            signal_kinds: Vec::new(),
        },
        MemoryKind::Claim | MemoryKind::Link | MemoryKind::Fact | MemoryKind::Relation => {
            if result.evidence_id.is_some() {
                RecallResultTrust {
                    mode: RecallResultTrustMode::Certify,
                    score: 1.0,
                    can_trust: true,
                    reasons: vec!["Result has source episode evidence.".into()],
                    signal_kinds: Vec::new(),
                }
            } else {
                RecallResultTrust {
                    mode: RecallResultTrustMode::Warn,
                    score: 0.5,
                    can_trust: false,
                    reasons: vec!["Result has no source episode evidence.".into()],
                    signal_kinds: vec![signal_kind_name(&HealthSignalKind::UnsupportedFact).into()],
                }
            }
        }
        MemoryKind::Procedure | MemoryKind::Intention => {
            if result.evidence_id.is_some() {
                RecallResultTrust {
                    mode: RecallResultTrustMode::Certify,
                    score: 1.0,
                    can_trust: true,
                    reasons: vec!["Result has source episode evidence.".into()],
                    signal_kinds: Vec::new(),
                }
            } else {
                RecallResultTrust {
                    mode: RecallResultTrustMode::Advisory,
                    score: 0.75,
                    can_trust: false,
                    reasons: vec![
                        "Result is actionable memory but has no source episode evidence.".into(),
                    ],
                    signal_kinds: Vec::new(),
                }
            }
        }
        MemoryKind::Entity => RecallResultTrust {
            mode: RecallResultTrustMode::Advisory,
            score: 0.75,
            can_trust: false,
            reasons: vec!["Result is an observed entity, not an evidence-backed assertion.".into()],
            signal_kinds: Vec::new(),
        },
    }
}

fn trust_from_signals(signals: Vec<&HealthSignal>) -> RecallResultTrust {
    let highest = signals
        .iter()
        .map(|signal| &signal.severity)
        .max_by_key(|severity| severity_rank(severity))
        .expect("signals are non-empty");
    let mode = match highest {
        HealthSeverity::High => RecallResultTrustMode::Block,
        HealthSeverity::Medium => RecallResultTrustMode::Warn,
        HealthSeverity::Low => RecallResultTrustMode::Advisory,
    };
    let score = match mode {
        RecallResultTrustMode::Certify => 1.0,
        RecallResultTrustMode::Advisory => 0.75,
        RecallResultTrustMode::Warn => 0.5,
        RecallResultTrustMode::Block => 0.0,
    };
    let mut signal_kinds = Vec::new();
    for signal in &signals {
        let kind = signal_kind_name(&signal.kind);
        if !signal_kinds.iter().any(|existing| existing == kind) {
            signal_kinds.push(kind.to_string());
        }
    }

    RecallResultTrust {
        can_trust: matches!(mode, RecallResultTrustMode::Certify),
        mode,
        score,
        reasons: signals
            .into_iter()
            .map(|signal| signal.message.clone())
            .collect(),
        signal_kinds,
    }
}

fn severity_rank(severity: &HealthSeverity) -> u8 {
    match severity {
        HealthSeverity::Low => 1,
        HealthSeverity::Medium => 2,
        HealthSeverity::High => 3,
    }
}

fn source_event_id_for_result(data: &MemoryData, result: &RecallResult) -> Option<String> {
    match result.kind {
        MemoryKind::Entity => data
            .entities
            .iter()
            .find(|entity| entity.id == result.id)
            .and_then(|entity| entity.source_event_ids.first().cloned()),
        MemoryKind::Episode => data
            .episodes
            .iter()
            .find(|episode| episode.id == result.id)
            .map(|episode| episode.event_id.clone()),
        MemoryKind::Claim | MemoryKind::Fact => projected_claims(data)
            .iter()
            .find(|claim| claim.id == result.id)
            .map(|claim| claim.event_id.clone()),
        MemoryKind::Link | MemoryKind::Relation => projected_links(data)
            .iter()
            .find(|link| link.id == result.id)
            .map(|link| link.event_id.clone()),
        MemoryKind::Procedure => data
            .procedures
            .iter()
            .find(|procedure| procedure.id == result.id)
            .map(|procedure| procedure.event_id.clone()),
        MemoryKind::Intention => data
            .intentions
            .iter()
            .find(|intention| intention.id == result.id)
            .map(|intention| intention.event_id.clone()),
    }
}

fn signal_kind_name(kind: &HealthSignalKind) -> &'static str {
    match kind {
        HealthSignalKind::NoEpisodes => "no_episodes",
        HealthSignalKind::UnsupportedFact => "unsupported_fact",
        HealthSignalKind::LowConfidenceFact => "low_confidence_fact",
        HealthSignalKind::ConflictingFact => "conflicting_fact",
        HealthSignalKind::StaleFact => "stale_fact",
        HealthSignalKind::SupersededFact => "superseded_fact",
        HealthSignalKind::IsolatedEntity => "isolated_entity",
    }
}

fn push_result(results: &mut Vec<RecallResult>, result: RecallResult, options: &RecallOptions) {
    if !kind_allowed(&result.kind, &options.kinds) {
        return;
    }
    if options.require_evidence && result.evidence_id.is_none() {
        return;
    }
    results.push(result);
}

fn kind_allowed(kind: &MemoryKind, allowed: &[MemoryKind]) -> bool {
    allowed.is_empty()
        || allowed
            .iter()
            .any(|allowed_kind| memory_kind_matches(kind, allowed_kind))
}

fn memory_kind_matches(kind: &MemoryKind, allowed: &MemoryKind) -> bool {
    kind == allowed
        || matches!((kind, allowed), (MemoryKind::Claim, MemoryKind::Fact))
        || matches!((kind, allowed), (MemoryKind::Link, MemoryKind::Relation))
}

fn scope_matches(item_scope: Option<&MemoryScope>, filter: Option<&MemoryScope>) -> bool {
    match filter {
        Some(filter) => item_scope.is_some_and(|item_scope| item_scope.key == filter.key),
        None => true,
    }
}

fn projected_claims(data: &MemoryData) -> &[Claim] {
    if data.claims.is_empty() {
        &data.facts
    } else {
        &data.claims
    }
}

fn projected_links(data: &MemoryData) -> &[Link] {
    if data.links.is_empty() {
        &data.relations
    } else {
        &data.links
    }
}

fn score(haystack: &str, matched_terms: &[String]) -> f32 {
    let normalized = normalize(haystack);
    let density = matched_terms.len() as f32 / tokenize(&normalized).len().max(1) as f32;
    matched_terms.len() as f32 + density
}

fn matched_terms(haystack: &str, terms: &[String]) -> Vec<String> {
    let haystack_terms = tokenize(haystack);
    terms
        .iter()
        .filter(|term| {
            haystack_terms
                .iter()
                .any(|haystack_term| haystack_term == *term)
        })
        .cloned()
        .collect()
}

fn is_relevant_match(matched_terms: &[String], query_terms: &[String]) -> bool {
    if matched_terms.is_empty() {
        return false;
    }

    let significant_query_terms = query_terms
        .iter()
        .filter(|term| is_significant_query_term(term));

    let mut has_significant_query_term = false;
    for term in significant_query_terms {
        has_significant_query_term = true;
        if matched_terms.iter().any(|matched| matched == term) {
            return true;
        }
    }

    !has_significant_query_term
}

fn is_significant_query_term(term: &str) -> bool {
    !matches!(
        term,
        "a" | "an"
            | "and"
            | "are"
            | "be"
            | "been"
            | "being"
            | "by"
            | "did"
            | "do"
            | "does"
            | "for"
            | "from"
            | "has"
            | "have"
            | "how"
            | "in"
            | "is"
            | "of"
            | "on"
            | "owned"
            | "owner"
            | "owns"
            | "the"
            | "to"
            | "was"
            | "were"
            | "what"
            | "when"
            | "where"
            | "who"
            | "why"
            | "with"
    )
}

fn tokenize(input: &str) -> Vec<String> {
    normalize(input)
        .split_whitespace()
        .filter(|term| term.len() > 1)
        .map(ToOwned::to_owned)
        .collect()
}

fn normalize(input: &str) -> String {
    input
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::model::{
        Claim, Episode, Link, MemoryData, MemoryKind, MemoryScope, MemoryScopeKind,
    };

    use super::{RecallOptions, recall, recall_scoped, recall_with_options};

    #[test]
    fn returns_episode_matches() {
        let data = MemoryData {
            event_count: 1,
            last_event_id: Some("event_1".to_string()),
            episodes: vec![Episode {
                id: "episode_1".to_string(),
                event_id: "event_1".to_string(),
                content: "Lena prefers concise release notes.".to_string(),
                tags: vec!["product".to_string()],
                mentions: Vec::new(),
                source_id: None,
                source_position: None,
                source_role: None,
                scope: None,
                created_at_ms: 1,
            }],
            ..MemoryData::default()
        };

        let results = recall(&data, "release notes", 10);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, MemoryKind::Episode);
        assert_eq!(results[0].evidence_id.as_deref(), Some("episode_1"));
    }

    #[test]
    fn gives_supported_claims_a_bonus() {
        let data = MemoryData {
            event_count: 2,
            last_event_id: Some("event_2".to_string()),
            claims: vec![
                Claim {
                    id: "claim_unsupported".to_string(),
                    event_id: "event_1".to_string(),
                    subject: "Lena".to_string(),
                    predicate: "prefers".to_string(),
                    object: "release notes".to_string(),
                    source_episode_id: None,
                    confidence: 0.8,
                    scope: None,
                    created_at_ms: 1,
                },
                Claim {
                    id: "claim_supported".to_string(),
                    event_id: "event_2".to_string(),
                    subject: "Lena".to_string(),
                    predicate: "prefers".to_string(),
                    object: "release notes".to_string(),
                    source_episode_id: Some("episode_1".to_string()),
                    confidence: 0.8,
                    scope: None,
                    created_at_ms: 1,
                },
            ],
            ..MemoryData::default()
        };

        let results = recall(&data, "release notes", 10);

        assert_eq!(results[0].kind, MemoryKind::Claim);
        assert_eq!(results[0].id, "claim_supported");
    }

    #[test]
    fn returns_canonical_link_kind() {
        let data = MemoryData {
            event_count: 1,
            last_event_id: Some("event_1".to_string()),
            links: vec![Link {
                id: "link_1".to_string(),
                event_id: "event_1".to_string(),
                from: "Lena".to_string(),
                relation: "owns".to_string(),
                to: "release notes".to_string(),
                source_episode_id: Some("episode_1".to_string()),
                confidence: 0.9,
                scope: None,
                created_at_ms: 1,
            }],
            ..MemoryData::default()
        };

        let results = recall(&data, "release notes", 10);

        assert_eq!(results[0].kind, MemoryKind::Link);
        assert_eq!(results[0].evidence_id.as_deref(), Some("episode_1"));
    }

    #[test]
    fn scoped_recall_only_returns_matching_scope() {
        let project_scope = MemoryScope::new(MemoryScopeKind::Project, "Nahuali").unwrap();
        let other_scope = MemoryScope::new(MemoryScopeKind::Project, "Other").unwrap();
        let data = MemoryData {
            event_count: 2,
            last_event_id: Some("event_2".to_string()),
            episodes: vec![
                Episode {
                    id: "episode_project".to_string(),
                    event_id: "event_1".to_string(),
                    content: "Lena owns release notes.".to_string(),
                    tags: Vec::new(),
                    mentions: Vec::new(),
                    source_id: None,
                    source_position: None,
                    source_role: None,
                    scope: Some(project_scope.clone()),
                    created_at_ms: 1,
                },
                Episode {
                    id: "episode_other".to_string(),
                    event_id: "event_2".to_string(),
                    content: "Lena owns release notes.".to_string(),
                    tags: Vec::new(),
                    mentions: Vec::new(),
                    source_id: None,
                    source_position: None,
                    source_role: None,
                    scope: Some(other_scope),
                    created_at_ms: 2,
                },
            ],
            ..MemoryData::default()
        };

        let results = recall_scoped(&data, "release notes", 10, &project_scope);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "episode_project");
        assert_eq!(
            results[0].scope.as_ref().map(|scope| &scope.key),
            Some(&project_scope.key)
        );
    }

    #[test]
    fn filters_recall_by_kind_and_evidence() {
        let data = MemoryData {
            event_count: 3,
            last_event_id: Some("event_3".to_string()),
            episodes: vec![Episode {
                id: "episode_1".to_string(),
                event_id: "event_1".to_string(),
                content: "Lena owns release notes.".to_string(),
                tags: Vec::new(),
                mentions: Vec::new(),
                source_id: None,
                source_position: None,
                source_role: None,
                scope: None,
                created_at_ms: 1,
            }],
            claims: vec![
                Claim {
                    id: "claim_unsupported".to_string(),
                    event_id: "event_2".to_string(),
                    subject: "Lena".to_string(),
                    predicate: "owns".to_string(),
                    object: "release notes".to_string(),
                    source_episode_id: None,
                    confidence: 0.8,
                    scope: None,
                    created_at_ms: 2,
                },
                Claim {
                    id: "claim_supported".to_string(),
                    event_id: "event_3".to_string(),
                    subject: "Lena".to_string(),
                    predicate: "owns".to_string(),
                    object: "release notes".to_string(),
                    source_episode_id: Some("episode_1".to_string()),
                    confidence: 0.8,
                    scope: None,
                    created_at_ms: 3,
                },
            ],
            ..MemoryData::default()
        };

        let results = recall_with_options(
            &data,
            "release notes",
            RecallOptions {
                limit: 10,
                kinds: vec![MemoryKind::Claim],
                require_evidence: true,
                ..RecallOptions::default()
            },
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "claim_supported");
        assert_eq!(results[0].kind, MemoryKind::Claim);
        assert_eq!(results[0].evidence_id.as_deref(), Some("episode_1"));
    }

    #[test]
    fn fact_and_relation_filters_match_canonical_kinds() {
        let data = MemoryData {
            event_count: 1,
            last_event_id: Some("event_1".to_string()),
            links: vec![Link {
                id: "link_1".to_string(),
                event_id: "event_1".to_string(),
                from: "Lena".to_string(),
                relation: "owns".to_string(),
                to: "release notes".to_string(),
                source_episode_id: Some("episode_1".to_string()),
                confidence: 0.9,
                scope: None,
                created_at_ms: 1,
            }],
            ..MemoryData::default()
        };

        let results = recall_with_options(
            &data,
            "release notes",
            RecallOptions {
                limit: 10,
                kinds: vec![MemoryKind::Relation],
                ..RecallOptions::default()
            },
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, MemoryKind::Link);
    }

    #[test]
    fn ignores_generic_relation_only_matches_when_query_has_specific_terms() {
        let data = MemoryData {
            event_count: 2,
            last_event_id: Some("event_2".to_string()),
            episodes: vec![Episode {
                id: "episode_1".to_string(),
                event_id: "event_1".to_string(),
                content: "Lena owns release notes.".to_string(),
                tags: Vec::new(),
                mentions: Vec::new(),
                source_id: None,
                source_position: None,
                source_role: None,
                scope: None,
                created_at_ms: 1,
            }],
            claims: vec![Claim {
                id: "claim_1".to_string(),
                event_id: "event_2".to_string(),
                subject: "Lena".to_string(),
                predicate: "owns".to_string(),
                object: "release notes".to_string(),
                source_episode_id: Some("episode_1".to_string()),
                confidence: 0.9,
                scope: None,
                created_at_ms: 2,
            }],
            ..MemoryData::default()
        };

        let results = recall_with_options(
            &data,
            "who owns deployment keys",
            RecallOptions {
                limit: 10,
                kinds: vec![MemoryKind::Claim],
                require_evidence: true,
                ..RecallOptions::default()
            },
        );

        assert!(results.is_empty());
    }
}
