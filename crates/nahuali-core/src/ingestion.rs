use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{
    IntentionKind, IntentionPriority, IntentionStatus, MemoryScope, ProcedureKind, Result,
    SourceKind, SourceRecordOptions, store::MemoryEngine,
};

/// Current source-neutral ingestion document version.
pub const MEMORY_INGEST_DOCUMENT_VERSION: u32 = 1;

/// Source-neutral ingestion document for provenance-aware intake.
///
/// This document is an adapter boundary, not the record ledger. It lets scripts,
/// future connectors, and agent clients submit source material plus explicit
/// memory records while preserving where those records came from.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct MemoryIngestDocument {
    /// Ingestion document version.
    pub version: u32,
    /// Source material represented by this document.
    pub source: IngestSource,
    /// Ground-truth source episodes or messages.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub episodes: Vec<IngestEpisode>,
    /// Explicit derived claims backed by source episodes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub claims: Vec<IngestClaim>,
    /// Explicit typed connections backed by source episodes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<IngestLink>,
    /// Explicit procedures or preferences backed by source episodes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub procedures: Vec<IngestProcedure>,
    /// Explicit future work, goals, reminders, or commitments.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub intentions: Vec<IngestIntention>,
}

/// Source metadata in an ingestion document.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct IngestSource {
    /// Source category.
    #[serde(default = "default_source_kind")]
    pub kind: SourceKind,
    /// Human-readable source title, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Source URI, path, or adapter-provided locator, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    /// Adapter-provided metadata preserved as source provenance.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
    /// Explicit memory context boundary for all records in this document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<MemoryScope>,
}

/// Source episode or message in an ingestion document.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct IngestEpisode {
    /// Optional local reference used by derived records in the same document.
    #[serde(default, rename = "ref", skip_serializing_if = "Option::is_none")]
    pub ref_id: Option<String>,
    /// Natural-language episode content.
    pub content: String,
    /// User-provided labels for filtering or recall.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Explicit entity names mentioned by this episode.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mentions: Vec<String>,
    /// Stable position within the source, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_position: Option<u32>,
    /// Source-local actor, role, or speaker, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_role: Option<String>,
}

/// Explicit claim in an ingestion document.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct IngestClaim {
    /// Entity or concept the assertion is about.
    pub subject: String,
    /// Relationship or attribute being asserted.
    pub predicate: String,
    /// Assertion value.
    pub object: String,
    /// Optional local episode reference from the same ingestion document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_episode_ref: Option<String>,
    /// Caller-provided confidence; defaults to `0.8` when omitted.
    #[serde(default = "default_confidence")]
    pub confidence: f32,
}

/// Explicit typed connection in an ingestion document.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct IngestLink {
    /// Source endpoint of the link.
    pub from: String,
    /// Link label.
    pub relation: String,
    /// Target endpoint of the link.
    pub to: String,
    /// Optional local episode reference from the same ingestion document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_episode_ref: Option<String>,
    /// Caller-provided confidence; defaults to `0.8` when omitted.
    #[serde(default = "default_confidence")]
    pub confidence: f32,
}

/// Explicit procedure or preference in an ingestion document.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct IngestProcedure {
    /// Whether this record is a procedure or preference.
    #[serde(default = "default_procedure_kind")]
    pub kind: ProcedureKind,
    /// Human-readable procedure or preference name.
    pub name: String,
    /// Operational instruction or preference body.
    pub body: String,
    /// Optional local episode reference from the same ingestion document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_episode_ref: Option<String>,
    /// Caller-provided confidence; defaults to `0.8` when omitted.
    #[serde(default = "default_confidence")]
    pub confidence: f32,
}

/// Explicit intention in an ingestion document.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct IngestIntention {
    /// Intention category.
    #[serde(default = "default_intention_kind")]
    pub kind: IntentionKind,
    /// Intention priority.
    #[serde(default = "default_intention_priority")]
    pub priority: IntentionPriority,
    /// Current lifecycle status.
    #[serde(default = "default_intention_status")]
    pub status: IntentionStatus,
    /// What should happen.
    pub description: String,
    /// Optional local episode reference from the same ingestion document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_episode_ref: Option<String>,
    /// Optional reason for non-active lifecycle status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_reason: Option<String>,
}

