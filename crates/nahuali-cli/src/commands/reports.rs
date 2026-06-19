use nahuali_core::{
    BriefingOptions, ConsolidationPlanOptions, IntentionReconciliationOptions, MemoryEngine,
    MemoryHookOptions, MemoryKind, OperatorReviewOptions, ProactiveOptions, ProjectViewOptions,
    RecallOptions, ReflectionOptions, SelfInspectionReviewAction, SelfInspectionReviewPriority,
    SleepModeOptions,
};

use crate::commands::scope::parse_scope;
use crate::{
    cli::{CliMemoryHookKind, CliRecallKind, CliReviewAction, CliReviewPriority},
    output,
};

pub(crate) fn status(
    memory: &mut MemoryEngine,
    database: &std::path::Path,
    json: bool,
) -> anyhow::Result<()> {
    let health = memory.inspect();
    let authority = memory.authority();
    let projection = memory.projection_validate()?;
    let data = memory.data();
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "database": database.display().to_string(),
                "event_count": data.event_count,
                "source_count": data.sources.len(),
                "entity_count": data.entities.len(),
                "episode_count": data.episodes.len(),
                "claim_count": data.claims.len(),
                "link_count": data.links.len(),
                "procedure_count": data.procedures.len(),
                "intention_count": data.intentions.len(),
                "review_decision_count": data.review_decisions.len(),
                "authority": authority,
                "health": health,
                "surrealdb_graph_projection": projection,
                "semantic_index_role": "derived",
            }))?
        );
    } else {
        println!("Database: {}", database.display());
        println!("Events: {}", data.event_count);
        println!("Sources: {}", data.sources.len());
        println!("Entities: {}", data.entities.len());
        println!("Episodes: {}", data.episodes.len());
        println!("Claims: {}", data.claims.len());
        println!("Links: {}", data.links.len());
        println!("Procedures: {}", data.procedures.len());
        println!("Intentions: {}", data.intentions.len());
        println!("Review decisions: {}", data.review_decisions.len());
        println!(
            "{}",
            crate::style::store_trust_line(&authority.mode, authority.score)
        );
        println!("Health signals: {}", health.signals.len());
        println!(
            "SurrealDB graph projection: {}",
            if projection.valid { "valid" } else { "invalid" }
        );
        println!("Semantic index: derived");
    }
    Ok(())
}

pub(crate) fn briefing(
    memory: &mut MemoryEngine,
    episode_limit: usize,
    intention_limit: usize,
    review_limit: usize,
    graph_seed_limit: usize,
    json: bool,
    database: &std::path::Path,
) -> anyhow::Result<()> {
    let report = memory.briefing_with_options(BriefingOptions {
        episode_limit,
        intention_limit,
        review_limit,
        graph_seed_limit,
    });
    if json {
        let mut envelope = serde_json::json!({
            "database": database.display().to_string(),
            "semantic_index_role": "derived",
            "source_projection": "rust",
            "report": report,
        });
        // Mirror the human-mode archive hint: agents reading the JSON briefing
        // should also learn that a low-authority historical reference exists.
        if let Ok(archive_db) = std::env::var("NAHUALI_ARCHIVE_DB")
            && !archive_db.trim().is_empty()
        {
            envelope["archive"] = serde_json::json!({
                "database": archive_db,
                "role": "historical_reference",
                "authority": "reference_only_unverified",
                "hint": "recall <topic> --archive",
            });
        }
        println!("{}", serde_json::to_string_pretty(&envelope)?);
    } else {
        output::print_briefing_report(&report);
        // When a read-only archive is configured, surface it so an agent whose
        // canonical briefing is thin on a topic knows historical reference
        // exists (e.g. after a fresh-start migration). The archive is never
        // authority; it is consulted on demand and clearly labeled.
        if let Ok(archive_db) = std::env::var("NAHUALI_ARCHIVE_DB")
            && !archive_db.trim().is_empty()
        {
            println!(
                "\n{}",
                crate::style::dim(&format!(
                    "Archive: {archive_db} available as historical reference — `nahuali recall <topic> --archive`"
                ))
            );
        }
    }
    Ok(())
}

