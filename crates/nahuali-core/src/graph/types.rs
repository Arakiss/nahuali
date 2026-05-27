/// Current memory graph report format version.
pub const MEMORY_GRAPH_VERSION: u32 = 1;

/// Options for traversing the projected memory graph.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct GraphTraversalOptions {
    /// Maximum number of graph hops from the seed node.
    pub max_depth: usize,
    /// Maximum number of nodes returned.
    pub limit: usize,
}

impl Default for GraphTraversalOptions {
    fn default() -> Self {
        Self {
            max_depth: 2,
            limit: 100,
        }
    }
}

/// Deterministic graph neighborhood over the current memory projection.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct MemoryGraphReport {
    /// Report format version.
    pub version: u32,
    /// Original seed string used for traversal.
    pub seed: String,
    /// Maximum graph depth requested.
    pub max_depth: usize,
    /// Maximum node count requested.
    pub limit: usize,
    /// Number of source events represented by the graph.
    pub event_count: usize,
    /// Projection-level authority decision.
    pub authority: AuthorityDecision,
    /// Aggregate graph counts.
    pub summary: MemoryGraphSummary,
    /// Nodes included in the traversed neighborhood.
    pub nodes: Vec<MemoryGraphNode>,
    /// Edges between included nodes.
    pub edges: Vec<MemoryGraphEdge>,
}

/// Aggregate counts for a graph neighborhood.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct MemoryGraphSummary {
    /// Number of returned nodes.
    pub node_count: usize,
    /// Number of returned edges.
    pub edge_count: usize,
    /// Entity nodes in the neighborhood.
    pub entity_count: usize,
    /// Memory item nodes in the neighborhood.
    pub memory_count: usize,
    /// Support/evidence edges in the neighborhood.
    pub support_edge_count: usize,
    /// Direct relation edges in the neighborhood.
    pub relation_edge_count: usize,
    /// Health signals attached to returned nodes.
    pub health_signal_count: usize,
    /// Review decisions attached to returned nodes.
    pub review_decision_count: usize,
}

/// Node in a memory graph neighborhood.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct MemoryGraphNode {
    /// Stable node identifier.
    pub id: String,
    /// Node kind.
    pub kind: MemoryGraphNodeKind,
    /// Human-readable node label.
    pub label: String,
    /// Hop distance from the matched seed.
    pub depth: usize,
    /// Evidence item identifiers attached to this node.
    pub evidence_ids: Vec<String>,
    /// Source event identifiers that created or mentioned this node.
    pub source_event_ids: Vec<String>,
    /// Number of health signals associated with this node.
    pub health_signal_count: usize,
    /// Number of review decisions associated with this node.
    pub review_decision_count: usize,
}

/// Memory graph node kind.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryGraphNodeKind {
    /// Named entity or concept.
    Entity,
    /// Observed source episode.
    Episode,
    /// Derived assertion.
    Claim,
    /// Typed connection.
    Link,
    /// Reusable procedure or preference.
    Procedure,
    /// Future work, goal, reminder, or commitment.
    Intention,
    /// Operator review decision.
    ReviewDecision,
}

/// Edge in a memory graph neighborhood.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct MemoryGraphEdge {
    /// Stable edge identifier.
    pub id: String,
    /// Source node identifier.
    pub from: String,
    /// Target node identifier.
    pub to: String,
    /// Edge kind.
    pub kind: MemoryGraphEdgeKind,
    /// Human-readable edge label.
    pub label: String,
    /// Confidence for derived claim/link edges, when available.
    pub confidence: Option<f32>,
    /// Evidence episode identifier, when the edge is evidence-backed.
    pub evidence_id: Option<String>,
}

/// Memory graph edge kind.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryGraphEdgeKind {
    /// An episode mentions an entity.
    Mentions,
    /// An episode supports a memory item.
    Supports,
    /// A claim connects its subject to the assertion node.
    ClaimSubject,
    /// A claim connects the assertion node to its object.
    ClaimObject,
    /// A link connects its source endpoint to the link node.
    LinkSource,
    /// A link connects the link node to its target endpoint.
    LinkTarget,
    /// Direct entity-to-entity relation projection.
    Relation,
    /// A review decision covers the target evidence node.
    Reviews,
}
