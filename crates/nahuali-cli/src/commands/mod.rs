mod artifacts;
#[cfg(feature = "attestation")]
mod attestation;
mod audit;
mod demo;
#[cfg(feature = "tui")]
mod explore;
mod init;
mod migration;
mod migration_legacy;
mod migration_timestamps;
mod migration_values;
mod preopen;
mod reconcile;
mod record;
mod reports;
mod scope;
mod text;
mod trust_report;

use std::path::PathBuf;

use anyhow::Context;
use nahuali_core::{LedgerAuditOptions, MemoryEngine, TrustReportOptions};

use crate::cli::{Cli, Command};

pub(crate) fn run(cli: Cli) -> anyhow::Result<()> {
    let verbose = cli.verbose;
    let database = cli.database.unwrap_or_else(default_database_name);
    if preopen::handle(&cli.command, &database)? {
        return Ok(());
    }

    let started = std::time::Instant::now();
    if verbose {
        verbose_open_line(&database);
    }
    let mut memory = open_store(&database, verbose)?;

    match cli.command {
        Command::Status { json } => reports::status(&mut memory, &database, json)?,
        #[cfg(feature = "tui")]
        Command::Explore => explore::explore(&mut memory, &database)?,
        Command::Reconcile { json } => reconcile::reconcile(&mut memory, &database, json)?,
        Command::Remember {
            content,
            tags,
            mentions,
            scope,
            json,
        } => record::remember(&mut memory, content, tags, mentions, scope, json)?,
        Command::Claim {
            subject,
            predicate,
            object,
            source_episode_id,
            source_last,
            confidence,
            scope,
            json,
        } => record::claim(
            &mut memory,
            record::EvidenceArgs {
                subject,
                predicate,
                object,
                source_episode_id,
                source_last,
                confidence,
                scope,
            },
            json,
        )?,
        Command::Fact {
            subject,
            predicate,
            object,
            source_episode_id,
            source_last,
            confidence,
            scope,
            json,
        } => record::fact(
            &mut memory,
            record::EvidenceArgs {
                subject,
                predicate,
                object,
                source_episode_id,
                source_last,
                confidence,
                scope,
            },
            json,
        )?,
        Command::Link {
            from,
            relation,
            to,
            source_episode_id,
            source_last,
            confidence,
            scope,
            json,
        } => record::link(
            &mut memory,
            record::LinkArgs {
                from,
                relation,
                to,
                source_episode_id,
                source_last,
                confidence,
                scope,
            },
            json,
        )?,
        Command::Relate {
            from,
            relation,
            to,
            source_episode_id,
            source_last,
            confidence,
            scope,
            json,
        } => record::relate(
            &mut memory,
            record::LinkArgs {
                from,
                relation,
                to,
                source_episode_id,
                source_last,
                confidence,
                scope,
            },
            json,
        )?,
        Command::Procedure {
            name,
            body,
            source_episode_id,
            source_last,
            confidence,
            scope,
            json,
        } => record::procedure(
            &mut memory,
            record::ProcedureArgs {
                name,
                body,
                source_episode_id,
                source_last,
                confidence,
                scope,
            },
            json,
        )?,
        Command::Preference {
            name,
            body,
            source_episode_id,
            source_last,
            confidence,
            scope,
            json,
        } => record::preference(
            &mut memory,
            record::ProcedureArgs {
                name,
                body,
                source_episode_id,
                source_last,
                confidence,
                scope,
            },
            json,
        )?,
        Command::Intention {
            description,
            kind,
            priority,
            source_episode_id,
            source_last,
            scope,
            json,
        } => record::intention(
            &mut memory,
            record::IntentionArgs {
                description,
                kind,
                priority,
                source_episode_id,
                source_last,
                scope,
            },
            json,
        )?,
        Command::IntentionStatus {
            id,
            status,
            reason,
            json,
        } => record::intention_status(&mut memory, id, status, reason, json)?,
        Command::IntentionUpdate {
            id,
            description,
            priority,
            deadline_at_ms,
            clear_deadline,
            depends_on,
            clear_dependencies,
            goal,
            clear_goal,
            progress,
            clear_progress,
            json,
        } => record::intention_update(
            &mut memory,
            record::IntentionUpdateArgs {
                id,
                description,
                priority,
                deadline_at_ms,
                clear_deadline,
                depends_on,
                clear_dependencies,
                goal,
                clear_goal,
                progress,
                clear_progress,
            },
            json,
        )?,
        Command::IntentionComplete { id, reason, json } => {
            record::intention_complete(&mut memory, id, reason, json)?
        }
        Command::IntentionBlock { id, reason, json } => {
            record::intention_block(&mut memory, id, reason, json)?
        }
        Command::IntentionDefer { id, reason, json } => {
            record::intention_defer(&mut memory, id, reason, json)?
        }
        Command::ReconcileIntentions {
            now_ms,
            stale_after_ms,
            json,
        } => reports::reconcile_intentions(&mut memory, &database, now_ms, stale_after_ms, json)?,
        Command::GoalProgress { json } => reports::goal_progress(&mut memory, &database, json)?,
        Command::Proactive {
            now_ms,
            deadline_horizon_ms,
            stale_after_ms,
            review_limit,
            json,
        } => reports::proactive(
            &mut memory,
            &database,
            reports::ProactiveArgs {
                now_ms,
                deadline_horizon_ms,
                stale_after_ms,
                review_limit,
            },
            json,
        )?,
        Command::Deadlines {
            now_ms,
            horizon_ms,
            json,
        } => reports::deadlines(&mut memory, &database, now_ms, horizon_ms, json)?,
        Command::Anomalies {
            now_ms,
            deadline_horizon_ms,
            stale_after_ms,
            review_limit,
            json,
        } => reports::anomalies(
            &mut memory,
            &database,
            reports::ProactiveArgs {
                now_ms,
                deadline_horizon_ms,
                stale_after_ms,
                review_limit,
            },
            json,
        )?,
        Command::AnomalyAcknowledge {
            id,
            note,
            dry_run,
            json,
        } => reports::anomaly_acknowledge(&mut memory, id, note, dry_run, json)?,
        Command::Briefing {
            episode_limit,
            intention_limit,
            review_limit,
            graph_seed_limit,
            json,
        } => reports::briefing(
            &mut memory,
            episode_limit,
            intention_limit,
            review_limit,
            graph_seed_limit,
            json,
            &database,
        )?,
        Command::SessionResume {
            episode_limit,
            intention_limit,
            review_limit,
            graph_seed_limit,
            json,
        } => reports::briefing(
            &mut memory,
            episode_limit,
            intention_limit,
            review_limit,
            graph_seed_limit,
            json,
            &database,
        )?,
        Command::Timeline { limit, json } => {
            reports::projection_timeline(&mut memory, &database, limit, json)?
        }
        Command::Pending { limit, json } => {
            reports::projection_pending(&mut memory, &database, limit, json)?
        }
        Command::Sleep {
            episode_limit,
            candidate_limit,
            cycle_limit,
            evidence_limit,
            json,
        } => reports::sleep(
            &mut memory,
            &database,
            episode_limit,
            candidate_limit,
            cycle_limit,
            evidence_limit,
            json,
        )?,
        Command::ConsolidationPlan {
            episode_limit,
            candidate_limit,
            cycle_limit,
            evidence_limit,
            review_limit,
            json,
        } => reports::consolidation_plan(
            &mut memory,
            &database,
            reports::ConsolidationPlanArgs {
                episode_limit,
                candidate_limit,
                cycle_limit,
                evidence_limit,
                review_limit,
            },
            json,
        )?,
        Command::Hook {
            kind,
            input,
            recall_limit,
            episode_limit,
            intention_limit,
            review_limit,
            graph_seed_limit,
            cycle_limit,
            evidence_limit,
            json,
        } => reports::hook(
            &mut memory,
            &database,
            reports::HookArgs {
                kind,
                input,
                recall_limit,
                episode_limit,
                intention_limit,
                review_limit,
                graph_seed_limit,
                cycle_limit,
                evidence_limit,
            },
            json,
        )?,
        Command::Recall {
            query,
            limit,
            authority,
            semantic,
            scope,
            kinds,
            require_evidence,
            as_of_ms,
            max_age_days,
            archive,
            json,
        } => reports::recall(
            &mut memory,
            reports::RecallArgs {
                query,
                limit,
                authority,
                semantic,
                scope,
                kinds,
                require_evidence,
                as_of_ms,
                max_age_days,
                archive,
                json,
            },
        )?,
        Command::Graph {
            seed,
            depth,
            limit,
            json,
        } => reports::graph(&mut memory, seed, depth, limit, json)?,
        Command::Project {
            entity,
            graph_depth,
            graph_limit,
            item_limit,
            recall_limit,
            review_limit,
            json,
        } => reports::project(
            &mut memory,
            &database,
            reports::ProjectArgs {
                entity,
                graph_depth,
                graph_limit,
                item_limit,
                recall_limit,
                review_limit,
            },
            json,
        )?,
        Command::SemanticRebuild { json } => {
            reports::semantic_rebuild(&mut memory, &database, json)?
        }
        Command::SemanticSync { json } => reports::semantic_sync(&mut memory, &database, json)?,
        Command::SemanticStatus { json } => reports::semantic_status(&mut memory, &database, json)?,
        Command::ProjectionStatus { json } => {
            reports::projection_status(&mut memory, &database, json)?
        }
        Command::ProjectionRebuild { json } => {
            reports::projection_rebuild(&mut memory, &database, json)?
        }
        Command::ProjectionValidate { json } => {
            reports::projection_validate(&mut memory, &database, json)?
        }
        Command::ProjectionEntities { query, limit, json } => {
            reports::projection_entities(&mut memory, &database, query, limit, json)?
        }
        Command::ProjectionTimeline { limit, json } => {
            reports::projection_timeline(&mut memory, &database, limit, json)?
        }
        Command::ProjectionPending { limit, json } => {
            reports::projection_pending(&mut memory, &database, limit, json)?
        }
        Command::ProjectionHealth { limit, json } => {
            reports::projection_health(&mut memory, &database, limit, json)?
        }
        Command::Inspect { json } => reports::inspect(&mut memory, json)?,
        Command::SelfInspect { json } => reports::self_inspect(&mut memory, json)?,
        Command::Reflect {
            cycle_limit,
            evidence_limit,
            json,
        } => reports::reflect(&mut memory, cycle_limit, evidence_limit, json)?,
        Command::Review {
            limit,
            min_priority,
            action,
            json,
        } => reports::review(&mut memory, limit, min_priority, action, json)?,
        Command::ReviewResolve {
            review_id,
            note,
            dry_run,
            json,
        } => reports::review_resolve(&mut memory, review_id, note, dry_run, json)?,
        #[cfg(feature = "attestation")]
        Command::AttestSign {
            key_file,
            output,
            json,
        } => attestation::sign(&mut memory, &key_file, output.as_deref(), json)?,
        #[cfg(feature = "attestation")]
        Command::AttestVerify {
            attestation: path,
            keyring,
            json,
        } => attestation::verify(&mut memory, &path, keyring.as_deref(), json)?,
        Command::Audit {
            from,
            to,
            since,
            until,
            #[cfg(feature = "attestation")]
            from_attestation,
            #[cfg(feature = "tamper-evidence")]
            inclusion_proof,
            json,
        } => {
            #[cfg(feature = "attestation")]
            let from = match from_attestation {
                Some(path) => Some(audit::resolve_attestation_anchor(&memory, &path, json)?),
                None => from,
            };
            audit::audit(
                &memory,
                LedgerAuditOptions {
                    from_sequence: from,
                    to_sequence: to,
                    since_ms: since,
                    until_ms: until,
                },
                #[cfg(feature = "tamper-evidence")]
                inclusion_proof,
                json,
            )?
        }
        Command::TrustReport {
            #[cfg(feature = "attestation")]
            attestation,
            html,
            json,
        } => {
            #[cfg(feature = "attestation")]
            let options = {
                let mut options = TrustReportOptions::default();
                if let Some(path) = attestation.as_deref() {
                    options.attestation = Some(trust_report::read_attestation(path)?);
                }
                options
            };
            #[cfg(not(feature = "attestation"))]
            let options = TrustReportOptions::default();
            trust_report::trust_report(&memory, options, html.as_deref(), json)?
        }
        Command::Maintenance { json } => reports::maintenance(&mut memory, &database, json)?,
        Command::Snapshot {
            output,
            dry_run,
            json,
        } => artifacts::snapshot(&mut memory, &database, output, dry_run, json)?,
        Command::SnapshotValidate { path, json } => {
            artifacts::snapshot_validate(&mut memory, &database, path, json)?
        }
        Command::Backup {
            output,
            dry_run,
            json,
        } => artifacts::backup(&mut memory, &database, output, dry_run, json)?,
        Command::Export { output, json } => {
            artifacts::export(&mut memory, &database, output, json)?
        }
        Command::Import {
            path,
            dry_run,
            json,
        } => artifacts::import(&mut memory, &database, path, dry_run, json)?,
        Command::Ingest {
            path,
            dry_run,
            json,
        } => artifacts::ingest(&mut memory, &database, path, dry_run, json)?,
        Command::IngestText {
            path,
            kind,
            title,
            chunking,
            tags,
            mentions,
            metadata,
            source_role,
            scope,
            max_chunk_bytes,
            dry_run,
            json,
        } => text::ingest_text(
            &mut memory,
            &database,
            text::TextFileArgs {
                path,
                kind,
                title,
                chunking,
                tags,
                mentions,
                metadata,
                source_role,
                scope,
                max_chunk_bytes,
                dry_run,
            },
            json,
        )?,
        Command::IngestDir {
            path,
            recursive,
            extensions,
            kind,
            chunking,
            tags,
            mentions,
            metadata,
            source_role,
            scope,
            max_chunk_bytes,
            dry_run,
            json,
        } => text::ingest_dir(
            &mut memory,
            &database,
            text::TextDirectoryArgs {
                path,
                recursive,
                extensions,
                kind,
                chunking,
                tags,
                mentions,
                metadata,
                source_role,
                scope,
                max_chunk_bytes,
                dry_run,
            },
            json,
        )?,
        Command::Data { json } => reports::data(&mut memory, json)?,
        Command::Demo { .. }
        | Command::Init { .. }
        | Command::Completions { .. }
        | Command::Validate { .. }
        | Command::BackupValidate { .. }
        | Command::BackupDrill { .. }
        | Command::Restore { .. }
        | Command::ConvertProjectionExport { .. }
        | Command::ConvertLegacyExport { .. } => {
            unreachable!("command returns before opening memory")
        }
    }

    if verbose {
        eprintln!("nahuali: done in {} ms", started.elapsed().as_millis());
    }
    Ok(())
}

