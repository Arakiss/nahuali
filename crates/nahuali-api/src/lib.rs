//! HTTP API surface for the Nahuali memory engine.
//!
//! The API is a thin transport layer over `nahuali-core`. It appends records
//! through the core engine, reads SurrealDB graph projection status through the
//! core projection APIs, and treats Qdrant as a derived semantic tier.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use axum::Json as AxumJson;
use axum::{
    Router,
    extract::{
        FromRequest, FromRequestParts, Request, State,
        rejection::{JsonRejection, QueryRejection},
    },
    http::{StatusCode, header, request::Parts},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use nahuali_core::{
    AnomalyAcknowledgementReport, AnomalyReport, AuthorityDecision, Claim, DeadlineReport, Episode,
    GoalProgressReport, GraphProjectionEpisode, GraphProjectionPendingIntention,
    GraphProjectionStatus, GraphProjectionValidation, HybridRecallReport, Intention, IntentionKind,
    IntentionPriority, IntentionReconciliationOptions, IntentionReconciliationReport,
    IntentionStatus, IntentionUpdateOptions, KnowledgeHealth, LedgerAudit, LedgerAuditOptions,
    Link, MemoryBriefingReport, MemoryEngine, MemoryGraphReport, MemoryKind, MemoryProactiveReport,
    MemoryScope, MemoryTrustReport, NahualiError, ProactiveOptions, Procedure, RecallOptions,
    ReviewResolutionReport, SemanticIndexReport, SemanticIndexStatus,
};
use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, RwLockMappedWriteGuard, RwLockReadGuard, RwLockWriteGuard};

/// Static OpenAPI contract for the beta HTTP surface.
pub const OPENAPI_JSON: &str = include_str!("../openapi.json");

/// HTTP API configuration.
#[derive(Clone, Debug)]
pub struct ApiConfig {
    database: PathBuf,
}

impl ApiConfig {
    /// Build API configuration for a SurrealDB-backed Nahuali database name.
    pub fn new(database: impl Into<PathBuf>) -> Self {
        Self {
            database: database.into(),
        }
    }

    /// Database path/name passed to `nahuali-core`.
    pub fn database(&self) -> &Path {
        &self.database
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self::new("memory")
    }
}

/// Build the HTTP router for the beta API.
pub fn router(config: ApiConfig) -> Router {
    let state = ApiState {
        database: Arc::new(config.database),
        engine: Arc::new(RwLock::new(None)),
    };

    Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .route("/v1/health", get(health))
        .route("/v1/status", get(status))
        .route("/v1/openapi.json", get(openapi))
        .route("/v1/episode", post(record_episode))
        .route("/v1/claim", post(record_claim))
        .route("/v1/link", post(record_link))
        .route("/v1/procedure", post(record_procedure))
        .route("/v1/intention", post(record_intention))
        .route("/v1/intention/update", post(update_intention))
        .route("/v1/intention/status", post(update_intention_status))
        .route("/v1/intention/reconcile", post(reconcile_intentions))
        .route("/v1/goal-progress", get(goal_progress))
        .route("/v1/proactive", post(proactive))
        .route("/v1/deadlines", post(deadlines))
        .route("/v1/anomalies", post(anomalies))
        .route("/v1/anomaly/acknowledge", post(acknowledge_anomaly))
        .route("/v1/recall", post(recall))
        .route("/v1/session-resume", post(session_resume))
        .route("/v1/memory-health", post(memory_health))
        .route("/v1/audit", get(audit))
        .route("/v1/trust-report", get(trust_report))
        .route("/v1/graph", get(graph))
        .route("/v1/timeline", get(timeline))
        .route("/v1/pending", get(pending))
        .route("/v1/projection/status", get(projection_status))
        .route("/v1/projection/rebuild", post(projection_rebuild))
        .route("/v1/projection/validate", post(projection_validate))
        .route("/v1/semantic/status", get(semantic_status))
        .route("/v1/semantic/rebuild", post(semantic_rebuild))
        .route("/v1/review/resolve", post(resolve_review))
        .fallback(not_found)
        .method_not_allowed_fallback(method_not_allowed)
        .with_state(state)
}

