#[test]
fn trust_report_composes_knowledge_authority_integrity_and_health() {
    let store = temp_store("trust-report-operator-path");

    run_ok(
        &store,
        &["remember", "Lena owns the release notes", "--tag", "product"],
    );
    run_ok(
        &store,
        &[
            "claim",
            "Lena",
            "owns",
            "release notes",
            "--confidence",
            "0.92",
            "--source-last",
        ],
    );

    let json = run_ok(&store, &["trust-report", "--json"]);
    assert_pretty_json(&json);
    let report: Value = serde_json::from_str(&json).expect("trust-report output is JSON");
    assert_eq!(report["version"], 1);
    assert_eq!(report["knowledge"]["episode_count"], 1);
    assert_eq!(report["knowledge"]["claim_count"], 1);
    assert_eq!(report["integrity"]["ledger_verified"], true);
    assert!(report["authority"]["mode"].is_string());
    assert!(report["health"].is_object());
    assert!(report["trustworthy"].is_boolean());
    assert!(
        report["verdict_reasons"]
            .as_array()
            .expect("verdict_reasons is an array")
            .iter()
            .any(|reason| reason
                .as_str()
                .is_some_and(|reason| reason.contains("ledger integrity verified")))
    );

    let human = run_ok(&store, &["trust-report"]);
    assert!(human.contains("Memory trust report"));
    assert!(human.contains("Integrity: verified"));

    let html_path = temp_store("trust-report-html-out");
    run_ok(
        &store,
        &[
            "trust-report",
            "--html",
            html_path.to_str().expect("temp path is UTF-8"),
        ],
    );
    let html = fs::read_to_string(&html_path).expect("the HTML dossier was written");
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("Memory Trust Report"));
    assert!(!html.contains("http://"));
    assert!(!html.contains("https://"));

    let _ = fs::remove_file(html_path);
    let _ = fs::remove_file(store);
}
