use sha2::{Digest, Sha256};

/// Current SurrealDB graph projection schema version.
pub const GRAPH_PROJECTION_VERSION: u32 = 2;

const GRAPH_PROJECTION_CHECKPOINT_ID: &str = "graph_v2";
const GRAPH_PROJECTION_REBUILD_LOCK_ID: &str = "graph_v2";
const GRAPH_PROJECTION_FENCING_SEQUENCE: &str = "projection_rebuild_fencing";
const GRAPH_PROJECTION_MUTATION_GUARD_SEQUENCE: &str = "projection_rebuild_mutation_guard";
const GRAPH_PROJECTION_REBUILD_LEASE_MS: u64 = 120_000;
const GRAPH_PROJECTION_REBUILD_WAIT_MS: u64 = 30_000;
const GRAPH_PROJECTION_REBUILD_POLL_MS: u64 = 50;
const GRAPH_PROJECTION_MANIFEST_ALGORITHM: &str = "sha256-canonical-json-v1";
const GRAPH_PROJECTION_MUTATION_BATCH_SIZE: usize = 128;

#[cfg(test)]
static INJECTED_GRAPH_PROJECTION_FAILURES: std::sync::OnceLock<
    std::sync::Mutex<BTreeSet<PathBuf>>,
> = std::sync::OnceLock::new();

const PROJECTED_NODE_TABLES: &[&str] = &[
    "projection_checkpoint",
    "projection_error",
    "memory_scope",
    "source_record",
    "episode",
    "entity",
    "claim",
    "procedure",
    "intention",
    "health_signal",
    "review_item",
    "review_decision",
    "inferred_claim",
    "contradiction",
    "anomaly_alert",
];

const PROJECTED_RELATION_TABLES: &[&str] = &[
    "mentions",
    "supports",
    "relates_to",
    "intention_depends_on",
];

// Checkpoint, error, and lease rows are control-plane state rather than the
// ledger-derived graph. Checkpoint/error counts and checkpoint versions remain
// validated; the lease is governed by owner+fence checks. All three are
// deliberately excluded from the self-referential content manifest.
const MANIFEST_NODE_TABLES: &[&str] = &[
    "memory_scope",
    "source_record",
    "episode",
    "entity",
    "claim",
    "procedure",
    "intention",
    "health_signal",
    "review_item",
    "review_decision",
    "inferred_claim",
    "contradiction",
    "anomaly_alert",
];

/// Table counts and checkpoint state for the SurrealDB graph projection.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct GraphProjectionStatus {
    /// Projection schema version expected by this runtime.
    pub projection_version: u32,
    /// In-memory projection model version expected by this runtime.
    pub memory_data_version: u32,
    /// Projection schema version persisted in the active checkpoint.
    pub checkpoint_projection_version: Option<u32>,
    /// Projection model version persisted in the active checkpoint.
    pub checkpoint_memory_data_version: Option<u32>,
    /// Manifest algorithm persisted in the active checkpoint.
    pub checkpoint_manifest_algorithm: Option<String>,
    /// Expected content digest persisted in the active checkpoint.
    pub checkpoint_manifest_digest: Option<String>,
    /// Per-table expected content digests persisted in the checkpoint.
    pub checkpoint_manifest_table_digests: BTreeMap<String, String>,
    /// Digest recomputed from the currently stored projected rows.
    pub actual_manifest_digest: String,
    /// Per-table digests recomputed from the currently stored projected rows.
    pub actual_manifest_table_digests: BTreeMap<String, String>,
    /// Number of ledger events included in the in-memory projection.
    pub ledger_event_count: usize,
    /// Latest ledger sequence in the in-memory projection.
    pub latest_sequence: Option<u64>,
    /// Latest ledger event identifier in the in-memory projection.
    pub latest_event_id: Option<String>,
    /// Latest sequence recorded in the SurrealDB projection checkpoint.
    pub checkpoint_sequence: Option<u64>,
    /// Latest event recorded in the SurrealDB projection checkpoint.
    pub checkpoint_event_id: Option<String>,
    /// Count of rows in every graph projection table.
    pub table_counts: BTreeMap<String, usize>,
    /// Whether table counts and checkpoint match the current ledger projection.
    pub in_sync: bool,
}

/// Report returned after rebuilding the SurrealDB graph projection.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct GraphProjectionRebuildReport {
    /// Projection status after rebuild.
    pub status: GraphProjectionStatus,
    /// Non-relation rows written during rebuild.
    pub node_rows_written: usize,
    /// Relation rows written during rebuild.
    pub relation_rows_written: usize,
}

/// Non-mutating graph projection validation result.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct GraphProjectionValidation {
    /// Projection status observed during validation.
    pub status: GraphProjectionStatus,
    /// Whether the graph projection matches the current ledger projection.
    pub valid: bool,
    /// Validation issues. Empty when `valid` is true.
    pub issues: Vec<String>,
}

/// Entity row read directly from the SurrealDB graph projection.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct GraphProjectionEntity {
    /// Projected memory identifier.
    pub memory_id: String,
    /// Entity display name.
    pub name: String,
    /// Number of projected mentions.
    pub mention_count: usize,
    /// Scope key, when present.
    pub scope_key: Option<String>,
    /// Event identifiers that mention this entity.
    pub source_event_ids: Vec<String>,
}

/// Episode row read directly from the SurrealDB graph projection.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct GraphProjectionEpisode {
    /// Projected memory identifier.
    pub memory_id: String,
    /// Source event identifier.
    pub event_id: String,
    /// Episode content.
    pub content: String,
    /// User-provided tags.
    pub tags: Vec<String>,
    /// Mentioned entity names.
    pub mentions: Vec<String>,
    /// Source record identifier, when available.
    pub source_id: Option<String>,
    /// Scope key, when present.
    pub scope_key: Option<String>,
    /// Creation timestamp in milliseconds since the Unix epoch.
    pub created_at_ms: u64,
}

/// Pending intention row read directly from the SurrealDB graph projection.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct GraphProjectionPendingIntention {
    /// Projected memory identifier.
    pub memory_id: String,
    /// Source event identifier.
    pub event_id: String,
    /// Intention kind.
    pub intention_kind: String,
    /// Current lifecycle status.
    pub status: String,
    /// Priority.
    pub priority: String,
    /// Intention description.
    pub description: String,
    /// Source episode identifier, when available.
    pub source_episode_id: Option<String>,
    /// Deadline or commitment timestamp in milliseconds since the Unix epoch.
    pub deadline_at_ms: Option<u64>,
    /// Intention identifiers that must complete before this item can proceed.
    pub depends_on: Vec<String>,
    /// Parent goal intention identifier, when this item contributes to a goal.
    pub goal_id: Option<String>,
    /// Operator-supplied progress estimate from 0 to 100.
    pub progress_percent: Option<u8>,
    /// Scope key, when present.
    pub scope_key: Option<String>,
    /// Creation timestamp in milliseconds since the Unix epoch.
    pub created_at_ms: u64,
    /// Update timestamp in milliseconds since the Unix epoch.
    pub updated_at_ms: u64,
}

/// Health signal row read directly from the SurrealDB graph projection.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct GraphProjectionHealthSignal {
    /// Projected signal identifier.
    pub memory_id: String,
    /// Signal kind.
    pub signal_kind: String,
    /// Signal severity.
    pub severity: String,
    /// Human-readable signal message.
    pub message: String,
    /// Evidence identifiers attached to the signal.
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Debug)]
struct EventMeta {
    sequence: u64,
}

#[derive(Clone, Debug, serde::Deserialize)]
struct ProjectionCheckpointRow {
    projection_version: Option<u32>,
    memory_data_version: Option<u32>,
    latest_sequence: Option<u64>,
    latest_event_id: Option<String>,
    manifest_algorithm: Option<String>,
    manifest_digest: Option<String>,
    #[serde(default)]
    manifest_table_digests: BTreeMap<String, String>,
}

#[derive(Clone, Debug, serde::Deserialize)]
struct ProjectionLeaseRow {
    owner_token: Option<String>,
    #[serde(default)]
    fencing_token: u64,
    #[serde(default)]
    expires_at_ms: u64,
}

#[derive(Debug)]
struct GraphProjectionLease {
    owner_token: String,
    fencing_token: u64,
    expires_at_ms: AtomicU64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ProjectionManifest {
    digest: String,
    table_digests: BTreeMap<String, String>,
}

struct ProjectionManifestBuilder {
    row_digests: BTreeMap<String, Vec<[u8; 32]>>,
}

#[derive(Default)]
struct ExistingProjection {
    rows: BTreeMap<String, BTreeMap<String, serde_json::Value>>,
}

enum ExistingRow {
    Missing,
    Unchanged,
    Changed,
}

struct ProjectionMutationBatch {
    statements: Vec<String>,
    relation_rows: Vec<String>,
    relation_table: Option<String>,
    bindings: Vec<(String, serde_json::Value)>,
    manifest_entries: Vec<(String, serde_json::Value)>,
    next_binding_index: usize,
    existing: ExistingProjection,
    unchanged_node_rows: usize,
    unchanged_relation_rows: usize,
}

struct ProjectedRelationInput<'a> {
    in_table: &'a str,
    in_id: &'a str,
    relation_table: &'a str,
    out_table: &'a str,
    out_id: &'a str,
    content: serde_json::Value,
}

