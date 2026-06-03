use nahuali_core::{
    BriefingEpisode, BriefingGraphSeed, BriefingIntention, BriefingSummary, MemoryBriefingReport,
};
use rmcp::schemars;
use serde::Serialize;

use super::{AuthorityDecisionView, HealthView, OperatorReviewItemView, json_string};

/// Aggregate counts for a session briefing.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct BriefingSummaryView {
    source_count: usize,
    episode_count: usize,
    entity_count: usize,
    active_intention_count: usize,
    high_priority_review_count: usize,
    critical_review_count: usize,
    high_review_count: usize,
    returned_episode_count: usize,
    returned_intention_count: usize,
    returned_review_count: usize,
    graph_seed_count: usize,
}

impl From<BriefingSummary> for BriefingSummaryView {
    fn from(summary: BriefingSummary) -> Self {
        Self {
            source_count: summary.source_count,
            episode_count: summary.episode_count,
            entity_count: summary.entity_count,
            active_intention_count: summary.active_intention_count,
            high_priority_review_count: summary.high_priority_review_count,
            critical_review_count: summary.critical_review_count,
            high_review_count: summary.high_review_count,
            returned_episode_count: summary.returned_episode_count,
            returned_intention_count: summary.returned_intention_count,
            returned_review_count: summary.returned_review_count,
            graph_seed_count: summary.graph_seed_count,
        }
    }
}

/// Recent-episode entry included in a briefing.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct BriefingEpisodeView {
    id: String,
    event_id: String,
    content: String,
    tags: Vec<String>,
    mentions: Vec<String>,
    source_id: Option<String>,
    source_position: Option<u32>,
    source_role: Option<String>,
    created_at_ms: u64,
}

impl From<BriefingEpisode> for BriefingEpisodeView {
    fn from(episode: BriefingEpisode) -> Self {
        Self {
            id: episode.id,
            event_id: episode.event_id,
            content: episode.content,
            tags: episode.tags,
            mentions: episode.mentions,
            source_id: episode.source_id,
            source_position: episode.source_position,
            source_role: episode.source_role,
            created_at_ms: episode.created_at_ms,
        }
    }
}

/// Active-intention entry included in a briefing.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct BriefingIntentionView {
    id: String,
    event_id: String,
    updated_event_id: String,
    kind: String,
    status: String,
    priority: String,
    description: String,
    source_episode_id: Option<String>,
    status_reason: Option<String>,
    created_at_ms: u64,
    updated_at_ms: u64,
}

impl From<BriefingIntention> for BriefingIntentionView {
    fn from(intention: BriefingIntention) -> Self {
        Self {
            id: intention.id,
            event_id: intention.event_id,
            updated_event_id: intention.updated_event_id,
            kind: json_string(&intention.kind),
            status: json_string(&intention.status),
            priority: json_string(&intention.priority),
            description: intention.description,
            source_episode_id: intention.source_episode_id,
            status_reason: intention.status_reason,
            created_at_ms: intention.created_at_ms,
            updated_at_ms: intention.updated_at_ms,
        }
    }
}

/// Entity seed for graph traversal included in a briefing.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct BriefingGraphSeedView {
    id: String,
    label: String,
    mention_count: usize,
    first_seen_at_ms: u64,
    last_seen_at_ms: u64,
    source_event_ids: Vec<String>,
}

impl From<BriefingGraphSeed> for BriefingGraphSeedView {
    fn from(seed: BriefingGraphSeed) -> Self {
        Self {
            id: seed.id,
            label: seed.label,
            mention_count: seed.mention_count,
            first_seen_at_ms: seed.first_seen_at_ms,
            last_seen_at_ms: seed.last_seen_at_ms,
            source_event_ids: seed.source_event_ids,
        }
    }
}

/// Structured session-briefing report surfacing authority, health, and seeds.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct BriefingReportView {
    version: u32,
    generated_at_ms: u64,
    event_count: usize,
    authority: AuthorityDecisionView,
    health: HealthView,
    summary: BriefingSummaryView,
    recent_episodes: Vec<BriefingEpisodeView>,
    active_intentions: Vec<BriefingIntentionView>,
    review_items: Vec<OperatorReviewItemView>,
    graph_seeds: Vec<BriefingGraphSeedView>,
}

impl From<MemoryBriefingReport> for BriefingReportView {
    fn from(report: MemoryBriefingReport) -> Self {
        Self {
            version: report.version,
            generated_at_ms: report.generated_at_ms,
            event_count: report.event_count,
            authority: AuthorityDecisionView::from(report.authority),
            health: HealthView::from(report.health),
            summary: BriefingSummaryView::from(report.summary),
            recent_episodes: report
                .recent_episodes
                .into_iter()
                .map(BriefingEpisodeView::from)
                .collect(),
            active_intentions: report
                .active_intentions
                .into_iter()
                .map(BriefingIntentionView::from)
                .collect(),
            review_items: report
                .review_items
                .into_iter()
                .map(OperatorReviewItemView::from)
                .collect(),
            graph_seeds: report
                .graph_seeds
                .into_iter()
                .map(BriefingGraphSeedView::from)
                .collect(),
        }
    }
}
