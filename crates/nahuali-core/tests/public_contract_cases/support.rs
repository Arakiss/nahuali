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
