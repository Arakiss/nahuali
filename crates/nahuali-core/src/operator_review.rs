use serde::{Deserialize, Serialize};

use crate::{
    AuthorityDecision, HealthDimension, HealthSeverity, MemoryData, SelfInspectionFindingKind,
    SelfInspectionReviewAction, SelfInspectionReviewItem, SelfInspectionReviewPriority,
    SelfInspectionReviewStatus, SelfInspectionWriteBackPolicy,
    self_inspection::{self, SelfInspectionReport},
};

/// Current operator-review report format version.
pub const OPERATOR_REVIEW_VERSION: u32 = 1;

/// Options for building an operator review queue.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct OperatorReviewOptions {
    /// Maximum number of items returned in the displayed queue.
    pub limit: usize,
    /// Minimum priority to include. `High` includes critical and high items.
    pub min_priority: Option<SelfInspectionReviewPriority>,
    /// Proposed review action to include.
    pub action: Option<SelfInspectionReviewAction>,
}

impl Default for OperatorReviewOptions {
    fn default() -> Self {
        Self {
            limit: 20,
            min_priority: None,
            action: None,
        }
    }
}

/// Non-mutating operator review queue derived from self-inspection.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct OperatorReviewReport {
    /// Report format version.
    pub version: u32,
    /// Timestamp in milliseconds when the report was generated.
    pub generated_at_ms: u64,
    /// Number of source events represented by the reviewed projection.
    pub event_count: usize,
    /// Total queue items after priority filtering and before display limiting.
    pub total_items: usize,
    /// Number of queue items returned in `items`.
    pub displayed_items: usize,
    /// Minimum priority used to filter the queue, if any.
    pub min_priority: Option<SelfInspectionReviewPriority>,
    /// Proposed review action used to filter the queue, if any.
    pub action: Option<SelfInspectionReviewAction>,
    /// Authority decision computed from the same projection.
    pub authority: AuthorityDecision,
    /// Aggregate review counts across all filtered items.
    pub summary: OperatorReviewSummary,
    /// Prioritized operator queue.
    pub items: Vec<OperatorReviewItem>,
    /// Explicit policy for automatic write-back.
    pub write_back_policy: SelfInspectionWriteBackPolicy,
}

/// Aggregate counts for an operator review queue.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct OperatorReviewSummary {
    /// Total number of filtered queue items.
    pub item_count: usize,
    /// Critical priority queue items.
    pub critical_count: usize,
    /// High priority queue items.
    pub high_count: usize,
    /// Medium priority queue items.
    pub medium_count: usize,
    /// Low priority queue items.
    pub low_count: usize,
    /// Queue items that require evidence capture.
    pub capture_evidence_count: usize,
    /// Queue items that require contradiction resolution.
    pub resolve_contradiction_count: usize,
    /// Queue items that require refreshing stale memory.
    pub refresh_memory_count: usize,
    /// Queue items that require linking disconnected memory.
    pub link_memory_count: usize,
    /// Queue items that require consolidating repeated patterns.
    pub consolidate_pattern_count: usize,
    /// Queue items that require intention review.
    pub review_intention_count: usize,
}

/// A single prioritized operator review item.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct OperatorReviewItem {
    /// Stable identifier within this report.
    pub id: String,
    /// Self-inspection finding identifier that created this item.
    pub finding_id: String,
    /// Finding category that produced the item.
    pub finding_kind: SelfInspectionFindingKind,
    /// Priority assigned by self-inspection.
    pub priority: SelfInspectionReviewPriority,
    /// Deterministic score used for ordering within priority bands.
    pub score: u32,
    /// Proposed operator action.
    pub action: SelfInspectionReviewAction,
    /// Review lifecycle status.
    pub status: SelfInspectionReviewStatus,
    /// Short review title.
    pub title: String,
    /// Review detail from the self-inspection finding.
    pub detail: String,
    /// Health severity of the source finding.
    pub source_severity: HealthSeverity,
    /// Health dimensions affected by the source finding.
    pub dimensions: Vec<HealthDimension>,
    /// Event or memory identifiers supporting the review item.
    pub evidence_ids: Vec<String>,
    /// Concrete, non-mutating operator guidance.
    pub operator_guidance: String,
}

pub(crate) fn operator_review(
    data: &MemoryData,
    options: OperatorReviewOptions,
) -> OperatorReviewReport {
    let report = self_inspection::self_inspect(data);
    operator_review_from_self_inspection(report, options)
}

