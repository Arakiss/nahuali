use std::fs;

use serde_json::{Value, json};

use super::{
    prompts, resources,
    support::{McpProcess, temp_store},
    workflow_ingest,
};

pub fn run() {
    let store = temp_store("stdio-workflow");
    let mut server = McpProcess::spawn(&store);

    initialize_server(&mut server);
    assert_tool_catalog(&mut server);
    workflow_ingest::assert_structured_ingest_dry_run(&mut server);
    workflow_ingest::assert_text_ingest_dry_run(&mut server);

    let ids = record_project_memory(&mut server);
    assert_briefing(&mut server, &ids);
    complete_intention(&mut server, &ids.intention_id);
    assert_recall_graph_and_health(&mut server, &ids.episode_id);
    assert_reflection_and_review(&mut server);
    assert_validation(&mut server);
    resources::assert_resources(&mut server, &ids.episode_id);
    prompts::assert_prompts(&mut server);

    server.shutdown();
    let _ = fs::remove_file(store);
}

struct WorkflowIds {
    episode_id: String,
    intention_id: String,
}

fn initialize_server(server: &mut McpProcess) {
    let initialized = server.request(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": {
                "name": "nahuali-test-client",
                "version": "0.1.0"
            }
        }
    }));
    assert_eq!(initialized["result"]["protocolVersion"], "2025-11-25");
    assert!(initialized["result"]["capabilities"]["tools"].is_object());
    assert!(initialized["result"]["capabilities"]["resources"].is_object());
    assert!(initialized["result"]["capabilities"]["prompts"].is_object());
    assert_eq!(initialized["result"]["serverInfo"]["name"], "nahuali");

    server.notify(json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    }));
}

fn assert_tool_catalog(server: &mut McpProcess) {
    let listed = server.request(json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list"
    }));
    let tools = listed["result"]["tools"]
        .as_array()
        .expect("tools/list returns tools");
    let recall_tool = tools
        .iter()
        .find(|tool| tool["name"] == "recall")
        .expect("recall tool is listed");
    let mut tool_names = tools
        .iter()
        .map(|tool| tool["name"].as_str().expect("tool has a name").to_string())
        .collect::<Vec<_>>();

    assert!(
        recall_tool["description"]
            .as_str()
            .unwrap_or_default()
            .contains("authority")
    );
    assert!(
        recall_tool["description"]
            .as_str()
            .unwrap_or_default()
            .contains("health")
    );
    assert!(
        recall_tool["description"]
            .as_str()
            .unwrap_or_default()
            .contains("result trust")
    );
    tool_names.sort();
    assert_eq!(
        tool_names,
        vec![
            "anomalies",
            "anomaly_acknowledge",
            "briefing",
            "claim",
            "consolidation_plan",
            "deadlines",
            "fact",
            "goal_progress",
            "graph",
            "ingest",
            "ingest_text",
            "inspect",
            "intention",
            "intention_status",
            "intention_update",
            "link",
            "memory_hook",
            "preference",
            "proactive",
            "procedure",
            "projection_rebuild",
            "projection_status",
            "projection_validate",
            "recall",
            "reconcile_intentions",
            "reflect",
            "relate",
            "remember",
            "review",
            "review_resolve",
            "self_inspect",
            "semantic_rebuild",
            "semantic_status",
            "validate"
        ]
    );
}

fn record_project_memory(server: &mut McpProcess) -> WorkflowIds {
    let remembered = server.request(json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "remember",
            "arguments": {
                "content": "Lena owns the release notes",
                "tags": ["product"],
                "mentions": ["Lena", "Release Notes"]
            }
        }
    }));
    let episode_id = remembered["result"]["structuredContent"]["episode"]["id"]
        .as_str()
        .expect("remember returns an episode id")
        .to_string();
    assert!(episode_id.starts_with("episode_"));
    assert_eq!(remembered["result"]["isError"], false);
    assert!(remembered["result"]["content"][0]["text"].is_string());

    assert_evidence_linked_fact(server, &episode_id);
    assert_evidence_linked_relation(server, &episode_id);
    assert_evidence_linked_claim(server, &episode_id);
    assert_evidence_linked_link(server, &episode_id);
    assert_procedure_and_preference(server);

    let intention = server.request(json!({
        "jsonrpc": "2.0",
        "id": 25,
        "method": "tools/call",
        "params": {
            "name": "intention",
            "arguments": {
                "description": "Ship release notes",
                "kind": "task",
                "priority": "high",
                "sourceLast": true
            }
        }
    }));
    let intention_id = intention["result"]["structuredContent"]["intention"]["id"]
        .as_str()
        .expect("intention returns an id")
        .to_string();
    assert_eq!(
        intention["result"]["structuredContent"]["intention"]["status"],
        "active"
    );

    WorkflowIds {
        episode_id,
        intention_id,
    }
}

