#[test]
fn public_api_imports_and_exports_interchange_without_copying_record_ledger() {
    let source_path = temp_store("public-api-interchange-source");
    let target_path = temp_store("public-api-interchange-target");

    let mut source = MemoryEngine::open(&source_path).expect("source store opens");
    let source_doc = source
        .record_source(
            SourceKind::Conversation,
            Some("Release review".to_string()),
            Some("fixture://release-review".to_string()),
            "checksum-release-review",
            82,
            BTreeMap::from([("adapter".to_string(), "fixture".to_string())]),
        )
        .expect("source records");
    let episode = source
        .remember_source_episode(
            "Lena wants release notes to stay concise.",
            vec!["product".to_string()],
            vec!["Lena".to_string(), "Release Notes".to_string()],
            source_doc.id,
            Some(1),
            Some("operator".to_string()),
        )
        .expect("episode records");
    source
        .add_claim(
            "Lena",
            "owns",
            "release notes",
            Some(episode.id.clone()),
            0.93,
        )
        .expect("claim records");
    source
        .add_link(
            "Lena",
            "owns",
            "Release Notes",
            Some(episode.id.clone()),
            0.91,
        )
        .expect("link records");
    source
        .add_preference(
            "Release notes",
            "Keep release notes concise.",
            Some(episode.id.clone()),
            0.88,
        )
        .expect("preference records");
    let intention = source
        .add_intention(
            "Ship release notes",
            IntentionKind::Task,
            IntentionPriority::High,
            Some(episode.id),
        )
        .expect("intention records");
    source
        .set_intention_status(
            intention.id,
            IntentionStatus::Blocked,
            Some("Waiting for review".to_string()),
        )
        .expect("status records");

    let interchange = source.export_interchange();
    assert_eq!(interchange.version, MEMORY_INTERCHANGE_VERSION);
    assert_eq!(interchange.sources.len(), 1);
    assert_eq!(interchange.episodes.len(), 1);
    assert_eq!(interchange.claims.len(), 1);
    assert_eq!(interchange.links.len(), 1);
    assert_eq!(interchange.procedures.len(), 1);
    assert_eq!(interchange.intentions.len(), 1);
    assert!(interchange.episodes[0].ref_id.as_deref().is_some());
    assert_eq!(
        interchange.sources[0].title.as_deref(),
        Some("Release review")
    );
    assert_eq!(
        interchange.sources[0].content_checksum.as_deref(),
        Some("checksum-release-review")
    );
    assert_eq!(
        interchange.episodes[0].source_ref.as_deref(),
        Some(interchange.sources[0].ref_id.as_str())
    );
    assert_eq!(interchange.episodes[0].source_position, Some(1));
    assert_eq!(interchange.episodes[0].source_role.as_deref(), Some("operator"));
    let episode_timestamp = interchange.episodes[0]
        .timestamp_ms
        .expect("interchange exports episode timestamp");
    let source_timestamp = interchange.sources[0]
        .timestamp_ms
        .expect("interchange exports source timestamp");
    let claim_timestamp = interchange.claims[0]
        .timestamp_ms
        .expect("interchange exports claim timestamp");
    let link_timestamp = interchange.links[0]
        .timestamp_ms
        .expect("interchange exports link timestamp");
    let procedure_timestamp = interchange.procedures[0]
        .timestamp_ms
        .expect("interchange exports procedure timestamp");
    let intention_timestamp = interchange.intentions[0]
        .timestamp_ms
        .expect("interchange exports intention timestamp");
    let status_timestamp = interchange.intentions[0]
        .status_timestamp_ms
        .expect("interchange exports intention status timestamp");
    assert!(
        !serde_json::to_string(&interchange)
            .expect("interchange serializes")
            .contains("event_")
    );

    let mut target = MemoryEngine::open(&target_path).expect("target store opens");
    let dry_run = target
        .import_interchange(&interchange, true)
        .expect("dry-run succeeds");
    assert!(dry_run.valid);
    assert!(dry_run.dry_run);
    assert_eq!(dry_run.appendable_event_count, 7);
    assert_eq!(dry_run.counts.sources, 1);
    assert_eq!(dry_run.preflight.source_count, 1);
    assert_eq!(dry_run.preflight.sourced_episode_count, 1);
    assert_eq!(dry_run.imported_event_count, 0);
    assert_eq!(target.events().len(), 0);

    let imported = target
        .import_interchange(&interchange, false)
        .expect("import succeeds");
    assert!(imported.valid);
    assert_eq!(imported.imported_event_count, 7);

    let reopened = MemoryEngine::open(&target_path).expect("target reopens");
    let data = reopened.data();
    assert_eq!(data.event_count, 7);
    assert_eq!(data.sources.len(), 1);
    assert_eq!(data.episodes.len(), 1);
    assert_eq!(data.claims.len(), 1);
    assert_eq!(data.links.len(), 1);
    assert_eq!(data.procedures.len(), 1);
    assert_eq!(data.intentions.len(), 1);
    assert_eq!(data.sources[0].created_at_ms, source_timestamp);
    assert_eq!(data.sources[0].title.as_deref(), Some("Release review"));
    assert_eq!(
        data.sources[0].content_checksum,
        "checksum-release-review".to_string()
    );
    assert_eq!(data.sources[0].metadata["adapter"], "fixture");
    assert_eq!(
        data.episodes[0].source_id.as_deref(),
        Some(data.sources[0].id.as_str())
    );
    assert_eq!(data.episodes[0].source_position, Some(1));
    assert_eq!(data.episodes[0].source_role.as_deref(), Some("operator"));
    assert_eq!(data.episodes[0].created_at_ms, episode_timestamp);
    assert_eq!(data.claims[0].created_at_ms, claim_timestamp);
    assert_eq!(data.links[0].created_at_ms, link_timestamp);
    assert_eq!(data.procedures[0].created_at_ms, procedure_timestamp);
    assert_eq!(data.intentions[0].created_at_ms, intention_timestamp);
    assert_eq!(data.intentions[0].updated_at_ms, status_timestamp);
    assert_eq!(data.intentions[0].status, IntentionStatus::Blocked);
    assert_eq!(
        data.intentions[0].status_reason.as_deref(),
        Some("Waiting for review")
    );
    assert_eq!(
        data.claims[0].source_episode_id.as_deref(),
        Some(data.episodes[0].id.as_str())
    );

    let _ = fs::remove_file(source_path);
    let _ = fs::remove_file(target_path);
}

