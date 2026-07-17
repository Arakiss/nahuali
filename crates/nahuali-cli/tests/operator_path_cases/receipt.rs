#[test]
fn claim_receipt_exports_privately_and_verifies_without_opening_a_store() {
    use std::path::Path;

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    let store = temp_database("claim-receipt-operator-path");
    let seed_path = temp_store("claim-receipt-seed");
    let policy_path = temp_store("claim-receipt-policy");
    let checkpoint_path = temp_store("claim-receipt-checkpoint");
    let receipt_path = temp_store("claim-receipt-document");
    let tampered_path = temp_store("claim-receipt-tampered");
    let synthetic_seed = "31".repeat(32);
    fs::write(&seed_path, &synthetic_seed).expect("write synthetic signing seed");

    run_ok(
        &store,
        &[
            "remember",
            "Hrafn observed the portable receipt decision",
            "--tag",
            "evidence",
        ],
    );
    let claim_output = run_ok(
        &store,
        &[
            "claim",
            "Hrafn",
            "retains",
            "portable receipts",
            "--source-last",
            "--confidence",
            "0.95",
            "--json",
        ],
    );
    let claim: Value = serde_json::from_str(&claim_output).expect("claim output is JSON");
    let claim_id = claim["id"].as_str().expect("claim has an id");

    run_ok(
        &store,
        &[
            "checkpoint-policy-init",
            "--origin",
            "operator-receipt-ledger",
            "--key-id",
            "operator-receipt",
            "--key-file",
            seed_path.to_str().expect("seed path is UTF-8"),
            "--output",
            policy_path.to_str().expect("policy path is UTF-8"),
        ],
    );
    run_ok(
        &store,
        &[
            "checkpoint-sign",
            "--policy",
            policy_path.to_str().expect("policy path is UTF-8"),
            "--key-id",
            "operator-receipt",
            "--key-file",
            seed_path.to_str().expect("seed path is UTF-8"),
            "--output",
            checkpoint_path.to_str().expect("checkpoint path is UTF-8"),
        ],
    );

    let exported = run_ok(
        &store,
        &[
            "receipt-export",
            "--claim-id",
            claim_id,
            "--checkpoint",
            checkpoint_path.to_str().expect("checkpoint path is UTF-8"),
            "--policy",
            policy_path.to_str().expect("policy path is UTF-8"),
            "--output",
            receipt_path.to_str().expect("receipt path is UTF-8"),
        ],
    );
    assert!(exported.contains("2 selected event(s)"));
    assert!(exported.contains("ledger-committed evidence only"));
    assert!(!exported.contains("Hrafn observed"));
    assert!(!exported.contains(&synthetic_seed));

    #[cfg(unix)]
    assert_eq!(
        fs::metadata(&receipt_path)
            .expect("receipt metadata")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );

    let receipt_before = fs::read(&receipt_path).expect("read receipt before overwrite attempt");
    let overwrite = run(
        &store,
        &[
            "receipt-export",
            "--claim-id",
            claim_id,
            "--checkpoint",
            checkpoint_path.to_str().expect("checkpoint path is UTF-8"),
            "--policy",
            policy_path.to_str().expect("policy path is UTF-8"),
            "--output",
            receipt_path.to_str().expect("receipt path is UTF-8"),
        ],
    );
    assert!(!overwrite.status.success());
    assert!(String::from_utf8_lossy(&overwrite.stderr).contains("refusing to overwrite"));
    assert_eq!(
        fs::read(&receipt_path).expect("read receipt after overwrite attempt"),
        receipt_before
    );

    let offline = run_at_endpoint(
        Path::new("invalid/database/path"),
        Path::new("unreachable-receipt-store"),
        &[
            "receipt-verify",
            receipt_path.to_str().expect("receipt path is UTF-8"),
            "--policy",
            policy_path.to_str().expect("policy path is UTF-8"),
            "--json",
        ],
    );
    assert!(
        offline.status.success(),
        "offline verification failed: {}",
        String::from_utf8_lossy(&offline.stderr)
    );
    let verdict: Value =
        serde_json::from_slice(&offline.stdout).expect("offline verdict is JSON");
    assert_eq!(verdict["receipt_integrity"]["verified"], true);
    assert_eq!(verdict["receipt_integrity"]["checkpoint_authorized"], true);
    assert_eq!(verdict["selected_event_count"], 2);
    assert_eq!(
        verdict["content_authority"]["classification"],
        "ledger_committed_evidence"
    );
    assert_eq!(verdict["content_authority"]["claim_truth_verified"], false);
    assert_eq!(
        verdict["content_authority"]["external_source_authenticity_verified"],
        false
    );
    assert_eq!(
        fs::read(&receipt_path).expect("verification leaves receipt unchanged"),
        receipt_before
    );

    let mut tampered: Value =
        serde_json::from_slice(&receipt_before).expect("receipt document is JSON");
    tampered["claim_event"]["event"]["payload"]["object"] =
        Value::String("rewritten history".to_string());
    fs::write(
        &tampered_path,
        serde_json::to_vec_pretty(&tampered).expect("encode tampered receipt"),
    )
    .expect("write tampered receipt");
    let rejected = run_at_endpoint(
        Path::new("invalid/database/path"),
        Path::new("unreachable-receipt-store"),
        &[
            "receipt-verify",
            tampered_path.to_str().expect("tampered path is UTF-8"),
            "--policy",
            policy_path.to_str().expect("policy path is UTF-8"),
            "--json",
        ],
    );
    assert!(!rejected.status.success());
    let rejected_verdict: Value =
        serde_json::from_slice(&rejected.stdout).expect("rejection remains valid JSON");
    assert_eq!(rejected_verdict["receipt_integrity"]["verified"], false);
    assert_eq!(
        rejected_verdict["receipt_integrity"]["selected_event_checksums_valid"],
        false
    );

    for path in [
        seed_path,
        policy_path,
        checkpoint_path,
        receipt_path,
        tampered_path,
    ] {
        let _ = fs::remove_file(path);
    }
}
