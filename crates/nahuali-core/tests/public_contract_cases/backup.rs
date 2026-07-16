#[test]
fn public_api_returns_graph_neighborhood() {
    let path = temp_store("public-api-graph-neighborhood");

    let mut memory = MemoryEngine::open(&path).expect("new store opens");
    let episode = memory
        .remember_with_mentions(
            "Lena owns the release notes.",
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
            0.92,
        )
        .expect("claim records");
    memory
        .add_link("Lena", "owns", "Release Notes", Some(episode.id), 0.9)
        .expect("link records");

    let report = memory
        .graph_neighborhood("Lena", 2, 20)
        .expect("graph traversal succeeds");

    assert_eq!(report.version, MEMORY_GRAPH_VERSION);
    assert_eq!(report.seed, "Lena");
    assert!(report.summary.node_count >= 4);
    assert!(report.summary.edge_count >= 3);
    assert!(
        report
            .nodes
            .iter()
            .any(|node| node.kind == MemoryGraphNodeKind::Entity && node.label == "Lena")
    );
    assert!(
        report
            .nodes
            .iter()
            .any(|node| node.kind == MemoryGraphNodeKind::Claim)
    );
    assert!(
        report
            .edges
            .iter()
            .any(|edge| edge.kind == MemoryGraphEdgeKind::Relation)
    );
    assert!(report.nodes.iter().all(|node| node.depth <= 2));

    let _ = fs::remove_file(path);
}

#[test]
fn public_api_returns_project_view() {
    let path = temp_store("public-api-project-view");

    let mut memory = MemoryEngine::open(&path).expect("new store opens");
    let episode = memory
        .remember_with_mentions(
            "Lena owns the release notes.",
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
            0.92,
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
            "Release notes style",
            "Keep release notes concise.",
            Some(episode.id.clone()),
            0.9,
        )
        .expect("preference records");
    memory
        .add_intention(
            "Ask Lena to publish release notes",
            IntentionKind::Task,
            IntentionPriority::High,
            Some(episode.id),
        )
        .expect("intention records");

    let report = memory
        .project_view_with_options(
            "Lena",
            ProjectViewOptions {
                graph_depth: 2,
                graph_limit: 20,
                item_limit: 5,
                recall_limit: 5,
                review_limit: 5,
            },
        )
        .expect("project view succeeds");

    assert_eq!(report.version, MEMORY_PROJECT_VIEW_VERSION);
    assert_eq!(report.query, "Lena");
    assert_eq!(report.matched_entity.as_ref().unwrap().name, "Lena");
    assert!(report.summary.matched_entity);
    assert_eq!(report.summary.claim_count, 1);
    assert_eq!(report.summary.link_count, 1);
    assert_eq!(report.summary.procedure_count, 1);
    assert_eq!(report.summary.intention_count, 1);
    assert!(report.summary.graph_node_count >= 4);
    assert!(
        report
            .recall_results
            .iter()
            .any(|result| result.kind == MemoryKind::Claim)
    );

    let no_match = memory.project_view("Unknown").expect("no-match view succeeds");
    assert!(no_match.matched_entity.is_none());
    assert!(!no_match.summary.matched_entity);
    assert_eq!(no_match.summary.graph_node_count, 0);

    let _ = fs::remove_file(path);
}

#[test]
fn public_api_clamps_fact_and_relation_confidence() {
    let path = temp_store("public-api-confidence-clamping");

    let mut memory = MemoryEngine::open(&path).expect("new store opens");
    let fact = memory
        .add_fact("Lena", "owns", "release notes", None, 2.0)
        .expect("fact records");
    let relation = memory
        .relate("Lena", "owns", "release notes", None, -1.0)
        .expect("relation records");

    assert_eq!(fact.confidence, 1.0);
    assert_eq!(relation.confidence, 0.0);
    assert_eq!(memory.data().facts[0].confidence, 1.0);
    assert_eq!(memory.data().relations[0].confidence, 0.0);

    let _ = fs::remove_file(path);
}

