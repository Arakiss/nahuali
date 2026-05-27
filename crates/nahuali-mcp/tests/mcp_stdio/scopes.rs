use std::fs;

use serde_json::json;

use super::support::{McpProcess, temp_store};

pub fn run() {
    let store = temp_store("stdio-scopes");
    let mut server = McpProcess::spawn(&store);

    initialize_server(&mut server);
    remember_scoped_episode(&mut server, 2, "Nahuali", "Lena owns Nahuali release notes");
    record_scoped_claim(&mut server);
    remember_scoped_episode(&mut server, 3, "Atlas", "Lena owns Atlas release notes");
    assert_scoped_recall(&mut server);

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
                "name": "nahuali-test-client",
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

fn remember_scoped_episode(server: &mut McpProcess, id: u64, name: &str, content: &str) {
    let remembered = server.request(json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": {
            "name": "remember",
            "arguments": {
                "content": content,
                "mentions": ["Lena"],
                "scope": {
                    "kind": "project",
                    "name": name
                }
            }
        }
    }));

    assert_eq!(remembered["result"]["isError"], false);
    assert_eq!(
        remembered["result"]["structuredContent"]["episode"]["scope"]["key"],
        format!("project:{}", name.to_ascii_lowercase())
    );
}

fn record_scoped_claim(server: &mut McpProcess) {
    let claimed = server.request(json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "tools/call",
        "params": {
            "name": "claim",
            "arguments": {
                "subject": "Lena",
                "predicate": "owns",
                "object": "release notes",
                "sourceLast": true,
                "scope": {
                    "kind": "project",
                    "name": "Nahuali"
                }
            }
        }
    }));

    assert_eq!(claimed["result"]["isError"], false);
    assert_eq!(
        claimed["result"]["structuredContent"]["claim"]["scope"]["key"],
        "project:nahuali"
    );
}

fn assert_scoped_recall(server: &mut McpProcess) {
    let recalled = server.request(json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {
            "name": "recall",
            "arguments": {
                "query": "release notes",
                "scope": {
                    "kind": "project",
                    "name": "Nahuali"
                }
            }
        }
    }));
    let results = recalled["result"]["structuredContent"]["results"]
        .as_array()
        .expect("recall returns results");

    assert!(!results.is_empty());
    assert!(results.iter().all(|result| {
        result["scope"]["key"]
            .as_str()
            .is_some_and(|key| key == "project:nahuali")
    }));
    assert!(!results.iter().any(|result| {
        result["excerpt"]
            .as_str()
            .unwrap_or_default()
            .contains("Atlas")
    }));

    let filtered = server.request(json!({
        "jsonrpc": "2.0",
        "id": 6,
        "method": "tools/call",
        "params": {
            "name": "recall",
            "arguments": {
                "query": "release notes",
                "scope": {
                    "kind": "project",
                    "name": "Nahuali"
                },
                "kinds": ["claim"],
                "requireEvidence": true
            }
        }
    }));
    let filtered_results = filtered["result"]["structuredContent"]["results"]
        .as_array()
        .expect("filtered recall returns results");

    assert_eq!(filtered_results.len(), 1);
    assert_eq!(filtered_results[0]["kind"], "claim");
    assert!(
        filtered_results[0]["evidence_id"]
            .as_str()
            .is_some_and(|id| id.starts_with("episode_"))
    );
}
