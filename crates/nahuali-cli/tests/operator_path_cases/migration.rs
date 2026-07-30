#[test]
fn legacy_export_conversion_supports_dogfood_migration_dry_run() {
    let store = temp_database("projection-export-conversion");
    let export_path = temp_store("migration-artifact").with_extension("projection.json");
    let interchange_path = temp_store("migration-artifact").with_extension("interchange.json");

    fs::write(
        &export_path,
        r#"{
  "exportedAt": "2026-04-23T08:45:00.000Z",
  "entities": [
    {
      "id": {"tb": "entity", "id": "lena"},
      "name": "Lena",
      "type": "person",
      "createdAt": "2026-04-23T08:45:00.000Z",
      "aliases": ["L."],
      "attributes": {
        "role": "release owner",
        "team": "product"
      }
    }
  ],
  "episodes": [
    {
      "id": {"tb": "episode", "id": "release"},
      "summary": "Lena owns the release notes.",
      "content": "Release notes should stay concise and cite evidence.",
      "entities": [{"entityId": {"tb": "entity", "id": "lena"}}],
      "tags": ["product"],
      "timestamp": "2026-04-23T08:45:30.125Z",
      "source": "conversation:release-review",
      "sourcePosition": 1,
      "operator": "release-chair"
    }
  ],
  "relations": [
    {
      "id": "relates_to:release",
      "in": {"tb": "entity", "id": "lena"},
      "out": "Release Notes",
      "type": "owns",
      "confidence": 0.91,
      "createdAt": 1776933900000
    }
  ],
  "procedures": [
    {
      "id": "procedure:release_notes",
      "name": "Release note style",
      "category": "writing",
      "description": "Keep release notes concise.",
      "rules": ["Cite evidence for shipped behavior."],
      "antiPatterns": ["Do not overpromise."],
      "createdAt": 1776933900000
    }
  ],
  "intentions": [
    {
      "id": "intention:ship_release",
      "description": "Ship release notes",
      "type": "task",
      "state": "completed",
      "priority": "high",
      "context": "Release readiness",
      "createdAt": 1776933900000,
      "completedAt": "2026-04-24T09:00:00.000Z"
    }
  ]
}"#,
    )
    .expect("projection export fixture writes");

    let converted = run_ok(
        &store,
        &[
            "convert-legacy-export",
            export_path.to_str().unwrap(),
            "--output",
            interchange_path.to_str().unwrap(),
            "--scope",
            "project:Nahuali",
            "--json",
        ],
    );
    let converted: Value = serde_json::from_str(&converted).expect("conversion output is JSON");
    assert_eq!(converted["detected_format"].as_str(), Some("json"));
    assert_eq!(converted["summary"]["source_count"], 1);
    assert_eq!(converted["summary"]["episode_count"], 1);
    assert_eq!(converted["summary"]["claim_count"], 4);
    assert_eq!(converted["summary"]["link_count"], 1);
    assert_eq!(converted["summary"]["procedure_count"], 1);
    assert_eq!(converted["summary"]["intention_count"], 1);
    assert_eq!(converted["summary"]["issue_count"], 0);
    let interchange = fs::read_to_string(&interchange_path).expect("interchange file reads");
    let interchange: Value = serde_json::from_str(&interchange).expect("interchange is JSON");
    let claims = interchange["claims"]
        .as_array()
        .expect("interchange includes claims");
    assert_eq!(
        interchange["sources"][0]["ref"].as_str(),
        Some("source:conversation_release_review")
    );
    assert_eq!(
        interchange["sources"][0]["title"].as_str(),
        Some("conversation:release-review")
    );
    assert_eq!(
        interchange["episodes"][0]["timestamp_ms"],
        serde_json::json!(1776933930125u64)
    );
    assert_eq!(
        interchange["episodes"][0]["source_role"].as_str(),
        Some("release-chair")
    );
    assert_eq!(
        interchange["episodes"][0]["source_ref"].as_str(),
        Some("source:conversation_release_review")
    );
    assert_eq!(
        interchange["episodes"][0]["source_position"],
        serde_json::json!(1)
    );
    assert!(claims.iter().all(|claim| {
        claim["source_episode_ref"]
            .as_str()
            .is_some_and(|source| source == "episode:release")
    }));
    assert!(claims
        .iter()
        .all(|claim| claim["timestamp_ms"] == serde_json::json!(1776933900000u64)));
    assert_eq!(
        interchange["links"][0]["source_episode_ref"].as_str(),
        Some("episode:release")
    );
    assert_eq!(
        interchange["links"][0]["timestamp_ms"],
        serde_json::json!(1776933900000u64)
    );
    assert_eq!(
        interchange["procedures"][0]["timestamp_ms"],
        serde_json::json!(1776933900000u64)
    );
    assert_eq!(
        interchange["intentions"][0]["timestamp_ms"],
        serde_json::json!(1776933900000u64)
    );
    assert_eq!(
        interchange["intentions"][0]["status_timestamp_ms"],
        serde_json::json!(1777021200000u64)
    );

    let dry_run = run_ok(
        &store,
        &[
            "import",
            interchange_path.to_str().unwrap(),
            "--dry-run",
            "--json",
        ],
    );
    let dry_run: Value = serde_json::from_str(&dry_run).expect("import dry-run output is JSON");
    assert_eq!(dry_run["report"]["valid"], true);
    assert_eq!(dry_run["report"]["dry_run"], true);
    assert_eq!(dry_run["report"]["appendable_event_count"], 10);
    assert_eq!(dry_run["report"]["counts"]["sources"], 1);
    assert_eq!(dry_run["report"]["preflight"]["source_count"], 1);
    assert_eq!(dry_run["report"]["preflight"]["sourced_episode_count"], 1);
    assert_eq!(dry_run["report"]["preflight"]["unsourced_episode_count"], 0);
    assert_eq!(dry_run["report"]["imported_event_count"], 0);
    assert_eq!(dry_run["report"]["preflight"]["derived_record_count"], 7);
    assert_eq!(
        dry_run["report"]["preflight"]["evidence_linked_record_count"],
        5
    );
    assert_eq!(dry_run["report"]["preflight"]["evidence_gap_count"], 2);
    assert_eq!(
        dry_run["report"]["preflight"]["referenced_episode_count"],
        1
    );
    assert_eq!(
        dry_run["report"]["preflight"]["unreferenced_episode_count"],
        0
    );
    assert_eq!(dry_run["report"]["preflight"]["scoped_record_count"], 9);
    assert_eq!(dry_run["report"]["preflight"]["unscoped_record_count"], 0);
    assert_eq!(
        dry_run["report"]["preflight"]["scope_keys"],
        serde_json::json!(["project:nahuali"])
    );
    assert_eq!(
        dry_run["report"]["readiness"]["self_inspection_summary"]["source_coverage_count"],
        1
    );
    assert_eq!(
        dry_run["report"]["readiness"]["self_inspection_summary"]["finding_count"],
        1
    );
    assert_eq!(dry_run["report"]["readiness"]["review_item_count"], 1);
    assert_eq!(
        dry_run["report"]["readiness"]["write_back_policy"]["automatic_write_back"],
        false
    );

    let human_dry_run = run_ok(
        &store,
        &["import", interchange_path.to_str().unwrap(), "--dry-run"],
    );
    assert!(human_dry_run.contains("Evidence gaps: 2"));
    assert!(human_dry_run.contains("Scopes: project:nahuali"));
    assert!(human_dry_run.contains("Readiness source coverage findings: 1"));
    assert!(human_dry_run.contains("Readiness review items: 1"));

    let imported = run_ok(
        &store,
        &["import", interchange_path.to_str().unwrap(), "--json"],
    );
    let imported: Value = serde_json::from_str(&imported).expect("import output is JSON");
    assert_eq!(imported["report"]["valid"], true);
    assert_eq!(imported["report"]["imported_event_count"], 10);

    let self_inspected = run_ok(&store, &["self-inspect", "--json"]);
    let self_inspected: Value =
        serde_json::from_str(&self_inspected).expect("self-inspect output is JSON");
    assert_eq!(
        self_inspected["write_back_policy"]["automatic_write_back"],
        false
    );
    assert_eq!(self_inspected["summary"]["source_coverage_count"], 1);
    assert!(self_inspected["findings"].as_array().unwrap().iter().any(|finding| {
        finding["kind"] == "source_coverage"
            && finding["detail"].as_str().is_some_and(|detail| {
                detail.contains("0 episode(s) lack source records")
                    && detail.contains("2 derived memory item(s)")
            })
    }));

    let recalled = run_ok(
        &store,
        &[
            "recall",
            "release notes",
            "--scope",
            "project:Nahuali",
            "--json",
        ],
    );
    let recalled: Value = serde_json::from_str(&recalled).expect("recall output is JSON");
    let results = recalled.as_array().expect("recall returns an array");
    assert!(results.iter().any(|result| {
        result["excerpt"]
            .as_str()
            .is_some_and(|excerpt| excerpt.contains("release notes"))
            && result["scope"]["key"] == "project:nahuali"
    }));
}

