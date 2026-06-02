use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use nahuali_core::{
    DEFAULT_INTENTION_STALE_AFTER_MS, DEFAULT_PROACTIVE_DEADLINE_HORIZON_MS,
    DEFAULT_TEXT_CHUNK_BYTES, IntentionKind, IntentionPriority, IntentionStatus, MemoryHookKind,
    MemoryKind, SelfInspectionReviewAction, SelfInspectionReviewPriority, SourceKind, TextChunking,
};

/// Parse and validate a `--confidence` value, rejecting anything outside the
/// `0.0..=1.0` range instead of silently clamping it.
fn parse_confidence(raw: &str) -> Result<f32, String> {
    let value: f32 = raw
        .parse()
        .map_err(|_| format!("`{raw}` is not a valid number"))?;
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(format!(
            "confidence must be between 0.0 and 1.0 (got {raw})"
        ));
    }
    Ok(value)
}

#[derive(Debug, Parser)]
#[command(name = "nahuali")]
#[command(version)]
#[command(about = "Self-inspecting memory for AI agents")]
#[command(arg_required_else_help = true)]
pub(crate) struct Cli {
    #[arg(
        long = "database",
        global = true,
        value_name = "NAME",
        help = "SurrealDB database name. Defaults to $NAHUALI_DB_DATABASE or memory"
    )]
    pub(crate) database: Option<PathBuf>,

    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    #[command(about = "Print operational memory status.")]
    Status {
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Record an observed episode.")]
    Remember {
        content: Vec<String>,
        #[arg(long = "tag", short = 't')]
        tags: Vec<String>,
        #[arg(long = "mention", short = 'm')]
        mentions: Vec<String>,
        #[arg(long, value_name = "KIND:NAME")]
        scope: Option<String>,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Record an evidence-backed canonical claim.")]
    Claim {
        subject: String,
        predicate: String,
        object: Vec<String>,
        #[arg(long = "source-episode")]
        source_episode_id: Option<String>,
        #[arg(long = "source-last", conflicts_with = "source_episode_id")]
        source_last: bool,
        #[arg(long, short = 'c', default_value_t = 0.8, value_parser = parse_confidence)]
        confidence: f32,
        #[arg(long, value_name = "KIND:NAME")]
        scope: Option<String>,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Record a compatibility fact.")]
    Fact {
        subject: String,
        predicate: String,
        object: Vec<String>,
        #[arg(long = "source-episode")]
        source_episode_id: Option<String>,
        #[arg(long = "source-last", conflicts_with = "source_episode_id")]
        source_last: bool,
        #[arg(long, short = 'c', default_value_t = 0.8, value_parser = parse_confidence)]
        confidence: f32,
        #[arg(long, value_name = "KIND:NAME")]
        scope: Option<String>,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Record a canonical typed link between entities.")]
    Link {
        from: String,
        relation: String,
        to: Vec<String>,
        #[arg(long = "source-episode")]
        source_episode_id: Option<String>,
        #[arg(long = "source-last", conflicts_with = "source_episode_id")]
        source_last: bool,
        #[arg(long, short = 'c', default_value_t = 0.8, value_parser = parse_confidence)]
        confidence: f32,
        #[arg(long, value_name = "KIND:NAME")]
        scope: Option<String>,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Record a compatibility relation between entities.")]
    Relate {
        from: String,
        relation: String,
        to: Vec<String>,
        #[arg(long = "source-episode")]
        source_episode_id: Option<String>,
        #[arg(long = "source-last", conflicts_with = "source_episode_id")]
        source_last: bool,
        #[arg(long, short = 'c', default_value_t = 0.8, value_parser = parse_confidence)]
        confidence: f32,
        #[arg(long, value_name = "KIND:NAME")]
        scope: Option<String>,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Record a reusable procedure.")]
    Procedure {
        name: String,
        body: Vec<String>,
        #[arg(long = "source-episode")]
        source_episode_id: Option<String>,
        #[arg(long = "source-last", conflicts_with = "source_episode_id")]
        source_last: bool,
        #[arg(long, short = 'c', default_value_t = 0.8, value_parser = parse_confidence)]
        confidence: f32,
        #[arg(long, value_name = "KIND:NAME")]
        scope: Option<String>,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Record a reusable preference.")]
    Preference {
        name: String,
        body: Vec<String>,
        #[arg(long = "source-episode")]
        source_episode_id: Option<String>,
        #[arg(long = "source-last", conflicts_with = "source_episode_id")]
        source_last: bool,
        #[arg(long, short = 'c', default_value_t = 0.8, value_parser = parse_confidence)]
        confidence: f32,
        #[arg(long, value_name = "KIND:NAME")]
        scope: Option<String>,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Record a future task, goal, reminder, or commitment.")]
    Intention {
        description: Vec<String>,
        #[arg(long, value_enum, default_value_t = CliIntentionKind::Task)]
        kind: CliIntentionKind,
        #[arg(long, value_enum, default_value_t = CliIntentionPriority::Medium)]
        priority: CliIntentionPriority,
        #[arg(long = "source-episode")]
        source_episode_id: Option<String>,
        #[arg(long = "source-last", conflicts_with = "source_episode_id")]
        source_last: bool,
        #[arg(long, value_name = "KIND:NAME")]
        scope: Option<String>,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Update an existing intention lifecycle state.")]
    IntentionStatus {
        id: String,
        #[arg(value_enum)]
        status: CliIntentionStatus,
        #[arg(long)]
        reason: Option<String>,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Update intention metadata without changing lifecycle state.")]
    IntentionUpdate {
        id: String,
        #[arg(long)]
        description: Option<String>,
        #[arg(long, value_enum)]
        priority: Option<CliIntentionPriority>,
        #[arg(long = "deadline-at-ms", conflicts_with = "clear_deadline")]
        deadline_at_ms: Option<u64>,
        #[arg(long = "clear-deadline")]
        clear_deadline: bool,
        #[arg(long = "depends-on", value_name = "ID")]
        depends_on: Vec<String>,
        #[arg(long = "clear-dependencies")]
        clear_dependencies: bool,
        #[arg(long, conflicts_with = "clear_goal")]
        goal: Option<String>,
        #[arg(long = "clear-goal")]
        clear_goal: bool,
        #[arg(long, conflicts_with = "clear_progress")]
        progress: Option<u8>,
        #[arg(long = "clear-progress")]
        clear_progress: bool,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Mark an intention completed.")]
    IntentionComplete {
        id: String,
        #[arg(long)]
        reason: Option<String>,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Mark an intention blocked.")]
    IntentionBlock {
        id: String,
        #[arg(long)]
        reason: Option<String>,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Mark an intention deferred.")]
    IntentionDefer {
        id: String,
        #[arg(long)]
        reason: Option<String>,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Print a non-mutating intention reconciliation report.")]
    ReconcileIntentions {
        #[arg(long = "now-ms")]
        now_ms: Option<u64>,
        #[arg(long = "stale-after-ms", default_value_t = DEFAULT_INTENTION_STALE_AFTER_MS)]
        stale_after_ms: u64,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Print non-mutating progress for goal intentions.")]
    GoalProgress {
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Print a non-mutating proactive operator report.")]
    Proactive {
        #[arg(long = "now-ms")]
        now_ms: Option<u64>,
        #[arg(
            long = "deadline-horizon-ms",
            default_value_t = DEFAULT_PROACTIVE_DEADLINE_HORIZON_MS
        )]
        deadline_horizon_ms: u64,
        #[arg(long = "stale-after-ms", default_value_t = DEFAULT_INTENTION_STALE_AFTER_MS)]
        stale_after_ms: u64,
        #[arg(long = "review-limit", default_value_t = 20)]
        review_limit: usize,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Print non-mutating proactive deadline signals.")]
    Deadlines {
        #[arg(long = "now-ms")]
        now_ms: Option<u64>,
        #[arg(
            long = "horizon-ms",
            default_value_t = DEFAULT_PROACTIVE_DEADLINE_HORIZON_MS
        )]
        horizon_ms: u64,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Print non-mutating proactive anomaly alerts.")]
    Anomalies {
        #[arg(long = "now-ms")]
        now_ms: Option<u64>,
        #[arg(long = "deadline-horizon-ms", default_value_t = 0)]
        deadline_horizon_ms: u64,
        #[arg(long = "stale-after-ms", default_value_t = DEFAULT_INTENTION_STALE_AFTER_MS)]
        stale_after_ms: u64,
        #[arg(long = "review-limit", default_value_t = 20)]
        review_limit: usize,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Acknowledge a proactive anomaly with an explicit audit note.")]
    AnomalyAcknowledge {
        id: String,
        #[arg(long)]
        note: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Print a compact non-mutating session briefing.")]
    Briefing {
        #[arg(long = "episode-limit", default_value_t = 5)]
        episode_limit: usize,
        #[arg(long = "intention-limit", default_value_t = 5)]
        intention_limit: usize,
        #[arg(long = "review-limit", default_value_t = 5)]
        review_limit: usize,
        #[arg(long = "graph-seed-limit", default_value_t = 8)]
        graph_seed_limit: usize,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Resume a session with briefing, authority, and review context.")]
    SessionResume {
        #[arg(long = "episode-limit", default_value_t = 5)]
        episode_limit: usize,
        #[arg(long = "intention-limit", default_value_t = 5)]
        intention_limit: usize,
        #[arg(long = "review-limit", default_value_t = 5)]
        review_limit: usize,
        #[arg(long = "graph-seed-limit", default_value_t = 8)]
        graph_seed_limit: usize,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Read recent memory timeline from the SurrealDB projection.")]
    Timeline {
        #[arg(long, short = 'l', default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Read pending intentions from the SurrealDB projection.")]
    Pending {
        #[arg(long, short = 'l', default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Run a non-mutating memory sleep pass.")]
    Sleep {
        #[arg(long = "episode-limit", default_value_t = 8)]
        episode_limit: usize,
        #[arg(long = "candidate-limit", default_value_t = 12)]
        candidate_limit: usize,
        #[arg(long = "cycle-limit", default_value_t = 8)]
        cycle_limit: usize,
        #[arg(long = "evidence-limit", default_value_t = 8)]
        evidence_limit: usize,
        #[arg(long)]
        json: bool,
    },
    #[command(
        about = "Plan replay, review gates, and write-back eligibility without mutating memory."
    )]
    ConsolidationPlan {
        #[arg(long = "episode-limit", default_value_t = 8)]
        episode_limit: usize,
        #[arg(long = "candidate-limit", default_value_t = 12)]
        candidate_limit: usize,
        #[arg(long = "cycle-limit", default_value_t = 8)]
        cycle_limit: usize,
        #[arg(long = "evidence-limit", default_value_t = 8)]
        evidence_limit: usize,
        #[arg(long = "review-limit", default_value_t = 20)]
        review_limit: usize,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Run a non-mutating memory hook for a host execution point.")]
    Hook {
        #[arg(value_enum)]
        kind: CliMemoryHookKind,
        #[arg(long)]
        input: Option<String>,
        #[arg(long = "recall-limit", default_value_t = 10)]
        recall_limit: usize,
        #[arg(long = "episode-limit", default_value_t = 5)]
        episode_limit: usize,
        #[arg(long = "intention-limit", default_value_t = 5)]
        intention_limit: usize,
        #[arg(long = "review-limit", default_value_t = 5)]
        review_limit: usize,
        #[arg(long = "graph-seed-limit", default_value_t = 8)]
        graph_seed_limit: usize,
        #[arg(long = "cycle-limit", default_value_t = 8)]
        cycle_limit: usize,
        #[arg(long = "evidence-limit", default_value_t = 8)]
        evidence_limit: usize,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Recall matching memory with optional authority context.")]
    Recall {
        query: Vec<String>,
        #[arg(long, short = 'l', default_value_t = 10)]
        limit: usize,
        #[arg(long)]
        authority: bool,
        #[arg(long, conflicts_with = "authority")]
        semantic: bool,
        #[arg(long, value_name = "KIND:NAME")]
        scope: Option<String>,
        #[arg(long = "kind", value_enum)]
        kinds: Vec<CliRecallKind>,
        #[arg(long = "require-evidence")]
        require_evidence: bool,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Traverse the projected memory graph around a seed.")]
    Graph {
        seed: Vec<String>,
        #[arg(long, default_value_t = 2)]
        depth: usize,
        #[arg(long, short = 'l', default_value_t = 100)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Print a focused project or entity dashboard.")]
    Project {
        #[arg(required = true)]
        entity: Vec<String>,
        #[arg(long = "graph-depth", default_value_t = 2)]
        graph_depth: usize,
        #[arg(long = "graph-limit", default_value_t = 100)]
        graph_limit: usize,
        #[arg(long = "item-limit", short = 'l', default_value_t = 10)]
        item_limit: usize,
        #[arg(long = "recall-limit", default_value_t = 10)]
        recall_limit: usize,
        #[arg(long = "review-limit", default_value_t = 10)]
        review_limit: usize,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Rebuild the derived Qdrant semantic index from the Rust projection.")]
    SemanticRebuild {
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Inspect the derived Qdrant semantic index status.")]
    SemanticStatus {
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Inspect the SurrealDB graph projection status.")]
    ProjectionStatus {
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Rebuild the SurrealDB graph projection from the record ledger.")]
    ProjectionRebuild {
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Validate the SurrealDB graph projection without mutating it.")]
    ProjectionValidate {
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Read projected entities from SurrealDB graph tables.")]
    ProjectionEntities {
        query: Vec<String>,
        #[arg(long, short = 'l', default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Read projected episode timeline from SurrealDB graph tables.")]
    ProjectionTimeline {
        #[arg(long, short = 'l', default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Read projected pending intentions from SurrealDB graph tables.")]
    ProjectionPending {
        #[arg(long, short = 'l', default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Read projected health signals from SurrealDB graph tables.")]
    ProjectionHealth {
        #[arg(long, short = 'l', default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Inspect knowledge health before trusting memory.")]
    Inspect {
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Produce a non-mutating self-inspection consolidation report.")]
    SelfInspect {
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Plan a non-mutating reflection cycle for operator approval.")]
    Reflect {
        #[arg(long = "cycle-limit", default_value_t = 8)]
        cycle_limit: usize,
        #[arg(long = "evidence-limit", default_value_t = 8)]
        evidence_limit: usize,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Print a prioritized non-mutating operator review queue.")]
    Review {
        #[arg(long, short = 'l', default_value_t = 20)]
        limit: usize,
        #[arg(long = "min-priority", value_enum)]
        min_priority: Option<CliReviewPriority>,
        #[arg(long, value_enum)]
        action: Option<CliReviewAction>,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Resolve an operator review item with an explicit audit note.")]
    ReviewResolve {
        review_id: String,
        #[arg(long)]
        note: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Validate the SurrealDB memory_record ledger without mutating it.")]
    Validate {
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Print a non-destructive maintenance report.")]
    Maintenance {
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Write or dry-run an optional projection snapshot.")]
    Snapshot {
        #[arg(long, short = 'o', value_name = "PATH")]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Validate an optional snapshot against record replay.")]
    SnapshotValidate {
        #[arg(value_name = "PATH")]
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Write or dry-run a local record-ledger backup.")]
    Backup {
        #[arg(long, short = 'o', value_name = "PATH")]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Validate a local record-ledger backup.")]
    BackupValidate {
        #[arg(value_name = "PATH")]
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Validate a backup and dry-run restore into a target database.")]
    BackupDrill {
        #[arg(value_name = "PATH")]
        path: PathBuf,
        #[arg(long = "target-database", value_name = "NAME")]
        target_database: PathBuf,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Restore a backup into an empty SurrealDB database.")]
    Restore {
        #[arg(value_name = "PATH")]
        path: PathBuf,
        #[arg(long = "target-database", value_name = "NAME")]
        target_database: PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Export a source-neutral memory interchange document.")]
    Export {
        #[arg(long, short = 'o', value_name = "PATH")]
        output: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Import a source-neutral memory interchange document.")]
    Import {
        #[arg(value_name = "PATH")]
        path: PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    #[command(
        about = "Convert a projected memory export into a source-neutral interchange document."
    )]
    ConvertProjectionExport {
        #[arg(value_name = "PATH")]
        path: PathBuf,
        #[arg(long, short = 'o', value_name = "PATH")]
        output: PathBuf,
        #[arg(long, value_name = "KIND:NAME")]
        scope: Option<String>,
        #[arg(long)]
        json: bool,
    },
    #[command(
        about = "Convert a historical structured or SurrealQL export into a source-neutral interchange document."
    )]
    ConvertLegacyExport {
        #[arg(value_name = "PATH")]
        path: PathBuf,
        #[arg(long, short = 'o', value_name = "PATH")]
        output: PathBuf,
        #[arg(long, value_name = "KIND:NAME")]
        scope: Option<String>,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Ingest a provenance-aware source document.")]
    Ingest {
        #[arg(value_name = "PATH")]
        path: PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Ingest a local text file as provenance-preserving source episodes.")]
    IngestText {
        #[arg(value_name = "PATH")]
        path: PathBuf,
        #[arg(long, value_enum, default_value_t = CliSourceKind::Document)]
        kind: CliSourceKind,
        #[arg(long)]
        title: Option<String>,
        #[arg(long, value_enum, default_value_t = CliTextChunking::Document)]
        chunking: CliTextChunking,
        #[arg(long = "tag", short = 't')]
        tags: Vec<String>,
        #[arg(long = "mention", short = 'm')]
        mentions: Vec<String>,
        #[arg(long = "metadata", value_name = "KEY=VALUE")]
        metadata: Vec<String>,
        #[arg(long = "role")]
        source_role: Option<String>,
        #[arg(long, value_name = "KIND:NAME")]
        scope: Option<String>,
        #[arg(long = "max-chunk-bytes", default_value_t = DEFAULT_TEXT_CHUNK_BYTES)]
        max_chunk_bytes: usize,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Ingest a directory of local text files after full batch preflight.")]
    IngestDir {
        #[arg(value_name = "PATH")]
        path: PathBuf,
        #[arg(long)]
        recursive: bool,
        #[arg(long = "extension", value_name = "EXT")]
        extensions: Vec<String>,
        #[arg(long, value_enum, default_value_t = CliSourceKind::Document)]
        kind: CliSourceKind,
        #[arg(long, value_enum, default_value_t = CliTextChunking::Document)]
        chunking: CliTextChunking,
        #[arg(long = "tag", short = 't')]
        tags: Vec<String>,
        #[arg(long = "mention", short = 'm')]
        mentions: Vec<String>,
        #[arg(long = "metadata", value_name = "KEY=VALUE")]
        metadata: Vec<String>,
        #[arg(long = "role")]
        source_role: Option<String>,
        #[arg(long, value_name = "KIND:NAME")]
        scope: Option<String>,
        #[arg(long = "max-chunk-bytes", default_value_t = DEFAULT_TEXT_CHUNK_BYTES)]
        max_chunk_bytes: usize,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Print the projected memory data.")]
    Data {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Clone, Debug, ValueEnum)]
pub(crate) enum CliIntentionKind {
    Task,
    Goal,
    Reminder,
}

#[derive(Clone, Debug, ValueEnum)]
pub(crate) enum CliIntentionPriority {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Clone, Debug, ValueEnum)]
pub(crate) enum CliIntentionStatus {
    Active,
    Completed,
    Abandoned,
    Blocked,
    Deferred,
}

#[derive(Clone, Debug, ValueEnum)]
pub(crate) enum CliReviewPriority {
    Critical,
    High,
    Medium,
    Low,
}

#[derive(Clone, Debug, ValueEnum)]
pub(crate) enum CliReviewAction {
    CaptureEvidence,
    ResolveContradiction,
    RefreshMemory,
    LinkMemory,
    ConsolidatePattern,
    ReviewIntention,
}

#[derive(Clone, Debug, ValueEnum)]
pub(crate) enum CliRecallKind {
    Entity,
    Episode,
    Claim,
    Link,
    Procedure,
    Intention,
    Fact,
    Relation,
}

#[derive(Clone, Debug, ValueEnum)]
pub(crate) enum CliMemoryHookKind {
    SessionStart,
    PrePrompt,
    PostAction,
    SessionClose,
    SleepCycle,
}

#[derive(Clone, Debug, ValueEnum)]
pub(crate) enum CliTextChunking {
    Document,
    Paragraphs,
    Lines,
}

#[derive(Clone, Debug, ValueEnum)]
pub(crate) enum CliSourceKind {
    Document,
    Conversation,
    Transcript,
    WebPage,
    Note,
    Other,
}

impl From<CliIntentionKind> for IntentionKind {
    fn from(value: CliIntentionKind) -> Self {
        match value {
            CliIntentionKind::Task => Self::Task,
            CliIntentionKind::Goal => Self::Goal,
            CliIntentionKind::Reminder => Self::Reminder,
        }
    }
}

impl From<CliIntentionPriority> for IntentionPriority {
    fn from(value: CliIntentionPriority) -> Self {
        match value {
            CliIntentionPriority::Low => Self::Low,
            CliIntentionPriority::Medium => Self::Medium,
            CliIntentionPriority::High => Self::High,
            CliIntentionPriority::Critical => Self::Critical,
        }
    }
}

impl From<CliIntentionStatus> for IntentionStatus {
    fn from(value: CliIntentionStatus) -> Self {
        match value {
            CliIntentionStatus::Active => Self::Active,
            CliIntentionStatus::Completed => Self::Completed,
            CliIntentionStatus::Abandoned => Self::Abandoned,
            CliIntentionStatus::Blocked => Self::Blocked,
            CliIntentionStatus::Deferred => Self::Deferred,
        }
    }
}

impl From<CliReviewPriority> for SelfInspectionReviewPriority {
    fn from(value: CliReviewPriority) -> Self {
        match value {
            CliReviewPriority::Critical => Self::Critical,
            CliReviewPriority::High => Self::High,
            CliReviewPriority::Medium => Self::Medium,
            CliReviewPriority::Low => Self::Low,
        }
    }
}

impl From<CliReviewAction> for SelfInspectionReviewAction {
    fn from(value: CliReviewAction) -> Self {
        match value {
            CliReviewAction::CaptureEvidence => Self::CaptureEvidence,
            CliReviewAction::ResolveContradiction => Self::ResolveContradiction,
            CliReviewAction::RefreshMemory => Self::RefreshMemory,
            CliReviewAction::LinkMemory => Self::LinkMemory,
            CliReviewAction::ConsolidatePattern => Self::ConsolidatePattern,
            CliReviewAction::ReviewIntention => Self::ReviewIntention,
        }
    }
}

impl From<CliRecallKind> for MemoryKind {
    fn from(value: CliRecallKind) -> Self {
        match value {
            CliRecallKind::Entity => Self::Entity,
            CliRecallKind::Episode => Self::Episode,
            CliRecallKind::Claim => Self::Claim,
            CliRecallKind::Link => Self::Link,
            CliRecallKind::Procedure => Self::Procedure,
            CliRecallKind::Intention => Self::Intention,
            CliRecallKind::Fact => Self::Fact,
            CliRecallKind::Relation => Self::Relation,
        }
    }
}

impl From<CliMemoryHookKind> for MemoryHookKind {
    fn from(value: CliMemoryHookKind) -> Self {
        match value {
            CliMemoryHookKind::SessionStart => Self::SessionStart,
            CliMemoryHookKind::PrePrompt => Self::PrePrompt,
            CliMemoryHookKind::PostAction => Self::PostAction,
            CliMemoryHookKind::SessionClose => Self::SessionClose,
            CliMemoryHookKind::SleepCycle => Self::SleepCycle,
        }
    }
}

impl From<CliTextChunking> for TextChunking {
    fn from(value: CliTextChunking) -> Self {
        match value {
            CliTextChunking::Document => Self::Document,
            CliTextChunking::Paragraphs => Self::Paragraphs,
            CliTextChunking::Lines => Self::Lines,
        }
    }
}

impl From<CliSourceKind> for SourceKind {
    fn from(value: CliSourceKind) -> Self {
        match value {
            CliSourceKind::Document => Self::Document,
            CliSourceKind::Conversation => Self::Conversation,
            CliSourceKind::Transcript => Self::Transcript,
            CliSourceKind::WebPage => Self::WebPage,
            CliSourceKind::Note => Self::Note,
            CliSourceKind::Other => Self::Other,
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::Cli;

    #[test]
    fn top_level_help_documents_database_and_primary_commands() {
        let mut command = Cli::command();
        let help = command.render_long_help().to_string();

        assert!(help.contains("Defaults to $NAHUALI_DB_DATABASE"));
        assert!(help.contains("remember"));
        assert!(help.contains("briefing"));
        assert!(help.contains("intention-update"));
        assert!(help.contains("reconcile-intentions"));
        assert!(help.contains("goal-progress"));
        assert!(help.contains("sleep"));
        assert!(help.contains("consolidation-plan"));
        assert!(help.contains("hook"));
        assert!(help.contains("recall"));
        assert!(help.contains("self-inspect"));
        assert!(help.contains("reflect"));
        assert!(help.contains("review"));
        assert!(help.contains("ingest-text"));
        assert!(help.contains("ingest-dir"));
        assert!(help.contains("validate"));
    }

    #[test]
    fn recall_help_documents_authority_json_path() {
        let mut command = Cli::command();
        let recall = command
            .find_subcommand_mut("recall")
            .expect("recall subcommand exists");
        let help = recall.render_long_help().to_string();

        assert!(help.contains("Recall matching memory"));
        assert!(help.contains("--authority"));
        assert!(help.contains("--scope"));
        assert!(help.contains("--json"));
    }

    #[test]
    fn validate_help_documents_non_destructive_path() {
        let mut command = Cli::command();
        let validate = command
            .find_subcommand_mut("validate")
            .expect("validate subcommand exists");
        let help = validate.render_long_help().to_string();

        assert!(help.contains("without mutating"));
        assert!(help.contains("memory_record ledger"));
        assert!(help.contains("--json"));
    }

    #[test]
    fn maintenance_help_documents_snapshot_paths() {
        let mut command = Cli::command();
        let snapshot = command
            .find_subcommand_mut("snapshot")
            .expect("snapshot subcommand exists");
        let help = snapshot.render_long_help().to_string();

        assert!(help.contains("optional projection snapshot"));
        assert!(help.contains("--output"));
        assert!(help.contains("--dry-run"));
    }
}