/// Counts reported by an ingestion dry-run or execution.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct IngestionCounts {
    /// Source records in the document.
    pub sources: usize,
    /// Episodes in the document.
    pub episodes: usize,
    /// Claims in the document.
    pub claims: usize,
    /// Links in the document.
    pub links: usize,
    /// Procedures and preferences in the document.
    pub procedures: usize,
    /// Intentions in the document.
    pub intentions: usize,
    /// Intention lifecycle updates needed to preserve non-active statuses.
    pub intention_status_updates: usize,
}

impl IngestionCounts {
    /// Return the total number of append-only events implied by the ingestion.
    pub fn event_count(&self) -> usize {
        self.sources
            + self.episodes
            + self.claims
            + self.links
            + self.procedures
            + self.intentions
            + self.intention_status_updates
    }
}

/// Boundary and evidence summary computed before ingestion writes.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct IngestionPreflight {
    /// Whether the source document declares an explicit memory scope.
    pub source_scoped: bool,
    /// Scope inherited by all records in this ingestion document, when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_scope: Option<MemoryScope>,
    /// Total bytes of source episode content inspected by ingestion.
    pub source_byte_len: u64,
    /// Explicit derived records included in the document.
    pub derived_record_count: usize,
    /// Derived records that cite a source episode reference.
    pub evidence_linked_record_count: usize,
    /// Derived records that do not cite source evidence.
    pub evidence_gap_count: usize,
    /// Unique source episodes referenced by derived records.
    pub referenced_episode_count: usize,
    /// Source episodes not referenced by any derived record.
    pub unreferenced_episode_count: usize,
}

/// Validation report returned by ingestion.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct IngestionReport {
    /// Document version that was inspected.
    pub version: u32,
    /// Whether the document can be ingested.
    pub valid: bool,
    /// Whether ingestion was a dry-run.
    pub dry_run: bool,
    /// Number of append-only events that would be written by valid ingestion.
    pub appendable_event_count: usize,
    /// Number of append-only events written by this execution.
    pub ingested_event_count: usize,
    /// Source-neutral record counts.
    pub counts: IngestionCounts,
    /// Scope, size, and evidence summary computed before ingestion writes.
    pub preflight: IngestionPreflight,
    /// Source identifier written by this execution, when applied.
    pub source_id: Option<String>,
    /// Episode identifiers written by this execution, when applied.
    pub episode_ids: Vec<String>,
    /// Validation issues found before ingestion.
    pub issues: Vec<IngestionIssue>,
}

/// Validation issue found in an ingestion document.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct IngestionIssue {
    /// Machine-readable issue kind.
    pub kind: IngestionIssueKind,
    /// Document path for the invalid field.
    pub path: String,
    /// Human-readable issue message.
    pub message: String,
}

/// Machine-readable ingestion validation issue kind.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IngestionIssueKind {
    /// Document version is not supported.
    UnsupportedVersion,
    /// A required text field is empty after trimming.
    EmptyField,
    /// A source must include at least a title or URI.
    EmptySourceLocator,
    /// At least one source episode is required.
    NoEpisodes,
    /// A local episode reference is duplicated.
    DuplicateReference,
    /// A derived record cites an episode reference that does not exist.
    UnknownSourceReference,
}

