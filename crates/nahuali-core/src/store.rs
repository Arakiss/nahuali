use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    AnomalyAcknowledgementOptions, AnomalyAcknowledgementReport, AnomalyReport, AuthorityDecision,
    AuthorityRecall, AutonomyLevel, BackupDrillReport, BackupRestoreReport, BackupValidation,
    BackupValidationOptions, BriefingOptions, ConsolidationPlanOptions, DeadlineReport,
    EpisodeRecorded, GraphTraversalOptions, IngestionReport, IntentionReconciliationOptions,
    IntentionReconciliationReport, IntentionUpdateOptions, MemoryBackup, MemoryBriefingReport,
    MemoryConsolidationPlanReport, MemoryGraphReport, MemoryHookKind, MemoryHookOptions,
    MemoryHookReport, MemoryIngestDocument, MemoryProactiveReport, MemoryReflectionReport,
    MemorySleepReport, ProactiveOptions, RecallOptions, ReflectionOptions, RepairKind,
    RepairProposal, RepairReport, ReviewResolutionOptions, ReviewResolutionReport,
    SleepModeOptions,
    backup::{self, SemanticTierRestorePolicy},
    briefing, consolidation_plan,
    database::{database_name, normalized_endpoint, resolved_namespace},
    error::{NahualiError, Result},
    event::{
        EVENT_ENVELOPE_VERSION, EventEnvelope, FactAsserted, IntentionRecorded,
        IntentionRecordedKind, IntentionRecordedPriority, IntentionRecordedStatus,
        IntentionStatusChanged, IntentionUpdated, MemoryEvent, ProcedureRecorded,
        ProcedureRecordedKind, RelationRecorded, SourceRecorded, SourceRecordedKind,
    },
    graph, hooks, ingestion,
    inspection::KnowledgeHealth,
    intention,
    interchange::{self, InterchangeImportReport, MemoryInterchange},
    maintenance::{
        self, MaintenanceReport, MemorySnapshot, SnapshotValidation, record_ledger_checksum,
    },
    model::{
        Claim, Episode, Fact, Intention, IntentionKind, IntentionPriority, IntentionStatus, Link,
        MEMORY_DATA_VERSION, MemoryData, MemoryScope, Procedure, ProcedureKind, RecallResult,
        Relation, SourceDocument, SourceKind,
    },
    operator_review::{self, OperatorReviewOptions, OperatorReviewReport},
    proactive,
    project::{self, MemoryProjectReport, ProjectViewOptions},
    projection, recall, reflection, review_writeback,
    schema::{GRAPH_PROJECTION_SCHEMA, MEMORY_RECORD_SCHEMA},
    self_inspection,
    self_inspection::SelfInspectionReport,
    self_repair, semantic,
    semantic::{HybridRecallReport, SemanticConfig, SemanticIndexReport, SemanticIndexStatus},
    sleep,
    validation::{
        RecordLedgerValidation, RecordLedgerValidationOptions, validate_record_ledger,
        validate_record_ledger_with_options,
    },
};
use serde::Deserialize;
use surrealdb::{
    Surreal,
    engine::remote::ws::{Client, Ws},
    opt::auth::Root,
};
use tokio::runtime::{Builder, Runtime};

static ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// SurrealDB-backed memory engine.
///
/// `MemoryEngine` validates the SurrealDB record ledger on open, projects the
/// current state in memory, and writes each mutation to the `memory_record`
/// table before refreshing the projection. The [`crate::LocalMemory`] alias
/// remains exported for compatibility with the initial pre-release Rust
/// foundation.
#[derive(Clone, Debug)]
pub struct MemoryEngine {
    path: PathBuf,
    events: Vec<EventEnvelope>,
    data: MemoryData,
    next_sequence: u64,
    /// When `true`, `append_at` accumulates events in memory without touching
    /// the database; the buffered records and a single graph rebuild are flushed
    /// once when the surrounding import batch commits. Off for ordinary single
    /// mutations, which still persist synchronously and stay byte-identical.
    batch_active: bool,
}

/// Options for registering provenance source material.
#[derive(Clone, Debug)]
pub struct SourceRecordOptions {
    /// Source category.
    pub kind: SourceKind,
    /// Human-readable source title, when available.
    pub title: Option<String>,
    /// Source URI, path, or adapter-provided locator, when available.
    pub uri: Option<String>,
    /// Deterministic checksum over the ingested source material.
    pub content_checksum: String,
    /// Total number of content bytes represented by this source.
    pub byte_len: u64,
    /// Adapter-provided metadata preserved as source provenance.
    pub metadata: BTreeMap<String, String>,
    /// Explicit memory context boundary, when provided.
    pub scope: Option<MemoryScope>,
}

/// Options for recording an episode from registered source material.
#[derive(Clone, Debug)]
pub struct SourceEpisodeOptions {
    /// Natural-language content recorded for the episode.
    pub content: String,
    /// User-provided labels for filtering or recall.
    pub tags: Vec<String>,
    /// Explicit entity names mentioned by this episode.
    pub mentions: Vec<String>,
    /// Source record that produced this episode.
    pub source_id: String,
    /// Stable position within the source, when known.
    pub source_position: Option<u32>,
    /// Source-local actor, role, or speaker, when known.
    pub source_role: Option<String>,
    /// Explicit memory context boundary, when provided.
    pub scope: Option<MemoryScope>,
}

include!("store/records.rs");
include!("store/import_records.rs");
include!("store/recall.rs");
include!("store/services.rs");
include!("store/ledger.rs");
include!("store/graph_projection.rs");
include!("store/tests.rs");
