use std::fs;

use serde_json::{Value, json};

use super::support::{McpProcess, temp_store};

/// The frozen public tool contract: every tool name paired with the title of the
/// typed result view its `outputSchema` is derived from. Listed in sorted name
/// order so this table doubles as the readable tool-surface inventory.
///
/// `tools/list` advertises a precise `outputSchema` for each tool because every
/// handler returns `rmcp::Json<T>` over a `schemars::JsonSchema` view. This table
/// is the wire-level proof of that parity: if a handler stops returning its typed
/// result (or returns an opaque value), the title moves and this test fails.
const EXPECTED_TOOL_CONTRACT: &[(&str, &str)] = &[
    ("anomalies", "AnomaliesResult"),
    ("anomaly_acknowledge", "AnomalyAcknowledgeResult"),
    ("briefing", "BriefingResult"),
    ("claim", "ClaimResult"),
    ("consolidation_plan", "ConsolidationPlanResult"),
    ("deadlines", "DeadlinesResult"),
    ("fact", "FactResult"),
    ("goal_progress", "GoalProgressResult"),
    ("graph", "GraphResult"),
    ("ingest", "IngestResult"),
    ("ingest_text", "IngestTextResult"),
    ("inspect", "InspectResult"),
    ("intention", "IntentionResult"),
    ("intention_status", "IntentionResult"),
    ("intention_update", "IntentionResult"),
    ("link", "LinkResult"),
    ("memory_hook", "MemoryHookResult"),
    ("preference", "ProcedureResult"),
    ("proactive", "ProactiveResult"),
    ("procedure", "ProcedureResult"),
    ("projection_rebuild", "ProjectionReportResult"),
    ("projection_status", "ProjectionStatusResult"),
    ("projection_validate", "ProjectionValidationResult"),
    ("recall", "RecallToolResult"),
    ("reconcile_intentions", "ReconcileIntentionsResult"),
    ("reflect", "ReflectResult"),
    ("relate", "RelateResult"),
    ("remember", "RememberResult"),
    ("review", "ReviewResult"),
    ("review_resolve", "ReviewResolveResult"),
    ("self_inspect", "SelfInspectResult"),
    ("semantic_rebuild", "SemanticReportResult"),
    ("semantic_status", "SemanticStatusResult"),
    ("validate", "ValidateResult"),
];

pub fn run() {
    let store = temp_store("stdio-contract");
    let mut server = McpProcess::spawn(&store);

    initialize(&mut server);
    let tools = list_tools(&mut server);

    assert_frozen_tool_surface(&tools);
    assert_every_tool_publishes_typed_schemas(&tools);
    assert_key_output_shapes(&tools);
    assert_error_contract(&mut server);

    server.shutdown();
    let _ = fs::remove_file(store);
}

fn initialize(server: &mut McpProcess) {
    let initialized = server.request(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": {
                "name": "nahuali-contract-client",
                "version": "0.1.0"
            }
        }
    }));
    assert_eq!(initialized["result"]["serverInfo"]["name"], "nahuali");
    assert!(initialized["result"]["capabilities"]["tools"].is_object());

    server.notify(json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    }));
}

fn list_tools(server: &mut McpProcess) -> Vec<Value> {
    let listed = server.request(json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list"
    }));
    listed["result"]["tools"]
        .as_array()
        .expect("tools/list returns a tools array")
        .clone()
}

fn tool<'a>(tools: &'a [Value], name: &str) -> &'a Value {
    tools
        .iter()
        .find(|candidate| candidate["name"] == name)
        .unwrap_or_else(|| panic!("tool `{name}` is advertised"))
}

/// Freeze the advertised tool surface: the exact set of tool names and the typed
/// result view each one maps to. Mirrors the API's `http_contract` path freeze.
fn assert_frozen_tool_surface(tools: &[Value]) {
    let mut listed_names = tools
        .iter()
        .map(|candidate| {
            candidate["name"]
                .as_str()
                .expect("tool has a name")
                .to_string()
        })
        .collect::<Vec<_>>();
    listed_names.sort();

    let expected_names = EXPECTED_TOOL_CONTRACT
        .iter()
        .map(|(name, _)| (*name).to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        listed_names, expected_names,
        "the advertised tool name set must match the frozen contract"
    );

    for (name, title) in EXPECTED_TOOL_CONTRACT {
        let advertised = tool(tools, name);
        assert_eq!(
            advertised["outputSchema"]["title"], *title,
            "tool `{name}` must advertise the `{title}` typed output view"
        );
    }
}

/// Every tool must publish a typed input and output contract, not an opaque blob.
/// This is the wire-level guarantee that the schema-parity work reaches clients:
/// a handler that silently returns an untyped value would fail here.
fn assert_every_tool_publishes_typed_schemas(tools: &[Value]) {
    for advertised in tools {
        let name = advertised["name"].as_str().expect("tool has a name");

        assert!(
            advertised["description"]
                .as_str()
                .is_some_and(|description| !description.trim().is_empty()),
            "tool `{name}` must advertise a non-empty description"
        );

        let input_schema = &advertised["inputSchema"];
        assert_eq!(
            input_schema["type"], "object",
            "tool `{name}` must advertise an object inputSchema"
        );

        let output_schema = &advertised["outputSchema"];
        assert!(
            output_schema.is_object(),
            "tool `{name}` must advertise an outputSchema"
        );
        assert_eq!(
            output_schema["type"], "object",
            "tool `{name}` must advertise an object outputSchema"
        );
        assert!(
            output_schema["title"]
                .as_str()
                .is_some_and(|title| !title.is_empty()),
            "tool `{name}` outputSchema must be a titled typed view"
        );
        assert!(
            output_schema["properties"]
                .as_object()
                .is_some_and(|properties| !properties.is_empty()),
            "tool `{name}` outputSchema must declare at least one property"
        );
    }
}

