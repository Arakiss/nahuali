/// Current SurrealDB graph projection schema version.
pub const GRAPH_PROJECTION_VERSION: u32 = 1;

const GRAPH_PROJECTION_CHECKPOINT_ID: &str = "graph_v1";
const GRAPH_PROJECTION_REBUILD_LOCK_ID: &str = "graph_v1";
const GRAPH_PROJECTION_REBUILD_LEASE_MS: u64 = 120_000;
const GRAPH_PROJECTION_REBUILD_WAIT_MS: u64 = 30_000;
const GRAPH_PROJECTION_REBUILD_POLL_MS: u64 = 50;

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

const PROJECTED_UNIQUE_INDEXES: &[(&str, &str)] = &[
    ("projection_checkpoint", "projection_checkpoint_version_idx"),
    ("memory_scope", "memory_scope_key_idx"),
    ("source_record", "source_record_memory_id_idx"),
    ("episode", "episode_memory_id_idx"),
    ("episode", "episode_event_idx"),
    ("entity", "entity_memory_id_idx"),
    ("claim", "claim_memory_id_idx"),
    ("claim", "claim_event_idx"),
    ("procedure", "procedure_memory_id_idx"),
    ("procedure", "procedure_event_idx"),
    ("intention", "intention_memory_id_idx"),
    ("intention", "intention_event_idx"),
    ("health_signal", "health_signal_memory_id_idx"),
    ("review_item", "review_item_memory_id_idx"),
    ("review_decision", "review_decision_memory_id_idx"),
    ("review_decision", "review_decision_event_idx"),
    ("inferred_claim", "inferred_claim_memory_id_idx"),
    ("contradiction", "contradiction_memory_id_idx"),
    ("anomaly_alert", "anomaly_alert_memory_id_idx"),
    ("mentions", "mentions_memory_id_idx"),
    ("supports", "supports_memory_id_idx"),
    ("relates_to", "relates_to_memory_id_idx"),
    ("intention_depends_on", "intention_depends_on_memory_id_idx"),
];

/// Table counts and checkpoint state for the SurrealDB graph projection.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct GraphProjectionStatus {
    /// Projection schema version expected by this runtime.
    pub projection_version: u32,
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
    latest_sequence: Option<u64>,
    latest_event_id: Option<String>,
}

struct ProjectedRelationInput<'a> {
    in_table: &'a str,
    in_id: &'a str,
    relation_table: &'a str,
    out_table: &'a str,
    out_id: &'a str,
    content: serde_json::Value,
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
        let path = self.path.clone();
        let query = query.map(str::to_string);
        block_on_database(async move {
            let db = open_database(&path).await?;
            let mut entities: Vec<GraphProjectionEntity> =
                select_projected_rows(&path, &db, "SELECT memory_id, name, mention_count, scope_key, source_event_ids, last_seen_at_ms FROM entity ORDER BY last_seen_at_ms DESC").await?;
            if let Some(query) = query.and_then(|query| clean_projection_query(&query)) {
                let query = query.to_ascii_lowercase();
                entities.retain(|entity| entity.name.to_ascii_lowercase().contains(&query));
            }
            entities.truncate(limit.max(1));
            Ok(entities)
        })
    }

    /// Read recent projected episodes directly from SurrealDB graph projection tables.
    pub fn projection_timeline(&self, limit: usize) -> Result<Vec<GraphProjectionEpisode>> {
        let path = self.path.clone();
        block_on_database(async move {
            let db = open_database(&path).await?;
            let mut episodes: Vec<GraphProjectionEpisode> =
                select_projected_rows(&path, &db, "SELECT memory_id, event_id, content, tags, mentions, source_id, scope_key, created_at_ms FROM episode ORDER BY created_at_ms DESC").await?;
            episodes.truncate(limit.max(1));
            Ok(episodes)
        })
    }

    /// Read active pending intentions directly from SurrealDB graph projection tables.
    pub fn projection_pending(
        &self,
        limit: usize,
    ) -> Result<Vec<GraphProjectionPendingIntention>> {
        let path = self.path.clone();
        block_on_database(async move {
            let db = open_database(&path).await?;
            let mut intentions: Vec<GraphProjectionPendingIntention> =
                select_projected_rows(&path, &db, "SELECT memory_id, event_id, intention_kind, status, priority, description, source_episode_id, deadline_at_ms, depends_on, goal_id, progress_percent, scope_key, created_at_ms, updated_at_ms FROM intention WHERE status = 'active' ORDER BY updated_at_ms DESC").await?;
            intentions.truncate(limit.max(1));
            Ok(intentions)
        })
    }

    /// Read projected health signals directly from SurrealDB graph projection tables.
    pub fn projection_health_signals(
        &self,
        limit: usize,
    ) -> Result<Vec<GraphProjectionHealthSignal>> {
        let path = self.path.clone();
        block_on_database(async move {
            let db = open_database(&path).await?;
            let mut signals: Vec<GraphProjectionHealthSignal> =
                select_projected_rows(&path, &db, "SELECT memory_id, signal_kind, severity, message, evidence_ids FROM health_signal").await?;
            signals.truncate(limit.max(1));
            Ok(signals)
        })
    }
}

