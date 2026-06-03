use nahuali_core::{
    AnomalyAcknowledgementReport, AnomalyAlert, AnomalyReport, AnomalySummary, AuthorityDecision,
    AuthorityRecall, BriefingEpisode, BriefingGraphSeed, BriefingIntention, BriefingSummary,
    CaptureOpportunity, Claim, ConsolidationBlockedItem, ConsolidationGate, ConsolidationOperation,
    ConsolidationPlanSummary, ConsolidationStage, DeadlineReport, DeadlineSignal, DeadlineSummary,
    Entity, Episode, Fact, GoalProgress, GoalProgressReport, HealthSignal, Intention,
    IntentionReconciliationIssue, IntentionReconciliationReport, KnowledgeHealth, Link,
    MemoryBriefingReport, MemoryConsolidationPlanReport, MemoryGraphEdge, MemoryGraphNode,
    MemoryGraphReport, MemoryGraphSummary, MemoryHookDirective, MemoryHookReport,
    MemoryHookSummary, MemoryProactiveReport, MemoryReflectionReport, MemoryScope,
    MemorySleepReport, OperatorReviewItem, OperatorReviewReport, OperatorReviewSummary,
    ProactiveSummary, Procedure, RecallResult, RecordLedgerIssue, ReflectionCycle,
    ReflectionFinding, ReflectionSourceCoverage, ReflectionSummary, Relation,
    ReviewResolutionReport, SelfInspectionFinding, SelfInspectionReport, SelfInspectionReviewItem,
    SelfInspectionSummary, SelfInspectionWriteBackPolicy, SleepConsolidationCandidate,
    SleepEpisodeReplay, SleepModeSummary, SleepStage,
};
use rmcp::schemars;
use serde::Serialize;

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct RecordLedgerIssueView {
    kind: String,
    severity: String,
    line: Option<usize>,
    message: String,
}

