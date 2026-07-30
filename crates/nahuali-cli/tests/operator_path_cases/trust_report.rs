#[test]
fn trust_report_composes_knowledge_authority_integrity_and_health() {
    let store = temp_database("trust-report-operator-path");

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
    assert_eq!(report["version"], 2);
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
                .is_some_and(|reason| reason.contains("recorded-history checks passed")))
    );

    let human = run_ok(&store, &["trust-report"]);
    assert!(human.contains("Memory trust report"));
    assert!(human.contains("Integrity: verified"));

    #[cfg(feature = "attestation")]
    {
        let seed_path = temp_store("trust-report-seed");
        let receipt_path = temp_store("trust-report-receipt");
        let keyring_path = temp_store("trust-report-keyring");
        fs::write(&seed_path, "01".repeat(32)).expect("write test signing seed");
        let signed = run_ok(
            &store,
            &[
                "attest-sign",
                "--key-file",
                seed_path.to_str().expect("seed path is UTF-8"),
                "--output",
                receipt_path.to_str().expect("receipt path is UTF-8"),
                "--json",
            ],
        );
        let receipt: Value = serde_json::from_str(&signed).expect("receipt is JSON");

        let self_signed = run(
            &store,
            &[
                "trust-report",
                "--attestation",
                receipt_path.to_str().expect("receipt path is UTF-8"),
                "--json",
            ],
        );
        assert!(!self_signed.status.success());
        let self_signed_report: Value =
            serde_json::from_slice(&self_signed.stdout).expect("failed report is still JSON");
        assert_eq!(self_signed_report["attestation"]["trusted"], false);
        assert!(
            self_signed_report["attestation"]
                .get("signer_authorized")
                .is_none(),
            "a self-signed receipt must not imply signer authorization"
        );

        fs::write(
            &keyring_path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "keys": [{
                    "key_id": "test-primary",
                    "public_key": receipt["public_key"],
                    "status": "active"
                }]
            }))
            .expect("serialize keyring"),
        )
        .expect("write test keyring");
        let trusted = run_ok(
            &store,
            &[
                "trust-report",
                "--attestation",
                receipt_path.to_str().expect("receipt path is UTF-8"),
                "--keyring",
                keyring_path.to_str().expect("keyring path is UTF-8"),
                "--json",
            ],
        );
        let trusted_report: Value =
            serde_json::from_str(&trusted).expect("trusted report is JSON");
        assert_eq!(trusted_report["attestation"]["trusted"], true);
        assert_eq!(trusted_report["attestation"]["signer_authorized"], true);

        let _ = fs::remove_file(seed_path);
        let _ = fs::remove_file(receipt_path);
        let _ = fs::remove_file(keyring_path);
    }

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
