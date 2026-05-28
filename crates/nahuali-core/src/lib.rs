//! Canonical self-inspecting memory engine for Nahuali.
//!
//! `nahuali-core` owns the record envelope model, deterministic projection,
//! lexical recall, and knowledge-health inspection used by the CLI, MCP server,
//! and local HTTP API. Persistence is backed by SurrealDB.
//!
//! Opening a memory database validates record sequence order and checksums
//! before projecting state, so callers can inspect memory without trusting a
//! mutable snapshot.
//!
//! # Minimal example
//!
//! ```no_run
//! use nahuali_core::MemoryEngine;
//!
//! # fn main() -> nahuali_core::Result<()> {
//! let mut memory = MemoryEngine::open("memory")?;
//! let episode = memory.remember("Lena owns the release notes.", vec!["product".into()])?;
//! memory.add_claim("Lena", "owns", "release notes", Some(episode.id), 0.92)?;
//!
//! let results = memory.recall("Lena release", 10)?;
//! let health = memory.inspect();
//! let self_inspection = memory.self_inspect();
//!
//! assert!(!results.is_empty());
//! assert_eq!(health.unsupported_fact_count, 0);
//! assert!(!self_inspection.write_back_policy.automatic_write_back);
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod adapters;
mod authority;
mod backup;
mod briefing;
mod consolidation_plan;
mod database;
mod error;
mod event;
mod graph;
mod hooks;
mod ingestion;
mod inspection;
mod intention;
mod interchange;
mod maintenance;
mod model;
mod operator_review;
mod proactive;
mod project;
mod projection;
mod recall;
mod reflection;
mod review_writeback;
mod schema;
mod self_inspection;
mod semantic;
mod sleep;
mod store;
mod validation;

