use nahuali_core::{
    BriefingOptions, ConsolidationPlanOptions, DEFAULT_TEXT_CHUNK_BYTES, IntentionKind,
    IntentionPriority, IntentionReconciliationOptions, IntentionStatus, IntentionUpdateOptions,
    MemoryEngine, MemoryHookOptions, MemoryIngestDocument, OperatorReviewOptions, ProactiveOptions,
    RecallOptions, ReflectionOptions, SelfInspectionReviewAction, SelfInspectionReviewPriority,
    SleepModeOptions, SourceKind, TextChunking, TextIngestOptions, build_text_ingest_document,
};
use rmcp::{
    Json,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    tool, tool_router,
};

use crate::{
    protocol::{
        AnomalyAcknowledgeArgs, AuthorityDecisionView, BriefingArgs, BriefingResult, ClaimResult,
        ClaimView, ConsolidationPlanArgs, ConsolidationPlanResult, DatabaseReportResult, FactArgs,
        FactResult, FactView, GraphArgs, GraphResult, IngestArgs, IngestResult, IngestTextArgs,
        IngestTextResult, InspectResult, IntentionArgs, IntentionKindArg, IntentionPriorityArg,
        IntentionReconcileArgs, IntentionResult, IntentionStatusArgs, IntentionUpdateArgs,
        IntentionView, LinkResult, LinkView, MemoryHookArgs, MemoryHookResult,
        OperatorReportResult, ProactiveArgs, ProcedureArgs, ProcedureResult, ProcedureView,
        ProjectionReportResult, ProjectionStatusResult, ProjectionValidationResult, RecallArgs,
        RecallResultView, RecallToolResult, RecordLedgerIssueView, ReflectArgs, ReflectResult,
        RelateArgs, RelateResult, RelationView, RememberArgs, RememberResult, ReviewArgs,
        ReviewResolveArgs, ReviewResolveResult, ReviewResult, SelfInspectResult,
        SemanticReportResult, SemanticStatusResult, SourceKindArg, TextChunkingArg, ValidateResult,
        parse_scope_arg,
    },
    server::NahualiMcpServer,
};

#[tool_router]
impl NahualiMcpServer {
    #[tool(description = "Store an episode as append-only memory ground truth.")]
    fn remember(
        &self,
        Parameters(args): Parameters<RememberArgs>,
    ) -> Result<Json<RememberResult>, String> {
        let episode = self.with_memory(|memory| {
            let scope = parse_scope_arg(args.scope)?;
            if let Some(scope) = scope {
                memory.remember_with_mentions_scoped(
                    args.content,
                    args.tags.unwrap_or_default(),
                    args.mentions.unwrap_or_default(),
                    scope,
                )
            } else {
                memory.remember_with_mentions(
                    args.content,
                    args.tags.unwrap_or_default(),
                    args.mentions.unwrap_or_default(),
                )
            }
            .map_err(|error| error.to_string())
        })?;
        Ok(Json(RememberResult {
            episode: episode.into(),
        }))
    }

    #[tool(description = "Ingest a provenance-aware source document.")]
    fn ingest(
        &self,
        Parameters(args): Parameters<IngestArgs>,
    ) -> Result<Json<IngestResult>, String> {
        let document: MemoryIngestDocument = serde_json::from_value(args.document)
            .map_err(|error| format!("invalid ingestion document: {error}"))?;
        let report = self.with_memory(|memory| {
            memory
                .ingest_document(&document, args.dry_run.unwrap_or(false))
                .map_err(|error| error.to_string())
        })?;
        let report = serde_json::to_value(report).map_err(|error| error.to_string())?;
        Ok(Json(IngestResult { report }))
    }

