pub const SERVER_NAME: &str = "nahuali";

pub(crate) const RESOURCE_SUMMARY: &str = "nahuali://database/summary";
pub(crate) const RESOURCE_SOURCES: &str = "nahuali://database/sources";
pub(crate) const RESOURCE_HEALTH: &str = "nahuali://database/health";
pub(crate) const RESOURCE_ENTITIES: &str = "nahuali://database/entities";
pub(crate) const RESOURCE_EPISODES: &str = "nahuali://database/episodes";
pub(crate) const RESOURCE_CLAIMS: &str = "nahuali://database/claims";
pub(crate) const RESOURCE_LINKS: &str = "nahuali://database/links";
pub(crate) const RESOURCE_FACTS: &str = "nahuali://database/facts";
pub(crate) const RESOURCE_RELATIONS: &str = "nahuali://database/relations";
pub(crate) const RESOURCE_PROCEDURES: &str = "nahuali://database/procedures";
pub(crate) const RESOURCE_INTENTIONS: &str = "nahuali://database/intentions";
pub(crate) const RESOURCE_EVENTS: &str = "nahuali://database/records";

pub(crate) const PROMPT_RECALL_WITH_HEALTH: &str = "recall_with_health_check";
pub(crate) const PROMPT_RECORD_EVIDENCE_BACKED_FACT: &str = "record_evidence_backed_fact";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpTool {
    pub name: &'static str,
    pub description: &'static str,
}

