use serde::{Deserialize, Serialize};

use crate::{
    MemoryData, NahualiError, OperatorReviewItem, OperatorReviewOptions, ReviewDecisionOutcome,
    SelfInspectionReviewAction,
    event::{ReviewRecorded, ReviewRecordedAction, ReviewRecordedOutcome},
    operator_review,
};

/// Current operator review-resolution report format version.
pub const REVIEW_RESOLUTION_VERSION: u32 = 1;

/// Options for explicitly resolving an operator review item.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ReviewResolutionOptions {
    /// Operator review item identifier to resolve.
    pub review_id: String,
    /// Operator-supplied resolution note.
    pub note: String,
    /// Whether to preview the resolution without writing a record.
    pub dry_run: bool,
}

/// Result of planning or applying an operator-approved review resolution.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ReviewResolutionReport {
    /// Report format version.
    pub version: u32,
    /// Whether this report was generated without writing a record.
    pub dry_run: bool,
    /// Whether an append-only review decision was written.
    pub applied: bool,
    /// Operator review item identifier that was handled.
    pub review_id: String,
    /// Self-inspection finding identifier that produced the review item.
    pub finding_id: String,
    /// Operator-selected outcome.
    pub outcome: ReviewDecisionOutcome,
    /// Operator-supplied resolution note.
    pub note: String,
    /// Event or memory identifiers covered by this review decision.
    pub evidence_ids: Vec<String>,
    /// Review item that was resolved.
    pub review_item: OperatorReviewItem,
    /// Review decision identifier, when known.
    pub decision_id: Option<String>,
    /// Event identifier appended to the record ledger, if applied.
    pub event_id: Option<String>,
    /// Non-mutating policy statement included for agent clients.
    pub policy: String,
}

pub(crate) struct PreparedReviewResolution {
    pub report: ReviewResolutionReport,
    pub event: ReviewRecorded,
}

pub(crate) fn prepare_review_resolution(
    data: &MemoryData,
    options: ReviewResolutionOptions,
    decision_id: String,
) -> Result<PreparedReviewResolution, NahualiError> {
    let review_id = options.review_id.trim().to_string();
    if review_id.is_empty() {
        return Err(NahualiError::EmptyContent);
    }

    let note = options.note.trim().to_string();
    if note.is_empty() {
        return Err(NahualiError::EmptyContent);
    }

    let review = operator_review::operator_review(
        data,
        OperatorReviewOptions {
            limit: usize::MAX,
            min_priority: None,
            ..OperatorReviewOptions::default()
        },
    );
    let item = review
        .items
        .into_iter()
        .find(|item| item.id == review_id)
        .ok_or_else(|| NahualiError::UnknownReviewItem {
            id: review_id.clone(),
        })?;

    let event = ReviewRecorded {
        id: decision_id.clone(),
        review_id: item.id.clone(),
        finding_id: item.finding_id.clone(),
        action: review_action(&item.action),
        outcome: ReviewRecordedOutcome::Resolved,
        note: note.clone(),
        evidence_ids: item.evidence_ids.clone(),
        scope: None,
    };
    let report = ReviewResolutionReport {
        version: REVIEW_RESOLUTION_VERSION,
        dry_run: options.dry_run,
        applied: false,
        review_id: item.id.clone(),
        finding_id: item.finding_id.clone(),
        outcome: ReviewDecisionOutcome::Resolved,
        note,
        evidence_ids: item.evidence_ids.clone(),
        review_item: item,
        decision_id: Some(decision_id),
        event_id: None,
        policy: "review resolutions require explicit operator approval and are stored as append-only audit events"
            .to_string(),
    };

    Ok(PreparedReviewResolution { report, event })
}

fn review_action(action: &SelfInspectionReviewAction) -> ReviewRecordedAction {
    match action {
        SelfInspectionReviewAction::CaptureEvidence => ReviewRecordedAction::CaptureEvidence,
        SelfInspectionReviewAction::ResolveContradiction => {
            ReviewRecordedAction::ResolveContradiction
        }
        SelfInspectionReviewAction::RefreshMemory => ReviewRecordedAction::RefreshMemory,
        SelfInspectionReviewAction::LinkMemory => ReviewRecordedAction::LinkMemory,
        SelfInspectionReviewAction::ConsolidatePattern => ReviewRecordedAction::ConsolidatePattern,
        SelfInspectionReviewAction::ReviewIntention => ReviewRecordedAction::ReviewIntention,
    }
}
