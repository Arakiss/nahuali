#[test]
fn snapshot_dry_run_write_and_validate_are_scriptable() {
    let store = temp_database("snapshot-scriptable-store");
    let snapshot = temp_store("snapshot-scriptable-artifact");
    let snapshot_arg = snapshot.display().to_string();

    run_ok(&store, &["remember", "Lena owns the release notes"]);

    let dry_run_output = run_ok(
        &store,
        &["snapshot", "--output", &snapshot_arg, "--dry-run", "--json"],
    );
    let dry_run: Value = serde_json::from_str(&dry_run_output).expect("dry-run output is JSON");
    assert_eq!(dry_run["database"], store.display().to_string());
    assert_eq!(dry_run["snapshot_path"], snapshot_arg);
    assert_eq!(dry_run["dry_run"], true);
    assert_eq!(dry_run["written"], false);
    assert_eq!(dry_run["summary"]["event_count"], 1);
    assert!(!snapshot.exists());

    let write_output = run_ok(&store, &["snapshot", "--output", &snapshot_arg, "--json"]);
    let written: Value = serde_json::from_str(&write_output).expect("write output is JSON");
    assert_eq!(written["dry_run"], false);
    assert_eq!(written["written"], true);
    assert!(snapshot.exists());

    let validate_output = run_ok(&store, &["snapshot-validate", &snapshot_arg, "--json"]);
    let validation: Value =
        serde_json::from_str(&validate_output).expect("snapshot validation output is JSON");
    assert_eq!(validation["database"], store.display().to_string());
    assert_eq!(validation["snapshot_path"], snapshot_arg);
    assert_eq!(validation["valid"], true);
    assert_eq!(validation["checksum_valid"], true);
    assert_eq!(validation["replay_equivalent"], true);

    let maintenance_output = run_ok(&store, &["maintenance", "--json"]);
    let maintenance: Value =
        serde_json::from_str(&maintenance_output).expect("maintenance output is JSON");
    assert_eq!(maintenance["database"], store.display().to_string());
    assert_eq!(maintenance["report"]["event_count"], 1);
    assert_eq!(maintenance["report"]["snapshot_supported"], true);
    assert_eq!(maintenance["report"]["compaction_supported"], false);

    let _ = fs::remove_file(store);
    let _ = fs::remove_file(snapshot);
}

#[test]
fn snapshot_validate_reports_corrupt_snapshot_as_json() {
    let store = temp_database("snapshot-corrupt-store");
    let snapshot = temp_store("snapshot-corrupt-artifact");
    let snapshot_arg = snapshot.display().to_string();

    run_ok(&store, &["remember", "Lena owns the release notes"]);
    fs::write(&snapshot, "not valid snapshot JSON\n").expect("corrupt snapshot writes");

    let output = run(&store, &["snapshot-validate", &snapshot_arg, "--json"]);

    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");
    let report: Value = serde_json::from_str(&stdout).expect("snapshot validation output is JSON");
    assert_eq!(report["database"], store.display().to_string());
    assert_eq!(report["snapshot_path"], snapshot_arg);
    assert_eq!(report["valid"], false);
    assert_eq!(report["issues"][0]["kind"], "parse_error");

    let validate_output = run_ok(&store, &["validate", "--json"]);
    let validation: Value =
        serde_json::from_str(&validate_output).expect("record-ledger validation output is JSON");
    assert_eq!(validation["valid"], true);
    assert_eq!(validation["event_count"], 1);

    let _ = fs::remove_file(store);
    let _ = fs::remove_file(snapshot);
}