#[derive(Clone)]
struct ApiState {
    database: Arc<PathBuf>,
    /// Cached memory engine, opened lazily on first database-touching request and
    /// reused across requests so the ledger replay and SurrealDB graph projection
    /// rebuild that `MemoryEngine::open` performs run once instead of per request.
    ///
    /// Read endpoints take a shared read lock; mutating endpoints take an exclusive
    /// write lock. Engine mutations keep their in-memory projection in sync with the
    /// ledger they persist to, so the cached engine returns the same responses a fresh
    /// per-request open would. `/health`, `/v1/health`, and `/` never touch the engine,
    /// so the lazy slot stays empty until a real memory request arrives.
    engine: Arc<RwLock<Option<MemoryEngine>>>,
}

type ApiResult<T> = Result<Json<T>, ApiError>;

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

#[derive(Serialize)]
struct ApiErrorResponse {
    error: ApiErrorBody,
}

#[derive(Serialize)]
struct ApiErrorBody {
    code: &'static str,
    message: String,
}

impl ApiError {
    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }
}

impl From<NahualiError> for ApiError {
    fn from(error: NahualiError) -> Self {
        let message = error.to_string();
        match error {
            NahualiError::EmptyContent => {
                Self::new(StatusCode::BAD_REQUEST, "empty_content", message)
            }
            NahualiError::EmptyQuery => Self::new(StatusCode::BAD_REQUEST, "empty_query", message),
            NahualiError::InvalidScope { .. } => {
                Self::new(StatusCode::BAD_REQUEST, "invalid_scope", message)
            }
            NahualiError::UnknownIntention { .. } => {
                Self::new(StatusCode::NOT_FOUND, "unknown_intention", message)
            }
            NahualiError::InvalidIntentionUpdate { .. } => {
                Self::new(StatusCode::BAD_REQUEST, "invalid_intention_update", message)
            }
            NahualiError::UnknownReviewItem { .. } => {
                Self::new(StatusCode::NOT_FOUND, "unknown_review_item", message)
            }
            NahualiError::UnknownSource { .. } => {
                Self::new(StatusCode::NOT_FOUND, "unknown_source", message)
            }
            NahualiError::SemanticHttp { .. }
            | NahualiError::SemanticApi { .. }
            | NahualiError::SemanticDecode { .. } => Self::new(
                StatusCode::BAD_GATEWAY,
                "semantic_tier_unavailable",
                message,
            ),
            _ => Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", message),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            AxumJson(ApiErrorResponse {
                error: ApiErrorBody {
                    code: self.code,
                    message: self.message,
                },
            }),
        )
            .into_response()
    }
}

impl From<JsonRejection> for ApiError {
    fn from(rejection: JsonRejection) -> Self {
        let (status, code) = match &rejection {
            JsonRejection::MissingJsonContentType(_) => {
                (StatusCode::UNSUPPORTED_MEDIA_TYPE, "unsupported_media_type")
            }
            JsonRejection::JsonDataError(_) => (StatusCode::BAD_REQUEST, "validation_error"),
            JsonRejection::JsonSyntaxError(_) => (StatusCode::BAD_REQUEST, "malformed_json"),
            JsonRejection::BytesRejection(_) => (StatusCode::BAD_REQUEST, "malformed_json"),
            _ => (StatusCode::BAD_REQUEST, "validation_error"),
        };
        Self::new(status, code, rejection.body_text())
    }
}

impl From<QueryRejection> for ApiError {
    fn from(rejection: QueryRejection) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "validation_error",
            rejection.body_text(),
        )
    }
}

