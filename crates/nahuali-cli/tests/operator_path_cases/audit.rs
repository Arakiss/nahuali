#[test]
fn audit_reports_changes_and_integrity_for_a_range() {
    let store = temp_database("audit-operator-path");

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
    run_ok(
        &store,
        &[
            "intention",
            "Ship release notes",
            "--priority",
            "high",
            "--source-last",
        ],
    );

    let audited = run_ok(&store, &["audit", "--json"]);
    assert_pretty_json(&audited);
    let report: Value = serde_json::from_str(&audited).expect("audit output is JSON");
    assert_eq!(report["from_sequence"], 0);
    assert_eq!(report["to_sequence"], 3);
    assert_eq!(report["total_event_count"], 3);
    assert_eq!(report["range_event_count"], 3);
    assert_eq!(report["integrity"]["verified"], true);
    assert_eq!(report["counts"]["episodes_recorded"], 1);
    assert_eq!(report["counts"]["facts_asserted"], 1);
    assert_eq!(report["counts"]["intentions_recorded"], 1);

    let entries = report["entries"].as_array().expect("entries is an array");
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0]["sequence"], 1);
    assert_eq!(entries[0]["kind"], "episode_recorded");
    assert_eq!(entries[2]["kind"], "intention_recorded");

    let proof_output = run_ok(&store, &["audit", "--inclusion-proof", "2", "--json"]);
    assert_pretty_json(&proof_output);
    let proof_report: Value = serde_json::from_str(&proof_output).expect("audit proof is JSON");
    let proof = &proof_report["inclusion_proof"];
    assert_eq!(proof["sequence"], 2);
    assert_eq!(proof["index"], 1);
    assert_eq!(proof["leaf_count"], 3);
    assert_eq!(proof["event_id"], entries[1]["id"]);
    assert_eq!(proof["merkle_root"], proof_report["integrity"]["merkle_root"]);
    assert_eq!(proof["verified"], true);
    assert!(
        proof["leaf_chain_hash"]
            .as_str()
            .expect("leaf hash")
            .len()
            > 32
    );
    assert!(
        proof["siblings"]
            .as_array()
            .expect("siblings")
            .iter()
            .all(|sibling| sibling.get("hash").is_some() && sibling.get("on_right").is_some())
    );

    // The exclusive lower bound drops the episode at sequence 1.
    let after_first = run_ok(&store, &["audit", "--from", "1", "--json"]);
    let after: Value = serde_json::from_str(&after_first).expect("audit output is JSON");
    assert_eq!(after["from_sequence"], 1);
    assert_eq!(after["range_event_count"], 2);
    assert_eq!(after["counts"]["episodes_recorded"], 0);
    assert_eq!(after["entries"].as_array().expect("entries").len(), 2);

    // The human surface gates on integrity and exits zero for an intact ledger.
    let human = run_ok(&store, &["audit"]);
    assert!(human.contains("Integrity: verified"));
    assert!(human.contains("Changes in range: 3"));
    let proof_human = run_ok(&store, &["audit", "--inclusion-proof", "2"]);
    assert!(proof_human.contains("Merkle inclusion proof: seq 2"));
    assert!(proof_human.contains("Proof verifies: yes"));

    let _ = fs::remove_file(store);
}
