use nahuali_core::{
    IngestionCounts, IngestionIssue, IngestionPreflight, IngestionReport, TextIngestBuildReport,
    TextIngestIssue,
};
use rmcp::schemars;
use serde::Serialize;
use serde_json::Value;

use super::{ScopeView, json_string};

/// Source-neutral record counts reported by ingestion.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct IngestionCountsView {
    sources: usize,
    episodes: usize,
    claims: usize,
    links: usize,
    procedures: usize,
    intentions: usize,
    intention_status_updates: usize,
}

impl From<IngestionCounts> for IngestionCountsView {
    fn from(counts: IngestionCounts) -> Self {
        Self {
            sources: counts.sources,
            episodes: counts.episodes,
            claims: counts.claims,
            links: counts.links,
            procedures: counts.procedures,
            intentions: counts.intentions,
            intention_status_updates: counts.intention_status_updates,
        }
    }
}

/// Boundary and evidence summary computed before ingestion writes.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct IngestionPreflightView {
    source_scoped: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_scope: Option<ScopeView>,
    source_byte_len: u64,
    derived_record_count: usize,
    evidence_linked_record_count: usize,
    evidence_gap_count: usize,
    referenced_episode_count: usize,
    unreferenced_episode_count: usize,
}

impl From<IngestionPreflight> for IngestionPreflightView {
    fn from(preflight: IngestionPreflight) -> Self {
        Self {
            source_scoped: preflight.source_scoped,
            source_scope: preflight.source_scope.map(ScopeView::from),
            source_byte_len: preflight.source_byte_len,
            derived_record_count: preflight.derived_record_count,
            evidence_linked_record_count: preflight.evidence_linked_record_count,
            evidence_gap_count: preflight.evidence_gap_count,
            referenced_episode_count: preflight.referenced_episode_count,
            unreferenced_episode_count: preflight.unreferenced_episode_count,
        }
    }
}

/// Validation issue found in an ingestion document.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct IngestionIssueView {
    kind: String,
    path: String,
    message: String,
}

impl From<IngestionIssue> for IngestionIssueView {
    fn from(issue: IngestionIssue) -> Self {
        Self {
            kind: json_string(&issue.kind),
            path: issue.path,
            message: issue.message,
        }
    }
}

/// Validation report returned by ingestion.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct IngestionReportView {
    version: u32,
    valid: bool,
    dry_run: bool,
    appendable_event_count: usize,
    ingested_event_count: usize,
    counts: IngestionCountsView,
    preflight: IngestionPreflightView,
    source_id: Option<String>,
    episode_ids: Vec<String>,
    issues: Vec<IngestionIssueView>,
}

impl From<IngestionReport> for IngestionReportView {
    fn from(report: IngestionReport) -> Self {
        Self {
            version: report.version,
            valid: report.valid,
            dry_run: report.dry_run,
            appendable_event_count: report.appendable_event_count,
            ingested_event_count: report.ingested_event_count,
            counts: IngestionCountsView::from(report.counts),
            preflight: IngestionPreflightView::from(report.preflight),
            source_id: report.source_id,
            episode_ids: report.episode_ids,
            issues: report
                .issues
                .into_iter()
                .map(IngestionIssueView::from)
                .collect(),
        }
    }
}

/// Validation issue returned by the text ingestion adapter.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct TextIngestIssueView {
    kind: String,
    path: String,
    message: String,
}

impl From<TextIngestIssue> for TextIngestIssueView {
    fn from(issue: TextIngestIssue) -> Self {
        Self {
            kind: json_string(&issue.kind),
            path: issue.path,
            message: issue.message,
        }
    }
}

/// Report returned by the text ingestion adapter. The generated `document`
/// mirrors the typed `IngestArgs.document` interchange format and is carried
/// through verbatim, matching how that document is modeled on the input side.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct TextIngestBuildReportView {
    version: u32,
    valid: bool,
    source_byte_len: u64,
    episode_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    document: Option<Value>,
    issues: Vec<TextIngestIssueView>,
}

impl From<TextIngestBuildReport> for TextIngestBuildReportView {
    fn from(report: TextIngestBuildReport) -> Self {
        Self {
            version: report.version,
            valid: report.valid,
            source_byte_len: report.source_byte_len,
            episode_count: report.episode_count,
            document: report
                .document
                .map(|document| serde_json::to_value(document).unwrap_or(Value::Null)),
            issues: report
                .issues
                .into_iter()
                .map(TextIngestIssueView::from)
                .collect(),
        }
    }
}
