use std::path::Path;

use anyhow::{Context, bail};
use clap::CommandFactory;
use clap_complete::generate;
use nahuali_core::MemoryEngine;

use crate::{
    cli::{Cli, Command},
    output,
};

use super::{migration, migration_legacy};

pub(crate) fn handle(command: &Command, database: &Path) -> anyhow::Result<bool> {
    match command {
        Command::Demo {} => super::demo::demo().map(|()| true),
        Command::Init { dry_run, force } => super::init::init(*dry_run, *force).map(|()| true),
        Command::Completions { shell } => {
            let mut command = Cli::command();
            let bin_name = command.get_name().to_string();
            generate(
                clap_complete::Shell::from(*shell),
                &mut command,
                bin_name,
                &mut std::io::stdout(),
            );
            Ok(true)
        }
        Command::Validate { json } => validate(database, *json),
        Command::BackupValidate { path, json } => backup_validate(path, *json),
        Command::BackupDrill {
            path,
            target_database,
            json,
        } => backup_drill(path, target_database, *json),
        Command::Restore {
            path,
            target_database,
            dry_run,
            json,
        } => restore(path, target_database, *dry_run, *json),
        Command::ConvertProjectionExport {
            path,
            output,
            scope,
            json,
        } => migration::convert_projection_export(path, output, scope.clone(), *json),
        Command::ConvertLegacyExport {
            path,
            output,
            scope,
            json,
        } => migration_legacy::convert_legacy_export(path, output, scope.clone(), *json),
        _ => Ok(false),
    }
}

fn validate(database: &Path, json: bool) -> anyhow::Result<bool> {
    let report = MemoryEngine::validate_store(database)
        .with_context(|| format!("failed to validate {}", database.display()))?;
    if json {
        if report.valid {
            let memory = MemoryEngine::open(database)
                .with_context(|| format!("failed to open {}", database.display()))?;
            let data = memory.data();
            println!(
                "{}",
                serde_json::json!({
                    "database": database.display().to_string(),
                    "record_ledger_table": "memory_record",
                    "projection": "rust",
                    "valid": true,
                    "event_count": report.event_count,
                    "source_count": data.sources.len(),
                    "entity_count": data.entities.len(),
                    "episode_count": data.episodes.len(),
                    "claim_count": data.claims.len(),
                    "link_count": data.links.len(),
                    "fact_count": data.facts.len(),
                    "relation_count": data.relations.len(),
                    "procedure_count": data.procedures.len(),
                    "intention_count": data.intentions.len(),
                    "review_decision_count": data.review_decisions.len(),
                    "last_event_id": data.last_event_id,
                    "supported_event_version": report.supported_event_version,
                    "observed_event_versions": report.observed_event_versions,
                    "legacy_event_count": report.legacy_event_count,
                    "migration_required": report.migration_required,
                    "issues": report.issues,
                })
            );
        } else {
            let mut value = serde_json::to_value(&report)?;
            if let Some(object) = value.as_object_mut() {
                object.insert(
                    "database".to_string(),
                    serde_json::json!(database.display().to_string()),
                );
                object.insert(
                    "record_ledger_table".to_string(),
                    serde_json::json!("memory_record"),
                );
                object.insert("projection".to_string(), serde_json::json!("rust"));
            }
            println!("{}", serde_json::to_string(&value)?);
        }
    } else if report.valid {
        let memory = MemoryEngine::open(database)
            .with_context(|| format!("failed to open {}", database.display()))?;
        let data = memory.data();
        println!("Database: {}", database.display());
        println!("Record ledger: memory_record");
        println!("Projection: Rust");
        println!("Status: valid");
        println!("Events: {}", report.event_count);
        println!("Sources: {}", data.sources.len());
        println!("Entities: {}", data.entities.len());
        println!("Episodes: {}", data.episodes.len());
        println!("Claims: {}", data.claims.len());
        println!("Links: {}", data.links.len());
        println!("Facts: {}", data.facts.len());
        println!("Relations: {}", data.relations.len());
        println!("Procedures: {}", data.procedures.len());
        println!("Intentions: {}", data.intentions.len());
        println!("Review decisions: {}", data.review_decisions.len());
        println!("Migration required: {}", report.migration_required);
        if let Some(last_event_id) = &data.last_event_id {
            println!("Last event: {last_event_id}");
        }
    } else {
        println!("Database: {}", database.display());
        println!("Record ledger: memory_record");
        println!("Projection: Rust");
        println!("Status: invalid");
        for issue in &report.issues {
            let line = issue
                .line
                .map(|line| format!("line {line}: "))
                .unwrap_or_default();
            println!(
                "- {line}{}: {}",
                output::issue_kind_name(&issue.kind),
                issue.message
            );
        }
    }

    if report.valid {
        return Ok(true);
    }
    bail!("memory record validation failed");
}

