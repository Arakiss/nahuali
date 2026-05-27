use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::{
    AuthorityDecision, Claim, Entity, Episode, Intention, IntentionStatus, KnowledgeHealth, Link,
    MemoryData, MemoryGraphReport, OperatorReviewItem, OperatorReviewOptions, Procedure,
    RecallOptions, RecallResult, Result, SelfInspectionReviewPriority, graph, operator_review,
    recall,
};

/// Current project/entity view report format version.
pub const MEMORY_PROJECT_VIEW_VERSION: u32 = 1;

/// Options for building a focused project/entity dashboard.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProjectViewOptions {
    /// Maximum graph depth used for the focused neighborhood.
    pub graph_depth: usize,
    /// Maximum graph nodes returned.
    pub graph_limit: usize,
    /// Maximum memory items returned per section.
    pub item_limit: usize,
    /// Maximum lexical recall results returned.
    pub recall_limit: usize,
    /// Maximum review items returned.
    pub review_limit: usize,
}

impl Default for ProjectViewOptions {
    fn default() -> Self {
        Self {
            graph_depth: 2,
            graph_limit: 100,
            item_limit: 10,
            recall_limit: 10,
            review_limit: 10,
        }
    }
}

/// Focused project/entity dashboard composed from the current projection.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct MemoryProjectReport {
    /// Report format version.
    pub version: u32,
    /// Original operator query.
    pub query: String,
    /// Timestamp in milliseconds when the report was generated.
    pub generated_at_ms: u64,
    /// Number of source events represented by the projection.
    pub event_count: usize,
    /// Best matching entity, when one exists.
    pub matched_entity: Option<Entity>,
    /// Projection-level authority decision.
    pub authority: AuthorityDecision,
    /// Knowledge-health report used to produce the authority decision.
    pub health: KnowledgeHealth,
    /// Aggregate counts for the focused dashboard.
    pub summary: ProjectViewSummary,
    /// Graph neighborhood around the query.
    pub graph: MemoryGraphReport,
    /// Lexical recall results for the query.
    pub recall_results: Vec<RecallResult>,
    /// Recent episodes associated with the query or matched entity.
    pub episodes: Vec<Episode>,
    /// Claims associated with the query or matched entity.
    pub claims: Vec<Claim>,
    /// Links associated with the query or matched entity.
    pub links: Vec<Link>,
    /// Procedures and preferences associated with the query.
    pub procedures: Vec<Procedure>,
    /// Intentions associated with the query.
    pub intentions: Vec<Intention>,
    /// Operator review items associated with the focused evidence.
    pub review_items: Vec<OperatorReviewItem>,
}

/// Aggregate counts for a project/entity dashboard.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProjectViewSummary {
    /// Whether the query matched a projected entity.
    pub matched_entity: bool,
    /// Number of graph nodes returned.
    pub graph_node_count: usize,
    /// Number of graph edges returned.
    pub graph_edge_count: usize,
    /// Number of recall results returned.
    pub recall_result_count: usize,
    /// Number of associated episodes returned.
    pub episode_count: usize,
    /// Number of associated claims returned.
    pub claim_count: usize,
    /// Number of associated links returned.
    pub link_count: usize,
    /// Number of associated procedures returned.
    pub procedure_count: usize,
    /// Number of associated intentions returned.
    pub intention_count: usize,
    /// Number of associated review items returned.
    pub review_item_count: usize,
}

