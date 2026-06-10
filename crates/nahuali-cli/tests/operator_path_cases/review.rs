#[test]
fn semantic_index_operator_path_is_scriptable() {
    let store = temp_store("semantic-index-operator-path");
    let collection_guard = QdrantCollectionGuard::new("operator_path");
    let collection = collection_guard.name();

    run_ok(
        &store,
        &[
            "remember",
            "Lena owns the release notes and keeps the changelog concise",
            "--tag",
            "product",
            "--mention",
            "Lena",
        ],
    );
    run_ok(
        &store,
        &["claim", "Lena", "owns", "release notes", "--source-last"],
    );
    run_ok(
        &store,
        &["link", "Lena", "owns", "Release Notes", "--source-last"],
    );
    run_ok(
        &store,
        &[
            "remember",
            "Lena owns scoped release notes",
            "--mention",
            "Lena",
            "--scope",
            "project:nahuali",
        ],
    );
    run_ok(
        &store,
        &[
            "claim",
            "Lena",
            "owns",
            "scoped release notes",
            "--source-last",
            "--scope",
            "project:nahuali",
        ],
    );

    let probe = run_with_semantic_collection(&store, &["semantic-status", "--json"], collection);
    if !probe.status.success() {
        let _ = fs::remove_file(store);
        return;
    }

    let rebuild_output =
        run_ok_with_semantic_collection(&store, &["semantic-rebuild", "--json"], collection);
    let rebuild: Value = serde_json::from_str(&rebuild_output).expect("rebuild output is JSON");
    assert_eq!(rebuild["database"], store.display().to_string());
    let scoped_collection = rebuild["report"]["collection_name"]
        .as_str()
        .expect("collection name is a string")
        .to_string();
    assert_ne!(scoped_collection, collection);
    assert!(scoped_collection.starts_with(&format!("{collection}__")));
    assert_eq!(rebuild["report"]["source_event_count"], 5);
    assert!(
        rebuild["report"]["indexed_point_count"]
            .as_u64()
            .unwrap_or_default()
            >= 4
    );

    let status_output =
        run_ok_with_semantic_collection(&store, &["semantic-status", "--json"], collection);
    let status: Value = serde_json::from_str(&status_output).expect("status output is JSON");
    assert_eq!(status["status"]["collection_exists"], true);
    assert_eq!(
        status["status"]["point_count"],
        rebuild["report"]["indexed_point_count"]
    );

    let recall_output = run_ok_with_semantic_collection(
        &store,
        &["recall", "release notes", "--semantic", "--json"],
        collection,
    );
    let recall: Value = serde_json::from_str(&recall_output).expect("hybrid recall output is JSON");
    assert_eq!(recall["collection_name"], scoped_collection);
    assert!(!recall["semantic_results"].as_array().unwrap().is_empty());
    assert!(
        recall["results"]
            .as_array()
            .unwrap()
            .iter()
            .any(|result| result["semantic_score"].is_number())
    );
    assert!(recall["authority"]["mode"].as_str().is_some());

    let scoped_recall_output = run_ok_with_semantic_collection(
        &store,
        &[
            "recall",
            "release notes",
            "--semantic",
            "--scope",
            "project:nahuali",
            "--kind",
            "claim",
            "--require-evidence",
            "--json",
        ],
        collection,
    );
    let scoped_recall: Value =
        serde_json::from_str(&scoped_recall_output).expect("scoped hybrid recall output is JSON");
    assert!(
        scoped_recall["semantic_results"]
            .as_array()
            .unwrap()
            .iter()
            .all(|result| result["scope_key"] == "project:nahuali")
    );
    assert!(
        scoped_recall["results"]
            .as_array()
            .unwrap()
            .iter()
            .all(|result| result["id"].as_str().is_some_and(|id| id.starts_with("claim_")))
    );

    let human_output = run_ok_with_semantic_collection(
        &store,
        &["recall", "release notes", "--semantic"],
        collection,
    );
    assert!(human_output.contains("Semantic collection:"));
    assert!(human_output.contains("semantic:"));

    let _ = fs::remove_file(store);
}