#[test]
fn public_api_rejects_invalid_interchange_without_mutating_store() {
    let target_path = temp_store("public-api-invalid-interchange-target");
    let mut target = MemoryEngine::open(&target_path).expect("target store opens");
    let invalid = MemoryInterchange {
        version: 999,
        claims: vec![nahuali_core::InterchangeClaim {
            subject: "Lena".to_string(),
            predicate: "owns".to_string(),
            object: "release notes".to_string(),
            source_episode_ref: Some("missing".to_string()),
            confidence: 0.9,
            scope: None,
            timestamp_ms: None,
        }],
        ..MemoryInterchange::default()
    };

    let report = target
        .import_interchange(&invalid, false)
        .expect("invalid import reports issues");

    assert!(!report.valid);
    assert_eq!(report.imported_event_count, 0);
    assert_eq!(target.events().len(), 0);
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == InterchangeIssueKind::UnsupportedVersion)
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == InterchangeIssueKind::UnknownSourceReference)
    );

    let _ = fs::remove_file(target_path);
}

#[test]
fn public_api_reports_unsupported_memory_health() {
    let path = temp_store("public-api-unsupported-memory");

    let mut memory = MemoryEngine::open(&path).expect("new store opens");
    memory
        .add_fact("Lena", "owns", "release notes", None, 0.4)
        .expect("unsupported fact records");

    let health = memory.inspect();
    assert_eq!(health.unsupported_fact_count, 1);
    assert_eq!(health.low_confidence_fact_count, 1);
    let unsupported_signal = health
        .signals
        .iter()
        .find(|signal| signal.kind == HealthSignalKind::UnsupportedFact)
        .expect("unsupported memory signal is present");
    assert_eq!(unsupported_signal.severity, HealthSeverity::Medium);
    assert_eq!(
        unsupported_signal.dimensions,
        vec![
            HealthDimension::UnsupportedMemory,
            HealthDimension::BlindSpot
        ]
    );
    assert!(
        unsupported_signal
            .evidence_ids
            .iter()
            .any(|id| id.starts_with("event_"))
    );
    assert!(unsupported_signal.message.contains("has no source episode"));
    assert!(
        health
            .signals
            .iter()
            .any(|signal| signal.kind == HealthSignalKind::LowConfidenceFact)
    );

    let _ = fs::remove_file(path);
}