pub(crate) struct HookArgs {
    pub(crate) kind: CliMemoryHookKind,
    pub(crate) input: Option<String>,
    pub(crate) recall_limit: usize,
    pub(crate) episode_limit: usize,
    pub(crate) intention_limit: usize,
    pub(crate) review_limit: usize,
    pub(crate) graph_seed_limit: usize,
    pub(crate) cycle_limit: usize,
    pub(crate) evidence_limit: usize,
}

pub(crate) struct ConsolidationPlanArgs {
    pub(crate) episode_limit: usize,
    pub(crate) candidate_limit: usize,
    pub(crate) cycle_limit: usize,
    pub(crate) evidence_limit: usize,
    pub(crate) review_limit: usize,
}

pub(crate) struct RecallArgs {
    pub(crate) query: Vec<String>,
    pub(crate) limit: usize,
    pub(crate) authority: bool,
    pub(crate) semantic: bool,
    pub(crate) scope: Option<String>,
    pub(crate) kinds: Vec<CliRecallKind>,
    pub(crate) require_evidence: bool,
    pub(crate) as_of_ms: Option<u64>,
    pub(crate) max_age_days: Option<u64>,
    pub(crate) archive: bool,
    pub(crate) json: bool,
}

pub(crate) struct ProjectArgs {
    pub(crate) entity: Vec<String>,
    pub(crate) graph_depth: usize,
    pub(crate) graph_limit: usize,
    pub(crate) item_limit: usize,
    pub(crate) recall_limit: usize,
    pub(crate) review_limit: usize,
}

pub(crate) struct ProactiveArgs {
    pub(crate) now_ms: Option<u64>,
    pub(crate) deadline_horizon_ms: u64,
    pub(crate) stale_after_ms: u64,
    pub(crate) review_limit: usize,
}

pub(crate) fn sleep(
    memory: &mut MemoryEngine,
    database: &std::path::Path,
    episode_limit: usize,
    candidate_limit: usize,
    cycle_limit: usize,
    evidence_limit: usize,
    json: bool,
) -> anyhow::Result<()> {
    let report = memory.sleep_with_options(SleepModeOptions {
        recent_episode_limit: episode_limit,
        candidate_limit,
        reflection: ReflectionOptions {
            cycle_limit,
            evidence_limit,
        },
    });

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "database": database.display().to_string(),
                "report": report,
            }))?
        );
    } else {
        output::print_sleep_report(&report);
    }
    Ok(())
}

pub(crate) fn consolidation_plan(
    memory: &mut MemoryEngine,
    database: &std::path::Path,
    args: ConsolidationPlanArgs,
    json: bool,
) -> anyhow::Result<()> {
    let report = memory.consolidation_plan_with_options(ConsolidationPlanOptions {
        recent_episode_limit: args.episode_limit,
        candidate_limit: args.candidate_limit,
        cycle_limit: args.cycle_limit,
        evidence_limit: args.evidence_limit,
        review_limit: args.review_limit,
    });

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "database": database.display().to_string(),
                "report": report,
            }))?
        );
    } else {
        output::print_consolidation_plan_report(&report);
    }
    Ok(())
}

pub(crate) fn hook(
    memory: &mut MemoryEngine,
    database: &std::path::Path,
    args: HookArgs,
    json: bool,
) -> anyhow::Result<()> {
    let report = memory.run_hook_with_options(MemoryHookOptions {
        kind: args.kind.into(),
        input: args.input,
        recall_limit: args.recall_limit,
        briefing: BriefingOptions {
            episode_limit: args.episode_limit,
            intention_limit: args.intention_limit,
            review_limit: args.review_limit,
            graph_seed_limit: args.graph_seed_limit,
        },
        reflection: ReflectionOptions {
            cycle_limit: args.cycle_limit,
            evidence_limit: args.evidence_limit,
        },
        sleep: SleepModeOptions {
            recent_episode_limit: args.episode_limit,
            candidate_limit: args.cycle_limit,
            reflection: ReflectionOptions {
                cycle_limit: args.cycle_limit,
                evidence_limit: args.evidence_limit,
            },
        },
    })?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "database": database.display().to_string(),
                "report": report,
            }))?
        );
    } else {
        output::print_hook_report(&report);
    }
    Ok(())
}

