use std::collections::BTreeMap;

use crate::inspection::{
    HealthSeverity, HealthSignal, HealthSignalKind, KnowledgeHealth, claim_lifecycles,
};
use crate::model::{
    Claim, ClaimLifecycle, IntentionStatus, Link, MemoryData, MemoryKind, MemoryScope,
    RecallResult, RecallResultTrust, RecallResultTrustMode, RecallScoreBreakdown,
    admitted_confidence,
};
use crate::{AuthorityDecision, AuthorityRecall, NahualiError, Result};
use serde::{Deserialize, Serialize};

/// Options for lexical recall over projected memory.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RecallOptions {
    /// Maximum number of results returned after filtering and scoring.
    pub limit: usize,
    /// Optional exact memory scope boundary.
    pub scope: Option<MemoryScope>,
    /// Optional memory kinds to include. Empty means all kinds.
    pub kinds: Vec<MemoryKind>,
    /// Require a concrete evidence identifier on every returned result.
    pub require_evidence: bool,
    /// Optional inclusive upper bound on memory age: only return memory created
    /// at or before this millisecond timestamp. The point-in-time / "as of date
    /// T" filter — recall what was known as of a checkpoint, for audit replay.
    pub as_of_ms: Option<u64>,
    /// Optional inclusive lower bound on memory age: only return memory created
    /// at or after this millisecond timestamp. Set to `now - threshold` to
    /// exclude stale memory at query time instead of only flagging it.
    pub since_ms: Option<u64>,
    /// Drop claims whose lifecycle is `superseded`. By default they are
    /// returned, marked, and ranked after every non-superseded result.
    #[serde(default)]
    pub exclude_superseded: bool,
    /// Weights that turn relevance, confidence, evidence, and recency into one
    /// deterministic score.
    #[serde(default)]
    pub ranking: RecallRankingPolicy,
}

impl Default for RecallOptions {
    fn default() -> Self {
        Self {
            limit: 10,
            scope: None,
            kinds: Vec::new(),
            require_evidence: false,
            as_of_ms: None,
            since_ms: None,
            exclude_superseded: false,
            ranking: RecallRankingPolicy::default(),
        }
    }
}

/// Default recency half-life: 30 days.
pub const DEFAULT_RECENCY_HALF_LIFE_MS: u64 = 30 * 24 * 60 * 60 * 1000;

/// Tunable, deterministic recall ranking policy.
///
/// A result's score is `relevance + confidence + evidence + recency`:
///
/// - `relevance`: matched query terms plus match density (plus the entity
///   mention bonus or the intention status bonus).
/// - `confidence`: `confidence_weight × recorded confidence` for claims, links,
///   and procedures.
/// - `evidence`: `evidence_bonus` when a claim, link, or procedure cites a
///   source episode.
/// - `recency`: `recency_weight × 0.5^(age / recency_half_life_ms)`, where age
///   is measured from the newest matching result, not the wall clock, so the
///   same store and query always rank the same way.
///
/// With the default `recency_weight` below `1.0`, recency reorders results that
/// match the same number of query terms but never outranks an additional
/// matched term. Ties break by newer timestamp, then by id. Superseded claims
/// are ranked after every non-superseded result regardless of score.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct RecallRankingPolicy {
    /// Multiplier applied to recorded confidence.
    pub confidence_weight: f32,
    /// Bonus for results that cite a source episode.
    pub evidence_bonus: f32,
    /// Maximum recency contribution, earned by the newest matching result.
    pub recency_weight: f32,
    /// Age, relative to the newest matching result, at which the recency
    /// contribution halves. Must be greater than zero.
    pub recency_half_life_ms: u64,
}

impl Default for RecallRankingPolicy {
    fn default() -> Self {
        Self {
            confidence_weight: 1.0,
            evidence_bonus: 0.25,
            recency_weight: 0.5,
            recency_half_life_ms: DEFAULT_RECENCY_HALF_LIFE_MS,
        }
    }
}

