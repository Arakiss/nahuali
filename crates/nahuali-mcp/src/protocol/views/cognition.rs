use nahuali_core::{
    AuthorityRecall, ConsolidationBlockedItem, ConsolidationGate, ConsolidationOperation,
    ConsolidationPlanSummary, ConsolidationStage, MemoryConsolidationPlanReport,
    MemoryHookDirective, MemoryHookReport, MemoryHookSummary, MemoryReflectionReport,
    MemorySleepReport, ReflectionCycle, ReflectionFinding, ReflectionSourceCoverage,
    ReflectionSummary, SleepConsolidationCandidate, SleepEpisodeReplay, SleepModeSummary,
    SleepStage,
};
use rmcp::schemars;
use serde::Serialize;

use super::{
    AuthorityDecisionView, BriefingReportView, HealthView, OperatorReviewReportView,
    RecallResultView, SelfInspectionReportView, SelfInspectionReviewItemView, WriteBackPolicyView,
    json_string,
};

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct HookSummaryView {
    recall_count: usize,
    briefing_episode_count: usize,
    briefing_intention_count: usize,
    review_item_count: usize,
    reflection_cycle_count: usize,
    self_inspection_finding_count: usize,
    sleep_stage_count: usize,
    sleep_candidate_count: usize,
    automatic_write_back: bool,
    should_pause_for_review: bool,
}

