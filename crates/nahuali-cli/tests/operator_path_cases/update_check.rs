/// A release far ahead of any real build, so the seeded notice always fires.
const FAKE_NEWER_RELEASE: &str = "99.0.0";

/// Enables the update check against an isolated home. The releases URL points
/// at a closed local port, so a check that wrongly reaches the network fails
/// fast instead of calling GitHub.
fn update_check_env(home: &Path) -> Vec<(&'static str, String)> {
    vec![
        ("NAHUALI_NO_UPDATE_CHECK", "0".to_string()),
        ("CI", "0".to_string()),
        ("NAHUALI_HOME", home.display().to_string()),
        ("NAHUALI_UPDATE_CHECK_URL", "http://127.0.0.1:9/releases".to_string()),
    ]
}

fn run_update_env(store: &Path, args: &[&str], envs: &[(&'static str, String)]) -> String {
    let envs = envs
        .iter()
        .map(|(key, value)| (*key, value.as_str()))
        .collect::<Vec<_>>();
    let output = run_with_env(store, args, &envs);
    assert!(
        output.status.success(),
        "command failed\nargs: {args:?}\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("stdout is UTF-8")
}

fn seed_fresh_update_cache(home: &Path, latest: &str) {
    fs::create_dir_all(home).expect("home is created");
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock is after epoch")
        .as_millis();
    fs::write(
        home.join("update-check.json"),
        format!(r#"{{"checked_at_ms": {now_ms}, "latest_version": "{latest}"}}"#),
    )
    .expect("cache is written");
}

#[test]
fn briefing_text_shows_a_cached_update_notice_that_json_never_carries() {
    let database = temp_database("update-notice-briefing");
    let home = temp_store("update-notice-briefing-home");
    seed_fresh_update_cache(&home, FAKE_NEWER_RELEASE);
    let envs = update_check_env(&home);
    let expected = format!(
        "Update available: {FAKE_NEWER_RELEASE} (you have {}) · https://github.com/Arakiss/nahuali/releases/tag/v{FAKE_NEWER_RELEASE}",
        env!("CARGO_PKG_VERSION")
    );

    for command in ["briefing", "session-resume"] {
        let human = run_update_env(&database, &[command], &envs);
        assert_eq!(
            human.matches("Update available:").count(),
            1,
            "{command} text shows the notice once:\n{human}"
        );
        assert!(human.contains(&expected), "{command} text:\n{human}");

        let json = run_update_env(&database, &[command, "--json"], &envs);
        assert!(
            !json.contains("Update available") && !json.contains(FAKE_NEWER_RELEASE),
            "{command} --json must never carry the notice:\n{json}"
        );
        serde_json::from_str::<Value>(&json).expect("JSON stays parseable");
    }

    let status = run_update_env(&database, &["status"], &envs);
    assert!(!status.contains("Update available"), "status:\n{status}");
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn opt_out_and_ci_silence_the_briefing_notice() {
    let database = temp_database("update-notice-opt-out");
    let home = temp_store("update-notice-opt-out-home");
    seed_fresh_update_cache(&home, FAKE_NEWER_RELEASE);

    for (key, value) in [("NAHUALI_NO_UPDATE_CHECK", "1"), ("CI", "true")] {
        let mut envs = update_check_env(&home);
        envs.retain(|(name, _)| *name != key);
        envs.push((key, value.to_string()));
        let human = run_update_env(&database, &["briefing"], &envs);
        assert!(
            !human.contains("Update available"),
            "{key}={value} must silence the notice:\n{human}"
        );
    }
    let _ = fs::remove_dir_all(&home);
}

/// Serves one canned HTTP response and returns the request line it received.
fn serve_release_list_once(body: String) -> (String, std::thread::JoinHandle<String>) {
    use std::io::{Read, Write};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind a local port");
    let url = format!(
        "http://{}/repos/Arakiss/nahuali/releases",
        listener.local_addr().expect("local address")
    );
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("one request arrives");
        let mut request = [0_u8; 8192];
        let read = stream.read(&mut request).expect("request is readable");
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream
            .write_all(response.as_bytes())
            .expect("response is written");
        String::from_utf8_lossy(&request[..read]).into_owned()
    });
    (url, server)
}

fn fake_release_list(tag: &str) -> String {
    let version = tag.trim_start_matches('v');
    let assets = [
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
    ]
    .iter()
    .map(|target| format!(r#"{{"name":"nahuali-v{version}-{target}.tar.gz"}}"#))
    .collect::<Vec<_>>()
    .join(",");
    format!(
        r#"[{{"tag_name":"nahuali-v{version}","draft":false,"assets":[{{"name":"sbom.cdx.json"}}]}},
            {{"tag_name":"{tag}","draft":false,"prerelease":true,"assets":[{assets}]}}]"#
    )
}

#[test]
fn update_check_fetches_now_prints_the_pinned_upgrade_and_caches_the_answer() {
    let database = temp_database("update-check-command");
    let home = temp_store("update-check-command-home");
    let (url, server) = serve_release_list_once(fake_release_list("v99.0.0-beta.2"));
    let mut envs = update_check_env(&home);
    envs.retain(|(name, _)| *name != "NAHUALI_UPDATE_CHECK_URL");
    envs.push(("NAHUALI_UPDATE_CHECK_URL", url));
    // An explicit check is not an automatic one: CI does not suppress it.
    envs.retain(|(name, _)| *name != "CI");
    envs.push(("CI", "true".to_string()));

    let output = run_update_env(&database, &["update-check"], &envs);
    let request = server.join().expect("server thread finishes");
    assert!(request.starts_with("GET /repos/Arakiss/nahuali/releases "), "{request}");
    assert!(
        request
            .to_ascii_lowercase()
            .contains(&format!("user-agent: nahuali/{}", env!("CARGO_PKG_VERSION"))),
        "{request}"
    );
    assert!(
        output.contains(&format!(
            "Update available: 99.0.0-beta.2 (you have {})",
            env!("CARGO_PKG_VERSION")
        )),
        "{output}"
    );
    assert!(
        output.contains(
            "curl -fsSLo /tmp/nahuali-install.sh https://raw.githubusercontent.com/Arakiss/nahuali/v99.0.0-beta.2/scripts/install.sh && NAHUALI_VERSION=v99.0.0-beta.2 sh /tmp/nahuali-install.sh"
        ),
        "{output}"
    );

    let cache: Value = serde_json::from_slice(
        &fs::read(home.join("update-check.json")).expect("the check is cached"),
    )
    .expect("cache is JSON");
    assert_eq!(cache["latest_version"], "99.0.0-beta.2");
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn update_check_json_reports_the_result_and_opt_out_makes_no_call() {
    let database = temp_database("update-check-json");
    let home = temp_store("update-check-json-home");
    let (url, server) = serve_release_list_once(fake_release_list("v99.0.0"));
    let mut envs = update_check_env(&home);
    envs.retain(|(name, _)| *name != "NAHUALI_UPDATE_CHECK_URL");
    envs.push(("NAHUALI_UPDATE_CHECK_URL", url));

    let output = run_update_env(&database, &["update-check", "--json"], &envs);
    server.join().expect("server thread finishes");
    assert_pretty_json(&output);
    let report: Value = serde_json::from_str(&output).expect("update-check --json is JSON");
    assert_eq!(report["checked"], true);
    assert_eq!(report["current_version"], env!("CARGO_PKG_VERSION"));
    // A prerelease build follows the prerelease train, which includes stable.
    assert_eq!(report["latest_version"], "99.0.0");
    assert_eq!(report["update_available"], true);
    assert_eq!(
        report["release_url"],
        "https://github.com/Arakiss/nahuali/releases/tag/v99.0.0"
    );

    envs.retain(|(name, _)| *name != "NAHUALI_NO_UPDATE_CHECK");
    envs.push(("NAHUALI_NO_UPDATE_CHECK", "1".to_string()));
    let disabled = run_update_env(&database, &["update-check"], &envs);
    assert!(
        disabled.contains("Update check disabled by NAHUALI_NO_UPDATE_CHECK"),
        "{disabled}"
    );
    let _ = fs::remove_dir_all(&home);
}
