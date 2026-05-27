#[test]
fn public_api_builds_text_ingest_document() {
    let report = build_text_ingest_document(
        "First observation.\n\nSecond observation.",
        TextIngestOptions {
            source_kind: SourceKind::Note,
            title: Some("Daily notes".to_string()),
            uri: Some("file:///notes/daily.md".to_string()),
            tags: vec!["journal".to_string()],
            mentions: vec!["Lena".to_string()],
            chunking: TextChunking::Paragraphs,
            max_chunk_bytes: DEFAULT_TEXT_CHUNK_BYTES,
            ..TextIngestOptions::default()
        },
    );

    assert!(report.valid);
    assert_eq!(report.version, TEXT_INGEST_ADAPTER_VERSION);
    assert_eq!(report.source_byte_len, 39);
    assert_eq!(report.episode_count, 2);
    let document = report.document.expect("document is generated");
    assert_eq!(document.version, MEMORY_INGEST_DOCUMENT_VERSION);
    assert_eq!(document.source.kind, SourceKind::Note);
    assert_eq!(document.source.title.as_deref(), Some("Daily notes"));
    assert_eq!(document.episodes[0].ref_id.as_deref(), Some("chunk-1"));
    assert_eq!(document.episodes[1].source_position, Some(2));
    assert_eq!(document.episodes[0].tags, vec!["journal"]);
    assert_eq!(document.episodes[0].mentions, vec!["Lena"]);
    assert!(document.claims.is_empty());
    assert!(document.links.is_empty());
    assert!(document.procedures.is_empty());
    assert!(document.intentions.is_empty());

    let invalid = build_text_ingest_document(
        "",
        TextIngestOptions {
            title: Some(" ".to_string()),
            ..TextIngestOptions::default()
        },
    );
    assert!(!invalid.valid);
    assert!(invalid.document.is_none());
    assert_eq!(invalid.issues[0].kind, TextIngestIssueKind::EmptyContent);
}

#[test]
fn public_api_returns_session_briefing() {
    let path = temp_store("public-api-session-briefing");

    let mut memory = MemoryEngine::open(&path).expect("new store opens");
    let source = memory
        .record_source(
            SourceKind::Conversation,
            Some("Release review".to_string()),
            Some("fixture://release-review".to_string()),
            "fnv1a64:briefing",
            64,
            BTreeMap::new(),
        )
        .expect("source records");
    let episode = memory
        .remember_source_episode(
            "Lena owns the release notes.",
            vec!["product".to_string()],
            vec!["Lena".to_string(), "Release Notes".to_string()],
            source.id.clone(),
            Some(1),
            Some("user".to_string()),
        )
        .expect("source episode records");
    memory
        .add_claim(
            "Lena",
            "owns",
            "release notes",
            Some(episode.id.clone()),
            0.93,
        )
        .expect("claim records");
    memory
        .add_claim("Lena", "owns", "changelog", Some(episode.id.clone()), 0.91)
        .expect("conflicting claim records");
    memory
        .add_link(
            "Lena",
            "owns",
            "Release Notes",
            Some(episode.id.clone()),
            0.9,
        )
        .expect("link records");
    memory
        .add_intention(
            "Ship release notes",
            IntentionKind::Task,
            IntentionPriority::High,
            Some(episode.id.clone()),
        )
        .expect("intention records");
    let before_events = memory.events().len();

    let briefing = memory.briefing_with_options(BriefingOptions {
        episode_limit: 1,
        intention_limit: 2,
        review_limit: 3,
        graph_seed_limit: 2,
    });

    assert_eq!(briefing.version, MEMORY_BRIEFING_VERSION);
    assert_eq!(briefing.event_count, before_events);
    assert_eq!(briefing.summary.source_count, 1);
    assert_eq!(briefing.summary.episode_count, 1);
    assert_eq!(briefing.summary.active_intention_count, 1);
    assert_eq!(briefing.summary.high_priority_review_count, 1);
    assert_eq!(briefing.summary.critical_review_count, 1);
    assert_eq!(briefing.authority.mode, AuthorityMode::Block);
    assert_eq!(briefing.health.conflicting_fact_count, 1);
    assert_eq!(briefing.recent_episodes.len(), 1);
    assert_eq!(
        briefing.recent_episodes[0].source_id.as_deref(),
        Some(source.id.as_str())
    );
    assert_eq!(
        briefing.active_intentions[0].description,
        "Ship release notes"
    );
    assert_eq!(
        briefing.review_items[0].priority,
        SelfInspectionReviewPriority::Critical
    );
    assert!(briefing.graph_seeds.iter().any(|seed| seed.label == "Lena"));
    assert_eq!(memory.events().len(), before_events);

    let _ = fs::remove_file(path);
}