/// Print the effective connection target to stderr for `--verbose`. The defaults
/// mirror nahuali-core's resolution (localhost:18000, namespace "nahuali").
fn verbose_open_line(database: &std::path::Path) {
    let endpoint =
        std::env::var("NAHUALI_DB_URL").unwrap_or_else(|_| "localhost:18000".to_string());
    let namespace = std::env::var("NAHUALI_DB_NAMESPACE").unwrap_or_else(|_| "nahuali".to_string());
    eprintln!(
        "nahuali: opening \"{}\" @ {} (ns {})",
        database.display(),
        endpoint,
        namespace
    );
}

/// Open the store, but turn an unreachable database into a calm, actionable
/// failure instead of a raw connection error: reassure that data is safe,
/// best-effort start the local stack if it is just stopped, and otherwise print
/// the exact command to bring it up. The ledger lives in the database's own
/// durable volume, so an unreachable service never means lost data.
fn open_store(database: &std::path::Path, verbose: bool) -> anyhow::Result<MemoryEngine> {
    match MemoryEngine::open(database) {
        Ok(engine) => Ok(engine),
        Err(error) => {
            let endpoint =
                std::env::var("NAHUALI_DB_URL").unwrap_or_else(|_| "localhost:18000".to_string());
            eprintln!();
            eprintln!("\u{2717} Cannot reach the Nahuali store at ws://{endpoint}.");
            eprintln!(
                "  Your data is safe \u{2014} nothing was lost or deleted; the database service"
            );
            eprintln!("  is just unreachable (the append-only ledger lives in its own volume).");

            let local = endpoint.contains("localhost") || endpoint.contains("127.0.0.1");
            if local
                && try_start_local_stack(verbose)
                && let Ok(engine) = MemoryEngine::open(database)
            {
                eprintln!("  \u{2713} started the local stack and reconnected.");
                return Ok(engine);
            }

            eprintln!("  Start the local stack and retry:");
            eprintln!("      docker compose up -d");
            Err(error).context("the Nahuali store is unreachable (your data is safe)")
        }
    }
}

/// Best-effort: if the standard local stack containers exist but are stopped,
/// start them and wait briefly for readiness. Returns true only when `docker
/// start` succeeds, so the caller can retry the connection. Never fatal.
fn try_start_local_stack(verbose: bool) -> bool {
    if verbose {
        eprintln!("  \u{2192} attempting to start the local stack containers\u{2026}");
    }
    let started = std::process::Command::new("docker")
        .args(["start", "nahual-mictlan-surrealdb", "nahual-tonalli-qdrant"])
        .output();
    match started {
        Ok(output) if output.status.success() => {
            // SurrealDB needs a moment to accept connections after a cold start.
            std::thread::sleep(std::time::Duration::from_secs(3));
            true
        }
        _ => false,
    }
}

fn default_database_name() -> PathBuf {
    std::env::var("NAHUALI_DB_DATABASE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("memory"))
}
