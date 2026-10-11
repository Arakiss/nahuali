fn json_of(store: &Path, args: &[&str]) -> Value {
    let output = run_ok(store, args);
    serde_json::from_str(&output).unwrap_or_else(|error| panic!("{args:?} is JSON: {error}\n{output}"))
}

fn recall_ids(store: &Path, query: &str) -> Vec<String> {
    json_of(store, &["recall", query, "--json"])
        .as_array()
        .expect("recall output is an array")
        .iter()
        .map(|result| result["id"].as_str().unwrap_or_default().to_string())
        .collect()
}

/// Mirrors the logged friction: a user recorded a personal detail, derived a
/// claim from it, and then had no way to erase it. Forgetting must remove the
/// content from the ledger, recall, the graph projection, and derived claims,
/// while leaving an auditable trace that names who, when, and why but never the
/// erased content itself.
#[test]
fn forget_erases_personal_data_and_its_derived_claims_with_an_audit_trace() {
    let store = temp_database("forget-personal-data");
    let secret = "Calle Luna 12";

    let kept = json_of(
        &store,
        &[
            "remember",
            "Petru prefiere reuniones por la manana",
            "--mention",
            "Petru",
            "--json",
        ],
    );
    let episode = json_of(
        &store,
        &[
            "remember",
            "Petru vive en Calle Luna 12, Valencia",
            "--mention",
            "Petru",
            "--json",
        ],
    );
    let episode_id = episode["id"].as_str().unwrap().to_string();
    let claim = json_of(
        &store,
        &["claim", "Petru", "direccion", secret, "--source-last", "--json"],
    );
    let claim_id = claim["id"].as_str().unwrap().to_string();
    let before_audit = json_of(&store, &["audit", "--json"]);
    let before_root = before_audit["integrity"]["merkle_root"].clone();
    assert_eq!(before_audit["total_event_count"], 3);
    let mut recalled = recall_ids(&store, "Calle Luna");
    recalled.sort();
    assert_eq!(recalled, vec![claim_id.clone(), episode_id.clone()]);

    let preview = json_of(
        &store,
        &[
            "forget",
            &episode_id,
            "--reason",
            "GDPR art. 17 request",
            "--requested-by",
            "petru",
            "--dry-run",
            "--json",
        ],
    );
    assert_eq!(preview["dry_run"], true);
    assert_eq!(preview["targets"], serde_json::json!([episode_id]));
    assert_eq!(
        preview["forgotten_ids"],
        serde_json::json!([claim_id, episode_id]),
        "forgetting an episode cascades to the claim derived from it"
    );
    assert_eq!(preview["redacted_event_count"], 2);
    assert_eq!(json_of(&store, &["audit", "--json"])["total_event_count"], 3);
    assert_eq!(recall_ids(&store, "Calle Luna").len(), 2, "dry-run writes nothing");

    let report = json_of(
        &store,
        &[
            "forget",
            &episode_id,
            "--reason",
            "GDPR art. 17 request",
            "--requested-by",
            "petru",
            "--json",
        ],
    );
    assert_eq!(report["dry_run"], false);
    assert_eq!(report["forgotten_ids"], serde_json::json!([claim_id, episode_id]));
    assert_eq!(report["redacted_event_count"], 2);
    let redacted_kinds: Vec<&str> = report["redacted_events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|record| record["kind"].as_str().unwrap())
        .collect();
    assert_eq!(redacted_kinds, vec!["episode_recorded", "fact_asserted"]);
    let forget_id = report["id"].as_str().expect("forget record id").to_string();
    assert!(forget_id.starts_with("forget_"), "{report}");

    assert!(recall_ids(&store, "Calle Luna").is_empty());
    assert!(recall_ids(&store, "direccion").is_empty());
    assert_eq!(recall_ids(&store, "reuniones manana"), vec![kept["id"].as_str().unwrap().to_string()]);

    let data = json_of(&store, &["data", "--json"]);
    assert_eq!(data["episodes"].as_array().unwrap().len(), 1);
    assert_eq!(data["claims"], serde_json::json!([]), "{data}");
    let forgotten = data["forgotten"].as_array().expect("forgotten audit records");
    assert_eq!(forgotten.len(), 1);
    assert_eq!(forgotten[0]["id"], forget_id);
    assert_eq!(forgotten[0]["reason"], "GDPR art. 17 request");
    assert_eq!(forgotten[0]["requested_by"], "petru");
    assert_eq!(forgotten[0]["forgotten_ids"], serde_json::json!([claim_id, episode_id]));
    assert!(forgotten[0]["forgotten_at_ms"].as_u64().unwrap() > 0);

    let audit = json_of(&store, &["audit", "--json"]);
    assert_eq!(audit["total_event_count"], 4);
    assert_eq!(audit["integrity"]["verified"], true, "{audit}");
    assert_eq!(audit["counts"]["memories_forgotten"], 1);
    let kinds: Vec<&str> = audit["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, vec!["episode_recorded", "redacted", "redacted", "memory_forgotten"]);
    let after_prefix = json_of(&store, &["audit", "--to", "3", "--json"]);
    assert_eq!(
        after_prefix["integrity"]["merkle_root"], before_root,
        "redaction keeps the hash chain and Merkle commitments over the prefix"
    );

    let validation = json_of(&store, &["validate", "--json"]);
    assert_eq!(validation["valid"], true, "{validation}");
    let projection = json_of(&store, &["projection-validate", "--json"]);
    assert_eq!(projection["validation"]["valid"], true, "{projection}");
    assert_eq!(projection["validation"]["status"]["in_sync"], true, "{projection}");

    let backup_path = temp_store("forget-backup");
    let backup_arg = backup_path.display().to_string();
    run_ok(&store, &["backup", "--output", &backup_arg, "--json"]);
    let backup_validation = json_of(&store, &["backup-validate", &backup_arg, "--json"]);
    assert_eq!(backup_validation["valid"], true, "{backup_validation}");

    let surfaces = [
        run_ok(&store, &["data", "--json"]),
        run_ok(&store, &["audit", "--json"]),
        run_ok(&store, &["export", "--json"]),
        run_ok(&store, &["timeline", "--json"]),
        run_ok(&store, &["projection-entities", "--json"]),
        run_ok(&store, &["review", "--json"]),
        fs::read_to_string(&backup_path).expect("backup is readable"),
    ];
    let _ = fs::remove_file(&backup_path);
    for surface in surfaces {
        assert!(!surface.contains("Calle Luna"), "erased content resurfaced: {surface}");
        assert!(!surface.contains("direccion"), "erased claim resurfaced: {surface}");
    }
}

#[test]
fn forget_a_claim_keeps_its_source_episode() {
    let store = temp_database("forget-claim-only");
    let episode = json_of(
        &store,
        &["remember", "Petru tiene 41 anos", "--mention", "Petru", "--json"],
    );
    let claim = json_of(
        &store,
        &["claim", "Petru", "edad", "41", "--source-last", "--json"],
    );
    let claim_id = claim["id"].as_str().unwrap();

    let report = json_of(
        &store,
        &["forget", claim_id, "--reason", "wrong age", "--requested-by", "operator", "--json"],
    );
    assert_eq!(report["forgotten_ids"], serde_json::json!([claim_id]));
    assert_eq!(report["redacted_event_count"], 1);

    let data = json_of(&store, &["data", "--json"]);
    assert_eq!(data["episodes"][0]["id"], episode["id"]);
    assert_eq!(data["claims"], serde_json::json!([]), "{data}");
}

#[test]
fn forget_rejects_unknown_ids_and_requires_a_reason() {
    let store = temp_database("forget-validation");
    run_ok(&store, &["remember", "Petru vive en Valencia"]);

    let unknown = run(&store, &["forget", "episode_missing", "--reason", "test"]);
    assert!(!unknown.status.success());
    let stderr = String::from_utf8_lossy(&unknown.stderr);
    assert!(stderr.contains("episode_missing"), "{stderr}");

    let missing_reason = run(&store, &["forget", "episode_missing"]);
    assert!(!missing_reason.status.success());
    assert!(String::from_utf8_lossy(&missing_reason.stderr).contains("--reason"));

    assert_eq!(json_of(&store, &["audit", "--json"])["total_event_count"], 1);
}