    #[tool(description = "Ingest direct text as provenance-preserving source episodes.")]
    fn ingest_text(
        &self,
        Parameters(args): Parameters<IngestTextArgs>,
    ) -> Result<Json<IngestTextResult>, String> {
        let adapter_report = build_text_ingest_document(
            &args.content,
            TextIngestOptions {
                source_kind: SourceKind::from(args.kind.unwrap_or(SourceKindArg::Document)),
                title: args.title,
                uri: args.uri,
                metadata: args.metadata.unwrap_or_default(),
                scope: parse_scope_arg(args.scope)?,
                tags: args.tags.unwrap_or_default(),
                mentions: args.mentions.unwrap_or_default(),
                source_role: args.source_role,
                chunking: TextChunking::from(args.chunking.unwrap_or(TextChunkingArg::Document)),
                max_chunk_bytes: args.max_chunk_bytes.unwrap_or(DEFAULT_TEXT_CHUNK_BYTES),
            },
        );
        let report = if let Some(document) = &adapter_report.document {
            let report = self.with_memory(|memory| {
                memory
                    .ingest_document(document, args.dry_run.unwrap_or(false))
                    .map_err(|error| error.to_string())
            })?;
            Some(serde_json::to_value(report).map_err(|error| error.to_string())?)
        } else {
            None
        };
        let adapter_report =
            serde_json::to_value(adapter_report).map_err(|error| error.to_string())?;

        Ok(Json(IngestTextResult {
            adapter_report,
            report,
        }))
    }

    #[tool(description = "Assert a claim, optionally linked to a source episode.")]
    fn claim(&self, Parameters(args): Parameters<FactArgs>) -> Result<Json<ClaimResult>, String> {
        let claim = self.with_memory(|memory| {
            let source_episode_id = Self::resolve_source_episode_id(
                memory,
                args.source_episode_id,
                args.source_last.unwrap_or(false),
            )?;
            let scope = parse_scope_arg(args.scope)?;

            if let Some(scope) = scope {
                memory.add_claim_scoped(
                    args.subject,
                    args.predicate,
                    args.object,
                    source_episode_id,
                    args.confidence.unwrap_or(0.8),
                    scope,
                )
            } else {
                memory.add_claim(
                    args.subject,
                    args.predicate,
                    args.object,
                    source_episode_id,
                    args.confidence.unwrap_or(0.8),
                )
            }
            .map_err(|error| error.to_string())
        })?;

        Ok(Json(ClaimResult {
            claim: ClaimView::from(claim),
        }))
    }

    #[tool(description = "Assert a fact, optionally linked to a source episode.")]
    fn fact(&self, Parameters(args): Parameters<FactArgs>) -> Result<Json<FactResult>, String> {
        let fact = self.with_memory(|memory| {
            let source_episode_id = Self::resolve_source_episode_id(
                memory,
                args.source_episode_id,
                args.source_last.unwrap_or(false),
            )?;
            let scope = parse_scope_arg(args.scope)?;

            if let Some(scope) = scope {
                memory.add_fact_scoped(
                    args.subject,
                    args.predicate,
                    args.object,
                    source_episode_id,
                    args.confidence.unwrap_or(0.8),
                    scope,
                )
            } else {
                memory.add_fact(
                    args.subject,
                    args.predicate,
                    args.object,
                    source_episode_id,
                    args.confidence.unwrap_or(0.8),
                )
            }
            .map_err(|error| error.to_string())
        })?;

        Ok(Json(FactResult {
            fact: FactView::from(fact),
        }))
    }

    #[tool(description = "Record a link, optionally linked to a source episode.")]
    fn link(&self, Parameters(args): Parameters<RelateArgs>) -> Result<Json<LinkResult>, String> {
        let link = self.with_memory(|memory| {
            let source_episode_id = Self::resolve_source_episode_id(
                memory,
                args.source_episode_id,
                args.source_last.unwrap_or(false),
            )?;
            let scope = parse_scope_arg(args.scope)?;

            if let Some(scope) = scope {
                memory.add_link_scoped(
                    args.from,
                    args.relation,
                    args.to,
                    source_episode_id,
                    args.confidence.unwrap_or(0.8),
                    scope,
                )
            } else {
                memory.add_link(
                    args.from,
                    args.relation,
                    args.to,
                    source_episode_id,
                    args.confidence.unwrap_or(0.8),
                )
            }
            .map_err(|error| error.to_string())
        })?;

        Ok(Json(LinkResult {
            link: LinkView::from(link),
        }))
    }

