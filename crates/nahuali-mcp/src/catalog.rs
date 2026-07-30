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
            description: "Use when you observe something worth remembering (a decision, event, or fact stated by the user) and want it preserved verbatim as append-only evidence. Episodes are the evidence other memory cites, so record them before asserting any claim or link. Then derive claims/links with `sourceLast: true` to cite this episode.",
        },
        McpTool {
            name: "ingest",
            description: "Use when importing an external source document (the structured interchange format) and you need its provenance preserved as it becomes episodes. Set `dryRun: true` first to preflight scope, size, and evidence-gap counts without writing. Then re-run without `dryRun` to append the records.",
        },
        McpTool {
            name: "ingest_text",
            description: "Use when you have raw UTF-8 text (notes, a transcript, a web page) to capture as source episodes without building the full interchange document. It chunks and preserves provenance but does not infer claims or links. Set `dryRun: true` to preflight; then re-run to append, and derive claims/links separately if needed.",
        },
        McpTool {
            name: "claim",
            description: "Use when an episode explicitly supports a subject-predicate-object assertion you want recallable, after calling `remember` (pass `sourceLast: true` to cite it). A claim is the canonical evidence-backed assertion; `fact` is a deprecated alias of this tool. Then run `inspect` to confirm the claim is supported, not orphaned.",
        },
        McpTool {
            name: "fact",
            description: "Deprecated alias of `claim`; prefer that. Kept for backward compatibility while clients migrate to `claim`.",
        },
        McpTool {
            name: "link",
            description: "Use when an episode explicitly supports a typed connection between two entities (from-relation-to) you want recallable, after `remember` (pass `sourceLast: true` to cite it). `link` is canonical; `relate` is a deprecated alias of this tool. Then run `inspect` to confirm the link is evidence-backed.",
        },
        McpTool {
            name: "relate",
            description: "Deprecated alias of `link`; prefer that. Kept for backward compatibility while clients migrate to `link`.",
        },
        McpTool {
            name: "procedure",
            description: "Use when an episode establishes a repeatable how-to or rule (a procedure) you want recalled later, after `remember` (pass `sourceLast: true` to cite it). For a stated behavioral preference rather than a procedure, use `preference`. Then run `inspect` to confirm it is supported.",
        },
        McpTool {
            name: "preference",
            description: "Use when an episode states a durable behavioral preference or rule (a coding convention, a communication style, an operating default) you want recalled later, after `remember` (pass `sourceLast: true` to cite it). Distinct from `procedure`: a preference is a stated rule, a procedure is a repeatable how-to. Then run `inspect` to confirm it is supported.",
        },
        McpTool {
            name: "intention",
            description: "Use when the user commits to future work, a goal, or a reminder that must survive across sessions. Capture it here so `briefing`, `deadlines`, and `reconcile_intentions` can surface it later. Then use `intention_update` to add a deadline or goal link, and `intention_status` to close it out.",
        },
        McpTool {
            name: "intention_status",
            description: "Use when an intention's lifecycle changes (it is completed, abandoned, blocked, or deferred) and you want that recorded with a reason. Find the intention id via `briefing` or `recall` first. Then re-check `reconcile_intentions` if you need the updated commitment picture.",
        },
        McpTool {
            name: "intention_update",
            description: "Use when you need to set an intention's deadline, goal link, dependencies, or progress without changing its lifecycle status. Look up the intention id via `briefing` or `recall` first. Then `goal_progress` and `deadlines` will reflect the new metadata.",
        },
        McpTool {
            name: "reconcile_intentions",
            description: "Use when you need a read-only picture of which intentions are stale, overdue, or need attention before deciding what to act on. It does not change any intention. Then act with `intention_status` or `intention_update`.",
        },
        McpTool {
            name: "goal_progress",
            description: "Use when you want a read-only roll-up of progress toward goals and their linked tasks. It does not change anything. Then update progress with `intention_update` if the picture is out of date.",
        },
        McpTool {
            name: "proactive",
            description: "Use when you want a read-only operator report of everything that may need attention (deadlines, anomalies, and review signals together). It does not change anything. Then drill in with `deadlines` or `anomalies`, or act through `review` and the explicit write tools.",
        },
        McpTool {
            name: "deadlines",
            description: "Use when you want a read-only list of upcoming or overdue intention deadlines within a horizon. It does not change anything. Then act with `intention_update` or `intention_status` on the items that need it.",
        },
        McpTool {
            name: "anomalies",
            description: "Use when you want a read-only list of memory anomalies (contradictions, unsupported assertions, stale facts) that may warrant review. It does not change anything. Then record a decision on one with `anomaly_acknowledge`.",
        },
        McpTool {
            name: "anomaly_acknowledge",
            description: "Use when you have reviewed an anomaly from `anomalies` and want to record an explicit, auditable decision about it with a note. Set `dryRun: true` to preview the decision without writing it. Then re-run without `dryRun` to append the audit record.",
        },
        McpTool {
            name: "briefing",
            description: "Use this first at the start of a session, before any other work: it returns the compact read-only pre-work surface (authority, health, recent episodes, active intentions, high-priority review items, graph seeds). It does not change anything. Then `recall` specifics or act on the surfaced intentions and review items.",
        },
        McpTool {
            name: "memory_hook",
            description: "Use when a host wants the right governed context bundled for a host execution point: pass `kind` as `session_start`, `pre_prompt`, `post_action`, `session_close`, or `sleep_cycle`. It is read-only and packages authority, directives, recall, and (for close/sleep) reflection or self-inspection. Then follow the returned directives with explicit tool calls.",
        },
        McpTool {
            name: "recall",
            description: "Use this before acting on anything memory might already know: it retrieves matching memory with per-result trust, evidence IDs, a store-level authority decision, and a health report in one call. Read each result's trust and the authority/health before relying on it. Then cite evidence IDs, or state the gap if authority or health is `warn`/`block`.",
        },
        McpTool {
            name: "graph",
            description: "Use when flat `recall` is not enough and you need the neighborhood around an entity (nodes, edges, evidence IDs, authority, and health/review overlays) to a given depth. It is read-only. Then `recall` or `inspect` specific nodes the traversal surfaced.",
        },
        McpTool {
            name: "inspect",
            description: "Use when you need the database-wide health snapshot (supported vs unsupported facts, contradictions, stale facts, blind spots) to gauge whether memory can be trusted right now. It is read-only and does not propose fixes. Then run `self_inspect` for a structured findings report before proposing any repair.",
        },
        McpTool {
            name: "self_inspect",
            description: "Use before proposing any memory repair: it returns a read-only consolidation report with health, authority, findings, proposed review items, and an explicit `automatic_write_back=false` policy. It never writes. Then turn proposals into a queue with `review`, and apply only through `review_resolve`.",
        },
        McpTool {
            name: "reflect",
            description: "Use when you want self-inspection findings grouped into prioritized reflection cycles (with evidence IDs and coverage) for operator approval, rather than the raw findings. It is read-only and keeps `automatic_write_back=false`. Then move approved items through `review` and `review_resolve`.",
        },
        McpTool {
            name: "consolidation_plan",
            description: "Use when you want the full sleep/consolidation plan (replay, extraction, reconciliation, review-gate, and commit-eligibility steps) before any write-back. It is read-only and keeps `automatic_write_back=false`. Then act on the review gate via `review` and `review_resolve`; this tool never commits on its own.",
        },
        McpTool {
            name: "review",
            description: "Use when you want the prioritized, read-only operator review queue derived from self-inspection; narrow it with `action` to one proposed operator action. Treat it as guidance, not automatic write-back. Then apply a chosen item with `review_resolve`, which is the only path that writes a decision back.",
        },
        McpTool {
            name: "review_resolve",
            description: "Use this as the only write-back path for review items: after picking one from `review`, resolve it with its id and an explicit operator note. Set `dryRun: true` to preview the audit decision without writing. Then re-run without `dryRun` to append the decision to the ledger.",
        },
        McpTool {
            name: "projection_status",
            description: "Use when you need to know whether the derived SurrealDB graph projection is current with the record ledger. It is read-only. Then run `projection_validate` to confirm consistency, or `projection_rebuild` if it is stale.",
        },
        McpTool {
            name: "projection_rebuild",
            description: "Use when `projection_status` or `projection_validate` shows the derived SurrealDB graph projection is stale or inconsistent. It rebuilds the derived tier from the record ledger and does not change ground truth. Then run `projection_validate` to confirm it now matches.",
        },
        McpTool {
            name: "projection_validate",
            description: "Use when you want to confirm the derived SurrealDB graph projection still matches the record ledger and report any drift. It is read-only. Then run `projection_rebuild` if it reports the projection is out of sync.",
        },
        McpTool {
            name: "semantic_status",
            description: "Use when you need to know whether the derived Qdrant semantic index is present and current. It is read-only. Then run `semantic_rebuild` if it is missing or stale.",
        },
        McpTool {
            name: "semantic_rebuild",
            description: "Use when `semantic_status` shows the derived Qdrant semantic index is missing or stale. It rebuilds the index from projected memory and does not change ground truth. Then run `semantic_status` to confirm it is current.",
        },
        McpTool {
            name: "semantic_sync",
            description: "Use after writing memory to make recall reflect it without dropping the index: it upserts current points into the derived Qdrant semantic index without recreating the collection, so recall does not gap. Use `semantic_rebuild` instead after changing the embedder. Then run `semantic_status` to confirm.",
        },
        McpTool {
            name: "validate",
            description: "Use when you need to confirm the append-only SurrealDB memory_record ledger is intact and report record counts and any migration needs. It is read-only and checks the ground-truth ledger itself, not a derived tier. Then run `projection_validate`/`semantic_status` to check the derived tiers if the ledger is healthy.",
        },
        McpTool {
            name: "audit",
            description: "Use when you need a non-mutating diff of what the append-only memory_record ledger recorded between two points, with the integrity of that history restated alongside it. Bound the range with `from`/`to` (exclusive then inclusive sequence) and optional `since`/`until` (millisecond timestamps); omit all to audit the whole ledger. It reports per-kind counts, per-event entries, and whether the history through the upper bound verifies. Then run `validate` if integrity does not verify.",
        },
        McpTool {
            name: "trust_report",
            description: "Use when you need one composed, non-mutating report before relying on memory: knowledge counts, authority, available ledger checks, knowledge health, and a bounded verdict with reasons. Internal ledger checks can detect corruption and broken links; rollback or a fully re-chained history requires a retained, policy-authorized external checkpoint. Then act on the reasons, for example with `review` or by capturing missing evidence.",
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