/// JSON body extractor that emits the universal [`ApiErrorResponse`] envelope on
/// any transport-level failure (wrong/missing `Content-Type`, malformed JSON,
/// unknown or missing fields) instead of Axum's bare `text/plain` rejection.
struct Json<T>(T);

impl<T, S> FromRequest<S> for Json<T>
where
    AxumJson<T>: FromRequest<S, Rejection = JsonRejection>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let AxumJson(value) = AxumJson::<T>::from_request(req, state).await?;
        Ok(Self(value))
    }
}

impl<T: Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> Response {
        AxumJson(self.0).into_response()
    }
}

/// Query-string extractor that maps [`QueryRejection`] into the universal
/// [`ApiErrorResponse`] envelope.
struct Query<T>(T);

impl<T, S> FromRequestParts<S> for Query<T>
where
    axum::extract::Query<T>: FromRequestParts<S, Rejection = QueryRejection>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let axum::extract::Query(value) =
            axum::extract::Query::<T>::from_request_parts(parts, state).await?;
        Ok(Self(value))
    }
}

async fn not_found() -> ApiError {
    ApiError::new(
        StatusCode::NOT_FOUND,
        "not_found",
        "No route matches the requested path. See GET /v1/openapi.json for the contract.",
    )
}

async fn method_not_allowed() -> ApiError {
    ApiError::new(
        StatusCode::METHOD_NOT_ALLOWED,
        "method_not_allowed",
        "The HTTP method is not allowed for this route. See GET /v1/openapi.json for the contract.",
    )
}

async fn root() -> impl IntoResponse {
    AxumJson(serde_json::json!({
        "service": "nahuali-api",
        "openapi": "/v1/openapi.json",
        "health": "/v1/health",
    }))
}

async fn health() -> impl IntoResponse {
    AxumJson(serde_json::json!({ "status": "ok" }))
}

/// Acquire an exclusive write guard over the cached engine, opening it lazily on
/// first use. Mutating endpoints use this so their `&mut MemoryEngine` calls run
/// against the shared cached engine instead of a fresh per-request open.
async fn write_engine(
    state: &ApiState,
) -> Result<RwLockMappedWriteGuard<'_, MemoryEngine>, ApiError> {
    let mut guard = state.engine.write().await;
    if guard.is_none() {
        let engine = MemoryEngine::open(state.database.as_ref()).map_err(ApiError::from)?;
        *guard = Some(engine);
    }
    Ok(RwLockWriteGuard::map(guard, |slot| {
        slot.as_mut().expect("engine initialized above")
    }))
}

/// Acquire a shared read guard over the cached engine, opening it lazily on first
/// use. Read endpoints use this so concurrent reads share one engine.
async fn read_engine(state: &ApiState) -> Result<RwLockReadGuard<'_, MemoryEngine>, ApiError> {
    {
        let guard = state.engine.read().await;
        if guard.is_some() {
            return Ok(RwLockReadGuard::map(guard, |slot| {
                slot.as_ref().expect("engine present under read guard")
            }));
        }
    }
    // Engine not yet opened: initialize it under the write lock, then downgrade to a
    // fresh read guard. A racing reader may have opened it first; that is fine because
    // `write_engine` is a no-op when the slot is already populated.
    write_engine(state).await?;
    let guard = state.engine.read().await;
    Ok(RwLockReadGuard::map(guard, |slot| {
        slot.as_ref().expect("engine initialized before read guard")
    }))
}

async fn openapi() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "application/json")], OPENAPI_JSON)
}

async fn status(State(state): State<ApiState>) -> ApiResult<StatusResponse> {
    let memory = read_engine(&state).await?;
    let data = memory.data();
    let health = memory.inspect();
    let authority = memory.authority();
    let projection = memory.projection_validate()?;

    Ok(Json(StatusResponse {
        database: state.database.display().to_string(),
        event_count: data.event_count,
        source_count: data.sources.len(),
        entity_count: data.entities.len(),
        episode_count: data.episodes.len(),
        claim_count: data.claims.len(),
        link_count: data.links.len(),
        procedure_count: data.procedures.len(),
        intention_count: data.intentions.len(),
        review_decision_count: data.review_decisions.len(),
        authority,
        health,
        surrealdb_graph_projection: projection,
        semantic_index_role: "derived",
    }))
}