    #[tool(description = "Record a relation, optionally linked to a source episode.")]
    fn relate(
        &self,
        Parameters(args): Parameters<RelateArgs>,
    ) -> Result<Json<RelateResult>, String> {
        let relation = self.with_memory(|memory| {
            let source_episode_id = Self::resolve_source_episode_id(
                memory,
                args.source_episode_id,
                args.source_last.unwrap_or(false),
            )?;
            let scope = parse_scope_arg(args.scope)?;

            if let Some(scope) = scope {
                memory.relate_scoped(
                    args.from,
                    args.relation,
                    args.to,
                    source_episode_id,
                    args.confidence.unwrap_or(0.8),
                    scope,
                )
            } else {
                memory.relate(
                    args.from,
                    args.relation,
                    args.to,
                    source_episode_id,
                    args.confidence.unwrap_or(0.8),
                )
            }
            .map_err(|error| error.to_string())
        })?;

        Ok(Json(RelateResult {
            relation: RelationView::from(relation),
        }))
    }

    #[tool(description = "Record a procedure, optionally linked to a source episode.")]
    fn procedure(
        &self,
        Parameters(args): Parameters<ProcedureArgs>,
    ) -> Result<Json<ProcedureResult>, String> {
        let procedure = self.with_memory(|memory| {
            let source_episode_id = Self::resolve_source_episode_id(
                memory,
                args.source_episode_id,
                args.source_last.unwrap_or(false),
            )?;
            let scope = parse_scope_arg(args.scope)?;

            if let Some(scope) = scope {
                memory.add_procedure_scoped(
                    args.name,
                    args.body,
                    source_episode_id,
                    args.confidence.unwrap_or(0.8),
                    scope,
                )
            } else {
                memory.add_procedure(
                    args.name,
                    args.body,
                    source_episode_id,
                    args.confidence.unwrap_or(0.8),
                )
            }
            .map_err(|error| error.to_string())
        })?;

        Ok(Json(ProcedureResult {
            procedure: ProcedureView::from(procedure),
        }))
    }

    #[tool(description = "Record a preference, optionally linked to a source episode.")]
    fn preference(
        &self,
        Parameters(args): Parameters<ProcedureArgs>,
    ) -> Result<Json<ProcedureResult>, String> {
        let preference = self.with_memory(|memory| {
            let source_episode_id = Self::resolve_source_episode_id(
                memory,
                args.source_episode_id,
                args.source_last.unwrap_or(false),
            )?;
            let scope = parse_scope_arg(args.scope)?;

            if let Some(scope) = scope {
                memory.add_preference_scoped(
                    args.name,
                    args.body,
                    source_episode_id,
                    args.confidence.unwrap_or(0.8),
                    scope,
                )
            } else {
                memory.add_preference(
                    args.name,
                    args.body,
                    source_episode_id,
                    args.confidence.unwrap_or(0.8),
                )
            }
            .map_err(|error| error.to_string())
        })?;

        Ok(Json(ProcedureResult {
            procedure: ProcedureView::from(preference),
        }))
    }

    #[tool(description = "Record future work, a goal, reminder, or commitment.")]
    fn intention(
        &self,
        Parameters(args): Parameters<IntentionArgs>,
    ) -> Result<Json<IntentionResult>, String> {
        let intention = self.with_memory(|memory| {
            let source_episode_id = Self::resolve_source_episode_id(
                memory,
                args.source_episode_id,
                args.source_last.unwrap_or(false),
            )?;
            let scope = parse_scope_arg(args.scope)?;

            if let Some(scope) = scope {
                memory.add_intention_scoped(
                    args.description,
                    IntentionKind::from(args.kind.unwrap_or(IntentionKindArg::Task)),
                    IntentionPriority::from(args.priority.unwrap_or(IntentionPriorityArg::Medium)),
                    source_episode_id,
                    scope,
                )
            } else {
                memory.add_intention(
                    args.description,
                    IntentionKind::from(args.kind.unwrap_or(IntentionKindArg::Task)),
                    IntentionPriority::from(args.priority.unwrap_or(IntentionPriorityArg::Medium)),
                    source_episode_id,
                )
            }
            .map_err(|error| error.to_string())
        })?;

        Ok(Json(IntentionResult {
            intention: IntentionView::from(intention),
        }))
    }

