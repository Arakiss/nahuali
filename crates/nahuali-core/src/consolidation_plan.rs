use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    AuthorityDecision, KnowledgeHealth, MemoryData, MemorySleepReport, OperatorReviewOptions,
    OperatorReviewReport, ReflectionOptions, SelfInspectionReviewAction,
    SelfInspectionReviewPriority, SelfInspectionWriteBackPolicy, SleepModeOptions, operator_review,
    sleep,
};

/// Current consolidation-plan report format version.
pub const MEMORY_CONSOLIDATION_PLAN_VERSION: u32 = 1;

/// Options for building a non-mutating memory consolidation plan.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ConsolidationPlanOptions {
    /// Maximum recent episodes included in replay operations.
    pub recent_episode_limit: usize,
    /// Maximum Sleep Mode consolidation candidates included.
    pub candidate_limit: usize,
    /// Maximum grouped reflection cycles included by Sleep Mode.
    pub cycle_limit: usize,
    /// Maximum evidence identifiers included per reflection cycle.
    pub evidence_limit: usize,
    /// Maximum operator review items included as gates.
    pub review_limit: usize,
}

impl Default for ConsolidationPlanOptions {
    fn default() -> Self {
        Self {
            recent_episode_limit: 8,
            candidate_limit: 12,
            cycle_limit: 8,
            evidence_limit: 8,
            review_limit: 20,
        }
    }
}

/// Non-mutating plan for replaying, extracting, reconciling, and gating memory work.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct MemoryConsolidationPlanReport {
    /// Report format version.
    pub version: u32,
    /// Timestamp in milliseconds when the report was generated.
    pub generated_at_ms: u64,
    /// Number of source events represented by the projection.
    pub event_count: usize,
    /// Projection-level authority decision.
    pub authority: AuthorityDecision,
    /// Knowledge-health report used by the plan.
    pub health: KnowledgeHealth,
    /// Aggregate operation counts.
    pub summary: ConsolidationPlanSummary,
    /// Deterministic pipeline stages.
    pub stages: Vec<ConsolidationStage>,
    /// Concrete non-mutating operations proposed by the plan.
    pub operations: Vec<ConsolidationOperation>,
    /// Operations that cannot become memory writes without more review or policy changes.
    pub blocked_items: Vec<ConsolidationBlockedItem>,
    /// Sleep Mode report used as a planning input.
    pub sleep: MemorySleepReport,
    /// Operator review report used as the write-back gate.
    pub review: OperatorReviewReport,
    /// Explicit policy for automatic write-back.
    pub write_back_policy: SelfInspectionWriteBackPolicy,
}

/// Aggregate counts for a consolidation plan.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ConsolidationPlanSummary {
    /// Number of pipeline stages.
    pub stage_count: usize,
    /// Number of operations in the plan.
    pub operation_count: usize,
    /// Number of replay operations.
    pub replay_operation_count: usize,
    /// Number of extraction candidate operations.
    pub extract_candidate_count: usize,
    /// Number of reconciliation operations.
    pub reconcile_candidate_count: usize,
    /// Number of explicit operator review gates.
    pub review_gate_count: usize,
    /// Number of commit operations ready to write.
    pub commit_ready_count: usize,
    /// Number of blocked operations.
    pub blocked_operation_count: usize,
    /// Number of operations waiting for operator review.
    pub needs_review_operation_count: usize,
    /// Whether this plan authorizes automatic write-back.
    pub automatic_write_back: bool,
}

/// Pipeline stage in a consolidation plan.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ConsolidationStage {
    /// Stable stage identifier.
    pub id: String,
    /// Stage status.
    pub status: ConsolidationStageStatus,
    /// Human-readable title.
    pub title: String,
    /// Stage detail.
    pub detail: String,
    /// Operation identifiers belonging to this stage.
    pub operation_ids: Vec<String>,
}

