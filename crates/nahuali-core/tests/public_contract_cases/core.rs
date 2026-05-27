#[test]
fn public_api_exposes_canonical_engine_and_compatibility_aliases() {
    let path = temp_store("public-api-canonical-engine");

    assert_eq!(
        std::any::type_name::<MemoryEngine>(),
        std::any::type_name::<LocalMemory>()
    );
    assert_eq!(
        std::any::type_name::<Fact>(),
        std::any::type_name::<Claim>()
    );
    assert_eq!(
        std::any::type_name::<Relation>(),
        std::any::type_name::<Link>()
    );

    let mut memory = MemoryEngine::open(&path).expect("new engine opens");
    let episode = memory
        .remember("Lena owns the release notes.", vec!["product".to_string()])
        .expect("episode records");
    let claim: Claim = memory
        .add_claim(
            "Lena",
            "owns",
            "release notes",
            Some(episode.id.clone()),
            0.92,
        )
        .expect("canonical claim records");
    let link: Link = memory
        .add_link(
            "Lena",
            "owns",
            "Release Notes",
            Some(episode.id.clone()),
            0.9,
        )
        .expect("canonical link records");
    let fact: Fact = memory
        .add_fact("Lena", "role", "editor", Some(episode.id.clone()), 0.8)
        .expect("compatibility fact records");
    let relation: Relation = memory
        .relate("Lena", "reviews", "Release Notes", Some(episode.id), 0.8)
        .expect("compatibility relation records");

    assert_eq!(claim.subject, "Lena");
    assert_eq!(link.from, "Lena");
    assert_eq!(fact.predicate, "role");
    assert_eq!(relation.relation, "reviews");
    assert!(
        MemoryEngine::validate_store(&path)
            .expect("ledger validates")
            .valid
    );

    let reopened = MemoryEngine::open(&path).expect("engine reopens");
    assert_eq!(reopened.data().claims.len(), 2);
    assert_eq!(reopened.data().links.len(), 2);
    assert_eq!(reopened.data().facts, reopened.data().claims);
    assert_eq!(reopened.data().relations, reopened.data().links);

    let _ = fs::remove_file(path);
}

#[test]
fn public_api_persists_projects_recalls_and_inspects_evidence() {
    let path = temp_store("public-api-supported-memory");

    let mut memory = MemoryEngine::open(&path).expect("new store opens");
    let episode = memory
        .remember("Lena owns the release notes.", vec!["product".to_string()])
        .expect("episode records");
    let fact = memory
        .add_fact(
            "Lena",
            "owns",
            "release notes",
            Some(episode.id.clone()),
            0.92,
        )
        .expect("fact records");
    let relation = memory
        .relate(
            "Lena",
            "owns",
            "release notes",
            Some(episode.id.clone()),
            0.9,
        )
        .expect("relation records");

    assert_eq!(memory.events().len(), 3);
    assert_eq!(memory.events()[0].sequence, 1);
    assert_eq!(memory.events()[1].sequence, 2);
    assert_eq!(memory.events()[2].sequence, 3);
    assert!(memory.events().iter().all(EventEnvelope::validate_checksum));

    let reopened = MemoryEngine::open(&path).expect("store reopens");
    let data = reopened.data();
    assert_eq!(data.event_count, 3);
    assert_eq!(data.episodes.len(), 1);
    assert_eq!(data.facts.len(), 1);
    assert_eq!(data.relations.len(), 1);
    assert_eq!(data.facts[0].id, fact.id);
    assert_eq!(data.relations[0].id, relation.id);
    assert_eq!(
        data.last_event_id.as_deref(),
        Some(reopened.events()[2].id.as_str())
    );

    let results = reopened
        .recall("Lena release", 10)
        .expect("recall succeeds");
    assert!(results.iter().any(|result| {
        result.kind == MemoryKind::Claim
            && result.evidence_id.as_deref() == Some(episode.id.as_str())
    }));
    let filtered = reopened
        .recall_with_options(
            "Lena release",
            RecallOptions {
                limit: 10,
                kinds: vec![MemoryKind::Claim],
                require_evidence: true,
                ..RecallOptions::default()
            },
        )
        .expect("filtered recall succeeds");
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].kind, MemoryKind::Claim);
    assert_eq!(filtered[0].evidence_id.as_deref(), Some(episode.id.as_str()));

    let health = reopened.inspect();
    assert_eq!(health.supported_fact_count, 1);
    assert_eq!(health.unsupported_fact_count, 0);
    assert_eq!(health.blind_spot_count, 0);

    let _ = fs::remove_file(path);
}

