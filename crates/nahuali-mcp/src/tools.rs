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
    #[tool(
        description = "Use when you observe something worth remembering (a decision, event, or fact stated by the user) and want it captured verbatim as append-only ground truth. Episodes are the evidence other memory cites, so record them before asserting any claim or link. Then derive claims/links with `sourceLast: true` to cite this episode.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
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

    #[tool(
        description = "Use when importing an external source document (the structured interchange format) and you need its provenance preserved as it becomes episodes. Set `dryRun: true` first to preflight scope, size, and evidence-gap counts without writing. Then re-run without `dryRun` to append the records.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
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

    #[tool(
        description = "Use when you have raw UTF-8 text (notes, a transcript, a web page) to capture as source episodes without building the full interchange document. It chunks and preserves provenance but does not infer claims or links. Set `dryRun: true` to preflight; then re-run to append, and derive claims/links separately if needed.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
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

    #[tool(
        description = "Use when an episode explicitly supports a subject-predicate-object assertion you want recallable, after calling `remember` (pass `sourceLast: true` to cite it). A claim is the canonical evidence-backed assertion; `fact` is a deprecated alias of this tool. Then run `inspect` to confirm the claim is supported, not orphaned.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
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

    #[tool(
        description = "Deprecated alias of `claim`; prefer that. Kept for backward compatibility while clients migrate to `claim`.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
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

    #[tool(
        description = "Use when an episode explicitly supports a typed connection between two entities (from-relation-to) you want recallable, after `remember` (pass `sourceLast: true` to cite it). `link` is canonical; `relate` is a deprecated alias of this tool. Then run `inspect` to confirm the link is evidence-backed.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
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

    #[tool(
        description = "Deprecated alias of `link`; prefer that. Kept for backward compatibility while clients migrate to `link`.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
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

    #[tool(
        description = "Use when an episode establishes a repeatable how-to or rule (a procedure) you want recalled later, after `remember` (pass `sourceLast: true` to cite it). For a stated behavioral preference rather than a procedure, use `preference`. Then run `inspect` to confirm it is supported.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
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

    #[tool(
        description = "Use when an episode states a durable behavioral preference or rule (a coding convention, a communication style, an operating default) you want recalled later, after `remember` (pass `sourceLast: true` to cite it). Distinct from `procedure`: a preference is a stated rule, a procedure is a repeatable how-to. Then run `inspect` to confirm it is supported.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
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

    #[tool(
        description = "Use when the user commits to future work, a goal, or a reminder that must survive across sessions. Capture it here so `briefing`, `deadlines`, and `reconcile_intentions` can surface it later. Then use `intention_update` to add a deadline or goal link, and `intention_status` to close it out.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
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

    #[tool(
        description = "Use when an intention's lifecycle changes (it is completed, abandoned, blocked, or deferred) and you want that recorded with a reason. Find the intention id via `briefing` or `recall` first. Then re-check `reconcile_intentions` if you need the updated commitment picture.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
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

    #[tool(
        description = "Use when you need to set an intention's deadline, goal link, dependencies, or progress without changing its lifecycle status. Look up the intention id via `briefing` or `recall` first. Then `goal_progress` and `deadlines` will reflect the new metadata.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
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

    #[tool(
        description = "Use when you need a read-only picture of which intentions are stale, overdue, or need attention before deciding what to act on. It does not change any intention. Then act with `intention_status` or `intention_update`.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
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

    #[tool(
        description = "Use when you want a read-only roll-up of progress toward goals and their linked tasks. It does not change anything. Then update progress with `intention_update` if the picture is out of date.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    fn goal_progress(&self) -> Result<Json<DatabaseReportResult>, String> {
        let report = self.with_memory(|memory| Ok(memory.goal_progress()))?;
        let report = serde_json::to_value(report).map_err(|error| error.to_string())?;
        Ok(Json(DatabaseReportResult {
            database: self.database.display().to_string(),
            report,
        }))
    }

    #[tool(
        description = "Use when you want a read-only operator report of everything that may need attention (deadlines, anomalies, and review signals together). It does not change anything. Then drill in with `deadlines` or `anomalies`, or act through `review` and the explicit write tools.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
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

    #[tool(
        description = "Use when you want a read-only list of upcoming or overdue intention deadlines within a horizon. It does not change anything. Then act with `intention_update` or `intention_status` on the items that need it.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
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

    #[tool(
        description = "Use when you want a read-only list of memory anomalies (contradictions, unsupported assertions, stale facts) that may warrant review. It does not change anything. Then record a decision on one with `anomaly_acknowledge`.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
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

    #[tool(
        description = "Use when you have reviewed an anomaly from `anomalies` and want to record an explicit, auditable decision about it with a note. Set `dryRun: true` to preview the decision without writing it. Then re-run without `dryRun` to append the audit record.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
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

    #[tool(
        description = "Use this first at the start of a session, before any other work: it returns the compact read-only pre-work surface (authority, health, recent episodes, active intentions, high-priority review items, graph seeds). It does not change anything. Then `recall` specifics or act on the surfaced intentions and review items.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
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
        Ok(Json(BriefingResult {
            report: report.into(),
        }))
    }

    #[tool(
        description = "Use when a host wants the right governed context bundled for a host execution point: pass `kind` as `session_start`, `pre_prompt`, `post_action`, `session_close`, or `sleep_cycle`. It is read-only and packages authority, directives, recall, and (for close/sleep) reflection or self-inspection. Then follow the returned directives with explicit tool calls.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
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
        Ok(Json(MemoryHookResult {
            report: report.into(),
        }))
    }

    #[tool(
        description = "Use this before acting on anything memory might already know: it retrieves matching memory with per-result trust, evidence IDs, a store-level authority decision, and a health report in one call. Read each result's trust and the authority/health before relying on it. Then cite evidence IDs, or state the gap if authority or health is `warn`/`block`.",
        annotations(read_only_hint = true, open_world_hint = false)
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

    #[tool(
        description = "Use when flat `recall` is not enough and you need the neighborhood around an entity (nodes, edges, evidence IDs, authority, and health/review overlays) to a given depth. It is read-only. Then `recall` or `inspect` specific nodes the traversal surfaced.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
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
        Ok(Json(GraphResult {
            report: report.into(),
        }))
    }

    #[tool(
        description = "Use when you need the database-wide health snapshot (supported vs unsupported facts, contradictions, stale facts, blind spots) to gauge whether memory can be trusted right now. It is read-only and does not propose fixes. Then run `self_inspect` for a structured findings report before proposing any repair.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    fn inspect(&self) -> Result<Json<InspectResult>, String> {
        let health = self.with_memory(|memory| Ok(memory.inspect()))?;
        Ok(Json(InspectResult {
            health: health.into(),
        }))
    }

    #[tool(
        description = "Use before proposing any memory repair: it returns a read-only consolidation report with health, authority, findings, proposed review items, and an explicit `automatic_write_back=false` policy. It never writes. Then turn proposals into a queue with `review`, and apply only through `review_resolve`.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    fn self_inspect(&self) -> Result<Json<SelfInspectResult>, String> {
        let report = self.with_memory(|memory| Ok(memory.self_inspect()))?;
        Ok(Json(SelfInspectResult {
            report: report.into(),
        }))
    }

    #[tool(
        description = "Use when you want self-inspection findings grouped into prioritized reflection cycles (with evidence IDs and coverage) for operator approval, rather than the raw findings. It is read-only and keeps `automatic_write_back=false`. Then move approved items through `review` and `review_resolve`.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
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
        Ok(Json(ReflectResult {
            report: report.into(),
        }))
    }

    #[tool(
        description = "Use when you want the full sleep/consolidation plan (replay, extraction, reconciliation, review-gate, and commit-eligibility steps) before any write-back. It is read-only and keeps `automatic_write_back=false`. Then act on the review gate via `review` and `review_resolve`; this tool never commits on its own.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
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
        Ok(Json(ConsolidationPlanResult {
            report: report.into(),
        }))
    }

    #[tool(
        description = "Use when you want the prioritized, read-only operator review queue derived from self-inspection; narrow it with `action` to one proposed operator action. Treat it as guidance, not automatic write-back. Then apply a chosen item with `review_resolve`, which is the only path that writes a decision back.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
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
        Ok(Json(ReviewResult {
            report: report.into(),
        }))
    }

    #[tool(
        description = "Use this as the only write-back path for review items: after picking one from `review`, resolve it with its id and an explicit operator note. Set `dryRun: true` to preview the audit decision without writing. Then re-run without `dryRun` to append the decision to the ledger.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
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

    #[tool(
        description = "Use when you need to know whether the derived SurrealDB graph projection is current with the record ledger. It is read-only. Then run `projection_validate` to confirm consistency, or `projection_rebuild` if it is stale.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
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

    #[tool(
        description = "Use when `projection_status` or `projection_validate` shows the derived SurrealDB graph projection is stale or inconsistent. It rebuilds the derived tier from the record ledger and does not change ground truth. Then run `projection_validate` to confirm it now matches.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
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

    #[tool(
        description = "Use when you want to confirm the derived SurrealDB graph projection still matches the record ledger and report any drift. It is read-only. Then run `projection_rebuild` if it reports the projection is out of sync.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
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

    #[tool(
        description = "Use when you need to know whether the derived Qdrant semantic index is present and current. It is read-only. Then run `semantic_rebuild` if it is missing or stale.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
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

    #[tool(
        description = "Use when `semantic_status` shows the derived Qdrant semantic index is missing or stale. It rebuilds the index from projected memory and does not change ground truth. Then run `semantic_status` to confirm it is current.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
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

    #[tool(
        description = "Use when you need to confirm the append-only SurrealDB memory_record ledger is intact and report record counts and any migration needs. It is read-only and checks the ground-truth ledger itself, not a derived tier. Then run `projection_validate`/`semantic_status` to check the derived tiers if the ledger is healthy.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
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
