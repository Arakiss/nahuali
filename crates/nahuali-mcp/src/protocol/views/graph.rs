use nahuali_core::{MemoryGraphEdge, MemoryGraphNode, MemoryGraphReport, MemoryGraphSummary};
use rmcp::schemars;
use serde::Serialize;

use super::{AuthorityDecisionView, json_string};

/// Aggregate counts for a graph neighborhood.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct GraphSummaryView {
    node_count: usize,
    edge_count: usize,
    entity_count: usize,
    memory_count: usize,
    support_edge_count: usize,
    relation_edge_count: usize,
    health_signal_count: usize,
    review_decision_count: usize,
}

impl From<MemoryGraphSummary> for GraphSummaryView {
    fn from(summary: MemoryGraphSummary) -> Self {
        Self {
            node_count: summary.node_count,
            edge_count: summary.edge_count,
            entity_count: summary.entity_count,
            memory_count: summary.memory_count,
            support_edge_count: summary.support_edge_count,
            relation_edge_count: summary.relation_edge_count,
            health_signal_count: summary.health_signal_count,
            review_decision_count: summary.review_decision_count,
        }
    }
}

/// A node in a graph neighborhood, with evidence and overlay counts.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct GraphNodeView {
    id: String,
    kind: String,
    label: String,
    depth: usize,
    evidence_ids: Vec<String>,
    source_event_ids: Vec<String>,
    health_signal_count: usize,
    review_decision_count: usize,
}

impl From<MemoryGraphNode> for GraphNodeView {
    fn from(node: MemoryGraphNode) -> Self {
        Self {
            id: node.id,
            kind: json_string(&node.kind),
            label: node.label,
            depth: node.depth,
            evidence_ids: node.evidence_ids,
            source_event_ids: node.source_event_ids,
            health_signal_count: node.health_signal_count,
            review_decision_count: node.review_decision_count,
        }
    }
}

/// An edge in a graph neighborhood.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct GraphEdgeView {
    id: String,
    from: String,
    to: String,
    kind: String,
    label: String,
    confidence: Option<f32>,
    evidence_id: Option<String>,
}

impl From<MemoryGraphEdge> for GraphEdgeView {
    fn from(edge: MemoryGraphEdge) -> Self {
        Self {
            id: edge.id,
            from: edge.from,
            to: edge.to,
            kind: json_string(&edge.kind),
            label: edge.label,
            confidence: edge.confidence,
            evidence_id: edge.evidence_id,
        }
    }
}

/// Structured graph-neighborhood report surfacing authority and evidence IDs.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct GraphReportView {
    version: u32,
    seed: String,
    max_depth: usize,
    limit: usize,
    event_count: usize,
    authority: AuthorityDecisionView,
    summary: GraphSummaryView,
    nodes: Vec<GraphNodeView>,
    edges: Vec<GraphEdgeView>,
}

impl From<MemoryGraphReport> for GraphReportView {
    fn from(report: MemoryGraphReport) -> Self {
        Self {
            version: report.version,
            seed: report.seed,
            max_depth: report.max_depth,
            limit: report.limit,
            event_count: report.event_count,
            authority: AuthorityDecisionView::from(report.authority),
            summary: GraphSummaryView::from(report.summary),
            nodes: report.nodes.into_iter().map(GraphNodeView::from).collect(),
            edges: report.edges.into_iter().map(GraphEdgeView::from).collect(),
        }
    }
}
