use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use nahuali_core::{
    DEFAULT_INTENTION_STALE_AFTER_MS, DEFAULT_PROACTIVE_DEADLINE_HORIZON_MS,
    DEFAULT_TEXT_CHUNK_BYTES, IntentionKind, IntentionPriority, IntentionStatus, MemoryHookKind,
    MemoryKind, SelfInspectionReviewAction, SelfInspectionReviewPriority, SourceKind, TextChunking,
};

/// Custom help template for the root command. clap renders every subcommand in a
/// single flat `Commands:` block ordered only by display order, so the grouped
/// section view lives in `{after-help}` (see [`GROUPED_COMMANDS`]) and the default
/// `{subcommands}` token is intentionally omitted here. Grouping is presentational
/// only: command names, flags, and behavior are unchanged.
const ROOT_HELP_TEMPLATE: &str = "\
{about-with-newline}
{usage-heading} {usage}{after-help}

Options:
{options}";

/// Grouped command listing rendered in the root help, with the everyday verbs
/// first inside each section. Kept in sync by hand with the `Command` variants.
const GROUPED_COMMANDS: &str = "\
Getting started:
  demo                  See the tamper-evidence trust story (no database, no Docker)
  init                  Wire your agent harness to use Nahuali (installs the skill)

Capture:
  remember              Record an observed episode
  claim                 Record an evidence-backed canonical claim
  fact                  Record a compatibility fact
  link                  Record a canonical typed link between entities
  relate                Record a compatibility relation between entities
  procedure             Record a reusable procedure
  preference            Record a reusable preference
  intention             Record a future task, goal, reminder, or commitment
  intention-status      Update an existing intention lifecycle state
  intention-update      Update intention metadata without changing lifecycle state
  intention-complete    Mark an intention completed
  intention-block       Mark an intention blocked
  intention-defer       Mark an intention deferred

Recall & inspect:
  recall                Recall matching memory with optional authority context
  status                Print operational memory status
  inspect               Inspect knowledge health before trusting memory
  self-inspect          Produce a non-mutating self-inspection consolidation report
  graph                 Traverse the projected memory graph around a seed
  project               Print a focused project or entity dashboard
  briefing              Print a compact non-mutating session briefing
  session-resume        Resume a session with briefing, authority, and review context
  timeline              Read recent memory timeline from the SurrealDB projection
  pending               Read pending intentions from the SurrealDB projection
  data                  Print the projected memory data

Reports:
  review                Print a prioritized non-mutating operator review queue
  review-resolve        Resolve an operator review item with an explicit audit note
  proactive             Print a non-mutating proactive operator report
  deadlines             Print non-mutating proactive deadline signals
  anomalies             Print non-mutating proactive anomaly alerts
  anomaly-acknowledge   Acknowledge a proactive anomaly with an explicit audit note
  goal-progress         Print non-mutating progress for goal intentions
  reconcile-intentions  Print a non-mutating intention reconciliation report
  reflect               Plan a non-mutating reflection cycle for operator approval
  sleep                 Run a non-mutating memory sleep pass
  consolidation-plan    Plan replay, review gates, and write-back eligibility
  hook                  Run a non-mutating memory hook for a host execution point

Maintenance:
  projection-status     Inspect the SurrealDB graph projection status
  projection-rebuild    Rebuild the SurrealDB graph projection from the record ledger
  projection-validate   Validate the SurrealDB graph projection without mutating it
  projection-entities   Read projected entities from SurrealDB graph tables
  projection-timeline   Read projected episode timeline from SurrealDB graph tables
  projection-pending    Read projected pending intentions from SurrealDB graph tables
  projection-health     Read projected health signals from SurrealDB graph tables
  semantic-rebuild      Rebuild the derived Qdrant semantic index from the projection
  semantic-sync         Synchronize the semantic index without dropping it (non-destructive)
  semantic-status       Inspect the derived Qdrant semantic index status
  validate              Validate the SurrealDB memory_record ledger without mutating it
  audit                 Audit what changed in the record ledger between two points
  trust-report          Print a composed memory trust report
  maintenance           Print a non-destructive maintenance report
  snapshot              Write or dry-run an optional projection snapshot
  snapshot-validate     Validate an optional snapshot against record replay