#[test]
fn public_api_calibrates_authority_from_health_signals() {
    let path = temp_store("public-api-authority-calibration");

    let mut memory = MemoryEngine::open(&path).expect("new store opens");
    let empty = memory
        .recall_with_authority("release notes", 10)
        .expect("empty recall succeeds");
    assert_eq!(empty.authority.mode, AuthorityMode::Block);
    assert_eq!(empty.authority.score, 0.0);
    assert!(!empty.authority.can_trust);
    assert_eq!(
        empty.authority.signal_kinds,
        vec![HealthSignalKind::NoEpisodes]
    );

    let episode = memory
        .remember("Lena owns the release notes.", vec!["product".to_string()])
        .expect("episode records");
    memory
        .add_claim(
            "Lena",
            "owns",
            "release notes",
            Some(episode.id.clone()),
            0.92,
        )
        .expect("claim records");

    let weak_graph = memory
        .recall_with_authority("release notes", 10)
        .expect("advisory recall succeeds");
    assert_eq!(weak_graph.authority.mode, AuthorityMode::Advisory);
    assert_eq!(weak_graph.authority.score, 0.75);
    assert!(!weak_graph.authority.can_trust);
    assert_eq!(
        weak_graph.authority.signal_kinds,
        vec![HealthSignalKind::IsolatedEntity]
    );
    let weak_graph_claim = weak_graph
        .results
        .iter()
        .find(|result| result.kind == MemoryKind::Claim)
        .expect("supported claim is returned");
    let weak_graph_claim_trust = weak_graph_claim
        .trust
        .as_ref()
        .expect("authority recall annotates result trust");
    assert_eq!(weak_graph_claim_trust.mode, RecallResultTrustMode::Certify);
    assert!(weak_graph_claim_trust.can_trust);

    memory
        .add_link("Lena", "owns", "release notes", Some(episode.id), 0.9)
        .expect("link records");
    let supported = memory
        .recall_with_authority("release notes", 10)
        .expect("certified recall succeeds");
    assert_eq!(supported.authority.mode, AuthorityMode::Certify);
    assert_eq!(supported.authority.score, 1.0);
    assert!(supported.authority.can_trust);
    assert!(supported.authority.signal_kinds.is_empty());
    assert!(
        supported
            .results
            .iter()
            .filter_map(|result| result.trust.as_ref())
            .any(|trust| trust.mode == RecallResultTrustMode::Certify && trust.can_trust)
    );

    memory
        .add_claim("Mateo", "owns", "deployment keys", None, 0.51)
        .expect("unsupported claim records");
    let mixed_store = memory
        .recall_with_authority("release notes", 10)
        .expect("mixed recall succeeds");
    assert_eq!(mixed_store.authority.mode, AuthorityMode::Warn);
    assert!(!mixed_store.authority.can_trust);
    let mixed_supported_claim = mixed_store
        .results
        .iter()
        .find(|result| result.kind == MemoryKind::Claim)
        .expect("supported claim remains visible");
    let mixed_supported_trust = mixed_supported_claim
        .trust
        .as_ref()
        .expect("supported claim has result trust");
    assert_eq!(mixed_supported_trust.mode, RecallResultTrustMode::Certify);
    assert!(mixed_supported_trust.can_trust);

    let unsupported = memory
        .recall_with_authority("deployment keys", 10)
        .expect("unsupported recall succeeds");
    let unsupported_claim = unsupported
        .results
        .iter()
        .find(|result| result.kind == MemoryKind::Claim)
        .expect("unsupported claim is returned");
    let unsupported_trust = unsupported_claim
        .trust
        .as_ref()
        .expect("unsupported claim has result trust");
    assert_eq!(unsupported_trust.mode, RecallResultTrustMode::Warn);
    assert!(!unsupported_trust.can_trust);
    assert!(
        unsupported_trust
            .signal_kinds
            .iter()
            .any(|kind| kind == "unsupported_fact")
    );

    let _ = fs::remove_file(path);
}