pub(crate) fn project_view(
    data: &MemoryData,
    query: &str,
    options: ProjectViewOptions,
) -> Result<MemoryProjectReport> {
    let query = query.trim();
    let item_limit = options.item_limit.max(1);
    let recall_limit = options.recall_limit.max(1);
    let review_limit = options.review_limit.max(1);
    let graph = graph::graph_neighborhood(
        data,
        query,
        graph::GraphTraversalOptions {
            max_depth: options.graph_depth,
            limit: options.graph_limit,
        },
    )?;
    let matched_entity = focused_entity(data, query);
    let graph_node_ids = graph
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<BTreeSet<_>>();
    let graph_evidence_ids = graph
        .nodes
        .iter()
        .flat_map(|node| {
            node.evidence_ids
                .iter()
                .chain(node.source_event_ids.iter())
                .map(String::as_str)
        })
        .collect::<BTreeSet<_>>();
    let focus = Focus::new(query, matched_entity.as_ref());

    let mut episodes = associated_episodes(data, &focus, &graph_node_ids, &graph_evidence_ids);
    episodes.sort_by(|left, right| {
        right
            .created_at_ms
            .cmp(&left.created_at_ms)
            .then_with(|| left.id.cmp(&right.id))
    });
    episodes.truncate(item_limit);
    let episode_ids = episodes
        .iter()
        .map(|episode| episode.id.as_str())
        .collect::<BTreeSet<_>>();

    let mut claims = associated_claims(data, &focus, &graph_node_ids, &episode_ids);
    claims.sort_by(|left, right| {
        right
            .confidence
            .partial_cmp(&left.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right.created_at_ms.cmp(&left.created_at_ms))
            .then_with(|| left.id.cmp(&right.id))
    });
    claims.truncate(item_limit);

    let mut links = associated_links(data, &focus, &graph_node_ids, &episode_ids);
    links.sort_by(|left, right| {
        right
            .confidence
            .partial_cmp(&left.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right.created_at_ms.cmp(&left.created_at_ms))
            .then_with(|| left.id.cmp(&right.id))
    });
    links.truncate(item_limit);

    let mut procedures = associated_procedures(data, &focus, &graph_node_ids, &episode_ids);
    procedures.sort_by(|left, right| {
        right
            .confidence
            .partial_cmp(&left.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right.created_at_ms.cmp(&left.created_at_ms))
            .then_with(|| left.id.cmp(&right.id))
    });
    procedures.truncate(item_limit);

    let mut intentions = associated_intentions(data, &focus, &graph_node_ids, &episode_ids);
    intentions.sort_by(|left, right| {
        intention_status_rank(&left.status)
            .cmp(&intention_status_rank(&right.status))
            .then_with(|| right.updated_at_ms.cmp(&left.updated_at_ms))
            .then_with(|| left.id.cmp(&right.id))
    });
    intentions.truncate(item_limit);

    let recall_results = recall::recall_with_options(
        data,
        query,
        RecallOptions {
            limit: recall_limit,
            kinds: Vec::new(),
            require_evidence: false,
            scope: matched_entity
                .as_ref()
                .and_then(|entity| entity.scope.as_ref().cloned()),
        },
    );
    let review_items = associated_review_items(data, &focus, &graph_evidence_ids, review_limit);
    let health = KnowledgeHealth::inspect(data);
    let authority = AuthorityDecision::evaluate(&health);
    let summary = ProjectViewSummary {
        matched_entity: matched_entity.is_some(),
        graph_node_count: graph.summary.node_count,
        graph_edge_count: graph.summary.edge_count,
        recall_result_count: recall_results.len(),
        episode_count: episodes.len(),
        claim_count: claims.len(),
        link_count: links.len(),
        procedure_count: procedures.len(),
        intention_count: intentions.len(),
        review_item_count: review_items.len(),
    };

    Ok(MemoryProjectReport {
        version: MEMORY_PROJECT_VIEW_VERSION,
        query: query.to_string(),
        generated_at_ms: now_ms(),
        event_count: data.event_count,
        matched_entity,
        authority,
        health,
        summary,
        graph,
        recall_results,
        episodes,
        claims,
        links,
        procedures,
        intentions,
        review_items,
    })
}

fn focused_entity(data: &MemoryData, query: &str) -> Option<Entity> {
    let query_key = normalized(query);
    let mut candidates = data
        .entities
        .iter()
        .filter(|entity| normalized(&entity.name) == query_key)
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        candidates = data
            .entities
            .iter()
            .filter(|entity| {
                let entity_key = normalized(&entity.name);
                entity_key.contains(&query_key) || query_key.contains(&entity_key)
            })
            .collect();
    }

    candidates
        .into_iter()
        .max_by(|left, right| {
            left.mention_count
                .cmp(&right.mention_count)
                .then_with(|| right.name.cmp(&left.name))
        })
        .cloned()
}

fn associated_episodes(
    data: &MemoryData,
    focus: &Focus,
    graph_node_ids: &BTreeSet<&str>,
    graph_evidence_ids: &BTreeSet<&str>,
) -> Vec<Episode> {
    data.episodes
        .iter()
        .filter(|episode| {
            graph_node_ids.contains(episode.id.as_str())
                || graph_evidence_ids.contains(episode.id.as_str())
                || graph_evidence_ids.contains(episode.event_id.as_str())
                || focus.matches(&episode.content)
                || episode
                    .mentions
                    .iter()
                    .any(|mention| focus.matches_exact_or_contains(mention))
        })
        .cloned()
        .collect()
}