impl ProjectedRelationInput<'_> {
    fn memory_id(&self) -> &str {
        self.content
            .get("memory_id")
            .and_then(serde_json::Value::as_str)
            .expect("projected relations always carry a logical memory_id")
    }
}

impl MemoryEngine {
    /// Return SurrealDB graph projection status for the current ledger projection.
    pub fn projection_status(&self) -> Result<GraphProjectionStatus> {
        let path = self.path.clone();
        let data = self.data.clone();
        let events = self.events.clone();
        block_on_database(async move { graph_projection_status(&path, &data, &events).await })
    }

    /// Rebuild the SurrealDB graph projection from the authoritative ledger.
    pub fn projection_rebuild(&mut self) -> Result<GraphProjectionRebuildReport> {
        let path = self.path.clone();
        block_on_database(async move { rebuild_graph_projection(&path).await })
    }

    /// Validate the SurrealDB graph projection without mutating it.
    pub fn projection_validate(&self) -> Result<GraphProjectionValidation> {
        let status = self.projection_status()?;
        let mut issues = Vec::new();
        if status.checkpoint_projection_version != Some(GRAPH_PROJECTION_VERSION) {
            issues.push(format!(
                "checkpoint projection version {:?} does not match runtime version {}",
                status.checkpoint_projection_version, GRAPH_PROJECTION_VERSION
            ));
        }
        if status.checkpoint_memory_data_version != Some(MEMORY_DATA_VERSION) {
            issues.push(format!(
                "checkpoint memory data version {:?} does not match runtime version {}",
                status.checkpoint_memory_data_version, MEMORY_DATA_VERSION
            ));
        }
        if status.checkpoint_manifest_algorithm.as_deref()
            != Some(GRAPH_PROJECTION_MANIFEST_ALGORITHM)
        {
            issues.push(format!(
                "checkpoint manifest algorithm {:?} does not match runtime algorithm {}",
                status.checkpoint_manifest_algorithm, GRAPH_PROJECTION_MANIFEST_ALGORITHM
            ));
        }
        if status.checkpoint_manifest_digest.as_deref()
            != Some(status.actual_manifest_digest.as_str())
        {
            issues.push(format!(
                "checkpoint manifest digest {:?} does not match projected content digest {}",
                status.checkpoint_manifest_digest, status.actual_manifest_digest
            ));
        }
        if status.checkpoint_manifest_table_digests != status.actual_manifest_table_digests {
            let mismatched = manifest_mismatched_tables(
                &status.checkpoint_manifest_table_digests,
                &status.actual_manifest_table_digests,
            );
            issues.push(format!(
                "checkpoint manifest table digests do not match projected content for: {}",
                mismatched.join(", ")
            ));
        }
        if status.checkpoint_sequence != status.latest_sequence {
            issues.push(format!(
                "checkpoint sequence {:?} does not match ledger sequence {:?}",
                status.checkpoint_sequence, status.latest_sequence
            ));
        }
        if status.checkpoint_event_id != status.latest_event_id {
            issues.push(format!(
                "checkpoint event {:?} does not match ledger event {:?}",
                status.checkpoint_event_id, status.latest_event_id
            ));
        }
        for (table, expected) in expected_graph_projection_counts(&self.data) {
            let actual = status.table_counts.get(&table).copied().unwrap_or(0);
            if actual != expected {
                issues.push(format!(
                    "table {table} has {actual} rows but expected {expected}"
                ));
            }
        }
        Ok(GraphProjectionValidation {
            valid: issues.is_empty(),
            status,
            issues,
        })
    }

