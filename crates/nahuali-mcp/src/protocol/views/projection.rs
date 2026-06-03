use std::collections::BTreeMap;

use nahuali_core::{
    GraphProjectionRebuildReport, GraphProjectionStatus, GraphProjectionValidation,
};
use rmcp::schemars;
use serde::Serialize;

/// Table counts and checkpoint state for the SurrealDB graph projection.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct GraphProjectionStatusView {
    projection_version: u32,
    ledger_event_count: usize,
    latest_sequence: Option<u64>,
    latest_event_id: Option<String>,
    checkpoint_sequence: Option<u64>,
    checkpoint_event_id: Option<String>,
    table_counts: BTreeMap<String, usize>,
    in_sync: bool,
}

impl From<GraphProjectionStatus> for GraphProjectionStatusView {
    fn from(status: GraphProjectionStatus) -> Self {
        Self {
            projection_version: status.projection_version,
            ledger_event_count: status.ledger_event_count,
            latest_sequence: status.latest_sequence,
            latest_event_id: status.latest_event_id,
            checkpoint_sequence: status.checkpoint_sequence,
            checkpoint_event_id: status.checkpoint_event_id,
            table_counts: status.table_counts,
            in_sync: status.in_sync,
        }
    }
}

/// Report returned after rebuilding the SurrealDB graph projection.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct GraphProjectionRebuildReportView {
    status: GraphProjectionStatusView,
    node_rows_written: usize,
    relation_rows_written: usize,
}

impl From<GraphProjectionRebuildReport> for GraphProjectionRebuildReportView {
    fn from(report: GraphProjectionRebuildReport) -> Self {
        Self {
            status: GraphProjectionStatusView::from(report.status),
            node_rows_written: report.node_rows_written,
            relation_rows_written: report.relation_rows_written,
        }
    }
}

/// Non-mutating graph projection validation result.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct GraphProjectionValidationView {
    status: GraphProjectionStatusView,
    valid: bool,
    issues: Vec<String>,
}

impl From<GraphProjectionValidation> for GraphProjectionValidationView {
    fn from(validation: GraphProjectionValidation) -> Self {
        Self {
            status: GraphProjectionStatusView::from(validation.status),
            valid: validation.valid,
            issues: validation.issues,
        }
    }
}
