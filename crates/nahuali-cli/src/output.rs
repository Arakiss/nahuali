use std::path::Path;

use nahuali_core::{
    AnomalyAcknowledgementReport, AnomalyReport, BackupIssueKind, DeadlineReport,
    GoalProgressReport, IngestionReport, IntentionReconciliationReport, InterchangeImportReport,
    MemoryBriefingReport, MemoryConsolidationPlanReport, MemoryGraphReport, MemoryHookReport,
    MemoryProactiveReport, MemoryProjectReport, MemoryReflectionReport, MemorySleepReport,
    OperatorReviewReport, RecordLedgerIssueKind, ReviewResolutionReport, SnapshotIssueKind,
};

pub(crate) fn issue_kind_name(kind: &RecordLedgerIssueKind) -> &'static str {
    match kind {
        RecordLedgerIssueKind::LegacyEnvelope => "legacy_envelope",
        RecordLedgerIssueKind::ParseError => "parse_error",
        RecordLedgerIssueKind::OutOfOrderSequence => "out_of_order_sequence",
        RecordLedgerIssueKind::UnsupportedVersion => "unsupported_version",
        RecordLedgerIssueKind::ChecksumMismatch => "checksum_mismatch",
        RecordLedgerIssueKind::HashChainBroken => "hash_chain_broken",
    }
}

pub(crate) fn snapshot_issue_kind_name(kind: &SnapshotIssueKind) -> &'static str {
    match kind {
        SnapshotIssueKind::ParseError => "parse_error",
        SnapshotIssueKind::UnsupportedVersion => "unsupported_version",
        SnapshotIssueKind::ChecksumMismatch => "checksum_mismatch",
        SnapshotIssueKind::RecordLedgerMismatch => "record_ledger_mismatch",
        SnapshotIssueKind::ReplayMismatch => "replay_mismatch",
    }
}

pub(crate) fn backup_issue_kind_name(kind: &BackupIssueKind) -> &'static str {
    match kind {
        BackupIssueKind::ParseError => "parse_error",
        BackupIssueKind::UnsupportedVersion => "unsupported_version",
        BackupIssueKind::ChecksumMismatch => "checksum_mismatch",
        BackupIssueKind::RecordLedgerMismatch => "record_ledger_mismatch",
        BackupIssueKind::RecordSequenceMismatch => "record_sequence_mismatch",
        BackupIssueKind::RecordChecksumMismatch => "record_checksum_mismatch",
        BackupIssueKind::TargetNotEmpty => "target_not_empty",
        BackupIssueKind::RestoreVerificationMismatch => "restore_verification_mismatch",
    }
}

/// Shorten `text` to a single scannable line of at most `max` characters.
fn one_line(text: &str, max: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= max {
        flat
    } else {
        let head: String = flat.chars().take(max).collect();
        format!("{}\u{2026}", head.trim_end())
    }
}

pub(crate) fn print_briefing_report(report: &MemoryBriefingReport) {
    println!("{}", crate::style::heading("Session briefing"));
    println!(
        "{}",
        crate::style::store_trust_line(&report.authority.mode, report.authority.score)
    );
    // One compact counts line instead of eight stacked lines.
    println!(
        "{}",
        crate::style::dim(&format!(
            "{} events · {} episodes · {} entities · {} sources · {} health signals · {} intentions · {} review",
            report.event_count,
            report.summary.episode_count,
            report.summary.entity_count,
            report.summary.source_count,
            report.health.blind_spot_count,
            report.summary.active_intention_count,
            report.summary.high_priority_review_count,
        ))
    );

    if !report.recent_episodes.is_empty() {
        println!("\n{}", crate::style::heading("Recent episodes"));
        for episode in &report.recent_episodes {
            // One scannable line per episode, with id/mentions dimmed beneath.
            println!("  · {}", one_line(&episode.content, 96));
            let mut meta = vec![crate::style::dim(&episode.id)];
            if !episode.mentions.is_empty() {
                meta.push(crate::style::dim(&format!(
                    "@{}",
                    episode.mentions.join(", ")
                )));
            }
            println!("    {}", meta.join("  "));
        }
    }

    if !report.active_intentions.is_empty() {
        println!("\n{}", crate::style::heading("Active intentions"));
        for intention in &report.active_intentions {
            println!(
                "  · [{:?}] {}",
                intention.priority,
                one_line(&intention.description, 88)
            );
        }
    }

    if !report.review_items.is_empty() {
        println!("\n{}", crate::style::heading("Review items"));
        for item in &report.review_items {
            println!("  · [{:?}] {}", item.priority, one_line(&item.title, 88));
            println!(
                "    {}",
                crate::style::dim(&one_line(&item.operator_guidance, 96))
            );
        }
    }

    if !report.graph_seeds.is_empty() {
        let seeds = report
            .graph_seeds
            .iter()
            .map(|seed| format!("{} (×{})", seed.label, seed.mention_count))
            .collect::<Vec<_>>()
            .join(" · ");
        println!(
            "\n{}  {}",
            crate::style::heading("Graph seeds"),
            crate::style::dim(&seeds)
        );
    }
}