    /// Read projected entities directly from SurrealDB graph projection tables.
    pub fn projection_entities(
        &self,
        query: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GraphProjectionEntity>> {
        let read_fence = self.graph_projection_read_fence()?;
        self.ensure_graph_projection_readable()?;
        let path = self.path.clone();
        let query = query.map(str::to_string);
        let entities = block_on_database(async move {
            let db = open_database(&path).await?;
            let mut entities: Vec<GraphProjectionEntity> =
                select_projected_rows(&path, &db, "SELECT memory_id, name, mention_count, scope_key, source_event_ids, last_seen_at_ms FROM entity ORDER BY last_seen_at_ms DESC").await?;
            if let Some(query) = query.and_then(|query| clean_projection_query(&query)) {
                let query = query.to_ascii_lowercase();
                entities.retain(|entity| entity.name.to_ascii_lowercase().contains(&query));
            }
            entities.truncate(limit.max(1));
            Ok(entities)
        })?;
        self.ensure_graph_projection_read_fence(read_fence)?;
        Ok(entities)
    }

    /// Read recent projected episodes directly from SurrealDB graph projection tables.
    pub fn projection_timeline(&self, limit: usize) -> Result<Vec<GraphProjectionEpisode>> {
        let read_fence = self.graph_projection_read_fence()?;
        self.ensure_graph_projection_readable()?;
        let path = self.path.clone();
        let episodes = block_on_database(async move {
            let db = open_database(&path).await?;
            let mut episodes: Vec<GraphProjectionEpisode> =
                select_projected_rows(&path, &db, "SELECT memory_id, event_id, content, tags, mentions, source_id, scope_key, created_at_ms FROM episode ORDER BY created_at_ms DESC").await?;
            episodes.truncate(limit.max(1));
            Ok(episodes)
        })?;
        self.ensure_graph_projection_read_fence(read_fence)?;
        Ok(episodes)
    }

    /// Read active pending intentions directly from SurrealDB graph projection tables.
    pub fn projection_pending(
        &self,
        limit: usize,
    ) -> Result<Vec<GraphProjectionPendingIntention>> {
        let read_fence = self.graph_projection_read_fence()?;
        self.ensure_graph_projection_readable()?;
        let path = self.path.clone();
        let intentions = block_on_database(async move {
            let db = open_database(&path).await?;
            let mut intentions: Vec<GraphProjectionPendingIntention> =
                select_projected_rows(&path, &db, "SELECT memory_id, event_id, intention_kind, status, priority, description, source_episode_id, deadline_at_ms, depends_on, goal_id, progress_percent, scope_key, created_at_ms, updated_at_ms FROM intention WHERE status = 'active' ORDER BY updated_at_ms DESC").await?;
            intentions.truncate(limit.max(1));
            Ok(intentions)
        })?;
        self.ensure_graph_projection_read_fence(read_fence)?;
        Ok(intentions)
    }

    /// Read projected health signals directly from SurrealDB graph projection tables.
    pub fn projection_health_signals(
        &self,
        limit: usize,
    ) -> Result<Vec<GraphProjectionHealthSignal>> {
        let read_fence = self.graph_projection_read_fence()?;
        self.ensure_graph_projection_readable()?;
        let path = self.path.clone();
        let signals = block_on_database(async move {
            let db = open_database(&path).await?;
            let mut signals: Vec<GraphProjectionHealthSignal> =
                select_projected_rows(&path, &db, "SELECT memory_id, signal_kind, severity, message, evidence_ids FROM health_signal").await?;
            signals.truncate(limit.max(1));
            Ok(signals)
        })?;
        self.ensure_graph_projection_read_fence(read_fence)?;
        Ok(signals)
    }

    fn ensure_graph_projection_readable(&self) -> Result<()> {
        let validation = self.projection_validate()?;
        if validation.valid {
            Ok(())
        } else {
            Err(NahualiError::GraphProjectionInvalid {
                issues: validation.issues.join("; "),
            })
        }
    }

    fn graph_projection_read_fence(&self) -> Result<u64> {
        let path = self.path.clone();
        block_on_database(async move {
            let db = open_database(&path).await?;
            read_idle_graph_projection_fence(&path, &db).await
        })
    }

    fn ensure_graph_projection_read_fence(&self, expected_fence: u64) -> Result<()> {
        let observed_fence = self.graph_projection_read_fence()?;
        if observed_fence == expected_fence {
            Ok(())
        } else {
            Err(NahualiError::GraphProjectionInvalid {
                issues: format!(
                    "projection rebuild fence changed during read from {expected_fence} to {observed_fence}"
                ),
            })
        }
    }
}

async fn rebuild_graph_projection(path: &Path) -> Result<GraphProjectionRebuildReport> {
    #[cfg(test)]
    if consume_injected_graph_projection_failure(path) {
        return Err(NahualiError::GraphProjectionRebuildBusy { timeout_ms: 0 });
    }
    let db = open_database(path).await?;
    let lock_token = make_id("projection_rebuild");
    let lease = acquire_graph_projection_rebuild_lock(path, &db, &lock_token).await?;
    let rebuild = async {
        // Coalesce only while holding our own fenced lease. Observing an
        // in-sync checkpoint while another owner can still clear it is not a
        // safe completion condition.
        if let Some(report) = completed_concurrent_rebuild(path).await? {
            return Ok(report);
        }
        let events = read_records(path).await?;
        let data = projection::project(&events);
        rebuild_graph_projection_locked(path, &data, &events, db.clone(), &lease).await
    }
    .await;
    let release = release_graph_projection_rebuild_lock(path, &db, &lease).await;
    match (rebuild, release) {
        (Ok(report), Ok(())) => Ok(report),
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
    }
}

async fn completed_concurrent_rebuild(
    path: &Path,
) -> Result<Option<GraphProjectionRebuildReport>> {
    let events_before = read_records(path).await?;
    let data = projection::project(&events_before);
    let status = graph_projection_status(path, &data, &events_before).await?;
    if !status.in_sync {
        return Ok(None);
    }

    // A writer may append after the first read. Only coalesce when the ledger
    // tip stays stable across the projection check.
    let events_after = read_records(path).await?;
    let before_tip = events_before
        .last()
        .map(|event| (event.sequence, event.id.as_str()));
    let after_tip = events_after
        .last()
        .map(|event| (event.sequence, event.id.as_str()));
    if events_before.len() != events_after.len() || before_tip != after_tip {
        return Ok(None);
    }

    Ok(Some(GraphProjectionRebuildReport {
        status,
        node_rows_written: 0,
        relation_rows_written: 0,
    }))
}

#[cfg(test)]
fn inject_graph_projection_failure_once(path: &Path) {
    INJECTED_GRAPH_PROJECTION_FAILURES
        .get_or_init(|| std::sync::Mutex::new(BTreeSet::new()))
        .lock()
        .expect("projection failure registry lock")
        .insert(path.to_path_buf());
}

#[cfg(test)]
fn consume_injected_graph_projection_failure(path: &Path) -> bool {
    INJECTED_GRAPH_PROJECTION_FAILURES
        .get_or_init(|| std::sync::Mutex::new(BTreeSet::new()))
        .lock()
        .expect("projection failure registry lock")
        .remove(path)
}

async fn rebuild_graph_projection_locked(
    path: &Path,
    data: &MemoryData,
    events: &[EventEnvelope],
    db: DatabaseSession,
    lease: &GraphProjectionLease,
) -> Result<GraphProjectionRebuildReport> {
    verify_graph_projection_lease(path, &db, lease).await?;
    let current_status = graph_projection_status_with_db(path, &db, data, events).await?;
    let existing = if projection_baseline_is_trustworthy(&current_status) {
        match read_existing_projection(path, &db).await? {
            Some(existing) => existing,
            None => {
                clear_graph_projection(path, &db, lease).await?;
                ExistingProjection::default()
            }
        }
    } else {
        clear_graph_projection(path, &db, lease).await?;
        ExistingProjection::default()
    };

    let event_meta = event_meta(events);
    let entity_ids = entity_id_lookup(data);
    let intention_ids = intention_id_lookup(data);
    let mut manifest = ProjectionManifestBuilder::new();
    let mut mutation_batch = ProjectionMutationBatch::new(existing);
    let mut node_rows_written: usize = 0;
    let mut relation_rows_written: usize = 0;

    for scope in projected_scopes(data) {
        create_projected_record(
            path,
            &db,
            lease,
            &mut mutation_batch,
            &mut manifest,
            ("memory_scope", &scope.key),
            serde_json::json!({
                "memory_id": scope.key,
                "scope_key": scope.key,
                "scope_kind": scope.kind.as_key(),
                "scope_name": scope.name,
                "projection_version": GRAPH_PROJECTION_VERSION,
            }),
        )
        .await?;
        node_rows_written += 1;
    }

    for source in &data.sources {
        let meta = event_meta.get(source.event_id.as_str());
        create_projected_record(
            path,
            &db,
            lease,
            &mut mutation_batch,
            &mut manifest,
            ("source_record", &source.id),
            with_scope(
                serde_json::json!({
                    "memory_id": source.id,
                    "event_id": source.event_id,
                    "source_sequence": meta.map(|meta| meta.sequence),
                    "source_kind": source.kind,
                    "title": source.title,
                    "uri": source.uri,
                    "content_checksum": source.content_checksum,
                    "byte_len": source.byte_len,
                    "metadata": source.metadata,
                    "created_at_ms": source.created_at_ms,
                    "projection_version": GRAPH_PROJECTION_VERSION,
                }),
                source.scope.as_ref(),
            ),
        )
        .await?;
        node_rows_written += 1;
    }

    for episode in &data.episodes {
        let meta = event_meta.get(episode.event_id.as_str());
        create_projected_record(
            path,
            &db,
            lease,
            &mut mutation_batch,
            &mut manifest,
            ("episode", &episode.id),
            with_scope(
                serde_json::json!({
                    "memory_id": episode.id,
                    "event_id": episode.event_id,
                    "source_sequence": meta.map(|meta| meta.sequence),
                    "content": episode.content,
                    "tags": episode.tags,
                    "mentions": episode.mentions,
                    "source_id": episode.source_id,
                    "source_position": episode.source_position,
                    "source_role": episode.source_role,
                    "created_at_ms": episode.created_at_ms,
                    "projection_version": GRAPH_PROJECTION_VERSION,
                }),
                episode.scope.as_ref(),
            ),
        )
        .await?;
        node_rows_written += 1;
    }

    for entity in &data.entities {
        let first_sequence = entity
            .source_event_ids
            .iter()
            .filter_map(|event_id| event_meta.get(event_id.as_str()).map(|meta| meta.sequence))
            .min();
        create_projected_record(
            path,
            &db,
            lease,
            &mut mutation_batch,
            &mut manifest,
            ("entity", &entity.id),
            with_scope(
                serde_json::json!({
                    "memory_id": entity.id,
                    "name": entity.name,
                    "mention_count": entity.mention_count,
                    "first_seen_at_ms": entity.first_seen_at_ms,
                    "last_seen_at_ms": entity.last_seen_at_ms,
                    "source_event_ids": entity.source_event_ids,
                    "source_sequence": first_sequence,
                    "projection_version": GRAPH_PROJECTION_VERSION,
                }),
                entity.scope.as_ref(),
            ),
        )
        .await?;
        node_rows_written += 1;
    }

    for claim in &data.claims {
        let meta = event_meta.get(claim.event_id.as_str());
        create_projected_record(
            path,
            &db,
            lease,
            &mut mutation_batch,
            &mut manifest,
            ("claim", &claim.id),
            with_scope(
                serde_json::json!({
                    "memory_id": claim.id,
                    "event_id": claim.event_id,
                    "source_sequence": meta.map(|meta| meta.sequence),
                    "subject": claim.subject,
                    "predicate": claim.predicate,
                    "object": claim.object,
                    "source_episode_id": claim.source_episode_id,
                    "confidence": claim.confidence,
                    "created_at_ms": claim.created_at_ms,
                    "projection_version": GRAPH_PROJECTION_VERSION,
                }),
                claim.scope.as_ref(),
            ),
        )
        .await?;
        node_rows_written += 1;
    }

    for procedure in &data.procedures {
        let meta = event_meta.get(procedure.event_id.as_str());
        create_projected_record(
            path,
            &db,
            lease,
            &mut mutation_batch,
            &mut manifest,
            ("procedure", &procedure.id),
            with_scope(
                serde_json::json!({
                    "memory_id": procedure.id,
                    "event_id": procedure.event_id,
                    "source_sequence": meta.map(|meta| meta.sequence),
                    "procedure_kind": procedure.kind,
                    "name": procedure.name,
                    "body": procedure.body,
                    "source_episode_id": procedure.source_episode_id,
                    "confidence": procedure.confidence,
                    "created_at_ms": procedure.created_at_ms,
                    "projection_version": GRAPH_PROJECTION_VERSION,
                }),
                procedure.scope.as_ref(),
            ),
        )
        .await?;
        node_rows_written += 1;
    }

    for intention in &data.intentions {
        let meta = event_meta.get(intention.event_id.as_str());
        create_projected_record(
            path,
            &db,
            lease,
            &mut mutation_batch,
            &mut manifest,
            ("intention", &intention.id),
            with_scope(
                serde_json::json!({
                    "memory_id": intention.id,
                    "event_id": intention.event_id,
                    "updated_event_id": intention.updated_event_id,
                    "source_sequence": meta.map(|meta| meta.sequence),
                    "intention_kind": intention.kind,
                    "status": intention.status,
                    "priority": intention.priority,
                    "description": intention.description,
                    "source_episode_id": intention.source_episode_id,
                    "status_reason": intention.status_reason,
                    "deadline_at_ms": intention.deadline_at_ms,
                    "depends_on": intention.depends_on,
                    "goal_id": intention.goal_id,
                    "progress_percent": intention.progress_percent,
                    "created_at_ms": intention.created_at_ms,
                    "updated_at_ms": intention.updated_at_ms,
                    "projection_version": GRAPH_PROJECTION_VERSION,
                }),
                intention.scope.as_ref(),
            ),
        )
        .await?;
        node_rows_written += 1;
    }

    for signal in KnowledgeHealth::inspect(data).signals {
        let memory_id = projected_hash_id(
            "health_signal",
            &[
                &format!("{:?}", signal.kind),
                &signal.message,
                &signal.evidence_ids.join("|"),
            ],
        );
        create_projected_record(
            path,
            &db,
            lease,
            &mut mutation_batch,
            &mut manifest,
            ("health_signal", &memory_id),
            serde_json::json!({
                "memory_id": memory_id,
                "signal_kind": signal.kind,
                "dimensions": signal.dimensions,
                "severity": signal.severity,
                "message": signal.message,
                "evidence_ids": signal.evidence_ids,
                "projection_version": GRAPH_PROJECTION_VERSION,
            }),
        )
        .await?;
        node_rows_written += 1;
    }

    for item in operator_review::operator_review(data, OperatorReviewOptions::default()).items {
        create_projected_record(
            path,
            &db,
            lease,
            &mut mutation_batch,
            &mut manifest,
            ("review_item", &item.id),
            serde_json::json!({
                "memory_id": item.id,
                "finding_id": item.finding_id,
                "finding_kind": item.finding_kind,
                "priority": item.priority,
                "score": item.score,
                "action": item.action,
                "status": item.status,
                "title": item.title,
                "detail": item.detail,
                "source_severity": item.source_severity,
                "dimensions": item.dimensions,
                "evidence_ids": item.evidence_ids,
                "operator_guidance": item.operator_guidance,
                "projection_version": GRAPH_PROJECTION_VERSION,
            }),
        )
        .await?;
        node_rows_written += 1;
    }

    for review in &data.review_decisions {
        let meta = event_meta.get(review.event_id.as_str());
        create_projected_record(
            path,
            &db,
            lease,
            &mut mutation_batch,
            &mut manifest,
            ("review_decision", &review.id),
            with_scope(
                serde_json::json!({
                    "memory_id": review.id,
                    "event_id": review.event_id,
                    "source_sequence": meta.map(|meta| meta.sequence),
                    "review_id": review.review_id,
                    "finding_id": review.finding_id,
                    "action": review.action,
                    "outcome": review.outcome,
                    "note": review.note,
                    "evidence_ids": review.evidence_ids,
                    "created_at_ms": review.created_at_ms,
                    "projection_version": GRAPH_PROJECTION_VERSION,
                }),
                review.scope.as_ref(),
            ),
        )
        .await?;
        node_rows_written += 1;
    }

    for alert in proactive::anomaly_report(data, ProactiveOptions::default()).alerts {
        let alert_id = alert.id.clone();
        create_projected_record(
            path,
            &db,
            lease,
            &mut mutation_batch,
            &mut manifest,
            ("anomaly_alert", &alert_id),
            serde_json::json!({
                "memory_id": alert.id,
                "alert_kind": alert.kind,
                "priority": alert.priority,
                "title": alert.title,
                "detail": alert.detail,
                "evidence_ids": alert.evidence_ids,
                "source_id": alert.source_id,
                "review_id": alert.review_id,
                "suggested_action": alert.suggested_action,
                "projection_version": GRAPH_PROJECTION_VERSION,
            }),
        )
        .await?;
        node_rows_written += 1;
    }

    // Commit node rows before relation batches. Besides keeping each batch
    // semantically uniform, this preserves SurrealDB 3.0.x compatibility: its
    // remote engine can reject a transaction that creates indexed nodes and
    // relates those freshly created records in the same commit.
    mutation_batch
        .flush(path, &db, lease, &mut manifest)
        .await?;

    for episode in &data.episodes {
        for mention in &episode.mentions {
            let Some(entity_id) = entity_ids.get(&entity_lookup_key(mention, episode.scope.as_ref()))
            else {
                continue;
            };
            let memory_id = projected_hash_id("mentions", &[&episode.id, entity_id]);
            relate_projected_records(
                path,
                &db,
                lease,
                &mut mutation_batch,
                &mut manifest,
                ProjectedRelationInput {
                    in_table: "episode",
                    in_id: &episode.id,
                    relation_table: "mentions",
                    out_table: "entity",
                    out_id: entity_id,
                    content: with_scope(
                    serde_json::json!({
                        "memory_id": memory_id,
                        "episode_id": episode.id,
                        "entity_id": entity_id,
                        "event_id": episode.event_id,
                        "projection_version": GRAPH_PROJECTION_VERSION,
                    }),
                    episode.scope.as_ref(),
                ),
                },
            )
            .await?;
            relation_rows_written += 1;
        }
    }

    for claim in &data.claims {
        if let Some(episode_id) = claim.source_episode_id.as_deref() {
            let memory_id = projected_hash_id("supports", &[&claim.id, episode_id]);
            relate_projected_records(
                path,
                &db,
                lease,
                &mut mutation_batch,
                &mut manifest,
                ProjectedRelationInput {
                    in_table: "claim",
                    in_id: &claim.id,
                    relation_table: "supports",
                    out_table: "episode",
                    out_id: episode_id,
                    content: with_scope(
                    serde_json::json!({
                        "memory_id": memory_id,
                        "subject_id": claim.id,
                        "evidence_id": episode_id,
                        "event_id": claim.event_id,
                        "projection_version": GRAPH_PROJECTION_VERSION,
                    }),
                    claim.scope.as_ref(),
                ),
                },
            )
            .await?;
            relation_rows_written += 1;
        }
    }

    for procedure in &data.procedures {
        if let Some(episode_id) = procedure.source_episode_id.as_deref() {
            let memory_id = projected_hash_id("supports", &[&procedure.id, episode_id]);
            relate_projected_records(
                path,
                &db,
                lease,
                &mut mutation_batch,
                &mut manifest,
                ProjectedRelationInput {
                    in_table: "procedure",
                    in_id: &procedure.id,
                    relation_table: "supports",
                    out_table: "episode",
                    out_id: episode_id,
                    content: with_scope(
                    serde_json::json!({
                        "memory_id": memory_id,
                        "subject_id": procedure.id,
                        "evidence_id": episode_id,
                        "event_id": procedure.event_id,
                        "projection_version": GRAPH_PROJECTION_VERSION,
                    }),
                    procedure.scope.as_ref(),
                ),
                },
            )
            .await?;
            relation_rows_written += 1;
        }
    }

    for intention in &data.intentions {
        if let Some(episode_id) = intention.source_episode_id.as_deref() {
            let memory_id = projected_hash_id("supports", &[&intention.id, episode_id]);
            relate_projected_records(
                path,
                &db,
                lease,
                &mut mutation_batch,
                &mut manifest,
                ProjectedRelationInput {
                    in_table: "intention",
                    in_id: &intention.id,
                    relation_table: "supports",
                    out_table: "episode",
                    out_id: episode_id,
                    content: with_scope(
                    serde_json::json!({
                        "memory_id": memory_id,
                        "subject_id": intention.id,
                        "evidence_id": episode_id,
                        "event_id": intention.event_id,
                        "projection_version": GRAPH_PROJECTION_VERSION,
                    }),
                    intention.scope.as_ref(),
                ),
                },
            )
            .await?;
            relation_rows_written += 1;
        }
    }

    for intention in &data.intentions {
        for dependency_id in &intention.depends_on {
            if !intention_ids.contains(dependency_id.as_str()) {
                continue;
            }
            let memory_id =
                projected_hash_id("intention_depends_on", &[&intention.id, dependency_id]);
            relate_projected_records(
                path,
                &db,
                lease,
                &mut mutation_batch,
                &mut manifest,
                ProjectedRelationInput {
                    in_table: "intention",
                    in_id: &intention.id,
                    relation_table: "intention_depends_on",
                    out_table: "intention",
                    out_id: dependency_id,
                    content: with_scope(
                        serde_json::json!({
                            "memory_id": memory_id,
                            "intention_id": intention.id,
                            "dependency_id": dependency_id,
                            "event_id": intention.updated_event_id,
                            "projection_version": GRAPH_PROJECTION_VERSION,
                        }),
                        intention.scope.as_ref(),
                    ),
                },
            )
            .await?;
            relation_rows_written += 1;
        }
    }

    for link in &data.links {
        let Some(from_id) = entity_ids.get(&entity_lookup_key(&link.from, link.scope.as_ref()))
        else {
            continue;
        };
        let Some(to_id) = entity_ids.get(&entity_lookup_key(&link.to, link.scope.as_ref())) else {
            continue;
        };
        relate_projected_records(
            path,
            &db,
            lease,
            &mut mutation_batch,
            &mut manifest,
            ProjectedRelationInput {
                in_table: "entity",
                in_id: from_id,
                relation_table: "relates_to",
                out_table: "entity",
                out_id: to_id,
                content: with_scope(
                serde_json::json!({
                    "memory_id": link.id,
                    "event_id": link.event_id,
                    "from_entity_id": from_id,
                    "from": link.from,
                    "relation": link.relation,
                    "to_entity_id": to_id,
                    "to": link.to,
                    "source_episode_id": link.source_episode_id,
                    "confidence": link.confidence,
                    "created_at_ms": link.created_at_ms,
                    "projection_version": GRAPH_PROJECTION_VERSION,
                }),
                link.scope.as_ref(),
            ),
            },
        )
        .await?;
        relation_rows_written += 1;
    }

    mutation_batch
        .flush(path, &db, lease, &mut manifest)
        .await?;
    delete_stale_projected_rows(path, &db, lease, &mut mutation_batch.existing).await?;
    node_rows_written = node_rows_written.saturating_sub(mutation_batch.unchanged_node_rows);
    relation_rows_written =
        relation_rows_written.saturating_sub(mutation_batch.unchanged_relation_rows);
    let expected_manifest = manifest.finish();
    let status_counts = expected_graph_projection_counts(data);
    create_single_projected_record(
        path,
        &db,
        lease,
        "projection_checkpoint",
        GRAPH_PROJECTION_CHECKPOINT_ID,
        serde_json::json!({
            "memory_id": GRAPH_PROJECTION_CHECKPOINT_ID,
            "checkpoint_id": GRAPH_PROJECTION_CHECKPOINT_ID,
            "projection_version": GRAPH_PROJECTION_VERSION,
            "memory_data_version": MEMORY_DATA_VERSION,
            "ledger_event_count": data.event_count,
            "latest_sequence": events.last().map(|event| event.sequence),
            "latest_event_id": data.last_event_id,
            "projected_at_ms": events.last().map(|event| event.timestamp_ms).unwrap_or(0),
            "table_counts": status_counts,
            "manifest_algorithm": GRAPH_PROJECTION_MANIFEST_ALGORITHM,
            "manifest_digest": expected_manifest.digest,
            "manifest_table_digests": expected_manifest.table_digests,
        }),
    )
    .await?;
    node_rows_written += 1;

    verify_graph_projection_lease(path, &db, lease).await?;
    let status = graph_projection_status_with_db(path, &db, data, events).await?;
    verify_graph_projection_lease(path, &db, lease).await?;
    ensure_graph_projection_rebuild_postcondition(GraphProjectionRebuildReport {
        status,
        node_rows_written,
        relation_rows_written,
    })
}

async fn acquire_graph_projection_rebuild_lock(
    path: &Path,
    db: &DatabaseSession,
    token: &str,
) -> Result<GraphProjectionLease> {
    initialize_graph_projection_rebuild_lock(path, db).await?;
    let attempts = GRAPH_PROJECTION_REBUILD_WAIT_MS / GRAPH_PROJECTION_REBUILD_POLL_MS;
    for attempt in 0..=attempts {
        let now_ms = now_ms();
        let expires_at_ms = now_ms.saturating_add(GRAPH_PROJECTION_REBUILD_LEASE_MS);
        let mut response = db
            .query_with_retry(
                path,
                format!(
                    "UPDATE ONLY projection_rebuild_lock:{GRAPH_PROJECTION_REBUILD_LOCK_ID} \
                     SET owner_token = $lease_token, expires_at_ms = $expires_at_ms, \
                         fencing_token = sequence::nextval('{GRAPH_PROJECTION_FENCING_SEQUENCE}') \
                     WHERE owner_token = NONE OR expires_at_ms < $now_ms \
                     RETURN AFTER"
                ),
                vec![
                    ("lease_token".to_string(), serde_json::json!(token)),
                    ("expires_at_ms".to_string(), serde_json::json!(expires_at_ms)),
                    ("now_ms".to_string(), serde_json::json!(now_ms)),
                ],
            )
            .await?;
        let lock: Option<serde_json::Value> = response
            .take(0)
            .map_err(|source| database_error(path, source))?;
        let lock = decode_projection_lease_row(path, lock)?;
        if let Some(lock) = lock
            && lock.owner_token.as_deref() == Some(token)
        {
            let lease = GraphProjectionLease {
                owner_token: token.to_string(),
                fencing_token: lock.fencing_token,
                expires_at_ms: AtomicU64::new(lock.expires_at_ms),
            };
            verify_graph_projection_lease(path, db, &lease).await?;
            return Ok(lease);
        }
        if attempt < attempts {
            tokio::time::sleep(std::time::Duration::from_millis(
                GRAPH_PROJECTION_REBUILD_POLL_MS,
            ))
            .await;
        }
    }
    Err(NahualiError::GraphProjectionRebuildBusy {
        timeout_ms: GRAPH_PROJECTION_REBUILD_WAIT_MS,
    })
}

async fn initialize_graph_projection_rebuild_lock(
    path: &Path,
    db: &DatabaseSession,
) -> Result<()> {
    db.query_with_retry(
        path,
        format!(
            "INSERT IGNORE INTO projection_rebuild_lock {{ \
                 id: projection_rebuild_lock:{GRAPH_PROJECTION_REBUILD_LOCK_ID}, \
                 owner_token: NONE, expires_at_ms: 0, fencing_token: 0 \
             }}"
        ),
        Vec::new(),
    )
    .await?;
    Ok(())
}

async fn verify_graph_projection_lease(
    path: &Path,
    db: &DatabaseSession,
    lease: &GraphProjectionLease,
) -> Result<()> {
    let mut response = db
        .query_with_retry(
            path,
            format!(
                "SELECT owner_token, fencing_token FROM ONLY \
                 projection_rebuild_lock:{GRAPH_PROJECTION_REBUILD_LOCK_ID}"
            ),
            Vec::new(),
        )
        .await?;
    let observed: Option<serde_json::Value> = response
        .take(0)
        .map_err(|source| database_error(path, source))?;
    let observed = decode_projection_lease_row(path, observed)?;
    if observed.as_ref().is_some_and(|observed| {
        observed.owner_token.as_deref() == Some(lease.owner_token.as_str())
            && observed.fencing_token == lease.fencing_token
    }) {
        Ok(())
    } else {
        Err(NahualiError::GraphProjectionLeaseLost {
            fencing_token: lease.fencing_token,
        })
    }
}

async fn release_graph_projection_rebuild_lock(
    path: &Path,
    db: &DatabaseSession,
    lease: &GraphProjectionLease,
) -> Result<()> {
    let mut response = db
        .query_with_retry(
            path,
            format!(
                "UPDATE ONLY projection_rebuild_lock:{GRAPH_PROJECTION_REBUILD_LOCK_ID} \
                 SET owner_token = NONE, expires_at_ms = 0 \
                 WHERE owner_token = $lease_token AND fencing_token = $fencing_token \
                 RETURN AFTER"
            ),
            vec![
                (
                    "lease_token".to_string(),
                    serde_json::json!(lease.owner_token.as_str()),
                ),
                (
                    "fencing_token".to_string(),
                    serde_json::json!(lease.fencing_token),
                ),
            ],
        )
        .await?;
    let released: Option<serde_json::Value> = response
        .take(0)
        .map_err(|source| database_error(path, source))?;
    let released = decode_projection_lease_row(path, released)?;
    if released
        .as_ref()
        .is_some_and(|released| released.fencing_token == lease.fencing_token)
    {
        Ok(())
    } else {
        Err(NahualiError::GraphProjectionLeaseLost {
            fencing_token: lease.fencing_token,
        })
    }
}

fn decode_projection_lease_row(
    path: &Path,
    row: Option<serde_json::Value>,
) -> Result<Option<ProjectionLeaseRow>> {
    row.map(|row| {
        serde_json::from_value(row).map_err(|source| NahualiError::DecodeRecord {
            path: path.to_path_buf(),
            record: 1,
            source,
        })
    })
    .transpose()
}

async fn read_idle_graph_projection_fence(
    path: &Path,
    db: &DatabaseSession,
) -> Result<u64> {
    let mut response = db
        .query_with_retry(
            path,
            format!(
                "SELECT owner_token, fencing_token FROM ONLY \
                 projection_rebuild_lock:{GRAPH_PROJECTION_REBUILD_LOCK_ID}"
            ),
            Vec::new(),
        )
        .await?;
    let row: Option<serde_json::Value> = response
        .take(0)
        .map_err(|source| database_error(path, source))?;
    let row = decode_projection_lease_row(path, row)?;
    match row {
        Some(row) if row.owner_token.is_none() => Ok(row.fencing_token),
        Some(_) => Err(NahualiError::GraphProjectionInvalid {
            issues: "a graph projection rebuild is active".to_string(),
        }),
        None => Err(NahualiError::GraphProjectionInvalid {
            issues: "the graph projection rebuild fence is missing".to_string(),
        }),
    }
}

async fn graph_projection_status(
    path: &Path,
    data: &MemoryData,
    events: &[EventEnvelope],
) -> Result<GraphProjectionStatus> {
    let db = open_database(path).await?;
    graph_projection_status_with_db(path, &db, data, events).await
}

async fn graph_projection_status_with_db(
    path: &Path,
    db: &DatabaseSession,
    data: &MemoryData,
    events: &[EventEnvelope],
) -> Result<GraphProjectionStatus> {
    let mut table_counts = BTreeMap::new();
    for table in ["projection_checkpoint", "projection_error"] {
        table_counts.insert(table.to_string(), count_projected_rows(path, db, table).await?);
    }

    let mut response = db
        .query_with_retry(
            path,
            "SELECT projection_version, memory_data_version, latest_sequence, latest_event_id, \
                    manifest_algorithm, manifest_digest, manifest_table_digests \
             FROM projection_checkpoint WHERE checkpoint_id = $checkpoint_id",
            vec![(
                "checkpoint_id".to_string(),
                serde_json::Value::String(GRAPH_PROJECTION_CHECKPOINT_ID.to_string()),
            )],
        )
        .await
        ?;
    let checkpoint_rows: Vec<serde_json::Value> = response
        .take(0)
        .map_err(|source| database_error(path, source))?;
    let checkpoints = checkpoint_rows
        .into_iter()
        .enumerate()
        .map(|(index, row)| {
            serde_json::from_value(row).map_err(|source| NahualiError::DecodeRecord {
                path: path.to_path_buf(),
                record: index + 1,
                source,
            })
        })
        .collect::<Result<Vec<ProjectionCheckpointRow>>>()?;
    let checkpoint = checkpoints.into_iter().next();
    let checkpoint_projection_version = checkpoint
        .as_ref()
        .and_then(|checkpoint| checkpoint.projection_version);
    let checkpoint_memory_data_version = checkpoint
        .as_ref()
        .and_then(|checkpoint| checkpoint.memory_data_version);
    let checkpoint_sequence = checkpoint
        .as_ref()
        .and_then(|checkpoint| checkpoint.latest_sequence);
    let checkpoint_event_id = checkpoint
        .as_ref()
        .and_then(|checkpoint| checkpoint.latest_event_id.clone());
    let checkpoint_manifest_algorithm = checkpoint
        .as_ref()
        .and_then(|checkpoint| checkpoint.manifest_algorithm.clone());
    let checkpoint_manifest_digest = checkpoint
        .as_ref()
        .and_then(|checkpoint| checkpoint.manifest_digest.clone());
    let checkpoint_manifest_table_digests = checkpoint
        .map(|checkpoint| checkpoint.manifest_table_digests)
        .unwrap_or_default();
    let (actual_manifest, manifest_table_counts) = read_projection_manifest(path, db).await?;
    table_counts.extend(manifest_table_counts);
    let latest_sequence = events.last().map(|event| event.sequence);
    let latest_event_id = data.last_event_id.clone();
    let expected_counts = expected_graph_projection_counts(data);
    let counts_match = expected_counts
        .iter()
        .all(|(table, expected)| table_counts.get(table).copied().unwrap_or(0) == *expected);

    Ok(GraphProjectionStatus {
        projection_version: GRAPH_PROJECTION_VERSION,
        memory_data_version: MEMORY_DATA_VERSION,
        checkpoint_projection_version,
        checkpoint_memory_data_version,
        checkpoint_manifest_algorithm: checkpoint_manifest_algorithm.clone(),
        checkpoint_manifest_digest: checkpoint_manifest_digest.clone(),
        checkpoint_manifest_table_digests: checkpoint_manifest_table_digests.clone(),
        actual_manifest_digest: actual_manifest.digest.clone(),
        actual_manifest_table_digests: actual_manifest.table_digests.clone(),
        ledger_event_count: data.event_count,
        latest_sequence,
        latest_event_id: latest_event_id.clone(),
        checkpoint_sequence,
        checkpoint_event_id: checkpoint_event_id.clone(),
        table_counts,
        in_sync: counts_match
            && checkpoint_projection_version == Some(GRAPH_PROJECTION_VERSION)
            && checkpoint_memory_data_version == Some(MEMORY_DATA_VERSION)
            && checkpoint_manifest_algorithm.as_deref()
                == Some(GRAPH_PROJECTION_MANIFEST_ALGORITHM)
            && checkpoint_manifest_digest.as_deref() == Some(actual_manifest.digest.as_str())
            && checkpoint_manifest_table_digests == actual_manifest.table_digests
            && checkpoint_sequence == latest_sequence
            && checkpoint_event_id == latest_event_id,
    })
}

async fn clear_graph_projection(
    path: &Path,
    db: &DatabaseSession,
    lease: &GraphProjectionLease,
) -> Result<()> {
    let statement = PROJECTED_RELATION_TABLES
        .iter()
        .chain(PROJECTED_NODE_TABLES.iter())
        .map(|table| format!("DELETE {table}"))
        .collect::<Vec<_>>()
        .join("; ");

    // DELETE maintains SurrealDB indexes transactionally. The lock-row write
    // and all projection deletes commit or roll back as one fenced unit.
    query_graph_projection_mutation(path, db, lease, statement, Vec::new()).await
}

fn projection_baseline_is_trustworthy(status: &GraphProjectionStatus) -> bool {
    status.checkpoint_projection_version == Some(GRAPH_PROJECTION_VERSION)
        && status.checkpoint_memory_data_version == Some(MEMORY_DATA_VERSION)
        && status.checkpoint_manifest_algorithm.as_deref()
            == Some(GRAPH_PROJECTION_MANIFEST_ALGORITHM)
        && status.checkpoint_manifest_digest.as_deref()
            == Some(status.actual_manifest_digest.as_str())
        && status.checkpoint_manifest_table_digests == status.actual_manifest_table_digests
        && status.table_counts.get("projection_checkpoint") == Some(&1)
        && status.table_counts.get("projection_error") == Some(&0)
}

async fn read_existing_projection(
    path: &Path,
    db: &DatabaseSession,
) -> Result<Option<ExistingProjection>> {
    let mut existing = ExistingProjection::default();
    for table in MANIFEST_NODE_TABLES
        .iter()
        .chain(PROJECTED_RELATION_TABLES.iter())
    {
        let mut response = db
            .query_with_retry(path, format!("SELECT * FROM {table}"), Vec::new())
            .await?;
        let rows: Vec<serde_json::Value> = response
            .take(0)
            .map_err(|source| database_error(path, source))?;
        let table_rows = existing.rows.entry((*table).to_string()).or_default();
        for mut row in rows {
            if let serde_json::Value::Object(object) = &mut row {
                object.remove("id");
                object.remove("in");
                object.remove("out");
            }
            let Some(memory_id) = row
                .get("memory_id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
            else {
                return Ok(None);
            };
            if table_rows.insert(memory_id, row).is_some() {
                return Ok(None);
            }
        }
    }
    Ok(Some(existing))
}

async fn delete_stale_projected_rows(
    path: &Path,
    db: &DatabaseSession,
    lease: &GraphProjectionLease,
    existing: &mut ExistingProjection,
) -> Result<()> {
    for table in PROJECTED_RELATION_TABLES
        .iter()
        .chain(MANIFEST_NODE_TABLES.iter())
    {
        let Some(rows) = existing.rows.remove(*table) else {
            continue;
        };
        let memory_ids = rows.into_keys().collect::<Vec<_>>();
        for chunk in memory_ids.chunks(GRAPH_PROJECTION_MUTATION_BATCH_SIZE) {
            delete_projected_rows_by_memory_id(path, db, lease, table, chunk).await?;
        }
    }
    Ok(())
}

async fn delete_projected_rows_by_memory_id(
    path: &Path,
    db: &DatabaseSession,
    lease: &GraphProjectionLease,
    table: &str,
    memory_ids: &[String],
) -> Result<()> {
    debug_assert!(
        MANIFEST_NODE_TABLES.contains(&table) || PROJECTED_RELATION_TABLES.contains(&table)
    );
    if memory_ids.is_empty() {
        return Ok(());
    }
    query_graph_projection_mutation(
        path,
        db,
        lease,
        format!("DELETE {table} WHERE memory_id IN $memory_ids"),
        vec![(
            "memory_ids".to_string(),
            serde_json::json!(memory_ids),
        )],
    )
    .await
}

async fn create_projected_record(
    path: &Path,
    db: &DatabaseSession,
    lease: &GraphProjectionLease,
    batch: &mut ProjectionMutationBatch,
    manifest: &mut ProjectionManifestBuilder,
    record: (&str, &str),
    content: serde_json::Value,
) -> Result<bool> {
    match batch.take_existing(record.0, record.1, &content) {
        ExistingRow::Unchanged => {
            batch.unchanged_node_rows += 1;
            manifest.record(record.0, &content);
            return Ok(false);
        }
        ExistingRow::Missing | ExistingRow::Changed => {}
    }
    batch.queue_record(record.0, record.1, content);
    if batch.is_full() {
        batch.flush(path, db, lease, manifest).await?;
    }
    Ok(true)
}

async fn create_single_projected_record(
    path: &Path,
    db: &DatabaseSession,
    lease: &GraphProjectionLease,
    table: &str,
    id: &str,
    content: serde_json::Value,
) -> Result<()> {
    query_graph_projection_mutation(
        path,
        db,
        lease,
        "UPSERT type::record($table, $id) CONTENT $content",
        vec![
            ("table".to_string(), serde_json::Value::String(table.to_string())),
            ("id".to_string(), serde_json::Value::String(id.to_string())),
            ("content".to_string(), content),
        ],
    )
    .await
}

async fn relate_projected_records(
    path: &Path,
    db: &DatabaseSession,
    lease: &GraphProjectionLease,
    batch: &mut ProjectionMutationBatch,
    manifest: &mut ProjectionManifestBuilder,
    input: ProjectedRelationInput<'_>,
) -> Result<bool> {
    match batch.take_existing(input.relation_table, input.memory_id(), &input.content) {
        ExistingRow::Unchanged => {
            batch.unchanged_relation_rows += 1;
            manifest.record(input.relation_table, &input.content);
            return Ok(false);
        }
        ExistingRow::Changed => {
            batch.flush(path, db, lease, manifest).await?;
            delete_projected_rows_by_memory_id(
                path,
                db,
                lease,
                input.relation_table,
                &[input.memory_id().to_string()],
            )
            .await?;
        }
        ExistingRow::Missing => {}
    }
    if batch.requires_relation_flush(input.relation_table) {
        batch.flush(path, db, lease, manifest).await?;
    }
    batch.queue_relation(input);
    if batch.is_full() {
        batch.flush(path, db, lease, manifest).await?;
    }
    Ok(true)
}

impl ProjectionMutationBatch {
    fn new(existing: ExistingProjection) -> Self {
        Self {
            statements: Vec::with_capacity(GRAPH_PROJECTION_MUTATION_BATCH_SIZE),
            relation_rows: Vec::with_capacity(GRAPH_PROJECTION_MUTATION_BATCH_SIZE),
            relation_table: None,
            bindings: Vec::with_capacity(GRAPH_PROJECTION_MUTATION_BATCH_SIZE * 5),
            manifest_entries: Vec::with_capacity(GRAPH_PROJECTION_MUTATION_BATCH_SIZE),
            next_binding_index: 0,
            existing,
            unchanged_node_rows: 0,
            unchanged_relation_rows: 0,
        }
    }

    fn take_existing(
        &mut self,
        table: &str,
        memory_id: &str,
        expected: &serde_json::Value,
    ) -> ExistingRow {
        let Some(stored) = self
            .existing
            .rows
            .get_mut(table)
            .and_then(|rows| rows.remove(memory_id))
        else {
            return ExistingRow::Missing;
        };
        if canonicalize_json(&stored) == canonicalize_json(expected) {
            ExistingRow::Unchanged
        } else {
            ExistingRow::Changed
        }
    }

    fn queue_record(&mut self, table: &str, id: &str, content: serde_json::Value) {
        debug_assert!(self.relation_table.is_none());
        let prefix = format!("projection_record_{}", self.next_binding_index);
        self.next_binding_index += 1;
        self.statements.push(format!(
            "UPSERT type::record(${prefix}_table, ${prefix}_id) CONTENT ${prefix}_content"
        ));
        self.bindings.extend([
            (
                format!("{prefix}_table"),
                serde_json::Value::String(table.to_string()),
            ),
            (
                format!("{prefix}_id"),
                serde_json::Value::String(id.to_string()),
            ),
            (format!("{prefix}_content"), content.clone()),
        ]);
        self.manifest_entries.push((table.to_string(), content));
    }

    fn queue_relation(&mut self, input: ProjectedRelationInput<'_>) {
        debug_assert!(self.statements.is_empty());
        let relation_table = PROJECTED_RELATION_TABLES
            .iter()
            .copied()
            .find(|table| *table == input.relation_table)
            .expect("projected relation tables are registered constants");
        self.relation_table
            .get_or_insert_with(|| relation_table.to_string());
        let prefix = format!("projection_relation_{}", self.next_binding_index);
        self.next_binding_index += 1;
        self.relation_rows.push(format!(
            "object::extend(${prefix}_content, {{ \
                 in: type::record(${prefix}_in_table, ${prefix}_in_id), \
                 out: type::record(${prefix}_out_table, ${prefix}_out_id) \
             }})"
        ));
        self.bindings.extend([
            (
                format!("{prefix}_in_table"),
                serde_json::Value::String(input.in_table.to_string()),
            ),
            (
                format!("{prefix}_in_id"),
                serde_json::Value::String(input.in_id.to_string()),
            ),
            (
                format!("{prefix}_out_table"),
                serde_json::Value::String(input.out_table.to_string()),
            ),
            (
                format!("{prefix}_out_id"),
                serde_json::Value::String(input.out_id.to_string()),
            ),
            (format!("{prefix}_content"), input.content.clone()),
        ]);
        self.manifest_entries
            .push((input.relation_table.to_string(), input.content));
    }

    fn requires_relation_flush(&self, relation_table: &str) -> bool {
        self.relation_table
            .as_deref()
            .is_some_and(|active| active != relation_table)
    }

    fn is_full(&self) -> bool {
        self.statements.len() + self.relation_rows.len()
            >= GRAPH_PROJECTION_MUTATION_BATCH_SIZE
    }

    async fn flush(
        &mut self,
        path: &Path,
        db: &DatabaseSession,
        lease: &GraphProjectionLease,
        manifest: &mut ProjectionManifestBuilder,
    ) -> Result<()> {
        if self.statements.is_empty() && self.relation_rows.is_empty() {
            return Ok(());
        }

        let relation_table = self.relation_table.take();
        let relation_rows = std::mem::take(&mut self.relation_rows);
        let statement = if let Some(relation_table) = relation_table {
            debug_assert!(self.statements.is_empty());
            format!(
                "INSERT RELATION INTO {relation_table} [{}]",
                relation_rows.join(", ")
            )
        } else {
            debug_assert!(relation_rows.is_empty());
            std::mem::take(&mut self.statements).join("; ")
        };
        let bindings = std::mem::take(&mut self.bindings);
        let manifest_entries = std::mem::take(&mut self.manifest_entries);
        self.next_binding_index = 0;

        query_graph_projection_mutation(path, db, lease, statement, bindings).await?;
        for (table, content) in manifest_entries {
            manifest.record(&table, &content);
        }
        Ok(())
    }
}

async fn query_graph_projection_mutation(
    path: &Path,
    db: &DatabaseSession,
    lease: &GraphProjectionLease,
    statement: impl AsRef<str>,
    mut bindings: Vec<(String, serde_json::Value)>,
) -> Result<()> {
    let guard_expires_at_ms = now_ms().saturating_add(GRAPH_PROJECTION_REBUILD_LEASE_MS);
    bindings.extend([
        (
            "projection_lease_token".to_string(),
            serde_json::json!(lease.owner_token.as_str()),
        ),
        (
            "projection_fencing_token".to_string(),
            serde_json::json!(lease.fencing_token),
        ),
        (
            "projection_guard_expires_at_ms".to_string(),
            serde_json::json!(guard_expires_at_ms),
        ),
    ]);
    // Every projection mutation first writes the permanent lock row inside the
    // same explicit transaction. A replacement owner must write that same row,
    // so SurrealKV produces a write-write conflict instead of permitting write
    // skew. Direct conflicts are retried by the database layer; commit-time
    // conflicts roll back the whole batch and are mapped to a typed lease loss
    // after the fresh fence read below.
    let query = format!(
        "BEGIN TRANSACTION; \
         LET $projection_lease = UPDATE ONLY \
             projection_rebuild_lock:{GRAPH_PROJECTION_REBUILD_LOCK_ID} \
             SET expires_at_ms = $projection_guard_expires_at_ms, \
                 mutation_guard_token = sequence::nextval('{GRAPH_PROJECTION_MUTATION_GUARD_SEQUENCE}') \
             WHERE owner_token = $projection_lease_token \
                 AND fencing_token = $projection_fencing_token \
             RETURN AFTER; \
         IF $projection_lease != NONE \
             AND $projection_lease.owner_token = $projection_lease_token \
             AND $projection_lease.fencing_token = $projection_fencing_token {{ \
             {}; \
             true \
         }} ELSE {{ \
             false \
         }}; \
         COMMIT TRANSACTION;",
        statement.as_ref()
    );
    let mut response = match db.query_with_retry(path, query, bindings).await {
        Ok(response) => response,
        Err(error) if is_failed_projection_transaction(&error) => {
            // SurrealDB 3.x reports an explicit transaction's commit conflict
            // as QueryError::NotExecuted. The transaction has rolled back; a
            // fresh lease read distinguishes a replacement-owner conflict and
            // preserves the projection API's typed fencing error.
            if let Err(lease_error @ NahualiError::GraphProjectionLeaseLost { .. }) =
                verify_graph_projection_lease(path, db, lease).await
            {
                return Err(lease_error);
            }
            return Err(error);
        }
        Err(error) => return Err(error),
    };
    let authorized: Option<serde_json::Value> = response
        .take(2)
        .map_err(|source| database_error(path, source))?;
    if authorized.as_ref().and_then(serde_json::Value::as_bool) == Some(true) {
        lease
            .expires_at_ms
            .store(guard_expires_at_ms, Ordering::Release);
        Ok(())
    } else {
        Err(NahualiError::GraphProjectionLeaseLost {
            fencing_token: lease.fencing_token,
        })
    }
}

fn is_failed_projection_transaction(error: &NahualiError) -> bool {
    matches!(
        error,
        NahualiError::Database { source, .. }
            if matches!(
                source.query_details(),
                Some(surrealdb::types::QueryError::NotExecuted)
            )
    )
}

impl ProjectionManifestBuilder {
    fn new() -> Self {
        let row_digests = MANIFEST_NODE_TABLES
            .iter()
            .chain(PROJECTED_RELATION_TABLES.iter())
            .map(|table| ((*table).to_string(), Vec::new()))
            .collect();
        Self { row_digests }
    }

    fn record(&mut self, table: &str, content: &serde_json::Value) {
        self.row_digests
            .get_mut(table)
            .expect("manifest tables are registered at construction")
            .push(canonical_json_digest(content));
    }

    fn finish(self) -> ProjectionManifest {
        projection_manifest(self.row_digests)
    }
}

async fn read_projection_manifest(
    path: &Path,
    db: &DatabaseSession,
) -> Result<(ProjectionManifest, BTreeMap<String, usize>)> {
    let mut manifest = ProjectionManifestBuilder::new();
    let mut table_counts = BTreeMap::new();
    for table in MANIFEST_NODE_TABLES
        .iter()
        .chain(PROJECTED_RELATION_TABLES.iter())
    {
        let mut response = db
            .query_with_retry(path, format!("SELECT * FROM {table}"), Vec::new())
            .await?;
        let rows: Vec<serde_json::Value> = response
            .take(0)
            .map_err(|source| database_error(path, source))?;
        table_counts.insert((*table).to_string(), rows.len());
        for mut row in rows {
            // SurrealDB injects physical record identity and relation endpoints.
            // The projected `memory_id` and all domain fields remain in the
            // digest, so row identity and content mutations are still detected.
            if let serde_json::Value::Object(object) = &mut row {
                object.remove("id");
                object.remove("in");
                object.remove("out");
            }
            manifest.record(table, &row);
        }
    }
    Ok((manifest.finish(), table_counts))
}

fn canonical_json_digest(value: &serde_json::Value) -> [u8; 32] {
    let canonical = canonicalize_json(value);
    let encoded = serde_json::to_vec(&canonical)
        .expect("serializing an in-memory JSON value cannot fail");
    Sha256::digest(encoded).into()
}

fn canonicalize_json(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.iter().map(canonicalize_json).collect())
        }
        serde_json::Value::Object(object) => {
            let sorted = object
                .iter()
                .map(|(key, value)| (key.clone(), canonicalize_json(value)))
                .collect::<BTreeMap<_, _>>();
            serde_json::Value::Object(sorted.into_iter().collect())
        }
        other => other.clone(),
    }
}