impl From<MemoryHookSummary> for HookSummaryView {
    fn from(summary: MemoryHookSummary) -> Self {
        Self {
            recall_count: summary.recall_count,
            briefing_episode_count: summary.briefing_episode_count,
            briefing_intention_count: summary.briefing_intention_count,
            review_item_count: summary.review_item_count,
            reflection_cycle_count: summary.reflection_cycle_count,
            self_inspection_finding_count: summary.self_inspection_finding_count,
            sleep_stage_count: summary.sleep_stage_count,
            sleep_candidate_count: summary.sleep_candidate_count,
            automatic_write_back: summary.automatic_write_back,
            should_pause_for_review: summary.should_pause_for_review,
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct HookDirectiveView {
    id: String,
    priority: String,
    title: String,
    detail: String,
    evidence_ids: Vec<String>,
}

impl From<MemoryHookDirective> for HookDirectiveView {
    fn from(directive: MemoryHookDirective) -> Self {
        Self {
            id: directive.id,
            priority: json_string(&directive.priority),
            title: directive.title,
            detail: directive.detail,
            evidence_ids: directive.evidence_ids,
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct HookRecallView {
    results: Vec<RecallResultView>,
    authority: AuthorityDecisionView,
    health: HealthView,
}

impl From<AuthorityRecall> for HookRecallView {
    fn from(recall: AuthorityRecall) -> Self {
        Self {
            results: recall
                .results
                .into_iter()
                .map(RecallResultView::from)
                .collect(),
            authority: AuthorityDecisionView::from(recall.authority),
            health: HealthView::from(recall.health),
        }
    }
}

/// Aggregate counts for a reflection report.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct ReflectionSummaryView {
    finding_count: usize,
    total_cycle_count: usize,
    displayed_cycle_count: usize,
    critical_cycle_count: usize,
    high_cycle_count: usize,
    medium_cycle_count: usize,
    low_cycle_count: usize,
    evidence_id_count: usize,
}

impl From<ReflectionSummary> for ReflectionSummaryView {
    fn from(summary: ReflectionSummary) -> Self {
        Self {
            finding_count: summary.finding_count,
            total_cycle_count: summary.total_cycle_count,
            displayed_cycle_count: summary.displayed_cycle_count,
            critical_cycle_count: summary.critical_cycle_count,
            high_cycle_count: summary.high_cycle_count,
            medium_cycle_count: summary.medium_cycle_count,
            low_cycle_count: summary.low_cycle_count,
            evidence_id_count: summary.evidence_id_count,
        }
    }
}

/// Source and evidence coverage across projected memory.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct ReflectionSourceCoverageView {
    source_count: usize,
    episode_count: usize,
    sourced_episode_count: usize,
    unsourced_episode_count: usize,
    derived_memory_count: usize,
    evidence_backed_memory_count: usize,
    unsupported_memory_count: usize,
    source_coverage_ratio: f32,
    evidence_coverage_ratio: f32,
}

impl From<ReflectionSourceCoverage> for ReflectionSourceCoverageView {
    fn from(coverage: ReflectionSourceCoverage) -> Self {
        Self {
            source_count: coverage.source_count,
            episode_count: coverage.episode_count,
            sourced_episode_count: coverage.sourced_episode_count,
            unsourced_episode_count: coverage.unsourced_episode_count,
            derived_memory_count: coverage.derived_memory_count,
            evidence_backed_memory_count: coverage.evidence_backed_memory_count,
            unsupported_memory_count: coverage.unsupported_memory_count,
            source_coverage_ratio: coverage.source_coverage_ratio,
            evidence_coverage_ratio: coverage.evidence_coverage_ratio,
        }
    }
}

/// A finding included in a grouped reflection cycle.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct ReflectionFindingView {
    id: String,
    kind: String,
    severity: String,
    title: String,
    detail: String,
    dimensions: Vec<String>,
    evidence_ids: Vec<String>,
    suggested_action: String,
}

impl From<ReflectionFinding> for ReflectionFindingView {
    fn from(finding: ReflectionFinding) -> Self {
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

/// Grouped reflection work for one operator-approved cycle.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct ReflectionCycleView {
    id: String,
    priority: String,
    action: String,
    title: String,
    rationale: String,
    finding_count: usize,
    evidence_ids: Vec<String>,
    findings: Vec<ReflectionFindingView>,
}

impl From<ReflectionCycle> for ReflectionCycleView {
    fn from(cycle: ReflectionCycle) -> Self {
        Self {
            id: cycle.id,
            priority: json_string(&cycle.priority),
            action: json_string(&cycle.action),
            title: cycle.title,
            rationale: cycle.rationale,
            finding_count: cycle.finding_count,
            evidence_ids: cycle.evidence_ids,
            findings: cycle
                .findings
                .into_iter()
                .map(ReflectionFindingView::from)
                .collect(),
        }
    }
}

/// Structured reflection report grouping self-inspection findings into cycles.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct MemoryReflectionReportView {
    version: u32,
    generated_at_ms: u64,
    event_count: usize,
    authority: AuthorityDecisionView,
    health: HealthView,
    summary: ReflectionSummaryView,
    source_coverage: ReflectionSourceCoverageView,
    cycles: Vec<ReflectionCycleView>,
    write_back_policy: WriteBackPolicyView,
}

impl From<MemoryReflectionReport> for MemoryReflectionReportView {
    fn from(report: MemoryReflectionReport) -> Self {
        Self {
            version: report.version,
            generated_at_ms: report.generated_at_ms,
            event_count: report.event_count,
            authority: AuthorityDecisionView::from(report.authority),
            health: HealthView::from(report.health),
            summary: ReflectionSummaryView::from(report.summary),
            source_coverage: ReflectionSourceCoverageView::from(report.source_coverage),
            cycles: report
                .cycles
                .into_iter()
                .map(ReflectionCycleView::from)
                .collect(),
            write_back_policy: WriteBackPolicyView::from(report.write_back_policy),
        }
    }
}

/// Aggregate counts for a Sleep Mode report.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct SleepModeSummaryView {
    replayed_episode_count: usize,
    finding_count: usize,
    reflection_cycle_count: usize,
    consolidation_candidate_count: usize,
    review_item_count: usize,
    pending_stage_count: usize,
    automatic_write_back: bool,
}

impl From<SleepModeSummary> for SleepModeSummaryView {
    fn from(summary: SleepModeSummary) -> Self {
        Self {
            replayed_episode_count: summary.replayed_episode_count,
            finding_count: summary.finding_count,
            reflection_cycle_count: summary.reflection_cycle_count,
            consolidation_candidate_count: summary.consolidation_candidate_count,
            review_item_count: summary.review_item_count,
            pending_stage_count: summary.pending_stage_count,
            automatic_write_back: summary.automatic_write_back,
        }
    }
}

/// A deterministic stage in a Sleep Mode pass.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct SleepStageView {
    id: String,
    status: String,
    title: String,
    detail: String,
    evidence_ids: Vec<String>,
}

impl From<SleepStage> for SleepStageView {
    fn from(stage: SleepStage) -> Self {
        Self {
            id: stage.id,
            status: json_string(&stage.status),
            title: stage.title,
            detail: stage.detail,
            evidence_ids: stage.evidence_ids,
        }
    }
}

/// An episode replayed by Sleep Mode.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct SleepEpisodeReplayView {
    id: String,
    event_id: String,
    content: String,
    tags: Vec<String>,
    mentions: Vec<String>,
    source_id: Option<String>,
    created_at_ms: u64,
}

impl From<SleepEpisodeReplay> for SleepEpisodeReplayView {
    fn from(episode: SleepEpisodeReplay) -> Self {
        Self {
            id: episode.id,
            event_id: episode.event_id,
            content: episode.content,
            tags: episode.tags,
            mentions: episode.mentions,
            source_id: episode.source_id,
            created_at_ms: episode.created_at_ms,
        }
    }
}