/// Spot-check that nested typed views actually reach the wire by resolving
/// `$ref`/`anyOf`/array indirection in a representative slice of the surface.
/// These are the fields existing structured-content tests assert at runtime; here
/// we prove the *advertised schema* still describes them.
fn assert_key_output_shapes(tools: &[Value]) {
    // remember -> EpisodeView carries identity and evidence linkage.
    let remember = &tool(tools, "remember")["outputSchema"];
    let episode = resolve(remember, &remember["properties"]["episode"]);
    for field in ["id", "content", "event_id"] {
        assert!(
            has_property(episode, field),
            "remember episode view must declare `{field}`"
        );
    }

    // recall -> evidence-backed results with a trust view, plus store-level
    // authority and health context.
    let recall = &tool(tools, "recall")["outputSchema"];
    let result_item = resolve(recall, &recall["properties"]["results"]);
    for field in ["kind", "evidence_id", "trust"] {
        assert!(
            has_property(result_item, field),
            "recall result view must declare `{field}`"
        );
    }
    let trust = resolve(recall, &result_item["properties"]["trust"]);
    for field in ["mode", "can_trust"] {
        assert!(
            has_property(trust, field),
            "recall trust view must declare `{field}`"
        );
    }
    let authority = resolve(recall, &recall["properties"]["authority"]);
    assert!(
        has_property(authority, "mode"),
        "recall authority view must declare `mode`"
    );
    let health = resolve(recall, &recall["properties"]["health"]);
    assert!(
        health["properties"]
            .as_object()
            .is_some_and(|properties| !properties.is_empty()),
        "recall must expose a non-empty typed health view"
    );

    // briefing -> report view carries ledger size.
    let briefing = &tool(tools, "briefing")["outputSchema"];
    let report = resolve(briefing, &briefing["properties"]["report"]);
    assert!(
        has_property(report, "event_count"),
        "briefing report view must declare `event_count`"
    );

    // validate -> ledger verdict with inline scalars and a typed issue list.
    let validate = &tool(tools, "validate")["outputSchema"];
    for field in ["valid", "issues", "event_count"] {
        assert!(
            has_property(validate, field),
            "validate result must declare `{field}`"
        );
    }

    // ingest -> report resolves to a non-empty typed interchange view.
    let ingest = &tool(tools, "ingest")["outputSchema"];
    let ingest_report = resolve(ingest, &ingest["properties"]["report"]);
    assert!(
        ingest_report["properties"]
            .as_object()
            .is_some_and(|properties| !properties.is_empty()),
        "ingest report must resolve to a non-empty typed view"
    );
}

/// The error contract: two stable channels a client must tell apart. Parameters
/// that violate a tool's `inputSchema`, and unknown tool names, are rejected as
/// pre-dispatch JSON-RPC errors with no tool result. A handler-level domain
/// failure instead comes back as a tool result with `isError` set, a
/// human-readable message, and no structured content — so a caller branches on
/// the channel, not on parsing prose.
fn assert_error_contract(server: &mut McpProcess) {
    // Missing a required parameter is a pre-dispatch JSON-RPC invalid-params error.
    let missing_param = server.request(json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": { "name": "recall", "arguments": {} }
    }));
    assert!(
        missing_param["result"].is_null(),
        "schema-invalid params must not return a tool result"
    );
    assert_eq!(
        missing_param["error"]["code"], -32602,
        "a missing required parameter must surface as a JSON-RPC invalid-params error"
    );

    // An unknown tool name is likewise a JSON-RPC error, not a tool result.
    let unknown_tool = server.request(json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": { "name": "does_not_exist", "arguments": {} }
    }));
    assert!(
        unknown_tool["result"].is_null(),
        "an unknown tool must not return a tool result"
    );
    assert_eq!(
        unknown_tool["error"]["code"], -32602,
        "an unknown tool must surface as a JSON-RPC error"
    );

    // A handler-level domain failure is a tool result with isError, a message,
    // and no structured content.
    let domain_error = server.request(json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "tools/call",
        "params": {
            "name": "claim",
            "arguments": {
                "subject": "Lena",
                "predicate": "owns",
                "object": "the release notes",
                "sourceEpisodeId": "episode-x",
                "sourceLast": true
            }
        }
    }));
    assert!(
        domain_error.get("error").is_none(),
        "a domain failure must not be a JSON-RPC protocol error"
    );
    assert_eq!(
        domain_error["result"]["isError"], true,
        "a domain failure must set isError on the tool result"
    );
    assert!(
        domain_error["result"]["structuredContent"].is_null(),
        "a domain failure must not carry structured content"
    );
    assert!(
        domain_error["result"]["content"][0]["text"]
            .as_str()
            .is_some_and(|text| !text.trim().is_empty()),
        "a domain failure must carry a human-readable message"
    );
}

/// Follow `schemars` indirection to the concrete schema node: array `items`,
/// `$ref` into `$defs`, and the `$ref` arm of a nullable `anyOf`.
fn resolve<'a>(schema: &'a Value, node: &'a Value) -> &'a Value {
    if node["type"] == "array" {
        return resolve(schema, &node["items"]);
    }
    if let Some(reference) = node["$ref"].as_str() {
        let name = reference
            .rsplit('/')
            .next()
            .expect("$ref names a definition");
        return &schema["$defs"][name];
    }
    if let Some(any_of) = node["anyOf"].as_array()
        && let Some(member) = any_of.iter().find(|member| member.get("$ref").is_some())
    {
        return resolve(schema, member);
    }
    node
}

fn has_property(node: &Value, name: &str) -> bool {
    node["properties"].get(name).is_some()
}