#[test]
fn public_api_projects_all_core_memory_families() {
    let path = temp_store("public-api-core-memory-families");

    let mut memory = MemoryEngine::open(&path).expect("new store opens");
    let episode = memory
        .remember_with_mentions(
            "Lena wants release notes to stay concise.",
            vec!["product".to_string()],
            vec!["Lena".to_string(), "Release Notes".to_string()],
        )
        .expect("episode records");
    let claim = memory
        .add_claim(
            "Lena",
            "owns",
            "release notes",
            Some(episode.id.clone()),
            0.93,
        )
        .expect("claim records");
    let link = memory
        .add_link(
            "Lena",
            "owns",
            "Release Notes",
            Some(episode.id.clone()),
            0.91,
        )
        .expect("link records");
    let procedure = memory
        .add_preference(
            "Release notes",
            "Keep release notes concise.",
            Some(episode.id.clone()),
            0.88,
        )
        .expect("preference records");
    let intention = memory
        .add_intention(
            "Ship release notes",
            IntentionKind::Task,
            IntentionPriority::High,
            Some(episode.id),
        )
        .expect("intention records");
    memory
        .set_intention_status(
            intention.id.clone(),
            IntentionStatus::Completed,
            Some("Done".to_string()),
        )
        .expect("intention status records");

    let reopened = MemoryEngine::open(&path).expect("store reopens");
    let data = reopened.data();

    assert_eq!(data.entities.len(), 2);
    assert_eq!(data.claims[0].id, claim.id);
    assert_eq!(data.links[0].id, link.id);
    assert_eq!(data.facts, data.claims);
    assert_eq!(data.relations, data.links);
    assert_eq!(data.procedures[0].id, procedure.id);
    assert_eq!(data.procedures[0].kind, ProcedureKind::Preference);
    assert_eq!(data.intentions[0].id, intention.id);
    assert_eq!(data.intentions[0].status, IntentionStatus::Completed);
    assert_eq!(data.intentions[0].status_reason.as_deref(), Some("Done"));

    let recalled = reopened
        .recall("release notes", 10)
        .expect("recall succeeds");
    assert!(
        recalled
            .iter()
            .any(|result| result.kind == MemoryKind::Claim)
    );
    assert!(
        recalled
            .iter()
            .any(|result| result.kind == MemoryKind::Link)
    );
    assert!(
        recalled
            .iter()
            .any(|result| result.kind == MemoryKind::Procedure)
    );
    assert!(
        recalled
            .iter()
            .any(|result| result.kind == MemoryKind::Intention)
    );

    let _ = fs::remove_file(path);
}