/// A candidate consolidation work item produced by Sleep Mode.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct SleepConsolidationCandidateView {
    id: String,
    kind: String,
    priority: String,
    action: String,
    title: String,
    rationale: String,
    evidence_ids: Vec<String>,
}

impl From<SleepConsolidationCandidate> for SleepConsolidationCandidateView {
    fn from(candidate: SleepConsolidationCandidate) -> Self {
        Self {
            id: candidate.id,
            kind: json_string(&candidate.kind),
            priority: json_string(&candidate.priority),
            action: json_string(&candidate.action),
            title: candidate.title,
            rationale: candidate.rationale,
            evidence_ids: candidate.evidence_ids,
        }
    }
}

/// Structured Sleep Mode report surfacing replay, candidates, and review items.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct MemorySleepReportView {
    version: u32,
    generated_at_ms: u64,
    event_count: usize,
    authority: AuthorityDecisionView,
    health: HealthView,
    summary: SleepModeSummaryView,
    stages: Vec<SleepStageView>,
    recent_episodes: Vec<SleepEpisodeReplayView>,
    consolidation_candidates: Vec<SleepConsolidationCandidateView>,
    review_items: Vec<SelfInspectionReviewItemView>,
    reflection: MemoryReflectionReportView,
    self_inspection: SelfInspectionReportView,
    write_back_policy: WriteBackPolicyView,
}

impl From<MemorySleepReport> for MemorySleepReportView {
    fn from(report: MemorySleepReport) -> Self {
        Self {
            version: report.version,
            generated_at_ms: report.generated_at_ms,
            event_count: report.event_count,
            authority: AuthorityDecisionView::from(report.authority),
            health: HealthView::from(report.health),
            summary: SleepModeSummaryView::from(report.summary),
            stages: report
                .stages
                .into_iter()
                .map(SleepStageView::from)
                .collect(),
            recent_episodes: report
                .recent_episodes
                .into_iter()
                .map(SleepEpisodeReplayView::from)
                .collect(),
            consolidation_candidates: report
                .consolidation_candidates
                .into_iter()
                .map(SleepConsolidationCandidateView::from)
                .collect(),
            review_items: report
                .review_items
                .into_iter()
                .map(SelfInspectionReviewItemView::from)
                .collect(),
            reflection: MemoryReflectionReportView::from(report.reflection),
            self_inspection: SelfInspectionReportView::from(report.self_inspection),
            write_back_policy: WriteBackPolicyView::from(report.write_back_policy),
        }
    }
}

/// Aggregate counts for a consolidation plan.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct ConsolidationPlanSummaryView {
    stage_count: usize,
    operation_count: usize,
    replay_operation_count: usize,
    extract_candidate_count: usize,
    reconcile_candidate_count: usize,
    review_gate_count: usize,
    commit_ready_count: usize,
    blocked_operation_count: usize,
    needs_review_operation_count: usize,
    automatic_write_back: bool,
}

impl From<ConsolidationPlanSummary> for ConsolidationPlanSummaryView {
    fn from(summary: ConsolidationPlanSummary) -> Self {
        Self {
            stage_count: summary.stage_count,
            operation_count: summary.operation_count,
            replay_operation_count: summary.replay_operation_count,
            extract_candidate_count: summary.extract_candidate_count,
            reconcile_candidate_count: summary.reconcile_candidate_count,
            review_gate_count: summary.review_gate_count,
            commit_ready_count: summary.commit_ready_count,
            blocked_operation_count: summary.blocked_operation_count,
            needs_review_operation_count: summary.needs_review_operation_count,
            automatic_write_back: summary.automatic_write_back,
        }
    }
}

/// A pipeline stage in a consolidation plan.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct ConsolidationStageView {
    id: String,
    status: String,
    title: String,
    detail: String,
    operation_ids: Vec<String>,
}

impl From<ConsolidationStage> for ConsolidationStageView {
    fn from(stage: ConsolidationStage) -> Self {
        Self {
            id: stage.id,
            status: json_string(&stage.status),
            title: stage.title,
            detail: stage.detail,
            operation_ids: stage.operation_ids,
        }
    }
}

/// The write-back gate attached to a consolidation operation.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct ConsolidationGateView {
    requires_operator_review: bool,
    automatic_write_back: bool,
    reason: String,
}

impl From<ConsolidationGate> for ConsolidationGateView {
    fn from(gate: ConsolidationGate) -> Self {
        Self {
            requires_operator_review: gate.requires_operator_review,
            automatic_write_back: gate.automatic_write_back,
            reason: gate.reason,
        }
    }
}

