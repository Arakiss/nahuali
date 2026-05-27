#[path = "operator_path_cases/support.rs"]
mod operator_path_support;

use std::fs;

use operator_path_support::{
    run, run_ok, run_ok_with_semantic_collection, run_with_semantic_collection,
    semantic_collection_name, temp_store,
};
use serde_json::Value;

include!("operator_path_cases/memory.rs");
include!("operator_path_cases/scopes.rs");
include!("operator_path_cases/ingestion.rs");
include!("operator_path_cases/migration.rs");
include!("operator_path_cases/review.rs");
include!("operator_path_cases/snapshots.rs");