fn assert_evidence_linked_fact(server: &mut McpProcess, episode_id: &str) {
    let fact = server.request(json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {
            "name": "fact",
            "arguments": {
                "subject": "Lena",
                "predicate": "owns",
                "object": "release notes",
                "confidence": 0.92,
                "sourceLast": true
            }
        }
    }));
    assert_eq!(
        fact["result"]["structuredContent"]["fact"]["source_episode_id"],
        episode_id
    );
}

fn assert_evidence_linked_relation(server: &mut McpProcess, episode_id: &str) {
    let relation = server.request(json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "tools/call",
        "params": {
            "name": "relate",
            "arguments": {
                "from": "Lena",
                "relation": "owns",
                "to": "release notes",
                "confidence": 0.9,
                "sourceLast": true
            }
        }
    }));
    assert_eq!(
        relation["result"]["structuredContent"]["relation"]["source_episode_id"],
        episode_id
    );
}

fn assert_evidence_linked_claim(server: &mut McpProcess, episode_id: &str) {
    let claim = server.request(json!({
        "jsonrpc": "2.0",
        "id": 21,
        "method": "tools/call",
        "params": {
            "name": "claim",
            "arguments": {
                "subject": "Lena",
                "predicate": "prefers",
                "object": "concise release notes",
                "confidence": 0.93,
                "sourceLast": true
            }
        }
    }));
    assert_eq!(
        claim["result"]["structuredContent"]["claim"]["source_episode_id"],
        episode_id
    );
    assert!(
        claim["result"]["structuredContent"]["claim"]["id"]
            .as_str()
            .unwrap_or_default()
            .starts_with("claim_")
    );
}

fn assert_evidence_linked_link(server: &mut McpProcess, episode_id: &str) {
    let link = server.request(json!({
        "jsonrpc": "2.0",
        "id": 22,
        "method": "tools/call",
        "params": {
            "name": "link",
            "arguments": {
                "from": "Lena",
                "relation": "prefers",
                "to": "Release Notes",
                "confidence": 0.91,
                "sourceLast": true
            }
        }
    }));
    assert_eq!(
        link["result"]["structuredContent"]["link"]["source_episode_id"],
        episode_id
    );
    assert!(
        link["result"]["structuredContent"]["link"]["id"]
            .as_str()
            .unwrap_or_default()
            .starts_with("link_")
    );
}

fn assert_procedure_and_preference(server: &mut McpProcess) {
    let procedure = server.request(json!({
        "jsonrpc": "2.0",
        "id": 23,
        "method": "tools/call",
        "params": {
            "name": "procedure",
            "arguments": {
                "name": "Release notes",
                "body": "Keep release notes concise",
                "confidence": 0.8,
                "sourceLast": true
            }
        }
    }));
    assert_eq!(
        procedure["result"]["structuredContent"]["procedure"]["kind"],
        "procedure"
    );

    let preference = server.request(json!({
        "jsonrpc": "2.0",
        "id": 24,
        "method": "tools/call",
        "params": {
            "name": "preference",
            "arguments": {
                "name": "Communication style",
                "body": "Prefer concise release notes",
                "confidence": 0.9,
                "sourceLast": true
            }
        }
    }));
    assert_eq!(
        preference["result"]["structuredContent"]["procedure"]["kind"],
        "preference"
    );
}

fn assert_briefing(server: &mut McpProcess, ids: &WorkflowIds) {
    let briefing = server.request(json!({
        "jsonrpc": "2.0",
        "id": 36,
        "method": "tools/call",
        "params": {
            "name": "briefing",
            "arguments": {
                "episodeLimit": 1,
                "intentionLimit": 2,
                "reviewLimit": 3,
                "graphSeedLimit": 2
            }
        }
    }));
    assert_eq!(
        briefing["result"]["structuredContent"]["report"]["event_count"],
        8
    );
    assert_eq!(
        briefing["result"]["structuredContent"]["report"]["summary"]["active_intention_count"],
        1
    );
    assert_eq!(
        briefing["result"]["structuredContent"]["report"]["recent_episodes"][0]["id"],
        ids.episode_id
    );
    assert_eq!(
        briefing["result"]["structuredContent"]["report"]["active_intentions"][0]["description"],
        "Ship release notes"
    );
    assert!(
        briefing["result"]["structuredContent"]["report"]["graph_seeds"]
            .as_array()
            .expect("briefing returns graph seeds")
            .iter()
            .any(|seed| seed["label"] == "Lena")
    );
}