fn projection_manifest(
    row_digests: BTreeMap<String, Vec<[u8; 32]>>,
) -> ProjectionManifest {
    let mut global_hasher = Sha256::new();
    digest_part(&mut global_hasher, b"nahuali-graph-projection-manifest-v1");
    let mut table_digests = BTreeMap::new();

    for (table, mut rows) in row_digests {
        rows.sort_unstable();
        let mut table_hasher = Sha256::new();
        digest_part(
            &mut table_hasher,
            b"nahuali-graph-projection-table-manifest-v1",
        );
        digest_part(&mut table_hasher, table.as_bytes());
        digest_part(&mut table_hasher, &(rows.len() as u64).to_be_bytes());
        for row in rows {
            digest_part(&mut table_hasher, &row);
        }
        let table_digest: [u8; 32] = table_hasher.finalize().into();
        digest_part(&mut global_hasher, table.as_bytes());
        digest_part(&mut global_hasher, &table_digest);
        table_digests.insert(table, hex_digest(&table_digest));
    }

    let digest: [u8; 32] = global_hasher.finalize().into();
    ProjectionManifest {
        digest: hex_digest(&digest),
        table_digests,
    }
}

fn digest_part(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn hex_digest(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

fn manifest_mismatched_tables(
    expected: &BTreeMap<String, String>,
    actual: &BTreeMap<String, String>,
) -> Vec<String> {
    expected
        .keys()
        .chain(actual.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|table| expected.get(*table) != actual.get(*table))
        .cloned()
        .collect()
}

fn ensure_graph_projection_rebuild_postcondition(
    report: GraphProjectionRebuildReport,
) -> Result<GraphProjectionRebuildReport> {
    if report.status.in_sync {
        return Ok(report);
    }

    let mismatched_tables = manifest_mismatched_tables(
        &report.status.checkpoint_manifest_table_digests,
        &report.status.actual_manifest_table_digests,
    );
    Err(NahualiError::GraphProjectionPostconditionFailed {
        issues: format!(
            "status.in_sync=false; checkpoint projection version {:?}; checkpoint memory data version {:?}; manifest tables [{}]",
            report.status.checkpoint_projection_version,
            report.status.checkpoint_memory_data_version,
            mismatched_tables.join(", ")
        ),
    })
}

async fn count_projected_rows(path: &Path, db: &DatabaseSession, table: &str) -> Result<usize> {
    let mut response = db
        .query_with_retry(path, format!("SELECT memory_id FROM {table}"), Vec::new())
        .await
        ?;
    let rows: Vec<serde_json::Value> = response
        .take(0)
        .map_err(|source| database_error(path, source))?;
    Ok(rows.len())
}

async fn select_projected_rows<T>(
    path: &Path,
    db: &DatabaseSession,
    query: &str,
) -> Result<Vec<T>>
where
    T: serde::de::DeserializeOwned,
{
    let mut response = db
        .query_with_retry(path, query, Vec::new())
        .await
        ?;
    let rows: Vec<serde_json::Value> = response
        .take(0)
        .map_err(|source| database_error(path, source))?;
    rows.into_iter()
        .enumerate()
        .map(|(index, row)| {
            serde_json::from_value(row).map_err(|source| NahualiError::DecodeRecord {
                path: path.to_path_buf(),
                record: index + 1,
                source,
            })
        })
        .collect()
}

fn projected_tables() -> impl Iterator<Item = &'static str> {
    PROJECTED_NODE_TABLES
        .iter()
        .chain(PROJECTED_RELATION_TABLES.iter())
        .copied()
}

fn expected_graph_projection_counts(data: &MemoryData) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for table in projected_tables() {
        counts.insert(table.to_string(), 0);
    }

    counts.insert("projection_checkpoint".to_string(), 1);
    counts.insert("memory_scope".to_string(), projected_scopes(data).len());
    counts.insert("source_record".to_string(), data.sources.len());
    counts.insert("episode".to_string(), data.episodes.len());
    counts.insert("entity".to_string(), data.entities.len());
    counts.insert("claim".to_string(), data.claims.len());
    counts.insert("procedure".to_string(), data.procedures.len());
    counts.insert("intention".to_string(), data.intentions.len());
    counts.insert(
        "health_signal".to_string(),
        KnowledgeHealth::inspect(data).signals.len(),
    );
    counts.insert(
        "review_item".to_string(),
        operator_review::operator_review(data, OperatorReviewOptions::default()).items.len(),
    );
    counts.insert("review_decision".to_string(), data.review_decisions.len());
    counts.insert(
        "anomaly_alert".to_string(),
        proactive::anomaly_report(data, ProactiveOptions::default()).alerts.len(),
    );

    let entity_ids = entity_id_lookup(data);
    let mention_count = data
        .episodes
        .iter()
        .map(|episode| {
            episode
                .mentions
                .iter()
                .filter(|mention| {
                    entity_ids.contains_key(&entity_lookup_key(mention, episode.scope.as_ref()))
                })
                .count()
        })
        .sum();
    counts.insert("mentions".to_string(), mention_count);

    let supports_count = data
        .claims
        .iter()
        .filter(|claim| claim.source_episode_id.is_some())
        .count()
        + data
            .procedures
            .iter()
            .filter(|procedure| procedure.source_episode_id.is_some())
            .count()
        + data
            .intentions
            .iter()
            .filter(|intention| intention.source_episode_id.is_some())
            .count();
    counts.insert("supports".to_string(), supports_count);

    let relates_to_count = data
        .links
        .iter()
        .filter(|link| {
            entity_ids.contains_key(&entity_lookup_key(&link.from, link.scope.as_ref()))
                && entity_ids.contains_key(&entity_lookup_key(&link.to, link.scope.as_ref()))
        })
        .count();
    counts.insert("relates_to".to_string(), relates_to_count);

    let intention_ids = intention_id_lookup(data);
    let intention_depends_on_count = data
        .intentions
        .iter()
        .map(|intention| {
            intention
                .depends_on
                .iter()
                .filter(|dependency_id| intention_ids.contains(dependency_id.as_str()))
                .count()
        })
        .sum();
    counts.insert(
        "intention_depends_on".to_string(),
        intention_depends_on_count,
    );

    counts
}