pub(crate) fn recall(memory: &mut MemoryEngine, args: RecallArgs) -> anyhow::Result<()> {
    let query = args.query.join(" ");
    let scope = parse_scope(args.scope)?;
    let kinds = args
        .kinds
        .into_iter()
        .map(MemoryKind::from)
        .collect::<Vec<_>>();
    let as_of_ms = args.as_of_ms;
    // "Older than N days" is resolved to an inclusive lower bound at query time.
    let since_ms = args.max_age_days.map(|days| {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis() as u64)
            .unwrap_or(0);
        now_ms.saturating_sub(days.saturating_mul(24 * 60 * 60 * 1000))
    });
    let archive = args.archive;
    let options = RecallOptions {
        limit: args.limit,
        scope,
        kinds,
        require_evidence: args.require_evidence,
        as_of_ms,
        since_ms,
    };

    if args.semantic {
        let recall = memory.hybrid_recall_with_options(&query, options.clone())?;
        if args.json {
            if archive {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "recall": recall,
                        "archive": archive_recall_json(&query, options),
                    }))?
                );
            } else {
                println!("{}", serde_json::to_string_pretty(&recall)?);
            }
            return Ok(());
        }
        println!(
            "{}",
            crate::style::store_trust_line(&recall.authority.mode, recall.authority.score)
        );
        println!("Semantic collection: {}", recall.collection_name);
        println!(
            "Embedding: {} dimensions={}",
            recall.embedding.model, recall.embedding.dimensions
        );
        print_hybrid_recall_results(recall.results);
        print_archive_recall(&query, options, archive);
        return Ok(());
    }

    if args.authority {
        let recall = memory.recall_with_authority_options(&query, options.clone())?;
        if args.json {
            if archive {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "recall": recall,
                        "archive": archive_recall_json(&query, options),
                    }))?
                );
            } else {
                println!("{}", serde_json::to_string_pretty(&recall)?);
            }
            return Ok(());
        }
        println!(
            "{}",
            crate::style::store_trust_line(&recall.authority.mode, recall.authority.score)
        );
        for reason in &recall.authority.reasons {
            println!("- {reason}");
        }
        print_recall_results(recall.results);
        print_archive_recall(&query, options, archive);
        return Ok(());
    }

    if args.json {
        let results = memory.recall_with_options(&query, options.clone())?;
        if archive {
            output::print_json(&serde_json::json!({
                "recall": results,
                "archive": archive_recall_json(&query, options),
            }))?;
        } else {
            output::print_json(&results)?;
        }
        return Ok(());
    }

    // Human mode: enrich results with per-result trust so the trust layer is
    // visible by default (without `--authority`). The `--json` path above is
    // left untouched so its bytes stay identical.
    let recall = memory.recall_with_authority_options(&query, options.clone())?;
    print_recall_results(recall.results);
    print_archive_recall(&query, options, archive);
    Ok(())
}

/// JSON twin of [`print_archive_recall`]: the archive section for
/// `recall --archive --json`. Same semantics — read-only second engine, always
/// lexical, never fatal — expressed as a status object so agents can branch on
/// it. Only emitted when `--archive` is set, so the non-archive JSON bytes stay
/// byte-identical to the pre-archive contract.
fn archive_recall_json(query: &str, options: RecallOptions) -> serde_json::Value {
    let archive_db =
        std::env::var("NAHUALI_ARCHIVE_DB").unwrap_or_else(|_| "ts-archive".to_string());
    let mut section = serde_json::json!({
        "database": archive_db,
        "authority": "reference_only_unverified",
    });
    let engine = match MemoryEngine::open(std::path::Path::new(&archive_db)) {
        Ok(engine) => engine,
        Err(_) => {
            section["status"] = serde_json::Value::from("unavailable");
            return section;
        }
    };
    match engine.recall_with_authority_options(query, options) {
        Ok(recall) => {
            section["status"] = serde_json::Value::from("ok");
            section["results"] =
                serde_json::to_value(recall.results).unwrap_or(serde_json::Value::Null);
        }
        Err(_) => {
            section["status"] = serde_json::Value::from("query_failed");
        }
    }
    section
}

