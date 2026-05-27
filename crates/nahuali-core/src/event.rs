use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::MemoryScope;

/// Current event-envelope format written by `nahuali-core`.
pub const EVENT_ENVELOPE_VERSION: u32 = 1;

/// Validated record-ledger entry.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct EventEnvelope {
    /// Event-envelope format version.
    #[serde(default = "default_event_version")]
    pub version: u32,
    /// Stable event identifier derived from the sequence and checksum.
    pub id: String,
    /// Monotonic event sequence number, starting at `1`.
    pub sequence: u64,
    /// Event timestamp in milliseconds since the Unix epoch.
    pub timestamp_ms: u64,
    /// Deterministic checksum over the version, sequence, timestamp, and payload.
    pub checksum: String,
    /// Typed event payload.
    pub payload: MemoryEvent,
}

impl EventEnvelope {
    /// Create a new event envelope and compute its checksum.
    pub fn new(sequence: u64, timestamp_ms: u64, payload: MemoryEvent) -> Self {
        let version = EVENT_ENVELOPE_VERSION;
        let checksum = checksum_for(version, sequence, timestamp_ms, &payload);
        Self {
            version,
            id: format!("event_{sequence}_{checksum}"),
            sequence,
            timestamp_ms,
            checksum,
            payload,
        }
    }

    /// Return whether the stored checksum matches the event body.
    pub fn validate_checksum(&self) -> bool {
        self.checksum
            == checksum_for(
                self.version,
                self.sequence,
                self.timestamp_ms,
                &self.payload,
            )
            || (self.version == EVENT_ENVELOPE_VERSION
                && self.checksum
                    == legacy_checksum_for(self.sequence, self.timestamp_ms, &self.payload))
    }
}

/// Typed event payload stored in the record ledger.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MemoryEvent {
    /// External source material was registered for provenance.
    SourceRecorded(SourceRecorded),
    /// Ground-truth episode was recorded.
    EpisodeRecorded(EpisodeRecorded),
    /// Derived fact was asserted.
    FactAsserted(FactAsserted),
    /// Derived relation was recorded.
    RelationRecorded(RelationRecorded),
    /// Reusable procedure or preference was recorded.
    ProcedureRecorded(ProcedureRecorded),
    /// Intention was recorded.
    IntentionRecorded(IntentionRecorded),
    /// Intention metadata was updated.
    IntentionUpdated(IntentionUpdated),
    /// Intention lifecycle state was changed.
    IntentionStatusChanged(IntentionStatusChanged),
    /// Operator reviewed and resolved a self-inspection item.
    ReviewRecorded(ReviewRecorded),
}

/// Payload for a source-recorded event.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SourceRecorded {
    /// Stable source identifier.
    pub id: String,
    /// Source category.
    pub kind: SourceRecordedKind,
    /// Human-readable title, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Source URI, path, or adapter-provided locator, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    /// Deterministic checksum over the ingested source material.
    pub content_checksum: String,
    /// Total number of content bytes represented by this source.
    pub byte_len: u64,
    /// Adapter-provided metadata preserved as source provenance.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
    /// Explicit memory context boundary, when provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<MemoryScope>,
}

/// Source category stored in the record ledger.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceRecordedKind {
    /// Generic document or note.
    Document,
    /// Conversation or chat transcript.
    Conversation,
    /// Meeting transcript.
    Transcript,
    /// Web page or URL-derived document.
    WebPage,
    /// Local note.
    Note,
    /// Source kind not modeled by this release.
    Other,
}

/// Payload for an episode-recorded event.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct EpisodeRecorded {
    /// Stable episode identifier.
    pub id: String,
    /// Natural-language content recorded for the episode.
    pub content: String,
    /// User-provided labels for filtering or recall.
    pub tags: Vec<String>,
    /// Explicit entity names mentioned by this episode.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mentions: Vec<String>,
    /// Source record that produced this episode, when ingested from a source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    /// Stable position within the source, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_position: Option<u32>,
    /// Source-local actor, role, or speaker, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_role: Option<String>,
    /// Explicit memory context boundary, when provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<MemoryScope>,
}

/// Payload for a fact-asserted event.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct FactAsserted {
    /// Stable fact identifier.
    pub id: String,
    /// Entity or concept the assertion is about.
    pub subject: String,
    /// Relationship or attribute being asserted.
    pub predicate: String,
    /// Assertion value.
    pub object: String,
    /// Optional source episode that supports this assertion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_episode_id: Option<String>,
    /// Caller-provided confidence after clamping to the `0.0..=1.0` range.
    pub confidence: f32,
    /// Explicit memory context boundary, when provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<MemoryScope>,
}

