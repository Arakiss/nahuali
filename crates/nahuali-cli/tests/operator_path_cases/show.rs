fn json_id(output: &str) -> String {
    let value: Value = serde_json::from_str(output).expect("output is JSON");
    value["id"]
        .as_str()
        .unwrap_or_else(|| panic!("output carries an id: {value}"))
        .to_string()
}

fn ledger_event_count(store: &Path) -> u64 {
    let output = run_ok(store, &["validate", "--json"]);
    let validation: Value = serde_json::from_str(&output).expect("validation output is JSON");
    validation["event_count"].as_u64().expect("event_count")
}

/// The operator pastes an id the CLI printed and gets the record back in full:
/// lifecycle, the ledger events that changed it, and its evidence both ways.
#[test]
fn show_prints_any_printed_id_with_history_and_evidence() {
    let store = temp_database("show-record");

    let episode_id = json_id(&run_ok(
        &store,
        &["remember", "Decided to ship completions", "--json"],
    ));
    let intention_id = json_id(&run_ok(
        &store,
        &[
            "intention",
            "Ship shell completions",
            "--source-last",
            "--priority",
            "high",
            "--json",
        ],
    ));
    run_ok(
        &store,
        &[
            "intention-complete",
            &intention_id,
            "--reason",
            "released in 0.9",
        ],
    );
    let events_before = ledger_event_count(&store);

    let view: Value =
        serde_json::from_str(&run_ok(&store, &["show", &intention_id, "--json"])).expect("JSON");
    assert_eq!(view["kind"], "intention");
    assert_eq!(view["id"], intention_id.as_str());
    assert_eq!(view["status"], "completed");
    assert_eq!(view["record"]["description"], "Ship shell completions");
    assert_eq!(view["record"]["priority"], "high");
    let history = view["history"].as_array().expect("history array");
    assert_eq!(history.len(), 2, "{history:?}");
    assert_eq!(history[0]["kind"], "intention_recorded");
    assert_eq!(history[1]["kind"], "intention_status_changed");
    assert_eq!(history[1]["summary"], "status -> completed: released in 0.9");
    assert_eq!(view["evidence"][0]["relation"], "source_episode");
    assert_eq!(view["evidence"][0]["id"], episode_id.as_str());
    assert_eq!(view["evidence"][0]["kind"], "episode");

    let human = run_ok(&store, &["show", &intention_id]);
    assert!(human.contains(&format!("Intention {intention_id}")), "{human}");
    assert!(human.contains("[completed]"), "{human}");
    assert!(human.contains("History (2)"), "{human}");
    assert!(human.contains("status -> completed: released in 0.9"), "{human}");
    assert!(human.contains(&episode_id), "{human}");

    let episode: Value =
        serde_json::from_str(&run_ok(&store, &["show", &episode_id, "--json"])).expect("JSON");
    assert_eq!(episode["kind"], "episode");
    assert!(
        episode["cited_by"]
            .as_array()
            .expect("cited_by array")
            .iter()
            .any(|reference| reference["id"] == intention_id.as_str()
                && reference["relation"] == "source_episode"),
        "the episode lists the intention that cites it: {episode}"
    );

    let event_id = history[1]["event_id"].as_str().expect("event id").to_string();
    let event: Value =
        serde_json::from_str(&run_ok(&store, &["show", &event_id, "--json"])).expect("JSON");
    assert_eq!(event["kind"], "event");
    assert_eq!(event["evidence"][0]["id"], intention_id.as_str());

    assert_eq!(
        ledger_event_count(&store),
        events_before,
        "show never writes memory"
    );
}

#[test]
fn show_fails_on_an_unknown_id_and_names_the_id_kinds() {
    let store = temp_database("show-unknown");
    run_ok(&store, &["remember", "Something happened"]);

    let output = run(&store, &["show", "intention_does_not_exist"]);

    assert!(!output.status.success(), "unknown ids exit non-zero");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("no memory record has the id \"intention_does_not_exist\""),
        "{stderr}"
    );
    for kind in ["episode_", "intention_", "entity_", "review_decision_", "event_"] {
        assert!(stderr.contains(kind), "error names {kind}: {stderr}");
    }
    assert!(output.stdout.is_empty(), "nothing printed to stdout on failure");
}

#[test]
fn show_explains_a_forgotten_id_with_its_audit_trace() {
    let store = temp_database("show-forgotten");
    let episode_id = json_id(&run_ok(
        &store,
        &["remember", "A private detail to erase", "--json"],
    ));
    run_ok(
        &store,
        &["forget", &episode_id, "--reason", "operator request"],
    );

    let view: Value =
        serde_json::from_str(&run_ok(&store, &["show", &episode_id, "--json"])).expect("JSON");

    assert_eq!(view["kind"], "forgotten");
    assert_eq!(view["status"], "forgotten");
    assert_eq!(view["record"]["reason"], "operator request");
    assert_eq!(view["history"][0]["kind"], "memory_forgotten");
    assert!(
        !view.to_string().contains("A private detail to erase"),
        "the erased content never comes back: {view}"
    );
}
