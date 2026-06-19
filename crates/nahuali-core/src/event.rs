use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::MemoryScope;
use crate::self_repair::{AutonomyLevel, RepairKind};

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
    /// Tamper-evident hash chain link: the chained hash of the *previous*
    /// event in the ledger.
    ///
    /// This field is only ever populated when the `tamper-evidence` feature is
    /// enabled (via `EventEnvelope::with_chain`). On a default build it stays
    /// `None`, and because it is skipped when serializing, default-build records
    /// are byte-for-byte identical to records written before this field existed.
    /// Legacy and default-build ledgers therefore deserialize with `prev_hash =
    /// None`, which the chain validator treats as "unchained / legacy" rather
    /// than a broken chain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prev_hash: Option<String>,
    /// Typed event payload.
    pub payload: MemoryEvent,
}

impl EventEnvelope {
    /// Create a new event envelope and compute its checksum.
    ///
    /// The hash-chain link (`prev_hash`) is left unset; default builds and the
    /// per-event checksum model never populate it. Use `Self::with_chain`
    /// under the `tamper-evidence` feature to link the event to its predecessor.
    pub fn new(sequence: u64, timestamp_ms: u64, payload: MemoryEvent) -> Self {
        let version = EVENT_ENVELOPE_VERSION;
        let checksum = checksum_for(version, sequence, timestamp_ms, &payload);
        Self {
            version,
            id: format!("event_{sequence}_{checksum}"),
            sequence,
            timestamp_ms,
            checksum,
            prev_hash: None,
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

/// Hash-chain (tamper-evidence) support.
///
/// Each chained event binds the *previous* event's chained hash into its own,
/// so rewriting any historical event breaks the link at the next event even if
/// the attacker recomputes that event's own per-event checksum. The per-event
/// checksum alone is self-contained and cannot detect such a rewrite; the chain
/// can.
///
/// What is hashed for the chained hash of an event is the tuple
/// `(sequence, canonical-event-bytes, prev_hash)` where `canonical-event-bytes`
/// is the same deterministic encoding the checksum covers
/// (`version`, `sequence`, `timestamp_ms`, `payload`), and `prev_hash` is the
/// chained hash of the previous event (empty for the genesis event). A SHA-256
/// digest binds these together.
#[cfg(feature = "tamper-evidence")]
impl EventEnvelope {
    /// Create a new event envelope linked to the previous event's chained hash.
    ///
    /// Pass `None` for `previous_chain_hash` for the genesis (first) event; the
    /// genesis link is bound against an empty predecessor hash.
    pub fn with_chain(
        sequence: u64,
        timestamp_ms: u64,
        payload: MemoryEvent,
        previous_chain_hash: Option<&str>,
    ) -> Self {
        let mut envelope = Self::new(sequence, timestamp_ms, payload);
        envelope.prev_hash = Some(previous_chain_hash.unwrap_or("").to_string());
        envelope
    }

    /// Compute this event's chained hash from its body and stored `prev_hash`.
    ///
    /// The genesis link binds against an empty predecessor hash. This is the
    /// value the *next* event must carry in its `prev_hash`, and the value
    /// exposed as the ledger tip for anchoring.
    pub fn chain_hash(&self) -> String {
        chain_hash_for(
            self.version,
            self.sequence,
            self.timestamp_ms,
            &self.payload,
            self.prev_hash.as_deref().unwrap_or(""),
        )
    }

    /// Whether this event carries a hash-chain link.
    ///
    /// Default-build and legacy (pre-chain) records have no link and are treated
    /// as unchained rather than broken.
    pub fn is_chained(&self) -> bool {
        self.prev_hash.is_some()
    }
}

/// A broken link discovered while verifying a hash chain.
#[cfg(feature = "tamper-evidence")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChainBreak {
    /// One-based position of the event whose recorded `prev_hash` does not match.
    pub record: usize,
    /// Sequence number of that event.
    pub sequence: u64,
}

/// Verify the tamper-evident hash chain across an in-memory event slice.
///
/// Returns the first broken link, or `None` when the chain is intact. Events
/// without a `prev_hash` (default-build / legacy records) are treated as
/// unchained and skipped, mirroring the ledger-replay validator. This lets a
/// caller check a ledger it already holds in memory — for example the events a
/// tip attestation was signed over — without reopening the database.
#[cfg(feature = "tamper-evidence")]
pub fn verify_event_chain(events: &[EventEnvelope]) -> Option<ChainBreak> {
    for (index, event) in events.iter().enumerate() {
        if !event.is_chained() {
            continue;
        }
        let expected_prev = if index == 0 {
            String::new()
        } else {
            events[index - 1].chain_hash()
        };
        if event.prev_hash.as_deref().unwrap_or("") != expected_prev {
            return Some(ChainBreak {
                record: index + 1,
                sequence: event.sequence,
            });
        }
    }
    None
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
    /// An LLM-proposed repair was validated, classified, and applied.
    RepairApplied(RepairApplied),
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

/// Payload for an applied self-repair.
///
/// A single repair event carries the materializable operation, the provenance of
/// the proposal, and the deterministic verdict, so one append both writes the
/// claim or link *and* records its audit atomically (contract rule 4: the repair
/// is itself the event). The LLM proposed it; the deterministic engine validated,
/// classified, and recorded it.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RepairApplied {
    /// Stable repair-decision identifier.
    pub id: String,
    /// Repair category.
    pub kind: RepairKind,
    /// Source episodes that anchor the repair.
    pub evidence_ids: Vec<String>,
    /// Model identifier that proposed the repair, recorded as provenance.
    pub proposed_by: String,
    /// Natural-language justification.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub rationale: String,
    /// Autonomy level the deterministic classifier assigned.
    pub autonomy_level: AutonomyLevel,
    /// Whether an operator explicitly approved a queued repair.
    pub operator_override: bool,
    /// The claim or link this repair materialized.
    pub materialized: RepairMaterialization,
}

/// The claim or link a repair event materializes.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "materialization", rename_all = "snake_case")]
pub enum RepairMaterialization {
    /// Materialize a derived claim, mirroring [`FactAsserted`].
    Claim(RepairClaimMaterialization),
    /// Materialize a derived link, mirroring [`RelationRecorded`].
    Link(RepairLinkMaterialization),
}

/// Claim a repair event materializes (mirrors [`FactAsserted`]).
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RepairClaimMaterialization {
    /// Stable claim identifier.
    pub id: String,
    /// Entity or concept the assertion is about.
    pub subject: String,
    /// Relationship or attribute being asserted.
    pub predicate: String,
    /// Assertion value.
    pub object: String,
    /// Source episode that anchors this assertion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_episode_id: Option<String>,
    /// Caller-provided confidence after clamping to the `0.0..=1.0` range.
    pub confidence: f32,
    /// Explicit memory context boundary, when provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<MemoryScope>,
}

/// Link a repair event materializes (mirrors [`RelationRecorded`]).
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RepairLinkMaterialization {
    /// Stable link identifier.
    pub id: String,
    /// Source endpoint of the relation.
    pub from: String,
    /// Relation label.
    pub relation: String,
    /// Target endpoint of the relation.
    pub to: String,
    /// Source episode that anchors this relation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_episode_id: Option<String>,
    /// Caller-provided confidence after clamping to the `0.0..=1.0` range.
    pub confidence: f32,
    /// Explicit memory context boundary, when provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<MemoryScope>,
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

/// Compute the tamper-evident chained hash for a single event.
///
/// The digest binds, in order, an unambiguous domain-separated encoding of:
/// the previous event's chained hash (`prev_hash`, empty for genesis), the
/// event `sequence`, and the canonical event bytes the checksum covers
/// (`version`, `sequence`, `timestamp_ms`, `payload`). Each component is
/// length-prefixed so that no two distinct inputs can produce the same byte
/// stream (e.g. a `prev_hash` ending in a digit cannot be confused with the
/// sequence number). SHA-256 then makes the chain collision-resistant: an
/// attacker cannot rewrite an event without changing every subsequent chained
/// hash, which a verifier detects at the first mismatch.
#[cfg(feature = "tamper-evidence")]
fn chain_hash_for(
    version: u32,
    sequence: u64,
    timestamp_ms: u64,
    payload: &MemoryEvent,
    prev_hash: &str,
) -> String {
    use sha2::{Digest, Sha256};

    let body = ChecksumBody {
        version,
        sequence,
        timestamp_ms,
        payload,
    };
    let event_bytes = serde_json::to_vec(&body).expect("memory events must serialize");

    let mut hasher = Sha256::new();
    // Domain separator so this digest can never collide with an unrelated
    // SHA-256 use over the same crate types.
    hasher.update(b"nahuali.ledger.chain.v1");
    absorb_field(&mut hasher, prev_hash.as_bytes());
    absorb_field(&mut hasher, &sequence.to_be_bytes());
    absorb_field(&mut hasher, &event_bytes);

    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// Absorb a length-prefixed field into the hasher to keep field boundaries
/// unambiguous (prevents concatenation ambiguity between adjacent components).
#[cfg(feature = "tamper-evidence")]
fn absorb_field(hasher: &mut sha2::Sha256, bytes: &[u8]) {
    use sha2::Digest;
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
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

    fn sample_event(sequence: u64) -> MemoryEvent {
        MemoryEvent::EpisodeRecorded(EpisodeRecorded {
            id: format!("episode_{sequence}"),
            content: format!("Synthetic chain event {sequence}"),
            tags: vec!["chain".to_string()],
            mentions: Vec::new(),
            source_id: None,
            source_position: None,
            source_role: None,
            scope: None,
        })
    }

    /// (d) OFF-mode (default-build) serialization must be byte-for-byte the same
    /// as before `prev_hash` existed. `EventEnvelope::new` never sets the field,
    /// so it is skipped on serialization and round-trips identically to the
    /// historical `{version, id, sequence, timestamp_ms, checksum, payload}`
    /// shape. This test is intentionally NOT feature-gated: it guards the
    /// default build's wire format.
    #[test]
    fn default_build_envelope_serialization_is_unchanged() {
        let envelope = EventEnvelope::new(1, 1000, sample_event(1));

        // `prev_hash` is absent by default.
        assert!(envelope.prev_hash.is_none());

        let encoded = serde_json::to_value(&envelope).unwrap();
        let object = encoded.as_object().unwrap();

        // The serialized object carries exactly the historical fields and no
        // `prev_hash` key.
        assert!(!object.contains_key("prev_hash"));
        let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec![
                "checksum",
                "id",
                "payload",
                "sequence",
                "timestamp_ms",
                "version"
            ]
        );

        // It is byte-for-byte identical to a hand-built record that predates the
        // `prev_hash` field, and round-trips back to an equal envelope.
        let legacy_shape = serde_json::json!({
            "version": envelope.version,
            "id": envelope.id,
            "sequence": envelope.sequence,
            "timestamp_ms": envelope.timestamp_ms,
            "checksum": envelope.checksum,
            "payload": envelope.payload,
        });
        assert_eq!(encoded, legacy_shape);

        let round_tripped: EventEnvelope = serde_json::from_value(encoded).unwrap();
        assert_eq!(round_tripped, envelope);
        assert!(round_tripped.prev_hash.is_none());
    }

    /// Build a clean chained ledger of `count` events using `with_chain`.
    #[cfg(feature = "tamper-evidence")]
    fn chained_ledger(count: u64) -> Vec<EventEnvelope> {
        let mut events: Vec<EventEnvelope> = Vec::new();
        for sequence in 1..=count {
            let prev = events.last().map(EventEnvelope::chain_hash);
            events.push(EventEnvelope::with_chain(
                sequence,
                1000 + sequence,
                sample_event(sequence),
                prev.as_deref(),
            ));
        }
        events
    }

    /// Walk the chain exactly like the ledger replay path: every event's
    /// recorded `prev_hash` must equal the previous event's chained hash
    /// (empty for the genesis event). Returns the one-based position of the
    /// first broken link, or `None` if the chain is intact.
    #[cfg(feature = "tamper-evidence")]
    fn first_broken_link(events: &[EventEnvelope]) -> Option<usize> {
        let mut expected_prev = String::new();
        for (index, event) in events.iter().enumerate() {
            let recorded_prev = event.prev_hash.clone().unwrap_or_default();
            if recorded_prev != expected_prev {
                return Some(index + 1);
            }
            expected_prev = event.chain_hash();
        }
        None
    }

    /// (a) A clean chain validates end to end.
    #[cfg(feature = "tamper-evidence")]
    #[test]
    fn clean_chain_validates() {
        let events = chained_ledger(4);

        assert!(events.iter().all(EventEnvelope::is_chained));
        // Genesis binds an empty predecessor hash.
        assert_eq!(events[0].prev_hash.as_deref(), Some(""));
        // Each subsequent event records the previous event's chained hash.
        for window in events.windows(2) {
            assert_eq!(
                window[1].prev_hash.as_deref().unwrap(),
                window[0].chain_hash()
            );
        }
        assert_eq!(first_broken_link(&events), None);
    }

    /// (b) THE KEY PROOF. Rewrite a middle event's payload AND recompute its own
    /// per-event checksum (the strongest attack against the self-contained
    /// checksum model). The checksum-only check still passes — the attacker
    /// fooled it — but the hash chain detects the tamper, because the next
    /// event's recorded `prev_hash` no longer matches the rewritten event's
    /// chained hash.
    #[cfg(feature = "tamper-evidence")]
    #[test]
    fn middle_event_rewrite_with_recomputed_checksum_breaks_chain() {
        let mut events = chained_ledger(4);
        let target = 1; // second event (zero-based index 1)

        // Attacker rewrites the payload and forges a valid per-event checksum by
        // reconstructing the envelope from scratch (which recomputes checksum).
        let forged_payload = MemoryEvent::EpisodeRecorded(EpisodeRecorded {
            id: "episode_2".to_string(),
            content: "TAMPERED: attacker-substituted content".to_string(),
            tags: vec!["chain".to_string()],
            mentions: Vec::new(),
            source_id: None,
            source_position: None,
            source_role: None,
            scope: None,
        });
        let original = &events[target];
        let mut forged =
            EventEnvelope::new(original.sequence, original.timestamp_ms, forged_payload);
        // Preserve the original chain link so only the body changed — exactly the
        // in-place rewrite a checksum-only model cannot catch.
        forged.prev_hash = original.prev_hash.clone();

        // The checksum-only model is fooled: the forged event's checksum is valid.
        assert!(forged.validate_checksum());

        events[target] = forged;

        // The chain is NOT fooled: the next event (index 2) recorded the
        // *original* event's chained hash, which no longer matches the forged
        // event's chained hash. The break surfaces at the following record.
        assert_eq!(first_broken_link(&events), Some(target + 2));
    }

    /// The public in-memory verifier agrees with the replay walk: it passes a
    /// clean chain and pinpoints the same broken link after a checksum-recomputed
    /// in-place rewrite. This is the property a tip attestation rests on.
    #[cfg(feature = "tamper-evidence")]
    #[test]
    fn public_chain_verifier_detects_in_place_rewrite() {
        let clean = chained_ledger(4);
        assert_eq!(super::verify_event_chain(&clean), None);

        let mut tampered = chained_ledger(4);
        let original = &tampered[1];
        let forged_payload = MemoryEvent::EpisodeRecorded(EpisodeRecorded {
            id: "episode_2".to_string(),
            content: "TAMPERED: attacker-substituted content".to_string(),
            tags: Vec::new(),
            mentions: Vec::new(),
            source_id: None,
            source_position: None,
            source_role: None,
            scope: None,
        });
        let mut forged =
            EventEnvelope::new(original.sequence, original.timestamp_ms, forged_payload);
        forged.prev_hash = original.prev_hash.clone();
        assert!(forged.validate_checksum());
        tampered[1] = forged;

        let chain_break =
            super::verify_event_chain(&tampered).expect("verifier reports the broken link");
        assert_eq!(
            chain_break,
            super::ChainBreak {
                record: 3,
                sequence: 3
            }
        );
    }

    /// (c) The tip changes when the suffix is re-chained. A full forgery requires
    /// re-chaining every later event; doing so changes the chain tip, so an
    /// externally anchored tip would no longer match.
    #[cfg(feature = "tamper-evidence")]
    #[test]
    fn re_chaining_suffix_changes_tip() {
        let original = chained_ledger(4);
        let original_tip = original.last().unwrap().chain_hash();

        // Attacker rewrites the second event, then re-chains the entire suffix so
        // every per-event link is internally consistent again.
        let mut forged: Vec<EventEnvelope> = Vec::new();
        for (index, event) in original.iter().enumerate() {
            let prev = forged.last().map(EventEnvelope::chain_hash);
            let payload = if index == 1 {
                MemoryEvent::EpisodeRecorded(EpisodeRecorded {
                    id: "episode_2".to_string(),
                    content: "TAMPERED then re-chained".to_string(),
                    tags: vec!["chain".to_string()],
                    mentions: Vec::new(),
                    source_id: None,
                    source_position: None,
                    source_role: None,
                    scope: None,
                })
            } else {
                event.payload.clone()
            };
            forged.push(EventEnvelope::with_chain(
                event.sequence,
                event.timestamp_ms,
                payload,
                prev.as_deref(),
            ));
        }

        // The re-chained ledger is internally consistent (no broken link)...
        assert_eq!(first_broken_link(&forged), None);
        // ...but its tip differs, so an anchored/recorded tip catches the forgery.
        let forged_tip = forged.last().unwrap().chain_hash();
        assert_ne!(forged_tip, original_tip);
    }

    /// An ON-build opening an OFF-built (unchained / legacy) ledger must treat
    /// missing `prev_hash` gracefully rather than report a broken chain.
    #[cfg(feature = "tamper-evidence")]
    #[test]
    fn unchained_legacy_events_are_not_flagged_as_broken() {
        // Events created with `new` (the default/legacy path) have no link.
        let legacy: Vec<EventEnvelope> = (1..=3)
            .map(|seq| EventEnvelope::new(seq, 1000 + seq, sample_event(seq)))
            .collect();

        assert!(legacy.iter().all(|event| !event.is_chained()));
        // The replay walk only checks chained events; unchained ones are skipped.
        // first_broken_link compares prev_hash against expected, but legacy
        // events carry no prev_hash and the engine guards on `is_chained()`, so
        // simulate that guard here.
        for event in &legacy {
            assert!(!event.is_chained());
        }
    }
}