/// Payload for a relation-recorded event.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RelationRecorded {
    /// Stable relation identifier.
    pub id: String,
    /// Source endpoint of the relation.
    pub from: String,
    /// Relation label.
    pub relation: String,
    /// Target endpoint of the relation.
    pub to: String,
    /// Optional source episode that supports this relation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_episode_id: Option<String>,
    /// Caller-provided confidence after clamping to the `0.0..=1.0` range.
    pub confidence: f32,
    /// Explicit memory context boundary, when provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<MemoryScope>,
}

/// Kind of reusable procedure payload.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProcedureRecordedKind {
    /// A reusable operational rule or workflow.
    Procedure,
    /// A behavioral preference that should guide future work.
    Preference,
}

/// Payload for a procedure-recorded event.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ProcedureRecorded {
    /// Stable procedure identifier.
    pub id: String,
    /// Whether this record is a procedure or preference.
    pub kind: ProcedureRecordedKind,
    /// Human-readable procedure or preference name.
    pub name: String,
    /// Operational instruction or preference body.
    pub body: String,
    /// Optional source episode that supports this procedure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_episode_id: Option<String>,
    /// Caller-provided confidence after clamping to the `0.0..=1.0` range.
    pub confidence: f32,
    /// Explicit memory context boundary, when provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<MemoryScope>,
}

/// Intention category payload.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IntentionRecordedKind {
    /// A concrete task.
    Task,
    /// A broader goal.
    Goal,
    /// A reminder for future attention.
    Reminder,
}

/// Intention priority payload.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IntentionRecordedPriority {
    /// Low urgency.
    Low,
    /// Normal urgency.
    Medium,
    /// High urgency.
    High,
    /// Critical urgency.
    Critical,
}

/// Intention lifecycle status payload.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IntentionRecordedStatus {
    /// Open and actionable.
    Active,
    /// Finished successfully.
    Completed,
    /// Intentionally dropped.
    Abandoned,
    /// Cannot progress right now.
    Blocked,
    /// Postponed for later.
    Deferred,
}

/// Payload for an intention-recorded event.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct IntentionRecorded {
    /// Stable intention identifier.
    pub id: String,
    /// Intention category.
    pub kind: IntentionRecordedKind,
    /// Intention priority.
    pub priority: IntentionRecordedPriority,
    /// What should happen.
    pub description: String,
    /// Optional source episode that supports this intention.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_episode_id: Option<String>,
    /// Deadline or commitment timestamp in milliseconds since the Unix epoch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline_at_ms: Option<u64>,
    /// Intention identifiers that must complete before this item can proceed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
    /// Parent goal intention identifier, when this item contributes to a goal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal_id: Option<String>,
    /// Operator-supplied progress estimate from 0 to 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress_percent: Option<u8>,
    /// Explicit memory context boundary, when provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<MemoryScope>,
}

/// Payload for an intention metadata update event.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct IntentionUpdated {
    /// Stable intention identifier.
    pub id: String,
    /// New description, when changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// New priority, when changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<IntentionRecordedPriority>,
    /// Deadline update. `Some(null)` clears the deadline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline_at_ms: Option<Option<u64>>,
    /// Full dependency replacement. An empty list clears dependencies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depends_on: Option<Vec<String>>,
    /// Parent goal update. `Some(null)` clears the parent goal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal_id: Option<Option<String>>,
    /// Progress update. `Some(null)` clears progress.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress_percent: Option<Option<u8>>,
}

/// Payload for an intention lifecycle event.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct IntentionStatusChanged {
    /// Stable intention identifier.
    pub id: String,
    /// New lifecycle status.
    pub status: IntentionRecordedStatus,
    /// Optional reason for the lifecycle change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Payload for an operator review decision.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ReviewRecorded {
    /// Stable review-decision identifier.
    pub id: String,
    /// Operator review item identifier that was handled.
    pub review_id: String,
    /// Self-inspection finding identifier that produced the review item.
    pub finding_id: String,
    /// Review action handled by the operator.
    pub action: ReviewRecordedAction,
    /// Operator-selected outcome.
    pub outcome: ReviewRecordedOutcome,
    /// Operator-supplied resolution note.
    pub note: String,
    /// Event or memory identifiers covered by this review decision.
    pub evidence_ids: Vec<String>,
    /// Explicit memory context boundary, when provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<MemoryScope>,
}

