#[test]
fn scoped_operator_paths_filter_recall() {
    let store = temp_store("scoped-operator-path");

    run_ok(
        &store,
        &[
            "remember",
            "Lena owns Nahuali release notes",
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
            "release notes",
            "--scope",
            "project:nahuali",
            "--source-last",
        ],
    );
    run_ok(
        &store,
        &[
            "claim",
            "Lena",
            "drafts",
            "release notes",
            "--scope",
            "project:nahuali",
        ],
    );
    run_ok(
        &store,
        &[
            "remember",
            "Lena owns Atlas release notes",
            "--mention",
            "Lena",
            "--scope",
            "project:atlas",
        ],
    );

    let scoped_json = run_ok(
        &store,
        &[
            "recall",
            "release notes",
            "--scope",
            "project:nahuali",
            "--json",
        ],
    );
    let results: Value = serde_json::from_str(&scoped_json).expect("recall output is JSON");
    let results = results.as_array().expect("recall JSON is an array");
    assert!(!results.is_empty());
    assert!(results.iter().all(|result| {
        result["scope"]["key"]
            .as_str()
            .is_some_and(|key| key == "project:nahuali")
    }));
    assert!(
        !results
            .iter()
            .any(|result| result["excerpt"].as_str().unwrap_or_default().contains("Atlas"))
    );

    let scoped_human = run_ok(
        &store,
        &["recall", "release notes", "--scope", "project:nahuali"],
    );
    assert!(scoped_human.contains("scope: project:nahuali"));

    let authority_json = run_ok(
        &store,
        &[
            "recall",
            "release notes",
            "--authority",
            "--scope",
            "project:nahuali",
            "--json",
        ],
    );
    let authority: Value =
        serde_json::from_str(&authority_json).expect("authority recall output is JSON");
    assert!(
        authority["results"]
            .as_array()
            .expect("authority recall includes results")
            .iter()
            .all(|result| result["scope"]["key"] == "project:nahuali")
    );

    let filtered_json = run_ok(
        &store,
        &[
            "recall",
            "release notes",
            "--scope",
            "project:nahuali",
            "--kind",
            "claim",
            "--require-evidence",
            "--json",
        ],
    );
    let filtered: Value =
        serde_json::from_str(&filtered_json).expect("filtered recall output is JSON");
    let filtered = filtered.as_array().expect("filtered recall JSON is an array");
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0]["kind"], "claim");
    assert!(
        filtered[0]["evidence_id"]
            .as_str()
            .is_some_and(|id| id.starts_with("episode_"))
    );
    assert!(filtered[0]["excerpt"].as_str().unwrap_or_default().contains("owns"));

    let filtered_authority_json = run_ok(
        &store,
        &[
            "recall",
            "release notes",
            "--authority",
            "--scope",
            "project:nahuali",
            "--kind",
            "claim",
            "--require-evidence",
            "--json",
        ],
    );
    let filtered_authority: Value = serde_json::from_str(&filtered_authority_json)
        .expect("filtered authority recall output is JSON");
    assert_eq!(filtered_authority["results"].as_array().unwrap().len(), 1);
    assert_eq!(filtered_authority["results"][0]["kind"], "claim");

}