fn complete_intention(server: &mut McpProcess, intention_id: &str) {
    let intention_status = server.request(json!({
        "jsonrpc": "2.0",
        "id": 26,
        "method": "tools/call",
        "params": {
            "name": "intention_status",
            "arguments": {
                "id": intention_id,
                "status": "completed",
                "reason": "Done"
            }
        }
    }));
    assert_eq!(
        intention_status["result"]["structuredContent"]["intention"]["status"],
        "completed"
    );
}

fn assert_recall_graph_and_health(server: &mut McpProcess, episode_id: &str) {
    let recalled = server.request(json!({
        "jsonrpc": "2.0",
        "id": 6,
        "method": "tools/call",
        "params": {
            "name": "recall",
            "arguments": {
                "query": "Lena release",
                "limit": 10
            }
        }
    }));
    assert_eq!(
        recalled["result"]["structuredContent"]["results"][0]["kind"],
        "claim"
    );
    assert_eq!(
        recalled["result"]["structuredContent"]["results"][0]["evidence_id"],
        episode_id
    );
    assert_eq!(
        recalled["result"]["structuredContent"]["results"][0]["trust"]["mode"],
        "certify"
    );
    assert_eq!(
        recalled["result"]["structuredContent"]["results"][0]["trust"]["can_trust"],
        true
    );
    assert_eq!(
        recalled["result"]["structuredContent"]["authority"]["mode"],
        "advisory"
    );
    assert_eq!(
        recalled["result"]["structuredContent"]["authority"]["score"],
        0.75
    );
    assert_eq!(
        recalled["result"]["structuredContent"]["authority"]["can_trust"],
        false
    );
    assert_eq!(
        recalled["result"]["structuredContent"]["authority"]["signal_kinds"],
        json!(["isolated_entity"])
    );
    assert_eq!(
        recalled["result"]["structuredContent"]["health"]["isolated_entity_count"],
        1
    );
    assert_health_signal(&recalled);
    assert_graph(server);
    assert_inspection(server);
}

fn assert_health_signal(recalled: &Value) {
    assert_eq!(
        recalled["result"]["structuredContent"]["health"]["signals"][0]["kind"],
        "isolated_entity"
    );
    assert_eq!(
        recalled["result"]["structuredContent"]["health"]["signals"][0]["severity"],
        "low"
    );
    assert_eq!(
        recalled["result"]["structuredContent"]["health"]["signals"][0]["dimensions"],
        json!(["connectivity", "blind_spot"])
    );
}

fn assert_graph(server: &mut McpProcess) {
    let graphed = server.request(json!({
        "jsonrpc": "2.0",
        "id": 34,
        "method": "tools/call",
        "params": {
            "name": "graph",
            "arguments": {
                "seed": "Lena",
                "depth": 2,
                "limit": 20
            }
        }
    }));
    assert_eq!(
        graphed["result"]["structuredContent"]["report"]["seed"],
        "Lena"
    );
    assert!(
        graphed["result"]["structuredContent"]["report"]["summary"]["node_count"]
            .as_u64()
            .unwrap_or_default()
            >= 4
    );
    assert!(
        graphed["result"]["structuredContent"]["report"]["edges"]
            .as_array()
            .unwrap()
            .iter()
            .any(|edge| edge["kind"] == "relation")
    );
}

fn assert_inspection(server: &mut McpProcess) {
    let inspected = server.request(json!({
        "jsonrpc": "2.0",
        "id": 7,
        "method": "tools/call",
        "params": {
            "name": "inspect",
            "arguments": {}
        }
    }));
    assert_eq!(
        inspected["result"]["structuredContent"]["health"]["supported_fact_count"],
        2
    );
    assert_eq!(
        inspected["result"]["structuredContent"]["health"]["unsupported_fact_count"],
        0
    );
}

fn assert_reflection_and_review(server: &mut McpProcess) {
    assert_self_inspection(server);
    assert_reflection(server);
    resolve_first_review_item(server);
}

fn assert_self_inspection(server: &mut McpProcess) {
    let self_inspected = server.request(json!({
        "jsonrpc": "2.0",
        "id": 27,
        "method": "tools/call",
        "params": {
            "name": "self_inspect",
            "arguments": {}
        }
    }));
    assert_eq!(
        self_inspected["result"]["structuredContent"]["report"]["write_back_policy"]["automatic_write_back"],
        false
    );
    assert_eq!(
        self_inspected["result"]["structuredContent"]["report"]["summary"]["blind_spot_count"],
        1
    );
    assert_eq!(
        self_inspected["result"]["structuredContent"]["report"]["summary"]["source_coverage_count"],
        1
    );
    assert_eq!(
        self_inspected["result"]["structuredContent"]["report"]["review_queue"][0]["status"],
        "proposed"
    );
}