#[test]
fn operator_review_queue_is_scriptable() {
    let store = temp_store("operator-review-store");

    run_ok(&store, &["claim", "Lena", "role", "CTO"]);
    run_ok(&store, &["claim", "Lena", "role", "VP Engineering"]);

    let review_output = run_ok(
        &store,
        &[
            "review",
            "--limit",
            "2",
            "--min-priority",
            "high",
            "--action",
            "resolve-contradiction",
            "--json",
        ],
    );
    let review: Value = serde_json::from_str(&review_output).expect("review output is JSON");
    assert_eq!(review["version"], 1);
    assert_eq!(review["event_count"], 2);
    assert_eq!(review["action"], "resolve_contradiction");
    assert_eq!(review["write_back_policy"]["automatic_write_back"], false);
    assert_eq!(
        review["write_back_policy"]["requires_operator_review"],
        true
    );
    assert!(review["total_items"].as_u64().unwrap_or_default() >= 1);
    assert!(review["displayed_items"].as_u64().unwrap_or_default() <= 2);
    assert_eq!(review["items"][0]["priority"], "critical");
    assert_eq!(review["items"][0]["action"], "resolve_contradiction");
    assert!(review["items"]
        .as_array()
        .unwrap()
        .iter()
        .all(|item| item["action"] == "resolve_contradiction"));
    assert!(
        review["items"][0]["operator_guidance"]
            .as_str()
            .unwrap_or_default()
            .contains("record the resolution")
    );

    let validate_output = run_ok(&store, &["validate", "--json"]);
    let validation: Value =
        serde_json::from_str(&validate_output).expect("validation output is JSON");
    assert_eq!(validation["event_count"], 2);

    let human = run_ok(&store, &["review", "--limit", "1"]);
    assert!(human.contains("Operator review"));
    assert!(human.contains("Next action:"));

    let _ = fs::remove_file(store);
}