fn event_meta(events: &[EventEnvelope]) -> BTreeMap<&str, EventMeta> {
    events
        .iter()
        .map(|event| {
            (
                event.id.as_str(),
                EventMeta {
                    sequence: event.sequence,
                },
            )
        })
        .collect()
}

fn projected_scopes(data: &MemoryData) -> Vec<MemoryScope> {
    let mut scopes = BTreeMap::new();
    for scope in data
        .sources
        .iter()
        .filter_map(|item| item.scope.clone())
        .chain(data.episodes.iter().filter_map(|item| item.scope.clone()))
        .chain(data.entities.iter().filter_map(|item| item.scope.clone()))
        .chain(data.claims.iter().filter_map(|item| item.scope.clone()))
        .chain(data.links.iter().filter_map(|item| item.scope.clone()))
        .chain(data.procedures.iter().filter_map(|item| item.scope.clone()))
        .chain(data.intentions.iter().filter_map(|item| item.scope.clone()))
        .chain(
            data.review_decisions
                .iter()
                .filter_map(|item| item.scope.clone()),
        )
    {
        scopes.insert(scope.key.clone(), scope);
    }
    scopes.into_values().collect()
}

fn entity_id_lookup(data: &MemoryData) -> BTreeMap<String, String> {
    data.entities
        .iter()
        .map(|entity| {
            (
                entity_lookup_key(&entity.name, entity.scope.as_ref()),
                entity.id.clone(),
            )
        })
        .collect()
}

