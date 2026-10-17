fn new_intention(store: &Path, description: &str) -> String {
    let output = run_ok(store, &["intention", description, "--json"]);
    let intention: Value = serde_json::from_str(&output).expect("intention output is JSON");
    intention["id"].as_str().expect("intention id").to_string()
}

fn recorded_events(store: &Path) -> u64 {
    let output = run_ok(store, &["validate", "--json"]);
    let validation: Value = serde_json::from_str(&output).expect("validation output is JSON");
    validation["event_count"].as_u64().expect("event_count")
}

/// Hundreds of stale intentions cannot be closed one by one: the operator
/// names a set, previews it, then applies it as ordinary lifecycle events.
#[test]
fn reconcile_intentions_closes_exactly_the_named_intentions() {
    let store = temp_database("intention-closure");
    let first = new_intention(&store, "Port the old exporter");
    let second = new_intention(&store, "Retire the legacy dashboard");
    let kept = new_intention(&store, "Keep this one open");
    let done = new_intention(&store, "Already shipped");
    run_ok(&store, &["intention-complete", &done, "--reason", "shipped"]);
    let before = recorded_events(&store);

    let preview: Value = serde_json::from_str(&run_ok(
        &store,
        &[
            "reconcile-intentions",
            "--close",
            "abandoned",
            "--id",
            &first,
            "--id",
            &second,
            "--id",
            &done,
            "--reason",
            "stale cleanup",
            "--json",
        ],
    ))
    .expect("preview is JSON");
    assert_eq!(preview["dry_run"], true);
    assert_eq!(preview["requested_count"], 3);
    let previewed = preview["changes"]
        .as_array()
        .expect("changes array")
        .iter()
        .map(|change| change["id"].as_str().unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(previewed, vec![first.as_str(), second.as_str()]);
    assert!(preview["changes"][0]["event_id"].is_null());
    assert_eq!(preview["skipped"][0]["id"], done.as_str());
    assert_eq!(recorded_events(&store), before, "a preview writes nothing");

    let human = run_ok(
        &store,
        &[
            "reconcile-intentions",
            "--close",
            "cancelled",
            "--id",
            &first,
            "--reason",
            "stale cleanup",
        ],
    );
    assert!(
        human.contains("Dry run: would mark 1 intention(s) abandoned"),
        "{human}"
    );
    assert!(human.contains("Nothing was written"), "{human}");

    let id_file = temp_store("intention-closure-ids");
    fs::write(
        &id_file,
        format!("{first}  # exporter\n\n{second}\n# {kept} stays open\n{done}\n"),
    )
    .expect("id file writes");
    let applied: Value = serde_json::from_str(&run_ok(
        &store,
        &[
            "reconcile-intentions",
            "--close",
            "abandoned",
            "--ids-from",
            id_file.to_str().expect("utf-8 path"),
            "--reason",
            "stale cleanup",
            "--apply",
            "--json",
        ],
    ))
    .expect("applied report is JSON");
    let _ = fs::remove_file(&id_file);
    assert_eq!(applied["dry_run"], false);
    let changes = applied["changes"].as_array().expect("changes array");
    assert_eq!(changes.len(), 2);
    assert_eq!(recorded_events(&store), before + 2, "one event per closure");

    for (change, id) in changes.iter().zip([&first, &second]) {
        assert_eq!(change["id"], id.as_str());
        assert_eq!(change["previous_status"], "active");
        assert_eq!(change["status"], "abandoned");
        let event_id = change["event_id"].as_str().expect("applied closures carry an event");
        let shown: Value =
            serde_json::from_str(&run_ok(&store, &["show", id, "--json"])).expect("JSON");
        assert_eq!(shown["status"], "abandoned");
        let last = shown["history"]
            .as_array()
            .and_then(|history| history.last())
            .expect("closure is in the history");
        assert_eq!(last["event_id"], event_id);
        assert_eq!(last["kind"], "intention_status_changed");
        assert_eq!(last["summary"], "status -> abandoned: stale cleanup");
    }
    let untouched: Value =
        serde_json::from_str(&run_ok(&store, &["show", &kept, "--json"])).expect("JSON");
    assert_eq!(untouched["status"], "active", "unnamed intentions stay open");
}

#[test]
fn reconcile_intentions_rejects_unknown_ids_and_missing_arguments_without_writing() {
    let store = temp_database("intention-closure-guards");
    let open = new_intention(&store, "Still open");
    let before = recorded_events(&store);

    let unknown = run(
        &store,
        &[
            "reconcile-intentions",
            "--close",
            "completed",
            "--id",
            &open,
            "--id",
            "intention_ghost",
            "--reason",
            "done",
            "--apply",
        ],
    );
    assert!(!unknown.status.success());
    let stderr = String::from_utf8_lossy(&unknown.stderr);
    assert!(stderr.contains("unknown intention: intention_ghost"), "{stderr}");
    assert_eq!(recorded_events(&store), before, "a bad id writes nothing");

    for args in [
        vec!["reconcile-intentions", "--close", "completed", "--reason", "x"],
        vec!["reconcile-intentions", "--close", "completed", "--id", open.as_str()],
        vec!["reconcile-intentions", "--id", open.as_str(), "--apply"],
        vec![
            "reconcile-intentions",
            "--close",
            "deferred",
            "--id",
            open.as_str(),
            "--reason",
            "x",
        ],
    ] {
        let output = run(&store, &args);
        assert!(!output.status.success(), "{args:?} must be rejected");
    }
    assert_eq!(recorded_events(&store), before);
}