Migration & backup:
  backup                Write or dry-run a local record-ledger backup
  backup-validate       Validate a local record-ledger backup
  backup-drill          Validate a backup and dry-run restore into a target database
  restore               Restore a backup into an empty SurrealDB database
  export                Export a source-neutral memory interchange document
  import                Import a source-neutral memory interchange document
  convert-projection-export  Convert a projected memory export into an interchange document
  convert-legacy-export      Convert a historical export into an interchange document
  ingest                Ingest a provenance-aware source document
  ingest-text           Ingest a local text file as provenance-preserving source episodes
  ingest-dir            Ingest a directory of local text files after full batch preflight

Shell:
  completions           Generate shell completion scripts for your terminal
  help                  Print this message or the help of the given subcommand(s)

Run 'nahuali <COMMAND> --help' for details and examples on a single command.";

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
#[command(help_template = ROOT_HELP_TEMPLATE)]
#[command(after_help = GROUPED_COMMANDS)]
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
    #[command(about = "See the tamper-evidence trust story — no database, no Docker, in seconds.")]
    Demo {},
    #[command(
        about = "Wire your agent harness to use Nahuali (installs the skill, prints the MCP config)."
    )]
    Init {
        /// Show what would be written without changing anything.
        #[arg(long)]
        dry_run: bool,
        /// Overwrite an existing installed skill.
        #[arg(long)]
        force: bool,
    },
    #[command(about = "Print operational memory status.")]
    Status {
        #[arg(long)]
        json: bool,
    },
    #[command(
        about = "Record an observed episode.",
        after_help = "Examples:\n  nahuali remember \"Shipped the recall CLI grouping\" --tag cli\n  nahuali remember \"Met with the design team\" --mention alice --scope project:nahuali\n  nahuali remember \"Fixed flaky test\" -t ci -t fix"
    )]
    Remember {
        #[arg(help = "Episode text to record (joined into a single observed episode)")]
        content: Vec<String>,
        #[arg(long = "tag", short = 't', help = "Tag to attach (repeatable)")]
        tags: Vec<String>,
        #[arg(
            long = "mention",
            short = 'm',
            help = "Entity mentioned in this episode (repeatable)"
        )]
        mentions: Vec<String>,
        #[arg(
            long,
            value_name = "KIND:NAME",
            help = "Scope this episode to a KIND:NAME entity, e.g. project:nahuali"
        )]
        scope: Option<String>,
        #[arg(long, help = "Emit machine-readable JSON instead of text")]
        json: bool,
    },
    #[command(
        about = "Record an evidence-backed canonical claim.",
        after_help = "Examples:\n  nahuali claim nahuali license FSL-1.1-MIT --source-last\n  nahuali claim alice role \"lead engineer\" --confidence 0.95 --scope project:nahuali\n  nahuali claim db backend SurrealDB --source-episode <episode-id>"
    )]
    Claim {
        #[arg(help = "Subject entity the claim is about, e.g. nahuali")]
        subject: String,
        #[arg(help = "Predicate or relationship, e.g. license")]
        predicate: String,
        #[arg(help = "Object value(s) of the claim (joined), e.g. FSL-1.1-MIT")]
        object: Vec<String>,
        #[arg(
            long = "source-episode",
            help = "Attribute the claim to an existing episode by id"
        )]
        source_episode_id: Option<String>,
        #[arg(
            long = "source-last",
            conflicts_with = "source_episode_id",
            help = "Attribute the claim to the most recent episode"
        )]
        source_last: bool,
        #[arg(
            long,
            short = 'c',
            default_value_t = 0.8,
            value_parser = parse_confidence,
            help = "Confidence in the claim, between 0.0 and 1.0"
        )]
        confidence: f32,
        #[arg(
            long,
            value_name = "KIND:NAME",
            help = "Scope this claim to a KIND:NAME entity, e.g. project:nahuali"
        )]
        scope: Option<String>,
        #[arg(long, help = "Emit machine-readable JSON instead of text")]
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
    #[command(
        about = "Record a future task, goal, reminder, or commitment.",
        after_help = "Examples:\n  nahuali intention \"Ship shell completions\" --priority high\n  nahuali intention \"Reach 80% test coverage\" --kind goal --scope project:nahuali\n  nahuali intention \"Renew the TLS cert\" --kind reminder --priority critical"
    )]
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
    #[command(
        about = "Print a compact non-mutating session briefing.",
        after_help = "Examples:\n  nahuali briefing\n  nahuali briefing --episode-limit 10 --review-limit 10\n  nahuali briefing --json"
    )]
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
    #[command(
        about = "Recall matching memory with optional authority context.",
        after_help = "Examples:\n  nahuali recall \"licensing decision\"\n  nahuali recall nahuali --authority --require-evidence\n  nahuali recall roadmap --kind intention --kind claim --limit 25 --scope project:nahuali"
    )]
    Recall {
        #[arg(help = "Search terms (joined) to match against memory")]
        query: Vec<String>,
        #[arg(
            long,
            short = 'l',
            default_value_t = 10,
            help = "Maximum number of results to return"
        )]
        limit: usize,
        #[arg(
            long,
            help = "Rank results by governance authority instead of plain relevance"
        )]
        authority: bool,
        #[arg(
            long,
            conflicts_with = "authority",
            help = "Use the semantic index for fuzzy matching instead of lexical search"
        )]
        semantic: bool,
        #[arg(
            long,
            value_name = "KIND:NAME",
            help = "Restrict results to a KIND:NAME scope entity, e.g. project:nahuali"
        )]
        scope: Option<String>,
        #[arg(
            long = "kind",
            value_enum,
            help = "Restrict results to one or more memory kinds (repeatable)"
        )]
        kinds: Vec<CliRecallKind>,
        #[arg(
            long = "require-evidence",
            help = "Only return results backed by recorded evidence"
        )]
        require_evidence: bool,
        #[arg(
            long = "as-of-ms",
            value_name = "MS",
            help = "Point-in-time recall: only memory created at or before this epoch-millisecond timestamp"
        )]
        as_of_ms: Option<u64>,
        #[arg(
            long = "max-age-days",
            value_name = "DAYS",
            help = "Exclude memory older than this many days at query time (audit-grade staleness filter)"
        )]
        max_age_days: Option<u64>,
        #[arg(long, help = "Emit machine-readable JSON instead of text")]
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
    #[command(
        about = "Synchronize the Qdrant semantic index without dropping it (non-destructive upsert)."
    )]
    SemanticSync {
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
    #[command(
        about = "Inspect knowledge health before trusting memory.",
        after_help = "Examples:\n  nahuali inspect\n  nahuali inspect --json"
    )]
    Inspect {
        #[arg(long)]
        json: bool,
    },
    #[command(
        about = "Produce a non-mutating self-inspection consolidation report.",
        after_help = "Examples:\n  nahuali self-inspect\n  nahuali self-inspect --json"
    )]
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
    #[command(
        about = "Print a prioritized non-mutating operator review queue.",
        after_help = "Examples:\n  nahuali review\n  nahuali review --min-priority high --limit 10\n  nahuali review --action resolve-contradiction"
    )]
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
    #[command(
        about = "Audit what changed in the record ledger between two points without mutating it."
    )]
    Audit {
        /// Exclusive lower sequence bound; audit changes after this sequence.
        #[arg(long, value_name = "SEQUENCE")]
        from: Option<u64>,
        /// Inclusive upper sequence bound; audit changes through this sequence.
        #[arg(long, value_name = "SEQUENCE")]
        to: Option<u64>,
        /// Inclusive lower timestamp bound in milliseconds since the Unix epoch.
        #[arg(long, value_name = "MS")]
        since: Option<u64>,
        /// Inclusive upper timestamp bound in milliseconds since the Unix epoch.
        #[arg(long, value_name = "MS")]
        until: Option<u64>,
        /// Anchor the lower bound on a signed attestation receipt and diff
        /// changes since that verified checkpoint. Overrides `--from`.
        #[cfg(feature = "attestation")]
        #[arg(long = "from-attestation", value_name = "PATH")]
        from_attestation: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Print a composed memory trust report without mutating memory.")]
    TrustReport {
        /// Verify a signed attestation receipt and fold it into the report as the
        /// evidence that the recorded history was not altered.
        #[cfg(feature = "attestation")]
        #[arg(long = "attestation", value_name = "PATH")]
        attestation: Option<PathBuf>,
        /// Also write a self-contained HTML dossier of the report to this path.
        #[arg(long, value_name = "PATH")]
        html: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    #[cfg(feature = "attestation")]
    #[command(about = "Sign the current tamper-evident chain tip with an Ed25519 key.")]
    AttestSign {
        #[arg(long = "key-file", value_name = "PATH")]
        key_file: PathBuf,
        #[arg(long, short = 'o', value_name = "PATH")]
        output: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    #[cfg(feature = "attestation")]
    #[command(about = "Verify a ledger tip attestation against the current ledger.")]
    AttestVerify {
        #[arg(value_name = "PATH")]
        attestation: PathBuf,
        #[arg(
            long,
            value_name = "PATH",
            help = "Trusted-key ring (JSON) to authorize the receipt against; rejects a revoked or unknown signing key even when its signature verifies"
        )]
        keyring: Option<PathBuf>,
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
    #[command(
        about = "Generate shell completion scripts for your terminal.",
        after_help = "Examples:\n  nahuali completions zsh > ~/.zsh/completions/_nahuali\n  nahuali completions bash > /etc/bash_completion.d/nahuali\n  nahuali completions fish > ~/.config/fish/completions/nahuali.fish"
    )]
    Completions {
        #[arg(value_enum, help = "Shell to generate a completion script for")]
        shell: CliShell,
    },
}

