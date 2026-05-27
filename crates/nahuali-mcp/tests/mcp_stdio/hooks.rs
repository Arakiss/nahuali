use std::fs;

use serde_json::json;

use super::support::{McpProcess, temp_store};

pub fn run() {
    let store = temp_store("stdio-memory-hook");
    let mut server = McpProcess::spawn(&store);

    initialize_server(&mut server);
    assert_memory_hook_is_listed(&mut server);
    record_memory(&mut server);
    assert_pre_prompt_hook(&mut server);
    assert_sleep_cycle_hook(&mut server);

    server.shutdown();
    let _ = fs::remove_file(store);
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
                "name": "nahuali-hook-test-client",
                "version": "0.1.0"
            }
        }
    }));
    assert_eq!(initialized["result"]["protocolVersion"], "2025-11-25");
    assert!(initialized["result"]["capabilities"]["tools"].is_object());

    server.notify(json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    }));
}

fn assert_memory_hook_is_listed(server: &mut McpProcess) {
    let listed = server.request(json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list"
    }));
    let tools = listed["result"]["tools"]
        .as_array()
        .expect("tools/list returns tools");
    let hook_tool = tools
        .iter()
        .find(|tool| tool["name"] == "memory_hook")
        .expect("memory_hook tool is listed");

    assert!(
        hook_tool["description"]
            .as_str()
            .unwrap_or_default()
            .contains("host execution point")
    );
}

fn record_memory(server: &mut McpProcess) {
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
    assert_eq!(remembered["result"]["isError"], false);

    let intention = server.request(json!({
        "jsonrpc": "2.0",
        "id": 4,
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
    assert_eq!(intention["result"]["isError"], false);
}

fn assert_pre_prompt_hook(server: &mut McpProcess) {
    let response = server.request(json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "tools/call",
        "params": {
            "name": "memory_hook",
            "arguments": {
                "kind": "pre_prompt",
                "input": "Who owns release notes?",
                "recallLimit": 5
            }
        }
    }));
    let report = &response["result"]["structuredContent"]["report"];

    assert_eq!(response["result"]["isError"], false);
    assert_eq!(report["version"], 1);
    assert_eq!(report["kind"], "pre_prompt");
    assert!(report["summary"]["recall_count"].as_u64().unwrap() >= 2);
    assert_eq!(report["summary"]["automatic_write_back"], false);
    assert_eq!(report["directives"][1]["id"], "memory-recall-required");
    assert!(report["recall"]["results"].as_array().unwrap().len() >= 2);
}

fn assert_sleep_cycle_hook(server: &mut McpProcess) {
    let response = server.request(json!({
        "jsonrpc": "2.0",
        "id": 6,
        "method": "tools/call",
        "params": {
            "name": "memory_hook",
            "arguments": {
                "kind": "sleep_cycle",
                "cycleLimit": 4,
                "evidenceLimit": 4
            }
        }
    }));
    let report = &response["result"]["structuredContent"]["report"];

    assert_eq!(response["result"]["isError"], false);
    assert_eq!(report["kind"], "sleep_cycle");
    assert!(report["reflection"].is_object());
    assert!(report["self_inspection"].is_object());
    assert!(report["sleep"].is_object());
    assert_eq!(report["summary"]["sleep_stage_count"], 4);
    assert!(report["summary"]["sleep_candidate_count"].as_u64().unwrap() >= 1);
    assert_eq!(
        report["self_inspection"]["write_back_policy"]["automatic_write_back"],
        false
    );
    assert_eq!(report["sleep"]["version"], 1);
    assert_eq!(report["sleep"]["summary"]["automatic_write_back"], false);
    assert_eq!(
        report["sleep"]["write_back_policy"]["automatic_write_back"],
        false
    );
    assert_eq!(report["sleep"]["stages"].as_array().unwrap().len(), 4);
    assert!(
        !report["sleep"]["recent_episodes"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        report["sleep"]["consolidation_candidates"]
            .as_array()
            .unwrap()
            .iter()
            .any(|candidate| candidate["kind"] == "source_coverage_gap")
    );
}