/// Federated read-only archive recall. When `--archive` is set, also query the
/// configured archive store (`$NAHUALI_ARCHIVE_DB`, default `ts-archive`) and
/// print its hits in a clearly-separated, low-authority "reference" section.
/// The archive is opened in a second engine and never written to — the canonical
/// store is untouched, so its trust genesis stays clean. Always lexical (the
/// archive has no semantic index), and never fatal: an unavailable archive is
/// noted, not raised.
fn print_archive_recall(query: &str, options: RecallOptions, enabled: bool) {
    if !enabled {
        return;
    }
    let archive_db =
        std::env::var("NAHUALI_ARCHIVE_DB").unwrap_or_else(|_| "ts-archive".to_string());

    println!();
    println!(
        "{}",
        crate::style::heading(&format!(
            "From archive · {archive_db} · reference only (unverified)"
        ))
    );

    let engine = match MemoryEngine::open(std::path::Path::new(&archive_db)) {
        Ok(engine) => engine,
        Err(_) => {
            println!(
                "{}",
                crate::style::dim(&format!("  archive \"{archive_db}\" unavailable — skipped"))
            );
            return;
        }
    };
    match engine.recall_with_authority_options(query, options) {
        Ok(recall) if recall.results.is_empty() => {
            println!("{}", crate::style::dim("  no archive matches"));
        }
        Ok(recall) => print_recall_results(recall.results),
        Err(_) => println!(
            "{}",
            crate::style::dim(&format!(
                "  archive \"{archive_db}\" query failed — skipped"
            ))
        ),
    }
}

pub(crate) fn graph(
    memory: &mut MemoryEngine,
    seed: Vec<String>,
    depth: usize,
    limit: usize,
    json: bool,
) -> anyhow::Result<()> {
    let report = memory.graph_neighborhood(&seed.join(" "), depth, limit)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        output::print_graph_report(&report);
    }
    Ok(())
}

pub(crate) fn project(
    memory: &mut MemoryEngine,
    database: &std::path::Path,
    args: ProjectArgs,
    json: bool,
) -> anyhow::Result<()> {
    let query = args.entity.join(" ");
    let report = memory.project_view_with_options(
        &query,
        ProjectViewOptions {
            graph_depth: args.graph_depth,
            graph_limit: args.graph_limit,
            item_limit: args.item_limit,
            recall_limit: args.recall_limit,
            review_limit: args.review_limit,
        },
    )?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "database": database.display().to_string(),
                "semantic_index_role": "derived",
                "source_projection": "rust",
                "report": report,
            }))?
        );
    } else {
        output::print_project_report(&report);
    }
    Ok(())
}

pub(crate) fn reconcile_intentions(
    memory: &mut MemoryEngine,
    database: &std::path::Path,
    now_ms: Option<u64>,
    stale_after_ms: u64,
    json: bool,
) -> anyhow::Result<()> {
    let mut options = IntentionReconciliationOptions {
        stale_after_ms,
        ..IntentionReconciliationOptions::default()
    };
    if let Some(now_ms) = now_ms {
        options.now_ms = now_ms;
    }
    let report = memory.reconcile_intentions_with_options(options);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "database": database.display().to_string(),
                "report": report,
            }))?
        );
    } else {
        output::print_intention_reconciliation_report(&report);
    }
    Ok(())
}

pub(crate) fn goal_progress(
    memory: &mut MemoryEngine,
    database: &std::path::Path,
    json: bool,
) -> anyhow::Result<()> {
    let report = memory.goal_progress();
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "database": database.display().to_string(),
                "report": report,
            }))?
        );
    } else {
        output::print_goal_progress_report(&report);
    }
    Ok(())
}