pub(crate) fn print_reflection_report(report: &MemoryReflectionReport) {
    println!("Reflection cycle");
    println!("Events: {}", report.event_count);
    println!(
        "{}",
        crate::style::store_trust_line(&report.authority.mode, report.authority.score)
    );
    println!("Findings: {}", report.summary.finding_count);
    println!("Cycles: {}", report.summary.displayed_cycle_count);
    println!(
        "Source coverage: {:.2}",
        report.source_coverage.source_coverage_ratio
    );
    println!(
        "Evidence coverage: {:.2}",
        report.source_coverage.evidence_coverage_ratio
    );
    println!(
        "Automatic write-back: {}",
        report.write_back_policy.automatic_write_back
    );

    if report.cycles.is_empty() {
        println!("No reflection cycles are pending.");
        return;
    }

    println!("Cycles:");
    for cycle in &report.cycles {
        println!(
            "- [{:?}] {} ({:?})",
            cycle.priority, cycle.title, cycle.action
        );
        println!("  {}", cycle.rationale);
        if !cycle.evidence_ids.is_empty() {
            println!("  evidence: {}", cycle.evidence_ids.join(", "));
        }
        for finding in &cycle.findings {
            println!("  - [{:?}] {}", finding.severity, finding.title);
            println!("    {}", finding.detail);
        }
    }
}

pub(crate) fn print_sleep_report(report: &MemorySleepReport) {
    println!("Sleep Mode");
    println!("Events: {}", report.event_count);
    println!(
        "{}",
        crate::style::store_trust_line(&report.authority.mode, report.authority.score)
    );
    println!(
        "Recent episodes replayed: {}",
        report.summary.replayed_episode_count
    );
    println!("Stages: {}", report.stages.len());
    println!("Pending stages: {}", report.summary.pending_stage_count);
    println!(
        "Consolidation candidates: {}",
        report.summary.consolidation_candidate_count
    );
    println!("Review items: {}", report.summary.review_item_count);
    println!(
        "Automatic write-back: {}",
        report.summary.automatic_write_back
    );

    if !report.stages.is_empty() {
        println!("Stages:");
        for stage in &report.stages {
            println!("- [{:?}] {}", stage.status, stage.title);
            println!("  {}", stage.detail);
            if !stage.evidence_ids.is_empty() {
                println!("  evidence: {}", stage.evidence_ids.join(", "));
            }
        }
    }

    if !report.recent_episodes.is_empty() {
        println!("Recent episodes:");
        for episode in &report.recent_episodes {
            println!("- {} {}", episode.id, episode.content);
            if !episode.tags.is_empty() {
                println!("  tags: {}", episode.tags.join(", "));
            }
            if !episode.mentions.is_empty() {
                println!("  mentions: {}", episode.mentions.join(", "));
            }
        }
    }

    if report.consolidation_candidates.is_empty() {
        println!("No consolidation candidates are pending.");
        return;
    }

    println!("Consolidation candidates:");
    for candidate in &report.consolidation_candidates {
        println!(
            "- [{:?}] {} ({:?})",
            candidate.priority, candidate.title, candidate.kind
        );
        println!("  {}", candidate.rationale);
        if !candidate.evidence_ids.is_empty() {
            println!("  evidence: {}", candidate.evidence_ids.join(", "));
        }
    }
}