/// Shells supported by `nahuali completions`. Mirrors `clap_complete::Shell`.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum CliShell {
    Bash,
    Zsh,
    Fish,
    #[value(name = "powershell")]
    PowerShell,
    Elvish,
}

impl From<CliShell> for clap_complete::Shell {
    fn from(value: CliShell) -> Self {
        match value {
            CliShell::Bash => Self::Bash,
            CliShell::Zsh => Self::Zsh,
            CliShell::Fish => Self::Fish,
            CliShell::PowerShell => Self::PowerShell,
            CliShell::Elvish => Self::Elvish,
        }
    }
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

    #[test]
    fn clap_command_configuration_is_valid() {
        // Catches malformed help templates, conflicting args, and bad value parsers.
        Cli::command().debug_assert();
    }

    #[test]
    fn top_level_help_groups_commands_into_sections() {
        let mut command = Cli::command();
        let help = command.render_long_help().to_string();

        assert!(help.contains("Capture:"));
        assert!(help.contains("Recall & inspect:"));
        assert!(help.contains("Reports:"));
        assert!(help.contains("Maintenance:"));
        assert!(help.contains("Migration & backup:"));
        assert!(help.contains("Shell:"));
        // The everyday verbs lead their sections.
        let capture = help.find("Capture:").expect("capture section");
        let remember = help.find("  remember").expect("remember listed");
        assert!(
            remember > capture,
            "remember should follow the Capture heading"
        );
    }