/// Consolidation pipeline stage status.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConsolidationStageStatus {
    /// The stage has non-mutating work ready to inspect.
    Ready,
    /// The stage found no work.
    Clear,
    /// The stage is waiting for explicit operator review.
    NeedsReview,
    /// The stage is blocked by policy or missing prerequisite work.
    Blocked,
}

/// Concrete non-mutating operation proposed by a consolidation plan.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ConsolidationOperation {
    /// Stable operation identifier.
    pub id: String,
    /// Operation category.
    pub kind: ConsolidationOperationKind,
    /// Operation status.
    pub status: ConsolidationOperationStatus,
    /// Priority when the operation comes from a review or consolidation item.
    pub priority: Option<SelfInspectionReviewPriority>,
    /// Proposed operator action when applicable.
    pub action: Option<SelfInspectionReviewAction>,
    /// Human-readable title.
    pub title: String,
    /// Why the operation exists.
    pub rationale: String,
    /// Event or memory identifiers supporting the operation.
    pub evidence_ids: Vec<String>,
    /// Write-back gate attached to this operation.
    pub gate: ConsolidationGate,
}

/// Consolidation operation category.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConsolidationOperationKind {
    /// Replay recent evidence before proposing changes.
    ReplayEvidence,
    /// Extract a candidate from existing evidence.
    ExtractCandidate,
    /// Reconcile a candidate against existing memory and operator intent.
    ReconcileCandidate,
    /// Route work through explicit operator review.
    ReviewGate,
    /// Decide whether any operation is eligible for write-back.
    CommitEligibility,
}

/// Consolidation operation status.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConsolidationOperationStatus {
    /// Non-mutating work is ready to inspect.
    Ready,
    /// No work is present for this operation.
    Clear,
    /// Explicit operator review is required before a write can exist.
    NeedsReview,
    /// Policy or missing prerequisites prevent write-back.
    Blocked,
}

/// Write-back gate attached to a consolidation operation.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ConsolidationGate {
    /// Whether an operator must review this operation before any write-back.
    pub requires_operator_review: bool,
    /// Whether this operation writes records automatically.
    pub automatic_write_back: bool,
    /// Human-readable gate reason.
    pub reason: String,
}

/// Operation that cannot become a memory write without more review or policy changes.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ConsolidationBlockedItem {
    /// Blocked operation identifier.
    pub operation_id: String,
    /// Operation status causing the block.
    pub status: ConsolidationOperationStatus,
    /// Blocking reason.
    pub reason: String,
    /// Evidence identifiers attached to the blocked operation.
    pub evidence_ids: Vec<String>,
}

pub(crate) fn consolidation_plan(
    data: &MemoryData,
    options: ConsolidationPlanOptions,
) -> MemoryConsolidationPlanReport {
    consolidation_plan_at(data, options, now_ms())
}

pub(crate) fn consolidation_plan_at(
    data: &MemoryData,
    options: ConsolidationPlanOptions,
    generated_at_ms: u64,
) -> MemoryConsolidationPlanReport {
    let sleep = sleep::sleep_mode_at(
        data,
        SleepModeOptions {
            recent_episode_limit: options.recent_episode_limit,
            candidate_limit: options.candidate_limit,
            reflection: ReflectionOptions {
                cycle_limit: options.cycle_limit,
                evidence_limit: options.evidence_limit,
            },
        },
        generated_at_ms,
    );
    let review = operator_review::operator_review(
        data,
        OperatorReviewOptions {
            limit: options.review_limit,
            min_priority: None,
            ..OperatorReviewOptions::default()
        },
    );
    let operations = operations_from_reports(&sleep, &review);
    let stages = stages_from_operations(&operations);
    let blocked_items = blocked_items(&operations);
    let summary = summarize(
        &stages,
        &operations,
        sleep.write_back_policy.automatic_write_back,
    );

    MemoryConsolidationPlanReport {
        version: MEMORY_CONSOLIDATION_PLAN_VERSION,
        generated_at_ms,
        event_count: data.event_count,
        authority: sleep.authority.clone(),
        health: sleep.health.clone(),
        summary,
        stages,
        operations,
        blocked_items,
        write_back_policy: sleep.write_back_policy.clone(),
        sleep,
        review,
    }
}