fn backup_validate(path: &Path, json: bool) -> anyhow::Result<bool> {
    let report = MemoryEngine::validate_backup(path)
        .with_context(|| format!("failed to validate backup {}", path.display()))?;
    if json {
        let mut value = serde_json::to_value(&report)?;
        if let Some(object) = value.as_object_mut() {
            object.insert(
                "backup_path".to_string(),
                serde_json::json!(path.display().to_string()),
            );
        }
        println!("{}", serde_json::to_string(&value)?);
    } else {
        println!("Backup: {}", path.display());
        println!("Status: {}", if report.valid { "valid" } else { "invalid" });
        println!("Checksum valid: {}", report.checksum_valid);
        println!("Records valid: {}", report.records_valid);
        if let Some(record_count) = report.backup_record_count {
            println!("Records: {record_count}");
        }
        if let Some(checksum) = &report.backup_record_ledger_checksum {
            println!("Record checksum: {checksum}");
        }
        if !report.issues.is_empty() {
            println!("Issues:");
            for issue in &report.issues {
                let record = issue
                    .record
                    .map(|record| format!("record {record}: "))
                    .unwrap_or_default();
                println!(
                    "- {record}{}: {}",
                    output::backup_issue_kind_name(&issue.kind),
                    issue.message
                );
            }
        }
    }

    if report.valid {
        return Ok(true);
    }
    bail!("backup validation failed");
}

fn backup_drill(path: &Path, target_database: &Path, json: bool) -> anyhow::Result<bool> {
    let report = MemoryEngine::backup_drill(path, target_database).with_context(|| {
        format!(
            "failed to drill backup {} into {}",
            path.display(),
            target_database.display()
        )
    })?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("Backup: {}", path.display());
        println!("Target database: {}", target_database.display());
        println!("Status: {}", if report.valid { "ready" } else { "blocked" });
        println!("Backup valid: {}", report.backup_validation.valid);
        println!("Restore dry-run valid: {}", report.restore_dry_run.valid);
        println!(
            "Target was empty: {}",
            report.restore_dry_run.target_was_empty
        );
        println!(
            "Semantic rebuild required: {}",
            report.semantic_rebuild_required
        );
        print_backup_issues("Backup issues", &report.backup_validation.issues);
        print_backup_issues("Restore issues", &report.restore_dry_run.issues);
        println!("Next actions:");
        for action in &report.operator_next_actions {
            println!("- {action}");
        }
    }

    if report.valid {
        return Ok(true);
    }
    bail!("backup drill failed validation");
}

fn restore(path: &Path, target_database: &Path, dry_run: bool, json: bool) -> anyhow::Result<bool> {
    let report =
        MemoryEngine::restore_backup(path, target_database, dry_run).with_context(|| {
            format!(
                "failed to restore backup {} into {}",
                path.display(),
                target_database.display()
            )
        })?;
    if json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        println!("Backup: {}", path.display());
        println!("Target database: {}", target_database.display());
        println!(
            "Status: {}",
            if report.valid {
                if dry_run { "dry-run" } else { "restored" }
            } else {
                "invalid"
            }
        );
        println!("Events: {}", report.appendable_event_count);
        println!("Restored events: {}", report.restored_event_count);
        println!("Target was empty: {}", report.target_was_empty);
        if let Some(checksum) = &report.record_ledger_checksum {
            println!("Record checksum: {checksum}");
        }
        if let Some(policy) = &report.semantic_restore_policy {
            println!("Semantic restore policy: {policy:?}");
        }
        print_backup_issues("Issues", &report.issues);
    }

    if report.valid {
        return Ok(true);
    }
    bail!("backup restore failed validation");
}

fn print_backup_issues(title: &str, issues: &[nahuali_core::BackupIssue]) {
    if issues.is_empty() {
        return;
    }
    println!("{title}:");
    for issue in issues {
        let record = issue
            .record
            .map(|record| format!("record {record}: "))
            .unwrap_or_default();
        println!(
            "- {record}{}: {}",
            output::backup_issue_kind_name(&issue.kind),
            issue.message
        );
    }
}
