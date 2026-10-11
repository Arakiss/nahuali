#[test]
fn init_mcp_config_uses_the_selected_database() {
    let database = temp_database("init-selected-database");
    let output = run_ok(&database, &["init", "--dry-run"]);
    let selected = database.to_str().expect("database name is UTF-8");

    assert!(output.contains("\"--database\""));
    assert!(
        output.contains(&format!("\"{selected}\"")),
        "init output did not contain selected database {selected}:\n{output}"
    );
}

fn nahuali_without_store_env() -> std::process::Command {
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_nahuali"));
    command
        .env_remove("NAHUALI_DB_URL")
        .env_remove("NAHUALI_DB_DATABASE")
        .env("NAHUALI_HOME", std::env::temp_dir().join("nahuali-cli-unused-home"));
    command
}

/// Mirrors the logged friction: `--database` reads like a path but is a
/// database name, and the store location could only be set through
/// `NAHUALI_DB_URL`.
#[test]
fn store_flag_selects_the_embedded_store_directory() {
    let store_dir = temp_store("store-flag-directory");
    let other_dir = temp_store("store-flag-env-directory");
    let store_arg = store_dir.display().to_string();

    let config = nahuali_without_store_env()
        .args(["--store", &store_arg, "config", "--json"])
        .env("NAHUALI_DB_URL", format!("surrealkv://{}", other_dir.display()))
        .output()
        .expect("nahuali runs");
    assert!(config.status.success());
    let config: Value = serde_json::from_slice(&config.stdout).expect("config is JSON");
    assert_eq!(
        config["endpoint"]["value"],
        format!("surrealkv://{}", store_dir.display())
    );
    assert_eq!(config["endpoint"]["source"], "flag", "--store beats NAHUALI_DB_URL");

    let remember = nahuali_without_store_env()
        .args(["--store", &store_arg, "remember", "Arnold uses Helix"])
        .output()
        .expect("nahuali runs");
    assert!(
        remember.status.success(),
        "{}",
        String::from_utf8_lossy(&remember.stderr)
    );
    assert!(store_dir.join("LOCK").exists(), "the store lives at --store");
    assert!(!other_dir.exists());

    let recall = nahuali_without_store_env()
        .args(["--store", &store_arg, "recall", "Helix", "--json"])
        .output()
        .expect("nahuali runs");
    let results: Value = serde_json::from_slice(&recall.stdout).expect("recall is JSON");
    assert_eq!(results[0]["excerpt"], "Arnold uses Helix");

    let path_as_database = nahuali_without_store_env()
        .args(["--store", &store_arg, "--database", "/tmp/my-store", "status"])
        .output()
        .expect("nahuali runs");
    assert!(!path_as_database.status.success());
    let stderr = String::from_utf8_lossy(&path_as_database.stderr);
    assert!(
        stderr.contains("--database names a database inside the store")
            && stderr.contains("--store <PATH>"),
        "{stderr}"
    );

    let help = nahuali_without_store_env()
        .arg("--help")
        .output()
        .expect("nahuali runs");
    let help = String::from_utf8_lossy(&help.stdout);
    assert!(help.contains("--store <PATH|URL>"), "{help}");
    assert!(help.contains("not a path"), "{help}");

    let _ = fs::remove_dir_all(store_dir);
}