#[test]
fn public_api_updates_reconciles_and_reports_goal_progress() {
    let path = temp_store("public-api-intention-lifecycle");

    let mut memory = MemoryEngine::open(&path).expect("new store opens");
    let goal = memory
        .add_intention(
            "Launch public beta",
            IntentionKind::Goal,
            IntentionPriority::High,
            None,
        )
        .expect("goal records");
    let dependency = memory
        .add_intention(
            "Prepare release checklist",
            IntentionKind::Task,
            IntentionPriority::Medium,
            None,
        )
        .expect("dependency records");
    let child = memory
        .add_intention(
            "Ship release notes",
            IntentionKind::Task,
            IntentionPriority::High,
            None,
        )
        .expect("child records");

    let updated = memory
        .update_intention(
            child.id.clone(),
            IntentionUpdateOptions {
                description: Some("Ship public release notes".to_string()),
                deadline_at_ms: Some(Some(50)),
                depends_on: Some(vec![
                    dependency.id.clone(),
                    dependency.id.clone(),
                    "missing_intention".to_string(),
                ]),
                goal_id: Some(Some(goal.id.clone())),
                progress_percent: Some(Some(25)),
                ..IntentionUpdateOptions::default()
            },
        )
        .expect("intention metadata updates");

    assert_eq!(updated.description, "Ship public release notes");
    assert_eq!(updated.deadline_at_ms, Some(50));
    assert_eq!(
        updated.depends_on,
        vec![dependency.id.clone(), "missing_intention".to_string()]
    );
    assert_eq!(updated.goal_id.as_deref(), Some(goal.id.as_str()));
    assert_eq!(updated.progress_percent, Some(25));

    let no_op = memory
        .update_intention(child.id.clone(), IntentionUpdateOptions::default())
        .unwrap_err();
    assert!(matches!(no_op, NahualiError::InvalidIntentionUpdate { .. }));

    let events_before_reconcile = memory.events().len();
    let report = memory.reconcile_intentions_with_options(IntentionReconciliationOptions {
        now_ms: 100,
        stale_after_ms: 0,
    });
    assert_eq!(memory.events().len(), events_before_reconcile);
    assert!(report.issues.iter().any(|issue| {
        issue.kind == IntentionReconciliationIssueKind::Overdue
            && issue.intention_id == child.id.as_str()
    }));
    assert!(report.issues.iter().any(|issue| {
        issue.kind == IntentionReconciliationIssueKind::WaitingOnDependency
            && issue.intention_id == child.id.as_str()
    }));
    assert!(report.issues.iter().any(|issue| {
        issue.kind == IntentionReconciliationIssueKind::MissingDependency
            && issue.intention_id == child.id.as_str()
    }));

    let progress = memory.goal_progress();
    assert_eq!(progress.goal_count, 1);
    assert_eq!(progress.goals[0].goal_id, goal.id);
    assert_eq!(progress.goals[0].child_count, 1);
    assert_eq!(progress.goals[0].active_count, 1);
    assert_eq!(progress.goals[0].derived_progress_percent, 0);

    let completed_dependency = memory
        .complete_intention(dependency.id.clone(), Some("Checklist ready".to_string()))
        .expect("dependency completes");
    assert_eq!(completed_dependency.status, IntentionStatus::Completed);
    let blocked_child = memory
        .block_intention(child.id.clone(), Some("Waiting for launch gate".to_string()))
        .expect("child blocks");
    assert_eq!(blocked_child.status, IntentionStatus::Blocked);
    let deferred_goal = memory
        .defer_intention(goal.id.clone(), Some("Review next launch window".to_string()))
        .expect("goal defers");
    assert_eq!(deferred_goal.status, IntentionStatus::Deferred);

    let progress = memory.goal_progress();
    assert_eq!(progress.goals[0].blocked_count, 1);
    assert_eq!(progress.goals[0].active_count, 0);

    let _ = fs::remove_file(path);
}

