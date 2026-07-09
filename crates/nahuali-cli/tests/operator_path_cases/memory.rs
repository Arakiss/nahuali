#[test]
fn source_last_operator_path_produces_supported_memory() {
    let store = temp_store("source-last-operator-path");

    let remembered = run_ok(
        &store,
        &[
            "remember",
            "Lena owns the release notes",
            "--tag",
            "product",
        ],
    );
    assert!(remembered.contains("episode_") && remembered.contains("recorded"));

    let asserted = run_ok(
        &store,
        &[
            "fact",
            "Lena",
            "owns",
            "release notes",
            "--confidence",
            "0.92",
            "--source-last",
        ],
    );
    assert!(asserted.contains("fact_") && asserted.contains("asserted"));

    let related = run_ok(
        &store,
        &[
            "relate",
            "Lena",
            "owns",
            "release notes",
            "--confidence",
            "0.9",
            "--source-last",
        ],
    );
    assert!(related.contains("relation_") && related.contains("Relation"));

    let recalled = run_ok(&store, &["recall", "Lena release"]);
    assert!(recalled.starts_with("- [Claim]"));
    assert!(recalled.contains("evidence: episode_"));

    let inspected = run_ok(&store, &["inspect", "--json"]);
    let health: Value = serde_json::from_str(&inspected).expect("inspect output is JSON");
    assert_eq!(health["episode_count"], 1);
    assert_eq!(health["fact_count"], 1);
    assert_eq!(health["relation_count"], 1);
    assert_eq!(health["supported_fact_count"], 1);
    assert_eq!(health["unsupported_fact_count"], 0);

    let status = run_ok(&store, &["status", "--json"]);
    let status: Value = serde_json::from_str(&status).expect("status output is JSON");
    assert_eq!(status["database"], store.display().to_string());
    assert_eq!(status["event_count"], 3);
    assert_eq!(status["surrealdb_graph_projection"]["valid"], true);

    let self_inspected = run_ok(&store, &["self-inspect", "--json"]);
    let report: Value = serde_json::from_str(&self_inspected).expect("self-inspect output is JSON");
    assert_eq!(report["write_back_policy"]["automatic_write_back"], false);
    assert_eq!(report["summary"]["finding_count"], 1);
    assert_eq!(report["summary"]["source_coverage_count"], 1);
    assert!(report["findings"].as_array().unwrap().iter().any(|finding| {
        finding["kind"] == "source_coverage"
            && finding["detail"]
                .as_str()
                .is_some_and(|detail| detail.contains("1 episode(s) lack source records"))
    }));
    assert!(report["review_queue"].as_array().unwrap().iter().any(|item| {
        item["action"] == "capture_evidence"
            && item["finding_id"]
                .as_str()
                .is_some_and(|id| id.contains("source_coverage"))
    }));

    let validated = run_ok(&store, &["validate", "--json"]);
    let validation: Value = serde_json::from_str(&validated).expect("validate output is JSON");
    assert_eq!(validation["database"], store.display().to_string());
    assert_eq!(validation["valid"], true);
    assert_eq!(validation["event_count"], 3);
    assert_eq!(validation["entity_count"], 2);
    assert_eq!(validation["episode_count"], 1);
    assert_eq!(validation["claim_count"], 1);
    assert_eq!(validation["link_count"], 1);
    assert_eq!(validation["fact_count"], 1);
    assert_eq!(validation["relation_count"], 1);

    let projection = run_ok(&store, &["projection-validate", "--json"]);
    let projection: Value =
        serde_json::from_str(&projection).expect("projection output is JSON");
    assert_eq!(projection["database"], store.display().to_string());
    assert_eq!(projection["projection_role"], "derived_from_memory_record");
    assert_eq!(projection["validation"]["valid"], true);
    assert_eq!(projection["validation"]["status"]["in_sync"], true);
    assert_eq!(projection["validation"]["status"]["table_counts"]["episode"], 1);
    assert_eq!(projection["validation"]["status"]["table_counts"]["claim"], 1);
    assert_eq!(
        projection["validation"]["status"]["table_counts"]["relates_to"],
        1
    );

    let projection_rebuild = run_ok(&store, &["projection-rebuild", "--json"]);
    let projection_rebuild: Value =
        serde_json::from_str(&projection_rebuild).expect("projection rebuild output is JSON");
    assert_eq!(projection_rebuild["report"]["status"]["in_sync"], true);
    assert_eq!(
        projection_rebuild["report"]["status"]["table_counts"]["supports"],
        1
    );

    let projected_entities = run_ok(&store, &["projection-entities", "Lena", "--json"]);
    let projected_entities: Value =
        serde_json::from_str(&projected_entities).expect("projection entities output is JSON");
    assert_eq!(projected_entities["entities"].as_array().unwrap().len(), 1);
    assert_eq!(projected_entities["entities"][0]["name"], "Lena");

    let projected_timeline = run_ok(&store, &["projection-timeline", "--json"]);
    let projected_timeline: Value =
        serde_json::from_str(&projected_timeline).expect("projection timeline output is JSON");
    assert_eq!(projected_timeline["episodes"].as_array().unwrap().len(), 1);
    assert_eq!(
        projected_timeline["episodes"][0]["content"],
        "Lena owns the release notes"
    );

    let timeline = run_ok(&store, &["timeline", "--json"]);
    let timeline: Value = serde_json::from_str(&timeline).expect("timeline output is JSON");
    assert_eq!(timeline["episodes"].as_array().unwrap().len(), 1);

    let _ = fs::remove_file(store);
}

