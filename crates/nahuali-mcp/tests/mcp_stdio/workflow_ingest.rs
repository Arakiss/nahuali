use serde_json::json;

use super::support::McpProcess;

pub fn assert_structured_ingest_dry_run(server: &mut McpProcess) {
    let response = server.request(json!({
        "jsonrpc": "2.0",
        "id": 35,
        "method": "tools/call",
        "params": {
            "name": "ingest",
            "arguments": {
                "dryRun": true,
                "document": {
                    "version": 1,
                    "source": {
                        "kind": "conversation",
                        "title": "MCP fixture",
                        "uri": "fixture://mcp"
                    },
                    "episodes": [
                        {
                            "ref": "message-1",
                            "content": "MCP dry-run ingestion keeps provenance.",
                            "mentions": ["MCP"]
                        }
                    ]
                }
            }
        }
    }));

    assert_eq!(
        response["result"]["structuredContent"]["report"]["valid"],
        true
    );
    assert_eq!(
        response["result"]["structuredContent"]["report"]["dry_run"],
        true
    );
    assert_eq!(
        response["result"]["structuredContent"]["report"]["ingested_event_count"],
        0
    );
    assert_eq!(
        response["result"]["structuredContent"]["report"]["preflight"]["derived_record_count"],
        0
    );
    assert_eq!(
        response["result"]["structuredContent"]["report"]["preflight"]["unreferenced_episode_count"],
        1
    );
}

pub fn assert_text_ingest_dry_run(server: &mut McpProcess) {
    let response = server.request(json!({
        "jsonrpc": "2.0",
        "id": 39,
        "method": "tools/call",
        "params": {
            "name": "ingest_text",
            "arguments": {
                "content": "MCP text source preserves evidence.\n\nMCP text source stays explicit.",
                "title": "MCP text source",
                "uri": "fixture://mcp-text",
                "kind": "note",
                "chunking": "paragraphs",
                "tags": ["mcp"],
                "mentions": ["MCP"],
                "metadata": {
                    "origin": "fixture"
                },
                "dryRun": true
            }
        }
    }));

    assert_eq!(
        response["result"]["structuredContent"]["adapter_report"]["valid"],
        true
    );
    assert_eq!(
        response["result"]["structuredContent"]["adapter_report"]["episode_count"],
        2
    );
    assert_eq!(
        response["result"]["structuredContent"]["adapter_report"]["document"]["source"]["kind"],
        "note"
    );
    assert_eq!(
        response["result"]["structuredContent"]["report"]["valid"],
        true
    );
    assert_eq!(
        response["result"]["structuredContent"]["report"]["dry_run"],
        true
    );
    assert_eq!(
        response["result"]["structuredContent"]["report"]["appendable_event_count"],
        3
    );
    assert_eq!(
        response["result"]["structuredContent"]["report"]["ingested_event_count"],
        0
    );
    assert_eq!(
        response["result"]["structuredContent"]["report"]["preflight"]["evidence_gap_count"],
        0
    );
    assert_eq!(
        response["result"]["structuredContent"]["report"]["preflight"]["unreferenced_episode_count"],
        2
    );
}