fn associated_claims(
    data: &MemoryData,
    focus: &Focus,
    graph_node_ids: &BTreeSet<&str>,
    episode_ids: &BTreeSet<&str>,
) -> Vec<Claim> {
    data.claims
        .iter()
        .filter(|claim| {
            graph_node_ids.contains(claim.id.as_str())
                || claim
                    .source_episode_id
                    .as_deref()
                    .is_some_and(|id| episode_ids.contains(id))
                || focus.matches_exact_or_contains(&claim.subject)
                || focus.matches_exact_or_contains(&claim.object)
                || focus.matches(&claim.predicate)
        })
        .cloned()
        .collect()
}

fn associated_links(
    data: &MemoryData,
    focus: &Focus,
    graph_node_ids: &BTreeSet<&str>,
    episode_ids: &BTreeSet<&str>,
) -> Vec<Link> {
    data.links
        .iter()
        .filter(|link| {
            graph_node_ids.contains(link.id.as_str())
                || link
                    .source_episode_id
                    .as_deref()
                    .is_some_and(|id| episode_ids.contains(id))
                || focus.matches_exact_or_contains(&link.from)
                || focus.matches_exact_or_contains(&link.to)
                || focus.matches(&link.relation)
        })
        .cloned()
        .collect()
}

fn associated_procedures(
    data: &MemoryData,
    focus: &Focus,
    graph_node_ids: &BTreeSet<&str>,
    episode_ids: &BTreeSet<&str>,
) -> Vec<Procedure> {
    data.procedures
        .iter()
        .filter(|procedure| {
            graph_node_ids.contains(procedure.id.as_str())
                || procedure
                    .source_episode_id
                    .as_deref()
                    .is_some_and(|id| episode_ids.contains(id))
                || focus.matches(&procedure.name)
                || focus.matches(&procedure.body)
        })
        .cloned()
        .collect()
}

fn associated_intentions(
    data: &MemoryData,
    focus: &Focus,
    graph_node_ids: &BTreeSet<&str>,
    episode_ids: &BTreeSet<&str>,
) -> Vec<Intention> {
    data.intentions
        .iter()
        .filter(|intention| {
            graph_node_ids.contains(intention.id.as_str())
                || intention
                    .source_episode_id
                    .as_deref()
                    .is_some_and(|id| episode_ids.contains(id))
                || focus.matches(&intention.description)
        })
        .cloned()
        .collect()
}

fn associated_review_items(
    data: &MemoryData,
    focus: &Focus,
    graph_evidence_ids: &BTreeSet<&str>,
    limit: usize,
) -> Vec<OperatorReviewItem> {
    let report = operator_review::operator_review(
        data,
        OperatorReviewOptions {
            limit: data.event_count + data.entities.len() + data.episodes.len() + 20,
            min_priority: Some(SelfInspectionReviewPriority::Low),
            ..OperatorReviewOptions::default()
        },
    );
    let mut items = report
        .items
        .into_iter()
        .filter(|item| {
            item.evidence_ids
                .iter()
                .any(|id| graph_evidence_ids.contains(id.as_str()))
                || focus.matches(&item.title)
                || focus.matches(&item.detail)
        })
        .collect::<Vec<_>>();
    items.truncate(limit);
    items
}

#[derive(Debug)]
struct Focus {
    query: String,
    entity: Option<String>,
}

impl Focus {
    fn new(query: &str, entity: Option<&Entity>) -> Self {
        Self {
            query: normalized(query),
            entity: entity.map(|entity| normalized(&entity.name)),
        }
    }

    fn matches(&self, value: &str) -> bool {
        let value = normalized(value);
        !self.query.is_empty()
            && (value.contains(&self.query)
                || self
                    .entity
                    .as_ref()
                    .is_some_and(|entity| value.contains(entity)))
    }

    fn matches_exact_or_contains(&self, value: &str) -> bool {
        let value = normalized(value);
        value == self.query
            || self.query.contains(&value)
            || value.contains(&self.query)
            || self.entity.as_ref().is_some_and(|entity| {
                value == *entity || value.contains(entity) || entity.contains(&value)
            })
    }
}