pub(crate) fn operator_review_from_self_inspection(
    report: SelfInspectionReport,
    options: OperatorReviewOptions,
) -> OperatorReviewReport {
    let limit = options.limit.max(1);
    let mut items = report
        .review_queue
        .iter()
        .filter(|item| priority_allowed(&item.priority, options.min_priority.as_ref()))
        .filter(|item| action_allowed(&item.action, options.action.as_ref()))
        .filter_map(|item| operator_item(item, &report))
        .collect::<Vec<_>>();
    sort_items(&mut items);

    let total_items = items.len();
    let summary = summarize(&items);
    items.truncate(limit);

    OperatorReviewReport {
        version: OPERATOR_REVIEW_VERSION,
        generated_at_ms: report.generated_at_ms,
        event_count: report.event_count,
        total_items,
        displayed_items: items.len(),
        min_priority: options.min_priority,
        action: options.action,
        authority: report.authority,
        summary,
        items,
        write_back_policy: report.write_back_policy,
    }
}

fn operator_item(
    item: &SelfInspectionReviewItem,
    report: &SelfInspectionReport,
) -> Option<OperatorReviewItem> {
    let finding = report
        .findings
        .iter()
        .find(|finding| finding.id == item.finding_id)?;

    Some(OperatorReviewItem {
        id: item.id.clone(),
        finding_id: item.finding_id.clone(),
        finding_kind: finding.kind.clone(),
        priority: item.priority.clone(),
        score: score_item(item, finding.source_severity()),
        action: item.action.clone(),
        status: item.status.clone(),
        title: item.title.clone(),
        detail: finding.detail.clone(),
        source_severity: finding.severity.clone(),
        dimensions: finding.dimensions.clone(),
        evidence_ids: item.evidence_ids.clone(),
        operator_guidance: operator_guidance(&item.action).to_string(),
    })
}

trait FindingSeverity {
    fn source_severity(&self) -> &HealthSeverity;
}

impl FindingSeverity for crate::SelfInspectionFinding {
    fn source_severity(&self) -> &HealthSeverity {
        &self.severity
    }
}

fn action_allowed(
    action: &SelfInspectionReviewAction,
    filter: Option<&SelfInspectionReviewAction>,
) -> bool {
    filter.map(|filter| action == filter).unwrap_or(true)
}

fn priority_allowed(
    priority: &SelfInspectionReviewPriority,
    min_priority: Option<&SelfInspectionReviewPriority>,
) -> bool {
    min_priority
        .map(|min_priority| priority_rank(priority) <= priority_rank(min_priority))
        .unwrap_or(true)
}

fn sort_items(items: &mut [OperatorReviewItem]) {
    items.sort_by(|left, right| {
        priority_rank(&left.priority)
            .cmp(&priority_rank(&right.priority))
            .then_with(|| right.score.cmp(&left.score))
            .then_with(|| left.title.cmp(&right.title))
            .then_with(|| left.id.cmp(&right.id))
    });
}

fn score_item(item: &SelfInspectionReviewItem, severity: &HealthSeverity) -> u32 {
    priority_score(&item.priority)
        + severity_score(severity)
        + item.evidence_ids.len().min(5) as u32
}

fn priority_rank(priority: &SelfInspectionReviewPriority) -> u8 {
    match priority {
        SelfInspectionReviewPriority::Critical => 0,
        SelfInspectionReviewPriority::High => 1,
        SelfInspectionReviewPriority::Medium => 2,
        SelfInspectionReviewPriority::Low => 3,
    }
}

fn priority_score(priority: &SelfInspectionReviewPriority) -> u32 {
    match priority {
        SelfInspectionReviewPriority::Critical => 100,
        SelfInspectionReviewPriority::High => 80,
        SelfInspectionReviewPriority::Medium => 60,
        SelfInspectionReviewPriority::Low => 40,
    }
}

fn severity_score(severity: &HealthSeverity) -> u32 {
    match severity {
        HealthSeverity::High => 9,
        HealthSeverity::Medium => 6,
        HealthSeverity::Low => 3,
    }
}

fn operator_guidance(action: &SelfInspectionReviewAction) -> &'static str {
    match action {
        SelfInspectionReviewAction::CaptureEvidence => {
            "Record or attach source evidence before relying on this memory."
        }
        SelfInspectionReviewAction::ResolveContradiction => {
            "Review the conflicting evidence, record the resolution as a new episode, then add corrected claims explicitly."
        }
        SelfInspectionReviewAction::RefreshMemory => {
            "Record a fresh observation before continuing to rely on this stale memory."
        }
        SelfInspectionReviewAction::LinkMemory => {
            "Add evidence-backed links or context so this memory is no longer isolated."
        }
        SelfInspectionReviewAction::ConsolidatePattern => {
            "Consolidate repeated observations only after confirming the pattern is durable."
        }
        SelfInspectionReviewAction::ReviewIntention => {
            "Review the intention and explicitly update, defer, block, complete, or abandon it."
        }
    }
}

