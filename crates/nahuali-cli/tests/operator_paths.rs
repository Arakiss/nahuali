#[path = "operator_path_cases/support.rs"]
mod operator_path_support;

use std::fs;

use operator_path_support::{
    QdrantCollectionGuard, run, run_at_endpoint, run_ok, run_ok_at_endpoint,
    run_ok_with_semantic_collection, run_with_semantic_collection, temp_database, temp_store,
};
use serde_json::Value;

#[test]
fn version_output_stays_stable_when_captured_by_agents_and_scripts() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_nahuali"))
        .arg("--version")
        .output()
        .expect("nahuali-cli runs");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).expect("version output is UTF-8"),
        format!("nahuali {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert!(
        output.stderr.is_empty(),
        "version output should not write to stderr"
    );
}

fn assert_pretty_json(output: &str) {
    assert!(
        output.starts_with("{\n  ") || output.starts_with("[\n  "),
        "JSON output should be pretty-printed, got: {output}"
    );
    assert!(
        output.ends_with('\n'),
        "JSON output should end with a newline, got: {output}"
    );
}

include!("operator_path_cases/audit.rs");
include!("operator_path_cases/init.rs");
#[cfg(feature = "attestation")]
include!("operator_path_cases/checkpoint.rs");
#[cfg(feature = "attestation")]
include!("operator_path_cases/receipt.rs");
include!("operator_path_cases/trust_report.rs");
include!("operator_path_cases/memory.rs");
include!("operator_path_cases/scopes.rs");
include!("operator_path_cases/ingestion.rs");
include!("operator_path_cases/migration.rs");
include!("operator_path_cases/review.rs");
include!("operator_path_cases/snapshots.rs");
