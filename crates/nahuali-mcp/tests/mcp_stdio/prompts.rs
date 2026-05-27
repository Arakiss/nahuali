use serde_json::json;

use super::support::McpProcess;

pub fn assert_prompts(server: &mut McpProcess) {
    let listed_prompts = server.request(json!({
        "jsonrpc": "2.0",
        "id": 17,
        "method": "prompts/list"
    }));
    let prompt_names = listed_prompts["result"]["prompts"]
        .as_array()
        .expect("prompts/list returns prompts")
        .iter()
        .map(|prompt| {
            prompt["name"]
                .as_str()
                .expect("prompt has a name")
                .to_string()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        prompt_names,
        vec!["recall_with_health_check", "record_evidence_backed_fact"]
    );

    assert_recall_prompt(server);
    assert_record_prompt(server);
    assert_invalid_prompt_error(server);
}

fn assert_recall_prompt(server: &mut McpProcess) {
    let recall_prompt = server.request(json!({
        "jsonrpc": "2.0",
        "id": 18,
        "method": "prompts/get",
        "params": {
            "name": "recall_with_health_check",
            "arguments": {
                "query": "Lena release"
            }
        }
    }));
    let recall_prompt_text = recall_prompt["result"]["messages"][0]["content"]["text"]
        .as_str()
        .expect("prompt returns text");
    assert!(recall_prompt_text.contains("Lena release"));
    assert!(recall_prompt_text.contains("inspect"));
    assert!(recall_prompt_text.contains("evidence_id"));
}

fn assert_record_prompt(server: &mut McpProcess) {
    let record_prompt = server.request(json!({
        "jsonrpc": "2.0",
        "id": 19,
        "method": "prompts/get",
        "params": {
            "name": "record_evidence_backed_fact",
            "arguments": {
                "observation": "Lena owns the release notes",
                "subject": "Lena",
                "predicate": "owns",
                "object": "release notes"
            }
        }
    }));
    let record_prompt_text = record_prompt["result"]["messages"][0]["content"]["text"]
        .as_str()
        .expect("prompt returns text");
    assert!(record_prompt_text.contains("sourceLast: true"));
    assert!(record_prompt_text.contains("Lena owns release notes"));
}

fn assert_invalid_prompt_error(server: &mut McpProcess) {
    let invalid_prompt = server.request(json!({
        "jsonrpc": "2.0",
        "id": 20,
        "method": "prompts/get",
        "params": {
            "name": "recall_with_health_check",
            "arguments": {}
        }
    }));
    assert_eq!(invalid_prompt["error"]["code"], -32602);
    assert_eq!(invalid_prompt["error"]["data"]["argument"], "query");
}
