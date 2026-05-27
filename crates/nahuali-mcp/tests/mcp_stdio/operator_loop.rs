use std::fs;

use serde_json::{Value, json};

use super::support::{McpProcess, temp_store};

pub fn run() {
    let store = temp_store("operator-loop");
    let mut server = McpProcess::spawn(&store);

    initialize_server(&mut server);
    let ids = record_goal_and_task(&mut server);
    update_task_metadata(&mut server, &ids);
    assert_goal_progress(&mut server, &ids);
    assert_reconciliation_and_proactive_reports(&mut server, &ids);
    assert_projection_tools(&mut server);

    server.shutdown();
    let _ = fs::remove_file(store);
}

struct OperatorIds {
    goal_id: String,
    task_id: String,
}

fn initialize_server(server: &mut McpProcess) {
    let initialized = server.request(json!({
        "jsonrpc": "2.0",
        "id": 100,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": {
                "name": "nahuali-operator-loop-test-client",
                "version": "0.1.0"
            }
        }
    }));
    assert_eq!(initialized["result"]["protocolVersion"], "2025-11-25");

    server.notify(json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    }));
}

fn record_goal_and_task(server: &mut McpProcess) -> OperatorIds {
    server.request(json!({
        "jsonrpc": "2.0",
        "id": 101,
        "method": "tools/call",
        "params": {
            "name": "remember",
            "arguments": {
                "content": "The public beta needs operator-loop parity.",
                "tags": ["beta"],
                "mentions": ["Nahuali"]
            }
        }
    }));

    let goal = server.request(json!({
        "jsonrpc": "2.0",
        "id": 102,
        "method": "tools/call",
        "params": {
            "name": "intention",
            "arguments": {
                "description": "Ship the public beta",
                "kind": "goal",
                "priority": "critical",
                "sourceLast": true
            }
        }
    }));
    let goal_id = goal["result"]["structuredContent"]["intention"]["id"]
        .as_str()
        .expect("goal intention has an id")
        .to_string();

    let task = server.request(json!({
        "jsonrpc": "2.0",
        "id": 103,
        "method": "tools/call",
        "params": {
            "name": "intention",
            "arguments": {
                "description": "Expose proactive operator loops in MCP",
                "kind": "task",
                "priority": "high",
                "sourceLast": true
            }
        }
    }));
    let task_id = task["result"]["structuredContent"]["intention"]["id"]
        .as_str()
        .expect("task intention has an id")
        .to_string();

    OperatorIds { goal_id, task_id }
}

fn update_task_metadata(server: &mut McpProcess, ids: &OperatorIds) {
    let updated = server.request(json!({
        "jsonrpc": "2.0",
        "id": 104,
        "method": "tools/call",
        "params": {
            "name": "intention_update",
            "arguments": {
                "id": ids.task_id,
                "deadlineAtMs": 50,
                "goalId": ids.goal_id,
                "progressPercent": 25
            }
        }
    }));

    assert_eq!(
        updated["result"]["structuredContent"]["intention"]["id"],
        ids.task_id
    );
    assert_eq!(
        updated["result"]["structuredContent"]["intention"]["deadline_at_ms"],
        50
    );
    assert_eq!(
        updated["result"]["structuredContent"]["intention"]["goal_id"],
        ids.goal_id
    );
    assert_eq!(
        updated["result"]["structuredContent"]["intention"]["progress_percent"],
        25
    );
}

fn assert_goal_progress(server: &mut McpProcess, ids: &OperatorIds) {
    let progress = server.request(json!({
        "jsonrpc": "2.0",
        "id": 105,
        "method": "tools/call",
        "params": {
            "name": "goal_progress",
            "arguments": {}
        }
    }));
    assert_eq!(
        progress["result"]["structuredContent"]["report"]["goal_count"],
        1
    );
    assert_eq!(
        progress["result"]["structuredContent"]["report"]["goals"][0]["goal_id"],
        ids.goal_id
    );
    assert_eq!(
        progress["result"]["structuredContent"]["report"]["goals"][0]["child_count"],
        1
    );
}