#[test]
fn public_api_writes_and_validates_optional_snapshot() {
    let path = temp_store("public-api-snapshot-store");
    let snapshot_path = temp_store("public-api-snapshot-valid");

    let mut memory = MemoryEngine::open(&path).expect("new store opens");
    memory
        .remember("Lena owns the release notes.", vec!["product".to_string()])
        .expect("episode records");

    let maintenance = memory.maintenance_report();
    assert!(maintenance.snapshot_supported);
    assert!(maintenance.snapshot_recommended);
    assert!(!maintenance.compaction_supported);

    let snapshot = memory
        .write_snapshot(&snapshot_path)
        .expect("snapshot writes");
    assert!(snapshot.checksum_valid());
    assert_eq!(snapshot.event_count, 1);
    assert_eq!(snapshot.data, memory.data().clone());

    let report = memory
        .validate_snapshot(&snapshot_path)
        .expect("snapshot validates");
    assert!(report.valid);
    assert!(report.checksum_valid);
    assert!(report.replay_equivalent);
    assert_eq!(report.snapshot_event_count, Some(1));
    assert_eq!(report.current_event_count, 1);

    let reopened = MemoryEngine::open(&path).expect("store still opens from record ledger");
    assert_eq!(reopened.data().event_count, 1);

    let _ = fs::remove_file(path);
    let _ = fs::remove_file(snapshot_path);
}

#[test]
fn public_api_rejects_stale_snapshot_without_touching_record_ledger() {
    let path = temp_store("public-api-stale-snapshot-store");
    let snapshot_path = temp_store("public-api-stale-snapshot");

    let mut memory = MemoryEngine::open(&path).expect("new store opens");
    memory
        .remember("Lena owns the release notes.", Vec::new())
        .expect("episode records");
    memory
        .write_snapshot(&snapshot_path)
        .expect("snapshot writes");
    memory
        .remember("Lena also owns the changelog.", Vec::new())
        .expect("second episode records");

    let report = memory
        .validate_snapshot(&snapshot_path)
        .expect("snapshot validation reports mismatch");
    assert!(!report.valid);
    assert!(report.checksum_valid);
    assert!(!report.replay_equivalent);
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == SnapshotIssueKind::RecordLedgerMismatch)
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == SnapshotIssueKind::ReplayMismatch)
    );

    let reopened = MemoryEngine::open(&path).expect("record ledger remains valid");
    assert_eq!(reopened.data().event_count, 2);

    let _ = fs::remove_file(path);
    let _ = fs::remove_file(snapshot_path);
}

#[test]
fn public_api_reports_corrupt_snapshot_without_touching_record_ledger() {
    let path = temp_store("public-api-corrupt-snapshot-store");
    let snapshot_path = temp_store("public-api-corrupt-snapshot");

    let mut memory = MemoryEngine::open(&path).expect("new store opens");
    memory
        .remember("Lena owns the release notes.", Vec::new())
        .expect("episode records");
    fs::write(&snapshot_path, "not valid snapshot JSON\n").expect("corrupt snapshot writes");

    let report = memory
        .validate_snapshot(&snapshot_path)
        .expect("corrupt snapshot returns validation report");
    assert!(!report.valid);
    assert!(!report.checksum_valid);
    assert!(!report.replay_equivalent);
    assert_eq!(report.issues[0].kind, SnapshotIssueKind::ParseError);

    let reopened = MemoryEngine::open(&path).expect("record ledger remains valid");
    assert_eq!(reopened.data().event_count, 1);

    let _ = fs::remove_file(path);
    let _ = fs::remove_file(snapshot_path);
}

