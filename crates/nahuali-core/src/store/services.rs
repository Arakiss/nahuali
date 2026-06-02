fn semantic_config_for_database(database: &Path) -> Result<SemanticConfig> {
    SemanticConfig::from_env()?.scoped_to_database(database_name(database))
}

fn semantic_collection_for_database(database: &Path) -> String {
    semantic_config_for_database(database)
        .map(|config| config.collection_name)
        .unwrap_or_else(|_| {
            SemanticConfig::default_local()
                .scoped_to_database(database_name(database))
                .expect("database names normalize to valid semantic collection suffixes")
                .collection_name
        })
}

impl MemoryEngine {
    /// Rebuild the Qdrant semantic index from the current deterministic projection.
    ///
    /// The SurrealDB record ledger remains authoritative. Existing Qdrant
    /// collection data for the configured collection is replaced because the
    /// semantic tier is a derived projection.
    pub fn rebuild_semantic_index(&self) -> Result<SemanticIndexReport> {
        let config = semantic_config_for_database(&self.path)?;
        self.rebuild_semantic_index_with_config(&config)
    }

    /// Rebuild the Qdrant semantic index with an explicit configuration.
    pub fn rebuild_semantic_index_with_config(
        &self,
        config: &SemanticConfig,
    ) -> Result<SemanticIndexReport> {
        semantic::rebuild_index(&self.data, config)
    }

    /// Return Qdrant semantic index status for the environment configuration.
    pub fn semantic_index_status(&self) -> Result<SemanticIndexStatus> {
        let config = semantic_config_for_database(&self.path)?;
        self.semantic_index_status_with_config(&config)
    }

    /// Return Qdrant semantic index status for an explicit configuration.
    pub fn semantic_index_status_with_config(
        &self,
        config: &SemanticConfig,
    ) -> Result<SemanticIndexStatus> {
        semantic::index_status(config)
    }

    /// Recall memory by merging deterministic lexical results with Qdrant semantic matches.
    ///
    /// `limit` is coerced to at least `1`. Empty queries are rejected after
    /// trimming. The returned report keeps lexical and semantic components
    /// separate so callers can explain why each result was ranked.
    pub fn hybrid_recall(&self, query: &str, limit: usize) -> Result<HybridRecallReport> {
        let config = semantic_config_for_database(&self.path)?;
        self.hybrid_recall_with_config(query, limit, &config)
    }

    /// Recall memory with hybrid scoring and lexical/Qdrant filters.
    pub fn hybrid_recall_with_options(
        &self,
        query: &str,
        options: RecallOptions,
    ) -> Result<HybridRecallReport> {
        let config = semantic_config_for_database(&self.path)?;
        self.hybrid_recall_with_options_and_config(query, options, &config)
    }

    /// Recall memory with hybrid scoring and an explicit semantic configuration.
    pub fn hybrid_recall_with_config(
        &self,
        query: &str,
        limit: usize,
        config: &SemanticConfig,
    ) -> Result<HybridRecallReport> {
        if query.trim().is_empty() {
            return Err(NahualiError::EmptyQuery);
        }

        let authority = self.authority();
        semantic::hybrid_recall(&self.data, query, limit.max(1), authority, config)
    }

    /// Recall memory with hybrid scoring, filters, and an explicit semantic configuration.
    pub fn hybrid_recall_with_options_and_config(
        &self,
        query: &str,
        options: RecallOptions,
        config: &SemanticConfig,
    ) -> Result<HybridRecallReport> {
        if query.trim().is_empty() {
            return Err(NahualiError::EmptyQuery);
        }

        let limit = options.limit.max(1);
        let authority = self.authority();
        semantic::hybrid_recall_with_options(&self.data, query, limit, options, authority, config)
    }

    /// Inspect the projected store for support, contradictions, staleness, and
    /// blind spots.
    pub fn inspect(&self) -> KnowledgeHealth {
        KnowledgeHealth::inspect(&self.data)
    }

    /// Evaluate whether the current projection can be trusted.
    pub fn authority(&self) -> AuthorityDecision {
        AuthorityDecision::evaluate(&self.inspect())
    }

    /// Produce a non-mutating self-inspection report for the current projection.
    ///
    /// The report converts health and authority signals into findings and a
    /// proposed review queue, but it never writes memory records by itself.
    pub fn self_inspect(&self) -> SelfInspectionReport {
        self_inspection::self_inspect(&self.data)
    }