    #[test]
    fn recall_help_documents_arguments_and_examples() {
        let mut command = Cli::command();
        let recall = command
            .find_subcommand_mut("recall")
            .expect("recall subcommand exists");
        let help = recall.render_long_help().to_string();

        // Previously-blank args now carry descriptions.
        assert!(help.contains("Search terms"));
        assert!(help.contains("Rank results by governance authority"));
        assert!(help.contains("Only return results backed by recorded evidence"));
        // Runnable examples are present.
        assert!(help.contains("Examples:"));
        assert!(help.contains("nahuali recall"));
    }

    #[test]
    fn claim_help_documents_positional_arguments() {
        let mut command = Cli::command();
        let claim = command
            .find_subcommand_mut("claim")
            .expect("claim subcommand exists");
        let help = claim.render_long_help().to_string();

        assert!(help.contains("Subject entity"));
        assert!(help.contains("Predicate"));
        assert!(help.contains("Object value"));
        assert!(help.contains("Examples:"));
    }

    #[test]
    fn completions_subcommand_exposes_supported_shells() {
        let mut command = Cli::command();
        let completions = command
            .find_subcommand_mut("completions")
            .expect("completions subcommand exists");
        let help = completions.render_long_help().to_string();

        for shell in ["bash", "zsh", "fish", "powershell", "elvish"] {
            assert!(help.contains(shell), "shell `{shell}` should be a value");
        }
    }
}