async fn record_episode(
    State(state): State<ApiState>,
    Json(request): Json<EpisodeRequest>,
) -> ApiResult<Episode> {
    let mut memory = write_engine(&state).await?;
    let episode = match request.scope {
        Some(scope) => memory.remember_with_mentions_scoped(
            request.content,
            request.tags,
            request.mentions,
            scope,
        )?,
        None => memory.remember_with_mentions(request.content, request.tags, request.mentions)?,
    };
    Ok(Json(episode))
}

async fn record_claim(
    State(state): State<ApiState>,
    Json(request): Json<ClaimRequest>,
) -> ApiResult<Claim> {
    let mut memory = write_engine(&state).await?;
    let claim = match request.scope {
        Some(scope) => memory.add_claim_scoped(
            request.subject,
            request.predicate,
            request.object,
            request.source_episode_id,
            request.confidence,
            scope,
        )?,
        None => memory.add_claim(
            request.subject,
            request.predicate,
            request.object,
            request.source_episode_id,
            request.confidence,
        )?,
    };
    Ok(Json(claim))
}

async fn record_link(
    State(state): State<ApiState>,
    Json(request): Json<LinkRequest>,
) -> ApiResult<Link> {
    let mut memory = write_engine(&state).await?;
    let link = match request.scope {
        Some(scope) => memory.add_link_scoped(
            request.from,
            request.relation,
            request.to,
            request.source_episode_id,
            request.confidence,
            scope,
        )?,
        None => memory.add_link(
            request.from,
            request.relation,
            request.to,
            request.source_episode_id,
            request.confidence,
        )?,
    };
    Ok(Json(link))
}

async fn record_procedure(
    State(state): State<ApiState>,
    Json(request): Json<ProcedureRequest>,
) -> ApiResult<Procedure> {
    let mut memory = write_engine(&state).await?;
    let procedure = match request.scope {
        Some(scope) => memory.add_procedure_scoped(
            request.name,
            request.body,
            request.source_episode_id,
            request.confidence,
            scope,
        )?,
        None => memory.add_procedure(
            request.name,
            request.body,
            request.source_episode_id,
            request.confidence,
        )?,
    };
    Ok(Json(procedure))
}

async fn record_intention(
    State(state): State<ApiState>,
    Json(request): Json<IntentionRequest>,
) -> ApiResult<Intention> {
    let mut memory = write_engine(&state).await?;
    let intention = match request.scope {
        Some(scope) => memory.add_intention_scoped(
            request.description,
            request.kind,
            request.priority,
            request.source_episode_id,
            scope,
        )?,
        None => memory.add_intention(
            request.description,
            request.kind,
            request.priority,
            request.source_episode_id,
        )?,
    };
    Ok(Json(intention))
}

async fn update_intention(
    State(state): State<ApiState>,
    Json(request): Json<IntentionUpdateRequest>,
) -> ApiResult<Intention> {
    let mut memory = write_engine(&state).await?;
    Ok(Json(memory.update_intention(
        request.id,
        IntentionUpdateOptions {
            description: request.description,
            priority: request.priority,
            deadline_at_ms: request.deadline_at_ms,
            depends_on: request.depends_on,
            goal_id: request.goal_id,
            progress_percent: request.progress_percent,
        },
    )?))
}

async fn update_intention_status(
    State(state): State<ApiState>,
    Json(request): Json<IntentionStatusRequest>,
) -> ApiResult<Intention> {
    let mut memory = write_engine(&state).await?;
    Ok(Json(memory.set_intention_status(
        request.id,
        request.status,
        request.reason,
    )?))
}

