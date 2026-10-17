use serde::{Deserialize, Serialize};

use crate::{
    AuthorityDecision, DEFAULT_INTENTION_STALE_AFTER_MS, IntentionKind, IntentionPriority,
    IntentionStatus, KnowledgeHealth, MemoryData, OperatorReviewItem, OperatorReviewOptions,
    ProjectScope, ProjectScopeSummary, SelfInspectionReviewPriority, intention_is_stale,
    operator_review,
};

/// Current session-briefing report format version.
pub const MEMORY_BRIEFING_VERSION: u32 = 1;

/// Options for building a compact session-continuity briefing.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct BriefingOptions {
    /// Maximum recent episodes returned.
    pub episode_limit: usize,
    /// Maximum active intentions returned.
    pub intention_limit: usize,
    /// Maximum high-priority review items returned.
    pub review_limit: usize,
    /// Maximum graph seed entities returned.
    pub graph_seed_limit: usize,
}

impl Default for BriefingOptions {
    fn default() -> Self {
        Self {
            episode_limit: 5,
            intention_limit: 5,
            review_limit: 5,
            graph_seed_limit: 8,
        }
    }
}

/// Compact pre-work report for humans, scripts, and agents.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct MemoryBriefingReport {
    /// Report format version.
    pub version: u32,
    /// Timestamp in milliseconds when the report was generated.
    pub generated_at_ms: u64,
    /// Number of source events represented by the projection.
    pub event_count: usize,
    /// Projection-level authority decision.
    pub authority: AuthorityDecision,
    /// Knowledge-health report used to produce the authority decision.
    pub health: KnowledgeHealth,
    /// Aggregate counts for the briefing.
    pub summary: BriefingSummary,
    /// Most recent observed episodes.
    pub recent_episodes: Vec<BriefingEpisode>,
    /// Active intentions: fresh ones first, then stale ones, each group sorted
    /// by priority and then most recently updated.
    pub active_intentions: Vec<BriefingIntention>,
    /// Critical or high-priority operator review items.
    pub review_items: Vec<OperatorReviewItem>,
    /// Entity seeds worth using for graph traversal.
    pub graph_seeds: Vec<BriefingGraphSeed>,
    /// Active project scope. When set, recent episodes, active intentions,
    /// and the intention counts cover only memory attributed to the project;
    /// health, review items, and graph seeds stay store-wide.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_scope: Option<ProjectScopeSummary>,
}

/// Aggregate counts for a session briefing.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct BriefingSummary {
    /// Number of projected source records.
    pub source_count: usize,
    /// Number of projected episodes.
    pub episode_count: usize,
    /// Number of projected entities.
    pub entity_count: usize,
    /// Number of active intentions in the full projection, or in the project
    /// when a project scope is active.
    pub active_intention_count: usize,
    /// Number of those active intentions not updated within
    /// [`DEFAULT_INTENTION_STALE_AFTER_MS`].
    #[serde(default)]
    pub stale_intention_count: usize,
    /// Number of critical or high-priority review items before display limiting.
    pub high_priority_review_count: usize,
    /// Number of critical review items before display limiting.
    pub critical_review_count: usize,
    /// Number of high review items before display limiting.
    pub high_review_count: usize,
    /// Number of recent episodes returned.
    pub returned_episode_count: usize,
    /// Number of active intentions returned.
    pub returned_intention_count: usize,
    /// Number of review items returned.
    pub returned_review_count: usize,
    /// Number of graph seeds returned.
    pub graph_seed_count: usize,
}

/// Episode entry included in a session briefing.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct BriefingEpisode {
    /// Stable episode identifier.
    pub id: String,
    /// Event identifier that created this episode.
    pub event_id: String,
    /// Natural-language episode content.
    pub content: String,
    /// User-provided labels.
    pub tags: Vec<String>,
    /// Explicit entity mentions.
    pub mentions: Vec<String>,
    /// Source record that produced this episode, when available.
    pub source_id: Option<String>,
    /// Stable position within the source, when known.
    pub source_position: Option<u32>,
    /// Source-local role or speaker, when known.
    pub source_role: Option<String>,
    /// Event timestamp in milliseconds since the Unix epoch.
    pub created_at_ms: u64,
}