#[test]
fn public_api_self_inspection_is_non_mutating_and_reviewable() {
    let path = temp_store("public-api-self-inspection");

    let mut memory = MemoryEngine::open(&path).expect("new store opens");
    let episode = memory
        .remember("Lena owns the release notes.", vec!["product".to_string()])
        .expect("episode records");
    memory
        .add_claim("Lena", "role", "CTO", Some(episode.id.clone()), 0.95)
        .expect("claim records");
    memory
        // Unprovenanced so it genuinely contradicts the sourced role claim; two
        // values sharing one episode would be a deliberate multi-valued record.
        .add_claim("Lena", "role", "VP Engineering", None, 0.9)
        .expect("conflicting claim records");
    memory
        .add_intention(
            "Ship release notes",
            IntentionKind::Task,
            IntentionPriority::High,
            None,
        )
        .expect("intention records");

    let event_count_before = memory.events().len();
    let report = memory.self_inspect();
    let replayed_projection = project_validated_events(memory.events());
    let projection_report = self_inspect_projection(memory.data());
    let projection_recall = recall_projection_with_authority(
        memory.data(),
        "Lena role",
        RecallOptions::default(),
    )
    .expect("read-only projection recall succeeds");

    assert_eq!(memory.events().len(), event_count_before);
    assert_eq!(&replayed_projection, memory.data());
    assert_eq!(projection_report, report);
    assert!(
        projection_recall
            .results
            .iter()
            .filter_map(|result| result.trust.as_ref())
            .any(|trust| trust.mode == RecallResultTrustMode::Block && !trust.can_trust)
    );
    assert!(!report.write_back_policy.automatic_write_back);
    assert!(report.write_back_policy.requires_operator_review);
    assert!(report.summary.contradiction_count >= 1);
    assert!(report.summary.latent_intention_count >= 1);
    assert_eq!(report.summary.finding_count, report.findings.len());
    assert_eq!(report.summary.finding_count, report.review_queue.len());
    assert!(
        report
            .review_queue
            .iter()
            .any(|item| item.status == nahuali_core::SelfInspectionReviewStatus::Proposed)
    );

    let _ = fs::remove_file(path);
}