/// Review action stored in the record ledger.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewRecordedAction {
    /// Missing evidence was reviewed.
    CaptureEvidence,
    /// A contradiction was reviewed.
    ResolveContradiction,
    /// Stale memory was reviewed.
    RefreshMemory,
    /// Disconnected memory was reviewed.
    LinkMemory,
    /// A repeated pattern was reviewed.
    ConsolidatePattern,
    /// A latent intention was reviewed.
    ReviewIntention,
}

/// Review decision outcome stored in the record ledger.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewRecordedOutcome {
    /// The operator resolved the finding and recorded the resolution.
    Resolved,
}

#[derive(Serialize)]
struct ChecksumBody<'a> {
    version: u32,
    sequence: u64,
    timestamp_ms: u64,
    payload: &'a MemoryEvent,
}

#[derive(Serialize)]
struct LegacyChecksumBody<'a> {
    sequence: u64,
    timestamp_ms: u64,
    payload: &'a MemoryEvent,
}

fn default_event_version() -> u32 {
    EVENT_ENVELOPE_VERSION
}

fn checksum_for(version: u32, sequence: u64, timestamp_ms: u64, payload: &MemoryEvent) -> String {
    let body = ChecksumBody {
        version,
        sequence,
        timestamp_ms,
        payload,
    };
    let encoded = serde_json::to_vec(&body).expect("memory events must serialize");
    format!("{:016x}", fnv1a64(&encoded))
}

fn legacy_checksum_for(sequence: u64, timestamp_ms: u64, payload: &MemoryEvent) -> String {
    let body = LegacyChecksumBody {
        sequence,
        timestamp_ms,
        payload,
    };
    let encoded = serde_json::to_vec(&body).expect("memory events must serialize");
    format!("{:016x}", fnv1a64(&encoded))
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    bytes.iter().fold(FNV_OFFSET, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
    })
}

#[cfg(test)]
mod tests {
    use super::{
        EVENT_ENVELOPE_VERSION, EpisodeRecorded, EventEnvelope, MemoryEvent, legacy_checksum_for,
    };

    #[test]
    fn event_checksum_is_deterministic() {
        let payload = MemoryEvent::EpisodeRecorded(EpisodeRecorded {
            id: "episode_1".to_string(),
            content: "Lena prefers concise release notes.".to_string(),
            tags: vec!["example".to_string()],
            mentions: Vec::new(),
            source_id: None,
            source_position: None,
            source_role: None,
            scope: None,
        });

        let first = EventEnvelope::new(1, 1000, payload.clone());
        let second = EventEnvelope::new(1, 1000, payload);

        assert_eq!(first, second);
        assert_eq!(first.version, EVENT_ENVELOPE_VERSION);
        assert!(first.validate_checksum());
        assert!(first.id.starts_with("event_1_"));
    }

    #[test]
    fn serialized_event_envelopes_include_version() {
        let event = EventEnvelope::new(
            1,
            1000,
            MemoryEvent::EpisodeRecorded(EpisodeRecorded {
                id: "episode_1".to_string(),
                content: "Lena prefers concise release notes.".to_string(),
                tags: vec!["example".to_string()],
                mentions: Vec::new(),
                source_id: None,
                source_position: None,
                source_role: None,
                scope: None,
            }),
        );

        let encoded = serde_json::to_value(event).unwrap();

        assert_eq!(encoded["version"], EVENT_ENVELOPE_VERSION);
    }

    #[test]
    fn pre_version_one_event_envelopes_remain_readable() {
        let payload = MemoryEvent::EpisodeRecorded(EpisodeRecorded {
            id: "episode_1".to_string(),
            content: "Lena prefers concise release notes.".to_string(),
            tags: vec!["example".to_string()],
            mentions: Vec::new(),
            source_id: None,
            source_position: None,
            source_role: None,
            scope: None,
        });
        let checksum = legacy_checksum_for(1, 1000, &payload);
        let encoded = serde_json::json!({
            "id": format!("event_1_{checksum}"),
            "sequence": 1,
            "timestamp_ms": 1000,
            "checksum": checksum,
            "payload": payload,
        });

        let decoded: EventEnvelope = serde_json::from_value(encoded).unwrap();

        assert_eq!(decoded.version, EVENT_ENVELOPE_VERSION);
        assert!(decoded.validate_checksum());
    }
}