fn summarize(items: &[OperatorReviewItem]) -> OperatorReviewSummary {
    OperatorReviewSummary {
        item_count: items.len(),
        critical_count: count_priority(items, SelfInspectionReviewPriority::Critical),
        high_count: count_priority(items, SelfInspectionReviewPriority::High),
        medium_count: count_priority(items, SelfInspectionReviewPriority::Medium),
        low_count: count_priority(items, SelfInspectionReviewPriority::Low),
        capture_evidence_count: count_action(items, SelfInspectionReviewAction::CaptureEvidence),
        resolve_contradiction_count: count_action(
            items,
            SelfInspectionReviewAction::ResolveContradiction,
        ),
        refresh_memory_count: count_action(items, SelfInspectionReviewAction::RefreshMemory),
        link_memory_count: count_action(items, SelfInspectionReviewAction::LinkMemory),
        consolidate_pattern_count: count_action(
            items,
            SelfInspectionReviewAction::ConsolidatePattern,
        ),
        review_intention_count: count_action(items, SelfInspectionReviewAction::ReviewIntention),
    }
}

fn count_priority(items: &[OperatorReviewItem], priority: SelfInspectionReviewPriority) -> usize {
    items
        .iter()
        .filter(|item| item.priority == priority)
        .count()
}

fn count_action(items: &[OperatorReviewItem], action: SelfInspectionReviewAction) -> usize {
    items.iter().filter(|item| item.action == action).count()
}

#[cfg(test)]
mod tests {
    use crate::{
        Claim, Entity, MemoryData, OPERATOR_REVIEW_VERSION, OperatorReviewOptions,
        SelfInspectionReviewAction, SelfInspectionReviewPriority, operator_review::operator_review,
    };

    #[test]
    fn prioritizes_critical_review_items() {
        let data = MemoryData {
            event_count: 2,
            entities: vec![entity("Lena")],
            claims: vec![
                claim("claim_1", "event_1", "Lena", "role", "CTO", 0.4),
                claim("claim_2", "event_2", "Lena", "role", "VP Engineering", 0.9),
            ],
            facts: vec![
                claim("claim_1", "event_1", "Lena", "role", "CTO", 0.4),
                claim("claim_2", "event_2", "Lena", "role", "VP Engineering", 0.9),
            ],
            ..MemoryData::default()
        };

        let report = operator_review(
            &data,
            OperatorReviewOptions {
                limit: 1,
                min_priority: Some(SelfInspectionReviewPriority::High),
                ..OperatorReviewOptions::default()
            },
        );

        assert_eq!(report.version, OPERATOR_REVIEW_VERSION);
        assert!(!report.write_back_policy.automatic_write_back);
        assert_eq!(report.displayed_items, 1);
        assert!(report.total_items >= 1);
        assert_eq!(
            report.items[0].priority,
            SelfInspectionReviewPriority::Critical
        );
        assert_eq!(
            report.items[0].action,
            SelfInspectionReviewAction::ResolveContradiction
        );
        assert!(
            report.items[0]
                .operator_guidance
                .contains("record the resolution")
        );

        let filtered = operator_review(
            &data,
            OperatorReviewOptions {
                limit: 10,
                action: Some(SelfInspectionReviewAction::ResolveContradiction),
                ..OperatorReviewOptions::default()
            },
        );
        assert_eq!(
            filtered.action,
            Some(SelfInspectionReviewAction::ResolveContradiction)
        );
        assert!(
            filtered
                .items
                .iter()
                .all(|item| { item.action == SelfInspectionReviewAction::ResolveContradiction })
        );
    }

    fn entity(name: &str) -> Entity {
        Entity {
            id: format!("entity_{}", name.to_ascii_lowercase().replace(' ', "_")),
            name: name.to_string(),
            mention_count: 1,
            first_seen_at_ms: 0,
            last_seen_at_ms: 0,
            source_event_ids: Vec::new(),
            scope: None,
        }
    }

    fn claim(
        id: &str,
        event_id: &str,
        subject: &str,
        predicate: &str,
        object: &str,
        confidence: f32,
    ) -> Claim {
        Claim {
            id: id.to_string(),
            event_id: event_id.to_string(),
            subject: subject.to_string(),
            predicate: predicate.to_string(),
            object: object.to_string(),
            source_episode_id: None,
            confidence,
            scope: None,
            created_at_ms: 0,
        }
    }
}