async fn reconcile_intentions(
    State(state): State<ApiState>,
    Json(request): Json<IntentionReconcileRequest>,
) -> ApiResult<IntentionReconcileResponse> {
    let memory = read_engine(&state).await?;
    let mut options = IntentionReconciliationOptions::default();
    if let Some(now_ms) = request.now_ms {
        options.now_ms = now_ms;
    }
    if let Some(stale_after_ms) = request.stale_after_ms {
        options.stale_after_ms = stale_after_ms;
    }

    Ok(Json(IntentionReconcileResponse {
        database: state.database.display().to_string(),
        report: memory.reconcile_intentions_with_options(options),
    }))
}

async fn goal_progress(State(state): State<ApiState>) -> ApiResult<GoalProgressResponse> {
    let memory = read_engine(&state).await?;
    Ok(Json(GoalProgressResponse {
        database: state.database.display().to_string(),
        report: memory.goal_progress(),
    }))
}

async fn proactive(
    State(state): State<ApiState>,
    Json(request): Json<ProactiveRequest>,
) -> ApiResult<ProactiveResponse> {
    let memory = read_engine(&state).await?;
    Ok(Json(ProactiveResponse {
        database: state.database.display().to_string(),
        source_projection: "rust",
        report: memory.proactive_with_options(request.into()),
    }))
}

async fn deadlines(
    State(state): State<ApiState>,
    Json(request): Json<ProactiveRequest>,
) -> ApiResult<DeadlineResponse> {
    let memory = read_engine(&state).await?;
    Ok(Json(DeadlineResponse {
        database: state.database.display().to_string(),
        source_projection: "rust",
        report: memory.deadlines_with_options(request.into()),
    }))
}

async fn anomalies(
    State(state): State<ApiState>,
    Json(request): Json<ProactiveRequest>,
) -> ApiResult<AnomalyResponse> {
    let memory = read_engine(&state).await?;
    Ok(Json(AnomalyResponse {
        database: state.database.display().to_string(),
        source_projection: "rust",
        report: memory.anomalies_with_options(request.into()),
    }))
}

async fn acknowledge_anomaly(
    State(state): State<ApiState>,
    Json(request): Json<AnomalyAcknowledgeRequest>,
) -> ApiResult<AnomalyAcknowledgeResponse> {
    let mut memory = write_engine(&state).await?;
    Ok(Json(AnomalyAcknowledgeResponse {
        database: state.database.display().to_string(),
        report: memory.acknowledge_anomaly(request.anomaly_id, request.note, request.dry_run)?,
    }))
}

async fn recall(
    State(state): State<ApiState>,
    Json(request): Json<RecallRequest>,
) -> ApiResult<RecallResponse> {
    let memory = read_engine(&state).await?;
    let options = RecallOptions {
        limit: request.limit,
        scope: request.scope,
        kinds: request.kinds,
        require_evidence: request.require_evidence,
        ..RecallOptions::default()
    };

    let authority_recall = memory.recall_with_authority_options(&request.query, options.clone())?;
    let semantic = if request.semantic {
        Some(memory.hybrid_recall_with_options(&request.query, options)?)
    } else {
        None
    };

    Ok(Json(RecallResponse {
        query: request.query,
        lexical_results: authority_recall.results,
        authority: authority_recall.authority,
        health: authority_recall.health,
        semantic,
    }))
}

async fn session_resume(
    State(state): State<ApiState>,
    Json(request): Json<SessionResumeRequest>,
) -> ApiResult<SessionResumeResponse> {
    let memory = read_engine(&state).await?;
    let briefing = memory.briefing_with_options(request.into());
    let health = memory.inspect();
    let authority = AuthorityDecision::evaluate(&health);

    Ok(Json(SessionResumeResponse {
        database: state.database.display().to_string(),
        authority,
        health,
        briefing,
    }))
}