    #[tool(description = "Change an intention lifecycle state.")]
    fn intention_status(
        &self,
        Parameters(args): Parameters<IntentionStatusArgs>,
    ) -> Result<Json<IntentionResult>, String> {
        let intention = self.with_memory(|memory| {
            memory
                .set_intention_status(args.id, IntentionStatus::from(args.status), args.reason)
                .map_err(|error| error.to_string())
        })?;

        Ok(Json(IntentionResult {
            intention: IntentionView::from(intention),
        }))
    }

    #[tool(description = "Update intention metadata without changing lifecycle status.")]
    fn intention_update(
        &self,
        Parameters(args): Parameters<IntentionUpdateArgs>,
    ) -> Result<Json<IntentionResult>, String> {
        let intention = self.with_memory(|memory| {
            memory
                .update_intention(
                    args.id,
                    IntentionUpdateOptions {
                        description: args.description,
                        priority: args.priority.map(IntentionPriority::from),
                        deadline_at_ms: args.deadline_at_ms,
                        depends_on: args.depends_on,
                        goal_id: args.goal_id,
                        progress_percent: args.progress_percent,
                    },
                )
                .map_err(|error| error.to_string())
        })?;

        Ok(Json(IntentionResult {
            intention: IntentionView::from(intention),
        }))
    }

    #[tool(description = "Produce a non-mutating intention reconciliation report.")]
    fn reconcile_intentions(
        &self,
        Parameters(args): Parameters<IntentionReconcileArgs>,
    ) -> Result<Json<DatabaseReportResult>, String> {
        let mut options = IntentionReconciliationOptions::default();
        if let Some(now_ms) = args.now_ms {
            options.now_ms = now_ms;
        }
        if let Some(stale_after_ms) = args.stale_after_ms {
            options.stale_after_ms = stale_after_ms;
        }

        let report =
            self.with_memory(|memory| Ok(memory.reconcile_intentions_with_options(options)))?;
        let report = serde_json::to_value(report).map_err(|error| error.to_string())?;
        Ok(Json(DatabaseReportResult {
            database: self.database.display().to_string(),
            report,
        }))
    }

    #[tool(description = "Produce a non-mutating goal progress report.")]
    fn goal_progress(&self) -> Result<Json<DatabaseReportResult>, String> {
        let report = self.with_memory(|memory| Ok(memory.goal_progress()))?;
        let report = serde_json::to_value(report).map_err(|error| error.to_string())?;
        Ok(Json(DatabaseReportResult {
            database: self.database.display().to_string(),
            report,
        }))
    }

    #[tool(description = "Produce a non-mutating proactive operator report.")]
    fn proactive(
        &self,
        Parameters(args): Parameters<ProactiveArgs>,
    ) -> Result<Json<OperatorReportResult>, String> {
        let report =
            self.with_memory(|memory| Ok(memory.proactive_with_options(proactive_options(args))))?;
        let report = serde_json::to_value(report).map_err(|error| error.to_string())?;
        Ok(Json(OperatorReportResult {
            database: self.database.display().to_string(),
            source_projection: "rust",
            report,
        }))
    }

    #[tool(description = "Produce non-mutating proactive deadline signals.")]
    fn deadlines(
        &self,
        Parameters(args): Parameters<ProactiveArgs>,
    ) -> Result<Json<OperatorReportResult>, String> {
        let report =
            self.with_memory(|memory| Ok(memory.deadlines_with_options(proactive_options(args))))?;
        let report = serde_json::to_value(report).map_err(|error| error.to_string())?;
        Ok(Json(OperatorReportResult {
            database: self.database.display().to_string(),
            source_projection: "rust",
            report,
        }))
    }

    #[tool(description = "Produce non-mutating proactive anomaly alerts.")]
    fn anomalies(
        &self,
        Parameters(args): Parameters<ProactiveArgs>,
    ) -> Result<Json<OperatorReportResult>, String> {
        let report =
            self.with_memory(|memory| Ok(memory.anomalies_with_options(proactive_options(args))))?;
        let report = serde_json::to_value(report).map_err(|error| error.to_string())?;
        Ok(Json(OperatorReportResult {
            database: self.database.display().to_string(),
            source_projection: "rust",
            report,
        }))
    }