fn assert_reflection(server: &mut McpProcess) {
    let reflected = server.request(json!({
        "jsonrpc": "2.0",
        "id": 38,
        "method": "tools/call",
        "params": {
            "name": "reflect",
            "arguments": {
                "cycleLimit": 5,
                "evidenceLimit": 5
            }
        }
    }));
    assert_eq!(
        reflected["result"]["structuredContent"]["report"]["write_back_policy"]["automatic_write_back"],
        false
    );
    assert_eq!(
        reflected["result"]["structuredContent"]["report"]["source_coverage"]["evidence_backed_memory_count"],
        7
    );
    let cycles = reflected["result"]["structuredContent"]["report"]["cycles"]
        .as_array()
        .expect("reflection returns cycles");
    assert!(cycles.iter().any(|cycle| cycle["action"] == "link_memory"));
    assert!(
        cycles
            .iter()
            .any(|cycle| cycle["action"] == "capture_evidence")
    );
}

fn resolve_first_review_item(server: &mut McpProcess) {
    let reviewed = server.request(json!({
        "jsonrpc": "2.0",
        "id": 32,
        "method": "tools/call",
        "params": {
            "name": "review",
            "arguments": {
                "limit": 5,
                "minPriority": "low"
            }
        }
    }));
    assert_eq!(
        reviewed["result"]["structuredContent"]["report"]["write_back_policy"]["automatic_write_back"],
        false
    );
    assert_eq!(
        reviewed["result"]["structuredContent"]["report"]["items"][0]["status"],
        "proposed"
    );
    assert_review_guidance(&reviewed);
    let review_action = reviewed["result"]["structuredContent"]["report"]["items"][0]["action"]
        .as_str()
        .expect("review item has action")
        .to_string();
    let action_filtered = server.request(json!({
        "jsonrpc": "2.0",
        "id": 321,
        "method": "tools/call",
        "params": {
            "name": "review",
            "arguments": {
                "limit": 5,
                "action": review_action.clone()
            }
        }
    }));
    let filtered_items = action_filtered["result"]["structuredContent"]["report"]["items"]
        .as_array()
        .expect("action-filtered review returns items");
    assert!(!filtered_items.is_empty());
    assert!(
        filtered_items
            .iter()
            .all(|item| item["action"] == review_action)
    );
    let review_id = reviewed["result"]["structuredContent"]["report"]["items"][0]["id"]
        .as_str()
        .expect("review item has id")
        .to_string();

    let review_resolved = server.request(json!({
        "jsonrpc": "2.0",
        "id": 33,
        "method": "tools/call",
        "params": {
            "name": "review_resolve",
            "arguments": {
                "reviewId": review_id,
                "note": "Operator reviewed this queue item in the MCP workflow."
            }
        }
    }));
    assert_eq!(
        review_resolved["result"]["structuredContent"]["report"]["applied"],
        true
    );
    assert_eq!(
        review_resolved["result"]["structuredContent"]["report"]["dry_run"],
        false
    );
    assert!(
        review_resolved["result"]["structuredContent"]["report"]["event_id"]
            .as_str()
            .unwrap_or_default()
            .starts_with("event_")
    );
}

fn assert_review_guidance(reviewed: &Value) {
    let guidance =
        reviewed["result"]["structuredContent"]["report"]["items"][0]["operator_guidance"]
            .as_str()
            .unwrap_or_default();
    assert!(guidance.contains("evidence-backed") || guidance.contains("Record"));
}

fn assert_validation(server: &mut McpProcess) {
    let validated = server.request(json!({
        "jsonrpc": "2.0",
        "id": 8,
        "method": "tools/call",
        "params": {
            "name": "validate",
            "arguments": {}
        }
    }));
    assert_eq!(validated["result"]["structuredContent"]["valid"], true);
    assert_eq!(validated["result"]["structuredContent"]["event_count"], 10);
    assert_eq!(validated["result"]["structuredContent"]["source_count"], 0);
    assert_eq!(
        validated["result"]["structuredContent"]["supported_event_version"],
        1
    );
    assert_eq!(
        validated["result"]["structuredContent"]["observed_event_versions"],
        json!([1])
    );
    assert_eq!(
        validated["result"]["structuredContent"]["legacy_event_count"],
        0
    );
    assert_eq!(
        validated["result"]["structuredContent"]["migration_required"],
        false
    );
    assert_eq!(
        validated["result"]["structuredContent"]["issues"],
        json!([])
    );
    assert_eq!(validated["result"]["structuredContent"]["entity_count"], 3);
    assert_eq!(validated["result"]["structuredContent"]["claim_count"], 2);
    assert_eq!(validated["result"]["structuredContent"]["link_count"], 2);
    assert_eq!(
        validated["result"]["structuredContent"]["procedure_count"],
        2
    );
    assert_eq!(
        validated["result"]["structuredContent"]["intention_count"],
        1
    );
    assert_eq!(
        validated["result"]["structuredContent"]["review_decision_count"],
        1
    );
}