#[test]
fn public_api_operator_review_prioritizes_self_inspection_work() {
    let path = temp_store("public-api-operator-review");

    let mut memory = MemoryEngine::open(&path).expect("new store opens");
    memory
        .add_claim("Lena", "role", "CTO", None, 0.4)
        .expect("claim records");
    memory
        .add_claim("Lena", "role", "VP Engineering", None, 0.9)
        .expect("conflicting claim records");

    let report = memory.operator_review_with_options(OperatorReviewOptions {
        limit: 2,
        min_priority: Some(SelfInspectionReviewPriority::High),
        ..OperatorReviewOptions::default()
    });

    assert_eq!(report.version, OPERATOR_REVIEW_VERSION);
    assert!(!report.write_back_policy.automatic_write_back);
    assert!(report.write_back_policy.requires_operator_review);
    assert!(report.total_items >= 1);
    assert!(report.displayed_items <= 2);
    assert_eq!(
        report.items[0].priority,
        SelfInspectionReviewPriority::Critical
    );
    assert_eq!(
        report.items[0].action,
        SelfInspectionReviewAction::ResolveContradiction
    );
    assert!(
        report.items[0]
            .operator_guidance
            .contains("record the resolution")
    );
    assert_eq!(memory.events().len(), 2);

    let filtered = memory.operator_review_with_options(OperatorReviewOptions {
        limit: 10,
        action: Some(SelfInspectionReviewAction::ResolveContradiction),
        ..OperatorReviewOptions::default()
    });
    assert_eq!(
        filtered.action,
        Some(SelfInspectionReviewAction::ResolveContradiction)
    );
    assert!(filtered.items.iter().all(|item| {
        item.action == SelfInspectionReviewAction::ResolveContradiction
    }));

    let reopened = MemoryEngine::open(&path).expect("store reopens");
    assert_eq!(reopened.events().len(), 2);

    let _ = fs::remove_file(path);
}

#[test]
fn public_api_resolves_operator_review_item_explicitly() {
    let path = temp_store("public-api-review-resolution");

    let mut memory = MemoryEngine::open(&path).expect("new store opens");
    memory
        .remember("Lena owns the release notes.", vec!["product".to_string()])
        .expect("episode records");
    memory
        .add_claim("Lena", "role", "CTO", None, 0.92)
        .expect("unsupported claim records");

    let review = memory.operator_review(10);
    let item = review
        .items
        .iter()
        .find(|item| item.finding_kind == SelfInspectionFindingKind::WeakEvidence)
        .expect("unsupported claim creates capture-evidence review")
        .clone();
    assert_eq!(item.action, SelfInspectionReviewAction::CaptureEvidence);

    let dry_run = memory
        .resolve_review_item(
            item.id.clone(),
            "Operator confirmed this claim from external release ownership notes.",
            true,
        )
        .expect("dry-run resolution succeeds");
    assert!(dry_run.dry_run);
    assert!(!dry_run.applied);
    assert_eq!(dry_run.review_id.as_str(), item.id.as_str());
    assert_eq!(memory.events().len(), 2);
    assert!(memory.data().review_decisions.is_empty());
    assert_eq!(memory.inspect().unsupported_fact_count, 1);

    let applied = memory
        .resolve_review_item(
            item.id.clone(),
            "Operator confirmed this claim from external release ownership notes.",
            false,
        )
        .expect("review resolution applies");
    assert!(!applied.dry_run);
    assert!(applied.applied);
    assert_eq!(applied.review_id.as_str(), item.id.as_str());
    assert!(
        applied
            .event_id
            .as_deref()
            .unwrap_or_default()
            .starts_with("event_")
    );
    assert_eq!(memory.events().len(), 3);
    assert_eq!(memory.data().review_decisions.len(), 1);
    assert_eq!(
        memory.data().review_decisions[0].outcome,
        ReviewDecisionOutcome::Resolved
    );
    assert_eq!(memory.inspect().unsupported_fact_count, 0);
    assert!(
        memory
            .operator_review(10)
            .items
            .iter()
            .all(|review_item| review_item.id.as_str() != item.id.as_str())
    );

    let reopened = MemoryEngine::open(&path).expect("engine reopens");
    assert_eq!(reopened.data().review_decisions.len(), 1);
    assert_eq!(reopened.inspect().unsupported_fact_count, 0);

    let _ = fs::remove_file(path);
}
