use serde::{Deserialize, Serialize};

use crate::{
    AuthorityDecision, IntentionKind, IntentionPriority, IntentionStatus, KnowledgeHealth,
    MemoryData, OperatorReviewItem, OperatorReviewOptions, SelfInspectionReviewPriority,
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
    /// Active intentions sorted by priority.
    pub active_intentions: Vec<BriefingIntention>,
    /// Critical or high-priority operator review items.
    pub review_items: Vec<OperatorReviewItem>,
    /// Entity seeds worth using for graph traversal.
    pub graph_seeds: Vec<BriefingGraphSeed>,
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
    /// Number of active intentions in the full projection.
    pub active_intention_count: usize,
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

pub(crate) fn briefing(data: &MemoryData, options: BriefingOptions) -> MemoryBriefingReport {
    briefing_at(data, options, now_ms())
}

pub(crate) fn briefing_at(
    data: &MemoryData,
    options: BriefingOptions,
    generated_at_ms: u64,
) -> MemoryBriefingReport {
    let episode_limit = options.episode_limit.max(1);
    let intention_limit = options.intention_limit.max(1);
    let review_limit = options.review_limit.max(1);
    let graph_seed_limit = options.graph_seed_limit.max(1);

    let health = KnowledgeHealth::inspect_at(data, generated_at_ms);
    let authority = AuthorityDecision::evaluate(&health);
    let recent_episodes = recent_episodes(data, episode_limit);
    let active_intentions = active_intentions(data, intention_limit);
    let review = operator_review::operator_review_at(
        data,
        OperatorReviewOptions {
            limit: review_limit,
            min_priority: Some(SelfInspectionReviewPriority::High),
            ..OperatorReviewOptions::default()
        },
        generated_at_ms,
    );
    let graph_seeds = graph_seeds(data, graph_seed_limit);
    let active_intention_count = data
        .intentions
        .iter()
        .filter(|intention| intention.status == IntentionStatus::Active)
        .count();
    let summary = BriefingSummary {
        source_count: data.sources.len(),
        episode_count: data.episodes.len(),
        entity_count: data.entities.len(),
        active_intention_count,
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
    }
}

fn recent_episodes(data: &MemoryData, limit: usize) -> Vec<BriefingEpisode> {
    let mut episodes = data.episodes.clone();
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

fn active_intentions(data: &MemoryData, limit: usize) -> Vec<BriefingIntention> {
    let mut intentions = data
        .intentions
        .iter()
        .filter(|intention| intention.status == IntentionStatus::Active)
        .cloned()
        .collect::<Vec<_>>();
    intentions.sort_by(|left, right| {
        intention_priority_rank(&left.priority)
            .cmp(&intention_priority_rank(&right.priority))
            .then_with(|| right.updated_at_ms.cmp(&left.updated_at_ms))
            .then_with(|| left.id.cmp(&right.id))
    });

    intentions
        .into_iter()
        .take(limit)
        .map(|intention| BriefingIntention {
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