#[test]
fn public_api_writes_validates_and_restores_local_backup() {
    let source_path = temp_store("public-api-backup-source");
    let target_path = temp_store("public-api-backup-target");
    let backup_path = temp_store("public-api-backup-artifact");

    let mut source = MemoryEngine::open(&source_path).expect("source store opens");
    let episode = source
        .remember_with_mentions(
            "Lena owns the release notes.",
            vec!["product".to_string()],
            vec!["Lena".to_string()],
        )
        .expect("episode records");
    source
        .add_claim(
            "Lena",
            "owns",
            "release notes",
            Some(episode.id.clone()),
            0.92,
        )
        .expect("claim records");
    source
        .add_link("Lena", "owns", "Release Notes", Some(episode.id), 0.9)
        .expect("link records");

    let backup = source.write_backup(&backup_path).expect("backup writes");

    assert_eq!(backup.version, MEMORY_BACKUP_VERSION);
    assert!(backup.checksum_valid());
    assert_eq!(backup.record_count, 3);
    assert_eq!(backup.records, source.events());
    assert_eq!(backup.semantic_tier.provider, SemanticTierProvider::Qdrant);
    assert!(backup.semantic_tier.derived);
    assert_eq!(backup.semantic_tier.collections.len(), 1);
    assert_ne!(
        backup.semantic_tier.collections[0],
        DEFAULT_SEMANTIC_COLLECTION
    );
    assert!(
        backup.semantic_tier.collections[0]
            .starts_with(&format!("{DEFAULT_SEMANTIC_COLLECTION}__"))
    );
    assert_eq!(
        backup.semantic_tier.snapshot_status,
        SemanticTierSnapshotStatus::NotIncluded
    );
    assert_eq!(
        backup.semantic_tier.restore_policy,
        SemanticTierRestorePolicy::RebuildFromRecords
    );

    let validation = MemoryEngine::validate_backup(&backup_path).expect("backup validates");
    assert!(validation.valid);
    assert!(validation.checksum_valid);
    assert!(validation.records_valid);
    assert_eq!(validation.backup_record_count, Some(3));

    let dry_run =
        MemoryEngine::restore_backup(&backup_path, &target_path, true).expect("dry-run succeeds");
    assert!(dry_run.valid);
    assert!(dry_run.dry_run);
    assert_eq!(dry_run.appendable_event_count, 3);
    assert_eq!(dry_run.restored_event_count, 0);
    assert!(dry_run.target_was_empty);
    assert!(!dry_run.graph_projection_rebuilt);
    assert!(!dry_run.graph_projection_valid);
    assert!(dry_run.semantic_rebuild_required);
    assert!(!dry_run.semantic_rebuild_completed);
    assert_eq!(dry_run.semantic_index_current, None);
    assert!(!dry_run.operationally_ready);

    let empty_target = MemoryEngine::open(&target_path).expect("target remains empty");
    assert_eq!(empty_target.events().len(), 0);

    let restored =
        MemoryEngine::restore_backup(&backup_path, &target_path, false).expect("restore succeeds");
    assert!(restored.valid);
    assert_eq!(restored.restored_event_count, 3);
    assert!(restored.graph_projection_rebuilt);
    assert!(restored.graph_projection_valid);
    assert!(restored.semantic_rebuild_required);
    assert!(!restored.semantic_rebuild_completed);
    assert_eq!(restored.semantic_index_current, None);
    assert!(!restored.operationally_ready);

    let reopened = MemoryEngine::open(&target_path).expect("restored target opens");
    assert_eq!(reopened.events(), source.events());
    assert_eq!(reopened.data(), source.data());
    assert!(reopened.projection_validate().expect("graph validates").valid);

    let blocked = MemoryEngine::restore_backup(&backup_path, &target_path, false)
        .expect("non-empty restore reports validation failure");
    assert!(!blocked.valid);
    assert_eq!(blocked.restored_event_count, 0);
    assert!(
        blocked
            .issues
            .iter()
            .any(|issue| issue.kind == BackupIssueKind::TargetNotEmpty)
    );

    let _ = fs::remove_file(source_path);
    let _ = fs::remove_file(target_path);
    let _ = fs::remove_file(backup_path);
}