/// Intention entry included in a session briefing.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct BriefingIntention {
    /// Stable intention identifier.
    pub id: String,
    /// Event identifier that created this intention.
    pub event_id: String,
    /// Event identifier that last changed this intention.
    pub updated_event_id: String,
    /// Intention category.
    pub kind: IntentionKind,
    /// Current lifecycle state.
    pub status: IntentionStatus,
    /// Priority for operator attention.
    pub priority: IntentionPriority,
    /// What should happen.
    pub description: String,
    /// Optional source episode that supports this intention.
    pub source_episode_id: Option<String>,
    /// Optional reason attached to the current lifecycle state.
    pub status_reason: Option<String>,
    /// Event timestamp in milliseconds since the Unix epoch.
    pub created_at_ms: u64,
    /// Timestamp for the latest lifecycle event.
    pub updated_at_ms: u64,
    /// Whether the intention has gone longer than
    /// [`DEFAULT_INTENTION_STALE_AFTER_MS`] without an update, the same
    /// threshold `reconcile-intentions` uses by default.
    #[serde(default)]
    pub stale: bool,
}

/// Entity seed included in a session briefing for graph traversal.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct BriefingGraphSeed {
    /// Stable entity identifier.
    pub id: String,
    /// Human-readable entity label.
    pub label: String,
    /// Number of projected mentions.
    pub mention_count: usize,
    /// Timestamp when the entity was first observed.
    pub first_seen_at_ms: u64,
    /// Timestamp when the entity was last observed.
    pub last_seen_at_ms: u64,
    /// Event identifiers that mention this entity.
    pub source_event_ids: Vec<String>,
}

pub(crate) fn briefing(
    data: &MemoryData,
    options: BriefingOptions,
    project: Option<&ProjectScope>,
) -> MemoryBriefingReport {
    briefing_at(data, options, project, now_ms())
}

pub(crate) fn briefing_at(
    data: &MemoryData,
    options: BriefingOptions,
    project: Option<&ProjectScope>,
    generated_at_ms: u64,
) -> MemoryBriefingReport {
    // A limit of 0 means "omit this section"; the summary counts still cover
    // the full projection (or the whole project when scoped).
    let health = KnowledgeHealth::inspect_at(data, generated_at_ms);
    let authority = AuthorityDecision::evaluate(&health);
    let recent_episodes = recent_episodes(data, project, options.episode_limit);
    let active_intentions =
        active_intentions(data, project, options.intention_limit, generated_at_ms);
    let mut review = operator_review::operator_review_at(
        data,
        OperatorReviewOptions {
            limit: options.review_limit,
            min_priority: Some(SelfInspectionReviewPriority::High),
            ..OperatorReviewOptions::default()
        },
        generated_at_ms,
    );
    // The operator review queue never returns fewer than one item on its own.
    review.items.truncate(options.review_limit);
    let graph_seeds = graph_seeds(data, options.graph_seed_limit);
    let active = data
        .intentions
        .iter()
        .filter(|intention| intention.status == IntentionStatus::Active)
        .filter(|intention| project.is_none_or(|scope| scope.includes_intention(&intention.id)));
    let active_intention_count = active.clone().count();
    let stale_intention_count = active
        .filter(|intention| is_stale(intention.updated_at_ms, generated_at_ms))
        .count();
    let summary = BriefingSummary {
        source_count: data.sources.len(),
        episode_count: data.episodes.len(),
        entity_count: data.entities.len(),
        active_intention_count,
        stale_intention_count,
        high_priority_review_count: review.total_items,
        critical_review_count: review.summary.critical_count,
        high_review_count: review.summary.high_count,
        returned_episode_count: recent_episodes.len(),
        returned_intention_count: active_intentions.len(),
        returned_review_count: review.items.len(),
        graph_seed_count: graph_seeds.len(),
    };

    MemoryBriefingReport {
        version: MEMORY_BRIEFING_VERSION,
        generated_at_ms,
        event_count: data.event_count,
        authority,
        health,
        summary,
        recent_episodes,
        active_intentions,
        review_items: review.items,
        graph_seeds,
        project_scope: project.map(|scope| scope.summary().clone()),
    }
}

fn recent_episodes(
    data: &MemoryData,
    project: Option<&ProjectScope>,
    limit: usize,
) -> Vec<BriefingEpisode> {
    let mut episodes = data
        .episodes
        .iter()
        .filter(|episode| project.is_none_or(|scope| scope.includes_episode(&episode.id)))
        .cloned()
        .collect::<Vec<_>>();
    episodes.sort_by(|left, right| {
        right
            .created_at_ms
            .cmp(&left.created_at_ms)
            .then_with(|| left.id.cmp(&right.id))
    });

    episodes
        .into_iter()
        .take(limit)
        .map(|episode| BriefingEpisode {
            id: episode.id,
            event_id: episode.event_id,
            content: episode.content,
            tags: episode.tags,
            mentions: episode.mentions,
            source_id: episode.source_id,
            source_position: episode.source_position,
            source_role: episode.source_role,
            created_at_ms: episode.created_at_ms,
        })
        .collect()
}