pub(crate) fn ingest(
    memory: &mut MemoryEngine,
    document: &MemoryIngestDocument,
    dry_run: bool,
) -> Result<IngestionReport> {
    let mut report = validate(document, dry_run);
    if !report.valid || dry_run {
        return Ok(report);
    }

    let content_checksum = content_checksum(document);
    let byte_len = source_byte_len(document);
    let source = memory.record_source_with_options(SourceRecordOptions {
        kind: document.source.kind.clone(),
        title: document.source.title.clone(),
        uri: document.source.uri.clone(),
        content_checksum,
        byte_len,
        metadata: document.source.metadata.clone(),
        scope: document.source.scope.clone(),
    })?;
    report.source_id = Some(source.id.clone());
    report.ingested_event_count += 1;

    let mut episode_refs = HashMap::new();
    for episode in &document.episodes {
        let imported = memory.remember_source_episode(
            episode.content.trim(),
            clean_strings(&episode.tags),
            clean_strings(&episode.mentions),
            source.id.clone(),
            episode.source_position,
            clean_optional(&episode.source_role),
        )?;
        if let Some(ref_id) = clean_optional(&episode.ref_id) {
            episode_refs.insert(ref_id, imported.id.clone());
        }
        report.episode_ids.push(imported.id);
        report.ingested_event_count += 1;
    }

    for claim in &document.claims {
        memory.add_claim(
            claim.subject.trim(),
            claim.predicate.trim(),
            claim.object.trim(),
            resolve_ref(&episode_refs, &claim.source_episode_ref),
            claim.confidence,
        )?;
        report.ingested_event_count += 1;
    }

    for link in &document.links {
        memory.add_link(
            link.from.trim(),
            link.relation.trim(),
            link.to.trim(),
            resolve_ref(&episode_refs, &link.source_episode_ref),
            link.confidence,
        )?;
        report.ingested_event_count += 1;
    }

    for procedure in &document.procedures {
        match &procedure.kind {
            ProcedureKind::Procedure => {
                memory.add_procedure(
                    procedure.name.trim(),
                    procedure.body.trim(),
                    resolve_ref(&episode_refs, &procedure.source_episode_ref),
                    procedure.confidence,
                )?;
            }
            ProcedureKind::Preference => {
                memory.add_preference(
                    procedure.name.trim(),
                    procedure.body.trim(),
                    resolve_ref(&episode_refs, &procedure.source_episode_ref),
                    procedure.confidence,
                )?;
            }
        }
        report.ingested_event_count += 1;
    }

    for intention in &document.intentions {
        let imported = memory.add_intention(
            intention.description.trim(),
            intention.kind.clone(),
            intention.priority.clone(),
            resolve_ref(&episode_refs, &intention.source_episode_ref),
        )?;
        report.ingested_event_count += 1;
        if intention.status != IntentionStatus::Active {
            memory.set_intention_status(
                imported.id,
                intention.status.clone(),
                clean_optional(&intention.status_reason),
            )?;
            report.ingested_event_count += 1;
        }
    }

    Ok(report)
}

fn validate(document: &MemoryIngestDocument, dry_run: bool) -> IngestionReport {
    let counts = IngestionCounts {
        sources: 1,
        episodes: document.episodes.len(),
        claims: document.claims.len(),
        links: document.links.len(),
        procedures: document.procedures.len(),
        intentions: document.intentions.len(),
        intention_status_updates: document
            .intentions
            .iter()
            .filter(|intention| intention.status != IntentionStatus::Active)
            .count(),
    };
    let mut issues = Vec::new();
    let mut episode_refs = HashSet::new();

    if document.version != MEMORY_INGEST_DOCUMENT_VERSION {
        issues.push(issue(
            IngestionIssueKind::UnsupportedVersion,
            "version",
            format!(
                "unsupported ingestion version {}, supported version is {}",
                document.version, MEMORY_INGEST_DOCUMENT_VERSION
            ),
        ));
    }

    if clean_optional(&document.source.title).is_none()
        && clean_optional(&document.source.uri).is_none()
    {
        issues.push(issue(
            IngestionIssueKind::EmptySourceLocator,
            "source",
            "source must include a title or uri",
        ));
    }

    if document.episodes.is_empty() {
        issues.push(issue(
            IngestionIssueKind::NoEpisodes,
            "episodes",
            "at least one source episode is required",
        ));
    }

    for (index, episode) in document.episodes.iter().enumerate() {
        require_text(
            &mut issues,
            format!("episodes[{index}].content"),
            &episode.content,
        );
        if let Some(ref_id) = clean_optional(&episode.ref_id)
            && !episode_refs.insert(ref_id.clone())
        {
            issues.push(issue(
                IngestionIssueKind::DuplicateReference,
                format!("episodes[{index}].ref"),
                format!("duplicate episode reference {ref_id}"),
            ));
        }
    }

    for (index, claim) in document.claims.iter().enumerate() {
        require_text(
            &mut issues,
            format!("claims[{index}].subject"),
            &claim.subject,
        );
        require_text(
            &mut issues,
            format!("claims[{index}].predicate"),
            &claim.predicate,
        );
        require_text(
            &mut issues,
            format!("claims[{index}].object"),
            &claim.object,
        );
        require_known_ref(
            &mut issues,
            format!("claims[{index}].source_episode_ref"),
            &claim.source_episode_ref,
            &episode_refs,
        );
    }

    for (index, link) in document.links.iter().enumerate() {
        require_text(&mut issues, format!("links[{index}].from"), &link.from);
        require_text(
            &mut issues,
            format!("links[{index}].relation"),
            &link.relation,
        );
        require_text(&mut issues, format!("links[{index}].to"), &link.to);
        require_known_ref(
            &mut issues,
            format!("links[{index}].source_episode_ref"),
            &link.source_episode_ref,
            &episode_refs,
        );
    }

    for (index, procedure) in document.procedures.iter().enumerate() {
        require_text(
            &mut issues,
            format!("procedures[{index}].name"),
            &procedure.name,
        );
        require_text(
            &mut issues,
            format!("procedures[{index}].body"),
            &procedure.body,
        );
        require_known_ref(
            &mut issues,
            format!("procedures[{index}].source_episode_ref"),
            &procedure.source_episode_ref,
            &episode_refs,
        );
    }

    for (index, intention) in document.intentions.iter().enumerate() {
        require_text(
            &mut issues,
            format!("intentions[{index}].description"),
            &intention.description,
        );
        require_known_ref(
            &mut issues,
            format!("intentions[{index}].source_episode_ref"),
            &intention.source_episode_ref,
            &episode_refs,
        );
    }

    let valid = issues.is_empty();
    let preflight = preflight(document, &counts);
    IngestionReport {
        version: document.version,
        valid,
        dry_run,
        appendable_event_count: counts.event_count(),
        ingested_event_count: 0,
        counts,
        preflight,
        source_id: None,
        episode_ids: Vec::new(),
        issues,
    }
}