pub(crate) fn proactive(
    memory: &mut MemoryEngine,
    database: &std::path::Path,
    args: ProactiveArgs,
    json: bool,
) -> anyhow::Result<()> {
    let options = proactive_options(args);
    let report = memory.proactive_with_options(options);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "database": database.display().to_string(),
                "source_projection": "rust",
                "report": report,
            }))?
        );
    } else {
        output::print_proactive_report(&report);
    }
    Ok(())
}

pub(crate) fn deadlines(
    memory: &mut MemoryEngine,
    database: &std::path::Path,
    now_ms: Option<u64>,
    horizon_ms: u64,
    json: bool,
) -> anyhow::Result<()> {
    let mut options = ProactiveOptions {
        deadline_horizon_ms: horizon_ms,
        ..ProactiveOptions::default()
    };
    if let Some(now_ms) = now_ms {
        options.now_ms = now_ms;
    }
    let report = memory.deadlines_with_options(options);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "database": database.display().to_string(),
                "source_projection": "rust",
                "report": report,
            }))?
        );
    } else {
        output::print_deadline_report(&report);
    }
    Ok(())
}

pub(crate) fn anomalies(
    memory: &mut MemoryEngine,
    database: &std::path::Path,
    args: ProactiveArgs,
    json: bool,
) -> anyhow::Result<()> {
    let options = proactive_options(args);
    let report = memory.anomalies_with_options(options);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "database": database.display().to_string(),
                "source_projection": "rust",
                "report": report,
            }))?
        );
    } else {
        output::print_anomaly_report(&report);
    }
    Ok(())
}

pub(crate) fn anomaly_acknowledge(
    memory: &mut MemoryEngine,
    id: String,
    note: String,
    dry_run: bool,
    json: bool,
) -> anyhow::Result<()> {
    let report = memory.acknowledge_anomaly(id, note, dry_run)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        output::print_anomaly_acknowledgement(&report);
    }
    Ok(())
}

fn proactive_options(args: ProactiveArgs) -> ProactiveOptions {
    let mut options = ProactiveOptions {
        deadline_horizon_ms: args.deadline_horizon_ms,
        stale_after_ms: args.stale_after_ms,
        review_limit: args.review_limit,
        ..ProactiveOptions::default()
    };
    if let Some(now_ms) = args.now_ms {
        options.now_ms = now_ms;
    }
    options
}

pub(crate) fn semantic_rebuild(
    memory: &mut MemoryEngine,
    database: &std::path::Path,
    json: bool,
) -> anyhow::Result<()> {
    let report = memory.rebuild_semantic_index()?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "database": database.display().to_string(),
                "report": report,
            }))?
        );
    } else {
        println!("Database: {}", database.display());
        println!("Collection: {}", report.collection_name);
        println!("Qdrant: {}", report.qdrant_url);
        println!("Semantic index: derived");
        println!("Source projection: Rust");
        println!("Source events: {}", report.source_event_count);
        println!("Indexed points: {}", report.indexed_point_count);
        println!(
            "Deleted existing collection: {}",
            report.deleted_existing_collection
        );
        println!(
            "Embedding: {} dimensions={}",
            report.embedding.model, report.embedding.dimensions
        );
    }
    Ok(())
}

pub(crate) fn semantic_sync(
    memory: &mut MemoryEngine,
    database: &std::path::Path,
    json: bool,
) -> anyhow::Result<()> {
    let report = memory.sync_semantic_index()?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "database": database.display().to_string(),
                "report": report,
            }))?
        );
    } else {
        println!("Database: {}", database.display());
        println!("Collection: {}", report.collection_name);
        println!("Qdrant: {}", report.qdrant_url);
        println!("Semantic index: derived (non-destructive sync)");
        println!("Source events: {}", report.source_event_count);
        println!("Synced points: {}", report.indexed_point_count);
        println!(
            "Embedding: {} dimensions={}",
            report.embedding.model, report.embedding.dimensions
        );
    }
    Ok(())
}