/// Whether an active intention is stale under the default reconciliation
/// threshold, so the briefing and `reconcile-intentions` agree.
fn is_stale(updated_at_ms: u64, now_ms: u64) -> bool {
    intention_is_stale(updated_at_ms, now_ms, DEFAULT_INTENTION_STALE_AFTER_MS)
}

fn active_intentions(
    data: &MemoryData,
    project: Option<&ProjectScope>,
    limit: usize,
    now_ms: u64,
) -> Vec<BriefingIntention> {
    let mut intentions = data
        .intentions
        .iter()
        .filter(|intention| intention.status == IntentionStatus::Active)
        .filter(|intention| project.is_none_or(|scope| scope.includes_intention(&intention.id)))
        .cloned()
        .collect::<Vec<_>>();
    // Fresh work first: an old critical intention nobody touched in months
    // should not crowd out the high-priority work in flight.
    intentions.sort_by(|left, right| {
        is_stale(left.updated_at_ms, now_ms)
            .cmp(&is_stale(right.updated_at_ms, now_ms))
            .then_with(|| {
                intention_priority_rank(&left.priority)
                    .cmp(&intention_priority_rank(&right.priority))
            })
            .then_with(|| right.updated_at_ms.cmp(&left.updated_at_ms))
            .then_with(|| left.id.cmp(&right.id))
    });

    intentions
        .into_iter()
        .take(limit)
        .map(|intention| BriefingIntention {
            stale: is_stale(intention.updated_at_ms, now_ms),
            id: intention.id,
            event_id: intention.event_id,
            updated_event_id: intention.updated_event_id,
            kind: intention.kind,
            status: intention.status,
            priority: intention.priority,
            description: intention.description,
            source_episode_id: intention.source_episode_id,
            status_reason: intention.status_reason,
            created_at_ms: intention.created_at_ms,
            updated_at_ms: intention.updated_at_ms,
        })
        .collect()
}

fn graph_seeds(data: &MemoryData, limit: usize) -> Vec<BriefingGraphSeed> {
    let mut entities = data.entities.clone();
    entities.sort_by(|left, right| {
        right
            .mention_count
            .cmp(&left.mention_count)
            .then_with(|| right.last_seen_at_ms.cmp(&left.last_seen_at_ms))
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.id.cmp(&right.id))
    });

    entities
        .into_iter()
        .take(limit)
        .map(|entity| BriefingGraphSeed {
            id: entity.id,
            label: entity.name,
            mention_count: entity.mention_count,
            first_seen_at_ms: entity.first_seen_at_ms,
            last_seen_at_ms: entity.last_seen_at_ms,
            source_event_ids: entity.source_event_ids,
        })
        .collect()
}