pub(crate) fn print_consolidation_plan_report(report: &MemoryConsolidationPlanReport) {
    println!("Consolidation plan");
    println!("Events: {}", report.event_count);
    println!(
        "{}",
        crate::style::store_trust_line(&report.authority.mode, report.authority.score)
    );
    println!("Stages: {}", report.summary.stage_count);
    println!("Operations: {}", report.summary.operation_count);
    println!(
        "Replay operations: {}",
        report.summary.replay_operation_count
    );
    println!(
        "Extraction candidates: {}",
        report.summary.extract_candidate_count
    );
    println!("Review gates: {}", report.summary.review_gate_count);
    println!(
        "Needs review: {}",
        report.summary.needs_review_operation_count
    );
    println!("Blocked: {}", report.summary.blocked_operation_count);
    println!(
        "Automatic write-back: {}",
        report.summary.automatic_write_back
    );

    if !report.stages.is_empty() {
        println!("Stages:");
        for stage in &report.stages {
            println!("- [{:?}] {}", stage.status, stage.title);
            println!("  {}", stage.detail);
            if !stage.operation_ids.is_empty() {
                println!("  operations: {}", stage.operation_ids.join(", "));
            }
        }
    }

    if report.operations.is_empty() {
        println!("No consolidation operations are pending.");
        return;
    }

    println!("Operations:");
    for operation in &report.operations {
        println!(
            "- [{:?}] {} ({:?})",
            operation.status, operation.title, operation.kind
        );
        println!("  {}", operation.rationale);
        println!("  gate: {}", operation.gate.reason);
        if !operation.evidence_ids.is_empty() {
            println!("  evidence: {}", operation.evidence_ids.join(", "));
        }
    }
}

pub(crate) fn print_hook_report(report: &MemoryHookReport) {
    println!("Memory hook");
    println!("Kind: {:?}", report.kind);
    println!("Events: {}", report.event_count);
    println!(
        "{}",
        crate::style::store_trust_line(&report.authority.mode, report.authority.score)
    );
    println!("Recall results: {}", report.summary.recall_count);
    println!("Review items: {}", report.summary.review_item_count);
    println!(
        "Reflection cycles: {}",
        report.summary.reflection_cycle_count
    );
    println!(
        "Self-inspection findings: {}",
        report.summary.self_inspection_finding_count
    );
    println!(
        "Automatic write-back: {}",
        report.summary.automatic_write_back
    );
    println!(
        "Pause for review: {}",
        report.summary.should_pause_for_review
    );

    if !report.directives.is_empty() {
        println!("Directives:");
        for directive in &report.directives {
            println!(
                "- [{:?}] {}: {}",
                directive.priority, directive.title, directive.detail
            );
            if !directive.evidence_ids.is_empty() {
                println!("  evidence: {}", directive.evidence_ids.join(", "));
            }
        }
    }

    if let Some(recall) = &report.recall {
        print_recall_section(&recall.results);
    }
}

pub(crate) fn print_ingestion_report(
    database: &Path,
    path: &Path,
    dry_run: bool,
    report: &IngestionReport,
) {
    println!("Database: {}", database.display());
    println!("Ingest document: {}", path.display());
    println!(
        "Status: {}",
        if report.valid {
            if dry_run { "dry-run" } else { "ingested" }
        } else {
            "invalid"
        }
    );
    println!("Events: {}", report.appendable_event_count);
    println!("Ingested events: {}", report.ingested_event_count);
    println!("Sources: {}", report.counts.sources);
    println!("Episodes: {}", report.counts.episodes);
    println!("Claims: {}", report.counts.claims);
    println!("Links: {}", report.counts.links);
    println!("Procedures: {}", report.counts.procedures);
    println!("Intentions: {}", report.counts.intentions);
    print_ingestion_preflight(report);
    if let Some(source_id) = &report.source_id {
        println!("Source: {source_id}");
    }
    if !report.episode_ids.is_empty() {
        println!("Episode IDs: {}", report.episode_ids.join(", "));
    }
    if !report.issues.is_empty() {
        println!("Issues:");
        for issue in &report.issues {
            println!("- {}: {}", issue.path, issue.message);
        }
    }
}

