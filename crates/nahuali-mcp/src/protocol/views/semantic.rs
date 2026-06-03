use nahuali_core::{
    EmbeddingProviderConfig, SemanticIndexReport, SemanticIndexStatus, SemanticPointSummary,
};
use rmcp::schemars;
use serde::Serialize;

use super::json_string;

/// Embedding provider configuration used by the semantic tier.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct EmbeddingProviderConfigView {
    kind: String,
    model: String,
    dimensions: usize,
}

impl From<EmbeddingProviderConfig> for EmbeddingProviderConfigView {
    fn from(config: EmbeddingProviderConfig) -> Self {
        Self {
            kind: json_string(&config.kind),
            model: config.model,
            dimensions: config.dimensions,
        }
    }
}

/// Compact metadata for one indexed semantic point.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct SemanticPointSummaryView {
    point_id: u64,
    kind: String,
    id: String,
    event_id: String,
    event_ids: Vec<String>,
    surreal_table: String,
    surreal_id: String,
    scope_key: Option<String>,
    entity_names: Vec<String>,
    source_ids: Vec<String>,
    text: String,
}

impl From<SemanticPointSummary> for SemanticPointSummaryView {
    fn from(point: SemanticPointSummary) -> Self {
        Self {
            kind: json_string(&point.kind),
            point_id: point.point_id,
            id: point.id,
            event_id: point.event_id,
            event_ids: point.event_ids,
            surreal_table: point.surreal_table,
            surreal_id: point.surreal_id,
            scope_key: point.scope_key,
            entity_names: point.entity_names,
            source_ids: point.source_ids,
            text: point.text,
        }
    }
}

/// Current Qdrant semantic index status.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct SemanticIndexStatusView {
    collection_name: String,
    qdrant_url: String,
    collection_exists: bool,
    point_count: usize,
}

impl From<SemanticIndexStatus> for SemanticIndexStatusView {
    fn from(status: SemanticIndexStatus) -> Self {
        Self {
            collection_name: status.collection_name,
            qdrant_url: status.qdrant_url,
            collection_exists: status.collection_exists,
            point_count: status.point_count,
        }
    }
}

/// Result of rebuilding a Qdrant semantic index from the current projection.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct SemanticIndexReportView {
    collection_name: String,
    qdrant_url: String,
    embedding: EmbeddingProviderConfigView,
    source_event_count: usize,
    indexed_point_count: usize,
    deleted_existing_collection: bool,
    points: Vec<SemanticPointSummaryView>,
}

impl From<SemanticIndexReport> for SemanticIndexReportView {
    fn from(report: SemanticIndexReport) -> Self {
        Self {
            collection_name: report.collection_name,
            qdrant_url: report.qdrant_url,
            embedding: EmbeddingProviderConfigView::from(report.embedding),
            source_event_count: report.source_event_count,
            indexed_point_count: report.indexed_point_count,
            deleted_existing_collection: report.deleted_existing_collection,
            points: report
                .points
                .into_iter()
                .map(SemanticPointSummaryView::from)
                .collect(),
        }
    }
}