fn assert_reconciliation_and_proactive_reports(server: &mut McpProcess, ids: &OperatorIds) {
    let reconciliation = server.request(json!({
        "jsonrpc": "2.0",
        "id": 106,
        "method": "tools/call",
        "params": {
            "name": "reconcile_intentions",
            "arguments": {
                "nowMs": 100,
                "staleAfterMs": 0
            }
        }
    }));
    assert!(
        reconciliation["result"]["structuredContent"]["report"]["issues"]
            .as_array()
            .expect("reconciliation returns issues")
            .iter()
            .any(|issue| issue["intention_id"] == ids.task_id && issue["kind"] == "overdue")
    );

    let deadlines = server.request(json!({
        "jsonrpc": "2.0",
        "id": 107,
        "method": "tools/call",
        "params": {
            "name": "deadlines",
            "arguments": {
                "nowMs": 100,
                "deadlineHorizonMs": 1000,
                "staleAfterMs": 0
            }
        }
    }));
    assert_eq!(
        deadlines["result"]["structuredContent"]["source_projection"],
        "rust"
    );
    assert_eq!(
        deadlines["result"]["structuredContent"]["report"]["summary"]["overdue_count"],
        1
    );

    let proactive = server.request(json!({
        "jsonrpc": "2.0",
        "id": 108,
        "method": "tools/call",
        "params": {
            "name": "proactive",
            "arguments": {
                "nowMs": 100,
                "deadlineHorizonMs": 1000,
                "staleAfterMs": 0,
                "reviewLimit": 10
            }
        }
    }));
    assert_eq!(
        proactive["result"]["structuredContent"]["report"]["summary"]["overdue_deadline_count"],
        1
    );
    assert_eq!(
        proactive["result"]["structuredContent"]["report"]["write_back_policy"]["automatic_write_back"],
        false
    );

    let anomalies = server.request(json!({
        "jsonrpc": "2.0",
        "id": 109,
        "method": "tools/call",
        "params": {
            "name": "anomalies",
            "arguments": {
                "nowMs": 100,
                "deadlineHorizonMs": 1000,
                "staleAfterMs": 0
            }
        }
    }));
    let alert_id = find_overdue_alert_id(&anomalies);

    let dry_run_ack = acknowledge_anomaly(server, 110, &alert_id, true);
    assert_eq!(
        dry_run_ack["result"]["structuredContent"]["report"]["dry_run"],
        true
    );
    assert_eq!(
        dry_run_ack["result"]["structuredContent"]["report"]["applied"],
        false
    );

    let applied_ack = acknowledge_anomaly(server, 111, &alert_id, false);
    assert_eq!(
        applied_ack["result"]["structuredContent"]["report"]["dry_run"],
        false
    );
    assert_eq!(
        applied_ack["result"]["structuredContent"]["report"]["applied"],
        true
    );
    assert!(
        applied_ack["result"]["structuredContent"]["report"]["event_id"]
            .as_str()
            .unwrap_or_default()
            .starts_with("event_")
    );
}

fn find_overdue_alert_id(anomalies: &Value) -> String {
    anomalies["result"]["structuredContent"]["report"]["alerts"]
        .as_array()
        .expect("anomalies returns alerts")
        .iter()
        .find(|alert| alert["kind"] == "overdue_deadline")
        .and_then(|alert| alert["id"].as_str())
        .expect("anomalies include overdue deadline alert")
        .to_string()
}

fn acknowledge_anomaly(server: &mut McpProcess, id: u64, alert_id: &str, dry_run: bool) -> Value {
    server.request(json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": {
            "name": "anomaly_acknowledge",
            "arguments": {
                "anomalyId": alert_id,
                "note": "Operator reviewed this alert in the MCP operator loop.",
                "dryRun": dry_run
            }
        }
    }))
}

fn assert_projection_tools(server: &mut McpProcess) {
    let status = server.request(json!({
        "jsonrpc": "2.0",
        "id": 112,
        "method": "tools/call",
        "params": {
            "name": "projection_status",
            "arguments": {}
        }
    }));
    assert_eq!(
        status["result"]["structuredContent"]["projection_role"],
        "derived_from_memory_record"
    );
    assert_eq!(
        status["result"]["structuredContent"]["status"]["in_sync"],
        true
    );

    let rebuild = server.request(json!({
        "jsonrpc": "2.0",
        "id": 113,
        "method": "tools/call",
        "params": {
            "name": "projection_rebuild",
            "arguments": {}
        }
    }));
    assert_eq!(
        rebuild["result"]["structuredContent"]["report"]["status"]["in_sync"],
        true
    );

    let validation = server.request(json!({
        "jsonrpc": "2.0",
        "id": 114,
        "method": "tools/call",
        "params": {
            "name": "projection_validate",
            "arguments": {}
        }
    }));
    assert_eq!(
        validation["result"]["structuredContent"]["validation"]["status"]["in_sync"],
        true
    );
}
