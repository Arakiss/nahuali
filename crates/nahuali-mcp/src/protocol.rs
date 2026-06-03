use std::collections::BTreeMap;

use nahuali_core::{
    IntentionKind, IntentionPriority, IntentionStatus, MemoryHookKind, MemoryKind, MemoryScope,
    MemoryScopeKind, SelfInspectionReviewAction, SelfInspectionReviewPriority, SourceKind,
    TextChunking,
};
use rmcp::schemars;
use serde::{Deserialize, Serialize};
use serde_json::Value;

mod views;

pub(crate) use views::*;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RememberArgs {
    pub(crate) content: String,
    pub(crate) tags: Option<Vec<String>>,
    pub(crate) mentions: Option<Vec<String>>,
    pub(crate) scope: Option<ScopeArg>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IngestArgs {
    /// Source-neutral ingestion document (the Nahuali interchange format,
    /// `MemoryIngestDocument`). Expected shape: `{ "version": 1, "source": {
    /// "kind", "title", "uri", "metadata", "scope" }, "episodes": [...],
    /// "claims": [...], "links": [...], "procedures": [...], "intentions": [...]
    /// }`. Derived records reference episodes by their local `ref`.
    pub(crate) document: Value,
    /// When true, validate and preflight the document without appending any
    /// records. Defaults to false.
    pub(crate) dry_run: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IngestTextArgs {
    pub(crate) content: String,
    pub(crate) title: Option<String>,
    pub(crate) uri: Option<String>,
    pub(crate) kind: Option<SourceKindArg>,
    pub(crate) tags: Option<Vec<String>>,
    pub(crate) mentions: Option<Vec<String>>,
    pub(crate) metadata: Option<BTreeMap<String, String>>,
    pub(crate) source_role: Option<String>,
    pub(crate) scope: Option<ScopeArg>,
    pub(crate) chunking: Option<TextChunkingArg>,
    pub(crate) max_chunk_bytes: Option<usize>,
    pub(crate) dry_run: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FactArgs {
    pub(crate) subject: String,
    pub(crate) predicate: String,
    pub(crate) object: String,
    /// Id of the source episode this assertion cites as evidence. Mutually
    /// exclusive with `sourceLast`.
    pub(crate) source_episode_id: Option<String>,
    /// When true, cite the most recently recorded episode as evidence. Use this
    /// right after `remember`. Mutually exclusive with `sourceEpisodeId`.
    pub(crate) source_last: Option<bool>,
    /// Confidence in this assertion, from 0.0 (unsure) to 1.0 (certain).
    /// Defaults to 0.8.
    pub(crate) confidence: Option<f32>,
    pub(crate) scope: Option<ScopeArg>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RelateArgs {
    pub(crate) from: String,
    pub(crate) relation: String,
    pub(crate) to: String,
    /// Id of the source episode this connection cites as evidence. Mutually
    /// exclusive with `sourceLast`.
    pub(crate) source_episode_id: Option<String>,
    /// When true, cite the most recently recorded episode as evidence. Use this
    /// right after `remember`. Mutually exclusive with `sourceEpisodeId`.
    pub(crate) source_last: Option<bool>,
    /// Confidence in this connection, from 0.0 (unsure) to 1.0 (certain).
    /// Defaults to 0.8.
    pub(crate) confidence: Option<f32>,
    pub(crate) scope: Option<ScopeArg>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProcedureArgs {
    pub(crate) name: String,
    pub(crate) body: String,
    /// Id of the source episode this procedure cites as evidence. Mutually
    /// exclusive with `sourceLast`.
    pub(crate) source_episode_id: Option<String>,
    /// When true, cite the most recently recorded episode as evidence. Use this
    /// right after `remember`. Mutually exclusive with `sourceEpisodeId`.
    pub(crate) source_last: Option<bool>,
    /// Confidence in this procedure, from 0.0 (unsure) to 1.0 (certain).
    /// Defaults to 0.8.
    pub(crate) confidence: Option<f32>,
    pub(crate) scope: Option<ScopeArg>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IntentionArgs {
    pub(crate) description: String,
    pub(crate) kind: Option<IntentionKindArg>,
    pub(crate) priority: Option<IntentionPriorityArg>,
    pub(crate) source_episode_id: Option<String>,
    pub(crate) source_last: Option<bool>,
    pub(crate) scope: Option<ScopeArg>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IntentionStatusArgs {
    pub(crate) id: String,
    pub(crate) status: IntentionStatusArg,
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IntentionUpdateArgs {
    pub(crate) id: String,
    pub(crate) description: Option<String>,
    pub(crate) priority: Option<IntentionPriorityArg>,
    /// Deadline as Unix epoch milliseconds. Pass `null` to clear an existing
    /// deadline; omit the field to leave it unchanged.
    pub(crate) deadline_at_ms: Option<Option<u64>>,
    pub(crate) depends_on: Option<Vec<String>>,
    pub(crate) goal_id: Option<Option<String>>,
    /// Progress from 0 to 100 percent. Pass `null` to clear it; omit to leave
    /// it unchanged.
    pub(crate) progress_percent: Option<Option<u8>>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IntentionReconcileArgs {
    /// Reference "now" as Unix epoch milliseconds. Defaults to the current
    /// system time; override only to reconcile against a fixed point.
    pub(crate) now_ms: Option<u64>,
    /// Age threshold in milliseconds after which an intention is treated as
    /// stale.
    pub(crate) stale_after_ms: Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProactiveArgs {
    /// Reference "now" as Unix epoch milliseconds. Defaults to the current
    /// system time; override only to evaluate against a fixed point.
    pub(crate) now_ms: Option<u64>,
    /// How far ahead, in milliseconds from `now`, to treat a deadline as
    /// upcoming.
    pub(crate) deadline_horizon_ms: Option<u64>,
    /// Age threshold in milliseconds after which a fact or intention is treated
    /// as stale.
    pub(crate) stale_after_ms: Option<u64>,
    pub(crate) review_limit: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AnomalyAcknowledgeArgs {
    pub(crate) anomaly_id: String,
    pub(crate) note: String,
    pub(crate) dry_run: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BriefingArgs {
    pub(crate) episode_limit: Option<usize>,
    pub(crate) intention_limit: Option<usize>,
    pub(crate) review_limit: Option<usize>,
    pub(crate) graph_seed_limit: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryHookArgs {
    pub(crate) kind: MemoryHookKindArg,
    pub(crate) input: Option<String>,
    pub(crate) recall_limit: Option<usize>,
    pub(crate) episode_limit: Option<usize>,
    pub(crate) intention_limit: Option<usize>,
    pub(crate) review_limit: Option<usize>,
    pub(crate) graph_seed_limit: Option<usize>,
    pub(crate) cycle_limit: Option<usize>,
    pub(crate) evidence_limit: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MemoryHookKindArg {
    SessionStart,
    PrePrompt,
    PostAction,
    SessionClose,
    SleepCycle,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum IntentionKindArg {
    Task,
    Goal,
    Reminder,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum IntentionPriorityArg {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum IntentionStatusArg {
    Active,
    Completed,
    Abandoned,
    Blocked,
    Deferred,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SourceKindArg {
    Document,
    Conversation,
    Transcript,
    WebPage,
    Note,
    Other,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TextChunkingArg {
    Document,
    Paragraphs,
    Lines,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScopeArg {
    pub(crate) kind: ScopeKindArg,
    pub(crate) name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ScopeKindArg {
    Personal,
    Project,
    Organization,
    Custom,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecallArgs {
    pub(crate) query: String,
    pub(crate) limit: Option<usize>,
    pub(crate) scope: Option<ScopeArg>,
    pub(crate) kinds: Option<Vec<RecallKindArg>>,
    pub(crate) require_evidence: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RecallKindArg {
    Entity,
    Episode,
    Claim,
    Link,
    Procedure,
    Intention,
    Fact,
    Relation,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GraphArgs {
    pub(crate) seed: String,
    pub(crate) depth: Option<usize>,
    pub(crate) limit: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReviewArgs {
    pub(crate) limit: Option<usize>,
    pub(crate) min_priority: Option<ReviewPriorityArg>,
    pub(crate) action: Option<ReviewActionArg>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReflectArgs {
    pub(crate) cycle_limit: Option<usize>,
    pub(crate) evidence_limit: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConsolidationPlanArgs {
    pub(crate) episode_limit: Option<usize>,
    pub(crate) candidate_limit: Option<usize>,
    pub(crate) cycle_limit: Option<usize>,
    pub(crate) evidence_limit: Option<usize>,
    pub(crate) review_limit: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReviewResolveArgs {
    pub(crate) review_id: String,
    pub(crate) note: String,
    pub(crate) dry_run: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReviewPriorityArg {
    Critical,
    High,
    Medium,
    Low,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReviewActionArg {
    CaptureEvidence,
    ResolveContradiction,
    RefreshMemory,
    LinkMemory,
    ConsolidatePattern,
    ReviewIntention,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct RememberResult {
    pub(crate) episode: EpisodeView,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct IngestResult {
    pub(crate) report: Value,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct IngestTextResult {
    pub(crate) adapter_report: Value,
    pub(crate) report: Option<Value>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct ClaimResult {
    pub(crate) claim: ClaimView,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct FactResult {
    pub(crate) fact: FactView,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct LinkResult {
    pub(crate) link: LinkView,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct RelateResult {
    pub(crate) relation: RelationView,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct ProcedureResult {
    pub(crate) procedure: ProcedureView,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct IntentionResult {
    pub(crate) intention: IntentionView,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct ProactiveResult {
    pub(crate) database: String,
    pub(crate) source_projection: &'static str,
    pub(crate) report: MemoryProactiveReportView,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct DeadlinesResult {
    pub(crate) database: String,
    pub(crate) source_projection: &'static str,
    pub(crate) report: DeadlineReportView,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct AnomaliesResult {
    pub(crate) database: String,
    pub(crate) source_projection: &'static str,
    pub(crate) report: AnomalyReportView,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct AnomalyAcknowledgeResult {
    pub(crate) database: String,
    pub(crate) report: AnomalyAcknowledgementReportView,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct DatabaseReportResult {
    pub(crate) database: String,
    pub(crate) report: Value,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct ProjectionStatusResult {
    pub(crate) database: String,
    pub(crate) projection_role: &'static str,
    pub(crate) status: Value,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct ProjectionReportResult {
    pub(crate) database: String,
    pub(crate) projection_role: &'static str,
    pub(crate) report: Value,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct ProjectionValidationResult {
    pub(crate) database: String,
    pub(crate) projection_role: &'static str,
    pub(crate) validation: Value,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct SemanticStatusResult {
    pub(crate) database: String,
    pub(crate) semantic_index_role: &'static str,
    pub(crate) status: Value,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct SemanticReportResult {
    pub(crate) database: String,
    pub(crate) semantic_index_role: &'static str,
    pub(crate) report: Value,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct BriefingResult {
    pub(crate) report: BriefingReportView,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct MemoryHookResult {
    pub(crate) report: MemoryHookReportView,
}

impl From<IntentionKindArg> for IntentionKind {
    fn from(value: IntentionKindArg) -> Self {
        match value {
            IntentionKindArg::Task => Self::Task,
            IntentionKindArg::Goal => Self::Goal,
            IntentionKindArg::Reminder => Self::Reminder,
        }
    }
}

impl From<IntentionPriorityArg> for IntentionPriority {
    fn from(value: IntentionPriorityArg) -> Self {
        match value {
            IntentionPriorityArg::Low => Self::Low,
            IntentionPriorityArg::Medium => Self::Medium,
            IntentionPriorityArg::High => Self::High,
            IntentionPriorityArg::Critical => Self::Critical,
        }
    }
}

impl From<IntentionStatusArg> for IntentionStatus {
    fn from(value: IntentionStatusArg) -> Self {
        match value {
            IntentionStatusArg::Active => Self::Active,
            IntentionStatusArg::Completed => Self::Completed,
            IntentionStatusArg::Abandoned => Self::Abandoned,
            IntentionStatusArg::Blocked => Self::Blocked,
            IntentionStatusArg::Deferred => Self::Deferred,
        }
    }
}

impl From<MemoryHookKindArg> for MemoryHookKind {
    fn from(value: MemoryHookKindArg) -> Self {
        match value {
            MemoryHookKindArg::SessionStart => Self::SessionStart,
            MemoryHookKindArg::PrePrompt => Self::PrePrompt,
            MemoryHookKindArg::PostAction => Self::PostAction,
            MemoryHookKindArg::SessionClose => Self::SessionClose,
            MemoryHookKindArg::SleepCycle => Self::SleepCycle,
        }
    }
}

impl From<ReviewPriorityArg> for SelfInspectionReviewPriority {
    fn from(value: ReviewPriorityArg) -> Self {
        match value {
            ReviewPriorityArg::Critical => Self::Critical,
            ReviewPriorityArg::High => Self::High,
            ReviewPriorityArg::Medium => Self::Medium,
            ReviewPriorityArg::Low => Self::Low,
        }
    }
}

impl From<ReviewActionArg> for SelfInspectionReviewAction {
    fn from(value: ReviewActionArg) -> Self {
        match value {
            ReviewActionArg::CaptureEvidence => Self::CaptureEvidence,
            ReviewActionArg::ResolveContradiction => Self::ResolveContradiction,
            ReviewActionArg::RefreshMemory => Self::RefreshMemory,
            ReviewActionArg::LinkMemory => Self::LinkMemory,
            ReviewActionArg::ConsolidatePattern => Self::ConsolidatePattern,
            ReviewActionArg::ReviewIntention => Self::ReviewIntention,
        }
    }
}

impl From<RecallKindArg> for MemoryKind {
    fn from(value: RecallKindArg) -> Self {
        match value {
            RecallKindArg::Entity => Self::Entity,
            RecallKindArg::Episode => Self::Episode,
            RecallKindArg::Claim => Self::Claim,
            RecallKindArg::Link => Self::Link,
            RecallKindArg::Procedure => Self::Procedure,
            RecallKindArg::Intention => Self::Intention,
            RecallKindArg::Fact => Self::Fact,
            RecallKindArg::Relation => Self::Relation,
        }
    }
}

impl From<SourceKindArg> for SourceKind {
    fn from(value: SourceKindArg) -> Self {
        match value {
            SourceKindArg::Document => Self::Document,
            SourceKindArg::Conversation => Self::Conversation,
            SourceKindArg::Transcript => Self::Transcript,
            SourceKindArg::WebPage => Self::WebPage,
            SourceKindArg::Note => Self::Note,
            SourceKindArg::Other => Self::Other,
        }
    }
}

impl From<TextChunkingArg> for TextChunking {
    fn from(value: TextChunkingArg) -> Self {
        match value {
            TextChunkingArg::Document => Self::Document,
            TextChunkingArg::Paragraphs => Self::Paragraphs,
            TextChunkingArg::Lines => Self::Lines,
        }
    }
}

impl From<ScopeKindArg> for MemoryScopeKind {
    fn from(value: ScopeKindArg) -> Self {
        match value {
            ScopeKindArg::Personal => Self::Personal,
            ScopeKindArg::Project => Self::Project,
            ScopeKindArg::Organization => Self::Organization,
            ScopeKindArg::Custom => Self::Custom,
        }
    }
}

pub(crate) fn parse_scope_arg(scope: Option<ScopeArg>) -> Result<Option<MemoryScope>, String> {
    scope
        .map(|scope| {
            MemoryScope::new(scope.kind.into(), scope.name).map_err(|error| error.to_string())
        })
        .transpose()
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct RecallToolResult {
    pub(crate) results: Vec<RecallResultView>,
    pub(crate) authority: AuthorityDecisionView,
    pub(crate) health: HealthView,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct GraphResult {
    pub(crate) report: GraphReportView,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct InspectResult {
    pub(crate) health: HealthView,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct SelfInspectResult {
    pub(crate) report: SelfInspectionReportView,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct ReflectResult {
    pub(crate) report: MemoryReflectionReportView,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct ConsolidationPlanResult {
    pub(crate) report: MemoryConsolidationPlanReportView,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct ReviewResult {
    pub(crate) report: OperatorReviewReportView,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct ReviewResolveResult {
    pub(crate) report: Value,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct ValidateResult {
    pub(crate) valid: bool,
    pub(crate) event_count: usize,
    pub(crate) source_count: usize,
    pub(crate) entity_count: usize,
    pub(crate) episode_count: usize,
    pub(crate) claim_count: usize,
    pub(crate) link_count: usize,
    pub(crate) fact_count: usize,
    pub(crate) relation_count: usize,
    pub(crate) procedure_count: usize,
    pub(crate) intention_count: usize,
    pub(crate) review_decision_count: usize,
    pub(crate) last_event_id: Option<String>,
    pub(crate) supported_event_version: u32,
    pub(crate) observed_event_versions: Vec<u32>,
    pub(crate) legacy_event_count: usize,
    pub(crate) migration_required: bool,
    pub(crate) issues: Vec<RecordLedgerIssueView>,
}