#[test]
fn backup_validate_and_restore_are_scriptable() {
    let source_store = temp_database("backup-source-store");
    let target_store = temp_database("backup-target-store");
    let backup_path = temp_store("backup-artifact");
    let backup_arg = backup_path.display().to_string();
    let target_arg = target_store.display().to_string();

    run_ok(
        &source_store,
        &[
            "remember",
            "Lena owns the release notes",
            "--tag",
            "product",
            "--mention",
            "Lena",
        ],
    );
    run_ok(
        &source_store,
        &["claim", "Lena", "owns", "release notes", "--source-last"],
    );

    let dry_run_output = run_ok(
        &source_store,
        &["backup", "--output", &backup_arg, "--dry-run", "--json"],
    );
    let dry_run: Value = serde_json::from_str(&dry_run_output).expect("dry-run output is JSON");
    assert_eq!(dry_run["database"], source_store.display().to_string());
    assert_eq!(dry_run["backup_path"], backup_arg);
    assert_eq!(dry_run["dry_run"], true);
    assert_eq!(dry_run["written"], false);
    assert_eq!(dry_run["summary"]["record_count"], 2);
    assert_eq!(
        dry_run["summary"]["semantic_tier"]["restore_policy"],
        "rebuild_from_records"
    );
    assert!(!backup_path.exists());

    let write_output = run_ok(
        &source_store,
        &["backup", "--output", &backup_arg, "--json"],
    );
    let written: Value = serde_json::from_str(&write_output).expect("write output is JSON");
    assert_eq!(written["written"], true);
    assert!(backup_path.exists());

    // Default backup validation is fail-closed: a store written by the default
    // (chained) CLI validates with `require_chained` true.
    let validate_output = run_ok(&source_store, &["backup-validate", &backup_arg, "--json"]);
    let validation: Value =
        serde_json::from_str(&validate_output).expect("backup validation output is JSON");
    assert_eq!(validation["backup_path"], backup_arg);
    assert_eq!(validation["valid"], true);
    assert_eq!(validation["checksum_valid"], true);
    assert_eq!(validation["records_valid"], true);
    assert_eq!(validation["chain_valid"], true);
    assert_eq!(validation["require_chained"], true);

    // The legacy-permissive escape hatch still validates a chained backup and
    // reports the relaxed posture.
    let permissive_validate_output = run_ok(
        &source_store,
        &["backup-validate", &backup_arg, "--allow-unchained", "--json"],
    );
    let permissive_validation: Value = serde_json::from_str(&permissive_validate_output)
        .expect("legacy-permissive backup validation is JSON");
    assert_eq!(permissive_validation["valid"], true);
    assert_eq!(permissive_validation["chain_valid"], true);
    assert_eq!(permissive_validation["require_chained"], false);

    // Default store validation is fail-closed too.
    let strict_store_output = run_ok(&source_store, &["validate", "--json"]);
    let strict_store: Value =
        serde_json::from_str(&strict_store_output).expect("default store validation is JSON");
    assert_eq!(strict_store["valid"], true);
    assert_eq!(strict_store["event_count"], 2);
    assert_eq!(strict_store["require_chained"], true);

    let drill_output = run_ok(
        &source_store,
        &[
            "backup-drill",
            &backup_arg,
            "--target-database",
            &target_arg,
            "--json",
        ],
    );
    let drill: Value = serde_json::from_str(&drill_output).expect("backup drill output is JSON");
    assert_eq!(drill["backup_path"], backup_arg);
    assert_eq!(drill["target_database"], target_arg);
    assert_eq!(drill["valid"], true);
    assert_eq!(drill["backup_validation"]["valid"], true);
    assert_eq!(drill["restore_dry_run"]["dry_run"], true);
    assert_eq!(drill["restore_dry_run"]["restored_event_count"], 0);
    assert_eq!(drill["restore_dry_run"]["target_was_empty"], true);
    assert_eq!(drill["semantic_rebuild_required"], true);
    assert!(
        drill["operator_next_actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|action| action
                .as_str()
                .unwrap_or_default()
                .contains("restore without --dry-run"))
    );

    let restore_dry_run_output = run_ok(
        &source_store,
        &[
            "restore",
            &backup_arg,
            "--target-database",
            &target_arg,
            "--dry-run",
            "--json",
        ],
    );
    let restore_dry_run: Value =
        serde_json::from_str(&restore_dry_run_output).expect("restore dry-run output is JSON");
    assert_eq!(restore_dry_run["valid"], true);
    assert_eq!(restore_dry_run["dry_run"], true);
    assert_eq!(restore_dry_run["appendable_event_count"], 2);
    assert_eq!(restore_dry_run["restored_event_count"], 0);
    assert_eq!(restore_dry_run["graph_projection_rebuilt"], false);
    assert_eq!(restore_dry_run["semantic_rebuild_required"], true);
    assert_eq!(restore_dry_run["operationally_ready"], false);

    let empty_validation = run_ok_at_endpoint(&target_store, &source_store, &["validate", "--json"]);
    let empty: Value = serde_json::from_str(&empty_validation).expect("validation output is JSON");
    assert_eq!(empty["event_count"], 0);

    let restore_output = run_ok(
        &source_store,
        &[
            "restore",
            &backup_arg,
            "--target-database",
            &target_arg,
            "--json",
        ],
    );
    let restored: Value = serde_json::from_str(&restore_output).expect("restore output is JSON");
    assert_eq!(restored["valid"], true);
    assert_eq!(restored["restored_event_count"], 2);
    assert_eq!(restored["semantic_restore_policy"], "rebuild_from_records");
    assert_eq!(restored["graph_projection_rebuilt"], true);
    assert_eq!(restored["graph_projection_valid"], true);
    assert_eq!(restored["semantic_rebuild_required"], true);
    assert_eq!(restored["semantic_rebuild_completed"], false);
    assert_eq!(restored["operationally_ready"], false);

    let target_validation =
        run_ok_at_endpoint(&target_store, &source_store, &["validate", "--json"]);
    let target: Value =
        serde_json::from_str(&target_validation).expect("target validation output is JSON");
    assert_eq!(target["event_count"], 2);
    assert_eq!(target["episode_count"], 1);
    assert_eq!(target["claim_count"], 1);

    let blocked_output = run(
        &source_store,
        &[
            "restore",
            &backup_arg,
            "--target-database",
            &target_arg,
            "--json",
        ],
    );
    assert!(!blocked_output.status.success());
    let stdout = String::from_utf8(blocked_output.stdout).expect("stdout is UTF-8");
    let blocked: Value = serde_json::from_str(&stdout).expect("blocked output is JSON");
    assert_eq!(blocked["valid"], false);
    assert_eq!(blocked["issues"][0]["kind"], "target_not_empty");

    let _ = fs::remove_file(source_store);
    let _ = fs::remove_file(target_store);
    let _ = fs::remove_file(backup_path);
}

