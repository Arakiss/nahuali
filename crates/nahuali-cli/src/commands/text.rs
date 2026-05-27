use std::{fs, path::PathBuf};

use anyhow::{Context, bail};
use nahuali_core::{MemoryEngine, SourceKind, TextChunking, TextIngestOptions};

use crate::{
    cli::{CliSourceKind, CliTextChunking},
    commands::scope::parse_scope,
    text_intake::{
        TextDirectoryOutput, TextFileIngestPlan, collect_text_files, normalize_extensions,
        parse_metadata, print_text_directory_ingestion_report, print_text_ingestion_report,
        text_directory_ingestion_json,
    },
};

pub(crate) fn ingest_text(
    memory: &mut MemoryEngine,
    database: &std::path::Path,
    args: TextFileArgs,
    json: bool,
) -> anyhow::Result<()> {
    let content = fs::read_to_string(&args.path)
        .with_context(|| format!("failed to read {}", args.path.display()))?;
    let title = args.title.or_else(|| {
        args.path
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string)
    });
    let scope = parse_scope(args.scope)?;
    let adapter_report = nahuali_core::build_text_ingest_document(
        &content,
        TextIngestOptions {
            source_kind: SourceKind::from(args.kind),
            title,
            uri: Some(args.path.display().to_string()),
            metadata: parse_metadata(args.metadata)?,
            scope,
            tags: args.tags,
            mentions: args.mentions,
            source_role: args.source_role,
            chunking: TextChunking::from(args.chunking),
            max_chunk_bytes: args.max_chunk_bytes,
        },
    );
    let ingestion_report = if let Some(document) = &adapter_report.document {
        Some(memory.ingest_document(document, args.dry_run)?)
    } else {
        None
    };

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "database": database.display().to_string(),
                "text_path": args.path.display().to_string(),
                "adapter_report": &adapter_report,
                "report": &ingestion_report,
            }))?
        );
    } else {
        print_text_ingestion_report(
            database,
            &args.path,
            args.dry_run,
            &adapter_report,
            ingestion_report.as_ref(),
        );
    }

    if !adapter_report.valid
        || ingestion_report
            .as_ref()
            .is_some_and(|report| !report.valid)
    {
        bail!("text ingestion failed validation");
    }
    Ok(())
}

pub(crate) fn ingest_dir(
    memory: &mut MemoryEngine,
    database: &std::path::Path,
    args: TextDirectoryArgs,
    json: bool,
) -> anyhow::Result<()> {
    let extensions = normalize_extensions(args.extensions)?;
    let files = collect_text_files(&args.path, args.recursive, &extensions)?;
    if files.is_empty() {
        bail!(
            "no text files found in {} for extensions: {}",
            args.path.display(),
            extensions.join(", ")
        );
    }

    let source_kind = SourceKind::from(args.kind);
    let chunking = TextChunking::from(args.chunking);
    let metadata = parse_metadata(args.metadata)?;
    let scope = parse_scope(args.scope)?;
    let mut plans = Vec::new();
    for file in files {
        let content = fs::read_to_string(&file)
            .with_context(|| format!("failed to read {}", file.display()))?;
        let title = file
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string);
        let adapter_report = nahuali_core::build_text_ingest_document(
            &content,
            TextIngestOptions {
                source_kind: source_kind.clone(),
                title,
                uri: Some(file.display().to_string()),
                metadata: metadata.clone(),
                scope: scope.clone(),
                tags: args.tags.clone(),
                mentions: args.mentions.clone(),
                source_role: args.source_role.clone(),
                chunking: chunking.clone(),
                max_chunk_bytes: args.max_chunk_bytes,
            },
        );
        let preflight_report = if let Some(document) = &adapter_report.document {
            Some(memory.ingest_document(document, true)?)
        } else {
            None
        };
        plans.push(TextFileIngestPlan {
            path: file,
            adapter_report,
            preflight_report,
        });
    }

    let valid = plans.iter().all(|plan| {
        plan.adapter_report.valid
            && plan
                .preflight_report
                .as_ref()
                .is_some_and(|report| report.valid)
    });
    let reports = if valid && !args.dry_run {
        let mut reports = Vec::new();
        for plan in &plans {
            let report = plan
                .adapter_report
                .document
                .as_ref()
                .map(|document| memory.ingest_document(document, false))
                .transpose()?;
            reports.push(report);
        }
        reports
    } else {
        plans
            .iter()
            .map(|plan| plan.preflight_report.clone())
            .collect::<Vec<_>>()
    };

    let output = TextDirectoryOutput {
        database,
        directory: &args.path,
        recursive: args.recursive,
        extensions: &extensions,
        dry_run: args.dry_run,
        valid,
        plans: &plans,
        reports: &reports,
    };

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&text_directory_ingestion_json(&output))?
        );
    } else {
        print_text_directory_ingestion_report(&output);
    }

    if !valid {
        bail!("directory text ingestion failed validation");
    }
    Ok(())
}

pub(crate) struct TextFileArgs {
    pub(crate) path: PathBuf,
    pub(crate) kind: CliSourceKind,
    pub(crate) title: Option<String>,
    pub(crate) chunking: CliTextChunking,
    pub(crate) tags: Vec<String>,
    pub(crate) mentions: Vec<String>,
    pub(crate) metadata: Vec<String>,
    pub(crate) source_role: Option<String>,
    pub(crate) scope: Option<String>,
    pub(crate) max_chunk_bytes: usize,
    pub(crate) dry_run: bool,
}

pub(crate) struct TextDirectoryArgs {
    pub(crate) path: PathBuf,
    pub(crate) recursive: bool,
    pub(crate) extensions: Vec<String>,
    pub(crate) kind: CliSourceKind,
    pub(crate) chunking: CliTextChunking,
    pub(crate) tags: Vec<String>,
    pub(crate) mentions: Vec<String>,
    pub(crate) metadata: Vec<String>,
    pub(crate) source_role: Option<String>,
    pub(crate) scope: Option<String>,
    pub(crate) max_chunk_bytes: usize,
    pub(crate) dry_run: bool,
}