fn operations_from_reports(
    sleep: &MemorySleepReport,
    review: &OperatorReviewReport,
) -> Vec<ConsolidationOperation> {
    let mut operations = Vec::new();

    if !sleep.recent_episodes.is_empty() {
        let evidence_ids = sleep
            .recent_episodes
            .iter()
            .map(|episode| episode.id.clone())
            .collect();
        operations.push(operation(
            "plan_replay_recent_episodes",
            ConsolidationOperationKind::ReplayEvidence,
            ConsolidationOperationStatus::Ready,
            None,
            None,
            "Replay recent evidence",
            format!(
                "Replay {} recent episode(s) before proposing consolidation work.",
                sleep.recent_episodes.len()
            ),
            evidence_ids,
            "Replay is read-only and does not write memory records.",
        ));
    }

    for candidate in &sleep.consolidation_candidates {
        operations.push(operation(
            format!("plan_extract_{}", candidate.id),
            ConsolidationOperationKind::ExtractCandidate,
            ConsolidationOperationStatus::Ready,
            Some(candidate.priority.clone()),
            Some(candidate.action.clone()),
            format!("Extract candidate: {}", candidate.title),
            candidate.rationale.clone(),
            candidate.evidence_ids.clone(),
            "Extraction is candidate-only; it must be reconciled before any write-back.",
        ));
        operations.push(operation(
            format!("plan_reconcile_{}", candidate.id),
            ConsolidationOperationKind::ReconcileCandidate,
            ConsolidationOperationStatus::NeedsReview,
            Some(candidate.priority.clone()),
            Some(candidate.action.clone()),
            format!("Reconcile candidate: {}", candidate.title),
            "Compare the candidate against current memory and operator intent before accepting it."
                .to_string(),
            candidate.evidence_ids.clone(),
            "Reconciliation requires explicit operator review before memory can change.",
        ));
    }

    for item in &review.items {
        operations.push(operation(
            format!("plan_review_gate_{}", item.id),
            ConsolidationOperationKind::ReviewGate,
            ConsolidationOperationStatus::NeedsReview,
            Some(item.priority.clone()),
            Some(item.action.clone()),
            format!("Review gate: {}", item.title),
            item.operator_guidance.clone(),
            item.evidence_ids.clone(),
            "Operator review is the write-back boundary for this memory work.",
        ));
    }

    operations.push(commit_operation(&operations));
    operations
}

#[allow(clippy::too_many_arguments)]
fn operation(
    id: impl Into<String>,
    kind: ConsolidationOperationKind,
    status: ConsolidationOperationStatus,
    priority: Option<SelfInspectionReviewPriority>,
    action: Option<SelfInspectionReviewAction>,
    title: impl Into<String>,
    rationale: impl Into<String>,
    evidence_ids: Vec<String>,
    gate_reason: impl Into<String>,
) -> ConsolidationOperation {
    ConsolidationOperation {
        id: id.into(),
        kind,
        status,
        priority,
        action,
        title: title.into(),
        rationale: rationale.into(),
        evidence_ids,
        gate: ConsolidationGate {
            requires_operator_review: true,
            automatic_write_back: false,
            reason: gate_reason.into(),
        },
    }
}

fn commit_operation(operations: &[ConsolidationOperation]) -> ConsolidationOperation {
    let pending_review_count = operations
        .iter()
        .filter(|operation| operation.status == ConsolidationOperationStatus::NeedsReview)
        .count();
    let evidence_ids = unique_limited(
        operations
            .iter()
            .flat_map(|operation| operation.evidence_ids.iter().cloned()),
        12,
    );
    let (status, rationale) = if pending_review_count > 0 {
        (
            ConsolidationOperationStatus::NeedsReview,
            format!(
                "{pending_review_count} operation(s) must clear operator review before write-back can be considered."
            ),
        )
    } else {
        (
            ConsolidationOperationStatus::Blocked,
            "Automatic write-back remains disabled; use explicit operator commands to record approved memory."
                .to_string(),
        )
    };

    operation(
        "plan_commit_eligibility",
        ConsolidationOperationKind::CommitEligibility,
        status,
        None,
        None,
        "Hold write-back behind explicit approval",
        rationale,
        evidence_ids,
        "The consolidation plan never writes records automatically.",
    )
}

