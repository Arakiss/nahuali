use serde_json::json;

use super::support::McpProcess;

pub fn assert_resources(server: &mut McpProcess, episode_id: &str) {
    let listed_resources = server.request(json!({
        "jsonrpc": "2.0",
        "id": 9,
        "method": "resources/list"
    }));
    let resource_uris = listed_resources["result"]["resources"]
        .as_array()
        .expect("resources/list returns resources")
        .iter()
        .map(|resource| {
            assert_eq!(resource["mimeType"], "application/json");
            resource["uri"]
                .as_str()
                .expect("resource has a uri")
                .to_string()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        resource_uris,
        vec![
            "nahuali://database/summary",
            "nahuali://database/sources",
            "nahuali://database/health",
            "nahuali://database/entities",
            "nahuali://database/episodes",
            "nahuali://database/claims",
            "nahuali://database/links",
            "nahuali://database/facts",
            "nahuali://database/relations",
            "nahuali://database/procedures",
            "nahuali://database/intentions",
            "nahuali://database/records",
        ]
    );

    assert_summary_resource(server);
    assert_health_and_source_resources(server);
    assert_projected_resources(server, episode_id);
    assert_missing_resource_error(server);
}

fn assert_summary_resource(server: &mut McpProcess) {
    let summary = server.read_json_resource(10, "nahuali://database/summary");
    assert_eq!(summary["event_count"], 10);
    assert_eq!(summary["source_count"], 0);
    assert_eq!(summary["entity_count"], 3);
    assert_eq!(summary["episode_count"], 1);
    assert_eq!(summary["claim_count"], 2);
    assert_eq!(summary["link_count"], 2);
    assert_eq!(summary["fact_count"], 2);
    assert_eq!(summary["relation_count"], 2);
    assert_eq!(summary["procedure_count"], 2);
    assert_eq!(summary["intention_count"], 1);
    assert_eq!(summary["review_decision_count"], 1);
    assert_eq!(summary["supported_fact_count"], 2);
    assert_eq!(summary["authority_mode"], "certify");
    assert_eq!(summary["authority_score"], 1.0);
    assert_eq!(summary["authority_can_trust"], true);
}

fn assert_health_and_source_resources(server: &mut McpProcess) {
    let health = server.read_json_resource(11, "nahuali://database/health");
    assert_eq!(health["supported_fact_count"], 2);
    assert_eq!(health["unsupported_fact_count"], 0);

    let sources = server.read_json_resource(37, "nahuali://database/sources");
    assert_eq!(sources, json!([]));
}

fn assert_projected_resources(server: &mut McpProcess, episode_id: &str) {
    let episodes = server.read_json_resource(12, "nahuali://database/episodes");
    assert_eq!(episodes[0]["id"], episode_id);
    assert_eq!(episodes[0]["content"], "Lena owns the release notes");
    assert_eq!(episodes[0]["mentions"], json!(["Lena", "Release Notes"]));

    let entities = server.read_json_resource(27, "nahuali://database/entities");
    assert_eq!(entities.as_array().expect("entities array").len(), 3);

    let claims = server.read_json_resource(28, "nahuali://database/claims");
    assert_eq!(claims.as_array().expect("claims array").len(), 2);

    let links = server.read_json_resource(29, "nahuali://database/links");
    assert_eq!(links.as_array().expect("links array").len(), 2);

    let facts = server.read_json_resource(13, "nahuali://database/facts");
    assert_eq!(facts[0]["source_episode_id"], episode_id);

    let relations = server.read_json_resource(14, "nahuali://database/relations");
    assert_eq!(relations[0]["source_episode_id"], episode_id);

    let procedures = server.read_json_resource(30, "nahuali://database/procedures");
    assert_eq!(procedures.as_array().expect("procedures array").len(), 2);

    let intentions = server.read_json_resource(31, "nahuali://database/intentions");
    assert_eq!(intentions[0]["status"], "completed");

    let events = server.read_json_resource(15, "nahuali://database/records");
    assert_eq!(
        events
            .as_array()
            .expect("events resource is an array")
            .len(),
        10
    );
}

fn assert_missing_resource_error(server: &mut McpProcess) {
    let missing_resource = server.request(json!({
        "jsonrpc": "2.0",
        "id": 16,
        "method": "resources/read",
        "params": {
            "uri": "nahuali://database/missing"
        }
    }));
    assert_eq!(missing_resource["error"]["code"], -32002);
    assert_eq!(
        missing_resource["error"]["data"]["uri"],
        "nahuali://database/missing"
    );
}