#[test]
fn legacy_export_conversion_accepts_surql_exports() {
    let store = temp_database("legacy-export-surql");
    let export_path = temp_store("migration-artifact").with_extension("backup.surql");
    let interchange_path = temp_store("migration-artifact").with_extension("interchange.json");

    fs::write(
        &export_path,
        r#"-- Nahuali Export 2026-04-23T08:45:00.000Z

-- Entities
INSERT INTO entity {"id":"entity:lena","name":"Lena","type":"person","createdAt":"2026-04-23T08:45:00.000Z","aliases":["L."],"attributes":{"role":"release owner","team":"product"}};

-- Episodes
INSERT INTO episode {"id":"episode:release","summary":"Lena owns the release notes.","content":"Release notes should stay concise and cite evidence.","entities":["Lena"],"tags":["product"],"timestamp":"2026-04-23T08:45:30.125Z","source":"conversation:release-review","sourcePosition":1,"operator":"release-chair"};

-- Relations
INSERT INTO relates_to {"id":"relates_to:release","in":"Lena","out":"Release Notes","type":"owns","confidence":0.91,"createdAt":"2026-04-23T08:45:00.000Z"};

-- Procedures
INSERT INTO procedure {"id":"procedure:release_notes","name":"Release note style","category":"writing","description":"Keep release notes concise.","rules":["Cite evidence for shipped behavior."],"antiPatterns":["Do not overpromise."],"createdAt":"2026-04-23T08:45:00.000Z"};

-- Intentions
INSERT INTO intention {"id":"intention:ship_release","description":"Ship release notes","type":"task","priority":"high","state":"completed","context":"Release readiness","createdAt":"2026-04-23T08:45:00.000Z","completedAt":"2026-04-24T09:00:00.000Z"};
"#,
    )
    .expect("legacy surql export fixture writes");

    let converted = run_ok(
        &store,
        &[
            "convert-legacy-export",
            export_path.to_str().unwrap(),
            "--output",
            interchange_path.to_str().unwrap(),
            "--scope",
            "project:Nahuali",
            "--json",
        ],
    );
    let converted: Value = serde_json::from_str(&converted).expect("conversion output is JSON");
    assert_eq!(converted["detected_format"].as_str(), Some("surql"));
    assert_eq!(converted["summary"]["source_count"], 1);
    assert_eq!(converted["summary"]["episode_count"], 1);
    assert_eq!(converted["summary"]["claim_count"], 4);
    assert_eq!(converted["summary"]["link_count"], 1);
    assert_eq!(converted["summary"]["procedure_count"], 1);
    assert_eq!(converted["summary"]["intention_count"], 1);
    assert_eq!(converted["summary"]["issue_count"], 0);

    let interchange = fs::read_to_string(&interchange_path).expect("interchange file reads");
    let interchange: Value = serde_json::from_str(&interchange).expect("interchange is JSON");
    assert_eq!(
        interchange["episodes"][0]["source_ref"].as_str(),
        Some("source:conversation_release_review")
    );
    assert_eq!(
        interchange["episodes"][0]["timestamp_ms"],
        serde_json::json!(1776933930125u64)
    );
    assert_eq!(
        interchange["intentions"][0]["status"].as_str(),
        Some("completed")
    );

    let dry_run = run_ok(
        &store,
        &[
            "import",
            interchange_path.to_str().unwrap(),
            "--dry-run",
            "--json",
        ],
    );
    let dry_run: Value = serde_json::from_str(&dry_run).expect("import dry-run output is JSON");
    assert_eq!(dry_run["report"]["valid"], true);
    assert_eq!(dry_run["report"]["appendable_event_count"], 10);
    assert_eq!(dry_run["report"]["preflight"]["scope_keys"], serde_json::json!(["project:nahuali"]));

    let imported = run_ok(
        &store,
        &["import", interchange_path.to_str().unwrap(), "--json"],
    );
    let imported: Value = serde_json::from_str(&imported).expect("import output is JSON");
    assert_eq!(imported["report"]["valid"], true);
    assert_eq!(imported["report"]["imported_event_count"], 10);
}