fn stages_from_operations(operations: &[ConsolidationOperation]) -> Vec<ConsolidationStage> {
    vec![
        stage(
            "replay",
            "Replay evidence",
            "Review recent evidence before extracting candidates.",
            operations,
            ConsolidationOperationKind::ReplayEvidence,
        ),
        stage(
            "extract",
            "Extract candidates",
            "Surface candidate consolidation work from existing evidence.",
            operations,
            ConsolidationOperationKind::ExtractCandidate,
        ),
        stage(
            "reconcile",
            "Reconcile candidates",
            "Check candidates against existing memory before accepting them.",
            operations,
            ConsolidationOperationKind::ReconcileCandidate,
        ),
        stage(
            "review_gate",
            "Gate through operator review",
            "Require explicit review before any proposed memory change can be trusted.",
            operations,
            ConsolidationOperationKind::ReviewGate,
        ),
        stage(
            "commit",
            "Decide commit eligibility",
            "Keep write-back behind explicit approval and append-only operator commands.",
            operations,
            ConsolidationOperationKind::CommitEligibility,
        ),
    ]
}

fn stage(
    id: impl Into<String>,
    title: impl Into<String>,
    detail: impl Into<String>,
    operations: &[ConsolidationOperation],
    kind: ConsolidationOperationKind,
) -> ConsolidationStage {
    let matching = operations
        .iter()
        .filter(|operation| operation.kind == kind)
        .collect::<Vec<_>>();
    let operation_ids = matching
        .iter()
        .map(|operation| operation.id.clone())
        .collect::<Vec<_>>();
    let status = if matching.is_empty() {
        ConsolidationStageStatus::Clear
    } else if matching
        .iter()
        .any(|operation| operation.status == ConsolidationOperationStatus::Blocked)
    {
        ConsolidationStageStatus::Blocked
    } else if matching
        .iter()
        .any(|operation| operation.status == ConsolidationOperationStatus::NeedsReview)
    {
        ConsolidationStageStatus::NeedsReview
    } else {
        ConsolidationStageStatus::Ready
    };

    ConsolidationStage {
        id: id.into(),
        status,
        title: title.into(),
        detail: detail.into(),
        operation_ids,
    }
}

fn blocked_items(operations: &[ConsolidationOperation]) -> Vec<ConsolidationBlockedItem> {
    operations
        .iter()
        .filter(|operation| {
            operation.status == ConsolidationOperationStatus::Blocked
                || operation.status == ConsolidationOperationStatus::NeedsReview
        })
        .map(|operation| ConsolidationBlockedItem {
            operation_id: operation.id.clone(),
            status: operation.status.clone(),
            reason: operation.gate.reason.clone(),
            evidence_ids: operation.evidence_ids.clone(),
        })
        .collect()
}

fn summarize(
    stages: &[ConsolidationStage],
    operations: &[ConsolidationOperation],
    automatic_write_back: bool,
) -> ConsolidationPlanSummary {
    ConsolidationPlanSummary {
        stage_count: stages.len(),
        operation_count: operations.len(),
        replay_operation_count: count_kind(operations, ConsolidationOperationKind::ReplayEvidence),
        extract_candidate_count: count_kind(
            operations,
            ConsolidationOperationKind::ExtractCandidate,
        ),
        reconcile_candidate_count: count_kind(
            operations,
            ConsolidationOperationKind::ReconcileCandidate,
        ),
        review_gate_count: count_kind(operations, ConsolidationOperationKind::ReviewGate),
        commit_ready_count: operations
            .iter()
            .filter(|operation| {
                operation.kind == ConsolidationOperationKind::CommitEligibility
                    && operation.status == ConsolidationOperationStatus::Ready
            })
            .count(),
        blocked_operation_count: count_status(operations, ConsolidationOperationStatus::Blocked),
        needs_review_operation_count: count_status(
            operations,
            ConsolidationOperationStatus::NeedsReview,
        ),
        automatic_write_back,
    }
}