pub(crate) fn print_ingestion_preflight(report: &IngestionReport) {
    let source_scope = report
        .preflight
        .source_scope
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| "none".to_string());

    println!("Scope: {source_scope}");
    println!("Source bytes: {}", report.preflight.source_byte_len);
    println!("Derived records: {}", report.preflight.derived_record_count);
    println!(
        "Evidence-linked records: {}",
        report.preflight.evidence_linked_record_count
    );
    println!("Evidence gaps: {}", report.preflight.evidence_gap_count);
    println!(
        "Referenced episodes: {}",
        report.preflight.referenced_episode_count
    );
    println!(
        "Unreferenced episodes: {}",
        report.preflight.unreferenced_episode_count
    );
}

pub(crate) fn print_interchange_import_preflight(report: &InterchangeImportReport) {
    println!("Episode bytes: {}", report.preflight.episode_byte_len);
    println!("Derived records: {}", report.preflight.derived_record_count);
    println!(
        "Evidence-linked records: {}",
        report.preflight.evidence_linked_record_count
    );
    println!("Evidence gaps: {}", report.preflight.evidence_gap_count);
    println!(
        "Referenced episodes: {}",
        report.preflight.referenced_episode_count
    );
    println!(
        "Unreferenced episodes: {}",
        report.preflight.unreferenced_episode_count
    );
    println!("Scoped records: {}", report.preflight.scoped_record_count);
    println!(
        "Unscoped records: {}",
        report.preflight.unscoped_record_count
    );
    if !report.preflight.scope_keys.is_empty() {
        println!("Scopes: {}", report.preflight.scope_keys.join(", "));
    }
    println!(
        "Readiness findings: {}",
        report.readiness.self_inspection_summary.finding_count
    );
    println!(
        "Readiness source coverage findings: {}",
        report
            .readiness
            .self_inspection_summary
            .source_coverage_count
    );
    println!(
        "Readiness review items: {}",
        report.readiness.review_item_count
    );
}

fn print_recall_section(results: &[nahuali_core::RecallResult]) {
    if results.is_empty() {
        return;
    }

    println!("Recalled memory:");
    for result in results {
        println!(
            "- [{:?}] {:.2} {}",
            result.kind, result.score, result.excerpt
        );
        if let Some(trust) = &result.trust {
            println!(
                "  confianza: {} (score {:.2})",
                crate::style::trust_badge(&trust.mode),
                trust.score
            );
        }
        if let Some(evidence_id) = &result.evidence_id {
            println!("  evidence: {evidence_id}");
        }
    }
}

pub(crate) fn print_operator_review(report: &OperatorReviewReport) {
    println!("Operator review");
    println!("Events: {}", report.event_count);
    println!(
        "{}",
        crate::style::store_trust_line(&report.authority.mode, report.authority.score)
    );
    println!("Total items: {}", report.total_items);
    println!("Displayed items: {}", report.displayed_items);
    println!("Critical: {}", report.summary.critical_count);
    println!("High: {}", report.summary.high_count);
    println!("Medium: {}", report.summary.medium_count);
    println!("Low: {}", report.summary.low_count);

    if report.items.is_empty() {
        println!("No immediate operator work is pending.");
        return;
    }

    println!("Queue:");
    for (index, item) in report.items.iter().enumerate() {
        println!(
            "{}. [{:?}] {} ({:?})",
            index + 1,
            item.priority,
            item.title,
            item.action
        );
        println!("   {}", item.detail);
        println!("   Next action: {}", item.operator_guidance);
        if !item.evidence_ids.is_empty() {
            println!("   Evidence: {}", item.evidence_ids.join(", "));
        }
    }
}