pub(crate) fn semantic_status(
    memory: &mut MemoryEngine,
    database: &std::path::Path,
    json: bool,
) -> anyhow::Result<()> {
    let status = memory.semantic_index_status()?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "database": database.display().to_string(),
                "semantic_index_role": "derived",
                "status": status,
            }))?
        );
    } else {
        println!("Database: {}", database.display());
        println!("Collection: {}", status.collection_name);
        println!("Qdrant: {}", status.qdrant_url);
        println!("Semantic index: derived");
        println!("Collection exists: {}", status.collection_exists);
        println!("Points: {}", status.point_count);
    }
    Ok(())
}

pub(crate) fn projection_status(
    memory: &mut MemoryEngine,
    database: &std::path::Path,
    json: bool,
) -> anyhow::Result<()> {
    let status = memory.projection_status()?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "database": database.display().to_string(),
                "projection_role": "derived_from_memory_record",
                "status": status,
            }))?
        );
    } else {
        println!("Database: {}", database.display());
        println!("Projection: SurrealDB graph");
        println!("Role: derived from memory_record ledger");
        println!("Version: {}", status.projection_version);
        println!("Ledger events: {}", status.ledger_event_count);
        println!("Latest sequence: {:?}", status.latest_sequence);
        println!("Checkpoint sequence: {:?}", status.checkpoint_sequence);
        println!("In sync: {}", status.in_sync);
        println!("Tables:");
        for (table, count) in status.table_counts {
            println!("- {table}: {count}");
        }
    }
    Ok(())
}

pub(crate) fn projection_rebuild(
    memory: &mut MemoryEngine,
    database: &std::path::Path,
    json: bool,
) -> anyhow::Result<()> {
    let report = memory.projection_rebuild()?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "database": database.display().to_string(),
                "projection_role": "derived_from_memory_record",
                "report": report,
            }))?
        );
    } else {
        println!("Database: {}", database.display());
        println!("Projection: SurrealDB graph");
        println!("Role: derived from memory_record ledger");
        println!("Node rows written: {}", report.node_rows_written);
        println!("Relation rows written: {}", report.relation_rows_written);
        println!("In sync: {}", report.status.in_sync);
    }
    Ok(())
}

pub(crate) fn projection_validate(
    memory: &mut MemoryEngine,
    database: &std::path::Path,
    json: bool,
) -> anyhow::Result<()> {
    let validation = memory.projection_validate()?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "database": database.display().to_string(),
                "projection_role": "derived_from_memory_record",
                "validation": validation,
            }))?
        );
    } else {
        println!("Database: {}", database.display());
        println!("Projection: SurrealDB graph");
        println!("Role: derived from memory_record ledger");
        println!("Valid: {}", validation.valid);
        println!("In sync: {}", validation.status.in_sync);
        if validation.issues.is_empty() {
            println!("Issues: none");
        } else {
            println!("Issues:");
            for issue in validation.issues {
                println!("- {issue}");
            }
        }
    }
    Ok(())
}

pub(crate) fn projection_entities(
    memory: &mut MemoryEngine,
    database: &std::path::Path,
    query: Vec<String>,
    limit: usize,
    json: bool,
) -> anyhow::Result<()> {
    let query = query.join(" ");
    let entities = memory.projection_entities(Some(&query), limit)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "database": database.display().to_string(),
                "projection_role": "derived_from_memory_record",
                "entities": entities,
            }))?
        );
    } else {
        for entity in entities {
            println!(
                "- {} mentions={} scope={}",
                entity.name,
                entity.mention_count,
                entity.scope_key.unwrap_or_else(|| "none".to_string())
            );
        }
    }
    Ok(())
}

pub(crate) fn projection_timeline(
    memory: &mut MemoryEngine,
    database: &std::path::Path,
    limit: usize,
    json: bool,
) -> anyhow::Result<()> {
    let episodes = memory.projection_timeline(limit)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "database": database.display().to_string(),
                "projection_role": "derived_from_memory_record",
                "episodes": episodes,
            }))?
        );
    } else {
        for episode in episodes {
            println!("- {} {}", episode.memory_id, episode.content);
        }
    }
    Ok(())
}