pub fn tool_catalog() -> Vec<McpTool> {
    vec![
        McpTool {
            name: "remember",
            description: "Store an episode as memory ground truth.",
        },
        McpTool {
            name: "ingest",
            description: "Ingest a provenance-aware source document.",
        },
        McpTool {
            name: "ingest_text",
            description: "Ingest direct text as provenance-preserving source episodes.",
        },
        McpTool {
            name: "claim",
            description: "Assert an evidence-linked claim.",
        },
        McpTool {
            name: "fact",
            description: "Assert an evidence-linked fact.",
        },
        McpTool {
            name: "link",
            description: "Record an evidence-linked typed connection.",
        },
        McpTool {
            name: "relate",
            description: "Record an evidence-linked relation.",
        },
        McpTool {
            name: "procedure",
            description: "Record an evidence-linked procedure.",
        },
        McpTool {
            name: "preference",
            description: "Record an evidence-linked behavioral preference.",
        },
        McpTool {
            name: "intention",
            description: "Record future work, a goal, reminder, or commitment.",
        },
        McpTool {
            name: "intention_status",
            description: "Change an intention lifecycle state.",
        },
        McpTool {
            name: "intention_update",
            description: "Update intention metadata without changing lifecycle state.",
        },
        McpTool {
            name: "reconcile_intentions",
            description: "Produce a non-mutating intention reconciliation report.",
        },
        McpTool {
            name: "goal_progress",
            description: "Produce a non-mutating goal progress report.",
        },
        McpTool {
            name: "proactive",
            description: "Produce a non-mutating proactive operator report.",
        },
        McpTool {
            name: "deadlines",
            description: "Produce non-mutating proactive deadline signals.",
        },
        McpTool {
            name: "anomalies",
            description: "Produce non-mutating proactive anomaly alerts.",
        },
        McpTool {
            name: "anomaly_acknowledge",
            description: "Acknowledge a proactive anomaly with an explicit audit note.",
        },
        McpTool {
            name: "briefing",
            description: "Produce a compact non-mutating session briefing.",
        },
        McpTool {
            name: "recall",
            description: "Retrieve memory with transparent scoring and evidence.",
        },
        McpTool {
            name: "graph",
            description: "Traverse the projected memory graph around a seed.",
        },
        McpTool {
            name: "inspect",
            description: "Inspect memory health before trusting recall.",
        },
        McpTool {
            name: "self_inspect",
            description: "Produce a non-mutating self-inspection consolidation report.",
        },
        McpTool {
            name: "reflect",
            description: "Plan a non-mutating reflection cycle for operator approval.",
        },
        McpTool {
            name: "consolidation_plan",
            description: "Plan non-mutating replay, review gates, and write-back eligibility.",
        },
        McpTool {
            name: "review",
            description: "Produce a prioritized non-mutating operator review queue.",
        },
        McpTool {
            name: "review_resolve",
            description: "Resolve an operator review item with an explicit audit note.",
        },
        McpTool {
            name: "projection_status",
            description: "Return derived SurrealDB graph-projection status.",
        },
        McpTool {
            name: "projection_rebuild",
            description: "Rebuild derived SurrealDB graph projection from the record ledger.",
        },
        McpTool {
            name: "projection_validate",
            description: "Validate derived SurrealDB graph projection against the record ledger.",
        },
        McpTool {
            name: "semantic_status",
            description: "Return Qdrant derived semantic-index status.",
        },
        McpTool {
            name: "semantic_rebuild",
            description: "Rebuild Qdrant semantic index from projected memory state.",
        },
        McpTool {
            name: "validate",
            description: "Validate the SurrealDB memory_record ledger.",
        },
    ]
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpResource {
    pub uri: &'static str,
    pub name: &'static str,
    pub title: &'static str,
    pub description: &'static str,
}

pub fn resource_catalog() -> Vec<McpResource> {
    vec![
        McpResource {
            uri: RESOURCE_SUMMARY,
            name: "summary",
            title: "Database Summary",
            description: "Projected counts and health summary rebuilt from the SurrealDB record ledger.",
        },
        McpResource {
            uri: RESOURCE_SOURCES,
            name: "sources",
            title: "Sources",
            description: "Projected source provenance records.",
        },
        McpResource {
            uri: RESOURCE_HEALTH,
            name: "health",
            title: "Knowledge Health",
            description: "Self-inspection signals, warnings, and evidence IDs.",
        },
        McpResource {
            uri: RESOURCE_ENTITIES,
            name: "entities",
            title: "Entities",
            description: "Projected entities discovered from mentions, claims, and links.",
        },
        McpResource {
            uri: RESOURCE_EPISODES,
            name: "episodes",
            title: "Episodes",
            description: "Observed ground-truth episodes projected from the SurrealDB record ledger.",
        },
        McpResource {
            uri: RESOURCE_CLAIMS,
            name: "claims",
            title: "Claims",
            description: "Projected claims with confidence and evidence links.",
        },
        McpResource {
            uri: RESOURCE_LINKS,
            name: "links",
            title: "Links",
            description: "Projected typed connections with confidence and evidence links.",
        },
        McpResource {
            uri: RESOURCE_FACTS,
            name: "facts",
            title: "Facts",
            description: "Projected facts with confidence and evidence links.",
        },
        McpResource {
            uri: RESOURCE_RELATIONS,
            name: "relations",
            title: "Relations",
            description: "Projected relation edges with confidence and evidence links.",
        },
        McpResource {
            uri: RESOURCE_PROCEDURES,
            name: "procedures",
            title: "Procedures",
            description: "Projected procedures and preferences with evidence links.",
        },
        McpResource {
            uri: RESOURCE_INTENTIONS,
            name: "intentions",
            title: "Intentions",
            description: "Projected intentions with lifecycle state.",
        },
        McpResource {
            uri: RESOURCE_EVENTS,
            name: "records",
            title: "Record Ledger",
            description: "SurrealDB memory_record envelopes used to rebuild the Rust projection.",
        },
    ]
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpPrompt {
    pub name: &'static str,
    pub title: &'static str,
    pub description: &'static str,
}

pub fn prompt_catalog() -> Vec<McpPrompt> {
    vec![
        McpPrompt {
            name: PROMPT_RECALL_WITH_HEALTH,
            title: "Recall With Health Check",
            description: "Recall memory while inspecting support, contradictions, and blind spots.",
        },
        McpPrompt {
            name: PROMPT_RECORD_EVIDENCE_BACKED_FACT,
            title: "Record Evidence-Backed Claim",
            description: "Capture an observation and derive a claim without losing evidence.",
        },
    ]
}
