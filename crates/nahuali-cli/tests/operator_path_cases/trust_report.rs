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

    let _ = fs::remove_file(store);
}
