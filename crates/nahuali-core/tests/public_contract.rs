#[path = "public_contract_cases/support.rs"]
mod public_contract_support;

use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use nahuali_core::{
    AnomalyKind, AuthorityMode, BackupIssueKind, BriefingOptions, Claim,
    ConsolidationOperationKind, ConsolidationOperationStatus, ConsolidationPlanOptions,
    DEFAULT_SEMANTIC_COLLECTION, DEFAULT_TEXT_CHUNK_BYTES, DeadlineState, EventEnvelope, Fact,
    HealthDimension, HealthSeverity, HealthSignalKind, IngestClaim, IngestEpisode, IngestIntention,
    IngestLink, IngestProcedure, IngestSource, IngestionIssueKind, IntentionKind,
    IntentionPriority, IntentionReconciliationIssueKind, IntentionReconciliationOptions,
    IntentionStatus, IntentionUpdateOptions, InterchangeIssueKind, Link, LocalMemory,
    MEMORY_BACKUP_VERSION, MEMORY_BRIEFING_VERSION, MEMORY_CONSOLIDATION_PLAN_VERSION,
    MEMORY_GRAPH_VERSION, MEMORY_HOOK_REPORT_VERSION, MEMORY_INGEST_DOCUMENT_VERSION,
    MEMORY_INTERCHANGE_VERSION, MEMORY_PROACTIVE_REPORT_VERSION, MEMORY_PROJECT_VIEW_VERSION,
    MEMORY_REFLECTION_VERSION, MEMORY_SLEEP_REPORT_VERSION, MemoryEngine, MemoryGraphEdgeKind,
    MemoryGraphNodeKind, MemoryHookKind, MemoryHookOptions, MemoryIngestDocument,
    MemoryInterchange, MemoryKind, MemoryScope, MemoryScopeKind, NahualiError,
    OPERATOR_REVIEW_VERSION, OperatorReviewOptions, ProactiveOptions, ProcedureKind,
    ProjectViewOptions, RecallOptions, RecallResultTrustMode, ReflectionOptions, Relation,
    ReviewDecisionOutcome, SelfInspectionFindingKind, SelfInspectionReviewAction,
    SelfInspectionReviewPriority, SemanticTierProvider, SemanticTierRestorePolicy,
    SemanticTierSnapshotStatus, SleepConsolidationCandidateKind, SleepModeOptions,
    SnapshotIssueKind, SourceKind, TEXT_INGEST_ADAPTER_VERSION, TextChunking, TextIngestIssueKind,
    TextIngestOptions, build_text_ingest_document,
};
use public_contract_support::{semantic_test_config, temp_store};

include!("public_contract_cases/core.rs");
include!("public_contract_cases/adapters.rs");
include!("public_contract_cases/review.rs");
include!("public_contract_cases/backup.rs");