async fn memory_health(State(state): State<ApiState>) -> ApiResult<MemoryHealthResponse> {
    let memory = read_engine(&state).await?;
    let health = memory.inspect();
    let authority = AuthorityDecision::evaluate(&health);

    Ok(Json(MemoryHealthResponse { authority, health }))
}

async fn graph(
    State(state): State<ApiState>,
    Query(query): Query<GraphQuery>,
) -> ApiResult<MemoryGraphReport> {
    let memory = read_engine(&state).await?;
    Ok(Json(memory.graph_neighborhood(
        &query.seed,
        query.depth.unwrap_or(2),
        query.limit.unwrap_or(50),
    )?))
}

async fn audit(
    State(state): State<ApiState>,
    Query(query): Query<AuditQuery>,
) -> ApiResult<LedgerAudit> {
    let memory = read_engine(&state).await?;
    Ok(Json(memory.audit_ledger(&LedgerAuditOptions {
        from_sequence: query.from,
        to_sequence: query.to,
        since_ms: query.since,
        until_ms: query.until,
    })))
}

async fn trust_report(State(state): State<ApiState>) -> ApiResult<MemoryTrustReport> {
    let memory = read_engine(&state).await?;
    Ok(Json(memory.trust_report()))
}

async fn timeline(
    State(state): State<ApiState>,
    Query(query): Query<LimitQuery>,
) -> ApiResult<TimelineResponse> {
    let memory = read_engine(&state).await?;
    Ok(Json(TimelineResponse {
        database: state.database.display().to_string(),
        projection_role: "derived_from_memory_record",
        episodes: memory.projection_timeline(query.limit.unwrap_or(20))?,
    }))
}

async fn pending(
    State(state): State<ApiState>,
    Query(query): Query<LimitQuery>,
) -> ApiResult<PendingResponse> {
    let memory = read_engine(&state).await?;
    Ok(Json(PendingResponse {
        database: state.database.display().to_string(),
        projection_role: "derived_from_memory_record",
        intentions: memory.projection_pending(query.limit.unwrap_or(20))?,
    }))
}

async fn projection_status(State(state): State<ApiState>) -> ApiResult<ProjectionStatusResponse> {
    let memory = read_engine(&state).await?;
    Ok(Json(ProjectionStatusResponse {
        database: state.database.display().to_string(),
        projection_role: "derived_from_memory_record",
        status: memory.projection_status()?,
    }))
}

async fn projection_rebuild(State(state): State<ApiState>) -> ApiResult<ProjectionRebuildResponse> {
    let mut memory = write_engine(&state).await?;
    Ok(Json(ProjectionRebuildResponse {
        database: state.database.display().to_string(),
        projection_role: "derived_from_memory_record",
        report: memory.projection_rebuild()?,
    }))
}

async fn projection_validate(
    State(state): State<ApiState>,
) -> ApiResult<ProjectionValidateResponse> {
    let memory = read_engine(&state).await?;
    Ok(Json(ProjectionValidateResponse {
        database: state.database.display().to_string(),
        projection_role: "derived_from_memory_record",
        validation: memory.projection_validate()?,
    }))
}

async fn semantic_status(State(state): State<ApiState>) -> ApiResult<SemanticStatusResponse> {
    let memory = read_engine(&state).await?;
    Ok(Json(SemanticStatusResponse {
        database: state.database.display().to_string(),
        semantic_index_role: "derived",
        status: memory.semantic_index_status()?,
    }))
}

async fn semantic_rebuild(State(state): State<ApiState>) -> ApiResult<SemanticRebuildResponse> {
    let memory = read_engine(&state).await?;
    Ok(Json(SemanticRebuildResponse {
        database: state.database.display().to_string(),
        semantic_index_role: "derived",
        report: memory.rebuild_semantic_index()?,
    }))
}