#[test]
fn restore_can_rebuild_every_derived_tier_in_one_operation() {
    let source_store = temp_database("complete-restore-source");
    let target_store = temp_database("complete-restore-target");
    let backup_path = temp_store("complete-restore-backup");
    let backup_arg = backup_path.display().to_string();
    let target_arg = target_store.display().to_string();
    let collection_guard = QdrantCollectionGuard::new("complete_restore");
    let collection = collection_guard.name();

    let probe = run_with_semantic_collection(
        &source_store,
        &["semantic-status", "--json"],
        collection,
    );
    if !probe.status.success() {
        let _ = fs::remove_file(backup_path);
        return;
    }

    run_ok(
        &source_store,
        &[
            "remember",
            "Hrafn verifies complete restore readiness",
            "--mention",
            "Hrafn",
        ],
    );
    run_ok(
        &source_store,
        &["backup", "--output", &backup_arg, "--json"],
    );
    let restored = run_ok_with_semantic_collection(
        &source_store,
        &[
            "restore",
            &backup_arg,
            "--target-database",
            &target_arg,
            "--rebuild-semantic",
            "--json",
        ],
        collection,
    );
    let report: Value = serde_json::from_str(&restored).expect("restore report is JSON");
    assert_eq!(report["valid"], true);
    assert_eq!(report["graph_projection_rebuilt"], true);
    assert_eq!(report["graph_projection_valid"], true);
    assert_eq!(report["semantic_rebuild_required"], false);
    assert_eq!(report["semantic_rebuild_completed"], true);
    assert_eq!(report["semantic_index_current"], true);
    assert_eq!(report["operationally_ready"], true);

    let target_validation =
        run_ok_at_endpoint(&target_store, &source_store, &["projection-validate", "--json"]);
    let graph: Value =
        serde_json::from_str(&target_validation).expect("projection validation is JSON");
    assert_eq!(graph["validation"]["valid"], true);

    let _ = fs::remove_file(backup_path);
}