fn preflight(document: &MemoryIngestDocument, counts: &IngestionCounts) -> IngestionPreflight {
    let source_scope = document.source.scope.clone();
    let derived_record_count = counts.claims + counts.links + counts.procedures + counts.intentions;
    let evidence_linked_record_count = evidence_linked_record_count(document);
    let referenced_episode_count = referenced_episode_count(document);

    IngestionPreflight {
        source_scoped: source_scope.is_some(),
        source_scope,
        source_byte_len: source_byte_len(document),
        derived_record_count,
        evidence_linked_record_count,
        evidence_gap_count: derived_record_count.saturating_sub(evidence_linked_record_count),
        referenced_episode_count,
        unreferenced_episode_count: document
            .episodes
            .len()
            .saturating_sub(referenced_episode_count),
    }
}

fn evidence_linked_record_count(document: &MemoryIngestDocument) -> usize {
    document
        .claims
        .iter()
        .filter(|record| clean_optional(&record.source_episode_ref).is_some())
        .count()
        + document
            .links
            .iter()
            .filter(|record| clean_optional(&record.source_episode_ref).is_some())
            .count()
        + document
            .procedures
            .iter()
            .filter(|record| clean_optional(&record.source_episode_ref).is_some())
            .count()
        + document
            .intentions
            .iter()
            .filter(|record| clean_optional(&record.source_episode_ref).is_some())
            .count()
}

fn referenced_episode_count(document: &MemoryIngestDocument) -> usize {
    let known_refs = document
        .episodes
        .iter()
        .filter_map(|episode| clean_optional(&episode.ref_id))
        .collect::<HashSet<_>>();
    let mut referenced_refs = HashSet::new();

    for ref_id in document
        .claims
        .iter()
        .filter_map(|record| clean_optional(&record.source_episode_ref))
        .chain(
            document
                .links
                .iter()
                .filter_map(|record| clean_optional(&record.source_episode_ref)),
        )
        .chain(
            document
                .procedures
                .iter()
                .filter_map(|record| clean_optional(&record.source_episode_ref)),
        )
        .chain(
            document
                .intentions
                .iter()
                .filter_map(|record| clean_optional(&record.source_episode_ref)),
        )
    {
        if known_refs.contains(&ref_id) {
            referenced_refs.insert(ref_id);
        }
    }

    referenced_refs.len()
}

