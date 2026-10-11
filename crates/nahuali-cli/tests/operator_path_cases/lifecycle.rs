fn recall_json(store: &Path, args: &[&str]) -> Vec<Value> {
    let output = run_ok(store, args);
    let parsed: Value = serde_json::from_str(&output).expect("recall output is JSON");
    let results = if parsed.is_array() {
        parsed
    } else {
        parsed["results"].clone()
    };
    results
        .as_array()
        .expect("recall results are an array")
        .clone()
}

fn claim_id(store: &Path, subject: &str, predicate: &str, object: &str) -> String {
    let output = run_ok(
        store,
        &["claim", subject, predicate, object, "--source-last", "--json"],
    );
    let claim: Value = serde_json::from_str(&output).expect("claim output is JSON");
    claim["id"]
        .as_str()
        .unwrap_or_else(|| panic!("claim output carries an id: {claim}"))
        .to_string()
}

/// Mirrors the logged friction: Arnold first used Vim (recorded with higher
/// confidence), later switched to Helix. Recall must say which value is current,
/// rank the superseded one below it, and keep saying so after an operator
/// resolves the supersession review item.
#[test]
fn recall_keeps_supersession_visible_before_and_after_review_resolution() {
    let store = temp_database("recall-supersession-lifecycle");

    run_ok(
        &store,
        &[
            "remember",
            "Arnold uses Vim as his main editor",
            "--mention",
            "Arnold",
        ],
    );
    let vim_output = run_ok(
        &store,
        &[
            "claim",
            "Arnold",
            "editor",
            "Vim",
            "--source-last",
            "--confidence",
            "0.95",
            "--json",
        ],
    );
    let vim: Value = serde_json::from_str(&vim_output).expect("claim output is JSON");
    let vim_id = vim["id"]
        .as_str()
        .expect("claim output carries an id")
        .to_string();

    run_ok(
        &store,
        &[
            "remember",
            "Arnold switched from Vim to Helix as his main editor",
            "--mention",
            "Arnold",
        ],
    );
    let helix_id = claim_id(&store, "Arnold", "editor", "Helix");

    let before = recall_json(
        &store,
        &["recall", "Arnold editor", "--kind", "claim", "--json"],
    );
    assert_eq!(before.len(), 2, "both claims stay recallable: {before:?}");
    assert_eq!(before[0]["id"], helix_id.as_str());
    assert_eq!(before[0]["lifecycle"]["status"], "current");
    assert_eq!(before[0]["lifecycle"]["supersedes"], serde_json::json!([vim_id]));
    assert_eq!(before[1]["id"], vim_id.as_str());
    assert_eq!(before[1]["lifecycle"]["status"], "superseded");
    assert_eq!(before[1]["lifecycle"]["superseded_by"], helix_id.as_str());
    assert_eq!(
        before[1]["lifecycle"]["superseded_at_ms"],
        before[0]["created_at_ms"]
    );
    let vim_confidence = before[1]["confidence"].as_f64().expect("confidence");
    assert!((vim_confidence - 0.95).abs() < 1e-6);
    let helix_confidence = before[0]["confidence"].as_f64().expect("confidence");
    assert!((helix_confidence - 0.8).abs() < 1e-6);
    for result in &before {
        let created = result["created_at_ms"].as_u64().expect("created_at_ms");
        let observed = result["observed_at_ms"].as_u64().expect("observed_at_ms");
        assert!(observed <= created, "evidence predates the claim: {result}");
        assert!(result["score_breakdown"]["relevance"].is_number());
        assert!(result["score_breakdown"]["recency"].is_number());
    }
    assert!(
        before[0]["created_at_ms"].as_u64() > before[1]["created_at_ms"].as_u64(),
        "Helix was recorded after Vim"
    );

    let review_output = run_ok(&store, &["review", "--json"]);
    let review: Value = serde_json::from_str(&review_output).expect("review output is JSON");
    let review_id = review["items"]
        .as_array()
        .expect("review items are an array")
        .iter()
        .find(|item| item["title"] == "Superseded memory")
        .and_then(|item| item["id"].as_str())
        .expect("supersession creates a review item")
        .to_string();
    let authority_before = recall_json(
        &store,
        &[
            "recall",
            "Arnold editor",
            "--kind",
            "claim",
            "--authority",
            "--json",
        ],
    );
    assert_eq!(authority_before[0]["id"], helix_id.as_str());
    assert_eq!(authority_before[0]["trust"]["mode"], "certify");
    assert_eq!(authority_before[1]["id"], vim_id.as_str());
    assert_eq!(authority_before[1]["trust"]["mode"], "warn");

    run_ok(
        &store,
        &[
            "review-resolve",
            &review_id,
            "--note",
            "Arnold confirmed he moved to Helix.",
        ],
    );

    let after = recall_json(
        &store,
        &[
            "recall",
            "Arnold editor",
            "--kind",
            "claim",
            "--authority",
            "--json",
        ],
    );
    assert_eq!(after.len(), 2);
    assert_eq!(after[0]["id"], helix_id.as_str());
    assert_eq!(after[0]["lifecycle"]["status"], "current");
    assert_eq!(after[0]["trust"]["mode"], "certify");
    assert_eq!(after[0]["trust"]["can_trust"], true);
    assert_eq!(after[1]["id"], vim_id.as_str());
    assert_eq!(after[1]["lifecycle"]["status"], "superseded");
    assert_eq!(after[1]["lifecycle"]["superseded_by"], helix_id.as_str());
    assert_eq!(after[1]["trust"]["mode"], "advisory");
    assert_eq!(after[1]["trust"]["can_trust"], false);
    assert_eq!(
        after[1]["trust"]["signal_kinds"],
        serde_json::json!(["superseded_fact"])
    );
    assert!(
        after[1]["trust"]["reasons"][0]
            .as_str()
            .unwrap_or_default()
            .contains("Arnold editor Helix"),
        "reason names the superseding value: {}",
        after[1]["trust"]
    );

    let current_only = recall_json(
        &store,
        &[
            "recall",
            "Arnold editor",
            "--kind",
            "claim",
            "--exclude-superseded",
            "--json",
        ],
    );
    assert_eq!(current_only.len(), 1);
    assert_eq!(current_only[0]["id"], helix_id.as_str());

    let human = run_ok(&store, &["recall", "Arnold editor", "--kind", "claim"]);
    let helix_line = human
        .find("Arnold editor Helix")
        .expect("human recall lists Helix");
    let vim_line = human.find("Arnold editor Vim").expect("human recall lists Vim");
    assert!(helix_line < vim_line, "current value is listed first:\n{human}");
    assert!(
        human.contains(&format!("superseded by {helix_id}")),
        "human recall marks the superseded claim:\n{human}"
    );

    let validation_output = run_ok(&store, &["validate", "--json"]);
    let validation: Value =
        serde_json::from_str(&validation_output).expect("validation output is JSON");
    assert_eq!(validation["event_count"], 5, "recall never writes memory");
}

/// A same-subject/predicate disagreement with no clear newest evidence-backed
/// value is a contradiction, not a refresh: neither side is demoted.
#[test]
fn recall_marks_contested_claims_without_picking_a_winner() {
    let store = temp_database("recall-contested-lifecycle");

    run_ok(&store, &["claim", "Lena", "role", "CTO"]);
    run_ok(&store, &["claim", "Lena", "role", "VP Engineering"]);

    let results = recall_json(&store, &["recall", "Lena role", "--kind", "claim", "--json"]);
    assert_eq!(results.len(), 2);
    for result in &results {
        assert_eq!(result["lifecycle"]["status"], "contested", "{result}");
        assert_eq!(
            result["lifecycle"]["contested_with"]
                .as_array()
                .map(Vec::len),
            Some(1)
        );
        assert!(result["lifecycle"]["superseded_by"].is_null());
    }

    let current_only = recall_json(
        &store,
        &[
            "recall",
            "Lena role",
            "--kind",
            "claim",
            "--exclude-superseded",
            "--json",
        ],
    );
    assert_eq!(current_only.len(), 2, "contested claims are never hidden");
}
