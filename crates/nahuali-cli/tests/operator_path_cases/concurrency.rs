/// Mirrors the logged friction: parallel reads against the default embedded
/// store failed with "LOCK is already locked". Readers and writers launched at
/// the same time must all succeed, and every write must land exactly once.
#[test]
fn parallel_cli_readers_and_writers_share_the_embedded_store() {
    let store = temp_database("parallel-readers-writers");
    run_ok(
        &store,
        &[
            "remember",
            "Arnold uses Helix as his main editor",
            "--mention",
            "Arnold",
        ],
    );
    run_ok(&store, &["claim", "Arnold", "editor", "Helix", "--source-last"]);

    let store_name: &Path = &store;
    let outcomes = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for _ in 0..6 {
            handles.push(scope.spawn(move || {
                (
                    "reader",
                    run(store_name, &["recall", "Arnold editor", "--json"]),
                )
            }));
        }
        for index in 0..2 {
            handles.push(scope.spawn(move || {
                let content = format!("Arnold pairs on the release notes, session {index}");
                (
                    "writer",
                    run(store_name, &["remember", &content, "--json"]),
                )
            }));
        }
        handles
            .into_iter()
            .map(|handle| handle.join().expect("command thread joins"))
            .collect::<Vec<_>>()
    });

    for (role, output) in &outcomes {
        assert!(
            output.status.success(),
            "{role} failed\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if *role == "reader" {
            let results: Value =
                serde_json::from_slice(&output.stdout).expect("reader output is JSON");
            assert_eq!(results[0]["excerpt"], "Arnold editor Helix");
        }
    }

    let validation_output = run_ok(&store, &["validate", "--json"]);
    let validation: Value =
        serde_json::from_str(&validation_output).expect("validation output is JSON");
    assert_eq!(validation["valid"], true);
    assert_eq!(validation["event_count"], 4);
}

/// A process that keeps the embedded store open (an MCP or API server, or a
/// stuck command) makes others wait up to the configured timeout, then fail
/// with an actionable message instead of SurrealKV's raw lock error.
#[test]
fn busy_embedded_store_times_out_with_guidance() {
    use fs2::FileExt;

    let store = temp_database("busy-store-timeout");
    run_ok(&store, &["remember", "Arnold uses Helix"]);

    let lock_path = operator_path_support::endpoint_path(&store).join("LOCK");
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&lock_path)
        .expect("embedded store has a LOCK file");
    lock.try_lock_exclusive()
        .expect("no other process holds the store");

    let started = std::time::Instant::now();
    let output = run_with_env(
        &store,
        &["recall", "Helix", "--json"],
        &[("NAHUALI_STORE_LOCK_TIMEOUT_MS", "400")],
    );
    let elapsed = started.elapsed();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("is busy"), "stderr: {stderr}");
    assert!(stderr.contains("NAHUALI_STORE_LOCK_TIMEOUT_MS"), "stderr: {stderr}");
    assert!(
        stderr.contains("locked by another process (waited"),
        "stderr: {stderr}"
    );
    assert!(
        elapsed >= std::time::Duration::from_millis(400),
        "waited for the configured timeout: {elapsed:?}"
    );

    let invalid = run_with_env(
        &store,
        &["recall", "Helix"],
        &[("NAHUALI_STORE_LOCK_TIMEOUT_MS", "soon")],
    );
    assert!(!invalid.status.success());
    assert!(
        String::from_utf8_lossy(&invalid.stderr)
            .contains("NAHUALI_STORE_LOCK_TIMEOUT_MS must be a whole number")
    );

    lock.unlock().expect("lock releases");
    drop(lock);
    let recovered = run_ok(&store, &["recall", "Helix", "--json"]);
    assert!(recovered.contains("Arnold uses Helix"));
}