fn require_text(issues: &mut Vec<IngestionIssue>, path: String, value: &str) {
    if value.trim().is_empty() {
        issues.push(issue(
            IngestionIssueKind::EmptyField,
            path,
            "field cannot be empty",
        ));
    }
}

fn require_known_ref(
    issues: &mut Vec<IngestionIssue>,
    path: String,
    value: &Option<String>,
    episode_refs: &HashSet<String>,
) {
    if let Some(ref_id) = clean_optional(value)
        && !episode_refs.contains(&ref_id)
    {
        issues.push(issue(
            IngestionIssueKind::UnknownSourceReference,
            path,
            format!("unknown episode reference {ref_id}"),
        ));
    }
}

fn issue(
    kind: IngestionIssueKind,
    path: impl Into<String>,
    message: impl Into<String>,
) -> IngestionIssue {
    IngestionIssue {
        kind,
        path: path.into(),
        message: message.into(),
    }
}

fn resolve_ref(episode_refs: &HashMap<String, String>, value: &Option<String>) -> Option<String> {
    clean_optional(value).and_then(|ref_id| episode_refs.get(&ref_id).cloned())
}

fn clean_optional(value: &Option<String>) -> Option<String> {
    value.as_ref().and_then(|value| {
        let value = value.trim().to_string();
        if value.is_empty() { None } else { Some(value) }
    })
}

fn clean_strings(values: &[String]) -> Vec<String> {
    values
        .iter()
        .filter_map(|value| {
            let value = value.trim().to_string();
            if value.is_empty() { None } else { Some(value) }
        })
        .collect()
}

fn source_byte_len(document: &MemoryIngestDocument) -> u64 {
    document
        .episodes
        .iter()
        .map(|episode| episode.content.len() as u64)
        .sum()
}

fn content_checksum(document: &MemoryIngestDocument) -> String {
    let mut bytes = Vec::new();
    push_field(&mut bytes, "version", document.version.to_string());
    push_field(&mut bytes, "kind", format!("{:?}", document.source.kind));
    push_optional(&mut bytes, "title", &document.source.title);
    push_optional(&mut bytes, "uri", &document.source.uri);
    for (key, value) in &document.source.metadata {
        push_field(&mut bytes, format!("metadata.{key}"), value);
    }
    for (index, episode) in document.episodes.iter().enumerate() {
        push_field(
            &mut bytes,
            format!("episode.{index}.content"),
            &episode.content,
        );
        push_optional(&mut bytes, format!("episode.{index}.ref"), &episode.ref_id);
        push_optional(
            &mut bytes,
            format!("episode.{index}.role"),
            &episode.source_role,
        );
        for tag in &episode.tags {
            push_field(&mut bytes, format!("episode.{index}.tag"), tag);
        }
        for mention in &episode.mentions {
            push_field(&mut bytes, format!("episode.{index}.mention"), mention);
        }
    }

    format!("fnv1a64:{:016x}", fnv1a64(&bytes))
}

fn push_optional(bytes: &mut Vec<u8>, key: impl AsRef<str>, value: &Option<String>) {
    if let Some(value) = clean_optional(value) {
        push_field(bytes, key, value);
    }
}

fn push_field(bytes: &mut Vec<u8>, key: impl AsRef<str>, value: impl AsRef<str>) {
    bytes.extend_from_slice(key.as_ref().as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(value.as_ref().as_bytes());
    bytes.push(0xff);
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    bytes.iter().fold(FNV_OFFSET, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
    })
}

fn default_source_kind() -> SourceKind {
    SourceKind::Document
}

fn default_confidence() -> f32 {
    0.8
}

fn default_procedure_kind() -> ProcedureKind {
    ProcedureKind::Procedure
}

fn default_intention_kind() -> IntentionKind {
    IntentionKind::Task
}

fn default_intention_priority() -> IntentionPriority {
    IntentionPriority::Medium
}

fn default_intention_status() -> IntentionStatus {
    IntentionStatus::Active
}

#[cfg(test)]
#[path = "ingestion_tests.rs"]
mod tests;