pub(crate) fn projection_pending(
    memory: &mut MemoryEngine,
    database: &std::path::Path,
    limit: usize,
    json: bool,
) -> anyhow::Result<()> {
    let intentions = memory.projection_pending(limit)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "database": database.display().to_string(),
                "projection_role": "derived_from_memory_record",
                "intentions": intentions,
            }))?
        );
    } else {
        for intention in intentions {
            println!(
                "- [{}/{}] {}",
                intention.priority, intention.status, intention.description
            );
        }
    }
    Ok(())
}

pub(crate) fn projection_health(
    memory: &mut MemoryEngine,
    database: &std::path::Path,
    limit: usize,
    json: bool,
) -> anyhow::Result<()> {
    let signals = memory.projection_health_signals(limit)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "database": database.display().to_string(),
                "projection_role": "derived_from_memory_record",
                "signals": signals,
            }))?
        );
    } else {
        for signal in signals {
            println!(
                "- [{}] {}: {}",
                signal.severity, signal.signal_kind, signal.message
            );
        }
    }
    Ok(())
}

pub(crate) fn inspect(memory: &mut MemoryEngine, json: bool) -> anyhow::Result<()> {
    let health = memory.inspect();
    if json {
        println!("{}", serde_json::to_string_pretty(&health)?);
    } else {
        let data = memory.data();
        println!("Entities: {}", data.entities.len());
        println!("Episodes: {}", health.episode_count);
        println!("Claims: {}", data.claims.len());
        println!("Links: {}", data.links.len());
        println!("Facts: {}", health.fact_count);
        println!("Relations: {}", health.relation_count);
        println!("Procedures: {}", data.procedures.len());
        println!("Intentions: {}", data.intentions.len());
        println!("Supported facts: {}", health.supported_fact_count);
        println!("Unsupported facts: {}", health.unsupported_fact_count);
        println!("Low-confidence facts: {}", health.low_confidence_fact_count);
        println!("Conflicting facts: {}", health.conflicting_fact_count);
        println!("Stale facts: {}", health.stale_fact_count);
        println!("Isolated entities: {}", health.isolated_entity_count);
        println!("Blind spots: {}", health.blind_spot_count);
        println!(
            "Average fact confidence: {:.2}",
            health.average_fact_confidence
        );
        if health.warnings.is_empty() {
            println!("Warnings: none");
        } else {
            println!("Warnings:");
            for warning in health.warnings {
                println!("- {warning}");
            }
        }
    }
    Ok(())
}