pub(crate) fn print_review_resolution(report: &ReviewResolutionReport) {
    println!("Review resolution");
    println!("Review item: {}", report.review_id);
    println!("Finding: {}", report.finding_id);
    println!("Outcome: {:?}", report.outcome);
    println!(
        "Status: {}",
        if report.applied { "applied" } else { "dry-run" }
    );
    println!("Note: {}", report.note);
    println!("Policy: {}", report.policy);
    if let Some(decision_id) = &report.decision_id {
        println!("Decision: {decision_id}");
    }
    if let Some(event_id) = &report.event_id {
        println!("Event: {event_id}");
    }
    if !report.evidence_ids.is_empty() {
        println!("Evidence: {}", report.evidence_ids.join(", "));
    }
}

pub(crate) fn print_graph_report(report: &MemoryGraphReport) {
    println!("Memory graph");
    println!("Seed: {}", report.seed);
    println!("Events: {}", report.event_count);
    println!(
        "{}",
        crate::style::store_trust_line(&report.authority.mode, report.authority.score)
    );
    println!("Nodes: {}", report.summary.node_count);
    println!("Edges: {}", report.summary.edge_count);
    println!("Health signals: {}", report.summary.health_signal_count);
    println!("Review decisions: {}", report.summary.review_decision_count);

    if report.nodes.is_empty() {
        println!("No graph nodes matched.");
        return;
    }

    println!("Nodes:");
    for node in &report.nodes {
        println!(
            "- depth={} [{:?}] {} {}",
            node.depth, node.kind, node.id, node.label
        );
        if !node.evidence_ids.is_empty() {
            println!("  evidence: {}", node.evidence_ids.join(", "));
        }
    }

    if !report.edges.is_empty() {
        println!("Edges:");
        for edge in &report.edges {
            println!(
                "- [{:?}] {} -> {} ({})",
                edge.kind, edge.from, edge.to, edge.label
            );
        }
    }
}

pub(crate) fn print_project_report(report: &MemoryProjectReport) {
    println!("Project view");
    println!("Query: {}", report.query);
    println!("Events: {}", report.event_count);
    println!(
        "{}",
        crate::style::store_trust_line(&report.authority.mode, report.authority.score)
    );
    println!("Health signals: {}", report.health.signals.len());
    match &report.matched_entity {
        Some(entity) => {
            println!(
                "Entity: {} mentions={} first_seen={} last_seen={}",
                entity.name, entity.mention_count, entity.first_seen_at_ms, entity.last_seen_at_ms
            );
        }
        None => println!("Entity: no projected entity matched"),
    }
    println!(
        "Graph: {} nodes, {} edges",
        report.summary.graph_node_count, report.summary.graph_edge_count
    );
    println!(
        "Memory: {} episodes, {} claims, {} links, {} procedures, {} intentions",
        report.summary.episode_count,
        report.summary.claim_count,
        report.summary.link_count,
        report.summary.procedure_count,
        report.summary.intention_count
    );
    println!("Review items: {}", report.summary.review_item_count);

    if !report.recall_results.is_empty() {
        println!("Recall:");
        for result in &report.recall_results {
            println!(
                "- [{:?}] {:.2} {}",
                result.kind, result.score, result.excerpt
            );
            if let Some(trust) = &result.trust {
                println!(
                    "  confianza: {} (score {:.2})",
                    crate::style::trust_badge(&trust.mode),
                    trust.score
                );
            }
            if let Some(evidence_id) = &result.evidence_id {
                println!("  evidence: {evidence_id}");
            }
        }
    }

    if !report.claims.is_empty() {
        println!("Claims:");
        for claim in &report.claims {
            println!(
                "- {:.2} {} {} {}",
                claim.confidence, claim.subject, claim.predicate, claim.object
            );
            if let Some(source_episode_id) = &claim.source_episode_id {
                println!("  evidence: {source_episode_id}");
            }
        }
    }

    if !report.links.is_empty() {
        println!("Links:");
        for link in &report.links {
            println!(
                "- {:.2} {} {} {}",
                link.confidence, link.from, link.relation, link.to
            );
            if let Some(source_episode_id) = &link.source_episode_id {
                println!("  evidence: {source_episode_id}");
            }
        }
    }

    if !report.episodes.is_empty() {
        println!("Recent episodes:");
        for episode in &report.episodes {
            println!("- {} {}", episode.id, episode.content);
            if !episode.mentions.is_empty() {
                println!("  mentions: {}", episode.mentions.join(", "));
            }
            if !episode.tags.is_empty() {
                println!("  tags: {}", episode.tags.join(", "));
            }
        }
    }

    if !report.procedures.is_empty() {
        println!("Procedures:");
        for procedure in &report.procedures {
            println!(
                "- [{:?}] {}: {}",
                procedure.kind, procedure.name, procedure.body
            );
            if let Some(source_episode_id) = &procedure.source_episode_id {
                println!("  evidence: {source_episode_id}");
            }
        }
    }

    if !report.intentions.is_empty() {
        println!("Intentions:");
        for intention in &report.intentions {
            println!(
                "- [{:?}/{:?}] {}",
                intention.priority, intention.status, intention.description
            );
            if let Some(source_episode_id) = &intention.source_episode_id {
                println!("  evidence: {source_episode_id}");
            }
        }
    }

    if !report.review_items.is_empty() {
        println!("Review items:");
        for item in &report.review_items {
            println!("- [{:?}] {} ({:?})", item.priority, item.title, item.action);
            println!("  {}", item.operator_guidance);
            if !item.evidence_ids.is_empty() {
                println!("  evidence: {}", item.evidence_ids.join(", "));
            }
        }
    }

    if report.recall_results.is_empty()
        && report.episodes.is_empty()
        && report.claims.is_empty()
        && report.links.is_empty()
        && report.procedures.is_empty()
        && report.intentions.is_empty()
    {
        println!("No focused memory matched this query.");
    }
}