    #[tool(description = "Acknowledge a proactive anomaly with an explicit audit note.")]
    fn anomaly_acknowledge(
        &self,
        Parameters(args): Parameters<AnomalyAcknowledgeArgs>,
    ) -> Result<Json<DatabaseReportResult>, String> {
        let report = self.with_memory(|memory| {
            memory
                .acknowledge_anomaly(args.anomaly_id, args.note, args.dry_run.unwrap_or(false))
                .map_err(|error| error.to_string())
        })?;
        let report = serde_json::to_value(report).map_err(|error| error.to_string())?;
        Ok(Json(DatabaseReportResult {
            database: self.database.display().to_string(),
            report,
        }))
    }

    #[tool(description = "Produce a compact non-mutating session briefing.")]
    fn briefing(
        &self,
        Parameters(args): Parameters<BriefingArgs>,
    ) -> Result<Json<BriefingResult>, String> {
        let report = self.with_memory(|memory| {
            Ok(memory.briefing_with_options(BriefingOptions {
                episode_limit: args.episode_limit.unwrap_or(5),
                intention_limit: args.intention_limit.unwrap_or(5),
                review_limit: args.review_limit.unwrap_or(5),
                graph_seed_limit: args.graph_seed_limit.unwrap_or(8),
            }))
        })?;
        let report = serde_json::to_value(report).map_err(|error| error.to_string())?;
        Ok(Json(BriefingResult { report }))
    }

    #[tool(description = "Run a non-mutating memory hook for a host execution point.")]
    fn memory_hook(
        &self,
        Parameters(args): Parameters<MemoryHookArgs>,
    ) -> Result<Json<MemoryHookResult>, String> {
        let report = self.with_memory(|memory| {
            memory
                .run_hook_with_options(MemoryHookOptions {
                    kind: args.kind.into(),
                    input: args.input,
                    recall_limit: args.recall_limit.unwrap_or(10),
                    briefing: BriefingOptions {
                        episode_limit: args.episode_limit.unwrap_or(5),
                        intention_limit: args.intention_limit.unwrap_or(5),
                        review_limit: args.review_limit.unwrap_or(5),
                        graph_seed_limit: args.graph_seed_limit.unwrap_or(8),
                    },
                    reflection: ReflectionOptions {
                        cycle_limit: args.cycle_limit.unwrap_or(8),
                        evidence_limit: args.evidence_limit.unwrap_or(8),
                    },
                    sleep: SleepModeOptions {
                        recent_episode_limit: args.episode_limit.unwrap_or(5),
                        candidate_limit: args.cycle_limit.unwrap_or(8),
                        reflection: ReflectionOptions {
                            cycle_limit: args.cycle_limit.unwrap_or(8),
                            evidence_limit: args.evidence_limit.unwrap_or(8),
                        },
                    },
                })
                .map_err(|error| error.to_string())
        })?;
        let report = serde_json::to_value(report).map_err(|error| error.to_string())?;
        Ok(Json(MemoryHookResult { report }))
    }