async fn rebuild_graph_projection(path: &Path) -> Result<GraphProjectionRebuildReport> {
    #[cfg(test)]
    if consume_injected_graph_projection_failure(path) {
        return Err(NahualiError::GraphProjectionRebuildBusy { timeout_ms: 0 });
    }
    let db = open_database(path).await?;
    let lock_token = make_id("projection_rebuild");
    if let Err(error) = acquire_graph_projection_rebuild_lock(path, &db, &lock_token).await {
        if matches!(&error, NahualiError::GraphProjectionRebuildBusy { .. })
            && let Some(report) = completed_concurrent_rebuild(path).await?
        {
            return Ok(report);
        }
        return Err(error);
    }
    let rebuild = async {
        let events = read_records(path).await?;
        let data = projection::project(&events);
        rebuild_graph_projection_locked(path, &data, &events, db.clone()).await
    }
    .await;
    let release = release_graph_projection_rebuild_lock(path, &db, &lock_token).await;
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
) -> Result<GraphProjectionRebuildReport> {
    clear_graph_projection(path, &db).await?;

    let event_meta = event_meta(events);
    let entity_ids = entity_id_lookup(data);
    let intention_ids = intention_id_lookup(data);
    let mut node_rows_written = 0;
    let mut relation_rows_written = 0;

    for scope in projected_scopes(data) {
        create_projected_record(
            path,
            &db,
            "memory_scope",
            &scope.key,
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
            "source_record",
            &source.id,
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
            "episode",
            &episode.id,
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
            "entity",
            &entity.id,
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
            "claim",
            &claim.id,
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
            "procedure",
            &procedure.id,
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
            "intention",
            &intention.id,
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
            "health_signal",
            &memory_id,
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
            "review_item",
            &item.id,
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
            "review_decision",
            &review.id,
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
            "anomaly_alert",
            &alert_id,
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

    let status_counts = expected_graph_projection_counts(data);
    create_projected_record(
        path,
        &db,
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
        }),
    )
    .await?;
    node_rows_written += 1;

    let status = graph_projection_status_with_db(path, &db, data, events).await?;
    Ok(GraphProjectionRebuildReport {
        status,
        node_rows_written,
        relation_rows_written,
    })
}

