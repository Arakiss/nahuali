#[path = "operator_path_cases/support.rs"]
mod operator_path_support;

use std::fs;

use operator_path_support::{
    QdrantCollectionGuard, run, run_ok, run_ok_at_endpoint, run_ok_with_semantic_collection,
    run_with_semantic_collection, temp_database, temp_store,
};
use serde_json::Value;

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
include!("operator_path_cases/trust_report.rs");
include!("operator_path_cases/memory.rs");
include!("operator_path_cases/scopes.rs");
include!("operator_path_cases/ingestion.rs");
include!("operator_path_cases/migration.rs");
include!("operator_path_cases/review.rs");
include!("operator_path_cases/snapshots.rs");