    /// Produce a non-mutating self-inspection report at an explicit timestamp.
    ///
    /// This is useful for deterministic tests and reproducible evaluations of
    /// staleness-sensitive findings.
    pub fn self_inspect_at(&self, now_ms: u64) -> SelfInspectionReport {
        self_inspection::self_inspect_at(&self.data, now_ms)
    }

    /// Produce a prioritized, non-mutating operator review queue.
    ///
    /// This report is derived from self-inspection and authority signals. It
    /// never writes memory records and keeps write-back behind explicit
    /// operator commands.
    pub fn operator_review(&self, limit: usize) -> OperatorReviewReport {
        self.operator_review_with_options(OperatorReviewOptions {
            limit,
            ..OperatorReviewOptions::default()
        })
    }

    /// Produce a prioritized operator review queue with explicit options.
    pub fn operator_review_with_options(
        &self,
        options: OperatorReviewOptions,
    ) -> OperatorReviewReport {
        operator_review::operator_review(&self.data, options)
    }

    /// Produce a compact non-mutating session briefing.
    ///
    /// Briefings combine authority, health, recent episodes, active intentions,
    /// critical or high-priority review items, and graph seeds for pre-work
    /// continuity. They never write records.
    pub fn briefing(&self) -> MemoryBriefingReport {
        self.briefing_with_options(BriefingOptions::default())
    }

    /// Produce a compact non-mutating session briefing with explicit limits.
    pub fn briefing_with_options(&self, options: BriefingOptions) -> MemoryBriefingReport {
        briefing::briefing(&self.data, options)
    }

    /// Produce a focused non-mutating project/entity dashboard.
    ///
    /// Project views compose graph traversal, recall, knowledge health,
    /// authority, related memory, and operator review context for one entity
    /// or project query. They never write memory records.
    pub fn project_view(&self, query: &str) -> Result<MemoryProjectReport> {
        self.project_view_with_options(query, ProjectViewOptions::default())
    }

    /// Produce a focused project/entity dashboard with explicit limits.
    pub fn project_view_with_options(
        &self,
        query: &str,
        options: ProjectViewOptions,
    ) -> Result<MemoryProjectReport> {
        project::project_view(&self.data, query, options)
    }

    /// Produce a non-mutating intention reconciliation report.
    pub fn reconcile_intentions(&self) -> IntentionReconciliationReport {
        self.reconcile_intentions_with_options(IntentionReconciliationOptions::default())
    }

    /// Produce a non-mutating intention reconciliation report with explicit options.
    pub fn reconcile_intentions_with_options(
        &self,
        options: IntentionReconciliationOptions,
    ) -> IntentionReconciliationReport {
        intention::reconcile_intentions(&self.data, options)
    }

    /// Produce a non-mutating goal progress report.
    pub fn goal_progress(&self) -> crate::GoalProgressReport {
        intention::goal_progress(&self.data)
    }

    /// Produce a non-mutating proactive operator report.
    ///
    /// The report composes deadline, anomaly, capture-opportunity, and
    /// high-risk review signals from the current Rust projection. It never
    /// writes records by itself.
    pub fn proactive(&self) -> MemoryProactiveReport {
        self.proactive_with_options(ProactiveOptions::default())
    }

    /// Produce a non-mutating proactive operator report with explicit options.
    pub fn proactive_with_options(&self, options: ProactiveOptions) -> MemoryProactiveReport {
        proactive::proactive_report(&self.data, options)
    }

    /// Produce a non-mutating deadline report from intention metadata.
    pub fn deadlines(&self) -> DeadlineReport {
        self.deadlines_with_options(ProactiveOptions::default())
    }

    /// Produce a non-mutating deadline report with explicit options.
    pub fn deadlines_with_options(&self, options: ProactiveOptions) -> DeadlineReport {
        proactive::deadline_report(&self.data, options)
    }

    /// Produce a non-mutating anomaly report.
    pub fn anomalies(&self) -> AnomalyReport {
        self.anomalies_with_options(ProactiveOptions::default())
    }

    /// Produce a non-mutating anomaly report with explicit options.
    pub fn anomalies_with_options(&self, options: ProactiveOptions) -> AnomalyReport {
        proactive::anomaly_report(&self.data, options)
    }

