mod artifacts;
#[cfg(feature = "attestation")]
mod attestation;
mod migration;
mod migration_legacy;
mod migration_timestamps;
mod migration_values;
mod preopen;
mod record;
mod reports;
mod scope;
mod text;

use std::path::PathBuf;

use anyhow::Context;
use nahuali_core::MemoryEngine;

use crate::cli::{Cli, Command};

pub(crate) fn run(cli: Cli) -> anyhow::Result<()> {
    let database = cli.database.unwrap_or_else(default_database_name);
    if preopen::handle(&cli.command, &database)? {
        return Ok(());
    }

    let mut memory = MemoryEngine::open(&database)
        .with_context(|| format!("failed to open {}", database.display()))?;

    match cli.command {
        Command::Status { json } => reports::status(&mut memory, &database, json)?,
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
            json,
        } => attestation::verify(&mut memory, &path, json)?,
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
        Command::Completions { .. }
        | Command::Validate { .. }
        | Command::BackupValidate { .. }
        | Command::BackupDrill { .. }
        | Command::Restore { .. }
        | Command::ConvertProjectionExport { .. }
        | Command::ConvertLegacyExport { .. } => {
            unreachable!("command returns before opening memory")
        }
    }

    Ok(())
}

fn default_database_name() -> PathBuf {
    std::env::var("NAHUALI_DB_DATABASE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("memory"))
}
