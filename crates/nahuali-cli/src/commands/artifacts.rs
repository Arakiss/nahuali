use std::{fs, path::PathBuf};

use anyhow::{Context, bail};
use nahuali_core::{MemoryEngine, MemoryIngestDocument, MemoryInterchange};

use crate::output;

pub(crate) fn snapshot(
    memory: &mut MemoryEngine,
    database: &std::path::Path,
    output: PathBuf,
    dry_run: bool,
    json: bool,
) -> anyhow::Result<()> {
    let snapshot = if dry_run {
        memory.create_snapshot()
    } else {
        memory
            .write_snapshot(&output)
            .with_context(|| format!("failed to write snapshot {}", output.display()))?
    };
    let summary = snapshot.summary();

    if json {
        output::print_json(&serde_json::json!({
            "database": database.display().to_string(),
            "snapshot_path": output.display().to_string(),
            "dry_run": dry_run,
            "written": !dry_run,
            "summary": summary,
        }))?;
    } else {
        println!("Database: {}", database.display());
        println!("Snapshot: {}", output.display());
        println!("Status: {}", if dry_run { "dry-run" } else { "written" });
        println!("Events: {}", summary.event_count);
        if let Some(last_event_id) = &summary.last_event_id {
            println!("Last event: {last_event_id}");
        }
        println!("Record checksum: {}", summary.record_ledger_checksum);
        println!("Snapshot checksum: {}", summary.checksum);
    }
    Ok(())
}

pub(crate) fn snapshot_validate(
    memory: &mut MemoryEngine,
    database: &std::path::Path,
    path: PathBuf,
    json: bool,
) -> anyhow::Result<()> {
    let report = memory
        .validate_snapshot(&path)
        .with_context(|| format!("failed to validate snapshot {}", path.display()))?;
    if json {
        let mut value = serde_json::to_value(&report)?;
        if let Some(object) = value.as_object_mut() {
            object.insert(
                "database".to_string(),
                serde_json::json!(database.display().to_string()),
            );
            object.insert(
                "snapshot_path".to_string(),
                serde_json::json!(path.display().to_string()),
            );
        }
        output::print_json(&value)?;
    } else {
        println!("Database: {}", database.display());
        println!("Snapshot: {}", path.display());
        println!("Status: {}", if report.valid { "valid" } else { "invalid" });
        println!("Current events: {}", report.current_event_count);
        if let Some(last_event_id) = &report.current_last_event_id {
            println!("Current last event: {last_event_id}");
        }
        println!("Checksum valid: {}", report.checksum_valid);
        println!("Replay equivalent: {}", report.replay_equivalent);
        if !report.issues.is_empty() {
            println!("Issues:");
            for issue in &report.issues {
                println!(
                    "- {}: {}",
                    output::snapshot_issue_kind_name(&issue.kind),
                    issue.message
                );
            }
        }
    }

    if !report.valid {
        bail!("snapshot validation failed");
    }
    Ok(())
}

pub(crate) fn backup(
    memory: &mut MemoryEngine,
    database: &std::path::Path,
    output: PathBuf,
    dry_run: bool,
    json: bool,
) -> anyhow::Result<()> {
    let backup = if dry_run {
        memory.create_backup()
    } else {
        memory
            .write_backup(&output)
            .with_context(|| format!("failed to write backup {}", output.display()))?
    };
    let summary = backup.summary();

    if json {
        output::print_json(&serde_json::json!({
            "database": database.display().to_string(),
            "backup_path": output.display().to_string(),
            "dry_run": dry_run,
            "written": !dry_run,
            "summary": summary,
        }))?;
    } else {
        println!("Database: {}", database.display());
        println!("Backup: {}", output.display());
        println!("Status: {}", if dry_run { "dry-run" } else { "written" });
        println!("Records: {}", summary.record_count);
        if let Some(last_event_id) = &summary.last_event_id {
            println!("Last event: {last_event_id}");
        }
        println!("Record checksum: {}", summary.record_ledger_checksum);
        println!("Backup checksum: {}", summary.checksum);
        println!("Semantic tier: {:?}", summary.semantic_tier.provider);
        println!(
            "Semantic restore policy: {:?}",
            summary.semantic_tier.restore_policy
        );
    }
    Ok(())
}

