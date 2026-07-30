use std::{
    collections::HashSet,
    sync::OnceLock,
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use nahuali_api::{ApiConfig, OPENAPI_JSON, router};
use serde_json::{Value, json};
use tokio::sync::Mutex;
use tower::ServiceExt;

static API_CONTRACT_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static API_TEST_ENDPOINT: OnceLock<()> = OnceLock::new();

#[test]
fn openapi_contract_matches_the_product_version() {
    let openapi: Value = serde_json::from_str(OPENAPI_JSON).expect("OpenAPI JSON parses");
    assert_eq!(openapi["info"]["version"], env!("CARGO_PKG_VERSION"));
}

#[test]
fn openapi_contract_has_the_frozen_beta_path_set() {
    let openapi: Value = serde_json::from_str(OPENAPI_JSON).expect("OpenAPI JSON parses");
    let mut paths = openapi["paths"]
        .as_object()
        .expect("OpenAPI paths is an object")
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    paths.sort_unstable();

    assert_eq!(
        paths,
        vec![
            "/v1/anomalies",
            "/v1/anomaly/acknowledge",
            "/v1/audit",
            "/v1/claim",
            "/v1/deadlines",
            "/v1/episode",
            "/v1/goal-progress",
            "/v1/graph",
            "/v1/intention",
            "/v1/intention/reconcile",
            "/v1/intention/status",
            "/v1/intention/update",
            "/v1/link",
            "/v1/memory-health",
            "/v1/openapi.json",
            "/v1/pending",
            "/v1/proactive",
            "/v1/procedure",
            "/v1/projection/rebuild",
            "/v1/projection/status",
            "/v1/projection/validate",
            "/v1/ready",
            "/v1/recall",
            "/v1/review/resolve",
            "/v1/semantic/rebuild",
            "/v1/semantic/status",
            "/v1/semantic/sync",
            "/v1/session-resume",
            "/v1/status",
            "/v1/timeline",
            "/v1/trust-report",
        ]
    );
}

#[test]
fn openapi_contract_has_beta_operation_shapes() {
    let openapi: Value = serde_json::from_str(OPENAPI_JSON).expect("OpenAPI JSON parses");
    let paths = openapi["paths"]
        .as_object()
        .expect("OpenAPI paths is an object");
    let mut operation_ids = HashSet::new();

    for (path, path_item) in paths {
        let methods = path_item
            .as_object()
            .expect("OpenAPI path item is an object");
        for (method, operation) in methods {
            assert!(
                method == "get" || method == "post",
                "{path} exposes unexpected HTTP method {method}"
            );

            let operation_id = operation["operationId"]
                .as_str()
                .unwrap_or_else(|| panic!("{method} {path} is missing operationId"));
            assert!(
                operation_ids.insert(operation_id.to_string()),
                "duplicate OpenAPI operationId {operation_id}"
            );
            assert!(
                operation["responses"]["200"]["$ref"].is_string(),
                "{operation_id} is missing a 200 JSON response ref"
            );
            assert!(
                operation["responses"]["default"]["$ref"].is_string(),
                "{operation_id} is missing the default structured error response"
            );

            if method == "post" {
                let request_ref = operation["requestBody"]["$ref"]
                    .as_str()
                    .unwrap_or_else(|| panic!("{operation_id} is missing a JSON request body ref"));
                let request_body = resolve_component_ref(&openapi, request_ref);
                assert!(
                    request_body["content"]["application/json"]["schema"]["$ref"].is_string(),
                    "{operation_id} request body must expose an application/json schema ref"
                );
            }
        }
    }

    assert_eq!(operation_ids.len(), paths.len());
    assert_schema_requires(&openapi, "EpisodeRequest", "content");
    assert_schema_requires(&openapi, "RecallRequest", "query");
    assert_schema_requires(&openapi, "ReviewResolveRequest", "review_id");

    let memory_scope_kinds = openapi["components"]["schemas"]["MemoryScope"]["properties"]["kind"]
        ["enum"]
        .as_array()
        .expect("MemoryScope kind enum is present");
    assert!(memory_scope_kinds.iter().any(|kind| kind == "project"));

    let graph_parameters = openapi["paths"]["/v1/graph"]["get"]["parameters"]
        .as_array()
        .expect("graph exposes query parameters");
    let seed_parameter = graph_parameters
        .iter()
        .map(|parameter| resolve_component_ref(&openapi, parameter["$ref"].as_str().unwrap()))
        .find(|parameter| parameter["name"] == "seed")
        .expect("graph seed parameter is documented");
    assert_eq!(seed_parameter["required"], true);

    assert_eq!(
        paths["/v1/trust-report"]["get"]["responses"]["200"]["$ref"],
        "#/components/responses/TrustReportResponse"
    );
    assert_schema_requires(&openapi, "MemoryTrustReport", "integrity");
    assert_schema_requires(&openapi, "MemoryTrustReport", "health");
    assert_schema_requires(&openapi, "TrustIntegrity", "ledger_verified");
    assert_schema_does_not_require(&openapi, "TrustIntegrity", "chain_intact");
    assert_schema_does_not_require(&openapi, "TrustIntegrity", "chain_status");
}

#[tokio::test]
async fn api_records_episode_claim_and_returns_authority_recall() {
    let _guard = api_contract_lock().lock().await;
    let app = router(ApiConfig::new(temp_database(
        "api_records_episode_claim_and_returns_authority_recall",
    )));

    let episode = json_request(
        app.clone(),
        "/v1/episode",
        json!({
            "content": "Lena owns the release notes.",
            "tags": ["product"],
            "mentions": ["Lena", "Release Notes"]
        }),
    )
    .await;
    assert_eq!(episode["content"], "Lena owns the release notes.");
    let episode_id = episode["id"].as_str().unwrap();

    let claim = json_request(
        app.clone(),
        "/v1/claim",
        json!({
            "subject": "Lena",
            "predicate": "owns",
            "object": "release notes",
            "source_episode_id": episode_id,
            "confidence": 0.92
        }),
    )
    .await;
    assert_eq!(claim["subject"], "Lena");
    assert_eq!(claim["source_episode_id"], episode_id);

    let recall = json_request(
        app.clone(),
        "/v1/recall",
        json!({
            "query": "Lena release",
            "limit": 10,
            "require_evidence": true
        }),
    )
    .await;
    assert!(recall["authority"]["mode"].is_string());
    assert!(
        recall["lexical_results"]
            .as_array()
            .unwrap()
            .iter()
            .any(|result| result["id"] == claim["id"])
    );
    assert!(recall.get("health").is_some());

    let projection = get_json(app, "/v1/projection/status").await;
    assert_eq!(projection["projection_role"], "derived_from_memory_record");
    assert_eq!(projection["status"]["in_sync"], true);
    assert_eq!(projection["status"]["table_counts"]["episode"], 1);
    assert_eq!(projection["status"]["table_counts"]["claim"], 1);
}

#[tokio::test]
async fn api_records_links_procedures_and_review_resolutions() {
    let _guard = api_contract_lock().lock().await;
    let app = router(ApiConfig::new(temp_database(
        "api_records_links_procedures_and_review_resolutions",
    )));

    let episode = json_request(
        app.clone(),
        "/v1/episode",
        json!({
            "content": "Lena owns the release notes and follows the beta RC checklist.",
            "tags": ["api", "beta"],
            "mentions": ["Lena", "Release Notes"]
        }),
    )
    .await;
    let episode_id = episode["id"].as_str().unwrap();

    let link = json_request(
        app.clone(),
        "/v1/link",
        json!({
            "from": "Lena",
            "relation": "owns",
            "to": "release notes",
            "source_episode_id": episode_id,
            "confidence": 0.91
        }),
    )
    .await;
    assert_eq!(link["from"], "Lena");
    assert_eq!(link["relation"], "owns");
    assert_eq!(link["to"], "release notes");
    assert_eq!(link["source_episode_id"], episode_id);

    let procedure = json_request(
        app.clone(),
        "/v1/procedure",
        json!({
            "name": "beta-rc-check",
            "body": "Run the local release-candidate gate before treating the beta as ready.",
            "source_episode_id": episode_id,
            "confidence": 0.88
        }),
    )
    .await;
    assert_eq!(procedure["kind"], "procedure");
    assert_eq!(procedure["name"], "beta-rc-check");
    assert_eq!(procedure["source_episode_id"], episode_id);

    let ready_claim = json_request(
        app.clone(),
        "/v1/claim",
        json!({
            "subject": "Beta API",
            "predicate": "status",
            "object": "ready",
            "source_episode_id": episode_id,
            "confidence": 0.92
        }),
    )
    .await;
    assert_eq!(ready_claim["source_episode_id"], episode_id);

    // A later, unprovenanced claim disagrees with the sourced one. Because the
    // two values do NOT share a source episode (this one has none), they are a
    // genuine contradiction rather than a same-episode multi-valued observation,
    // so the store must raise a contradiction review item.
    let blocked_claim = json_request(
        app.clone(),
        "/v1/claim",
        json!({
            "subject": "Beta API",
            "predicate": "status",
            "object": "blocked",
            "confidence": 0.92
        }),
    )
    .await;
    assert!(blocked_claim["source_episode_id"].is_null());

    let proactive = json_request(
        app.clone(),
        "/v1/proactive",
        json!({
            "review_limit": 10
        }),
    )
    .await;
    let review_id = proactive["report"]["high_risk_review_items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["finding_kind"] == "contradiction")
        .and_then(|item| item["id"].as_str())
        .expect("conflicting claims create a high-risk review item")
        .to_string();

    let dry_run = json_request(
        app.clone(),
        "/v1/review/resolve",
        json!({
            "review_id": review_id,
            "note": "Operator confirmed this claim from release ownership notes.",
            "dry_run": true
        }),
    )
    .await;
    assert_eq!(dry_run["dry_run"], true);
    assert_eq!(dry_run["applied"], false);
    assert_eq!(dry_run["review_id"], review_id);

    let status_before_apply = get_json(app.clone(), "/v1/status").await;
    assert_eq!(status_before_apply["link_count"], 1);
    assert_eq!(status_before_apply["procedure_count"], 1);
    assert_eq!(status_before_apply["review_decision_count"], 0);

    let applied = json_request(
        app.clone(),
        "/v1/review/resolve",
        json!({
            "review_id": review_id,
            "note": "Operator confirmed this claim from release ownership notes."
        }),
    )
    .await;
    assert_eq!(applied["dry_run"], false);
    assert_eq!(applied["applied"], true);
    assert_eq!(applied["review_id"], review_id);
    assert!(
        applied["event_id"]
            .as_str()
            .unwrap_or_default()
            .starts_with("event_")
    );

    let status_after_apply = get_json(app, "/v1/status").await;
    assert_eq!(status_after_apply["review_decision_count"], 1);
}

#[tokio::test]
async fn api_reads_projection_timeline_pending_and_session_resume() {
    let _guard = api_contract_lock().lock().await;
    let app = router(ApiConfig::new(temp_database(
        "api_reads_projection_timeline_pending_and_session_resume",
    )));

    let episode = json_request(
        app.clone(),
        "/v1/episode",
        json!({
            "content": "The beta API needs a real contract.",
            "tags": ["api"],
            "mentions": ["Nahuali API"]
        }),
    )
    .await;

    let intention = json_request(
        app.clone(),
        "/v1/intention",
        json!({
            "description": "Write the local client contract tests",
            "kind": "task",
            "priority": "high",
            "source_episode_id": episode["id"]
        }),
    )
    .await;
    assert_eq!(intention["status"], "active");

    let timeline = get_json(app.clone(), "/v1/timeline?limit=5").await;
    assert_eq!(timeline["projection_role"], "derived_from_memory_record");
    assert_eq!(timeline["episodes"].as_array().unwrap().len(), 1);

    let pending = get_json(app.clone(), "/v1/pending?limit=5").await;
    assert_eq!(pending["projection_role"], "derived_from_memory_record");
    assert_eq!(pending["intentions"][0]["memory_id"], intention["id"]);

    let audit = get_json(app.clone(), "/v1/audit").await;
    assert_eq!(audit["integrity"]["verified"], true);
    assert_eq!(audit["from_sequence"], 0);
    assert!(audit["range_event_count"].as_u64().unwrap() >= 1);
    assert!(audit["counts"]["episodes_recorded"].as_u64().unwrap() >= 1);
    assert!(
        audit["entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["kind"] == "episode_recorded")
    );

    let trust = get_json(app.clone(), "/v1/trust-report").await;
    assert_eq!(trust["integrity"]["ledger_verified"], true);
    assert!(trust["knowledge"]["episode_count"].as_u64().unwrap() >= 1);
    assert!(trust["authority"]["mode"].is_string());
    assert!(trust["trustworthy"].is_boolean());

    let resume = json_request(
        app,
        "/v1/session-resume",
        json!({
            "episode_limit": 5,
            "intention_limit": 5,
            "review_limit": 5,
            "graph_seed_limit": 5
        }),
    )
    .await;
    assert!(resume["authority"]["mode"].is_string());
    assert_eq!(resume["briefing"]["summary"]["episode_count"], 1);
    assert_eq!(resume["briefing"]["summary"]["active_intention_count"], 1);
}

#[tokio::test]
async fn api_exposes_operator_loop_contract() {
    let _guard = api_contract_lock().lock().await;
    let app = router(ApiConfig::new(temp_database(
        "api_exposes_operator_loop_contract",
    )));

    let episode = json_request(
        app.clone(),
        "/v1/episode",
        json!({
            "content": "The public beta needs operator-loop parity.",
            "tags": ["beta"],
            "mentions": ["Nahuali API"]
        }),
    )
    .await;

    let goal = json_request(
        app.clone(),
        "/v1/intention",
        json!({
            "description": "Ship the public beta",
            "kind": "goal",
            "priority": "critical",
            "source_episode_id": episode["id"]
        }),
    )
    .await;

    let task = json_request(
        app.clone(),
        "/v1/intention",
        json!({
            "description": "Expose proactive operator loops in the API",
            "kind": "task",
            "priority": "high",
            "source_episode_id": episode["id"]
        }),
    )
    .await;

    let updated = json_request(
        app.clone(),
        "/v1/intention/update",
        json!({
            "id": task["id"],
            "deadline_at_ms": 50,
            "goal_id": goal["id"],
            "progress_percent": 25
        }),
    )
    .await;
    assert_eq!(updated["deadline_at_ms"], 50);
    assert_eq!(updated["goal_id"], goal["id"]);
    assert_eq!(updated["progress_percent"], 25);

    let progress = get_json(app.clone(), "/v1/goal-progress").await;
    assert_eq!(progress["report"]["goal_count"], 1);
    assert_eq!(progress["report"]["goals"][0]["goal_id"], goal["id"]);
    assert_eq!(progress["report"]["goals"][0]["child_count"], 1);

    let reconciliation = json_request(
        app.clone(),
        "/v1/intention/reconcile",
        json!({
            "now_ms": 100,
            "stale_after_ms": 0
        }),
    )
    .await;
    assert!(
        reconciliation["report"]["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|issue| issue["intention_id"] == task["id"] && issue["kind"] == "overdue")
    );

    let deadlines = json_request(
        app.clone(),
        "/v1/deadlines",
        json!({
            "now_ms": 100,
            "deadline_horizon_ms": 1000,
            "stale_after_ms": 0
        }),
    )
    .await;
    assert_eq!(deadlines["source_projection"], "rust");
    assert_eq!(deadlines["report"]["summary"]["overdue_count"], 1);

    let proactive = json_request(
        app.clone(),
        "/v1/proactive",
        json!({
            "now_ms": 100,
            "deadline_horizon_ms": 1000,
            "stale_after_ms": 0,
            "review_limit": 10
        }),
    )
    .await;
    assert_eq!(proactive["report"]["summary"]["overdue_deadline_count"], 1);
    assert_eq!(
        proactive["report"]["write_back_policy"]["automatic_write_back"],
        false
    );

    let anomalies = json_request(
        app.clone(),
        "/v1/anomalies",
        json!({
            "now_ms": 100,
            "deadline_horizon_ms": 1000,
            "stale_after_ms": 0
        }),
    )
    .await;
    let alert_id = anomalies["report"]["alerts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|alert| alert["kind"] == "overdue_deadline")
        .and_then(|alert| alert["id"].as_str())
        .expect("overdue deadline alert exists")
        .to_string();

    let dry_run_ack = json_request(
        app.clone(),
        "/v1/anomaly/acknowledge",
        json!({
            "anomaly_id": alert_id,
            "note": "Operator reviewed this alert in the API contract test.",
            "dry_run": true
        }),
    )
    .await;
    assert_eq!(dry_run_ack["report"]["dry_run"], true);
    assert_eq!(dry_run_ack["report"]["applied"], false);

    let openapi = get_json(app.clone(), "/v1/openapi.json").await;
    assert!(
        openapi["paths"]
            .as_object()
            .unwrap()
            .contains_key("/v1/proactive")
    );
    assert!(
        openapi["paths"]
            .as_object()
            .unwrap()
            .contains_key("/v1/intention/update")
    );

    let status = json_request(
        app,
        "/v1/intention/status",
        json!({
            "id": task["id"],
            "status": "completed",
            "reason": "API parity validated"
        }),
    )
    .await;
    assert_eq!(status["status"], "completed");
    assert_eq!(status["status_reason"], "API parity validated");
}

#[tokio::test]
async fn api_returns_structured_core_errors() {
    let _guard = api_contract_lock().lock().await;
    let app = router(ApiConfig::new(temp_database(
        "api_returns_structured_core_errors",
    )));

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/episode")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "content": " " }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response_json(response).await;
    assert_eq!(body["error"]["code"], "empty_content");
}

#[tokio::test]
async fn api_health_does_not_open_the_engine() {
    let _guard = api_contract_lock().lock().await;
    // Intentionally point at a database name that is never opened; /health must
    // answer 200 without touching SurrealDB.
    let app = router(ApiConfig::new(temp_database(
        "api_health_does_not_open_the_engine",
    )));

    for uri in ["/health", "/v1/health"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{uri} should be 200");
        let body = response_json(response).await;
        assert_eq!(body["status"], "ok", "{uri} body");
    }

    let root = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(root.status(), StatusCode::OK);
    let root_body = response_json(root).await;
    assert_eq!(root_body["openapi"], "/v1/openapi.json");
    assert_eq!(root_body["readiness"], "/v1/ready");
}

#[tokio::test]
async fn api_readiness_verifies_ledger_graph_and_configured_semantic_state() {
    let _guard = api_contract_lock().lock().await;
    let database = temp_database("api_readiness_verifies_configured_tiers");
    let app = router(ApiConfig::new(&database));
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let empty = response_json(response).await;
    assert_eq!(empty["ready"], false);
    assert_eq!(empty["ledger"]["ready"], true);
    assert_eq!(empty["graph"]["ready"], false);

    let rebuild = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/projection/rebuild")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rebuild.status(), StatusCode::OK);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let ready = response_json(response).await;
    assert_eq!(ready["ready"], true);
    assert_eq!(ready["ledger"]["ready"], true);
    assert_eq!(ready["graph"]["ready"], true);
    assert_eq!(ready["semantic"]["required"], false);
    assert_eq!(ready["semantic"]["status"], "not_required");

    let semantic_database = temp_database("api_readiness_requires_semantic");
    let semantic_app = router(ApiConfig::new(&semantic_database).require_semantic(true));
    let before = semantic_app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(before.status(), StatusCode::SERVICE_UNAVAILABLE);
    let before = response_json(before).await;
    assert_eq!(before["ready"], false);
    assert_eq!(before["semantic"]["required"], true);
    assert_eq!(before["semantic"]["ready"], false);

    json_request(
        semantic_app.clone(),
        "/v1/episode",
        json!({"content": "Hrafn verifies HTTP readiness."}),
    )
    .await;
    let rebuild = semantic_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/semantic/rebuild")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    if rebuild.status() == StatusCode::BAD_GATEWAY {
        return;
    }
    assert_eq!(rebuild.status(), StatusCode::OK);

    let after = semantic_app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(after.status(), StatusCode::OK);
    let after = response_json(after).await;
    assert_eq!(after["ready"], true);
    assert_eq!(after["semantic"]["status"], "current");
    assert_eq!(after["semantic"]["missing_point_count"], 0);
    assert_eq!(after["semantic"]["orphan_point_count"], 0);
    assert_eq!(after["semantic"]["stale_point_count"], 0);
}

#[tokio::test]
async fn api_wraps_transport_errors_in_the_structured_envelope() {
    let _guard = api_contract_lock().lock().await;
    let app = router(ApiConfig::new(temp_database(
        "api_wraps_transport_errors_in_the_structured_envelope",
    )));

    // Unmatched route -> 404 with the universal envelope (was an empty body).
    let not_found = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/does-not-exist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(not_found.status(), StatusCode::NOT_FOUND);
    let not_found_body = response_json(not_found).await;
    assert_eq!(not_found_body["error"]["code"], "not_found");
    assert!(not_found_body["error"]["message"].is_string());

    // Wrong method on a real route -> 405 with the envelope (was an empty body).
    let method_not_allowed = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/episode")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(method_not_allowed.status(), StatusCode::METHOD_NOT_ALLOWED);
    let method_body = response_json(method_not_allowed).await;
    assert_eq!(method_body["error"]["code"], "method_not_allowed");

    // Malformed JSON -> 400 malformed_json (was a text/plain body).
    let malformed = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/episode")
                .header("content-type", "application/json")
                .body(Body::from("{ this is not json"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(malformed.status(), StatusCode::BAD_REQUEST);
    let malformed_body = response_json(malformed).await;
    assert_eq!(malformed_body["error"]["code"], "malformed_json");

    // Missing required field -> 400 validation_error (was a text/plain body).
    let missing_field = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/episode")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "tags": ["x"] }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_field.status(), StatusCode::BAD_REQUEST);
    let missing_field_body = response_json(missing_field).await;
    assert_eq!(missing_field_body["error"]["code"], "validation_error");

    // Unknown field -> 400 validation_error (deny_unknown_fields contract).
    let unknown_field = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/recall")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "query": "x", "authority": true }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unknown_field.status(), StatusCode::BAD_REQUEST);
    let unknown_field_body = response_json(unknown_field).await;
    assert_eq!(unknown_field_body["error"]["code"], "validation_error");

    // Missing/wrong Content-Type -> 415 unsupported_media_type.
    let wrong_content_type = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/episode")
                .header("content-type", "text/plain")
                .body(Body::from(json!({ "content": "hi" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        wrong_content_type.status(),
        StatusCode::UNSUPPORTED_MEDIA_TYPE
    );
    let wrong_content_type_body = response_json(wrong_content_type).await;
    assert_eq!(
        wrong_content_type_body["error"]["code"],
        "unsupported_media_type"
    );
}

async fn json_request(app: axum::Router, uri: &str, body: Value) -> Value {
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = response_json(response).await;
    assert_eq!(status, StatusCode::OK, "{uri} returned {status}: {body}");
    body
}

async fn get_json(app: axum::Router, uri: &str) -> Value {
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = response_json(response).await;
    assert_eq!(status, StatusCode::OK, "{uri} returned {status}: {body}");
    body
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

fn resolve_component_ref<'a>(openapi: &'a Value, reference: &str) -> &'a Value {
    let path = reference
        .strip_prefix("#/")
        .unwrap_or_else(|| panic!("unsupported OpenAPI reference {reference}"));
    let mut value = openapi;
    for segment in path.split('/') {
        value = &value[segment];
    }
    if let Some(nested) = value.get("$ref").and_then(Value::as_str) {
        resolve_component_ref(openapi, nested)
    } else {
        value
    }
}

fn assert_schema_requires(openapi: &Value, schema: &str, property: &str) {
    let required = openapi["components"]["schemas"][schema]["required"]
        .as_array()
        .unwrap_or_else(|| panic!("{schema} schema is missing required properties"));
    assert!(
        required.iter().any(|required| required == property),
        "{schema} must require {property}"
    );
}

fn assert_schema_does_not_require(openapi: &Value, schema: &str, property: &str) {
    let required = openapi["components"]["schemas"][schema]["required"]
        .as_array()
        .unwrap_or_else(|| panic!("{schema} schema is missing required properties"));
    assert!(
        !required.iter().any(|required| required == property),
        "{schema} must leave {property} optional for builds without tamper-evidence"
    );
}

fn temp_database(name: &str) -> String {
    API_TEST_ENDPOINT.get_or_init(|| {
        // Workspace test binaries run concurrently. Keep this contract suite on
        // its own process-specific store, so it cannot contend with another
        // binary for the default embedded SurrealKV directory.
        let endpoint = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/test-data")
            .join(format!("nahuali-api-http-contract-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&endpoint);
        unsafe {
            std::env::set_var(
                "NAHUALI_DB_URL",
                format!("surrealkv://{}", endpoint.display()),
            )
        };
    });
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("nahuali_{name}_{nanos}")
}

fn api_contract_lock() -> &'static Mutex<()> {
    API_CONTRACT_LOCK.get_or_init(|| Mutex::new(()))
}