impl From<RecordLedgerIssue> for RecordLedgerIssueView {
    fn from(issue: RecordLedgerIssue) -> Self {
        Self {
            kind: json_string(&issue.kind),
            severity: json_string(&issue.severity),
            line: issue.line,
            message: issue.message,
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct ScopeView {
    kind: String,
    name: String,
    key: String,
}

impl From<MemoryScope> for ScopeView {
    fn from(scope: MemoryScope) -> Self {
        Self {
            kind: json_string(&scope.kind),
            name: scope.name,
            key: scope.key,
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct EpisodeView {
    id: String,
    event_id: String,
    content: String,
    tags: Vec<String>,
    mentions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<ScopeView>,
    created_at_ms: u64,
}

impl From<Episode> for EpisodeView {
    fn from(episode: Episode) -> Self {
        Self {
            id: episode.id,
            event_id: episode.event_id,
            content: episode.content,
            tags: episode.tags,
            mentions: episode.mentions,
            scope: episode.scope.map(ScopeView::from),
            created_at_ms: episode.created_at_ms,
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct EntityView {
    id: String,
    name: String,
    mention_count: usize,
    first_seen_at_ms: u64,
    last_seen_at_ms: u64,
    source_event_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<ScopeView>,
}

impl From<Entity> for EntityView {
    fn from(entity: Entity) -> Self {
        Self {
            id: entity.id,
            name: entity.name,
            mention_count: entity.mention_count,
            first_seen_at_ms: entity.first_seen_at_ms,
            last_seen_at_ms: entity.last_seen_at_ms,
            source_event_ids: entity.source_event_ids,
            scope: entity.scope.map(ScopeView::from),
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct ClaimView {
    id: String,
    event_id: String,
    subject: String,
    predicate: String,
    object: String,
    source_episode_id: Option<String>,
    confidence: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<ScopeView>,
    created_at_ms: u64,
}

impl From<Claim> for ClaimView {
    fn from(claim: Claim) -> Self {
        Self {
            id: claim.id,
            event_id: claim.event_id,
            subject: claim.subject,
            predicate: claim.predicate,
            object: claim.object,
            source_episode_id: claim.source_episode_id,
            confidence: claim.confidence,
            scope: claim.scope.map(ScopeView::from),
            created_at_ms: claim.created_at_ms,
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct FactView {
    id: String,
    event_id: String,
    subject: String,
    predicate: String,
    object: String,
    source_episode_id: Option<String>,
    confidence: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<ScopeView>,
    created_at_ms: u64,
}

impl From<Fact> for FactView {
    fn from(fact: Fact) -> Self {
        Self {
            id: fact.id,
            event_id: fact.event_id,
            subject: fact.subject,
            predicate: fact.predicate,
            object: fact.object,
            source_episode_id: fact.source_episode_id,
            confidence: fact.confidence,
            scope: fact.scope.map(ScopeView::from),
            created_at_ms: fact.created_at_ms,
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct LinkView {
    id: String,
    event_id: String,
    from: String,
    relation: String,
    to: String,
    source_episode_id: Option<String>,
    confidence: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<ScopeView>,
    created_at_ms: u64,
}

impl From<Link> for LinkView {
    fn from(link: Link) -> Self {
        Self {
            id: link.id,
            event_id: link.event_id,
            from: link.from,
            relation: link.relation,
            to: link.to,
            source_episode_id: link.source_episode_id,
            confidence: link.confidence,
            scope: link.scope.map(ScopeView::from),
            created_at_ms: link.created_at_ms,
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct RelationView {
    id: String,
    event_id: String,
    from: String,
    relation: String,
    to: String,
    source_episode_id: Option<String>,
    confidence: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<ScopeView>,
    created_at_ms: u64,
}

impl From<Relation> for RelationView {
    fn from(relation: Relation) -> Self {
        Self {
            id: relation.id,
            event_id: relation.event_id,
            from: relation.from,
            relation: relation.relation,
            to: relation.to,
            source_episode_id: relation.source_episode_id,
            confidence: relation.confidence,
            scope: relation.scope.map(ScopeView::from),
            created_at_ms: relation.created_at_ms,
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct ProcedureView {
    id: String,
    event_id: String,
    kind: String,
    name: String,
    body: String,
    source_episode_id: Option<String>,
    confidence: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<ScopeView>,
    created_at_ms: u64,
}

impl From<Procedure> for ProcedureView {
    fn from(procedure: Procedure) -> Self {
        Self {
            kind: json_string(&procedure.kind),
            id: procedure.id,
            event_id: procedure.event_id,
            name: procedure.name,
            body: procedure.body,
            source_episode_id: procedure.source_episode_id,
            confidence: procedure.confidence,
            scope: procedure.scope.map(ScopeView::from),
            created_at_ms: procedure.created_at_ms,
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct IntentionView {
    id: String,
    event_id: String,
    updated_event_id: String,
    kind: String,
    status: String,
    priority: String,
    description: String,
    source_episode_id: Option<String>,
    status_reason: Option<String>,
    deadline_at_ms: Option<u64>,
    depends_on: Vec<String>,
    goal_id: Option<String>,
    progress_percent: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<ScopeView>,
    created_at_ms: u64,
    updated_at_ms: u64,
}

impl From<Intention> for IntentionView {
    fn from(intention: Intention) -> Self {
        Self {
            kind: json_string(&intention.kind),
            status: json_string(&intention.status),
            priority: json_string(&intention.priority),
            id: intention.id,
            event_id: intention.event_id,
            updated_event_id: intention.updated_event_id,
            description: intention.description,
            source_episode_id: intention.source_episode_id,
            status_reason: intention.status_reason,
            deadline_at_ms: intention.deadline_at_ms,
            depends_on: intention.depends_on,
            goal_id: intention.goal_id,
            progress_percent: intention.progress_percent,
            scope: intention.scope.map(ScopeView::from),
            created_at_ms: intention.created_at_ms,
            updated_at_ms: intention.updated_at_ms,
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct RecallResultView {
    kind: String,
    id: String,
    score: f32,
    excerpt: String,
    evidence_id: Option<String>,
    matched_terms: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<ScopeView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    trust: Option<RecallResultTrustView>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct RecallResultTrustView {
    mode: String,
    score: f32,
    can_trust: bool,
    reasons: Vec<String>,
    signal_kinds: Vec<String>,
}

impl From<RecallResult> for RecallResultView {
    fn from(result: RecallResult) -> Self {
        Self {
            kind: json_string(&result.kind),
            id: result.id,
            score: result.score,
            excerpt: result.excerpt,
            evidence_id: result.evidence_id,
            matched_terms: result.matched_terms,
            scope: result.scope.map(ScopeView::from),
            trust: result.trust.map(|trust| RecallResultTrustView {
                mode: json_string(&trust.mode),
                score: trust.score,
                can_trust: trust.can_trust,
                reasons: trust.reasons,
                signal_kinds: trust.signal_kinds,
            }),
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct AuthorityDecisionView {
    mode: String,
    score: f32,
    can_trust: bool,
    reasons: Vec<String>,
    signal_kinds: Vec<String>,
}

impl From<AuthorityDecision> for AuthorityDecisionView {
    fn from(authority: AuthorityDecision) -> Self {
        let mode = json_string(&authority.mode);
        let signal_kinds = authority.signal_kinds.iter().map(json_string).collect();
        Self {
            mode,
            score: authority.score,
            can_trust: authority.can_trust,
            reasons: authority.reasons,
            signal_kinds,
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct HealthView {
    event_count: usize,
    episode_count: usize,
    fact_count: usize,
    relation_count: usize,
    entity_count: usize,
    supported_fact_count: usize,
    unsupported_fact_count: usize,
    low_confidence_fact_count: usize,
    conflicting_fact_count: usize,
    stale_fact_count: usize,
    isolated_entity_count: usize,
    blind_spot_count: usize,
    average_fact_confidence: f32,
    signals: Vec<HealthSignalView>,
    warnings: Vec<String>,
}

impl From<KnowledgeHealth> for HealthView {
    fn from(health: KnowledgeHealth) -> Self {
        Self {
            event_count: health.event_count,
            episode_count: health.episode_count,
            fact_count: health.fact_count,
            relation_count: health.relation_count,
            entity_count: health.entity_count,
            supported_fact_count: health.supported_fact_count,
            unsupported_fact_count: health.unsupported_fact_count,
            low_confidence_fact_count: health.low_confidence_fact_count,
            conflicting_fact_count: health.conflicting_fact_count,
            stale_fact_count: health.stale_fact_count,
            isolated_entity_count: health.isolated_entity_count,
            blind_spot_count: health.blind_spot_count,
            average_fact_confidence: health.average_fact_confidence,
            signals: health
                .signals
                .into_iter()
                .map(HealthSignalView::from)
                .collect(),
            warnings: health.warnings,
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct HealthSignalView {
    kind: String,
    dimensions: Vec<String>,
    severity: String,
    message: String,
    evidence_ids: Vec<String>,
}

impl From<HealthSignal> for HealthSignalView {
    fn from(signal: HealthSignal) -> Self {
        let kind = json_string(&signal.kind);
        let dimensions = signal.dimensions.iter().map(json_string).collect();
        let severity = json_string(&signal.severity);
        Self {
            kind,
            dimensions,
            severity,
            message: signal.message,
            evidence_ids: signal.evidence_ids,
        }
    }
}

/// Explicit write-back policy mirrored from the core self-inspection report.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct WriteBackPolicyView {
    automatic_write_back: bool,
    requires_operator_review: bool,
    message: String,
}

impl From<SelfInspectionWriteBackPolicy> for WriteBackPolicyView {
    fn from(policy: SelfInspectionWriteBackPolicy) -> Self {
        Self {
            automatic_write_back: policy.automatic_write_back,
            requires_operator_review: policy.requires_operator_review,
            message: policy.message,
        }
    }
}

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

/// Aggregate counts for a self-inspection report.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct SelfInspectionSummaryView {
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

/// Aggregate counts for a graph neighborhood.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct GraphSummaryView {
    node_count: usize,
    edge_count: usize,
    entity_count: usize,
    memory_count: usize,
    support_edge_count: usize,
    relation_edge_count: usize,
    health_signal_count: usize,
    review_decision_count: usize,
}

impl From<MemoryGraphSummary> for GraphSummaryView {
    fn from(summary: MemoryGraphSummary) -> Self {
        Self {
            node_count: summary.node_count,
            edge_count: summary.edge_count,
            entity_count: summary.entity_count,
            memory_count: summary.memory_count,
            support_edge_count: summary.support_edge_count,
            relation_edge_count: summary.relation_edge_count,
            health_signal_count: summary.health_signal_count,
            review_decision_count: summary.review_decision_count,
        }
    }
}

/// A node in a graph neighborhood, with evidence and overlay counts.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct GraphNodeView {
    id: String,
    kind: String,
    label: String,
    depth: usize,
    evidence_ids: Vec<String>,
    source_event_ids: Vec<String>,
    health_signal_count: usize,
    review_decision_count: usize,
}

impl From<MemoryGraphNode> for GraphNodeView {
    fn from(node: MemoryGraphNode) -> Self {
        Self {
            id: node.id,
            kind: json_string(&node.kind),
            label: node.label,
            depth: node.depth,
            evidence_ids: node.evidence_ids,
            source_event_ids: node.source_event_ids,
            health_signal_count: node.health_signal_count,
            review_decision_count: node.review_decision_count,
        }
    }
}

/// An edge in a graph neighborhood.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct GraphEdgeView {
    id: String,
    from: String,
    to: String,
    kind: String,
    label: String,
    confidence: Option<f32>,
    evidence_id: Option<String>,
}

impl From<MemoryGraphEdge> for GraphEdgeView {
    fn from(edge: MemoryGraphEdge) -> Self {
        Self {
            id: edge.id,
            from: edge.from,
            to: edge.to,
            kind: json_string(&edge.kind),
            label: edge.label,
            confidence: edge.confidence,
            evidence_id: edge.evidence_id,
        }
    }
}

/// Structured graph-neighborhood report surfacing authority and evidence IDs.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct GraphReportView {
    version: u32,
    seed: String,
    max_depth: usize,
    limit: usize,
    event_count: usize,
    authority: AuthorityDecisionView,
    summary: GraphSummaryView,
    nodes: Vec<GraphNodeView>,
    edges: Vec<GraphEdgeView>,
}

impl From<MemoryGraphReport> for GraphReportView {
    fn from(report: MemoryGraphReport) -> Self {
        Self {
            version: report.version,
            seed: report.seed,
            max_depth: report.max_depth,
            limit: report.limit,
            event_count: report.event_count,
            authority: AuthorityDecisionView::from(report.authority),
            summary: GraphSummaryView::from(report.summary),
            nodes: report.nodes.into_iter().map(GraphNodeView::from).collect(),
            edges: report.edges.into_iter().map(GraphEdgeView::from).collect(),
        }
    }
}

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

/// Aggregate counts for a proactive operator report.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct ProactiveSummaryView {
    deadline_count: usize,
    overdue_deadline_count: usize,
    due_soon_deadline_count: usize,
    anomaly_count: usize,
    critical_anomaly_count: usize,
    high_anomaly_count: usize,
    capture_opportunity_count: usize,
    high_risk_review_count: usize,
    should_pause: bool,
}

impl From<ProactiveSummary> for ProactiveSummaryView {
    fn from(summary: ProactiveSummary) -> Self {
        Self {
            deadline_count: summary.deadline_count,
            overdue_deadline_count: summary.overdue_deadline_count,
            due_soon_deadline_count: summary.due_soon_deadline_count,
            anomaly_count: summary.anomaly_count,
            critical_anomaly_count: summary.critical_anomaly_count,
            high_anomaly_count: summary.high_anomaly_count,
            capture_opportunity_count: summary.capture_opportunity_count,
            high_risk_review_count: summary.high_risk_review_count,
            should_pause: summary.should_pause,
        }
    }
}

/// A proactive evidence-capture opportunity.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct CaptureOpportunityView {
    id: String,
    review_id: String,
    priority: String,
    title: String,
    detail: String,
    evidence_ids: Vec<String>,
    suggested_action: String,
}

impl From<CaptureOpportunity> for CaptureOpportunityView {
    fn from(opportunity: CaptureOpportunity) -> Self {
        Self {
            priority: json_string(&opportunity.priority),
            id: opportunity.id,
            review_id: opportunity.review_id,
            title: opportunity.title,
            detail: opportunity.detail,
            evidence_ids: opportunity.evidence_ids,
            suggested_action: opportunity.suggested_action,
        }
    }
}

/// Aggregate deadline counts.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct DeadlineSummaryView {
    deadline_count: usize,
    overdue_count: usize,
    due_soon_count: usize,
    scheduled_count: usize,
}

impl From<DeadlineSummary> for DeadlineSummaryView {
    fn from(summary: DeadlineSummary) -> Self {
        Self {
            deadline_count: summary.deadline_count,
            overdue_count: summary.overdue_count,
            due_soon_count: summary.due_soon_count,
            scheduled_count: summary.scheduled_count,
        }
    }
}

/// A single deadline signal derived from intention metadata.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct DeadlineSignalView {
    id: String,
    intention_id: String,
    description: String,
    intention_priority: String,
    status: String,
    deadline_at_ms: u64,
    state: String,
    priority: String,
    detail: String,
    evidence_ids: Vec<String>,
}

impl From<DeadlineSignal> for DeadlineSignalView {
    fn from(signal: DeadlineSignal) -> Self {
        Self {
            intention_priority: json_string(&signal.intention_priority),
            status: json_string(&signal.status),
            state: json_string(&signal.state),
            priority: json_string(&signal.priority),
            id: signal.id,
            intention_id: signal.intention_id,
            description: signal.description,
            deadline_at_ms: signal.deadline_at_ms,
            detail: signal.detail,
            evidence_ids: signal.evidence_ids,
        }
    }
}

/// Deadline-only report derived from intention metadata.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct DeadlineReportView {
    version: u32,
    generated_at_ms: u64,
    now_ms: u64,
    horizon_ms: u64,
    summary: DeadlineSummaryView,
    deadlines: Vec<DeadlineSignalView>,
}

impl From<DeadlineReport> for DeadlineReportView {
    fn from(report: DeadlineReport) -> Self {
        Self {
            version: report.version,
            generated_at_ms: report.generated_at_ms,
            now_ms: report.now_ms,
            horizon_ms: report.horizon_ms,
            summary: DeadlineSummaryView::from(report.summary),
            deadlines: report
                .deadlines
                .into_iter()
                .map(DeadlineSignalView::from)
                .collect(),
        }
    }
}

/// Aggregate anomaly counts.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct AnomalySummaryView {
    critical_count: usize,
    high_count: usize,
    medium_count: usize,
    low_count: usize,
}

impl From<AnomalySummary> for AnomalySummaryView {
    fn from(summary: AnomalySummary) -> Self {
        Self {
            critical_count: summary.critical_count,
            high_count: summary.high_count,
            medium_count: summary.medium_count,
            low_count: summary.low_count,
        }
    }
}

/// A single proactive anomaly alert.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct AnomalyAlertView {
    id: String,
    kind: String,
    priority: String,
    title: String,
    detail: String,
    evidence_ids: Vec<String>,
    source_id: String,
    review_id: Option<String>,
    suggested_action: String,
}

impl From<AnomalyAlert> for AnomalyAlertView {
    fn from(alert: AnomalyAlert) -> Self {
        Self {
            kind: json_string(&alert.kind),
            priority: json_string(&alert.priority),
            id: alert.id,
            title: alert.title,
            detail: alert.detail,
            evidence_ids: alert.evidence_ids,
            source_id: alert.source_id,
            review_id: alert.review_id,
            suggested_action: alert.suggested_action,
        }
    }
}

/// Non-mutating anomaly report surfacing actionable alerts.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct AnomalyReportView {
    version: u32,
    generated_at_ms: u64,
    summary: AnomalySummaryView,
    alert_count: usize,
    alerts: Vec<AnomalyAlertView>,
}

impl From<AnomalyReport> for AnomalyReportView {
    fn from(report: AnomalyReport) -> Self {
        Self {
            version: report.version,
            generated_at_ms: report.generated_at_ms,
            summary: AnomalySummaryView::from(report.summary),
            alert_count: report.alert_count,
            alerts: report
                .alerts
                .into_iter()
                .map(AnomalyAlertView::from)
                .collect(),
        }
    }
}

/// Structured proactive operator report bundling deadlines and anomalies.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct MemoryProactiveReportView {
    version: u32,
    generated_at_ms: u64,
    event_count: usize,
    authority: AuthorityDecisionView,
    summary: ProactiveSummaryView,
    deadlines: DeadlineReportView,
    anomalies: AnomalyReportView,
    capture_opportunities: Vec<CaptureOpportunityView>,
    high_risk_review_items: Vec<OperatorReviewItemView>,
    write_back_policy: WriteBackPolicyView,
}

impl From<MemoryProactiveReport> for MemoryProactiveReportView {
    fn from(report: MemoryProactiveReport) -> Self {
        Self {
            version: report.version,
            generated_at_ms: report.generated_at_ms,
            event_count: report.event_count,
            authority: AuthorityDecisionView::from(report.authority),
            summary: ProactiveSummaryView::from(report.summary),
            deadlines: DeadlineReportView::from(report.deadlines),
            anomalies: AnomalyReportView::from(report.anomalies),
            capture_opportunities: report
                .capture_opportunities
                .into_iter()
                .map(CaptureOpportunityView::from)
                .collect(),
            high_risk_review_items: report
                .high_risk_review_items
                .into_iter()
                .map(OperatorReviewItemView::from)
                .collect(),
            write_back_policy: WriteBackPolicyView::from(report.write_back_policy),
        }
    }
}

/// Result of planning or applying an anomaly acknowledgement.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct AnomalyAcknowledgementReportView {
    version: u32,
    dry_run: bool,
    applied: bool,
    anomaly_id: String,
    note: String,
    alert: AnomalyAlertView,
    evidence_ids: Vec<String>,
    decision_id: Option<String>,
    event_id: Option<String>,
    policy: String,
}

impl From<AnomalyAcknowledgementReport> for AnomalyAcknowledgementReportView {
    fn from(report: AnomalyAcknowledgementReport) -> Self {
        Self {
            version: report.version,
            dry_run: report.dry_run,
            applied: report.applied,
            anomaly_id: report.anomaly_id,
            note: report.note,
            alert: AnomalyAlertView::from(report.alert),
            evidence_ids: report.evidence_ids,
            decision_id: report.decision_id,
            event_id: report.event_id,
            policy: report.policy,
        }
    }
}

/// Reconciliation issue for a single intention.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct IntentionReconciliationIssueView {
    id: String,
    intention_id: String,
    kind: String,
    priority: String,
    title: String,
    detail: String,
    evidence_ids: Vec<String>,
}

impl From<IntentionReconciliationIssue> for IntentionReconciliationIssueView {
    fn from(issue: IntentionReconciliationIssue) -> Self {
        Self {
            kind: json_string(&issue.kind),
            priority: json_string(&issue.priority),
            id: issue.id,
            intention_id: issue.intention_id,
            title: issue.title,
            detail: issue.detail,
            evidence_ids: issue.evidence_ids,
        }
    }
}

/// Non-mutating reconciliation report for active work.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct IntentionReconciliationReportView {
    version: u32,
    generated_at_ms: u64,
    intention_count: usize,
    issue_count: usize,
    issues: Vec<IntentionReconciliationIssueView>,
}

impl From<IntentionReconciliationReport> for IntentionReconciliationReportView {
    fn from(report: IntentionReconciliationReport) -> Self {
        Self {
            version: report.version,
            generated_at_ms: report.generated_at_ms,
            intention_count: report.intention_count,
            issue_count: report.issue_count,
            issues: report
                .issues
                .into_iter()
                .map(IntentionReconciliationIssueView::from)
                .collect(),
        }
    }
}

/// Progress aggregate for one goal intention.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct GoalProgressView {
    goal_id: String,
    description: String,
    status: String,
    explicit_progress_percent: Option<u8>,
    derived_progress_percent: u8,
    child_count: usize,
    completed_count: usize,
    active_count: usize,
    blocked_count: usize,
    deferred_count: usize,
    abandoned_count: usize,
    child_ids: Vec<String>,
}

impl From<GoalProgress> for GoalProgressView {
    fn from(goal: GoalProgress) -> Self {
        Self {
            status: json_string(&goal.status),
            goal_id: goal.goal_id,
            description: goal.description,
            explicit_progress_percent: goal.explicit_progress_percent,
            derived_progress_percent: goal.derived_progress_percent,
            child_count: goal.child_count,
            completed_count: goal.completed_count,
            active_count: goal.active_count,
            blocked_count: goal.blocked_count,
            deferred_count: goal.deferred_count,
            abandoned_count: goal.abandoned_count,
            child_ids: goal.child_ids,
        }
    }
}

/// Non-mutating goal progress report.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct GoalProgressReportView {
    version: u32,
    generated_at_ms: u64,
    goal_count: usize,
    goals: Vec<GoalProgressView>,
}

impl From<GoalProgressReport> for GoalProgressReportView {
    fn from(report: GoalProgressReport) -> Self {
        Self {
            version: report.version,
            generated_at_ms: report.generated_at_ms,
            goal_count: report.goal_count,
            goals: report
                .goals
                .into_iter()
                .map(GoalProgressView::from)
                .collect(),
        }
    }
}

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

pub(crate) fn json_string(value: &impl Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| "unknown".to_string())
}