fn intention_priority_rank(priority: &IntentionPriority) -> u8 {
    match priority {
        IntentionPriority::Critical => 0,
        IntentionPriority::High => 1,
        IntentionPriority::Medium => 2,
        IntentionPriority::Low => 3,
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{BriefingOptions, briefing_at};
    use crate::{
        DEFAULT_INTENTION_STALE_AFTER_MS, Intention, IntentionKind, IntentionPriority,
        IntentionStatus, MemoryData,
    };

    const DAY_MS: u64 = 24 * 60 * 60 * 1000;
    const NOW_MS: u64 = 400 * DAY_MS;

    fn intention(id: &str, priority: IntentionPriority, age_days: u64) -> Intention {
        Intention {
            id: id.to_string(),
            event_id: format!("event_{id}"),
            updated_event_id: format!("event_{id}"),
            kind: IntentionKind::Task,
            status: IntentionStatus::Active,
            priority,
            description: id.to_string(),
            source_episode_id: None,
            status_reason: None,
            deadline_at_ms: None,
            depends_on: Vec::new(),
            goal_id: None,
            progress_percent: None,
            scope: None,
            created_at_ms: 0,
            updated_at_ms: NOW_MS - age_days * DAY_MS,
        }
    }

    fn data(intentions: Vec<Intention>) -> MemoryData {
        MemoryData {
            intentions,
            ..MemoryData::default()
        }
    }

    fn options(intention_limit: usize) -> BriefingOptions {
        BriefingOptions {
            intention_limit,
            ..BriefingOptions::default()
        }
    }

    #[test]
    fn fresh_intentions_lead_and_stale_ones_follow_in_priority_order() {
        let data = data(vec![
            intention("old_critical", IntentionPriority::Critical, 92),
            intention("older_critical", IntentionPriority::Critical, 120),
            intention("fresh_low", IntentionPriority::Low, 1),
            intention("fresh_high", IntentionPriority::High, 3),
            intention("fresh_high_newer", IntentionPriority::High, 2),
            intention("old_high", IntentionPriority::High, 61),
        ]);

        let report = briefing_at(&data, options(10), None, NOW_MS);
        let order = report
            .active_intentions
            .iter()
            .map(|intention| (intention.id.as_str(), intention.stale))
            .collect::<Vec<_>>();
        assert_eq!(
            order,
            vec![
                ("fresh_high_newer", false),
                ("fresh_high", false),
                ("fresh_low", false),
                ("old_critical", true),
                ("older_critical", true),
                ("old_high", true),
            ]
        );
        assert_eq!(report.summary.active_intention_count, 6);
        assert_eq!(report.summary.stale_intention_count, 3);
    }

    #[test]
    fn stale_count_covers_intentions_hidden_by_the_limit() {
        let data = data(vec![
            intention("fresh", IntentionPriority::Medium, 0),
            intention("stale_a", IntentionPriority::Critical, 45),
            intention("stale_b", IntentionPriority::Critical, 46),
        ]);

        let report = briefing_at(&data, options(1), None, NOW_MS);
        assert_eq!(report.active_intentions.len(), 1);
        assert_eq!(report.active_intentions[0].id, "fresh");
        assert_eq!(report.summary.stale_intention_count, 2);
    }

    #[test]
    fn stale_uses_the_reconciliation_threshold_exclusively() {
        let at_threshold = Intention {
            updated_at_ms: NOW_MS - DEFAULT_INTENTION_STALE_AFTER_MS,
            ..intention("at_threshold", IntentionPriority::Medium, 0)
        };
        let past_threshold = Intention {
            updated_at_ms: NOW_MS - DEFAULT_INTENTION_STALE_AFTER_MS - 1,
            ..intention("past_threshold", IntentionPriority::Medium, 0)
        };
        let report = briefing_at(
            &data(vec![at_threshold, past_threshold]),
            options(5),
            None,
            NOW_MS,
        );
        let stale = |id: &str| {
            report
                .active_intentions
                .iter()
                .find(|intention| intention.id == id)
                .map(|intention| intention.stale)
        };
        assert_eq!(stale("at_threshold"), Some(false));
        assert_eq!(stale("past_threshold"), Some(true));
    }

    #[test]
    fn project_scope_keeps_only_attributed_episodes_and_intentions() {
        let episode = |id: &str, mention: &str| crate::Episode {
            id: id.to_string(),
            event_id: format!("event_{id}"),
            content: format!("{id} notes"),
            tags: Vec::new(),
            mentions: vec![mention.to_string()],
            source_id: None,
            source_position: None,
            source_role: None,
            scope: None,
            created_at_ms: NOW_MS,
        };
        let mut data = data(vec![
            Intention {
                source_episode_id: Some("episode_nahuali".to_string()),
                ..intention("intention_nahuali", IntentionPriority::High, 1)
            },
            Intention {
                source_episode_id: Some("episode_orvian".to_string()),
                ..intention("intention_orvian", IntentionPriority::Critical, 1)
            },
            Intention {
                source_episode_id: Some("episode_nahuali".to_string()),
                ..intention("intention_nahuali_stale", IntentionPriority::Low, 90)
            },
        ]);
        data.episodes = vec![
            episode("episode_nahuali", "Nahuali"),
            episode("episode_orvian", "Orvian"),
        ];
        let scope = crate::project::project_scope(&data, "nahuali").expect("scope builds");

        let report = briefing_at(&data, options(10), Some(&scope), NOW_MS);

        let episodes = report
            .recent_episodes
            .iter()
            .map(|episode| episode.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(episodes, vec!["episode_nahuali"]);
        let intentions = report
            .active_intentions
            .iter()
            .map(|intention| intention.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            intentions,
            vec!["intention_nahuali", "intention_nahuali_stale"]
        );
        assert_eq!(report.summary.active_intention_count, 2);
        assert_eq!(report.summary.stale_intention_count, 1);
        let summary = report.project_scope.expect("scope is reported");
        assert_eq!(summary.project, "nahuali");
        assert_eq!(summary.scope_key, "project:nahuali");
        assert_eq!(summary.episode_count, 1);
        assert_eq!(summary.intention_count, 2);

        let unscoped = briefing_at(&data, options(10), None, NOW_MS);
        assert!(unscoped.project_scope.is_none());
        assert_eq!(unscoped.summary.active_intention_count, 3);
    }
}