    /// Acknowledge a proactive anomaly through an explicit append-only review decision.
    ///
    /// Dry runs return the same report shape without appending a record. Applied
    /// acknowledgements are projected as review decisions and can suppress the
    /// acknowledged alert in future anomaly reports.
    pub fn acknowledge_anomaly(
        &mut self,
        anomaly_id: impl Into<String>,
        note: impl Into<String>,
        dry_run: bool,
    ) -> Result<AnomalyAcknowledgementReport> {
        let prepared = proactive::prepare_anomaly_acknowledgement(
            &self.data,
            AnomalyAcknowledgementOptions {
                anomaly_id: anomaly_id.into(),
                note: note.into(),
                dry_run,
            },
            make_id("review_decision"),
        )?;

        if dry_run {
            return Ok(prepared.report);
        }

        let mut report = prepared.report;
        let envelope = self.append(MemoryEvent::ReviewRecorded(prepared.event))?;
        report.applied = true;
        report.event_id = Some(envelope.id);

        Ok(report)
    }

    /// Run a non-mutating memory hook for a host execution point.
    ///
    /// Hooks package the memory context a host should load at session start,
    /// before prompts, after actions, at session close, or during a sleep-cycle
    /// consolidation pass. They never write memory records.
    pub fn run_hook(&self, kind: MemoryHookKind) -> Result<MemoryHookReport> {
        self.run_hook_with_options(MemoryHookOptions {
            kind,
            ..MemoryHookOptions::default()
        })
    }

    /// Run a non-mutating memory hook with explicit limits and input.
    pub fn run_hook_with_options(&self, options: MemoryHookOptions) -> Result<MemoryHookReport> {
        hooks::memory_hook(&self.data, options)
    }

    /// Produce a non-mutating Sleep Mode consolidation report.
    ///
    /// Sleep Mode replays recent episodes, groups self-inspection work, and
    /// proposes consolidation candidates while keeping write-back behind
    /// explicit operator approval.
    pub fn sleep(&self) -> MemorySleepReport {
        self.sleep_with_options(SleepModeOptions::default())
    }

    /// Produce a non-mutating Sleep Mode report with explicit limits.
    pub fn sleep_with_options(&self, options: SleepModeOptions) -> MemorySleepReport {
        sleep::sleep_mode(&self.data, options)
    }

    /// Produce a non-mutating consolidation plan for replay, review, and write-back gates.
    ///
    /// The plan turns Sleep Mode and operator-review signals into an explicit
    /// pipeline, but it never writes memory records by itself.
    pub fn consolidation_plan(&self) -> MemoryConsolidationPlanReport {
        self.consolidation_plan_with_options(ConsolidationPlanOptions::default())
    }

    /// Produce a non-mutating consolidation plan with explicit limits.
    pub fn consolidation_plan_with_options(
        &self,
        options: ConsolidationPlanOptions,
    ) -> MemoryConsolidationPlanReport {
        consolidation_plan::consolidation_plan(&self.data, options)
    }

    /// Produce a non-mutating reflection cycle for consolidation planning.
    ///
    /// Reflection groups self-inspection findings into operator-approved cycles,
    /// reports source/evidence coverage, and keeps all write-back behind
    /// explicit follow-up commands.
    pub fn reflect(&self) -> MemoryReflectionReport {
        self.reflect_with_options(ReflectionOptions::default())
    }

    /// Produce a non-mutating reflection cycle with explicit limits.
    pub fn reflect_with_options(&self, options: ReflectionOptions) -> MemoryReflectionReport {
        reflection::reflect(&self.data, options)
    }

    /// Preview an explicit operator review resolution without writing records.
    pub fn review_resolution_plan(
        &self,
        review_id: impl Into<String>,
        note: impl Into<String>,
    ) -> Result<ReviewResolutionReport> {
        let prepared = review_writeback::prepare_review_resolution(
            &self.data,
            ReviewResolutionOptions {
                review_id: review_id.into(),
                note: note.into(),
                dry_run: true,
            },
            make_id("review_decision"),
        )?;

        Ok(prepared.report)
    }

    /// Resolve an operator review item through an explicit append-only decision.
    ///
    /// Dry runs return the same report shape without appending a record. Applied
    /// resolutions are projected as review decisions and can reduce future
    /// self-inspection work for the reviewed evidence.
    pub fn resolve_review_item(
        &mut self,
        review_id: impl Into<String>,
        note: impl Into<String>,
        dry_run: bool,
    ) -> Result<ReviewResolutionReport> {
        let prepared = review_writeback::prepare_review_resolution(
            &self.data,
            ReviewResolutionOptions {
                review_id: review_id.into(),
                note: note.into(),
                dry_run,
            },
            make_id("review_decision"),
        )?;

        if dry_run {
            return Ok(prepared.report);
        }

        let mut report = prepared.report;
        let envelope = self.append(MemoryEvent::ReviewRecorded(prepared.event))?;
        report.applied = true;
        report.event_id = Some(envelope.id);

        Ok(report)
    }