#[test]
fn briefing_is_scriptable() {
    let store = temp_store("briefing-scriptable");

    run_ok(
        &store,
        &[
            "remember",
            "Lena owns the release notes",
            "--tag",
            "product",
            "--mention",
            "Lena",
            "--mention",
            "Release Notes",
        ],
    );
    run_ok(
        &store,
        &[
            "claim",
            "Lena",
            "owns",
            "release notes",
            "--confidence",
            "0.92",
            "--source-last",
        ],
    );
    run_ok(
        &store,
        &[
            "claim",
            "Lena",
            "owns",
            "changelog",
            "--confidence",
            "0.91",
        ],
    );
    run_ok(
        &store,
        &[
            "intention",
            "Ship release notes",
            "--priority",
            "high",
            "--source-last",
        ],
    );

    let output = run_ok(
        &store,
        &[
            "briefing",
            "--episode-limit",
            "1",
            "--intention-limit",
            "2",
            "--review-limit",
            "3",
            "--graph-seed-limit",
            "2",
            "--json",
        ],
    );
    let briefing: Value = serde_json::from_str(&output).expect("briefing output is JSON");
    assert_eq!(briefing["database"], store.display().to_string());
    assert_eq!(briefing["report"]["version"], 1);
    assert_eq!(briefing["report"]["event_count"], 4);
    assert_eq!(briefing["report"]["authority"]["mode"], "block");
    assert_eq!(briefing["report"]["summary"]["active_intention_count"], 1);
    assert_eq!(
        briefing["report"]["summary"]["high_priority_review_count"],
        1
    );
    assert_eq!(
        briefing["report"]["recent_episodes"][0]["content"],
        "Lena owns the release notes"
    );
    assert_eq!(
        briefing["report"]["active_intentions"][0]["description"],
        "Ship release notes"
    );
    assert_eq!(
        briefing["report"]["review_items"][0]["priority"],
        "critical"
    );
    assert!(
        briefing["report"]["graph_seeds"]
            .as_array()
            .unwrap()
            .iter()
            .any(|seed| seed["label"] == "Lena")
    );

    let projected_pending = run_ok(&store, &["projection-pending", "--json"]);
    let projected_pending: Value =
        serde_json::from_str(&projected_pending).expect("projection pending output is JSON");
    assert_eq!(projected_pending["intentions"].as_array().unwrap().len(), 1);
    assert_eq!(
        projected_pending["intentions"][0]["description"],
        "Ship release notes"
    );

    let pending = run_ok(&store, &["pending", "--json"]);
    let pending: Value = serde_json::from_str(&pending).expect("pending output is JSON");
    assert_eq!(pending["intentions"].as_array().unwrap().len(), 1);

    let resumed = run_ok(&store, &["session-resume", "--json"]);
    let resumed: Value = serde_json::from_str(&resumed).expect("session-resume output is JSON");
    assert_eq!(resumed["report"]["summary"]["active_intention_count"], 1);

    let human = run_ok(&store, &["briefing", "--episode-limit", "1"]);
    assert!(human.contains("Session briefing"));
    assert!(human.contains("Store trust: BLOCK"));
    assert!(human.contains("Graph seeds"));

    let _ = fs::remove_file(store);
}