pub(crate) fn print_intention_reconciliation_report(report: &IntentionReconciliationReport) {
    println!("Intention reconciliation");
    println!("Intentions: {}", report.intention_count);
    println!("Issues: {}", report.issue_count);
    println!("Generated at: {}", report.generated_at_ms);

    if report.issues.is_empty() {
        println!("No intention reconciliation issues are pending.");
        return;
    }

    println!("Issues:");
    for issue in &report.issues {
        println!(
            "- [{:?}] {:?} {}",
            issue.priority, issue.kind, issue.intention_id
        );
        println!("  {}", issue.detail);
        if !issue.evidence_ids.is_empty() {
            println!("  evidence: {}", issue.evidence_ids.join(", "));
        }
    }
}

pub(crate) fn print_goal_progress_report(report: &GoalProgressReport) {
    println!("Goal progress");
    println!("Goals: {}", report.goal_count);
    println!("Generated at: {}", report.generated_at_ms);

    if report.goals.is_empty() {
        println!("No goal intentions are recorded.");
        return;
    }

    println!("Goals:");
    for goal in &report.goals {
        println!(
            "- [{:?}] {} derived={}%",
            goal.status, goal.description, goal.derived_progress_percent
        );
        println!(
            "  children: {} completed={} active={} blocked={} deferred={} abandoned={}",
            goal.child_count,
            goal.completed_count,
            goal.active_count,
            goal.blocked_count,
            goal.deferred_count,
            goal.abandoned_count
        );
        if let Some(progress) = goal.explicit_progress_percent {
            println!("  explicit progress: {progress}%");
        }
        if !goal.child_ids.is_empty() {
            println!("  child ids: {}", goal.child_ids.join(", "));
        }
    }
}

