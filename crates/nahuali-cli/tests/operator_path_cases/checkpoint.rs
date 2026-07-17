#[test]
fn checkpoint_v2_operator_path_enforces_external_threshold_policy_and_match_modes() {
    let store = temp_database("checkpoint-v2-operator-path");
    let seed_a_path = temp_store("checkpoint-v2-seed-a");
    let seed_b_path = temp_store("checkpoint-v2-seed-b");
    let policy_path = temp_store("checkpoint-v2-policy");
    let checkpoint_path = temp_store("checkpoint-v2-signed");
    let one_signature_path = temp_store("checkpoint-v2-one-signature");
    let oversized_seed_path = temp_store("checkpoint-v2-oversized-seed");
    let oversized_policy_path = temp_store("checkpoint-v2-oversized-policy");
    let synthetic_seed_a = "11".repeat(32);
    let synthetic_seed_b = "22".repeat(32);

    fs::write(&seed_a_path, &synthetic_seed_a).expect("write first synthetic signing seed");
    fs::write(&seed_b_path, &synthetic_seed_b).expect("write second synthetic signing seed");
    run_ok(
        &store,
        &["remember", "Hrafn retains the signed checkpoint decision"],
    );

    let policy_output = run_ok(
        &store,
        &[
            "checkpoint-policy-init",
            "--origin",
            "operator-test-ledger",
            "--key-id",
            "operator-a",
            "--key-file",
            seed_a_path.to_str().expect("seed path is UTF-8"),
            "--key-id",
            "operator-b",
            "--key-file",
            seed_b_path.to_str().expect("seed path is UTF-8"),
            "--minimum-signatures",
            "2",
            "--output",
            policy_path.to_str().expect("policy path is UTF-8"),
        ],
    );
    assert!(policy_output.contains("threshold of 2"));
    assert!(!policy_output.contains(&synthetic_seed_a));
    assert!(!policy_output.contains(&synthetic_seed_b));

    let policy_before = fs::read(&policy_path).expect("read policy before overwrite attempt");
    let overwrite_policy = run(
        &store,
        &[
            "checkpoint-policy-init",
            "--origin",
            "operator-test-ledger",
            "--key-id",
            "operator-a",
            "--key-file",
            seed_a_path.to_str().expect("seed path is UTF-8"),
            "--minimum-signatures",
            "1",
            "--output",
            policy_path.to_str().expect("policy path is UTF-8"),
        ],
    );
    assert!(!overwrite_policy.status.success());
    assert!(String::from_utf8_lossy(&overwrite_policy.stderr).contains("refusing to overwrite"));
    assert_eq!(
        fs::read(&policy_path).expect("read policy after overwrite attempt"),
        policy_before
    );

    let insufficient = run(
        &store,
        &[
            "checkpoint-sign",
            "--policy",
            policy_path.to_str().expect("policy path is UTF-8"),
            "--key-id",
            "operator-a",
            "--key-file",
            seed_a_path.to_str().expect("seed path is UTF-8"),
            "--output",
            one_signature_path
                .to_str()
                .expect("one-signature path is UTF-8"),
        ],
    );
    assert!(!insufficient.status.success());
    assert!(!one_signature_path.exists());
    assert!(!String::from_utf8_lossy(&insufficient.stderr).contains(&synthetic_seed_a));

    let signed_output = run_ok(
        &store,
        &[
            "checkpoint-sign",
            "--policy",
            policy_path.to_str().expect("policy path is UTF-8"),
            "--key-id",
            "operator-a",
            "--key-file",
            seed_a_path.to_str().expect("seed path is UTF-8"),
            "--key-id",
            "operator-b",
            "--key-file",
            seed_b_path.to_str().expect("seed path is UTF-8"),
            "--output",
            checkpoint_path.to_str().expect("checkpoint path is UTF-8"),
        ],
    );
    assert!(signed_output.contains("2 accepted signature(s)"));
    assert!(!signed_output.contains(&synthetic_seed_a));
    assert!(!signed_output.contains(&synthetic_seed_b));

    let current = run_ok(
        &store,
        &[
            "checkpoint-verify",
            checkpoint_path.to_str().expect("checkpoint path is UTF-8"),
            "--policy",
            policy_path.to_str().expect("policy path is UTF-8"),
            "--json",
        ],
    );
    assert_pretty_json(&current);
    let current_verdict: Value = serde_json::from_str(&current).expect("current verdict is JSON");
    assert_eq!(current_verdict["trusted"], true);
    assert_eq!(current_verdict["match_mode"], "current");
    assert_eq!(current_verdict["accepted_signature_count"], 2);
    assert_eq!(current_verdict["minimum_signature_count"], 2);
    assert!(current_verdict.get("signatures").is_some());

    let checkpoint_before =
        fs::read(&checkpoint_path).expect("read checkpoint before overwrite attempt");
    let overwrite_checkpoint = run(
        &store,
        &[
            "checkpoint-sign",
            "--policy",
            policy_path.to_str().expect("policy path is UTF-8"),
            "--key-id",
            "operator-a",
            "--key-file",
            seed_a_path.to_str().expect("seed path is UTF-8"),
            "--key-id",
            "operator-b",
            "--key-file",
            seed_b_path.to_str().expect("seed path is UTF-8"),
            "--output",
            checkpoint_path.to_str().expect("checkpoint path is UTF-8"),
        ],
    );
    assert!(!overwrite_checkpoint.status.success());
    assert_eq!(
        fs::read(&checkpoint_path).expect("read checkpoint after overwrite attempt"),
        checkpoint_before
    );

    run_ok(
        &store,
        &["remember", "A later append remains outside the checkpoint"],
    );
    let stale_current = run(
        &store,
        &[
            "checkpoint-verify",
            checkpoint_path.to_str().expect("checkpoint path is UTF-8"),
            "--policy",
            policy_path.to_str().expect("policy path is UTF-8"),
            "--mode",
            "current",
            "--json",
        ],
    );
    assert!(!stale_current.status.success());
    let stale_verdict: Value = serde_json::from_slice(&stale_current.stdout)
        .expect("untrusted current verdict remains valid JSON");
    assert_eq!(stale_verdict["trusted"], false);
    assert_eq!(stale_verdict["appended_event_count"], 1);
    assert_eq!(stale_verdict["current_size_matches"], false);

    let historical = run_ok(
        &store,
        &[
            "checkpoint-verify",
            checkpoint_path.to_str().expect("checkpoint path is UTF-8"),
            "--policy",
            policy_path.to_str().expect("policy path is UTF-8"),
            "--mode",
            "historical",
            "--json",
        ],
    );
    let historical_verdict: Value =
        serde_json::from_str(&historical).expect("historical verdict is JSON");
    assert_eq!(historical_verdict["trusted"], true);
    assert_eq!(historical_verdict["match_mode"], "historical");
    assert_eq!(historical_verdict["appended_event_count"], 1);
    let historical_human = run_ok(
        &store,
        &[
            "checkpoint-verify",
            checkpoint_path.to_str().expect("checkpoint path is UTF-8"),
            "--policy",
            policy_path.to_str().expect("policy path is UTF-8"),
            "--mode",
            "historical",
        ],
    );
    assert!(historical_human.contains("TRUSTED HISTORICAL CHECKPOINT"));
    assert!(historical_human.contains("1 event(s) were appended after it"));

    let mismatched_pairs = run(
        &store,
        &[
            "checkpoint-policy-init",
            "--origin",
            "operator-test-ledger",
            "--key-id",
            "operator-a",
            "--key-id",
            "operator-b",
            "--key-file",
            seed_a_path.to_str().expect("seed path is UTF-8"),
            "--output",
            one_signature_path
                .to_str()
                .expect("mismatch output path is UTF-8"),
        ],
    );
    assert!(!mismatched_pairs.status.success());
    assert!(String::from_utf8_lossy(&mismatched_pairs.stderr).contains("same number of times"));

    fs::write(&oversized_seed_path, vec![b'0'; 64 * 1024 + 1])
        .expect("write oversized signing key input");
    let oversized_seed = run(
        &store,
        &[
            "checkpoint-policy-init",
            "--origin",
            "operator-test-ledger",
            "--key-id",
            "operator-a",
            "--key-file",
            oversized_seed_path
                .to_str()
                .expect("oversized seed path is UTF-8"),
            "--output",
            one_signature_path
                .to_str()
                .expect("oversized seed output path is UTF-8"),
        ],
    );
    assert!(!oversized_seed.status.success());
    assert!(String::from_utf8_lossy(&oversized_seed.stderr).contains("64 KiB input limit"));

    fs::write(&oversized_policy_path, vec![b' '; 64 * 1024 + 1])
        .expect("write oversized policy input");
    let oversized_policy = run(
        &store,
        &[
            "checkpoint-verify",
            checkpoint_path.to_str().expect("checkpoint path is UTF-8"),
            "--policy",
            oversized_policy_path
                .to_str()
                .expect("oversized policy path is UTF-8"),
            "--json",
        ],
    );
    assert!(!oversized_policy.status.success());
    assert!(String::from_utf8_lossy(&oversized_policy.stderr).contains("64 KiB input limit"));

    for path in [
        seed_a_path,
        seed_b_path,
        policy_path,
        checkpoint_path,
        one_signature_path,
        oversized_seed_path,
        oversized_policy_path,
    ] {
        let _ = fs::remove_file(path);
    }
}