fn intention_status_rank(status: &IntentionStatus) -> u8 {
    match status {
        IntentionStatus::Active => 0,
        IntentionStatus::Blocked => 1,
        IntentionStatus::Deferred => 2,
        IntentionStatus::Completed => 3,
        IntentionStatus::Abandoned => 4,
    }
}

fn normalized(input: &str) -> String {
    input
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use crate::{
        Claim, Entity, Episode, Intention, IntentionKind, IntentionPriority, IntentionStatus, Link,
        MemoryData,
        project::{MEMORY_PROJECT_VIEW_VERSION, ProjectViewOptions, project_view},
    };

    #[test]
    fn builds_focused_project_dashboard_from_projected_memory() {
        let data = MemoryData {
            event_count: 5,
            entities: vec![
                Entity {
                    id: "entity_lena".to_string(),
                    name: "Lena".to_string(),
                    mention_count: 4,
                    first_seen_at_ms: 1,
                    last_seen_at_ms: 5,
                    source_event_ids: vec!["event_1".to_string(), "event_2".to_string()],
                    scope: None,
                },
                Entity {
                    id: "entity_release_notes".to_string(),
                    name: "Release Notes".to_string(),
                    mention_count: 2,
                    first_seen_at_ms: 2,
                    last_seen_at_ms: 5,
                    source_event_ids: vec!["event_3".to_string()],
                    scope: None,
                },
            ],
            episodes: vec![Episode {
                id: "episode_1".to_string(),
                event_id: "event_1".to_string(),
                content: "Lena owns release notes.".to_string(),
                tags: vec!["product".to_string()],
                mentions: vec!["Lena".to_string()],
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
                object: "Release Notes".to_string(),
                source_episode_id: Some("episode_1".to_string()),
                confidence: 0.95,
                scope: None,
                created_at_ms: 2,
            }],
            links: vec![Link {
                id: "link_1".to_string(),
                event_id: "event_3".to_string(),
                from: "Lena".to_string(),
                relation: "owns".to_string(),
                to: "Release Notes".to_string(),
                source_episode_id: Some("episode_1".to_string()),
                confidence: 0.9,
                scope: None,
                created_at_ms: 3,
            }],
            intentions: vec![Intention {
                id: "intention_1".to_string(),
                event_id: "event_4".to_string(),
                updated_event_id: "event_4".to_string(),
                kind: IntentionKind::Task,
                status: IntentionStatus::Active,
                priority: IntentionPriority::High,
                description: "Ask Lena to publish release notes.".to_string(),
                source_episode_id: Some("episode_1".to_string()),
                status_reason: None,
                deadline_at_ms: None,
                depends_on: Vec::new(),
                goal_id: None,
                progress_percent: None,
                scope: None,
                created_at_ms: 4,
                updated_at_ms: 4,
            }],
            ..MemoryData::default()
        };

        let report = project_view(
            &data,
            "Lena",
            ProjectViewOptions {
                item_limit: 5,
                recall_limit: 5,
                graph_depth: 2,
                graph_limit: 20,
                review_limit: 5,
            },
        )
        .expect("project view builds");

        assert_eq!(report.version, MEMORY_PROJECT_VIEW_VERSION);
        assert_eq!(report.matched_entity.as_ref().unwrap().name, "Lena");
        assert!(report.summary.matched_entity);
        assert_eq!(report.summary.episode_count, 1);
        assert_eq!(report.summary.claim_count, 1);
        assert_eq!(report.summary.link_count, 1);
        assert_eq!(report.summary.intention_count, 1);
        assert!(report.summary.graph_node_count >= 3);
        assert!(
            report
                .recall_results
                .iter()
                .any(|result| result.kind == crate::MemoryKind::Claim)
        );
    }

    #[test]
    fn returns_empty_focus_without_inventing_memory() {
        let data = MemoryData::default();
        let report =
            project_view(&data, "Unknown", ProjectViewOptions::default()).expect("view builds");

        assert!(report.matched_entity.is_none());
        assert!(!report.summary.matched_entity);
        assert_eq!(report.summary.graph_node_count, 0);
        assert!(report.episodes.is_empty());
        assert!(report.claims.is_empty());
        assert!(report.links.is_empty());
        assert!(report.recall_results.is_empty());
    }
}