impl RecallRankingPolicy {
    /// Reject weights that are negative or not finite and a zero half-life.
    pub fn validate(&self) -> Result<()> {
        for (name, value) in [
            ("confidence_weight", self.confidence_weight),
            ("evidence_bonus", self.evidence_bonus),
            ("recency_weight", self.recency_weight),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(NahualiError::InvalidRecallRanking {
                    message: format!("{name} must be a finite number >= 0 (got {value})"),
                });
            }
        }
        if self.recency_half_life_ms == 0 {
            return Err(NahualiError::InvalidRecallRanking {
                message: "recency_half_life_ms must be greater than zero".to_string(),
            });
        }
        Ok(())
    }

    pub(crate) fn recency(&self, age_ms: u64) -> f32 {
        let half_lives = age_ms as f64 / self.recency_half_life_ms as f64;
        (f64::from(self.recency_weight) * 0.5_f64.powf(half_lives)) as f32
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

/// Scoring inputs for one candidate before the recency reference is known.
pub(crate) struct Candidate {
    pub(crate) relevance: f32,
    pub(crate) confidence: Option<f32>,
    pub(crate) has_evidence: bool,
    pub(crate) created_at_ms: u64,
    pub(crate) updated_at_ms: Option<u64>,
    pub(crate) lifecycle: Option<ClaimLifecycle>,
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

    let lifecycles = claim_lifecycles(data);
    let episode_times = data
        .episodes
        .iter()
        .map(|episode| (episode.id.as_str(), episode.created_at_ms))
        .collect::<BTreeMap<_, _>>();
    let mut results = Vec::new();

    for entity in &data.entities {
        if !scope_matches(entity.scope.as_ref(), options.scope.as_ref()) {
            continue;
        }
        if !within_time_window(entity.last_seen_at_ms, &options) {
            continue;
        }
        let haystack = entity.name.clone();
        let matched_terms = matched_terms(&haystack, &terms);
        if !is_relevant_match(&matched_terms, &terms) {
            continue;
        }

        push_result(
            &mut results,
            Candidate {
                relevance: score(&haystack, &matched_terms) + entity.mention_count as f32 * 0.05,
                confidence: None,
                has_evidence: false,
                created_at_ms: entity.first_seen_at_ms,
                updated_at_ms: Some(entity.last_seen_at_ms),
                lifecycle: None,
            },
            RecallResult {
                kind: MemoryKind::Entity,
                id: entity.id.clone(),
                score: 0.0,
                excerpt: entity.name.clone(),
                evidence_id: entity.source_event_ids.first().cloned(),
                matched_terms,
                scope: entity.scope.clone(),
                trust: None,
                created_at_ms: None,
                updated_at_ms: None,
                observed_at_ms: None,
                confidence: None,
                lifecycle: None,
                score_breakdown: None,
            },
            &options,
        );
    }

    for episode in &data.episodes {
        if !scope_matches(episode.scope.as_ref(), options.scope.as_ref()) {
            continue;
        }
        if !within_time_window(episode.created_at_ms, &options) {
            continue;
        }
        let haystack = format!("{} {}", episode.content, episode.tags.join(" "));
        let matched_terms = matched_terms(&haystack, &terms);
        if !is_relevant_match(&matched_terms, &terms) {
            continue;
        }

        push_result(
            &mut results,
            Candidate {
                relevance: score(&haystack, &matched_terms),
                confidence: None,
                has_evidence: false,
                created_at_ms: episode.created_at_ms,
                updated_at_ms: None,
                lifecycle: None,
            },
            RecallResult {
                kind: MemoryKind::Episode,
                id: episode.id.clone(),
                score: 0.0,
                excerpt: episode.content.clone(),
                evidence_id: Some(episode.id.clone()),
                matched_terms,
                scope: episode.scope.clone(),
                observed_at_ms: Some(episode.created_at_ms),
                trust: None,
                created_at_ms: None,
                updated_at_ms: None,
                confidence: None,
                lifecycle: None,
                score_breakdown: None,
            },
            &options,
        );
    }

    for claim in projected_claims(data) {
        if !scope_matches(claim.scope.as_ref(), options.scope.as_ref()) {
            continue;
        }
        if !within_time_window(claim.created_at_ms, &options) {
            continue;
        }
        let haystack = format!("{} {} {}", claim.subject, claim.predicate, claim.object);
        let matched_terms = matched_terms(&haystack, &terms);
        if !is_relevant_match(&matched_terms, &terms) {
            continue;
        }

        push_result(
            &mut results,
            Candidate {
                relevance: score(&haystack, &matched_terms),
                confidence: Some(claim.confidence),
                has_evidence: claim.source_episode_id.is_some(),
                created_at_ms: claim.created_at_ms,
                updated_at_ms: None,
                lifecycle: Some(
                    lifecycles
                        .get(&claim.id)
                        .cloned()
                        .unwrap_or_else(ClaimLifecycle::current),
                ),
            },
            RecallResult {
                kind: MemoryKind::Claim,
                id: claim.id.clone(),
                score: 0.0,
                excerpt: format!("{} {} {}", claim.subject, claim.predicate, claim.object),
                evidence_id: claim.source_episode_id.clone(),
                matched_terms,
                scope: claim.scope.clone(),
                observed_at_ms: observed_at(&episode_times, claim.source_episode_id.as_deref()),
                trust: None,
                created_at_ms: None,
                updated_at_ms: None,
                confidence: None,
                lifecycle: None,
                score_breakdown: None,
            },
            &options,
        );
    }

    for link in projected_links(data) {
        if !scope_matches(link.scope.as_ref(), options.scope.as_ref()) {
            continue;
        }
        if !within_time_window(link.created_at_ms, &options) {
            continue;
        }
        let haystack = format!("{} {} {}", link.from, link.relation, link.to);
        let matched_terms = matched_terms(&haystack, &terms);
        if !is_relevant_match(&matched_terms, &terms) {
            continue;
        }

        push_result(
            &mut results,
            Candidate {
                relevance: score(&haystack, &matched_terms),
                confidence: Some(link.confidence),
                has_evidence: link.source_episode_id.is_some(),
                created_at_ms: link.created_at_ms,
                updated_at_ms: None,
                lifecycle: None,
            },
            RecallResult {
                kind: MemoryKind::Link,
                id: link.id.clone(),
                score: 0.0,
                excerpt: format!("{} {} {}", link.from, link.relation, link.to),
                evidence_id: link.source_episode_id.clone(),
                matched_terms,
                scope: link.scope.clone(),
                observed_at_ms: observed_at(&episode_times, link.source_episode_id.as_deref()),
                trust: None,
                created_at_ms: None,
                updated_at_ms: None,
                confidence: None,
                lifecycle: None,
                score_breakdown: None,
            },
            &options,
        );
    }

    for procedure in &data.procedures {
        if !scope_matches(procedure.scope.as_ref(), options.scope.as_ref()) {
            continue;
        }
        if !within_time_window(procedure.created_at_ms, &options) {
            continue;
        }
        let haystack = format!("{:?} {} {}", procedure.kind, procedure.name, procedure.body);
        let matched_terms = matched_terms(&haystack, &terms);
        if !is_relevant_match(&matched_terms, &terms) {
            continue;
        }

        push_result(
            &mut results,
            Candidate {
                relevance: score(&haystack, &matched_terms),
                confidence: Some(procedure.confidence),
                has_evidence: procedure.source_episode_id.is_some(),
                created_at_ms: procedure.created_at_ms,
                updated_at_ms: None,
                lifecycle: None,
            },
            RecallResult {
                kind: MemoryKind::Procedure,
                id: procedure.id.clone(),
                score: 0.0,
                excerpt: format!("{}: {}", procedure.name, procedure.body),
                evidence_id: procedure.source_episode_id.clone(),
                matched_terms,
                scope: procedure.scope.clone(),
                observed_at_ms: observed_at(&episode_times, procedure.source_episode_id.as_deref()),
                trust: None,
                created_at_ms: None,
                updated_at_ms: None,
                confidence: None,
                lifecycle: None,
                score_breakdown: None,
            },
            &options,
        );
    }

    for intention in &data.intentions {
        if !scope_matches(intention.scope.as_ref(), options.scope.as_ref()) {
            continue;
        }
        if !within_time_window(intention.created_at_ms, &options) {
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
            Candidate {
                relevance: score(&haystack, &matched_terms) + status_bonus,
                confidence: None,
                has_evidence: false,
                created_at_ms: intention.created_at_ms,
                updated_at_ms: Some(intention.updated_at_ms),
                lifecycle: None,
            },
            RecallResult {
                kind: MemoryKind::Intention,
                id: intention.id.clone(),
                score: 0.0,
                excerpt: intention.description.clone(),
                evidence_id: intention.source_episode_id.clone(),
                matched_terms,
                scope: intention.scope.clone(),
                observed_at_ms: observed_at(&episode_times, intention.source_episode_id.as_deref()),
                trust: None,
                created_at_ms: None,
                updated_at_ms: None,
                confidence: None,
                lifecycle: None,
                score_breakdown: None,
            },
            &options,
        );
    }

    let mut results = rank(results, &options.ranking);
    results.truncate(options.limit.max(1));
    results
}

/// Turn candidates into scored results and order them: non-superseded before
/// superseded, then score, then newer timestamp, then id.
pub(crate) fn rank(
    candidates: Vec<(Candidate, RecallResult)>,
    policy: &RecallRankingPolicy,
) -> Vec<RecallResult> {
    let reference_ms = candidates
        .iter()
        .map(|(candidate, _)| recency_timestamp(candidate))
        .max()
        .unwrap_or_default();
    let mut ranked = candidates
        .into_iter()
        .map(|(candidate, mut result)| {
            let recency =
                policy.recency(reference_ms.saturating_sub(recency_timestamp(&candidate)));
            let breakdown = score_breakdown(policy, &candidate, recency);
            result.score = breakdown.total();
            result.score_breakdown = Some(breakdown);
            result.confidence = candidate.confidence;
            result.created_at_ms = Some(candidate.created_at_ms);
            result.updated_at_ms = candidate
                .updated_at_ms
                .filter(|updated| *updated != candidate.created_at_ms);
            result.lifecycle = candidate.lifecycle;
            (recency_timestamp_of(&result), result)
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|(left_ts, left), (right_ts, right)| {
        is_superseded(left)
            .cmp(&is_superseded(right))
            .then_with(|| right.score.total_cmp(&left.score))
            .then_with(|| right_ts.cmp(left_ts))
            .then_with(|| left.id.cmp(&right.id))
    });
    ranked.into_iter().map(|(_, result)| result).collect()
}

/// Combine one candidate's relevance, confidence, and evidence with its
/// already-decayed `recency` contribution.
///
/// A confidence that the ledger would not admit (NaN or outside `0.0..=1.0`,
/// reachable through caller-supplied projections) ranks as its admitted value,
/// or `0.0` for NaN, so one malformed item cannot outrank or destabilize the rest.
pub(crate) fn score_breakdown(
    policy: &RecallRankingPolicy,
    candidate: &Candidate,
    recency: f32,
) -> RecallScoreBreakdown {
    RecallScoreBreakdown {
        relevance: candidate.relevance,
        confidence: candidate
            .confidence
            .map(|confidence| {
                admitted_confidence(confidence).unwrap_or(0.0) * policy.confidence_weight
            })
            .unwrap_or_default(),
        evidence: if candidate.has_evidence {
            policy.evidence_bonus
        } else {
            0.0
        },
        recency,
    }
}

fn recency_timestamp(candidate: &Candidate) -> u64 {
    candidate.updated_at_ms.unwrap_or(candidate.created_at_ms)
}

fn recency_timestamp_of(result: &RecallResult) -> u64 {
    result
        .updated_at_ms
        .or(result.created_at_ms)
        .unwrap_or_default()
}

pub(crate) fn is_superseded(result: &RecallResult) -> bool {
    result
        .lifecycle
        .as_ref()
        .is_some_and(ClaimLifecycle::is_superseded)
}

fn observed_at(episode_times: &BTreeMap<&str, u64>, episode_id: Option<&str>) -> Option<u64> {
    episode_id.and_then(|id| episode_times.get(id).copied())
}

/// Recall directly from an already projected memory state and attach the same
/// health, authority, and per-result trust decisions used by [`crate::MemoryEngine`].
///
/// This read-only entry point is useful for import previews, deterministic
/// evaluations, and dependency-free demonstrations that must exercise the real
/// trust policy without opening a database. It evaluates caller-supplied
/// projected state; it does not prove that the projection came from a valid
/// ledger. Validate and project the ledger before treating integrity as part of
/// the result's trust boundary.
pub fn recall_projection_with_authority(
    data: &MemoryData,
    query: &str,
    options: RecallOptions,
) -> Result<AuthorityRecall> {
    if query.trim().is_empty() {
        return Err(NahualiError::EmptyQuery);
    }
    options.ranking.validate()?;

    let scope = options.scope.clone();
    let mut results = recall_with_options(data, query, options);
    let store_health = KnowledgeHealth::inspect(data);
    let store_authority = AuthorityDecision::evaluate(&store_health);
    let scoped_data = scope.as_ref().map(|scope| data_for_scope(data, scope));
    let authority_data = scoped_data.as_ref().unwrap_or(data);
    let health = KnowledgeHealth::inspect(authority_data);
    let authority = AuthorityDecision::evaluate(&health);
    attach_result_trust(data, &store_health, &mut results);

    Ok(AuthorityRecall {
        results,
        authority,
        health,
        store_authority,
        store_health,
    })
}

fn data_for_scope(data: &MemoryData, scope: &MemoryScope) -> MemoryData {
    let mut scoped = data.clone();
    let matches = |item_scope: Option<&MemoryScope>| {
        item_scope.is_some_and(|item_scope| item_scope.key == scope.key)
    };
    scoped.sources.retain(|item| matches(item.scope.as_ref()));
    scoped.entities.retain(|item| matches(item.scope.as_ref()));
    scoped.episodes.retain(|item| matches(item.scope.as_ref()));
    scoped.claims.retain(|item| matches(item.scope.as_ref()));
    scoped.links.retain(|item| matches(item.scope.as_ref()));
    scoped
        .procedures
        .retain(|item| matches(item.scope.as_ref()));
    scoped
        .intentions
        .retain(|item| matches(item.scope.as_ref()));
    scoped
        .review_decisions
        .retain(|item| matches(item.scope.as_ref()));
    scoped.repairs.retain(|item| matches(item.scope.as_ref()));
    scoped.facts.retain(|item| matches(item.scope.as_ref()));
    scoped.relations.retain(|item| matches(item.scope.as_ref()));

    let mut event_ids = std::collections::BTreeSet::new();
    for item in &scoped.sources {
        event_ids.insert(item.event_id.clone());
    }
    for item in &scoped.entities {
        event_ids.extend(item.source_event_ids.iter().cloned());
    }
    for item in &scoped.episodes {
        event_ids.insert(item.event_id.clone());
    }
    for item in &scoped.claims {
        event_ids.insert(item.event_id.clone());
    }
    for item in &scoped.links {
        event_ids.insert(item.event_id.clone());
    }
    for item in &scoped.procedures {
        event_ids.insert(item.event_id.clone());
    }
    for item in &scoped.intentions {
        event_ids.insert(item.event_id.clone());
        event_ids.insert(item.updated_event_id.clone());
    }
    for item in &scoped.review_decisions {
        event_ids.insert(item.event_id.clone());
    }
    for item in &scoped.repairs {
        event_ids.insert(item.event_id.clone());
    }
    scoped.event_count = event_ids.len();
    scoped.last_event_id = data
        .last_event_id
        .as_ref()
        .filter(|event_id| event_ids.contains(*event_id))
        .cloned();
    scoped
}

/// Whether an item created at `created_at_ms` falls inside the optional
/// `[since_ms, as_of_ms]` recall window. Both bounds are inclusive and
/// independently optional; with neither set, every item passes.
fn within_time_window(created_at_ms: u64, options: &RecallOptions) -> bool {
    let below_ceiling = options
        .as_of_ms
        .is_none_or(|as_of_ms| created_at_ms <= as_of_ms);
    let above_floor = options
        .since_ms
        .is_none_or(|since_ms| created_at_ms >= since_ms);
    below_ceiling && above_floor
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

/// Attach the same per-result trust verdict to hybrid recall results.
///
/// Reuses [`result_trust`] — the exact policy lexical recall runs — so a record
/// that certifies lexically certifies hybridly. The hybrid result carries enough
/// identity (kind, id, evidence) to reconstruct the recall proxy the verdict
/// needs.
pub(crate) fn attach_hybrid_result_trust(
    data: &MemoryData,
    health: &KnowledgeHealth,
    results: &mut [crate::HybridRecallResult],
) {
    for result in results {
        let proxy = RecallResult {
            kind: result.kind.clone(),
            id: result.id.clone(),
            score: result.score,
            excerpt: result.excerpt.clone(),
            evidence_id: result.evidence_id.clone(),
            matched_terms: result.matched_terms.clone(),
            scope: None,
            trust: None,
            created_at_ms: None,
            updated_at_ms: None,
            observed_at_ms: None,
            confidence: None,
            lifecycle: result.lifecycle.clone(),
            score_breakdown: None,
        };
        result.trust = Some(result_trust(data, health, &proxy));
    }
}

fn result_trust(
    data: &MemoryData,
    health: &KnowledgeHealth,
    result: &RecallResult,
) -> RecallResultTrust {
    let source_event_id = source_event_id_for_result(data, result);
    // A supersession signal cites both the retired and the replacing claim; it
    // weakens only the retired one. The replacing claim is the current value.
    let replaced_an_older_value = result
        .lifecycle
        .as_ref()
        .is_some_and(|lifecycle| !lifecycle.is_superseded());
    let local_signals = source_event_id
        .as_deref()
        .map(|event_id| {
            health
                .signals
                .iter()
                .filter(|signal| signal.evidence_ids.iter().any(|id| id == event_id))
                .filter(|signal| {
                    !(replaced_an_older_value && signal.kind == HealthSignalKind::SupersededFact)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if !local_signals.is_empty() {
        return trust_from_signals(local_signals);
    }

    if let Some(superseded_by) = result
        .lifecycle
        .as_ref()
        .filter(|lifecycle| lifecycle.is_superseded())
        .and_then(|lifecycle| lifecycle.superseded_by.as_deref())
    {
        let replacement = projected_claims(data)
            .iter()
            .find(|claim| claim.id == superseded_by)
            .map(|claim| format!("{} {} {}", claim.subject, claim.predicate, claim.object))
            .unwrap_or_else(|| superseded_by.to_string());
        return RecallResultTrust {
            mode: RecallResultTrustMode::Advisory,
            score: 0.75,
            can_trust: false,
            reasons: vec![format!(
                "Superseded by '{replacement}' ({superseded_by}); kept as history, not current memory."
            )],
            signal_kinds: vec![signal_kind_name(&HealthSignalKind::SupersededFact).into()],
        };
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

/// Whether a recall hit from a derived index (such as the semantic tier) still
/// names an item in the ledger projection. Derived indexes can lag the ledger;
/// recall must never surface an item the projection no longer contains.
pub(crate) fn projection_contains(data: &MemoryData, kind: &MemoryKind, id: &str) -> bool {
    match kind {
        MemoryKind::Entity => data.entities.iter().any(|entity| entity.id == id),
        MemoryKind::Episode => data.episodes.iter().any(|episode| episode.id == id),
        MemoryKind::Claim | MemoryKind::Fact => {
            projected_claims(data).iter().any(|claim| claim.id == id)
        }
        MemoryKind::Link | MemoryKind::Relation => {
            projected_links(data).iter().any(|link| link.id == id)
        }
        MemoryKind::Procedure => data.procedures.iter().any(|procedure| procedure.id == id),
        MemoryKind::Intention => data.intentions.iter().any(|intention| intention.id == id),
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
        HealthSignalKind::StaleEpisode => "stale_episode",
        HealthSignalKind::SupersededFact => "superseded_fact",
        HealthSignalKind::IsolatedEntity => "isolated_entity",
    }
}

fn push_result(
    results: &mut Vec<(Candidate, RecallResult)>,
    candidate: Candidate,
    result: RecallResult,
    options: &RecallOptions,
) {
    if !kind_allowed(&result.kind, &options.kinds) {
        return;
    }
    if options.require_evidence && result.evidence_id.is_none() {
        return;
    }
    if options.exclude_superseded
        && candidate
            .lifecycle
            .as_ref()
            .is_some_and(ClaimLifecycle::is_superseded)
    {
        return;
    }
    results.push((candidate, result));
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
        Claim, ClaimStatus, Episode, Link, MemoryData, MemoryKind, MemoryScope, MemoryScopeKind,
    };

    use super::{
        RecallOptions, RecallRankingPolicy, recall, recall_projection_with_authority,
        recall_scoped, recall_with_options,
    };

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

    /// D3 parity: the hybrid per-result trust verdict has the same shape and value
    /// as the lexical one for the same store — a record that certifies lexically
    /// certifies hybridly — because both paths call the same `result_trust` policy.
    #[test]
    fn hybrid_result_trust_matches_lexical_verdict() {
        use crate::{HybridRecallResult, KnowledgeHealth, RecallResultTrustMode};

        use super::{attach_hybrid_result_trust, attach_result_trust};

        let data = MemoryData {
            event_count: 3,
            last_event_id: Some("event_3".to_string()),
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
            claims: vec![
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
                Claim {
                    id: "claim_unsupported".to_string(),
                    event_id: "event_3".to_string(),
                    subject: "Lena".to_string(),
                    predicate: "prefers".to_string(),
                    object: "release notes".to_string(),
                    source_episode_id: None,
                    confidence: 0.8,
                    scope: None,
                    created_at_ms: 1,
                },
            ],
            ..MemoryData::default()
        };
        let health = KnowledgeHealth::inspect(&data);

        // Lexical results with trust attached.
        let mut lexical = recall(&data, "release notes", 10);
        attach_result_trust(&data, &health, &mut lexical);

        // Hybrid results mirroring the same items, trust attached by the reused
        // policy.
        let mut hybrid: Vec<HybridRecallResult> = lexical
            .iter()
            .map(|result| HybridRecallResult {
                kind: result.kind.clone(),
                id: result.id.clone(),
                score: result.score,
                lexical_score: Some(result.score),
                semantic_score: None,
                authority_score: 1.0,
                trust: None,
                excerpt: result.excerpt.clone(),
                evidence_id: result.evidence_id.clone(),
                matched_terms: result.matched_terms.clone(),
                explanations: Vec::new(),
                lifecycle: result.lifecycle.clone(),
            })
            .collect();
        attach_hybrid_result_trust(&data, &health, &mut hybrid);

        assert!(!lexical.is_empty());
        assert_eq!(lexical.len(), hybrid.len());
        for (lex, hyb) in lexical.iter().zip(hybrid.iter()) {
            let lexical_trust = lex.trust.as_ref().expect("lexical trust attached");
            let hybrid_trust = hyb.trust.as_ref().expect("hybrid trust attached");
            // Same verdict shape and value.
            assert_eq!(lexical_trust.mode, hybrid_trust.mode);
            assert_eq!(lexical_trust.can_trust, hybrid_trust.can_trust);
            assert!((lexical_trust.score - hybrid_trust.score).abs() < f32::EPSILON);
            assert_eq!(lexical_trust.signal_kinds, hybrid_trust.signal_kinds);
        }
        // At least one record certifies, and it certifies on both paths.
        assert!(
            hybrid
                .iter()
                .any(|result| result.trust.as_ref().unwrap().mode
                    == RecallResultTrustMode::Certify)
        );
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
    fn time_window_filters_recall_by_creation_age() {
        let data = MemoryData {
            event_count: 3,
            last_event_id: Some("event_3".to_string()),
            episodes: vec![
                Episode {
                    id: "episode_old".to_string(),
                    event_id: "event_1".to_string(),
                    content: "Lena owned the release notes.".to_string(),
                    tags: vec!["product".to_string()],
                    mentions: Vec::new(),
                    source_id: None,
                    source_position: None,
                    source_role: None,
                    scope: None,
                    created_at_ms: 100,
                },
                Episode {
                    id: "episode_new".to_string(),
                    event_id: "event_2".to_string(),
                    content: "Mateo owns the release notes now.".to_string(),
                    tags: vec!["product".to_string()],
                    mentions: Vec::new(),
                    source_id: None,
                    source_position: None,
                    source_role: None,
                    scope: None,
                    created_at_ms: 1000,
                },
            ],
            ..MemoryData::default()
        };

        // Point-in-time: as of t=500, only the older episode was known.
        let as_of = recall_with_options(
            &data,
            "release notes",
            RecallOptions {
                as_of_ms: Some(500),
                ..RecallOptions::default()
            },
        );
        assert_eq!(as_of.len(), 1);
        assert_eq!(as_of[0].id, "episode_old");

        // Freshness floor: since t=500, only the newer episode survives.
        let since = recall_with_options(
            &data,
            "release notes",
            RecallOptions {
                since_ms: Some(500),
                ..RecallOptions::default()
            },
        );
        assert_eq!(since.len(), 1);
        assert_eq!(since[0].id, "episode_new");

        // No bounds: both are returned.
        let unbounded = recall(&data, "release notes", 10);
        assert_eq!(unbounded.len(), 2);
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
    fn scoped_authority_ignores_conflicts_from_another_scope() {
        use crate::AuthorityMode;

        let project_scope = MemoryScope::new(MemoryScopeKind::Project, "Nahuali").unwrap();
        let other_scope = MemoryScope::new(MemoryScopeKind::Project, "Other").unwrap();
        let fresh = u64::MAX / 2;
        let data = MemoryData {
            event_count: 6,
            last_event_id: Some("event_other_claim_2".to_string()),
            episodes: vec![
                Episode {
                    id: "episode_project".to_string(),
                    event_id: "event_project_episode".to_string(),
                    content: "Lena owns release notes.".to_string(),
                    tags: Vec::new(),
                    mentions: Vec::new(),
                    source_id: None,
                    source_position: None,
                    source_role: None,
                    scope: Some(project_scope.clone()),
                    created_at_ms: fresh,
                },
                Episode {
                    id: "episode_other_1".to_string(),
                    event_id: "event_other_episode_1".to_string(),
                    content: "Launch is Tuesday.".to_string(),
                    tags: Vec::new(),
                    mentions: Vec::new(),
                    source_id: None,
                    source_position: None,
                    source_role: None,
                    scope: Some(other_scope.clone()),
                    created_at_ms: fresh,
                },
                Episode {
                    id: "episode_other_2".to_string(),
                    event_id: "event_other_episode_2".to_string(),
                    content: "Launch is Friday.".to_string(),
                    tags: Vec::new(),
                    mentions: Vec::new(),
                    source_id: None,
                    source_position: None,
                    source_role: None,
                    scope: Some(other_scope.clone()),
                    created_at_ms: fresh,
                },
            ],
            claims: vec![
                Claim {
                    id: "claim_project".to_string(),
                    event_id: "event_project_claim".to_string(),
                    subject: "Lena".to_string(),
                    predicate: "owns".to_string(),
                    object: "release notes".to_string(),
                    source_episode_id: Some("episode_project".to_string()),
                    confidence: 0.9,
                    scope: Some(project_scope.clone()),
                    created_at_ms: fresh,
                },
                Claim {
                    id: "claim_other_1".to_string(),
                    event_id: "event_other_claim_1".to_string(),
                    subject: "Launch".to_string(),
                    predicate: "day".to_string(),
                    object: "Tuesday".to_string(),
                    source_episode_id: Some("episode_other_1".to_string()),
                    confidence: 0.9,
                    scope: Some(other_scope.clone()),
                    created_at_ms: fresh,
                },
                Claim {
                    id: "claim_other_2".to_string(),
                    event_id: "event_other_claim_2".to_string(),
                    subject: "Launch".to_string(),
                    predicate: "day".to_string(),
                    object: "Friday".to_string(),
                    source_episode_id: Some("episode_other_2".to_string()),
                    confidence: 0.9,
                    scope: Some(other_scope),
                    created_at_ms: fresh,
                },
            ],
            ..MemoryData::default()
        };

        let report = recall_projection_with_authority(
            &data,
            "release notes",
            RecallOptions {
                scope: Some(project_scope),
                ..RecallOptions::default()
            },
        )
        .unwrap();

        assert_eq!(report.authority.mode, AuthorityMode::Certify);
        assert_eq!(report.store_authority.mode, AuthorityMode::Block);
        assert_eq!(report.health.conflicting_fact_count, 0);
        assert_eq!(report.store_health.conflicting_fact_count, 1);
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

    fn sourced_claim(id: &str, episode: &str, object: &str, confidence: f32, at: u64) -> Claim {
        Claim {
            id: id.to_string(),
            event_id: format!("event_{id}"),
            subject: "Arnold".to_string(),
            predicate: "editor".to_string(),
            object: object.to_string(),
            source_episode_id: Some(episode.to_string()),
            confidence,
            scope: None,
            created_at_ms: at,
        }
    }

    fn observed_episode(id: &str, content: &str, at: u64) -> Episode {
        Episode {
            id: id.to_string(),
            event_id: format!("event_{id}"),
            content: content.to_string(),
            tags: Vec::new(),
            mentions: Vec::new(),
            source_id: None,
            source_position: None,
            source_role: None,
            scope: None,
            created_at_ms: at,
        }
    }

    const DAY_MS: u64 = 24 * 60 * 60 * 1000;
    // Far enough in the future that staleness checks never fire.
    const FRESH_MS: u64 = u64::MAX / 4;

    fn editor_switch(old_confidence: f32, new_confidence: f32) -> MemoryData {
        let day = DAY_MS;
        MemoryData {
            event_count: 4,
            last_event_id: Some("event_claim_helix".to_string()),
            episodes: vec![
                observed_episode("episode_vim", "Arnold uses Vim", FRESH_MS + 10 * day),
                observed_episode(
                    "episode_helix",
                    "Arnold switched to Helix",
                    FRESH_MS + 70 * day,
                ),
            ],
            claims: vec![
                sourced_claim(
                    "claim_vim",
                    "episode_vim",
                    "Vim",
                    old_confidence,
                    FRESH_MS + 10 * day,
                ),
                sourced_claim(
                    "claim_helix",
                    "episode_helix",
                    "Helix",
                    new_confidence,
                    FRESH_MS + 70 * day,
                ),
            ],
            ..MemoryData::default()
        }
    }

    #[test]
    fn superseded_claim_ranks_after_current_even_with_higher_confidence() {
        let data = editor_switch(1.0, 0.5);
        let results = recall_with_options(
            &data,
            "Arnold editor",
            RecallOptions {
                kinds: vec![MemoryKind::Claim],
                ..RecallOptions::default()
            },
        );

        assert_eq!(
            results
                .iter()
                .map(|result| result.id.as_str())
                .collect::<Vec<_>>(),
            ["claim_helix", "claim_vim"]
        );
        assert!(results[1].score > results[0].score, "tier wins over score");
        let vim = results[1]
            .lifecycle
            .as_ref()
            .expect("claims carry lifecycle");
        assert_eq!(vim.status, ClaimStatus::Superseded);
        assert_eq!(vim.superseded_by.as_deref(), Some("claim_helix"));
        assert_eq!(vim.superseded_at_ms, Some(FRESH_MS + 70 * DAY_MS));
        let helix = results[0]
            .lifecycle
            .as_ref()
            .expect("claims carry lifecycle");
        assert_eq!(helix.status, ClaimStatus::Current);
        assert_eq!(helix.supersedes, ["claim_vim"]);
        assert_eq!(results[0].observed_at_ms, Some(FRESH_MS + 70 * DAY_MS));
        assert_eq!(results[1].confidence, Some(1.0));

        let current_only = recall_with_options(
            &data,
            "Arnold editor",
            RecallOptions {
                kinds: vec![MemoryKind::Claim],
                exclude_superseded: true,
                ..RecallOptions::default()
            },
        );
        assert_eq!(current_only.len(), 1);
        assert_eq!(current_only[0].id, "claim_helix");
    }

    #[test]
    fn recency_decays_by_half_life_from_the_newest_match() {
        let data = editor_switch(0.8, 0.8);
        let results = recall_with_options(
            &data,
            "Arnold editor",
            RecallOptions {
                kinds: vec![MemoryKind::Claim],
                ..RecallOptions::default()
            },
        );
        let helix = results[0].score_breakdown.as_ref().unwrap();
        let vim = results[1].score_breakdown.as_ref().unwrap();
        // 60 days apart with a 30-day half-life: two halvings of the 0.5 weight.
        assert!((helix.recency - 0.5).abs() < 1e-6);
        assert!((vim.recency - 0.125).abs() < 1e-6);
        assert!((helix.confidence - 0.8).abs() < 1e-6);
        assert!((helix.evidence - 0.25).abs() < 1e-6);
        let total = helix.relevance + helix.confidence + helix.evidence + helix.recency;
        assert!((results[0].score - total).abs() < 1e-6);
    }

    #[test]
    fn recency_reorders_equal_matches_but_never_beats_an_extra_matched_term() {
        let day = 24 * 60 * 60 * 1000;
        let data = MemoryData {
            event_count: 2,
            last_event_id: Some("event_new".to_string()),
            episodes: vec![
                observed_episode("episode_old", "release checklist Lena", day),
                observed_episode("episode_new", "release owner is Mateo now", 400 * day),
            ],
            ..MemoryData::default()
        };

        let both_terms = recall(&data, "release checklist", 10);
        assert_eq!(
            both_terms[0].id, "episode_old",
            "two matched terms beat recency"
        );

        let one_term = recall(&data, "release", 10);
        assert_eq!(one_term[0].id, "episode_new", "same match, newer wins");

        let no_recency = recall_with_options(
            &data,
            "release",
            RecallOptions {
                ranking: RecallRankingPolicy {
                    recency_weight: 0.0,
                    ..RecallRankingPolicy::default()
                },
                ..RecallOptions::default()
            },
        );
        // Without recency the shorter (denser) old episode wins on relevance.
        assert_eq!(no_recency[0].id, "episode_old");
    }

    #[test]
    fn ranking_is_independent_of_unrelated_newer_memory() {
        let data = editor_switch(0.8, 0.8);
        let baseline = recall(&data, "Arnold editor", 10);

        let mut with_unrelated = data.clone();
        with_unrelated.episodes.push(observed_episode(
            "episode_unrelated",
            "Mateo bought coffee",
            u64::MAX / 2,
        ));
        let again = recall(&with_unrelated, "Arnold editor", 10);

        assert_eq!(baseline, again);
    }

    #[test]
    fn confidence_weight_is_tunable() {
        let data = MemoryData {
            event_count: 2,
            last_event_id: Some("event_b".to_string()),
            claims: vec![
                Claim {
                    subject: "Lena".to_string(),
                    predicate: "owns".to_string(),
                    object: "billing".to_string(),
                    ..sourced_claim("claim_a", "episode_1", "", 0.9, 5)
                },
                Claim {
                    subject: "Lena".to_string(),
                    predicate: "owns".to_string(),
                    object: "billing".to_string(),
                    ..sourced_claim("claim_b", "episode_1", "", 0.3, 5)
                },
            ],
            ..MemoryData::default()
        };
        let weighted = recall(&data, "billing", 10);
        assert_eq!(weighted[0].id, "claim_a");
        assert!((weighted[0].score - weighted[1].score - 0.6).abs() < 1e-5);

        let unweighted = recall_with_options(
            &data,
            "billing",
            RecallOptions {
                ranking: RecallRankingPolicy {
                    confidence_weight: 0.0,
                    ..RecallRankingPolicy::default()
                },
                ..RecallOptions::default()
            },
        );
        assert!((unweighted[0].score - unweighted[1].score).abs() < 1e-6);
        assert_eq!(unweighted[0].id, "claim_a", "exact ties break by id");
    }

    #[test]
    fn invalid_ranking_policy_is_rejected() {
        let data = editor_switch(0.8, 0.8);
        for ranking in [
            RecallRankingPolicy {
                recency_weight: -0.1,
                ..RecallRankingPolicy::default()
            },
            RecallRankingPolicy {
                confidence_weight: f32::NAN,
                ..RecallRankingPolicy::default()
            },
            RecallRankingPolicy {
                recency_half_life_ms: 0,
                ..RecallRankingPolicy::default()
            },
        ] {
            let error = recall_projection_with_authority(
                &data,
                "Arnold editor",
                RecallOptions {
                    ranking,
                    ..RecallOptions::default()
                },
            )
            .unwrap_err();
            assert!(matches!(
                error,
                crate::NahualiError::InvalidRecallRanking { .. }
            ));
        }
    }

    #[test]
    fn current_value_is_not_weakened_by_the_supersession_it_caused() {
        use crate::RecallResultTrustMode;

        let data = editor_switch(0.8, 0.8);
        let report =
            recall_projection_with_authority(&data, "Arnold editor", RecallOptions::default())
                .unwrap();
        let trust_of = |id: &str| {
            report
                .results
                .iter()
                .find(|result| result.id == id)
                .and_then(|result| result.trust.clone())
                .expect("trust attached")
        };
        assert_eq!(trust_of("claim_helix").mode, RecallResultTrustMode::Certify);
        let vim = trust_of("claim_vim");
        assert_eq!(vim.mode, RecallResultTrustMode::Warn);
        assert_eq!(vim.signal_kinds, ["superseded_fact"]);
    }

    #[test]
    fn projection_contains_rejects_items_missing_from_the_ledger() {
        let data = editor_switch(0.8, 0.8);
        assert!(super::projection_contains(
            &data,
            &MemoryKind::Claim,
            "claim_vim"
        ));
        assert!(super::projection_contains(
            &data,
            &MemoryKind::Fact,
            "claim_vim"
        ));
        assert!(!super::projection_contains(
            &data,
            &MemoryKind::Claim,
            "claim_gone"
        ));
        assert!(super::projection_contains(
            &data,
            &MemoryKind::Episode,
            "episode_vim"
        ));
        assert!(!super::projection_contains(
            &data,
            &MemoryKind::Episode,
            "episode_gone"
        ));
    }
}