#[test]
fn public_api_reports_and_acknowledges_proactive_work() {
    let path = temp_store("public-api-proactive-work");

    let mut memory = MemoryEngine::open(&path).expect("new store opens");
    let episode = memory
        .remember("Lena owns the release notes.", vec!["product".to_string()])
        .expect("episode records");
    memory
        .add_claim("Lena", "owns", "release notes", None, 0.9)
        .expect("unsupported claim records");
    let intention = memory
        .add_intention(
            "Ship release notes",
            IntentionKind::Task,
            IntentionPriority::High,
            Some(episode.id),
        )
        .expect("intention records");
    memory
        .update_intention(
            intention.id.clone(),
            IntentionUpdateOptions {
                deadline_at_ms: Some(Some(50)),
                ..IntentionUpdateOptions::default()
            },
        )
        .expect("deadline updates");

    let options = ProactiveOptions {
        now_ms: 100,
        deadline_horizon_ms: 100,
        stale_after_ms: 0,
        review_limit: 20,
    };
    let event_count_before_report = memory.events().len();
    let report = memory.proactive_with_options(options.clone());

    assert_eq!(report.version, MEMORY_PROACTIVE_REPORT_VERSION);
    assert_eq!(memory.events().len(), event_count_before_report);
    assert_eq!(report.deadlines.summary.overdue_count, 1);
    assert_eq!(report.deadlines.deadlines[0].state, DeadlineState::Overdue);
    assert!(report.anomalies.alerts.iter().any(|alert| {
        alert.kind == AnomalyKind::OverdueDeadline
            && alert.evidence_ids.contains(&memory.events()[3].id)
    }));
    assert!(
        report
            .anomalies
            .alerts
            .iter()
            .any(|alert| alert.kind == AnomalyKind::UnsupportedMemory)
    );
    assert!(!report.capture_opportunities.is_empty());
    assert!(!report.write_back_policy.automatic_write_back);

    let overdue_alert_id = report
        .anomalies
        .alerts
        .iter()
        .find(|alert| alert.kind == AnomalyKind::OverdueDeadline)
        .expect("overdue alert exists")
        .id
        .clone();
    let dry_run = memory
        .acknowledge_anomaly(overdue_alert_id.clone(), "Reviewed deadline", true)
        .expect("dry-run acknowledgement reports");
    assert!(dry_run.dry_run);
    assert!(!dry_run.applied);
    assert_eq!(memory.events().len(), event_count_before_report);

    let applied = memory
        .acknowledge_anomaly(overdue_alert_id.clone(), "Reviewed deadline", false)
        .expect("acknowledgement applies");
    assert!(applied.applied);
    assert!(applied.event_id.is_some());
    assert_eq!(memory.events().len(), event_count_before_report + 1);

    let after_ack = memory.anomalies_with_options(options);
    assert!(
        after_ack
            .alerts
            .iter()
            .all(|alert| alert.id != overdue_alert_id)
    );

    let _ = fs::remove_file(path);
}

#[test]
fn public_api_builds_memory_hook_report() {
    let path = temp_store("public-api-memory-hook");

    let mut memory = MemoryEngine::open(&path).expect("new store opens");
    let episode = memory
        .remember_with_mentions(
            "Lena owns the release notes.",
            vec!["product".to_string()],
            vec!["Lena".to_string(), "Release Notes".to_string()],
        )
        .expect("episode records");
    memory
        .add_intention(
            "Ship release notes",
            IntentionKind::Task,
            IntentionPriority::High,
            Some(episode.id),
        )
        .expect("intention records");

    let report = memory
        .run_hook_with_options(MemoryHookOptions {
            kind: MemoryHookKind::PrePrompt,
            input: Some("Who owns release notes?".to_string()),
            recall_limit: 5,
            ..MemoryHookOptions::default()
        })
        .expect("hook report builds");

    assert_eq!(report.version, MEMORY_HOOK_REPORT_VERSION);
    assert_eq!(report.kind, MemoryHookKind::PrePrompt);
    assert!(report.summary.recall_count >= 2);
    assert!(report.recall.is_some());
    assert!(report.briefing.is_none());
    assert!(report.reflection.is_none());
    assert!(
        report
            .directives
            .iter()
            .any(|directive| directive.id == "memory-recall-required")
    );

    let sleep = memory
        .run_hook(MemoryHookKind::SleepCycle)
        .expect("sleep hook builds");
    assert!(sleep.self_inspection.is_some());
    assert!(sleep.reflection.is_some());
    assert!(sleep.sleep.is_some());
    assert!(!sleep.summary.automatic_write_back);

    let _ = fs::remove_file(path);
}