#[test]
fn public_api_runs_non_mutating_backup_drill() {
    let source_path = temp_store("public-api-backup-drill-source");
    let target_path = temp_store("public-api-backup-drill-target");
    let backup_path = temp_store("public-api-backup-drill-file");

    let mut source = MemoryEngine::open(&source_path).expect("source opens");
    source
        .remember("Lena owns the release notes.", vec!["product".to_string()])
        .expect("episode records");
    source.write_backup(&backup_path).expect("backup writes");

    let drill = MemoryEngine::backup_drill(&backup_path, &target_path).expect("drill runs");

    assert!(drill.valid);
    assert_eq!(drill.backup_path, backup_path.display().to_string());
    assert_eq!(drill.target_database, target_path.display().to_string());
    assert!(drill.backup_validation.valid);
    assert!(drill.restore_dry_run.valid);
    assert!(drill.restore_dry_run.dry_run);
    assert_eq!(drill.restore_dry_run.restored_event_count, 0);
    assert!(drill.restore_dry_run.target_was_empty);
    assert!(drill.semantic_rebuild_required);
    assert!(
        drill
            .operator_next_actions
            .iter()
            .any(|action| action.contains("restore without --dry-run"))
    );

    let target = MemoryEngine::open(&target_path).expect("target opens after dry-run");
    assert_eq!(target.events().len(), 0);

    let _ = fs::remove_file(source_path);
    let _ = fs::remove_file(target_path);
    let _ = fs::remove_file(backup_path);
}

#[test]
fn public_api_reports_corrupt_backup_without_touching_target() {
    let target_path = temp_store("public-api-corrupt-backup-target");
    let backup_path = temp_store("public-api-corrupt-backup-artifact");
    fs::write(&backup_path, "not valid backup JSON\n").expect("corrupt backup writes");

    let validation = MemoryEngine::validate_backup(&backup_path)
        .expect("corrupt backup returns validation report");
    assert!(!validation.valid);
    assert_eq!(validation.issues[0].kind, BackupIssueKind::ParseError);

    let restore =
        MemoryEngine::restore_backup(&backup_path, &target_path, false).expect("restore reports");
    assert!(!restore.valid);
    assert_eq!(restore.restored_event_count, 0);

    let target = MemoryEngine::open(&target_path).expect("target remains empty");
    assert_eq!(target.events().len(), 0);

    let _ = fs::remove_file(target_path);
    let _ = fs::remove_file(backup_path);
}

#[test]
fn public_api_accepts_relative_database_and_snapshot_paths() {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after epoch")
        .as_nanos();
    let path = PathBuf::from(format!(
        "nahuali-core-relative-store-{}-{nanos}",
        std::process::id()
    ));
    let snapshot_path = PathBuf::from(format!(
        "nahuali-core-relative-snapshot-{}-{nanos}.json",
        std::process::id()
    ));
    let _ = fs::remove_file(&path);
    let _ = fs::remove_file(&snapshot_path);

    let mut memory = MemoryEngine::open(&path).expect("relative store opens");
    memory
        .remember("Lena owns the release notes.", Vec::new())
        .expect("relative store writes");
    memory
        .write_snapshot(&snapshot_path)
        .expect("relative snapshot writes");

    assert!(snapshot_path.exists());
    assert!(memory.validate_snapshot(&snapshot_path).unwrap().valid);

    let _ = fs::remove_file(path);
    let _ = fs::remove_file(snapshot_path);
}

#[test]
fn public_api_rejects_empty_inputs() {
    let path = temp_store("public-api-empty-inputs");

    let mut memory = MemoryEngine::open(&path).expect("new store opens");

    assert!(matches!(
        memory.remember("   ", Vec::new()),
        Err(NahualiError::EmptyContent)
    ));
    assert!(matches!(
        memory.add_fact("Lena", " ", "release notes", None, 0.8),
        Err(NahualiError::EmptyContent)
    ));
    assert!(matches!(
        memory.relate("Lena", "owns", " ", None, 0.8),
        Err(NahualiError::EmptyContent)
    ));
    assert!(matches!(
        memory.recall("   ", 10),
        Err(NahualiError::EmptyQuery)
    ));

    let _ = fs::remove_file(path);
}