    /// Return a deterministic graph neighborhood around a seed entity or memory item.
    pub fn graph_neighborhood(
        &self,
        seed: &str,
        max_depth: usize,
        limit: usize,
    ) -> Result<MemoryGraphReport> {
        self.graph_neighborhood_with_options(seed, GraphTraversalOptions { max_depth, limit })
    }

    /// Return a deterministic graph neighborhood with explicit traversal options.
    pub fn graph_neighborhood_with_options(
        &self,
        seed: &str,
        options: GraphTraversalOptions,
    ) -> Result<MemoryGraphReport> {
        graph::graph_neighborhood(&self.data, seed, options)
    }

    /// Return a non-destructive maintenance report for the current store.
    pub fn maintenance_report(&self) -> MaintenanceReport {
        maintenance::maintenance_report(&self.events)
    }

    /// Build an optional snapshot from the current validated projection.
    ///
    /// The returned snapshot is not authoritative. It must be validated against
    /// the SurrealDB record ledger before use, and opening a store never trusts
    /// snapshots.
    pub fn create_snapshot(&self) -> MemorySnapshot {
        maintenance::create_snapshot(&self.events, &self.data)
    }

    /// Write an optional snapshot without mutating the SurrealDB record ledger.
    pub fn write_snapshot(&self, path: impl AsRef<Path>) -> Result<MemorySnapshot> {
        let snapshot = self.create_snapshot();
        maintenance::write_snapshot_file(path.as_ref(), &snapshot)?;
        Ok(snapshot)
    }

    /// Validate a snapshot against a fresh replay of the current record ledger.
    pub fn validate_snapshot(&self, path: impl AsRef<Path>) -> Result<SnapshotValidation> {
        maintenance::validate_snapshot_file(path.as_ref(), &self.events, &self.data)
    }

    /// Build a local backup from the current authoritative record ledger.
    ///
    /// The backup preserves event envelopes exactly and records the Qdrant
    /// semantic tier as derived metadata that should be rebuilt after restore.
    pub fn create_backup(&self) -> MemoryBackup {
        backup::create_backup(
            &self.path,
            &self.events,
            vec![semantic_collection_for_database(&self.path)],
        )
    }

    /// Write a local backup without mutating the SurrealDB record ledger.
    pub fn write_backup(&self, path: impl AsRef<Path>) -> Result<MemoryBackup> {
        let backup = self.create_backup();
        backup::write_backup_file(path.as_ref(), &backup)?;
        Ok(backup)
    }

    /// Validate a local backup file without mutating any database.
    pub fn validate_backup(path: impl AsRef<Path>) -> Result<BackupValidation> {
        backup::validate_backup_file(path.as_ref())
    }

    /// Restore a backup into an empty target database.
    ///
    /// The target database must contain no records. Restores preserve event
    /// envelopes exactly so checksums, sequences, and IDs remain stable.
    pub fn restore_backup(
        path: impl AsRef<Path>,
        target_database: impl AsRef<Path>,
        dry_run: bool,
    ) -> Result<BackupRestoreReport> {
        let path = path.as_ref();
        let target_database = target_database.as_ref();
        let validation = backup::validate_backup_file(path)?;
        let target_path = target_database.to_path_buf();
        let target_events = block_on_database(async move { read_records(&target_path).await })?;
        let target_was_empty = target_events.is_empty();
        let mut issues = validation.issues.clone();

        if !target_was_empty {
            issues.push(backup::target_not_empty_issue(target_events.len()));
        }

        let semantic_restore_policy = validation
            .semantic_tier
            .as_ref()
            .map(|tier| tier.restore_policy.clone())
            .or(Some(SemanticTierRestorePolicy::RebuildFromRecords));
        let appendable_event_count = validation.backup_record_count.unwrap_or_default();
        let mut report = BackupRestoreReport {
            valid: validation.valid && target_was_empty,
            dry_run,
            backup_path: path.display().to_string(),
            target_database: target_database.display().to_string(),
            appendable_event_count,
            restored_event_count: 0,
            target_was_empty,
            record_ledger_checksum: validation.backup_record_ledger_checksum.clone(),
            semantic_restore_policy,
            issues,
        };

        if dry_run || !report.valid {
            return Ok(report);
        }

        let backup = backup::read_backup_file(path)?;
        let expected_checksum = backup.record_ledger_checksum.clone();
        let write_path = target_database.to_path_buf();
        let events = backup.records.clone();
        block_on_database(async move { write_records(&write_path, &events).await })?;

        let read_path = target_database.to_path_buf();
        let restored_events = block_on_database(async move { read_records(&read_path).await })?;
        let restored_checksum = record_ledger_checksum(&restored_events);
        report.restored_event_count = restored_events.len();

        if restored_events.len() != backup.records.len() || restored_checksum != expected_checksum {
            report.valid = false;
            report.issues.push(backup::restore_verification_issue());
        }

        Ok(report)
    }