fn intention_id_lookup(data: &MemoryData) -> BTreeSet<&str> {
    data.intentions
        .iter()
        .map(|intention| intention.id.as_str())
        .collect()
}

fn entity_lookup_key(name: &str, scope: Option<&MemoryScope>) -> String {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    let key = name.to_ascii_lowercase();
    if let Some(scope) = scope {
        format!("{}::{key}", scope.key)
    } else {
        key
    }
}

fn with_scope(mut value: serde_json::Value, scope: Option<&MemoryScope>) -> serde_json::Value {
    if let serde_json::Value::Object(object) = &mut value {
        object.insert(
            "scope_key".to_string(),
            scope
                .map(|scope| serde_json::Value::String(scope.key.clone()))
                .unwrap_or(serde_json::Value::Null),
        );
        object.insert(
            "scope_kind".to_string(),
            scope
                .map(|scope| serde_json::Value::String(scope.kind.as_key().to_string()))
                .unwrap_or(serde_json::Value::Null),
        );
        object.insert(
            "scope_name".to_string(),
            scope
                .map(|scope| serde_json::Value::String(scope.name.clone()))
                .unwrap_or(serde_json::Value::Null),
        );
    }
    value
}

fn projected_hash_id(prefix: &str, parts: &[&str]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for part in parts {
        for byte in part.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{prefix}_{hash:016x}")
}

fn clean_projection_query(query: &str) -> Option<String> {
    let query = query.split_whitespace().collect::<Vec<_>>().join(" ");
    if query.is_empty() { None } else { Some(query) }
}