pub(crate) fn self_inspect(memory: &mut MemoryEngine, json: bool) -> anyhow::Result<()> {
    let report = memory.self_inspect();
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("Events: {}", report.event_count);
        println!(
            "{}",
            crate::style::store_trust_line(&report.authority.mode, report.authority.score)
        );
        println!("Findings: {}", report.summary.finding_count);
        println!("Review items: {}", report.review_queue.len());
        println!(
            "Automatic write-back: {}",
            report.write_back_policy.automatic_write_back
        );
        println!(
            "Repair candidates: {} ({})",
            report.repair_signal.candidate_count, report.repair_signal.guidance
        );
        if report.findings.is_empty() {
            println!("No self-inspection findings.");
        } else {
            println!("Findings:");
            for finding in report.findings {
                println!(
                    "- [{:?}] {:?}: {}",
                    finding.kind, finding.severity, finding.title
                );
                println!("  {}", finding.detail);
                println!("  action: {}", finding.suggested_action);
                if !finding.evidence_ids.is_empty() {
                    println!("  evidence: {}", finding.evidence_ids.join(", "));
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn reflect(
    memory: &mut MemoryEngine,
    cycle_limit: usize,
    evidence_limit: usize,
    json: bool,
) -> anyhow::Result<()> {
    let report = memory.reflect_with_options(ReflectionOptions {
        cycle_limit,
        evidence_limit,
    });
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        output::print_reflection_report(&report);
    }
    Ok(())
}

pub(crate) fn review(
    memory: &mut MemoryEngine,
    limit: usize,
    min_priority: Option<CliReviewPriority>,
    action: Option<CliReviewAction>,
    json: bool,
) -> anyhow::Result<()> {
    let report = memory.operator_review_with_options(OperatorReviewOptions {
        limit,
        min_priority: min_priority.map(SelfInspectionReviewPriority::from),
        action: action.map(SelfInspectionReviewAction::from),
    });
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        output::print_operator_review(&report);
    }
    Ok(())
}

pub(crate) fn review_resolve(
    memory: &mut MemoryEngine,
    review_id: String,
    note: String,
    dry_run: bool,
    json: bool,
) -> anyhow::Result<()> {
    let report = memory.resolve_review_item(review_id, note, dry_run)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        output::print_review_resolution(&report);
    }
    Ok(())
}

pub(crate) fn maintenance(
    memory: &mut MemoryEngine,
    database: &std::path::Path,
    json: bool,
) -> anyhow::Result<()> {
    let report = memory.maintenance_report();
    if json {
        println!(
            "{}",
            serde_json::json!({
                "database": database.display().to_string(),
                "report": report,
            })
        );
    } else {
        println!("Database: {}", database.display());
        println!("Events: {}", report.event_count);
        println!("Snapshots supported: {}", report.snapshot_supported);
        println!("Snapshot recommended: {}", report.snapshot_recommended);
        println!("Compaction supported: {}", report.compaction_supported);
        println!("Compaction policy: {}", report.compaction_policy);
        if let Some(last_event_id) = &report.last_event_id {
            println!("Last event: {last_event_id}");
        }
        println!("Actions:");
        for action in report.actions {
            println!("- {action}");
        }
    }
    Ok(())
}

pub(crate) fn data(memory: &mut MemoryEngine, json: bool) -> anyhow::Result<()> {
    let data = memory.data();
    if json {
        println!("{}", serde_json::to_string_pretty(data)?);
    } else {
        println!("Events: {}", data.event_count);
        println!("Sources: {}", data.sources.len());
        println!("Entities: {}", data.entities.len());
        println!("Episodes: {}", data.episodes.len());
        println!("Claims: {}", data.claims.len());
        println!("Links: {}", data.links.len());
        println!("Procedures: {}", data.procedures.len());
        println!("Intentions: {}", data.intentions.len());
    }
    Ok(())
}

fn print_recall_results(results: Vec<nahuali_core::RecallResult>) {
    if results.is_empty() {
        println!("No memory matched.");
        return;
    }
    for result in results {
        println!(
            "- [{:?}] {} score={:.2}",
            result.kind, result.id, result.score
        );
        println!("  {}", result.excerpt);
        if let Some(evidence_id) = result.evidence_id {
            println!("  evidence: {evidence_id}");
        }
        if let Some(trust) = result.trust {
            println!(
                "  trust: {} (score {:.2})",
                crate::style::trust_badge(&trust.mode),
                trust.score
            );
            for reason in trust.reasons {
                println!("  trust reason: {reason}");
            }
        }
        if let Some(scope) = result.scope {
            println!("  scope: {}", scope.key);
        }
    }
}

fn print_hybrid_recall_results(results: Vec<nahuali_core::HybridRecallResult>) {
    if results.is_empty() {
        println!("No memory matched.");
        return;
    }
    for result in results {
        println!(
            "- [{:?}] {} score={:.2}",
            result.kind, result.id, result.score
        );
        if let Some(lexical_score) = result.lexical_score {
            println!("  lexical: {lexical_score:.2}");
        }
        if let Some(semantic_score) = result.semantic_score {
            println!("  semantic: {semantic_score:.2}");
        }
        println!("  {}", result.excerpt);
        if let Some(evidence_id) = result.evidence_id {
            println!("  evidence: {evidence_id}");
        }
        if !result.explanations.is_empty() {
            println!("  explanations: {}", result.explanations.join(", "));
        }
    }
}