#[test]
fn public_api_returns_reflection_cycle() {
    let path = temp_store("public-api-reflection-cycle");

    let mut memory = MemoryEngine::open(&path).expect("new store opens");
    let source = memory
        .record_source(
            SourceKind::Conversation,
            Some("Release reflection".to_string()),
            Some("fixture://release-reflection".to_string()),
            "fnv1a64:reflection",
            64,
            BTreeMap::new(),
        )
        .expect("source records");
    let episode = memory
        .remember_source_episode(
            "Lena owns the release notes.",
            vec!["product".to_string()],
            vec!["Lena".to_string(), "Release Notes".to_string()],
            source.id,
            Some(1),
            Some("user".to_string()),
        )
        .expect("source episode records");
    memory
        .add_claim(
            "Lena",
            "owns",
            "release notes",
            Some(episode.id.clone()),
            0.93,
        )
        .expect("claim records");
    memory
        .add_claim("Lena", "owns", "changelog", Some(episode.id.clone()), 0.91)
        .expect("conflicting claim records");
    memory
        .add_intention(
            "Ship release notes",
            IntentionKind::Task,
            IntentionPriority::High,
            None,
        )
        .expect("unsupported intention records");
    let before_events = memory.events().len();

    let reflection = memory.reflect_with_options(ReflectionOptions {
        cycle_limit: 4,
        evidence_limit: 4,
    });

    assert_eq!(reflection.version, MEMORY_REFLECTION_VERSION);
    assert_eq!(reflection.event_count, before_events);
    assert_eq!(reflection.authority.mode, AuthorityMode::Block);
    assert!(reflection.summary.finding_count >= 2);
    assert!(reflection.summary.total_cycle_count >= 2);
    assert_eq!(reflection.summary.critical_cycle_count, 1);
    assert!(!reflection.write_back_policy.automatic_write_back);
    assert!(reflection.write_back_policy.requires_operator_review);
    assert_eq!(reflection.source_coverage.source_count, 1);
    assert_eq!(reflection.source_coverage.sourced_episode_count, 1);
    assert_eq!(reflection.source_coverage.unsupported_memory_count, 1);
    assert_eq!(reflection.source_coverage.evidence_coverage_ratio, 0.67);
    assert_eq!(
        reflection.cycles[0].action,
        SelfInspectionReviewAction::ResolveContradiction
    );
    assert_eq!(
        reflection.cycles[0].priority,
        SelfInspectionReviewPriority::Critical
    );
    assert!(!reflection.cycles[0].evidence_ids.is_empty());
    assert_eq!(memory.events().len(), before_events);

    let _ = fs::remove_file(path);
}

