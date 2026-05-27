use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, bail};
use nahuali_core::{IngestionReport, TextIngestBuildReport};

use crate::output;

pub(crate) struct TextFileIngestPlan {
    pub(crate) path: PathBuf,
    pub(crate) adapter_report: TextIngestBuildReport,
    pub(crate) preflight_report: Option<IngestionReport>,
}

pub(crate) struct TextDirectoryOutput<'a> {
    pub(crate) database: &'a Path,
    pub(crate) directory: &'a Path,
    pub(crate) recursive: bool,
    pub(crate) extensions: &'a [String],
    pub(crate) dry_run: bool,
    pub(crate) valid: bool,
    pub(crate) plans: &'a [TextFileIngestPlan],
    pub(crate) reports: &'a [Option<IngestionReport>],
}

pub(crate) fn parse_metadata(values: Vec<String>) -> anyhow::Result<BTreeMap<String, String>> {
    let mut metadata = BTreeMap::new();
    for value in values {
        let (key, metadata_value) = value
            .split_once('=')
            .with_context(|| format!("metadata value `{value}` must use KEY=VALUE"))?;
        let key = key.trim();
        if key.is_empty() {
            bail!("metadata key cannot be empty");
        }
        metadata.insert(key.to_string(), metadata_value.trim().to_string());
    }

    Ok(metadata)
}

pub(crate) fn normalize_extensions(values: Vec<String>) -> anyhow::Result<Vec<String>> {
    let values = if values.is_empty() {
        vec!["md".to_string(), "markdown".to_string(), "txt".to_string()]
    } else {
        values
    };
    let mut extensions = BTreeSet::new();
    for value in values {
        let extension = value.trim().trim_start_matches('.').to_lowercase();
        if extension.is_empty() {
            bail!("extension cannot be empty");
        }
        extensions.insert(extension);
    }

    Ok(extensions.into_iter().collect())
}

pub(crate) fn collect_text_files(
    directory: &Path,
    recursive: bool,
    extensions: &[String],
) -> anyhow::Result<Vec<PathBuf>> {
    let metadata = fs::metadata(directory)
        .with_context(|| format!("failed to inspect {}", directory.display()))?;
    if !metadata.is_dir() {
        bail!("{} is not a directory", directory.display());
    }

    let mut files = Vec::new();
    collect_text_files_into(directory, recursive, extensions, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_text_files_into(
    directory: &Path,
    recursive: bool,
    extensions: &[String],
    files: &mut Vec<PathBuf>,
) -> anyhow::Result<()> {
    let mut entries = fs::read_dir(directory)
        .with_context(|| format!("failed to read directory {}", directory.display()))?
        .collect::<std::io::Result<Vec<_>>>()
        .with_context(|| format!("failed to read directory entry in {}", directory.display()))?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", path.display()))?;
        if file_type.is_dir() {
            if recursive {
                collect_text_files_into(&path, recursive, extensions, files)?;
            }
            continue;
        }
        if file_type.is_file() && has_extension(&path, extensions) {
            files.push(path);
        }
    }

    Ok(())
}

fn has_extension(path: &Path, extensions: &[String]) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_lowercase())
        .is_some_and(|extension| extensions.iter().any(|candidate| candidate == &extension))
}

pub(crate) fn print_text_ingestion_report(
    database: &Path,
    path: &Path,
    dry_run: bool,
    adapter_report: &TextIngestBuildReport,
    ingestion_report: Option<&IngestionReport>,
) {
    println!("Database: {}", database.display());
    println!("Text source: {}", path.display());
    println!(
        "Status: {}",
        if adapter_report.valid {
            if dry_run { "dry-run" } else { "ingested" }
        } else {
            "invalid"
        }
    );
    println!("Source bytes: {}", adapter_report.source_byte_len);
    println!("Generated episodes: {}", adapter_report.episode_count);
    if let Some(report) = ingestion_report {
        println!("Events: {}", report.appendable_event_count);
        println!("Ingested events: {}", report.ingested_event_count);
        output::print_ingestion_preflight(report);
        if let Some(source_id) = &report.source_id {
            println!("Source: {source_id}");
        }
        if !report.episode_ids.is_empty() {
            println!("Episode IDs: {}", report.episode_ids.join(", "));
        }
        if !report.issues.is_empty() {
            println!("Ingestion issues:");
            for issue in &report.issues {
                println!("- {}: {}", issue.path, issue.message);
            }
        }
    }
    if !adapter_report.issues.is_empty() {
        println!("Adapter issues:");
        for issue in &adapter_report.issues {
            println!("- {}: {}", issue.path, issue.message);
        }
    }
}

pub(crate) fn text_directory_ingestion_json(output: &TextDirectoryOutput<'_>) -> serde_json::Value {
    let appendable_event_count = output
        .reports
        .iter()
        .filter_map(Option::as_ref)
        .map(|report| report.appendable_event_count)
        .sum::<usize>();
    let ingested_event_count = output
        .reports
        .iter()
        .filter_map(Option::as_ref)
        .map(|report| report.ingested_event_count)
        .sum::<usize>();
    let files = output
        .plans
        .iter()
        .zip(output.reports.iter())
        .map(|(plan, report)| {
            serde_json::json!({
                "text_path": plan.path.display().to_string(),
                "adapter_report": &plan.adapter_report,
                "report": report,
            })
        })
        .collect::<Vec<_>>();

    serde_json::json!({
        "database": output.database.display().to_string(),
        "directory_path": output.directory.display().to_string(),
        "recursive": output.recursive,
        "extensions": output.extensions,
        "dry_run": output.dry_run,
        "valid": output.valid,
        "file_count": output.plans.len(),
        "appendable_event_count": appendable_event_count,
        "ingested_event_count": ingested_event_count,
        "files": files,
    })
}

pub(crate) fn print_text_directory_ingestion_report(context: &TextDirectoryOutput<'_>) {
    let output = text_directory_ingestion_json(context);
    println!("Database: {}", context.database.display());
    println!("Text directory: {}", context.directory.display());
    println!("Recursive: {}", context.recursive);
    println!("Extensions: {}", context.extensions.join(", "));
    println!(
        "Status: {}",
        if context.valid {
            if context.dry_run {
                "dry-run"
            } else {
                "ingested"
            }
        } else {
            "invalid"
        }
    );
    println!("Files: {}", output["file_count"]);
    println!("Events: {}", output["appendable_event_count"]);
    println!("Ingested events: {}", output["ingested_event_count"]);

    for (plan, report) in context.plans.iter().zip(context.reports.iter()) {
        println!("- {}", plan.path.display());
        println!(
            "  generated episodes: {}",
            plan.adapter_report.episode_count
        );
        if let Some(report) = report {
            println!("  events: {}", report.appendable_event_count);
            println!("  ingested events: {}", report.ingested_event_count);
            println!("  evidence gaps: {}", report.preflight.evidence_gap_count);
            println!(
                "  unreferenced episodes: {}",
                report.preflight.unreferenced_episode_count
            );
        }
        if !plan.adapter_report.issues.is_empty() {
            println!("  adapter issues:");
            for issue in &plan.adapter_report.issues {
                println!("  - {}: {}", issue.path, issue.message);
            }
        }
        if let Some(report) = report
            && !report.issues.is_empty()
        {
            println!("  ingestion issues:");
            for issue in &report.issues {
                println!("  - {}: {}", issue.path, issue.message);
            }
        }
    }
}