pub(crate) fn export(
    memory: &mut MemoryEngine,
    database: &std::path::Path,
    output: Option<PathBuf>,
    json: bool,
) -> anyhow::Result<()> {
    let interchange = memory.export_interchange();
    let encoded = serde_json::to_string_pretty(&interchange)?;
    let summary = serde_json::json!({
        "version": interchange.version,
        "episode_count": interchange.episodes.len(),
        "claim_count": interchange.claims.len(),
        "link_count": interchange.links.len(),
        "procedure_count": interchange.procedures.len(),
        "intention_count": interchange.intentions.len(),
    });

    if let Some(output) = output {
        if let Some(parent) = output
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&output, format!("{encoded}\n"))
            .with_context(|| format!("failed to write {}", output.display()))?;
        if json {
            output::print_json(&serde_json::json!({
                "database": database.display().to_string(),
                "interchange_path": output.display().to_string(),
                "summary": summary,
            }))?;
        } else {
            println!("Database: {}", database.display());
            println!("Interchange: {}", output.display());
            println!("Status: exported");
            println!("Episodes: {}", interchange.episodes.len());
            println!("Claims: {}", interchange.claims.len());
            println!("Links: {}", interchange.links.len());
            println!("Procedures: {}", interchange.procedures.len());
            println!("Intentions: {}", interchange.intentions.len());
        }
    } else {
        println!("{encoded}");
    }
    Ok(())
}

pub(crate) fn import(
    memory: &mut MemoryEngine,
    database: &std::path::Path,
    path: PathBuf,
    dry_run: bool,
    json: bool,
) -> anyhow::Result<()> {
    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let interchange: MemoryInterchange = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    let report = memory.import_interchange(&interchange, dry_run)?;
    if json {
        output::print_json(&serde_json::json!({
            "database": database.display().to_string(),
            "interchange_path": path.display().to_string(),
            "report": report,
        }))?;
    } else {
        println!("Database: {}", database.display());
        println!("Interchange: {}", path.display());
        println!(
            "Status: {}",
            if report.valid {
                if dry_run { "dry-run" } else { "imported" }
            } else {
                "invalid"
            }
        );
        println!("Events: {}", report.appendable_event_count);
        println!("Imported events: {}", report.imported_event_count);
        println!("Episodes: {}", report.counts.episodes);
        println!("Claims: {}", report.counts.claims);
        println!("Links: {}", report.counts.links);
        println!("Procedures: {}", report.counts.procedures);
        println!("Intentions: {}", report.counts.intentions);
        output::print_interchange_import_preflight(&report);
        if !report.issues.is_empty() {
            println!("Issues:");
            for issue in &report.issues {
                println!("- {}: {}", issue.path, issue.message);
            }
        }
    }

    if !report.valid {
        bail!("interchange import failed validation");
    }
    Ok(())
}

pub(crate) fn ingest(
    memory: &mut MemoryEngine,
    database: &std::path::Path,
    path: PathBuf,
    dry_run: bool,
    json: bool,
) -> anyhow::Result<()> {
    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let document: MemoryIngestDocument = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    let report = memory.ingest_document(&document, dry_run)?;
    if json {
        output::print_json(&serde_json::json!({
            "database": database.display().to_string(),
            "ingest_path": path.display().to_string(),
            "report": report,
        }))?;
    } else {
        output::print_ingestion_report(database, &path, dry_run, &report);
    }

    if !report.valid {
        bail!("ingestion failed validation");
    }
    Ok(())
}
