use nahuali_core::{OperatorReviewItem, OperatorReviewReport, OperatorReviewSummary};
use rmcp::schemars;
use serde::Serialize;

use super::{AuthorityDecisionView, WriteBackPolicyView, json_string};

/// Prioritized operator review item, shared by the briefing and review tools.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct OperatorReviewItemView {
    id: String,
    finding_id: String,
    finding_kind: String,
    priority: String,
    score: u32,
    action: String,
    status: String,
    title: String,
    detail: String,
    source_severity: String,
    dimensions: Vec<String>,
    evidence_ids: Vec<String>,
    operator_guidance: String,
}

impl From<OperatorReviewItem> for OperatorReviewItemView {
    fn from(item: OperatorReviewItem) -> Self {
        Self {
            id: item.id,
            finding_id: item.finding_id,
            finding_kind: json_string(&item.finding_kind),
            priority: json_string(&item.priority),
            score: item.score,
            action: json_string(&item.action),
            status: json_string(&item.status),
            title: item.title,
            detail: item.detail,
            source_severity: json_string(&item.source_severity),
            dimensions: item.dimensions.iter().map(json_string).collect(),
            evidence_ids: item.evidence_ids,
            operator_guidance: item.operator_guidance,
        }
    }
}

/// Aggregate counts for the operator review queue.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct OperatorReviewSummaryView {
    item_count: usize,
    critical_count: usize,
    high_count: usize,
    medium_count: usize,
    low_count: usize,
    capture_evidence_count: usize,
    resolve_contradiction_count: usize,
    refresh_memory_count: usize,
    link_memory_count: usize,
    consolidate_pattern_count: usize,
    review_intention_count: usize,
}

impl From<OperatorReviewSummary> for OperatorReviewSummaryView {
    fn from(summary: OperatorReviewSummary) -> Self {
        Self {
            item_count: summary.item_count,
            critical_count: summary.critical_count,
            high_count: summary.high_count,
            medium_count: summary.medium_count,
            low_count: summary.low_count,
            capture_evidence_count: summary.capture_evidence_count,
            resolve_contradiction_count: summary.resolve_contradiction_count,
            refresh_memory_count: summary.refresh_memory_count,
            link_memory_count: summary.link_memory_count,
            consolidate_pattern_count: summary.consolidate_pattern_count,
            review_intention_count: summary.review_intention_count,
        }
    }
}

/// Structured operator-review report surfacing authority, summary, and queue.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct OperatorReviewReportView {
    version: u32,
    generated_at_ms: u64,
    event_count: usize,
    total_items: usize,
    displayed_items: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_priority: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    action: Option<String>,
    authority: AuthorityDecisionView,
    summary: OperatorReviewSummaryView,
    items: Vec<OperatorReviewItemView>,
    write_back_policy: WriteBackPolicyView,
}

impl From<OperatorReviewReport> for OperatorReviewReportView {
    fn from(report: OperatorReviewReport) -> Self {
        Self {
            version: report.version,
            generated_at_ms: report.generated_at_ms,
            event_count: report.event_count,
            total_items: report.total_items,
            displayed_items: report.displayed_items,
            min_priority: report.min_priority.as_ref().map(json_string),
            action: report.action.as_ref().map(json_string),
            authority: AuthorityDecisionView::from(report.authority),
            summary: OperatorReviewSummaryView::from(report.summary),
            items: report
                .items
                .into_iter()
                .map(OperatorReviewItemView::from)
                .collect(),
            write_back_policy: WriteBackPolicyView::from(report.write_back_policy),
        }
    }
}
