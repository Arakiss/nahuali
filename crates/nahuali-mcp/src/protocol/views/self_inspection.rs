use nahuali_core::{
    SelfInspectionFinding, SelfInspectionReport, SelfInspectionReviewItem, SelfInspectionSummary,
};
use rmcp::schemars;
use serde::Serialize;

use super::{AuthorityDecisionView, HealthView, WriteBackPolicyView, json_string};

/// Aggregate counts for a self-inspection report.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct SelfInspectionSummaryView {
    distinct_flagged_record_count: usize,
    finding_count: usize,
    contradiction_count: usize,
    stale_memory_count: usize,
    blind_spot_count: usize,
    weak_evidence_count: usize,
    source_coverage_count: usize,
    low_confidence_count: usize,
    consolidation_opportunity_count: usize,
    latent_intention_count: usize,
    high_priority_review_count: usize,
}

impl From<SelfInspectionSummary> for SelfInspectionSummaryView {
    fn from(summary: SelfInspectionSummary) -> Self {
        Self {
            distinct_flagged_record_count: summary.distinct_flagged_record_count,
            finding_count: summary.finding_count,
            contradiction_count: summary.contradiction_count,
            stale_memory_count: summary.stale_memory_count,
            blind_spot_count: summary.blind_spot_count,
            weak_evidence_count: summary.weak_evidence_count,
            source_coverage_count: summary.source_coverage_count,
            low_confidence_count: summary.low_confidence_count,
            consolidation_opportunity_count: summary.consolidation_opportunity_count,
            latent_intention_count: summary.latent_intention_count,
            high_priority_review_count: summary.high_priority_review_count,
        }
    }
}

/// A single self-inspection finding with evidence IDs.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct SelfInspectionFindingView {
    id: String,
    kind: String,
    severity: String,
    title: String,
    detail: String,
    dimensions: Vec<String>,
    evidence_ids: Vec<String>,
    suggested_action: String,
}

impl From<SelfInspectionFinding> for SelfInspectionFindingView {
    fn from(finding: SelfInspectionFinding) -> Self {
        Self {
            id: finding.id,
            kind: json_string(&finding.kind),
            severity: json_string(&finding.severity),
            title: finding.title,
            detail: finding.detail,
            dimensions: finding.dimensions.iter().map(json_string).collect(),
            evidence_ids: finding.evidence_ids,
            suggested_action: finding.suggested_action,
        }
    }
}

/// A proposed self-inspection review item.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct SelfInspectionReviewItemView {
    id: String,
    finding_id: String,
    priority: String,
    action: String,
    status: String,
    title: String,
    detail: String,
    evidence_ids: Vec<String>,
}

impl From<SelfInspectionReviewItem> for SelfInspectionReviewItemView {
    fn from(item: SelfInspectionReviewItem) -> Self {
        Self {
            id: item.id,
            finding_id: item.finding_id,
            priority: json_string(&item.priority),
            action: json_string(&item.action),
            status: json_string(&item.status),
            title: item.title,
            detail: item.detail,
            evidence_ids: item.evidence_ids,
        }
    }
}

/// Structured self-inspection report surfacing health, authority, and findings.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct SelfInspectionReportView {
    version: u32,
    generated_at_ms: u64,
    event_count: usize,
    health: HealthView,
    authority: AuthorityDecisionView,
    summary: SelfInspectionSummaryView,
    findings: Vec<SelfInspectionFindingView>,
    review_queue: Vec<SelfInspectionReviewItemView>,
    write_back_policy: WriteBackPolicyView,
}

impl From<SelfInspectionReport> for SelfInspectionReportView {
    fn from(report: SelfInspectionReport) -> Self {
        Self {
            version: report.version,
            generated_at_ms: report.generated_at_ms,
            event_count: report.event_count,
            health: HealthView::from(report.health),
            authority: AuthorityDecisionView::from(report.authority),
            summary: SelfInspectionSummaryView::from(report.summary),
            findings: report
                .findings
                .into_iter()
                .map(SelfInspectionFindingView::from)
                .collect(),
            review_queue: report
                .review_queue
                .into_iter()
                .map(SelfInspectionReviewItemView::from)
                .collect(),
            write_back_policy: WriteBackPolicyView::from(report.write_back_policy),
        }
    }
}