#[test]
fn projection_export_conversion_accepts_legacy_projection_aliases() {
    let store = temp_database("projection-export-legacy-aliases");
    let export_path = temp_store("migration-artifact").with_extension("projection.json");
    let interchange_path = temp_store("migration-artifact").with_extension("interchange.json");

    fs::write(
        &export_path,
        r#"{
  "data": {
    "entity": [
      {
        "id": "entity:lena",
        "name": "Lena",
        "type": "person",
        "createdAt": "2026-04-23T08:45:00.000Z",
        "attributes": {
          "role": "release owner"
        }
      }
    ],
    "episode": [
      {
        "id": "episode:release",
        "title": "Release meeting",
        "body": "Lena promised concise release notes.",
        "entityNames": ["Lena"],
        "emotions": ["focused"],
        "tags": "product",
        "timestamp": "2026-04-23T08:45:30.125Z",
        "operator": "release-chair"
      }
    ],
    "relates_to": [
      {
        "id": "relates_to:release",
        "fromEntity": "Lena",
        "toEntity": {"label": "Release Notes"},
        "relationType": "custom",
        "customType": "owns",
        "confidence": 0.91,
        "createdAt": "2026-04-23T08:45:00.000Z"
      }
    ],
    "procedure": [
      {
        "id": "procedure:release_notes",
        "name": "Release note style",
        "category": "writing",
        "description": "Keep release notes concise.",
        "rules": ["Cite evidence for shipped behavior."],
        "antiPatterns": ["Do not overpromise."],
        "triggers": [{"type": "keyword", "value": "release", "weight": 1}],
        "examples": [{"input": "A shipped change", "output": "A concise note"}],
        "priority": 80,
        "entityScope": ["Release Notes"],
        "contextScope": ["launch"],
        "createdAt": "2026-04-23T08:45:00.000Z"
      }
    ],
    "intention": [
      {
        "id": "intention:ship_release",
        "description": "Ship release notes",
        "type": "deadline",
        "status": "done",
        "importance": 0.95,
        "targetDate": "2026-04-24T09:00:00.000Z",
        "createdAt": "2026-04-23T08:45:00.000Z",
        "completedAt": "2026-04-24T09:00:00.000Z",
        "notes": ["Waiting for the final changelog."],
        "tags": ["release"],
        "entityNames": ["Lena"]
      }
    ]
  }
}"#,
    )
    .expect("legacy projection export fixture writes");

    let converted = run_ok(
        &store,
        &[
            "convert-projection-export",
            export_path.to_str().unwrap(),
            "--output",
            interchange_path.to_str().unwrap(),
            "--scope",
            "project:Nahuali",
            "--json",
        ],
    );
    let converted: Value = serde_json::from_str(&converted).expect("conversion output is JSON");
    assert_eq!(converted["summary"]["episode_count"], 1);
    assert_eq!(converted["summary"]["claim_count"], 2);
    assert_eq!(converted["summary"]["link_count"], 1);
    assert_eq!(converted["summary"]["procedure_count"], 1);
    assert_eq!(converted["summary"]["intention_count"], 1);
    assert_eq!(converted["summary"]["issue_count"], 0);
    assert_eq!(converted["summary"]["source_counts"]["entities"], 1);
    assert_eq!(converted["summary"]["source_counts"]["episodes"], 1);
    assert_eq!(converted["summary"]["source_counts"]["relations"], 1);

    let interchange = fs::read_to_string(&interchange_path).expect("interchange file reads");
    let interchange: Value = serde_json::from_str(&interchange).expect("interchange is JSON");
    assert_eq!(
        interchange["episodes"][0]["tags"],
        serde_json::json!(["focused", "product"])
    );
    assert_eq!(
        interchange["episodes"][0]["mentions"],
        serde_json::json!(["Lena"])
    );
    assert_eq!(
        interchange["episodes"][0]["timestamp_ms"],
        serde_json::json!(1776933930125u64)
    );
    assert_eq!(
        interchange["episodes"][0]["source_role"].as_str(),
        Some("release-chair")
    );
    assert_eq!(interchange["links"][0]["from"].as_str(), Some("Lena"));
    assert_eq!(
        interchange["links"][0]["to"].as_str(),
        Some("Release Notes")
    );
    assert_eq!(interchange["links"][0]["relation"].as_str(), Some("owns"));

    let procedure_body = interchange["procedures"][0]["body"]
        .as_str()
        .expect("procedure body is preserved");
    assert!(procedure_body.contains("Triggers:"));
    assert!(procedure_body.contains("Examples:"));
    assert!(procedure_body.contains("Entity scope:"));

    let intention = &interchange["intentions"][0];
    assert_eq!(intention["kind"].as_str(), Some("reminder"));
    assert_eq!(intention["priority"].as_str(), Some("critical"));
    assert_eq!(intention["status"].as_str(), Some("completed"));
    assert_eq!(
        intention["status_timestamp_ms"],
        serde_json::json!(1777021200000u64)
    );
    assert!(intention["description"]
        .as_str()
        .is_some_and(|description| description.contains("Target date: 2026-04-24")));
}