async fn resolve_review(
    State(state): State<ApiState>,
    Json(request): Json<ReviewResolveRequest>,
) -> ApiResult<ReviewResolutionReport> {
    let mut memory = write_engine(&state).await?;
    Ok(Json(memory.resolve_review_item(
        request.review_id,
        request.note,
        request.dry_run,
    )?))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EpisodeRequest {
    content: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    mentions: Vec<String>,
    #[serde(default)]
    scope: Option<MemoryScope>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClaimRequest {
    subject: String,
    predicate: String,
    object: String,
    #[serde(default)]
    source_episode_id: Option<String>,
    #[serde(default = "default_confidence")]
    confidence: f32,
    #[serde(default)]
    scope: Option<MemoryScope>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LinkRequest {
    from: String,
    relation: String,
    to: String,
    #[serde(default)]
    source_episode_id: Option<String>,
    #[serde(default = "default_confidence")]
    confidence: f32,
    #[serde(default)]
    scope: Option<MemoryScope>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProcedureRequest {
    name: String,
    body: String,
    #[serde(default)]
    source_episode_id: Option<String>,
    #[serde(default = "default_confidence")]
    confidence: f32,
    #[serde(default)]
    scope: Option<MemoryScope>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IntentionRequest {
    description: String,
    kind: IntentionKind,
    priority: IntentionPriority,
    #[serde(default)]
    source_episode_id: Option<String>,
    #[serde(default)]
    scope: Option<MemoryScope>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IntentionUpdateRequest {
    id: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    priority: Option<IntentionPriority>,
    #[serde(default)]
    deadline_at_ms: Option<Option<u64>>,
    #[serde(default)]
    depends_on: Option<Vec<String>>,
    #[serde(default)]
    goal_id: Option<Option<String>>,
    #[serde(default)]
    progress_percent: Option<Option<u8>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IntentionStatusRequest {
    id: String,
    status: IntentionStatus,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IntentionReconcileRequest {
    #[serde(default)]
    now_ms: Option<u64>,
    #[serde(default)]
    stale_after_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProactiveRequest {
    #[serde(default)]
    now_ms: Option<u64>,
    #[serde(default)]
    deadline_horizon_ms: Option<u64>,
    #[serde(default)]
    stale_after_ms: Option<u64>,
    #[serde(default)]
    review_limit: Option<usize>,
}

impl From<ProactiveRequest> for ProactiveOptions {
    fn from(request: ProactiveRequest) -> Self {
        let mut options = ProactiveOptions::default();
        if let Some(now_ms) = request.now_ms {
            options.now_ms = now_ms;
        }
        if let Some(deadline_horizon_ms) = request.deadline_horizon_ms {
            options.deadline_horizon_ms = deadline_horizon_ms;
        }
        if let Some(stale_after_ms) = request.stale_after_ms {
            options.stale_after_ms = stale_after_ms;
        }
        if let Some(review_limit) = request.review_limit {
            options.review_limit = review_limit;
        }
        options
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AnomalyAcknowledgeRequest {
    anomaly_id: String,
    note: String,
    #[serde(default)]
    dry_run: bool,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecallRequest {
    query: String,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    scope: Option<MemoryScope>,
    #[serde(default)]
    kinds: Vec<MemoryKind>,
    #[serde(default)]
    require_evidence: bool,
    #[serde(default)]
    semantic: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionResumeRequest {
    #[serde(default = "default_episode_limit")]
    episode_limit: usize,
    #[serde(default = "default_intention_limit")]
    intention_limit: usize,
    #[serde(default = "default_review_limit")]
    review_limit: usize,
    #[serde(default = "default_graph_seed_limit")]
    graph_seed_limit: usize,
}

impl From<SessionResumeRequest> for nahuali_core::BriefingOptions {
    fn from(request: SessionResumeRequest) -> Self {
        Self {
            episode_limit: request.episode_limit,
            intention_limit: request.intention_limit,
            review_limit: request.review_limit,
            graph_seed_limit: request.graph_seed_limit,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GraphQuery {
    seed: String,
    #[serde(default)]
    depth: Option<usize>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuditQuery {
    #[serde(default)]
    from: Option<u64>,
    #[serde(default)]
    to: Option<u64>,
    #[serde(default)]
    since: Option<u64>,
    #[serde(default)]
    until: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LimitQuery {
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewResolveRequest {
    review_id: String,
    note: String,
    #[serde(default)]
    dry_run: bool,
}

#[derive(Serialize)]
struct StatusResponse {
    database: String,
    event_count: usize,
    source_count: usize,
    entity_count: usize,
    episode_count: usize,
    claim_count: usize,
    link_count: usize,
    procedure_count: usize,
    intention_count: usize,
    review_decision_count: usize,
    authority: AuthorityDecision,
    health: KnowledgeHealth,
    surrealdb_graph_projection: GraphProjectionValidation,
    semantic_index_role: &'static str,
}

#[derive(Serialize)]
struct RecallResponse {
    query: String,
    lexical_results: Vec<nahuali_core::RecallResult>,
    authority: AuthorityDecision,
    health: KnowledgeHealth,
    #[serde(skip_serializing_if = "Option::is_none")]
    semantic: Option<HybridRecallReport>,
}

#[derive(Serialize)]
struct SessionResumeResponse {
    database: String,
    authority: AuthorityDecision,
    health: KnowledgeHealth,
    briefing: MemoryBriefingReport,
}

#[derive(Serialize)]
struct MemoryHealthResponse {
    authority: AuthorityDecision,
    health: KnowledgeHealth,
}

#[derive(Serialize)]
struct IntentionReconcileResponse {
    database: String,
    report: IntentionReconciliationReport,
}

#[derive(Serialize)]
struct GoalProgressResponse {
    database: String,
    report: GoalProgressReport,
}

#[derive(Serialize)]
struct ProactiveResponse {
    database: String,
    source_projection: &'static str,
    report: MemoryProactiveReport,
}

#[derive(Serialize)]
struct DeadlineResponse {
    database: String,
    source_projection: &'static str,
    report: DeadlineReport,
}

#[derive(Serialize)]
struct AnomalyResponse {
    database: String,
    source_projection: &'static str,
    report: AnomalyReport,
}

#[derive(Serialize)]
struct AnomalyAcknowledgeResponse {
    database: String,
    report: AnomalyAcknowledgementReport,
}

#[derive(Serialize)]
struct TimelineResponse {
    database: String,
    projection_role: &'static str,
    episodes: Vec<GraphProjectionEpisode>,
}

#[derive(Serialize)]
struct PendingResponse {
    database: String,
    projection_role: &'static str,
    intentions: Vec<GraphProjectionPendingIntention>,
}

#[derive(Serialize)]
struct ProjectionStatusResponse {
    database: String,
    projection_role: &'static str,
    status: GraphProjectionStatus,
}

#[derive(Serialize)]
struct ProjectionRebuildResponse {
    database: String,
    projection_role: &'static str,
    report: nahuali_core::GraphProjectionRebuildReport,
}

#[derive(Serialize)]
struct ProjectionValidateResponse {
    database: String,
    projection_role: &'static str,
    validation: GraphProjectionValidation,
}

#[derive(Serialize)]
struct SemanticStatusResponse {
    database: String,
    semantic_index_role: &'static str,
    status: SemanticIndexStatus,
}

#[derive(Serialize)]
struct SemanticRebuildResponse {
    database: String,
    semantic_index_role: &'static str,
    report: SemanticIndexReport,
}

fn default_confidence() -> f32 {
    1.0
}

fn default_limit() -> usize {
    10
}

fn default_episode_limit() -> usize {
    5
}

fn default_intention_limit() -> usize {
    5
}

fn default_review_limit() -> usize {
    5
}

fn default_graph_seed_limit() -> usize {
    8
}