pub use adapters::{
    DEFAULT_TEXT_CHUNK_BYTES, TEXT_INGEST_ADAPTER_VERSION, TextChunking, TextIngestBuildReport,
    TextIngestIssue, TextIngestIssueKind, TextIngestOptions, build_text_ingest_document,
};
pub use authority::{AuthorityDecision, AuthorityMode, AuthorityRecall};
pub use backup::{
    BackupDrillReport, BackupIssue, BackupIssueKind, BackupIssueSeverity, BackupRestoreReport,
    BackupSummary, BackupValidation, MEMORY_BACKUP_VERSION, MemoryBackup, SemanticTierBackup,
    SemanticTierProvider, SemanticTierRestorePolicy, SemanticTierSnapshotStatus,
};
pub use briefing::{
    BriefingEpisode, BriefingGraphSeed, BriefingIntention, BriefingOptions, BriefingSummary,
    MEMORY_BRIEFING_VERSION, MemoryBriefingReport,
};
pub use consolidation_plan::{
    ConsolidationBlockedItem, ConsolidationGate, ConsolidationOperation,
    ConsolidationOperationKind, ConsolidationOperationStatus, ConsolidationPlanOptions,
    ConsolidationPlanSummary, ConsolidationStage, ConsolidationStageStatus,
    MEMORY_CONSOLIDATION_PLAN_VERSION, MemoryConsolidationPlanReport,
};
pub use error::{NahualiError, Result};
pub use event::{
    EVENT_ENVELOPE_VERSION, EpisodeRecorded, EventEnvelope, FactAsserted, IntentionRecorded,
    IntentionRecordedKind, IntentionRecordedPriority, IntentionRecordedStatus,
    IntentionStatusChanged, IntentionUpdated, MemoryEvent, ProcedureRecorded,
    ProcedureRecordedKind, RelationRecorded, ReviewRecorded, ReviewRecordedAction,
    ReviewRecordedOutcome, SourceRecorded, SourceRecordedKind,
};
pub use graph::{
    GraphTraversalOptions, MEMORY_GRAPH_VERSION, MemoryGraphEdge, MemoryGraphEdgeKind,
    MemoryGraphNode, MemoryGraphNodeKind, MemoryGraphReport, MemoryGraphSummary,
};
pub use hooks::{
    MEMORY_HOOK_REPORT_VERSION, MemoryHookDirective, MemoryHookDirectivePriority, MemoryHookKind,
    MemoryHookOptions, MemoryHookReport, MemoryHookSummary,
};
pub use ingestion::{
    IngestClaim, IngestEpisode, IngestIntention, IngestLink, IngestProcedure, IngestSource,
    IngestionCounts, IngestionIssue, IngestionIssueKind, IngestionPreflight, IngestionReport,
    MEMORY_INGEST_DOCUMENT_VERSION, MemoryIngestDocument,
};
pub use inspection::{
    HealthDimension, HealthSeverity, HealthSignal, HealthSignalKind, KnowledgeHealth,
};
pub use intention::{
    DEFAULT_INTENTION_STALE_AFTER_MS, GOAL_PROGRESS_VERSION, GoalProgress, GoalProgressReport,
    INTENTION_RECONCILIATION_VERSION, IntentionReconciliationIssue,
    IntentionReconciliationIssueKind, IntentionReconciliationOptions,
    IntentionReconciliationPriority, IntentionReconciliationReport, IntentionUpdateOptions,
};
pub use interchange::{
    InterchangeClaim, InterchangeEpisode, InterchangeImportCounts, InterchangeImportPreflight,
    InterchangeImportReadiness, InterchangeImportReport, InterchangeIntention, InterchangeIssue,
    InterchangeIssueKind, InterchangeLink, InterchangeProcedure, InterchangeSource,
    MEMORY_INTERCHANGE_VERSION, MemoryInterchange,
};
pub use maintenance::{
    MEMORY_SNAPSHOT_VERSION, MaintenanceReport, MemorySnapshot, SnapshotIssue, SnapshotIssueKind,
    SnapshotIssueSeverity, SnapshotSummary, SnapshotValidation,
};
pub use model::{
    Claim, Entity, Episode, Fact, Intention, IntentionKind, IntentionPriority, IntentionStatus,
    Link, MEMORY_DATA_VERSION, MemoryData, MemoryKind, MemoryScope, MemoryScopeKind, Procedure,
    ProcedureKind, RecallResult, RecallResultTrust, RecallResultTrustMode, Relation,
    ReviewDecision, ReviewDecisionAction, ReviewDecisionOutcome, SourceDocument, SourceKind,
};
pub use operator_review::{
    OPERATOR_REVIEW_VERSION, OperatorReviewItem, OperatorReviewOptions, OperatorReviewReport,
    OperatorReviewSummary,
};
pub use proactive::{
    AnomalyAcknowledgementOptions, AnomalyAcknowledgementReport, AnomalyAlert, AnomalyKind,
    AnomalyReport, AnomalySummary, CaptureOpportunity, DEFAULT_PROACTIVE_DEADLINE_HORIZON_MS,
    DeadlineReport, DeadlineSignal, DeadlineState, DeadlineSummary,
    MEMORY_ANOMALY_ACKNOWLEDGEMENT_VERSION, MEMORY_ANOMALY_REPORT_VERSION,
    MEMORY_DEADLINE_REPORT_VERSION, MEMORY_PROACTIVE_REPORT_VERSION, MemoryProactiveReport,
    ProactiveOptions, ProactivePriority, ProactiveSummary,
};
pub use project::{
    MEMORY_PROJECT_VIEW_VERSION, MemoryProjectReport, ProjectViewOptions, ProjectViewSummary,
};
pub use recall::RecallOptions;
pub use reflection::{
    MEMORY_REFLECTION_VERSION, MemoryReflectionReport, ReflectionCycle, ReflectionFinding,
    ReflectionOptions, ReflectionSourceCoverage, ReflectionSummary,
};
pub use review_writeback::{
    REVIEW_RESOLUTION_VERSION, ReviewResolutionOptions, ReviewResolutionReport,
};
pub use schema::{GRAPH_PROJECTION_SCHEMA, MEMORY_RECORD_SCHEMA};
pub use self_inspection::{
    SELF_INSPECTION_REPORT_VERSION, SelfInspectionFinding, SelfInspectionFindingKind,
    SelfInspectionReport, SelfInspectionReviewAction, SelfInspectionReviewItem,
    SelfInspectionReviewPriority, SelfInspectionReviewStatus, SelfInspectionSummary,
    SelfInspectionWriteBackPolicy,
};
pub use semantic::{
    DEFAULT_EMBEDDING_DIMENSIONS, DEFAULT_QDRANT_URL, DEFAULT_SEMANTIC_COLLECTION,
    EmbeddingProviderConfig, EmbeddingProviderKind, HybridRecallReport, HybridRecallResult,
    SEMANTIC_INDEX_SCHEMA_VERSION, SemanticConfig, SemanticIndexReport, SemanticIndexStatus,
    SemanticMatch, SemanticPointSummary,
};
pub use sleep::{
    MEMORY_SLEEP_REPORT_VERSION, MemorySleepReport, SleepConsolidationCandidate,
    SleepConsolidationCandidateKind, SleepEpisodeReplay, SleepModeOptions, SleepModeSummary,
    SleepStage, SleepStageStatus,
};
pub use store::{
    GRAPH_PROJECTION_VERSION, GraphProjectionEntity, GraphProjectionEpisode,
    GraphProjectionHealthSignal, GraphProjectionPendingIntention, GraphProjectionRebuildReport,
    GraphProjectionStatus, GraphProjectionValidation, MemoryEngine, SourceEpisodeOptions,
    SourceRecordOptions,
};
/// Compatibility alias for the initial pre-release memory engine name.
///
/// New Rust callers should use [`MemoryEngine`]. This alias exists so existing
/// pre-release integrations keep compiling while the public API moves away from
/// storage-location naming.
pub type LocalMemory = MemoryEngine;
pub use validation::{
    RecordLedgerIssue, RecordLedgerIssueKind, RecordLedgerIssueSeverity, RecordLedgerValidation,
    validate_record_ledger,
};
