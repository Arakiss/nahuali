use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    model::{
        IntentionKind, IntentionPriority, IntentionStatus, MemoryScope, ProcedureKind, SourceKind,
    },
    self_inspection::{SelfInspectionSummary, SelfInspectionWriteBackPolicy},
};

/// Current source-neutral memory interchange document version.
pub const MEMORY_INTERCHANGE_VERSION: u32 = 1;

/// Source-neutral memory document for import/export workflows.
///
/// This is not the record ledger and is not a projection snapshot. It is a stable
/// public bridge format for synthetic data, future private converters, and
/// cross-store transfers.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct MemoryInterchange {
    /// Interchange document version.
    pub version: u32,
    /// Source documents or transcripts represented by this import.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<InterchangeSource>,
    /// Ground-truth episodes to import.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub episodes: Vec<InterchangeEpisode>,
    /// Derived claims to import.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub claims: Vec<InterchangeClaim>,
    /// Derived links to import.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<InterchangeLink>,
    /// Procedures and preferences to import.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub procedures: Vec<InterchangeProcedure>,
    /// Future work, goals, reminders, and commitments to import.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub intentions: Vec<InterchangeIntention>,
}

impl Default for MemoryInterchange {
    fn default() -> Self {
        Self {
            version: MEMORY_INTERCHANGE_VERSION,
            sources: Vec::new(),
            episodes: Vec::new(),
            claims: Vec::new(),
            links: Vec::new(),
            procedures: Vec::new(),
            intentions: Vec::new(),
        }
    }
}

/// Source record in an interchange document.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct InterchangeSource {
    /// Local source reference used by episodes in the same document.
    #[serde(rename = "ref")]
    pub ref_id: String,
    /// Source category.
    #[serde(default = "default_source_kind")]
    pub kind: SourceKind,
    /// Human-readable title, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Source URI, path, or adapter-provided locator, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    /// Deterministic checksum over source material, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_checksum: Option<String>,
    /// Total bytes represented by the source, when known.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub byte_len: u64,
    /// Adapter-provided source metadata.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
    /// Explicit memory context boundary, when provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<MemoryScope>,
    /// Event timestamp in milliseconds since the Unix epoch, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp_ms: Option<u64>,
}

/// Episode record in an interchange document.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct InterchangeEpisode {
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
    /// Source-local role, speaker, or operator name, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_role: Option<String>,
    /// Local source reference from the same interchange document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<String>,
    /// Stable position within the source, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_position: Option<u32>,
    /// Explicit memory context boundary, when provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<MemoryScope>,
    /// Event timestamp in milliseconds since the Unix epoch, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp_ms: Option<u64>,
}

/// Claim record in an interchange document.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct InterchangeClaim {
    /// Entity or concept the assertion is about.
    pub subject: String,
    /// Relationship or attribute being asserted.
    pub predicate: String,
    /// Assertion value.
    pub object: String,
    /// Optional local episode reference from the same interchange document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_episode_ref: Option<String>,
    /// Caller-provided confidence; defaults to `0.8` when omitted.
    #[serde(default = "default_confidence")]
    pub confidence: f32,
    /// Explicit memory context boundary, when provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<MemoryScope>,
    /// Event timestamp in milliseconds since the Unix epoch, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp_ms: Option<u64>,
}

/// Link record in an interchange document.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct InterchangeLink {
    /// Source endpoint of the link.
    pub from: String,
    /// Link label.
    pub relation: String,
    /// Target endpoint of the link.
    pub to: String,
    /// Optional local episode reference from the same interchange document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_episode_ref: Option<String>,
    /// Caller-provided confidence; defaults to `0.8` when omitted.
    #[serde(default = "default_confidence")]
    pub confidence: f32,
    /// Explicit memory context boundary, when provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<MemoryScope>,
    /// Event timestamp in milliseconds since the Unix epoch, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp_ms: Option<u64>,
}