pub(crate) fn print_proactive_report(report: &MemoryProactiveReport) {
    println!("Proactive operator report");
    println!("Events: {}", report.event_count);
    println!(
        "{}",
        crate::style::store_trust_line(&report.authority.mode, report.authority.score)
    );
    println!("Deadlines: {}", report.summary.deadline_count);
    println!(
        "Overdue deadlines: {}",
        report.summary.overdue_deadline_count
    );
    println!("Anomalies: {}", report.summary.anomaly_count);
    println!(
        "Critical anomalies: {}",
        report.summary.critical_anomaly_count
    );
    println!("High anomalies: {}", report.summary.high_anomaly_count);
    println!(
        "Capture opportunities: {}",
        report.summary.capture_opportunity_count
    );
    println!("Pause for review: {}", report.summary.should_pause);
    println!(
        "Automatic write-back: {}",
        report.write_back_policy.automatic_write_back
    );

    if !report.deadlines.deadlines.is_empty() {
        println!("Deadline signals:");
        print_deadline_rows(&report.deadlines);
    }
    if !report.anomalies.alerts.is_empty() {
        println!("Anomaly alerts:");
        print_anomaly_rows(&report.anomalies);
    }
    if !report.capture_opportunities.is_empty() {
        println!("Capture opportunities:");
        for opportunity in &report.capture_opportunities {
            println!(
                "- [{:?}] {} ({})",
                opportunity.priority, opportunity.title, opportunity.id
            );
            println!("  {}", opportunity.detail);
            if !opportunity.evidence_ids.is_empty() {
                println!("  evidence: {}", opportunity.evidence_ids.join(", "));
            }
        }
    }
}

pub(crate) fn print_deadline_report(report: &DeadlineReport) {
    println!("Deadline signals");
    println!("Generated at: {}", report.generated_at_ms);
    println!("Horizon ms: {}", report.horizon_ms);
    println!("Deadlines: {}", report.summary.deadline_count);
    println!("Overdue: {}", report.summary.overdue_count);
    println!("Due soon: {}", report.summary.due_soon_count);
    println!("Scheduled: {}", report.summary.scheduled_count);

    if report.deadlines.is_empty() {
        println!("No deadline signals are pending.");
        return;
    }

    print_deadline_rows(report);
}

pub(crate) fn print_anomaly_report(report: &AnomalyReport) {
    println!("Anomaly alerts");
    println!("Generated at: {}", report.generated_at_ms);
    println!("Alerts: {}", report.alert_count);
    println!("Critical: {}", report.summary.critical_count);
    println!("High: {}", report.summary.high_count);
    println!("Medium: {}", report.summary.medium_count);
    println!("Low: {}", report.summary.low_count);

    if report.alerts.is_empty() {
        println!("No anomaly alerts are pending.");
        return;
    }

    print_anomaly_rows(report);
}

pub(crate) fn print_anomaly_acknowledgement(report: &AnomalyAcknowledgementReport) {
    println!("Anomaly acknowledgement");
    println!("Alert: {}", report.anomaly_id);
    println!(
        "Status: {}",
        if report.applied { "applied" } else { "dry-run" }
    );
    println!("Note: {}", report.note);
    println!("Policy: {}", report.policy);
    if let Some(decision_id) = &report.decision_id {
        println!("Decision: {decision_id}");
    }
    if let Some(event_id) = &report.event_id {
        println!("Event: {event_id}");
    }
    if !report.evidence_ids.is_empty() {
        println!("Evidence: {}", report.evidence_ids.join(", "));
    }
}

fn print_deadline_rows(report: &DeadlineReport) {
    for deadline in &report.deadlines {
        println!(
            "- [{:?}] {:?} {} {}",
            deadline.priority, deadline.state, deadline.intention_id, deadline.description
        );
        println!("  {}", deadline.detail);
        if !deadline.evidence_ids.is_empty() {
            println!("  evidence: {}", deadline.evidence_ids.join(", "));
        }
    }
}

fn print_anomaly_rows(report: &AnomalyReport) {
    for alert in &report.alerts {
        println!(
            "- [{:?}] {:?} {} ({})",
            alert.priority, alert.kind, alert.title, alert.id
        );
        println!("  {}", alert.detail);
        println!("  next: {}", alert.suggested_action);
        if !alert.evidence_ids.is_empty() {
            println!("  evidence: {}", alert.evidence_ids.join(", "));
        }
    }
}
