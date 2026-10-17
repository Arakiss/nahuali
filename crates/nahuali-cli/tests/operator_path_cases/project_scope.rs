fn record_project_work(store: &Path) -> (String, String) {
    run_ok(
        store,
        &["episode", "Cut the beta release", "-m", "Nahuali"],
    );
    let nahuali: Value = serde_json::from_str(&run_ok(
        store,
        &["intention", "Publish release notes", "--source-last", "--json"],
    ))
    .expect("intention output is JSON");
    run_ok(store, &["episode", "Deployed the API", "-m", "Orvian"]);
    let orvian: Value = serde_json::from_str(&run_ok(
        store,
        &[
            "intention",
            "Rotate the API keys",
            "--source-last",
            "--priority",
            "critical",
            "--json",
        ],
    ))
    .expect("intention output is JSON");
    (
        nahuali["id"].as_str().expect("id").to_string(),
        orvian["id"].as_str().expect("id").to_string(),
    )
}

/// The briefing mixed every project; `--project` keeps one project's episodes
/// and intentions, and always says that a scope is active.
#[test]
fn briefing_and_pending_scope_to_one_project() {
    let store = temp_database("project-scope");
    let (nahuali_id, orvian_id) = record_project_work(&store);

    let briefing: Value = serde_json::from_str(&run_ok(
        &store,
        &["briefing", "--project", "nahuali", "--json"],
    ))
    .expect("briefing output is JSON");
    let report = &briefing["report"];
    assert_eq!(report["project_scope"]["project"], "nahuali");
    assert_eq!(report["project_scope"]["scope_key"], "project:nahuali");
    assert_eq!(report["project_scope"]["matched_entity"], "Nahuali");
    assert_eq!(report["project_scope"]["episode_count"], 1);
    let intentions = report["active_intentions"]
        .as_array()
        .expect("intentions array");
    assert_eq!(intentions.len(), 1, "{intentions:?}");
    assert_eq!(intentions[0]["id"], nahuali_id.as_str());
    assert_eq!(report["summary"]["active_intention_count"], 1);
    let episodes = report["recent_episodes"].as_array().expect("episodes array");
    assert_eq!(episodes.len(), 1);
    assert_eq!(episodes[0]["content"], "Cut the beta release");

    let unscoped: Value = serde_json::from_str(&run_ok(&store, &["briefing", "--json"]))
        .expect("briefing output is JSON");
    assert!(unscoped["report"]["project_scope"].is_null());
    assert_eq!(unscoped["report"]["summary"]["active_intention_count"], 2);

    let human = run_ok(&store, &["briefing", "--project", "nahuali"]);
    assert!(human.contains("Scope: project nahuali"), "{human}");
    assert!(human.contains("Publish release notes"), "{human}");
    assert!(!human.contains("Rotate the API keys"), "{human}");

    let pending: Value = serde_json::from_str(&run_ok(
        &store,
        &["pending", "--project", "orvian", "--json"],
    ))
    .expect("pending output is JSON");
    assert_eq!(pending["project_scope"]["project"], "orvian");
    let pending_ids = pending["intentions"]
        .as_array()
        .expect("intentions array")
        .iter()
        .map(|intention| intention["memory_id"].as_str().unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(pending_ids, vec![orvian_id.as_str()]);

    let stale = run_ok(&store, &["pending", "--project", "orvian", "--stale", "30"]);
    assert!(stale.contains("Scope: project orvian"), "{stale}");
    assert!(
        stale.contains("No stale active intentions are attributed to project orvian."),
        "{stale}"
    );

    let unknown = run_ok(&store, &["pending", "--project", "atlas"]);
    assert!(
        unknown.contains("Scope: project atlas matched no episodes or intentions"),
        "{unknown}"
    );
}

#[test]
fn an_empty_project_name_is_rejected() {
    let store = temp_database("project-scope-empty");
    run_ok(&store, &["remember", "Something happened"]);

    let output = run(&store, &["briefing", "--project", "  "]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("project name cannot be empty"), "{stderr}");
}
