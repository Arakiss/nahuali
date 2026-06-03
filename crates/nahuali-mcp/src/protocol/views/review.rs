use nahuali_core::ReviewResolutionReport;
use rmcp::schemars;
use serde::Serialize;

use super::{OperatorReviewItemView, json_string};

/// Result of planning or applying an operator-approved review resolution.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct ReviewResolutionReportView {
    version: u32,
    dry_run: bool,
    applied: bool,
    review_id: String,
    finding_id: String,
    outcome: String,
    note: String,
    evidence_ids: Vec<String>,
    review_item: OperatorReviewItemView,
    decision_id: Option<String>,
    event_id: Option<String>,
    policy: String,
}

impl From<ReviewResolutionReport> for ReviewResolutionReportView {
    fn from(report: ReviewResolutionReport) -> Self {
        Self {
            outcome: json_string(&report.outcome),
            version: report.version,
            dry_run: report.dry_run,
            applied: report.applied,
            review_id: report.review_id,
            finding_id: report.finding_id,
            note: report.note,
            evidence_ids: report.evidence_ids,
            review_item: OperatorReviewItemView::from(report.review_item),
            decision_id: report.decision_id,
            event_id: report.event_id,
            policy: report.policy,
        }
    }
}
