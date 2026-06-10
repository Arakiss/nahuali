use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use nahuali_core::SemanticConfig;

pub fn temp_store(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "nahuali-core-{name}-{}-{nanos}",
        std::process::id()
    ));
    let _ = fs::remove_file(&path);
    path
}

pub fn semantic_test_config(name: &str) -> SemanticConfig {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after epoch")
        .as_nanos();
    SemanticConfig::local_with_collection(format!(
        "nahuali_core_{name}_{}_{nanos}",
        std::process::id()
    ))
    .expect("semantic test collection name is valid")
}

/// Drops every Qdrant collection derived from this test's unique collection
/// name when the test ends, pass or fail. Leaked per-test collections degrade
/// the shared dev Qdrant until collection creation times out, so cleanup must
/// not depend on the test body running to completion.
pub struct QdrantCollectionGuard {
    name: String,
}

pub fn guard_for_collection(name: &str) -> QdrantCollectionGuard {
    QdrantCollectionGuard {
        name: name.to_string(),
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