#[test]
fn review_resolution_is_scriptable() {
    let store = temp_store("review-resolution-store");

    run_ok(
        &store,
        &[
            "remember",
            "Lena owns the release notes",
            "--tag",
            "product",
        ],
    );
    run_ok(&store, &["claim", "Lena", "role", "CTO"]);

    let review_output = run_ok(&store, &["review", "--json"]);
    let review: Value = serde_json::from_str(&review_output).expect("review output is JSON");
    let review_id = review["items"]
        .as_array()
        .expect("review items are an array")
        .iter()
        .find(|item| item["finding_kind"] == "weak_evidence")
        .and_then(|item| item["id"].as_str())
        .expect("unsupported claim creates capture-evidence item")
        .to_string();

    let dry_run_output = run_ok(
        &store,
        &[
            "review-resolve",
            &review_id,
            "--note",
            "Operator confirmed this from release ownership notes.",
            "--dry-run",
            "--json",
        ],
    );
    let dry_run: Value = serde_json::from_str(&dry_run_output).expect("dry-run output is JSON");
    assert_eq!(dry_run["dry_run"], true);
    assert_eq!(dry_run["applied"], false);
    assert_eq!(dry_run["review_id"], review_id);

    let validation_output = run_ok(&store, &["validate", "--json"]);
    let validation: Value =
        serde_json::from_str(&validation_output).expect("validation output is JSON");
    assert_eq!(validation["event_count"], 2);
    assert_eq!(validation["review_decision_count"], 0);

    let apply_output = run_ok(
        &store,
        &[
            "review-resolve",
            &review_id,
            "--note",
            "Operator confirmed this from release ownership notes.",
            "--json",
        ],
    );
    let applied: Value = serde_json::from_str(&apply_output).expect("apply output is JSON");
    assert_eq!(applied["dry_run"], false);
    assert_eq!(applied["applied"], true);
    assert_eq!(applied["review_id"], review_id);
    assert!(
        applied["event_id"]
            .as_str()
            .unwrap_or_default()
            .starts_with("event_")
    );

    let inspect_output = run_ok(&store, &["inspect", "--json"]);
    let health: Value = serde_json::from_str(&inspect_output).expect("inspect output is JSON");
    assert_eq!(health["unsupported_fact_count"], 0);

    let review_after_output = run_ok(&store, &["review", "--json"]);
    let review_after: Value =
        serde_json::from_str(&review_after_output).expect("review output is JSON");
    assert!(
        review_after["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|item| item["id"].as_str() != Some(review_id.as_str()))
    );

    let validation_output = run_ok(&store, &["validate", "--json"]);
    let validation: Value =
        serde_json::from_str(&validation_output).expect("validation output is JSON");
    assert_eq!(validation["event_count"], 3);
    assert_eq!(validation["review_decision_count"], 1);

    let _ = fs::remove_file(store);
}

#[test]
fn graph_neighborhood_is_scriptable() {
    let store = temp_store("graph-neighborhood-store");

    run_ok(
        &store,
        &[
            "remember",
            "Lena owns the release notes",
            "--tag",
            "product",
            "--mention",
            "Lena",
            "--mention",
            "Release Notes",
        ],
    );
    run_ok(
        &store,
        &["claim", "Lena", "owns", "release notes", "--source-last"],
    );
    run_ok(
        &store,
        &["link", "Lena", "owns", "Release Notes", "--source-last"],
    );

    let graph_output = run_ok(
        &store,
        &["graph", "Lena", "--depth", "2", "--limit", "20", "--json"],
    );
    let graph: Value = serde_json::from_str(&graph_output).expect("graph output is JSON");
    assert_eq!(graph["version"], 1);
    assert_eq!(graph["seed"], "Lena");
    assert!(graph["summary"]["node_count"].as_u64().unwrap_or_default() >= 4);
    assert!(graph["summary"]["edge_count"].as_u64().unwrap_or_default() >= 3);
    assert!(
        graph["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["kind"] == "entity" && node["label"] == "Lena")
    );
    assert!(
        graph["edges"]
            .as_array()
            .unwrap()
            .iter()
            .any(|edge| edge["kind"] == "relation")
    );

    let human = run_ok(&store, &["graph", "Lena", "--limit", "5"]);
    assert!(human.contains("Memory graph"));
    assert!(human.contains("Nodes:"));

    let _ = fs::remove_file(store);
}

#[test]
fn import_reports_invalid_interchange_as_json_without_mutating_store() {
    let store = temp_store("invalid-interchange-store");
    let interchange_path = temp_store("invalid-interchange-document");
    let interchange_arg = interchange_path.display().to_string();
    fs::write(
        &interchange_path,
        r#"{"version":999,"claims":[{"subject":"Lena","predicate":"owns","object":"release notes","source_episode_ref":"missing"}]}"#,
    )
    .expect("invalid interchange writes");

    let output = run(&store, &["import", &interchange_arg, "--json"]);

    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");
    let report: Value = serde_json::from_str(&stdout).expect("import output is JSON");
    assert_eq!(report["database"], store.display().to_string());
    assert_eq!(report["report"]["valid"], false);
    assert_eq!(report["report"]["imported_event_count"], 0);
    assert_eq!(report["report"]["issues"][0]["kind"], "unsupported_version");

    let validation_output = run_ok(&store, &["validate", "--json"]);
    let validation: Value =
        serde_json::from_str(&validation_output).expect("validation output is JSON");
    assert_eq!(validation["event_count"], 0);

    let _ = fs::remove_file(store);
    let _ = fs::remove_file(interchange_path);
}

#[test]
fn source_last_requires_an_existing_episode() {
    let store = temp_store("source-last-empty-store");
    let output = run(
        &store,
        &["fact", "Lena", "owns", "release notes", "--source-last"],
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
    assert!(
        stderr.contains("--source-last requires at least one episode in the selected database")
    );
    assert!(stderr.contains("nahuali remember"));

    let _ = fs::remove_file(store);
}

#[test]
fn source_last_conflicts_with_manual_source_episode() {
    let store = temp_store("source-last-conflict");
    run_ok(&store, &["remember", "Lena owns the release notes"]);

    let output = run(
        &store,
        &[
            "fact",
            "Lena",
            "owns",
            "release notes",
            "--source-episode",
            "episode_manual",
            "--source-last",
        ],
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
    assert!(
        stderr.contains("cannot be used with")
            || stderr.contains("cannot be used with '--source-episode")
    );

    let _ = fs::remove_file(store);
}

#[test]
fn validate_human_output_reports_database_and_status() {
    let store = temp_store("validate-human-store-path");
    run_ok(&store, &["remember", "Lena owns the release notes"]);

    let output = run_ok(&store, &["validate"]);

    assert!(output.contains(&format!("Database: {}", store.display())));
    assert!(output.contains("Record ledger: memory_record"));
    assert!(output.contains("Projection: Rust"));
    assert!(output.contains("Status: valid"));
    assert!(output.contains("Events: 1"));

    let _ = fs::remove_file(store);
}