#[test]
fn projection_export_conversion_reports_bad_timestamps_without_blocking_records() {
    let store = temp_database("projection-export-bad-timestamp");
    let export_path = temp_store("migration-artifact").with_extension("projection.json");
    let interchange_path = temp_store("migration-artifact").with_extension("interchange.json");

    fs::write(
        &export_path,
        r#"{
  "entities": [
    {
      "id": "entity:lena",
      "name": "Lena",
      "type": "person",
      "createdAt": "not-a-date"
    }
  ],
  "episodes": [
    {
      "id": "episode:release",
      "summary": "Lena owns the release notes.",
      "entities": ["Lena"]
    }
  ]
}"#,
    )
    .expect("bad timestamp projection export fixture writes");

    let converted = run_ok(
        &store,
        &[
            "convert-projection-export",
            export_path.to_str().unwrap(),
            "--output",
            interchange_path.to_str().unwrap(),
            "--json",
        ],
    );
    let converted: Value = serde_json::from_str(&converted).expect("conversion output is JSON");
    assert_eq!(converted["summary"]["episode_count"], 1);
    assert_eq!(converted["summary"]["claim_count"], 1);
    assert_eq!(converted["summary"]["issue_count"], 1);
    assert_eq!(converted["issues"][0]["path"].as_str(), Some("entities[0].createdAt"));

    let interchange = fs::read_to_string(&interchange_path).expect("interchange file reads");
    let interchange: Value = serde_json::from_str(&interchange).expect("interchange is JSON");
    assert_eq!(interchange["claims"][0]["timestamp_ms"], serde_json::Value::Null);
}

#[test]
fn projection_export_conversion_rejects_empty_payloads() {
    let store = temp_database("projection-export-empty");
    let export_path = temp_store("migration-artifact").with_extension("projection.json");
    let interchange_path = temp_store("migration-artifact").with_extension("interchange.json");

    fs::write(&export_path, r#"{"entities":[],"episodes":[]}"#)
        .expect("empty projection export fixture writes");

    let output = run(
        &store,
        &[
            "convert-projection-export",
            export_path.to_str().unwrap(),
            "--output",
            interchange_path.to_str().unwrap(),
            "--json",
        ],
    );

    assert!(
        !output.status.success(),
        "empty projection export should fail closed"
    );
    assert!(
        !interchange_path.exists(),
        "failed conversion should not write an interchange document"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("did not contain convertible memory records"),
        "stderr should explain the conversion failure, got: {stderr}"
    );
}
