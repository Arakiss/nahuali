use serde_json::json;

use super::support::{McpProcess, temp_store};

pub fn run() {
    let store = temp_store("consolidation-plan");
    let mut server = McpProcess::spawn(&store);
    initialize(&mut server);
    seed_repeated_memory(&mut server);
    assert_consolidation_plan(&mut server);
    server.shutdown();
    let _ = std::fs::remove_file(store);
}

fn initialize(server: &mut McpProcess) {
    let initialized = server.request(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": {"name": "nahuali-test", "version": "0.1.0"}
        }
    }));
    assert_eq!(initialized["result"]["serverInfo"]["name"], "nahuali");
    server.notify(json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized",
        "params": {}
    }));
}

fn seed_repeated_memory(server: &mut McpProcess) {
    for (id, content) in [
        (2, "Lena reviewed the release notes"),
        (3, "Lena updated the launch checklist"),
        (4, "Lena shipped the release notes"),
    ] {
        let remembered = server.request(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {
                "name": "remember",
                "arguments": {
                    "content": content,
                    "tags": ["product"],
                    "mentions": ["Lena"]
                }
            }
        }));
        assert!(remembered["result"]["structuredContent"]["episode"]["id"].is_string());
    }
}

fn assert_consolidation_plan(server: &mut McpProcess) {
    let planned = server.request(json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "tools/call",
        "params": {
            "name": "consolidation_plan",
            "arguments": {
                "episodeLimit": 2,
                "candidateLimit": 8,
                "cycleLimit": 4,
                "evidenceLimit": 4,
                "reviewLimit": 8
            }
        }
    }));
    let report = &planned["result"]["structuredContent"]["report"];
    assert_eq!(report["version"], 1);
    assert_eq!(report["event_count"], 3);
    assert_eq!(report["summary"]["stage_count"], 5);
    assert_eq!(report["summary"]["automatic_write_back"], false);
    assert_eq!(report["write_back_policy"]["automatic_write_back"], false);
    assert!(
        report["operations"]
            .as_array()
            .expect("operations is an array")
            .iter()
            .any(|operation| operation["kind"] == "commit_eligibility")
    );
    assert!(
        report["blocked_items"]
            .as_array()
            .expect("blocked items is an array")
            .iter()
            .any(|item| item["status"] == "needs_review")
    );
}