#[test]
fn public_api_returns_sleep_mode_report() {
    let path = temp_store("public-api-sleep-mode");

    let mut memory = MemoryEngine::open(&path).expect("new store opens");
    memory
        .remember_with_mentions(
            "Lena discussed release notes.",
            vec!["product".to_string()],
            vec!["Lena".to_string()],
        )
        .expect("first episode records");
    memory
        .remember_with_mentions(
            "Lena refined release notes.",
            vec!["product".to_string()],
            vec!["Lena".to_string()],
        )
        .expect("second episode records");

    let event_count_before_sleep = memory.events().len();
    let report = memory.sleep_with_options(SleepModeOptions {
        recent_episode_limit: 2,
        candidate_limit: 6,
        ..SleepModeOptions::default()
    });

    assert_eq!(report.version, MEMORY_SLEEP_REPORT_VERSION);
    assert_eq!(report.event_count, 2);
    assert_eq!(report.summary.replayed_episode_count, 2);
    assert!(
        report
            .consolidation_candidates
            .iter()
            .any(|candidate| {
                candidate.kind == SleepConsolidationCandidateKind::RepeatedEpisodeTag
            })
    );
    assert!(!report.write_back_policy.automatic_write_back);
    assert_eq!(memory.events().len(), event_count_before_sleep);

    let hook = memory
        .run_hook(MemoryHookKind::SleepCycle)
        .expect("sleep hook builds");
    assert!(hook.sleep.is_some());
    assert!(hook.summary.sleep_stage_count > 0);

    let _ = fs::remove_file(path);
}

#[test]
fn public_api_returns_consolidation_plan_report_without_mutating() {
    let path = temp_store("public-api-consolidation-plan");

    let mut memory = MemoryEngine::open(&path).expect("new store opens");
    memory
        .remember_with_mentions(
            "Lena discussed release notes.",
            vec!["product".to_string()],
            vec!["Lena".to_string()],
        )
        .expect("first episode records");
    memory
        .remember_with_mentions(
            "Lena refined release notes.",
            vec!["product".to_string()],
            vec!["Lena".to_string()],
        )
        .expect("second episode records");
    memory
        .remember_with_mentions(
            "Lena shipped release notes.",
            vec!["product".to_string()],
            vec!["Lena".to_string()],
        )
        .expect("third episode records");

    let event_count_before_plan = memory.events().len();
    let report = memory.consolidation_plan_with_options(ConsolidationPlanOptions {
        recent_episode_limit: 2,
        candidate_limit: 8,
        cycle_limit: 4,
        evidence_limit: 4,
        review_limit: 8,
    });

    assert_eq!(report.version, MEMORY_CONSOLIDATION_PLAN_VERSION);
    assert_eq!(report.event_count, 3);
    assert_eq!(report.summary.stage_count, 5);
    assert_eq!(report.summary.replay_operation_count, 1);
    assert!(report.summary.extract_candidate_count >= 1);
    assert!(report.summary.review_gate_count >= 1);
    assert!(!report.summary.automatic_write_back);
    assert!(!report.write_back_policy.automatic_write_back);
    assert!(report.operations.iter().any(|operation| {
        operation.kind == ConsolidationOperationKind::CommitEligibility
            && operation.status == ConsolidationOperationStatus::NeedsReview
    }));
    assert_eq!(memory.events().len(), event_count_before_plan);

    let _ = fs::remove_file(path);
}

