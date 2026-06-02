use nahuali_core::{
    AuthorityDecision, BriefingEpisode, BriefingGraphSeed, BriefingIntention, BriefingSummary,
    Claim, Entity, Episode, Fact, HealthSignal, Intention, KnowledgeHealth, Link,
    MemoryBriefingReport, MemoryGraphEdge, MemoryGraphNode, MemoryGraphReport, MemoryGraphSummary,
    MemoryScope, OperatorReviewItem, OperatorReviewReport, OperatorReviewSummary, Procedure,
    RecallResult, RecordLedgerIssue, Relation, SelfInspectionFinding, SelfInspectionReport,
    SelfInspectionReviewItem, SelfInspectionSummary, SelfInspectionWriteBackPolicy,
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

pub(crate) fn json_string(value: &impl Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| "unknown".to_string())
}