fn count_kind(operations: &[ConsolidationOperation], kind: ConsolidationOperationKind) -> usize {
    operations
        .iter()
        .filter(|operation| operation.kind == kind)
        .count()
}

fn count_status(
    operations: &[ConsolidationOperation],
    status: ConsolidationOperationStatus,
) -> usize {
    operations
        .iter()
        .filter(|operation| operation.status == status)
        .count()
}

fn unique_limited(values: impl Iterator<Item = String>, limit: usize) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    for value in values {
        if seen.insert(value.clone()) {
            output.push(value);
        }
        if output.len() >= limit {
            break;
        }
    }
    output
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time before Unix epoch")
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use crate::{
        ConsolidationOperationKind, ConsolidationOperationStatus, ConsolidationPlanOptions,
        Episode, MEMORY_CONSOLIDATION_PLAN_VERSION, MemoryData,
        consolidation_plan::consolidation_plan_at,
    };

    #[test]
    fn builds_review_gated_pipeline_from_sleep_candidates() {
        let data = MemoryData {
            event_count: 3,
            last_event_id: Some("event_3".to_string()),
            episodes: vec![
                episode("episode_1", "event_1", 1),
                episode("episode_2", "event_2", 2),
                episode("episode_3", "event_3", 3),
            ],
            ..MemoryData::default()
        };

        let report = consolidation_plan_at(
            &data,
            ConsolidationPlanOptions {
                recent_episode_limit: 2,
                candidate_limit: 8,
                cycle_limit: 4,
                evidence_limit: 4,
                review_limit: 8,
            },
            123,
        );

        assert_eq!(report.version, MEMORY_CONSOLIDATION_PLAN_VERSION);
        assert_eq!(report.generated_at_ms, 123);
        assert_eq!(report.summary.stage_count, 5);
        assert_eq!(report.summary.replay_operation_count, 1);
        assert!(report.summary.extract_candidate_count >= 1);
        assert_eq!(
            report.summary.extract_candidate_count,
            report.summary.reconcile_candidate_count
        );
        assert!(report.summary.review_gate_count >= 1);
        assert!(!report.summary.automatic_write_back);
        assert!(report.operations.iter().any(|operation| {
            operation.kind == ConsolidationOperationKind::CommitEligibility
                && operation.status == ConsolidationOperationStatus::NeedsReview
        }));
    }

    #[test]
    fn keeps_write_back_blocked_when_no_review_work_is_ready() {
        let data = MemoryData {
            event_count: 1,
            last_event_id: Some("event_1".to_string()),
            episodes: vec![episode("episode_1", "event_1", 1)],
            ..MemoryData::default()
        };

        let report = consolidation_plan_at(&data, ConsolidationPlanOptions::default(), 123);

        assert!(!report.write_back_policy.automatic_write_back);
        assert_eq!(report.summary.commit_ready_count, 0);
        assert!(report.operations.iter().any(|operation| {
            operation.kind == ConsolidationOperationKind::CommitEligibility
                && operation.status == ConsolidationOperationStatus::Blocked
        }));
        assert_eq!(report.blocked_items.len(), 1);
    }

    fn episode(id: &str, event_id: &str, created_at_ms: u64) -> Episode {
        Episode {
            id: id.to_string(),
            event_id: event_id.to_string(),
            content: "Lena discussed release notes.".to_string(),
            tags: vec!["product".to_string()],
            mentions: vec!["Lena".to_string()],
            source_id: Some("source_1".to_string()),
            source_position: None,
            source_role: None,
            scope: None,
            created_at_ms,
        }
    }
}
