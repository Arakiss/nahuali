#[test]
fn import_export_interchange_is_non_destructive_and_scriptable() {
    let source_store = temp_database("interchange-source-store");
    let target_store = temp_database("interchange-target-store");
    let interchange_path = temp_store("interchange-document");
    let interchange_arg = interchange_path.display().to_string();

    run_ok(
        &source_store,
        &[
            "remember",
            "Lena wants release notes kept concise",
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
    run_ok(
        &source_store,
        &["link", "Lena", "owns", "Release Notes", "--source-last"],
    );

    let export_output = run_ok(
        &source_store,
        &["export", "--output", &interchange_arg, "--json"],
    );
    let exported: Value = serde_json::from_str(&export_output).expect("export output is JSON");
    assert_eq!(exported["database"], source_store.display().to_string());
    assert_eq!(exported["interchange_path"], interchange_arg);
    assert_eq!(exported["summary"]["episode_count"], 1);
    assert_eq!(exported["summary"]["claim_count"], 1);
    assert!(interchange_path.exists());

    let stdout_document = run_ok(&source_store, &["export", "--json"]);
    let interchange: Value =
        serde_json::from_str(&stdout_document).expect("stdout export is interchange JSON");
    assert_eq!(interchange["version"], 1);
    assert!(interchange["episodes"][0]["ref"].as_str().is_some());

    let dry_run_output = run_ok(
        &target_store,
        &["import", &interchange_arg, "--dry-run", "--json"],
    );
    let dry_run: Value = serde_json::from_str(&dry_run_output).expect("dry-run output is JSON");
    assert_eq!(dry_run["database"], target_store.display().to_string());
    assert_eq!(dry_run["report"]["valid"], true);
    assert_eq!(dry_run["report"]["dry_run"], true);
    assert_eq!(dry_run["report"]["appendable_event_count"], 3);
    assert_eq!(dry_run["report"]["imported_event_count"], 0);

    let empty_validation = run_ok(&target_store, &["validate", "--json"]);
    let empty: Value = serde_json::from_str(&empty_validation).expect("validation output is JSON");
    assert_eq!(empty["event_count"], 0);

    let import_output = run_ok(&target_store, &["import", &interchange_arg, "--json"]);
    let imported: Value = serde_json::from_str(&import_output).expect("import output is JSON");
    assert_eq!(imported["report"]["valid"], true);
    assert_eq!(imported["report"]["imported_event_count"], 3);

    let validation_output = run_ok(&target_store, &["validate", "--json"]);
    let validation: Value =
        serde_json::from_str(&validation_output).expect("validation output is JSON");
    assert_eq!(validation["event_count"], 3);
    assert_eq!(validation["episode_count"], 1);
    assert_eq!(validation["claim_count"], 1);
    assert_eq!(validation["link_count"], 1);

    let _ = fs::remove_file(source_store);
    let _ = fs::remove_file(target_store);
    let _ = fs::remove_file(interchange_path);
}

#[test]
fn ingestion_document_is_scriptable() {
    let store = temp_database("ingestion-document-store");
    let document_path = temp_store("ingestion-document");
    let document_arg = document_path.display().to_string();
    fs::write(
        &document_path,
        serde_json::to_string_pretty(&serde_json::json!({
            "version": 1,
            "source": {
                "kind": "conversation",
                "title": "Release review",
                "uri": "fixture://release-review",
                "metadata": {
                    "adapter": "fixture"
                }
            },
            "episodes": [
                {
                    "ref": "message-1",
                    "content": "Lena owns the release notes.",
                    "tags": ["product"],
                    "mentions": ["Lena", "Release Notes"],
                    "source_position": 1,
                    "source_role": "user"
                },
                {
                    "ref": "message-2",
                    "content": "Release notes should stay concise.",
                    "tags": ["product"],
                    "mentions": ["Release Notes"],
                    "source_position": 2,
                    "source_role": "assistant"
                }
            ],
            "claims": [
                {
                    "subject": "Lena",
                    "predicate": "owns",
                    "object": "release notes",
                    "source_episode_ref": "message-1",
                    "confidence": 0.94
                }
            ],
            "links": [
                {
                    "from": "Lena",
                    "relation": "owns",
                    "to": "Release Notes",
                    "source_episode_ref": "message-1",
                    "confidence": 0.92
                }
            ],
            "procedures": [
                {
                    "kind": "preference",
                    "name": "Release notes",
                    "body": "Keep release notes concise.",
                    "source_episode_ref": "message-2",
                    "confidence": 0.9
                }
            ],
            "intentions": [
                {
                    "kind": "task",
                    "priority": "high",
                    "description": "Ship release notes",
                    "source_episode_ref": "message-1"
                }
            ]
        }))
        .expect("ingestion document serializes"),
    )
    .expect("ingestion document writes");

    let dry_run_output = run_ok(&store, &["ingest", &document_arg, "--dry-run", "--json"]);
    let dry_run: Value = serde_json::from_str(&dry_run_output).expect("dry-run output is JSON");
    assert_eq!(dry_run["database"], store.display().to_string());
    assert_eq!(dry_run["ingest_path"], document_arg);
    assert_eq!(dry_run["report"]["valid"], true);
    assert_eq!(dry_run["report"]["dry_run"], true);
    assert_eq!(dry_run["report"]["appendable_event_count"], 7);
    assert_eq!(dry_run["report"]["ingested_event_count"], 0);
    assert_eq!(dry_run["report"]["preflight"]["source_scoped"], false);
    assert_eq!(dry_run["report"]["preflight"]["derived_record_count"], 4);
    assert_eq!(
        dry_run["report"]["preflight"]["evidence_linked_record_count"],
        4
    );
    assert_eq!(dry_run["report"]["preflight"]["evidence_gap_count"], 0);
    assert_eq!(
        dry_run["report"]["preflight"]["referenced_episode_count"],
        2
    );
    assert_eq!(
        dry_run["report"]["preflight"]["unreferenced_episode_count"],
        0
    );

    let empty_validation = run_ok(&store, &["validate", "--json"]);
    let empty: Value = serde_json::from_str(&empty_validation).expect("validation output is JSON");
    assert_eq!(empty["event_count"], 0);

    let ingest_output = run_ok(&store, &["ingest", &document_arg, "--json"]);
    let ingested: Value = serde_json::from_str(&ingest_output).expect("ingest output is JSON");
    assert_eq!(ingested["report"]["valid"], true);
    assert_eq!(ingested["report"]["dry_run"], false);
    assert_eq!(ingested["report"]["ingested_event_count"], 7);
    assert!(
        ingested["report"]["source_id"]
            .as_str()
            .unwrap_or_default()
            .starts_with("source_")
    );
    assert_eq!(
        ingested["report"]["episode_ids"].as_array().unwrap().len(),
        2
    );

    let data_output = run_ok(&store, &["data", "--json"]);
    let data: Value = serde_json::from_str(&data_output).expect("data output is JSON");
    assert_eq!(data["sources"].as_array().unwrap().len(), 1);
    assert_eq!(data["sources"][0]["kind"], "conversation");
    assert_eq!(data["sources"][0]["metadata"]["adapter"], "fixture");
    assert_eq!(data["episodes"][0]["source_position"], 1);
    assert_eq!(data["episodes"][0]["source_role"], "user");
    assert_eq!(data["claims"].as_array().unwrap().len(), 1);
    assert_eq!(data["links"].as_array().unwrap().len(), 1);
    assert_eq!(data["procedures"].as_array().unwrap().len(), 1);
    assert_eq!(data["intentions"].as_array().unwrap().len(), 1);

    let validation_output = run_ok(&store, &["validate", "--json"]);
    let validation: Value =
        serde_json::from_str(&validation_output).expect("validation output is JSON");
    assert_eq!(validation["event_count"], 7);
    assert_eq!(validation["source_count"], 1);
    assert_eq!(validation["episode_count"], 2);
    assert_eq!(validation["claim_count"], 1);
    assert_eq!(validation["link_count"], 1);

    let human = run_ok(&store, &["ingest", &document_arg, "--dry-run"]);
    assert!(human.contains("Ingest document:"));
    assert!(human.contains("Status: dry-run"));
    assert!(human.contains("Evidence gaps: 0"));

    let _ = fs::remove_file(store);
    let _ = fs::remove_file(document_path);
}

#[test]
fn text_file_ingestion_is_scriptable() {
    let store = temp_database("text-file-ingestion-store");
    let text_path = temp_store("text-file-source").with_extension("md");
    let text_arg = text_path.display().to_string();
    fs::write(
        &text_path,
        "Lena owns the release notes.\n\nRelease notes should stay concise.\n",
    )
    .expect("text source writes");

    let dry_run_output = run_ok(
        &store,
        &[
            "ingest-text",
            &text_arg,
            "--kind",
            "note",
            "--title",
            "Release notes source",
            "--chunking",
            "paragraphs",
            "--tag",
            "product",
            "--mention",
            "Lena",
            "--metadata",
            "origin=fixture",
            "--dry-run",
            "--json",
        ],
    );
    let dry_run: Value = serde_json::from_str(&dry_run_output).expect("dry-run output is JSON");
    assert_eq!(dry_run["database"], store.display().to_string());
    assert_eq!(dry_run["text_path"], text_arg);
    assert_eq!(dry_run["adapter_report"]["valid"], true);
    assert_eq!(dry_run["adapter_report"]["episode_count"], 2);
    assert_eq!(
        dry_run["adapter_report"]["document"]["source"]["kind"],
        "note"
    );
    assert_eq!(
        dry_run["adapter_report"]["document"]["source"]["metadata"]["origin"],
        "fixture"
    );
    assert_eq!(dry_run["report"]["valid"], true);
    assert_eq!(dry_run["report"]["dry_run"], true);
    assert_eq!(dry_run["report"]["appendable_event_count"], 3);
    assert_eq!(dry_run["report"]["ingested_event_count"], 0);
    assert_eq!(dry_run["report"]["preflight"]["derived_record_count"], 0);
    assert_eq!(dry_run["report"]["preflight"]["evidence_gap_count"], 0);
    assert_eq!(
        dry_run["report"]["preflight"]["unreferenced_episode_count"],
        2
    );

    let empty_validation = run_ok(&store, &["validate", "--json"]);
    let empty: Value = serde_json::from_str(&empty_validation).expect("validation output is JSON");
    assert_eq!(empty["event_count"], 0);

    let ingest_output = run_ok(
        &store,
        &[
            "ingest-text",
            &text_arg,
            "--kind",
            "note",
            "--title",
            "Release notes source",
            "--chunking",
            "paragraphs",
            "--tag",
            "product",
            "--mention",
            "Lena",
            "--metadata",
            "origin=fixture",
            "--json",
        ],
    );
    let ingested: Value = serde_json::from_str(&ingest_output).expect("ingest output is JSON");
    assert_eq!(ingested["report"]["valid"], true);
    assert_eq!(ingested["report"]["dry_run"], false);
    assert_eq!(ingested["report"]["ingested_event_count"], 3);
    assert!(
        ingested["report"]["source_id"]
            .as_str()
            .unwrap_or_default()
            .starts_with("source_")
    );
    assert_eq!(
        ingested["report"]["episode_ids"].as_array().unwrap().len(),
        2
    );

    let data_output = run_ok(&store, &["data", "--json"]);
    let data: Value = serde_json::from_str(&data_output).expect("data output is JSON");
    assert_eq!(data["sources"].as_array().unwrap().len(), 1);
    assert_eq!(data["sources"][0]["kind"], "note");
    assert_eq!(data["sources"][0]["title"], "Release notes source");
    assert_eq!(data["sources"][0]["metadata"]["origin"], "fixture");
    assert_eq!(data["episodes"].as_array().unwrap().len(), 2);
    assert_eq!(data["episodes"][0]["tags"], serde_json::json!(["product"]));
    assert_eq!(data["episodes"][0]["mentions"], serde_json::json!(["Lena"]));
    assert_eq!(data["episodes"][1]["source_position"], 2);

    let validation_output = run_ok(&store, &["validate", "--json"]);
    let validation: Value =
        serde_json::from_str(&validation_output).expect("validation output is JSON");
    assert_eq!(validation["event_count"], 3);
    assert_eq!(validation["source_count"], 1);
    assert_eq!(validation["episode_count"], 2);

    let human = run_ok(
        &store,
        &[
            "ingest-text",
            &text_arg,
            "--title",
            "Release notes source",
            "--chunking",
            "paragraphs",
            "--dry-run",
        ],
    );
    assert!(human.contains("Text source:"));
    assert!(human.contains("Status: dry-run"));
    assert!(human.contains("Evidence gaps: 0"));

    let _ = fs::remove_file(store);
    let _ = fs::remove_file(text_path);
}

#[test]
fn directory_text_ingestion_is_atomic() {
    let store = temp_database("directory-text-ingestion-store");
    let source_dir = temp_store("directory-text-source");
    let nested_dir = source_dir.join("nested");
    fs::create_dir_all(&nested_dir).expect("source directory creates");
    fs::write(source_dir.join("one.md"), "Lena owns the release notes.\n")
        .expect("first source writes");
    fs::write(nested_dir.join("two.txt"), "Release notes stay concise.\n")
        .expect("second source writes");
    fs::write(source_dir.join("empty.md"), "   \n").expect("invalid source writes");
    let source_arg = source_dir.display().to_string();

    let invalid_output = run(
        &store,
        &[
            "ingest-dir",
            &source_arg,
            "--recursive",
            "--chunking",
            "paragraphs",
            "--tag",
            "product",
            "--mention",
            "Lena",
            "--dry-run",
            "--json",
        ],
    );
    assert!(!invalid_output.status.success());
    let stdout = String::from_utf8(invalid_output.stdout).expect("stdout is UTF-8");
    let invalid: Value = serde_json::from_str(&stdout).expect("invalid output is JSON");
    assert_eq!(invalid["valid"], false);
    assert_eq!(invalid["file_count"], 3);
    assert!(
        invalid["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["adapter_report"]["valid"] == false)
    );

    let empty_validation = run_ok(&store, &["validate", "--json"]);
    let empty: Value = serde_json::from_str(&empty_validation).expect("validation output is JSON");
    assert_eq!(empty["event_count"], 0);

    fs::remove_file(source_dir.join("empty.md")).expect("invalid source removes");

    let dry_run_output = run_ok(
        &store,
        &[
            "ingest-dir",
            &source_arg,
            "--recursive",
            "--extension",
            "md",
            "--extension",
            "txt",
            "--kind",
            "note",
            "--chunking",
            "paragraphs",
            "--tag",
            "product",
            "--mention",
            "Lena",
            "--metadata",
            "origin=batch-fixture",
            "--dry-run",
            "--json",
        ],
    );
    let dry_run: Value = serde_json::from_str(&dry_run_output).expect("dry-run output is JSON");
    assert_eq!(dry_run["valid"], true);
    assert_eq!(dry_run["dry_run"], true);
    assert_eq!(dry_run["file_count"], 2);
    assert_eq!(dry_run["appendable_event_count"], 4);
    assert_eq!(dry_run["ingested_event_count"], 0);
    assert!(
        dry_run["files"]
            .as_array()
            .unwrap()
            .iter()
            .all(|file| file["report"]["preflight"]["unreferenced_episode_count"] == 1)
    );

    let empty_validation = run_ok(&store, &["validate", "--json"]);
    let empty: Value = serde_json::from_str(&empty_validation).expect("validation output is JSON");
    assert_eq!(empty["event_count"], 0);

    let ingest_output = run_ok(
        &store,
        &[
            "ingest-dir",
            &source_arg,
            "--recursive",
            "--extension",
            "md",
            "--extension",
            "txt",
            "--kind",
            "note",
            "--chunking",
            "paragraphs",
            "--tag",
            "product",
            "--mention",
            "Lena",
            "--metadata",
            "origin=batch-fixture",
            "--json",
        ],
    );
    let ingested: Value = serde_json::from_str(&ingest_output).expect("ingest output is JSON");
    assert_eq!(ingested["valid"], true);
    assert_eq!(ingested["dry_run"], false);
    assert_eq!(ingested["file_count"], 2);
    assert_eq!(ingested["ingested_event_count"], 4);
    assert!(
        ingested["files"]
            .as_array()
            .unwrap()
            .iter()
            .all(|file| file["adapter_report"]["document"]["source"]["kind"] == "note")
    );

    let data_output = run_ok(&store, &["data", "--json"]);
    let data: Value = serde_json::from_str(&data_output).expect("data output is JSON");
    assert_eq!(data["sources"].as_array().unwrap().len(), 2);
    assert_eq!(data["episodes"].as_array().unwrap().len(), 2);
    assert!(
        data["sources"]
            .as_array()
            .unwrap()
            .iter()
            .all(|source| source["metadata"]["origin"] == "batch-fixture")
    );

    let validation_output = run_ok(&store, &["validate", "--json"]);
    let validation: Value =
        serde_json::from_str(&validation_output).expect("validation output is JSON");
    assert_eq!(validation["event_count"], 4);
    assert_eq!(validation["source_count"], 2);
    assert_eq!(validation["episode_count"], 2);

    let human = run_ok(
        &store,
        &["ingest-dir", &source_arg, "--recursive", "--dry-run"],
    );
    assert!(human.contains("Text directory:"));
    assert!(human.contains("Status: dry-run"));
    assert!(human.contains("evidence gaps: 0"));

    let _ = fs::remove_file(store);
    let _ = fs::remove_dir_all(source_dir);
}
