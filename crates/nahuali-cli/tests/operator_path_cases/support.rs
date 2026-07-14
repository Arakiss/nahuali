use std::{
    fs,
    ops::Deref,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

pub fn run_ok(store: &Path, args: &[&str]) -> String {
    let output = run(store, args);
    assert!(
        output.status.success(),
        "command failed\nargs: {args:?}\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).expect("stdout is UTF-8")
}

pub fn run_ok_with_semantic_collection(store: &Path, args: &[&str], collection: &str) -> String {
    let output = run_with_semantic_collection(store, args, collection);
    assert!(
        output.status.success(),
        "command failed\nargs: {args:?}\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).expect("stdout is UTF-8")
}

pub fn run(store: &Path, args: &[&str]) -> Output {
    run_at_endpoint(store, store, args)
}

pub fn run_at_endpoint(database: &Path, endpoint_store: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_nahuali"));
    command
        .arg("--database")
        .arg(database)
        .env("NAHUALI_DB_URL", test_endpoint(endpoint_store));
    command.args(args);
    command.output().expect("nahuali-cli runs")
}

pub fn run_ok_at_endpoint(database: &Path, endpoint_store: &Path, args: &[&str]) -> String {
    let output = run_at_endpoint(database, endpoint_store, args);
    assert!(
        output.status.success(),
        "command failed\nargs: {args:?}\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).expect("stdout is UTF-8")
}

pub fn run_with_semantic_collection(store: &Path, args: &[&str], collection: &str) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_nahuali"));
    command
        .arg("--database")
        .arg(store)
        .env("NAHUALI_DB_URL", test_endpoint(store))
        .env("NAHUALI_QDRANT_COLLECTION", collection);
    command.args(args);
    command.output().expect("nahuali-cli runs")
}

pub fn temp_store(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after epoch")
        .as_nanos();
    let path =
        std::env::temp_dir().join(format!("nahuali-cli-{name}-{}-{nanos}", std::process::id()));
    let _ = fs::remove_file(&path);
    path
}

/// A unique, clean SurrealDB database identifier for a test store. The CLI
/// refuses a path-like `--database` name, so database stores use this instead of
/// a temp-dir path (which `temp_store` still provides for artifact FILES like
/// snapshots, backups, and interchange documents).
pub struct TempDatabase {
    name: PathBuf,
    endpoint: PathBuf,
}

impl Deref for TempDatabase {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.name
    }
}

impl AsRef<Path> for TempDatabase {
    fn as_ref(&self) -> &Path {
        &self.name
    }
}

impl Drop for TempDatabase {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.endpoint);
    }
}

pub fn temp_database(name: &str) -> TempDatabase {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after epoch")
        .as_nanos();
    let sanitized: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect();
    let name = PathBuf::from(format!(
        "nahuali_cli_{sanitized}_{}_{nanos}",
        std::process::id()
    ));
    let endpoint = endpoint_path(&name);
    let _ = fs::remove_dir_all(&endpoint);
    TempDatabase { name, endpoint }
}

fn endpoint_path(store: &Path) -> PathBuf {
    std::env::temp_dir()
        .join("nahuali-cli-test-stores")
        .join(store.as_os_str())
}

fn test_endpoint(store: &Path) -> String {
    format!("surrealkv://{}", endpoint_path(store).display())
}

pub fn semantic_collection_name(name: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after epoch")
        .as_nanos();
    format!("nahuali_cli_{name}_{}_{nanos}", std::process::id())
}

/// Drops every Qdrant collection derived from this test's unique base name
/// when the test ends, pass or fail. Leaked per-test collections degrade the
/// shared dev Qdrant until collection creation times out (observed at 151
/// leaked collections), so cleanup must not depend on the test body running
/// to completion.
pub struct QdrantCollectionGuard {
    name: String,
}

impl QdrantCollectionGuard {
    pub fn new(name: &str) -> Self {
        Self {
            name: semantic_collection_name(name),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Drop for QdrantCollectionGuard {
    fn drop(&mut self) {
        // Best-effort: an unreachable Qdrant just leaves the collection for
        // the next run; never fail the test from cleanup.
        let base = std::env::var("NAHUALI_QDRANT_URL")
            .unwrap_or_else(|_| "http://localhost:16333".to_string());
        let base = base.trim_end_matches('/').to_string();
        let Ok(client) = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
        else {
            return;
        };
        let Ok(response) = client.get(format!("{base}/collections")).send() else {
            return;
        };
        let Ok(body) = response.json::<serde_json::Value>() else {
            return;
        };
        let Some(collections) = body
            .get("result")
            .and_then(|result| result.get("collections"))
            .and_then(|collections| collections.as_array())
        else {
            return;
        };
        for collection in collections {
            if let Some(name) = collection.get("name").and_then(|name| name.as_str())
                && name.starts_with(&self.name)
            {
                let _ = client.delete(format!("{base}/collections/{name}")).send();
            }
        }
    }
}