#[test]
fn public_api_rebuilds_qdrant_semantic_index_and_hybrid_recall() {
    let path = temp_store("public-api-semantic-index");
    let config = semantic_test_config("public_api_semantic_index");

    let mut memory = MemoryEngine::open(&path).expect("new store opens");
    if memory.semantic_index_status_with_config(&config).is_err() {
        let _ = fs::remove_file(path);
        return;
    }

    let episode = memory
        .remember_with_mentions(
            "Lena owns the release notes and keeps the changelog concise.",
            vec!["product".to_string()],
            vec!["Lena".to_string(), "Release Notes".to_string()],
        )
        .expect("episode records");
    memory
        .add_claim(
            "Lena",
            "owns",
            "release notes",
            Some(episode.id.clone()),
            0.93,
        )
        .expect("claim records");
    memory
        .add_link(
            "Lena",
            "owns",
            "Release Notes",
            Some(episode.id.clone()),
            0.91,
        )
        .expect("link records");
    memory
        .add_preference(
            "Release notes",
            "Keep release notes concise.",
            Some(episode.id),
            0.88,
        )
        .expect("preference records");

    let first_report = memory
        .rebuild_semantic_index_with_config(&config)
        .expect("semantic index rebuilds");
    assert_eq!(first_report.collection_name, config.collection_name);
    assert_eq!(first_report.source_event_count, memory.data().event_count);
    assert_eq!(first_report.indexed_point_count, first_report.points.len());
    assert!(first_report.indexed_point_count >= 5);

    let status = memory
        .semantic_index_status_with_config(&config)
        .expect("semantic status reads");
    assert!(status.collection_exists);
    assert_eq!(status.point_count, first_report.indexed_point_count);

    let hybrid = memory
        .hybrid_recall_with_config("release notes", 10, &config)
        .expect("hybrid recall succeeds");
    assert_eq!(hybrid.collection_name, config.collection_name);
    assert!(!hybrid.lexical_results.is_empty());
    assert!(!hybrid.semantic_results.is_empty());
    assert!(
        hybrid
            .results
            .iter()
            .any(|result| result.semantic_score.is_some())
    );
    assert!(hybrid.results.iter().any(|result| {
        result
            .explanations
            .iter()
            .any(|note| note.contains("authority="))
    }));

    let second_report = memory
        .rebuild_semantic_index_with_config(&config)
        .expect("semantic index rebuild is idempotent");
    assert!(second_report.deleted_existing_collection);
    assert_eq!(
        second_report.indexed_point_count,
        first_report.indexed_point_count
    );

    let _ = fs::remove_file(path);
}

#[test]
fn public_api_filters_qdrant_semantic_recall_by_scope_and_kind() {
    let path = temp_store("public-api-semantic-scope-filter");
    let config = semantic_test_config("public_api_semantic_scope_filter");
    let nahuali_scope = MemoryScope::new(MemoryScopeKind::Project, "Nahuali").unwrap();
    let other_scope = MemoryScope::new(MemoryScopeKind::Project, "Other").unwrap();

    let mut memory = MemoryEngine::open(&path).expect("new store opens");
    if memory.semantic_index_status_with_config(&config).is_err() {
        let _ = fs::remove_file(path);
        return;
    }

    let nahuali_episode = memory
        .remember_with_mentions_scoped(
            "Lena owns Nahuali release notes.",
            vec!["product".to_string()],
            vec!["Lena".to_string(), "Release Notes".to_string()],
            nahuali_scope.clone(),
        )
        .expect("scoped episode records");
    let nahuali_claim = memory
        .add_claim_scoped(
            "Lena",
            "owns",
            "Nahuali release notes",
            Some(nahuali_episode.id),
            0.93,
            nahuali_scope.clone(),
        )
        .expect("scoped claim records");
    let other_episode = memory
        .remember_with_mentions_scoped(
            "Lena owns other release notes.",
            vec!["product".to_string()],
            vec!["Lena".to_string(), "Release Notes".to_string()],
            other_scope.clone(),
        )
        .expect("other scoped episode records");
    let other_claim = memory
        .add_claim_scoped(
            "Lena",
            "owns",
            "other release notes",
            Some(other_episode.id),
            0.91,
            other_scope,
        )
        .expect("other scoped claim records");

    memory
        .rebuild_semantic_index_with_config(&config)
        .expect("semantic index rebuilds");
    let scoped = memory
        .hybrid_recall_with_options_and_config(
            "release notes",
            RecallOptions {
                limit: 10,
                scope: Some(nahuali_scope.clone()),
                kinds: vec![MemoryKind::Claim],
                require_evidence: true,
            },
            &config,
        )
        .expect("scoped semantic recall succeeds");

    assert!(
        scoped
            .semantic_results
            .iter()
            .all(|result| result.scope_key.as_deref() == Some(nahuali_scope.key.as_str()))
    );
    assert!(scoped.results.iter().any(|result| result.id == nahuali_claim.id));
    assert!(!scoped.results.iter().any(|result| result.id == other_claim.id));

    let _ = fs::remove_file(path);
}