#[test]
fn intention_lifecycle_is_scriptable() {
    let store = temp_store("intention-lifecycle-scriptable");

    let goal_output = run_ok(
        &store,
        &[
            "intention",
            "Launch public beta",
            "--kind",
            "goal",
            "--priority",
            "high",
            "--json",
        ],
    );
    let goal: Value = serde_json::from_str(&goal_output).expect("goal output is JSON");
    let goal_id = goal["id"].as_str().unwrap().to_string();

    let dependency_output = run_ok(
        &store,
        &[
            "intention",
            "Prepare release checklist",
            "--priority",
            "medium",
            "--json",
        ],
    );
    let dependency: Value =
        serde_json::from_str(&dependency_output).expect("dependency output is JSON");
    let dependency_id = dependency["id"].as_str().unwrap().to_string();

    let child_output = run_ok(
        &store,
        &[
            "intention",
            "Ship release notes",
            "--priority",
            "high",
            "--json",
        ],
    );
    let child: Value = serde_json::from_str(&child_output).expect("child output is JSON");
    let child_id = child["id"].as_str().unwrap().to_string();

    let updated_output = run_ok(
        &store,
        &[
            "intention-update",
            child_id.as_str(),
            "--description",
            "Ship public release notes",
            "--deadline-at-ms",
            "50",
            "--depends-on",
            dependency_id.as_str(),
            "--depends-on",
            "missing_intention",
            "--goal",
            goal_id.as_str(),
            "--progress",
            "25",
            "--json",
        ],
    );
    let updated: Value =
        serde_json::from_str(&updated_output).expect("updated output is JSON");
    assert_eq!(updated["description"], "Ship public release notes");
    assert_eq!(updated["deadline_at_ms"], 50);
    assert_eq!(updated["depends_on"][0], dependency_id);
    assert_eq!(updated["depends_on"][1], "missing_intention");
    assert_eq!(updated["goal_id"], goal_id);
    assert_eq!(updated["progress_percent"], 25);

    let projected_pending = run_ok(&store, &["projection-pending", "--json"]);
    let projected_pending: Value =
        serde_json::from_str(&projected_pending).expect("projection pending output is JSON");
    let projected_child = projected_pending["intentions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|intention| intention["memory_id"] == child_id.as_str())
        .expect("child is projected as pending");
    assert_eq!(projected_child["deadline_at_ms"], 50);
    assert_eq!(projected_child["progress_percent"], 25);
    assert_eq!(projected_child["depends_on"][0], dependency_id);

    let reconcile_output = run_ok(
        &store,
        &[
            "reconcile-intentions",
            "--now-ms",
            "100",
            "--stale-after-ms",
            "0",
            "--json",
        ],
    );
    let reconcile: Value =
        serde_json::from_str(&reconcile_output).expect("reconcile output is JSON");
    assert_eq!(reconcile["database"], store.display().to_string());
    assert!(reconcile["report"]["issues"].as_array().unwrap().iter().any(
        |issue| issue["kind"] == "overdue" && issue["intention_id"] == child_id.as_str()
    ));
    assert!(reconcile["report"]["issues"].as_array().unwrap().iter().any(
        |issue| issue["kind"] == "waiting_on_dependency" && issue["intention_id"] == child_id.as_str()
    ));
    assert!(reconcile["report"]["issues"].as_array().unwrap().iter().any(
        |issue| issue["kind"] == "missing_dependency" && issue["intention_id"] == child_id.as_str()
    ));

    let proactive_output = run_ok(
        &store,
        &[
            "proactive",
            "--now-ms",
            "100",
            "--deadline-horizon-ms",
            "100",
            "--stale-after-ms",
            "0",
            "--json",
        ],
    );
    let proactive: Value =
        serde_json::from_str(&proactive_output).expect("proactive output is JSON");
    assert_eq!(proactive["database"], store.display().to_string());
    assert_eq!(proactive["source_projection"], "rust");
    assert_eq!(proactive["report"]["version"], 1);
    assert_eq!(
        proactive["report"]["deadlines"]["summary"]["overdue_count"],
        1
    );
    assert!(proactive["report"]["anomalies"]["alerts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|alert| alert["kind"] == "overdue_deadline"));

    let deadlines_output = run_ok(
        &store,
        &[
            "deadlines",
            "--now-ms",
            "100",
            "--horizon-ms",
            "100",
            "--json",
        ],
    );
    let deadlines: Value =
        serde_json::from_str(&deadlines_output).expect("deadlines output is JSON");
    assert_eq!(deadlines["report"]["summary"]["overdue_count"], 1);
    assert_eq!(deadlines["report"]["deadlines"][0]["state"], "overdue");

    let anomalies_output = run_ok(
        &store,
        &[
            "anomalies",
            "--now-ms",
            "100",
            "--stale-after-ms",
            "0",
            "--json",
        ],
    );
    let anomalies: Value =
        serde_json::from_str(&anomalies_output).expect("anomalies output is JSON");
    let overdue_alert_id = anomalies["report"]["alerts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|alert| alert["kind"] == "overdue_deadline")
        .and_then(|alert| alert["id"].as_str())
        .expect("overdue alert exists")
        .to_string();

    let dry_run_ack = run_ok(
        &store,
        &[
            "anomaly-acknowledge",
            overdue_alert_id.as_str(),
            "--note",
            "Reviewed deadline",
            "--dry-run",
            "--json",
        ],
    );
    let dry_run_ack: Value =
        serde_json::from_str(&dry_run_ack).expect("dry-run ack output is JSON");
    assert_eq!(dry_run_ack["dry_run"], true);
    assert_eq!(dry_run_ack["applied"], false);
    assert_eq!(dry_run_ack["anomaly_id"], overdue_alert_id);

    let applied_ack = run_ok(
        &store,
        &[
            "anomaly-acknowledge",
            overdue_alert_id.as_str(),
            "--note",
            "Reviewed deadline",
            "--json",
        ],
    );
    let applied_ack: Value =
        serde_json::from_str(&applied_ack).expect("applied ack output is JSON");
    assert_eq!(applied_ack["applied"], true);
    assert!(applied_ack["event_id"].as_str().is_some());

    let after_ack = run_ok(
        &store,
        &[
            "anomalies",
            "--now-ms",
            "100",
            "--stale-after-ms",
            "0",
            "--json",
        ],
    );
    let after_ack: Value = serde_json::from_str(&after_ack).expect("post-ack anomalies are JSON");
    assert!(after_ack["report"]["alerts"]
        .as_array()
        .unwrap()
        .iter()
        .all(|alert| alert["id"].as_str() != Some(overdue_alert_id.as_str())));

    let progress_output = run_ok(&store, &["goal-progress", "--json"]);
    let progress: Value =
        serde_json::from_str(&progress_output).expect("goal progress output is JSON");
    assert_eq!(progress["report"]["goal_count"], 1);
    assert_eq!(progress["report"]["goals"][0]["goal_id"], goal_id);
    assert_eq!(progress["report"]["goals"][0]["child_count"], 1);
    assert_eq!(progress["report"]["goals"][0]["active_count"], 1);

    let completed_output = run_ok(
        &store,
        &[
            "intention-complete",
            dependency_id.as_str(),
            "--reason",
            "Checklist ready",
            "--json",
        ],
    );
    let completed: Value =
        serde_json::from_str(&completed_output).expect("completed output is JSON");
    assert_eq!(completed["status"], "completed");

    let blocked_output = run_ok(
        &store,
        &[
            "intention-block",
            child_id.as_str(),
            "--reason",
            "Waiting for launch gate",
            "--json",
        ],
    );
    let blocked: Value = serde_json::from_str(&blocked_output).expect("blocked output is JSON");
    assert_eq!(blocked["status"], "blocked");
    assert_eq!(blocked["status_reason"], "Waiting for launch gate");

    let human = run_ok(
        &store,
        &[
            "intention-defer",
            goal_id.as_str(),
            "--reason",
            "Review next launch window",
        ],
    );
    assert!(human.contains("intention_") && human.contains("Intention"));
    assert!(human.contains("Deferred"));

    let human_reconcile = run_ok(
        &store,
        &[
            "reconcile-intentions",
            "--now-ms",
            "100",
            "--stale-after-ms",
            "0",
        ],
    );
    assert!(human_reconcile.contains("Intention reconciliation"));
    assert!(human_reconcile.contains("Blocked"));

    let human_progress = run_ok(&store, &["goal-progress"]);
    assert!(human_progress.contains("Goal progress"));
    assert!(human_progress.contains("blocked=1"));

    let human_proactive = run_ok(&store, &["proactive", "--now-ms", "100"]);
    assert!(human_proactive.contains("Proactive operator report"));
    assert!(human_proactive.contains("Anomalies:"));

    let _ = fs::remove_file(store);
}

#[test]
fn project_view_is_scriptable() {
    let store = temp_store("project-view-scriptable");

    run_ok(
        &store,
        &[
            "remember",
            "Lena owns the release notes",
            "--tag",
            "product",
            "--mention",
            "Lena",
            "--mention",
            "Release Notes",
        ],
    );
    run_ok(
        &store,
        &[
            "claim",
            "Lena",
            "owns",
            "release notes",
            "--confidence",
            "0.92",
            "--source-last",
        ],
    );
    run_ok(
        &store,
        &[
            "link",
            "Lena",
            "owns",
            "Release Notes",
            "--confidence",
            "0.91",
            "--source-last",
        ],
    );
    run_ok(
        &store,
        &[
            "preference",
            "Release notes style",
            "Keep release notes concise",
            "--confidence",
            "0.9",
            "--source-last",
        ],
    );
    run_ok(
        &store,
        &[
            "intention",
            "Ask Lena to publish release notes",
            "--priority",
            "high",
            "--source-last",
        ],
    );

    let output = run_ok(
        &store,
        &[
            "project",
            "Lena",
            "--item-limit",
            "5",
            "--recall-limit",
            "5",
            "--review-limit",
            "5",
            "--json",
        ],
    );
    let project: Value = serde_json::from_str(&output).expect("project output is JSON");
    assert_eq!(project["database"], store.display().to_string());
    assert_eq!(project["source_projection"], "rust");
    assert_eq!(project["report"]["version"], 1);
    assert_eq!(project["report"]["matched_entity"]["name"], "Lena");
    assert_eq!(project["report"]["summary"]["matched_entity"], true);
    assert_eq!(project["report"]["summary"]["claim_count"], 1);
    assert_eq!(project["report"]["summary"]["link_count"], 1);
    assert_eq!(project["report"]["summary"]["procedure_count"], 1);
    assert_eq!(project["report"]["summary"]["intention_count"], 1);
    assert!(
        project["report"]["recall_results"]
            .as_array()
            .unwrap()
            .iter()
            .any(|result| result["kind"] == "claim")
    );

    let human = run_ok(&store, &["project", "Lena", "--item-limit", "2"]);
    assert!(human.contains("Project view"));
    assert!(human.contains("Entity: Lena"));
    assert!(human.contains("Claims:"));
    assert!(human.contains("Intentions:"));

    let _ = fs::remove_file(store);
}

#[test]
fn sleep_mode_is_scriptable() {
    let store = temp_store("sleep-mode-scriptable");

    run_ok(
        &store,
        &[
            "remember",
            "Lena reviewed the release notes",
            "--tag",
            "product",
            "--mention",
            "Lena",
            "--mention",
            "Release Notes",
        ],
    );
    run_ok(
        &store,
        &[
            "remember",
            "Lena updated the launch checklist",
            "--tag",
            "product",
            "--mention",
            "Lena",
        ],
    );
    run_ok(
        &store,
        &[
            "remember",
            "The launch checklist still needs a backup owner",
            "--tag",
            "operations",
            "--mention",
            "Launch Checklist",
        ],
    );

    let output = run_ok(
        &store,
        &[
            "sleep",
            "--episode-limit",
            "2",
            "--candidate-limit",
            "6",
            "--cycle-limit",
            "4",
            "--evidence-limit",
            "4",
            "--json",
        ],
    );
    let sleep: Value = serde_json::from_str(&output).expect("sleep output is JSON");
    assert_eq!(sleep["database"], store.display().to_string());
    assert_eq!(sleep["report"]["version"], 1);
    assert_eq!(sleep["report"]["event_count"], 3);
    assert_eq!(sleep["report"]["summary"]["replayed_episode_count"], 2);
    assert_eq!(sleep["report"]["summary"]["automatic_write_back"], false);
    assert_eq!(
        sleep["report"]["write_back_policy"]["automatic_write_back"],
        false
    );
    assert_eq!(sleep["report"]["stages"].as_array().unwrap().len(), 4);
    assert_eq!(
        sleep["report"]["recent_episodes"].as_array().unwrap().len(),
        2
    );
    assert!(
        sleep["report"]["consolidation_candidates"]
            .as_array()
            .unwrap()
            .iter()
            .any(|candidate| candidate["kind"] == "repeated_episode_tag")
    );

    let human = run_ok(
        &store,
        &["sleep", "--episode-limit", "1", "--candidate-limit", "2"],
    );
    assert!(human.contains("Sleep Mode"));
    assert!(human.contains("Stages:"));
    assert!(human.contains("Automatic write-back: false"));

    let _ = fs::remove_file(store);
}

#[test]
fn consolidation_plan_is_scriptable() {
    let store = temp_store("consolidation-plan-scriptable");

    run_ok(
        &store,
        &[
            "remember",
            "Lena reviewed the release notes",
            "--tag",
            "product",
            "--mention",
            "Lena",
        ],
    );
    run_ok(
        &store,
        &[
            "remember",
            "Lena updated the launch checklist",
            "--tag",
            "product",
            "--mention",
            "Lena",
        ],
    );
    run_ok(
        &store,
        &[
            "remember",
            "Lena shipped the release notes",
            "--tag",
            "product",
            "--mention",
            "Lena",
        ],
    );

    let output = run_ok(
        &store,
        &[
            "consolidation-plan",
            "--episode-limit",
            "2",
            "--candidate-limit",
            "8",
            "--cycle-limit",
            "4",
            "--evidence-limit",
            "4",
            "--review-limit",
            "8",
            "--json",
        ],
    );
    let plan: Value = serde_json::from_str(&output).expect("plan output is JSON");
    assert_eq!(plan["database"], store.display().to_string());
    assert_eq!(plan["report"]["version"], 1);
    assert_eq!(plan["report"]["event_count"], 3);
    assert_eq!(plan["report"]["summary"]["stage_count"], 5);
    assert_eq!(
        plan["report"]["summary"]["automatic_write_back"],
        false
    );
    assert_eq!(
        plan["report"]["write_back_policy"]["automatic_write_back"],
        false
    );
    assert!(
        plan["report"]["operations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|operation| operation["kind"] == "commit_eligibility")
    );
    assert!(
        plan["report"]["blocked_items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["status"] == "needs_review")
    );

    let human = run_ok(
        &store,
        &[
            "consolidation-plan",
            "--episode-limit",
            "1",
            "--candidate-limit",
            "2",
        ],
    );
    assert!(human.contains("Consolidation plan"));
    assert!(human.contains("Operations:"));
    assert!(human.contains("Automatic write-back: false"));

    let _ = fs::remove_file(store);
}

#[test]
fn hook_runtime_is_scriptable() {
    let store = temp_store("hook-runtime-scriptable");

    run_ok(
        &store,
        &[
            "remember",
            "Lena owns the release notes",
            "--tag",
            "product",
            "--mention",
            "Lena",
            "--mention",
            "Release Notes",
        ],
    );
    run_ok(
        &store,
        &[
            "intention",
            "Ship release notes",
            "--priority",
            "high",
            "--source-last",
        ],
    );

    let output = run_ok(
        &store,
        &[
            "hook",
            "pre-prompt",
            "--input",
            "Who owns release notes?",
            "--recall-limit",
            "5",
            "--json",
        ],
    );
    let hook: Value = serde_json::from_str(&output).expect("hook output is JSON");
    assert_eq!(hook["database"], store.display().to_string());
    assert_eq!(hook["report"]["version"], 1);
    assert_eq!(hook["report"]["kind"], "pre_prompt");
    assert_eq!(hook["report"]["input"], "Who owns release notes?");
    assert!(hook["report"]["summary"]["recall_count"].as_u64().unwrap() >= 2);
    assert_eq!(hook["report"]["summary"]["automatic_write_back"], false);
    assert_eq!(hook["report"]["directives"][1]["id"], "memory-recall-required");
    assert!(hook["report"]["recall"]["results"].as_array().unwrap().len() >= 2);

    let sleep = run_ok(&store, &["hook", "sleep-cycle", "--json"]);
    let sleep_hook: Value = serde_json::from_str(&sleep).expect("sleep hook output is JSON");
    assert_eq!(sleep_hook["report"]["kind"], "sleep_cycle");
    assert_eq!(
        sleep_hook["report"]["self_inspection"]["write_back_policy"]["automatic_write_back"],
        false
    );
    assert!(sleep_hook["report"]["reflection"].is_object());

    let human = run_ok(
        &store,
        &[
            "hook",
            "pre-prompt",
            "--input",
            "Who owns release notes?",
        ],
    );
    assert!(human.contains("Memory hook"));
    assert!(human.contains("Use recalled memory before responding"));
    assert!(human.contains("Automatic write-back: false"));

    let missing_input = run(&store, &["hook", "pre-prompt"]);
    assert!(!missing_input.status.success());
    assert!(String::from_utf8_lossy(&missing_input.stderr).contains("query cannot be empty"));

    let _ = fs::remove_file(store);
}

#[test]
fn reflection_cycle_is_scriptable() {
    let store = temp_store("reflection-cycle-scriptable");

    run_ok(
        &store,
        &[
            "remember",
            "Lena owns the release notes",
            "--tag",
            "product",
            "--mention",
            "Lena",
            "--mention",
            "Release Notes",
        ],
    );
    run_ok(
        &store,
        &[
            "claim",
            "Lena",
            "owns",
            "release notes",
            "--confidence",
            "0.92",
            "--source-last",
        ],
    );
    run_ok(
        &store,
        &[
            "claim",
            "Lena",
            "owns",
            "changelog",
            "--confidence",
            "0.91",
        ],
    );

    let output = run_ok(
        &store,
        &[
            "reflect",
            "--cycle-limit",
            "4",
            "--evidence-limit",
            "4",
            "--json",
        ],
    );
    let reflection: Value = serde_json::from_str(&output).expect("reflect output is JSON");
    assert_eq!(reflection["version"], 1);
    assert_eq!(reflection["event_count"], 3);
    assert_eq!(reflection["authority"]["mode"], "block");
    assert_eq!(reflection["summary"]["critical_cycle_count"], 1);
    assert_eq!(reflection["cycles"][0]["priority"], "critical");
    assert_eq!(reflection["cycles"][0]["action"], "resolve_contradiction");
    assert_eq!(
        reflection["write_back_policy"]["automatic_write_back"],
        false
    );

    let human = run_ok(&store, &["reflect", "--cycle-limit", "2"]);
    assert!(human.contains("Reflection cycle"));
    assert!(human.contains("Resolve contradictions"));
    assert!(human.contains("Automatic write-back: false"));

    let _ = fs::remove_file(store);
}

#[test]
fn json_output_covers_expanded_memory_families() {
    let store = temp_store("json-output-expanded-families");

    let episode_output = run_ok(
        &store,
        &[
            "remember",
            "Lena wants release notes kept concise",
            "--tag",
            "product",
            "--mention",
            "Lena",
            "--mention",
            "Release Notes",
            "--json",
        ],
    );
    let episode: Value = serde_json::from_str(&episode_output).expect("episode output is JSON");
    let episode_id = episode["id"]
        .as_str()
        .expect("episode includes id")
        .to_string();
    assert_eq!(
        episode["mentions"],
        serde_json::json!(["Lena", "Release Notes"])
    );

    let claim_output = run_ok(
        &store,
        &[
            "claim",
            "Lena",
            "owns",
            "release notes",
            "--confidence",
            "0.93",
            "--source-last",
            "--json",
        ],
    );
    let claim: Value = serde_json::from_str(&claim_output).expect("claim output is JSON");
    assert!(
        claim["id"]
            .as_str()
            .unwrap_or_default()
            .starts_with("claim_")
    );
    assert_eq!(claim["source_episode_id"], episode_id);

    let link_output = run_ok(
        &store,
        &[
            "link",
            "Lena",
            "owns",
            "Release Notes",
            "--confidence",
            "0.91",
            "--source-last",
            "--json",
        ],
    );
    let link: Value = serde_json::from_str(&link_output).expect("link output is JSON");
    assert!(link["id"].as_str().unwrap_or_default().starts_with("link_"));
    assert_eq!(link["source_episode_id"], episode_id);

    let preference_output = run_ok(
        &store,
        &[
            "preference",
            "Release notes",
            "Keep release notes concise",
            "--source-last",
            "--json",
        ],
    );
    let preference: Value =
        serde_json::from_str(&preference_output).expect("preference output is JSON");
    assert!(
        preference["id"]
            .as_str()
            .unwrap_or_default()
            .starts_with("preference_")
    );
    assert_eq!(preference["kind"], "preference");

    let intention_output = run_ok(
        &store,
        &[
            "intention",
            "Ship release notes",
            "--kind",
            "task",
            "--priority",
            "high",
            "--source-last",
            "--json",
        ],
    );
    let intention: Value =
        serde_json::from_str(&intention_output).expect("intention output is JSON");
    let intention_id = intention["id"]
        .as_str()
        .expect("intention output includes id")
        .to_string();
    assert_eq!(intention["status"], "active");
    assert_eq!(intention["priority"], "high");

    let status_output = run_ok(
        &store,
        &[
            "intention-status",
            &intention_id,
            "completed",
            "--reason",
            "Done",
            "--json",
        ],
    );
    let updated: Value =
        serde_json::from_str(&status_output).expect("intention status output is JSON");
    assert_eq!(updated["status"], "completed");
    assert_eq!(updated["status_reason"], "Done");

    let data_output = run_ok(&store, &["data", "--json"]);
    let data: Value = serde_json::from_str(&data_output).expect("data output is JSON");
    assert_eq!(data["entities"].as_array().unwrap().len(), 2);
    assert_eq!(data["claims"].as_array().unwrap().len(), 1);
    assert_eq!(data["links"].as_array().unwrap().len(), 1);
    assert_eq!(data["procedures"].as_array().unwrap().len(), 1);
    assert_eq!(data["intentions"][0]["status"], "completed");

    let validation_output = run_ok(&store, &["validate", "--json"]);
    let validation: Value =
        serde_json::from_str(&validation_output).expect("validation output is JSON");
    assert_eq!(validation["database"], store.display().to_string());
    assert_eq!(validation["event_count"], 6);
    assert_eq!(validation["entity_count"], 2);
    assert_eq!(validation["claim_count"], 1);
    assert_eq!(validation["link_count"], 1);
    assert_eq!(validation["procedure_count"], 1);
    assert_eq!(validation["intention_count"], 1);

    let authority_output = run_ok(
        &store,
        &["recall", "release notes", "--authority", "--json"],
    );
    let authority: Value =
        serde_json::from_str(&authority_output).expect("authority recall output is JSON");
    assert_eq!(authority["authority"]["mode"], "certify");
    assert_eq!(authority["authority"]["can_trust"], true);
    assert!(authority["results"].as_array().unwrap().len() >= 3);
    assert!(
        authority["results"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|result| result["trust"].as_object())
            .any(|trust| trust["mode"] == "certify" && trust["can_trust"] == true)
    );

    let _ = fs::remove_file(store);
}