    /// Run a non-mutating recovery drill for a backup and target database.
    ///
    /// The drill validates the backup and performs a restore dry-run against the
    /// target database. It never writes backup records.
    pub fn backup_drill(
        path: impl AsRef<Path>,
        target_database: impl AsRef<Path>,
    ) -> Result<BackupDrillReport> {
        let path = path.as_ref();
        let target_database = target_database.as_ref();
        let backup_validation = backup::validate_backup_file(path)?;
        let restore_dry_run = Self::restore_backup(path, target_database, true)?;

        Ok(backup::backup_drill_report(
            path,
            target_database,
            backup_validation,
            restore_dry_run,
        ))
    }

    /// Export the current projection as a source-neutral interchange document.
    ///
    /// The returned document is not a record ledger or snapshot. It can be used to
    /// seed another store through [`Self::import_interchange`].
    pub fn export_interchange(&self) -> MemoryInterchange {
        interchange::export(&self.data)
    }

    /// Import a source-neutral interchange document by appending new records.
    ///
    /// The document is fully validated before any event is written. A dry-run
    /// returns the same validation report without mutating the store.
    pub fn import_interchange(
        &mut self,
        document: &MemoryInterchange,
        dry_run: bool,
    ) -> Result<InterchangeImportReport> {
        interchange::import(self, document, dry_run)
    }

    /// Ingest a structured source-neutral document with provenance.
    ///
    /// The document is fully validated before any records are written. A dry-run
    /// returns the same report shape without mutating the store.
    pub fn ingest_document(
        &mut self,
        document: &MemoryIngestDocument,
        dry_run: bool,
    ) -> Result<IngestionReport> {
        ingestion::ingest(self, document, dry_run)
    }

    /// Compatibility no-op.
    ///
    /// Mutating operations write synchronously today, so there is no separate
    /// buffered state to flush.
    pub fn save(&self) -> Result<()> {
        Ok(())
    }

    fn append(&mut self, payload: MemoryEvent) -> Result<EventEnvelope> {
        self.append_at(now_ms(), payload)
    }

    /// Current tamper-evident chain tip: the chained hash of the last appended
    /// event, or `None` for an empty ledger.
    ///
    /// This is the value to anchor (e.g. publish or sign) so that even a full
    /// re-chaining of the ledger suffix becomes detectable: re-chaining changes
    /// the tip, and an externally recorded tip will no longer match. Tip signing
    /// itself is out of scope here (see the TODO in the implementation).
    #[cfg(feature = "tamper-evidence")]
    pub fn chain_tip(&self) -> Option<String> {
        self.events.last().map(EventEnvelope::chain_hash)
    }

    pub(crate) fn append_at(
        &mut self,
        timestamp_ms: u64,
        payload: MemoryEvent,
    ) -> Result<EventEnvelope> {
        // FEATURE OFF (default): plain self-contained envelope, `prev_hash` stays
        // `None` and is skipped on serialization — byte-identical to before.
        #[cfg(not(feature = "tamper-evidence"))]
        let envelope = EventEnvelope::new(self.next_sequence, timestamp_ms, payload);
        // FEATURE ON: bind the previous event's chained hash so any later rewrite
        // of this or an earlier event breaks the chain at the next event.
        //
        // TODO(tamper-evidence): optionally sign/anchor `chain_tip()` so a full
        // suffix re-chain (which changes the tip) is also externally detectable.
        #[cfg(feature = "tamper-evidence")]
        let envelope = {
            let previous_chain_hash = self.events.last().map(EventEnvelope::chain_hash);
            EventEnvelope::with_chain(
                self.next_sequence,
                timestamp_ms,
                payload,
                previous_chain_hash.as_deref(),
            )
        };
        let write_path = self.path.clone();
        let write_envelope = envelope.clone();
        block_on_database(async move { write_record(&write_path, &write_envelope).await })?;

        self.events.push(envelope.clone());
        self.data = projection::project(&self.events);
        let graph_path = self.path.clone();
        let graph_data = self.data.clone();
        let graph_events = self.events.clone();
        block_on_database(async move {
            rebuild_graph_projection(&graph_path, &graph_data, &graph_events).await
        })?;
        self.next_sequence += 1;

        Ok(envelope)
    }
}