    #[tool(
        description = "Retrieve memory with transparent scoring, evidence, result trust, authority, and health."
    )]
    fn recall(
        &self,
        Parameters(args): Parameters<RecallArgs>,
    ) -> Result<Json<RecallToolResult>, String> {
        let recall = self.with_memory(|memory| {
            let scope = parse_scope_arg(args.scope)?;
            let options = RecallOptions {
                limit: args.limit.unwrap_or(10),
                scope,
                kinds: args
                    .kinds
                    .unwrap_or_default()
                    .into_iter()
                    .map(Into::into)
                    .collect(),
                require_evidence: args.require_evidence.unwrap_or(false),
            };
            memory
                .recall_with_authority_options(&args.query, options)
                .map_err(|error| error.to_string())
        })?;
        Ok(Json(RecallToolResult {
            results: recall
                .results
                .into_iter()
                .map(RecallResultView::from)
                .collect(),
            authority: AuthorityDecisionView::from(recall.authority),
            health: recall.health.into(),
        }))
    }

    #[tool(description = "Traverse the projected memory graph around a seed.")]
    fn graph(&self, Parameters(args): Parameters<GraphArgs>) -> Result<Json<GraphResult>, String> {
        let report = self.with_memory(|memory| {
            memory
                .graph_neighborhood(
                    &args.seed,
                    args.depth.unwrap_or(2),
                    args.limit.unwrap_or(100),
                )
                .map_err(|error| error.to_string())
        })?;
        let report = serde_json::to_value(report).map_err(|error| error.to_string())?;
        Ok(Json(GraphResult { report }))
    }

    #[tool(description = "Inspect knowledge health before trusting recall.")]
    fn inspect(&self) -> Result<Json<InspectResult>, String> {
        let health = self.with_memory(|memory| Ok(memory.inspect()))?;
        Ok(Json(InspectResult {
            health: health.into(),
        }))
    }

    #[tool(description = "Produce a non-mutating self-inspection consolidation report.")]
    fn self_inspect(&self) -> Result<Json<SelfInspectResult>, String> {
        let report = self.with_memory(|memory| Ok(memory.self_inspect()))?;
        let report = serde_json::to_value(report).map_err(|error| error.to_string())?;
        Ok(Json(SelfInspectResult { report }))
    }

    #[tool(description = "Plan a non-mutating reflection cycle for operator approval.")]
    fn reflect(
        &self,
        Parameters(args): Parameters<ReflectArgs>,
    ) -> Result<Json<ReflectResult>, String> {
        let report = self.with_memory(|memory| {
            Ok(memory.reflect_with_options(ReflectionOptions {
                cycle_limit: args.cycle_limit.unwrap_or(8),
                evidence_limit: args.evidence_limit.unwrap_or(8),
            }))
        })?;
        let report = serde_json::to_value(report).map_err(|error| error.to_string())?;
        Ok(Json(ReflectResult { report }))
    }

    #[tool(description = "Plan non-mutating replay, review gates, and write-back eligibility.")]
    fn consolidation_plan(
        &self,
        Parameters(args): Parameters<ConsolidationPlanArgs>,
    ) -> Result<Json<ConsolidationPlanResult>, String> {
        let report = self.with_memory(|memory| {
            Ok(
                memory.consolidation_plan_with_options(ConsolidationPlanOptions {
                    recent_episode_limit: args.episode_limit.unwrap_or(8),
                    candidate_limit: args.candidate_limit.unwrap_or(12),
                    cycle_limit: args.cycle_limit.unwrap_or(8),
                    evidence_limit: args.evidence_limit.unwrap_or(8),
                    review_limit: args.review_limit.unwrap_or(20),
                }),
            )
        })?;
        let report = serde_json::to_value(report).map_err(|error| error.to_string())?;
        Ok(Json(ConsolidationPlanResult { report }))
    }

    #[tool(description = "Produce a prioritized non-mutating operator review queue.")]
    fn review(
        &self,
        Parameters(args): Parameters<ReviewArgs>,
    ) -> Result<Json<ReviewResult>, String> {
        let report = self.with_memory(|memory| {
            Ok(memory.operator_review_with_options(OperatorReviewOptions {
                limit: args.limit.unwrap_or(20),
                min_priority: args.min_priority.map(SelfInspectionReviewPriority::from),
                action: args.action.map(SelfInspectionReviewAction::from),
            }))
        })?;
        let report = serde_json::to_value(report).map_err(|error| error.to_string())?;
        Ok(Json(ReviewResult { report }))
    }

    #[tool(description = "Resolve an operator review item with an explicit audit note.")]
    fn review_resolve(
        &self,
        Parameters(args): Parameters<ReviewResolveArgs>,
    ) -> Result<Json<ReviewResolveResult>, String> {
        let report = self.with_memory(|memory| {
            memory
                .resolve_review_item(args.review_id, args.note, args.dry_run.unwrap_or(false))
                .map_err(|error| error.to_string())
        })?;
        let report = serde_json::to_value(report).map_err(|error| error.to_string())?;
        Ok(Json(ReviewResolveResult { report }))
    }

    #[tool(description = "Return derived SurrealDB graph-projection status.")]
    fn projection_status(&self) -> Result<Json<ProjectionStatusResult>, String> {
        let status = self.with_memory(|memory| {
            memory
                .projection_status()
                .map_err(|error| error.to_string())
        })?;
        let status = serde_json::to_value(status).map_err(|error| error.to_string())?;
        Ok(Json(ProjectionStatusResult {
            database: self.database.display().to_string(),
            projection_role: "derived_from_memory_record",
            status,
        }))
    }

    #[tool(description = "Rebuild derived SurrealDB graph projection from the record ledger.")]
    fn projection_rebuild(&self) -> Result<Json<ProjectionReportResult>, String> {
        let report = self.with_memory(|memory| {
            memory
                .projection_rebuild()
                .map_err(|error| error.to_string())
        })?;
        let report = serde_json::to_value(report).map_err(|error| error.to_string())?;
        Ok(Json(ProjectionReportResult {
            database: self.database.display().to_string(),
            projection_role: "derived_from_memory_record",
            report,
        }))
    }

    #[tool(description = "Validate derived SurrealDB graph projection against the record ledger.")]
    fn projection_validate(&self) -> Result<Json<ProjectionValidationResult>, String> {
        let validation = self.with_memory(|memory| {
            memory
                .projection_validate()
                .map_err(|error| error.to_string())
        })?;
        let validation = serde_json::to_value(validation).map_err(|error| error.to_string())?;
        Ok(Json(ProjectionValidationResult {
            database: self.database.display().to_string(),
            projection_role: "derived_from_memory_record",
            validation,
        }))
    }

    #[tool(description = "Return Qdrant derived semantic-index status.")]
    fn semantic_status(&self) -> Result<Json<SemanticStatusResult>, String> {
        let status = self.with_memory(|memory| {
            memory
                .semantic_index_status()
                .map_err(|error| error.to_string())
        })?;
        let status = serde_json::to_value(status).map_err(|error| error.to_string())?;
        Ok(Json(SemanticStatusResult {
            database: self.database.display().to_string(),
            semantic_index_role: "derived",
            status,
        }))
    }

    #[tool(description = "Rebuild Qdrant semantic index from the projected memory state.")]
    fn semantic_rebuild(&self) -> Result<Json<SemanticReportResult>, String> {
        let report = self.with_memory(|memory| {
            memory
                .rebuild_semantic_index()
                .map_err(|error| error.to_string())
        })?;
        let report = serde_json::to_value(report).map_err(|error| error.to_string())?;
        Ok(Json(SemanticReportResult {
            database: self.database.display().to_string(),
            semantic_index_role: "derived",
            report,
        }))
    }

    #[tool(description = "Validate the SurrealDB memory_record ledger.")]
    fn validate(&self) -> Result<Json<ValidateResult>, String> {
        let validation =
            MemoryEngine::validate_store(&self.database).map_err(|error| error.to_string())?;
        let validation = self.with_memory(|memory| {
            let data = memory.data();
            Ok(ValidateResult {
                valid: validation.valid,
                event_count: validation.event_count,
                source_count: data.sources.len(),
                entity_count: data.entities.len(),
                episode_count: data.episodes.len(),
                claim_count: data.claims.len(),
                link_count: data.links.len(),
                fact_count: data.facts.len(),
                relation_count: data.relations.len(),
                procedure_count: data.procedures.len(),
                intention_count: data.intentions.len(),
                review_decision_count: data.review_decisions.len(),
                last_event_id: data.last_event_id.clone(),
                supported_event_version: validation.supported_event_version,
                observed_event_versions: validation.observed_event_versions,
                legacy_event_count: validation.legacy_event_count,
                migration_required: validation.migration_required,
                issues: validation
                    .issues
                    .into_iter()
                    .map(RecordLedgerIssueView::from)
                    .collect(),
            })
        })?;
        Ok(Json(validation))
    }
}

fn proactive_options(args: ProactiveArgs) -> ProactiveOptions {
    let mut options = ProactiveOptions::default();
    if let Some(now_ms) = args.now_ms {
        options.now_ms = now_ms;
    }
    if let Some(deadline_horizon_ms) = args.deadline_horizon_ms {
        options.deadline_horizon_ms = deadline_horizon_ms;
    }
    if let Some(stale_after_ms) = args.stale_after_ms {
        options.stale_after_ms = stale_after_ms;
    }
    if let Some(review_limit) = args.review_limit {
        options.review_limit = review_limit;
    }
    options
}

pub(crate) fn tool_router() -> ToolRouter<NahualiMcpServer> {
    NahualiMcpServer::tool_router()
}