/// Procedure or preference record in an interchange document.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct InterchangeProcedure {
    /// Whether this record is a procedure or preference.
    #[serde(default = "default_procedure_kind")]
    pub kind: ProcedureKind,
    /// Human-readable procedure or preference name.
    pub name: String,
    /// Operational instruction or preference body.
    pub body: String,
    /// Optional local episode reference from the same interchange document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_episode_ref: Option<String>,
    /// Caller-provided confidence; defaults to `0.8` when omitted.
    #[serde(default = "default_confidence")]
    pub confidence: f32,
    /// Explicit memory context boundary, when provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<MemoryScope>,
    /// Event timestamp in milliseconds since the Unix epoch, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp_ms: Option<u64>,
}

/// Intention record in an interchange document.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct InterchangeIntention {
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
    /// Optional local episode reference from the same interchange document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_episode_ref: Option<String>,
    /// Optional reason for non-active lifecycle status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_reason: Option<String>,
    /// Explicit memory context boundary, when provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<MemoryScope>,
    /// Creation event timestamp in milliseconds since the Unix epoch, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp_ms: Option<u64>,
    /// Lifecycle event timestamp in milliseconds since the Unix epoch, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_timestamp_ms: Option<u64>,
}

/// Counts reported by an interchange import dry-run or execution.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct InterchangeImportCounts {
    /// Sources in the document.
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

impl InterchangeImportCounts {
    /// Return the total number of append-only events implied by the import.
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

#[path = "interchange_preflight.rs"]
mod preflight;
pub use preflight::InterchangeImportPreflight;

#[path = "interchange_readiness.rs"]
mod readiness;

#[path = "interchange_ops.rs"]
mod ops;
#[cfg(test)]
pub(crate) use ops::validate;
pub(crate) use ops::{export, import};

/// Validation report returned by an interchange import.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct InterchangeImportReport {
    /// Document version that was inspected.
    pub version: u32,
    /// Whether the document can be imported.
    pub valid: bool,
    /// Whether the import was a dry-run.
    pub dry_run: bool,
    /// Number of append-only events that would be written by a valid import.
    pub appendable_event_count: usize,
    /// Number of append-only events written by this execution.
    pub imported_event_count: usize,
    /// Source-neutral record counts.
    pub counts: InterchangeImportCounts,
    /// Scope, size, and evidence summary computed before import writes.
    pub preflight: InterchangeImportPreflight,
    /// Non-mutating self-inspection forecast for the incoming document.
    pub readiness: InterchangeImportReadiness,
    /// Validation issues found before import.
    pub issues: Vec<InterchangeIssue>,
}

/// Non-mutating migration-readiness forecast computed before import writes.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct InterchangeImportReadiness {
    /// Self-inspection summary projected from the incoming document.
    pub self_inspection_summary: SelfInspectionSummary,
    /// Number of proposed review items the incoming document would create.
    pub review_item_count: usize,
    /// Explicit write-back policy copied from the self-inspection forecast.
    pub write_back_policy: SelfInspectionWriteBackPolicy,
}

/// Validation issue found in an interchange document.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct InterchangeIssue {
    /// Machine-readable issue kind.
    pub kind: InterchangeIssueKind,
    /// Document path for the invalid field.
    pub path: String,
    /// Human-readable issue message.
    pub message: String,
}

/// Machine-readable interchange validation issue kind.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InterchangeIssueKind {
    /// Document version is not supported.
    UnsupportedVersion,
    /// A required text field is empty after trimming.
    EmptyField,
    /// A local episode reference is duplicated.
    DuplicateReference,
    /// A derived record cites an episode reference that does not exist.
    UnknownSourceReference,
    /// An episode cites a source reference that does not exist.
    UnknownSourceDocumentReference,
}

fn clean_optional(value: &Option<String>) -> Option<String> {
    value.as_ref().and_then(|value| {
        let value = value.trim().to_string();
        if value.is_empty() { None } else { Some(value) }
    })
}

fn default_confidence() -> f32 {
    0.8
}

fn default_source_kind() -> SourceKind {
    SourceKind::Other
}

fn is_zero(value: &u64) -> bool {
    *value == 0
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
#[path = "interchange_tests.rs"]
mod tests;