async fn acquire_graph_projection_rebuild_lock(
    path: &Path,
    db: &DatabaseSession,
    token: &str,
) -> Result<()> {
    let attempts = GRAPH_PROJECTION_REBUILD_WAIT_MS / GRAPH_PROJECTION_REBUILD_POLL_MS;
    for attempt in 0..=attempts {
        let now_ms = now_ms();
        let expires_at_ms = now_ms.saturating_add(GRAPH_PROJECTION_REBUILD_LEASE_MS);
        let mut response = db
            .query_with_retry(
                path,
                format!(
                    "UPSERT ONLY projection_rebuild_lock:{GRAPH_PROJECTION_REBUILD_LOCK_ID} \
                     SET owner_token = $lease_token, expires_at_ms = $expires_at_ms \
                     WHERE expires_at_ms = NONE OR expires_at_ms < $now_ms OR owner_token = $lease_token \
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
        if lock
            .as_ref()
            .and_then(|lock| lock.get("owner_token"))
            .and_then(serde_json::Value::as_str)
            == Some(token)
        {
            return Ok(());
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

async fn release_graph_projection_rebuild_lock(
    path: &Path,
    db: &DatabaseSession,
    token: &str,
) -> Result<()> {
    db.query_with_retry(
        path,
        format!(
            "DELETE projection_rebuild_lock:{GRAPH_PROJECTION_REBUILD_LOCK_ID} WHERE owner_token = $lease_token"
        ),
        vec![("lease_token".to_string(), serde_json::json!(token))],
    )
    .await?;
    Ok(())
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
    for table in projected_tables() {
        table_counts.insert(table.to_string(), count_projected_rows(path, db, table).await?);
    }

    let mut response = db
        .query_with_retry(
            path,
            "SELECT latest_sequence, latest_event_id FROM projection_checkpoint WHERE checkpoint_id = $checkpoint_id",
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
    let checkpoint_sequence = checkpoint
        .as_ref()
        .and_then(|checkpoint| checkpoint.latest_sequence);
    let checkpoint_event_id = checkpoint.and_then(|checkpoint| checkpoint.latest_event_id);
    let latest_sequence = events.last().map(|event| event.sequence);
    let latest_event_id = data.last_event_id.clone();
    let expected_counts = expected_graph_projection_counts(data);
    let counts_match = expected_counts
        .iter()
        .all(|(table, expected)| table_counts.get(table).copied().unwrap_or(0) == *expected);

    Ok(GraphProjectionStatus {
        projection_version: GRAPH_PROJECTION_VERSION,
        ledger_event_count: data.event_count,
        latest_sequence,
        latest_event_id: latest_event_id.clone(),
        checkpoint_sequence,
        checkpoint_event_id: checkpoint_event_id.clone(),
        table_counts,
        in_sync: counts_match
            && checkpoint_sequence == latest_sequence
            && checkpoint_event_id == latest_event_id,
    })
}

async fn clear_graph_projection(path: &Path, db: &DatabaseSession) -> Result<()> {
    for table in PROJECTED_RELATION_TABLES
        .iter()
        .chain(PROJECTED_NODE_TABLES.iter())
    {
        let query = format!("DELETE {table}");
        db.query_with_retry(path, query, Vec::new()).await?;
    }
    for (table, index) in PROJECTED_UNIQUE_INDEXES {
        db.query_with_retry(
            path,
            format!("REBUILD INDEX {index} ON TABLE {table}"),
            Vec::new(),
        )
        .await?;
    }
    Ok(())
}

async fn create_projected_record(
    path: &Path,
    db: &DatabaseSession,
    table: &str,
    id: &str,
    content: serde_json::Value,
) -> Result<()> {
    db.query_with_retry(
        path,
        "CREATE type::record($table, $id) CONTENT $content",
        vec![
            ("table".to_string(), serde_json::Value::String(table.to_string())),
            ("id".to_string(), serde_json::Value::String(id.to_string())),
            ("content".to_string(), content),
        ],
    )
    .await?;
    Ok(())
}

async fn relate_projected_records(
    path: &Path,
    db: &DatabaseSession,
    input: ProjectedRelationInput<'_>,
) -> Result<()> {
    let query = format!(
        "LET $in_record = type::record($in_table, $in_id); LET $out_record = type::record($out_table, $out_id); RELATE $in_record->{}->$out_record CONTENT $content;",
        input.relation_table
    );
    db.query_with_retry(
        path,
        query,
        vec![
            ("in_table".to_string(), serde_json::Value::String(input.in_table.to_string())),
            ("in_id".to_string(), serde_json::Value::String(input.in_id.to_string())),
            ("out_table".to_string(), serde_json::Value::String(input.out_table.to_string())),
            ("out_id".to_string(), serde_json::Value::String(input.out_id.to_string())),
            ("content".to_string(), input.content),
        ],
    )
    .await?;
    Ok(())
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