#[test]
fn public_api_ingests_source_document_with_provenance() {
    let path = temp_store("public-api-ingestion-provenance");

    let mut metadata = BTreeMap::new();
    metadata.insert("adapter".to_string(), "fixture".to_string());
    let document = MemoryIngestDocument {
        version: MEMORY_INGEST_DOCUMENT_VERSION,
        source: IngestSource {
            kind: SourceKind::Conversation,
            title: Some("Release review".to_string()),
            uri: Some("fixture://release-review".to_string()),
            metadata,
            scope: None,
        },
        episodes: vec![
            IngestEpisode {
                ref_id: Some("message-1".to_string()),
                content: "Lena owns the release notes.".to_string(),
                tags: vec!["product".to_string()],
                mentions: vec!["Lena".to_string(), "Release Notes".to_string()],
                source_position: Some(1),
                source_role: Some("user".to_string()),
            },
            IngestEpisode {
                ref_id: Some("message-2".to_string()),
                content: "Release notes should stay concise.".to_string(),
                tags: vec!["product".to_string()],
                mentions: vec!["Release Notes".to_string()],
                source_position: Some(2),
                source_role: Some("assistant".to_string()),
            },
        ],
        claims: vec![IngestClaim {
            subject: "Lena".to_string(),
            predicate: "owns".to_string(),
            object: "release notes".to_string(),
            source_episode_ref: Some("message-1".to_string()),
            confidence: 0.94,
        }],
        links: vec![IngestLink {
            from: "Lena".to_string(),
            relation: "owns".to_string(),
            to: "Release Notes".to_string(),
            source_episode_ref: Some("message-1".to_string()),
            confidence: 0.92,
        }],
        procedures: vec![IngestProcedure {
            kind: ProcedureKind::Preference,
            name: "Release notes".to_string(),
            body: "Keep release notes concise.".to_string(),
            source_episode_ref: Some("message-2".to_string()),
            confidence: 0.9,
        }],
        intentions: vec![IngestIntention {
            kind: IntentionKind::Task,
            priority: IntentionPriority::High,
            status: IntentionStatus::Active,
            description: "Ship release notes".to_string(),
            source_episode_ref: Some("message-1".to_string()),
            status_reason: None,
        }],
    };

    let mut memory = MemoryEngine::open(&path).expect("new store opens");
    let dry_run = memory
        .ingest_document(&document, true)
        .expect("dry-run ingestion reports");
    assert!(dry_run.valid);
    assert!(dry_run.dry_run);
    assert_eq!(dry_run.appendable_event_count, 7);
    assert_eq!(dry_run.ingested_event_count, 0);
    assert_eq!(memory.events().len(), 0);

    let report = memory
        .ingest_document(&document, false)
        .expect("ingestion applies");
    assert!(report.valid);
    assert_eq!(report.ingested_event_count, 7);
    assert!(report.source_id.is_some());
    assert_eq!(report.episode_ids.len(), 2);

    let reopened = MemoryEngine::open(&path).expect("store reopens");
    let data = reopened.data();
    assert_eq!(data.sources.len(), 1);
    assert_eq!(data.sources[0].kind, SourceKind::Conversation);
    assert_eq!(data.sources[0].title.as_deref(), Some("Release review"));
    assert_eq!(data.sources[0].metadata["adapter"], "fixture");
    assert_eq!(data.episodes.len(), 2);
    assert_eq!(data.episodes[0].source_id, report.source_id);
    assert_eq!(data.episodes[0].source_position, Some(1));
    assert_eq!(data.episodes[0].source_role.as_deref(), Some("user"));
    assert_eq!(data.claims.len(), 1);
    assert_eq!(
        data.claims[0].source_episode_id.as_deref(),
        Some(report.episode_ids[0].as_str())
    );
    assert_eq!(data.links.len(), 1);
    assert_eq!(data.procedures.len(), 1);
    assert_eq!(data.intentions.len(), 1);
    assert!(
        MemoryEngine::validate_store(&path)
            .expect("ledger validates")
            .valid
    );

    let invalid_document = MemoryIngestDocument {
        version: MEMORY_INGEST_DOCUMENT_VERSION,
        source: IngestSource {
            kind: SourceKind::Document,
            title: Some("Invalid".to_string()),
            uri: None,
            metadata: BTreeMap::new(),
            scope: None,
        },
        episodes: vec![IngestEpisode {
            ref_id: Some("message-1".to_string()),
            content: "Lena owns release notes.".to_string(),
            tags: Vec::new(),
            mentions: Vec::new(),
            source_position: None,
            source_role: None,
        }],
        claims: vec![IngestClaim {
            subject: "Lena".to_string(),
            predicate: "owns".to_string(),
            object: "release notes".to_string(),
            source_episode_ref: Some("missing".to_string()),
            confidence: 0.9,
        }],
        links: Vec::new(),
        procedures: Vec::new(),
        intentions: Vec::new(),
    };
    let before_invalid = reopened.events().len();
    let mut reopened = MemoryEngine::open(&path).expect("store reopens");
    let invalid = reopened
        .ingest_document(&invalid_document, false)
        .expect("invalid ingestion reports");
    assert!(!invalid.valid);
    assert_eq!(
        invalid.issues[0].kind,
        IngestionIssueKind::UnknownSourceReference
    );
    assert_eq!(reopened.events().len(), before_invalid);

    let _ = fs::remove_file(path);
}