#[test]
fn authority_json_reports_blocking_health_contract() {
    let store = temp_database("authority-json-contract");

    run_ok(
        &store,
        &["fact", "Atlas", "status", "draft", "--confidence", "0.4"],
    );

    let output = run_ok(&store, &["recall", "Atlas status", "--authority", "--json"]);
    let recall: Value = serde_json::from_str(&output).expect("authority recall output is JSON");

    assert_eq!(recall["authority"]["mode"], "block");
    assert_eq!(recall["authority"]["score"], 0.0);
    assert_eq!(recall["authority"]["can_trust"], false);
    assert_eq!(
        recall["authority"]["signal_kinds"],
        serde_json::json!([
            "no_episodes",
            "unsupported_fact",
            "low_confidence_fact",
            "isolated_entity"
        ])
    );
    assert!(
        recall["authority"]["reasons"][0]
            .as_str()
            .unwrap_or_default()
            .contains("No episodes")
    );
    let unsupported_result = recall["results"]
        .as_array()
        .expect("recall results are an array")
        .iter()
        .find(|result| result["kind"] == "claim")
        .expect("unsupported claim result is returned");
    assert_eq!(unsupported_result["trust"]["mode"], "warn");
    assert_eq!(unsupported_result["trust"]["can_trust"], false);
    assert_eq!(
        unsupported_result["trust"]["signal_kinds"],
        serde_json::json!(["unsupported_fact", "low_confidence_fact"])
    );

    let signals = recall["health"]["signals"]
        .as_array()
        .expect("health signals are an array");
    let unsupported = signals
        .iter()
        .find(|signal| signal["kind"] == "unsupported_fact")
        .expect("unsupported fact signal is present");
    assert_eq!(unsupported["severity"], "medium");
    assert_eq!(
        unsupported["dimensions"],
        serde_json::json!(["unsupported_memory", "blind_spot"])
    );
    assert!(
        unsupported["evidence_ids"]
            .as_array()
            .expect("evidence ids are an array")
            .iter()
            .any(|id| id.as_str().unwrap_or_default().starts_with("event_"))
    );

    let projection_health = run_ok(&store, &["projection-health", "--json"]);
    let projection_health: Value =
        serde_json::from_str(&projection_health).expect("projection health output is JSON");
    assert!(
        projection_health["signals"]
            .as_array()
            .unwrap()
            .iter()
            .any(|signal| signal["signal_kind"] == "unsupported_fact")
    );

    let _ = fs::remove_file(store);
}

#[test]
fn json_output_is_scriptable_for_primary_commands() {
    let store = temp_database("json-output-contract");

    let episode_output = run_ok(
        &store,
        &[
            "remember",
            "Lena owns the release notes",
            "--tag",
            "product",
            "--json",
        ],
    );
    assert_pretty_json(&episode_output);
    let episode: Value = serde_json::from_str(&episode_output).expect("episode output is JSON");
    let episode_id = episode["id"]
        .as_str()
        .expect("episode JSON includes an id")
        .to_string();
    assert!(episode_id.starts_with("episode_"));
    assert_eq!(episode["content"], "Lena owns the release notes");
    assert_eq!(episode["tags"], serde_json::json!(["product"]));

    let fact_output = run_ok(
        &store,
        &[
            "fact",
            "Lena",
            "owns",
            "release notes",
            "--confidence",
            "0.92",
            "--source-last",
            "--json",
        ],
    );
    assert_pretty_json(&fact_output);
    let fact: Value = serde_json::from_str(&fact_output).expect("fact output is JSON");
    assert!(fact["id"].as_str().unwrap_or_default().starts_with("fact_"));
    assert_eq!(fact["source_episode_id"], episode_id);
    assert_eq!(fact["confidence"], 0.92);

    let relation_output = run_ok(
        &store,
        &[
            "relate",
            "Lena",
            "owns",
            "release notes",
            "--confidence",
            "0.9",
            "--source-last",
            "--json",
        ],
    );
    assert_pretty_json(&relation_output);
    let relation: Value = serde_json::from_str(&relation_output).expect("relation output is JSON");
    assert!(
        relation["id"]
            .as_str()
            .unwrap_or_default()
            .starts_with("relation_")
    );
    assert_eq!(relation["source_episode_id"], episode_id);
    assert_eq!(relation["confidence"], 0.9);

    let recall_output = run_ok(&store, &["recall", "Lena release", "--json"]);
    assert_pretty_json(&recall_output);
    let recall_results: Value =
        serde_json::from_str(&recall_output).expect("recall output is JSON");
    let first = recall_results
        .as_array()
        .and_then(|results| results.first())
        .expect("recall JSON includes at least one result");
    assert_eq!(first["kind"], "claim");
    assert_eq!(first["evidence_id"], episode_id);

    let _ = fs::remove_file(store);
}