/// A concrete non-mutating operation proposed by a consolidation plan.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct ConsolidationOperationView {
    id: String,
    kind: String,
    status: String,
    priority: Option<String>,
    action: Option<String>,
    title: String,
    rationale: String,
    evidence_ids: Vec<String>,
    gate: ConsolidationGateView,
}

impl From<ConsolidationOperation> for ConsolidationOperationView {
    fn from(operation: ConsolidationOperation) -> Self {
        Self {
            id: operation.id,
            kind: json_string(&operation.kind),
            status: json_string(&operation.status),
            priority: operation.priority.as_ref().map(json_string),
            action: operation.action.as_ref().map(json_string),
            title: operation.title,
            rationale: operation.rationale,
            evidence_ids: operation.evidence_ids,
            gate: ConsolidationGateView::from(operation.gate),
        }
    }
}

/// An operation that cannot become a memory write without more review.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct ConsolidationBlockedItemView {
    operation_id: String,
    status: String,
    reason: String,
    evidence_ids: Vec<String>,
}

impl From<ConsolidationBlockedItem> for ConsolidationBlockedItemView {
    fn from(item: ConsolidationBlockedItem) -> Self {
        Self {
            operation_id: item.operation_id,
            status: json_string(&item.status),
            reason: item.reason,
            evidence_ids: item.evidence_ids,
        }
    }
}

/// Structured consolidation-plan report surfacing stages, operations, and gates.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct MemoryConsolidationPlanReportView {
    version: u32,
    generated_at_ms: u64,
    event_count: usize,
    authority: AuthorityDecisionView,
    health: HealthView,
    summary: ConsolidationPlanSummaryView,
    stages: Vec<ConsolidationStageView>,
    operations: Vec<ConsolidationOperationView>,
    blocked_items: Vec<ConsolidationBlockedItemView>,
    sleep: MemorySleepReportView,
    review: OperatorReviewReportView,
    write_back_policy: WriteBackPolicyView,
}

impl From<MemoryConsolidationPlanReport> for MemoryConsolidationPlanReportView {
    fn from(report: MemoryConsolidationPlanReport) -> Self {
        Self {
            version: report.version,
            generated_at_ms: report.generated_at_ms,
            event_count: report.event_count,
            authority: AuthorityDecisionView::from(report.authority),
            health: HealthView::from(report.health),
            summary: ConsolidationPlanSummaryView::from(report.summary),
            stages: report
                .stages
                .into_iter()
                .map(ConsolidationStageView::from)
                .collect(),
            operations: report
                .operations
                .into_iter()
                .map(ConsolidationOperationView::from)
                .collect(),
            blocked_items: report
                .blocked_items
                .into_iter()
                .map(ConsolidationBlockedItemView::from)
                .collect(),
            sleep: MemorySleepReportView::from(report.sleep),
            review: OperatorReviewReportView::from(report.review),
            write_back_policy: WriteBackPolicyView::from(report.write_back_policy),
        }
    }
}

/// Typed view of a `memory_hook` report. Every surface — the hook's own
/// structure (kind, summary, directives), the dedicated sub-report views
/// (authority, briefing, recall, self-inspection), and the heavier optional
/// `reflection` and `sleep` sub-reports it can embed — is fully typed.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct MemoryHookReportView {
    version: u32,
    generated_at_ms: u64,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    input: Option<String>,
    event_count: usize,
    authority: AuthorityDecisionView,
    summary: HookSummaryView,
    directives: Vec<HookDirectiveView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    briefing: Option<BriefingReportView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    recall: Option<HookRecallView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    self_inspection: Option<SelfInspectionReportView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reflection: Option<MemoryReflectionReportView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sleep: Option<MemorySleepReportView>,
}

impl From<MemoryHookReport> for MemoryHookReportView {
    fn from(report: MemoryHookReport) -> Self {
        Self {
            version: report.version,
            generated_at_ms: report.generated_at_ms,
            kind: json_string(&report.kind),
            input: report.input,
            event_count: report.event_count,
            authority: AuthorityDecisionView::from(report.authority),
            summary: HookSummaryView::from(report.summary),
            directives: report
                .directives
                .into_iter()
                .map(HookDirectiveView::from)
                .collect(),
            briefing: report.briefing.map(BriefingReportView::from),
            recall: report.recall.map(HookRecallView::from),
            self_inspection: report.self_inspection.map(SelfInspectionReportView::from),
            reflection: report.reflection.map(MemoryReflectionReportView::from),
            sleep: report.sleep.map(MemorySleepReportView::from),
        }
    }
}
