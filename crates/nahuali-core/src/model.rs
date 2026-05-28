use std::{collections::BTreeMap, fmt, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::error::{NahualiError, Result};

/// Current deterministic projection schema version.
pub const MEMORY_DATA_VERSION: u32 = 6;

/// Fully projected memory state derived from an record ledger.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct MemoryData {
    /// Projection schema version.
    pub version: u32,
    /// Number of validated events included in the projection.
    pub event_count: usize,
    /// Identifier of the last event included in the projection.
    pub last_event_id: Option<String>,
    /// Source documents or transcripts that produced ingested memory.
    pub sources: Vec<SourceDocument>,
    /// Entities discovered from episode mentions, claims, and links.
    pub entities: Vec<Entity>,
    /// Ground-truth observed episodes.
    pub episodes: Vec<Episode>,
    /// Canonical derived assertions.
    pub claims: Vec<Claim>,
    /// Canonical derived typed connections.
    pub links: Vec<Link>,
    /// Reusable procedures and preferences.
    pub procedures: Vec<Procedure>,
    /// Future work, goals, reminders, and commitments.
    pub intentions: Vec<Intention>,
    /// Operator-approved review decisions over self-inspection findings.
    pub review_decisions: Vec<ReviewDecision>,
    /// Compatibility mirror for `claims`.
    pub facts: Vec<Fact>,
    /// Compatibility mirror for `links`.
    pub relations: Vec<Relation>,
}

impl Default for MemoryData {
    fn default() -> Self {
        Self {
            version: MEMORY_DATA_VERSION,
            event_count: 0,
            last_event_id: None,
            sources: Vec::new(),
            entities: Vec::new(),
            episodes: Vec::new(),
            claims: Vec::new(),
            links: Vec::new(),
            procedures: Vec::new(),
            intentions: Vec::new(),
            review_decisions: Vec::new(),
            facts: Vec::new(),
            relations: Vec::new(),
        }
    }
}

/// Explicit context boundary for memory items.
///
/// A scope is not an authorization boundary. It is durable metadata that lets
/// callers separate personal, project, organization, or custom memory contexts
/// before recall and review.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct MemoryScope {
    /// Scope category.
    pub kind: MemoryScopeKind,
    /// Human-readable operator-provided label.
    pub name: String,
    /// Deterministic normalized key used for equality and filtering.
    pub key: String,
}

impl MemoryScope {
    /// Build a deterministic scope from a kind and operator-provided name.
    pub fn new(kind: MemoryScopeKind, name: impl Into<String>) -> Result<Self> {
        let name = clean_scope_name(&name.into()).ok_or_else(|| NahualiError::InvalidScope {
            message: "scope name cannot be empty".to_string(),
        })?;
        let key = format!("{}:{}", kind.as_key(), scope_name_key(&name));
        Ok(Self { kind, name, key })
    }

    /// Parse the compact `kind:name` operator format.
    pub fn parse(value: &str) -> Result<Self> {
        value.parse()
    }
}

impl FromStr for MemoryScope {
    type Err = NahualiError;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        let (kind, name) = value
            .split_once(':')
            .ok_or_else(|| NahualiError::InvalidScope {
                message: "scope must use the kind:name format".to_string(),
            })?;
        let kind = kind.parse()?;
        Self::new(kind, name)
    }
}

impl fmt::Display for MemoryScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.key)
    }
}

/// Scope category for a memory item.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum MemoryScopeKind {
    /// A private personal memory context.
    Personal,
    /// A project-specific memory context.
    Project,
    /// An organization-level memory context.
    Organization,
    /// A caller-defined memory context.
    Custom,
}

impl MemoryScopeKind {
    /// Return the stable key prefix for this scope kind.
    pub fn as_key(&self) -> &'static str {
        match self {
            Self::Personal => "personal",
            Self::Project => "project",
            Self::Organization => "organization",
            Self::Custom => "custom",
        }
    }
}

impl FromStr for MemoryScopeKind {
    type Err = NahualiError;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "personal" => Ok(Self::Personal),
            "project" => Ok(Self::Project),
            "organization" | "org" => Ok(Self::Organization),
            "custom" => Ok(Self::Custom),
            other => Err(NahualiError::InvalidScope {
                message: format!("unsupported scope kind {other:?}"),
            }),
        }
    }
}

fn clean_scope_name(name: &str) -> Option<String> {
    let cleaned = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

fn scope_name_key(name: &str) -> String {
    let slug = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    if slug.is_empty() {
        "scope".to_string()
    } else {
        slug
    }
}

/// Source material registered for provenance.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SourceDocument {
    /// Stable source identifier.
    pub id: String,
    /// Event identifier that registered this source.
    pub event_id: String,
    /// Source category.
    pub kind: SourceKind,
    /// Human-readable title, when available.
    pub title: Option<String>,
    /// Source URI, path, or adapter-provided locator, when available.
    pub uri: Option<String>,
    /// Deterministic checksum over the ingested source material.
    pub content_checksum: String,
    /// Total number of content bytes represented by this source.
    pub byte_len: u64,
    /// Adapter-provided metadata preserved as source provenance.
    pub metadata: BTreeMap<String, String>,
    /// Explicit memory context boundary, when provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<MemoryScope>,
    /// Event timestamp in milliseconds since the Unix epoch.
    pub created_at_ms: u64,
}

/// Source category.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
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

/// Named thing known by the memory projection.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Entity {
    /// Stable entity identifier derived from the normalized name.
    pub id: String,
    /// First observed entity name.
    pub name: String,
    /// Number of projected mentions across claims, links, and episodes.
    pub mention_count: usize,
    /// Event timestamp for the first projected mention.
    pub first_seen_at_ms: u64,
    /// Event timestamp for the latest projected mention.
    pub last_seen_at_ms: u64,
    /// Event identifiers that mention this entity.
    pub source_event_ids: Vec<String>,
    /// Explicit memory context boundary, when provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<MemoryScope>,
}

/// Observed memory item stored as ground-truth context.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Episode {
    /// Stable episode identifier.
    pub id: String,
    /// Event identifier that created this episode.
    pub event_id: String,
    /// Natural-language content recorded for the episode.
    pub content: String,
    /// User-provided labels for filtering or recall.
    pub tags: Vec<String>,
    /// Explicit entity names mentioned by this episode.
    pub mentions: Vec<String>,
    /// Source record that produced this episode, when ingested from a source.
    pub source_id: Option<String>,
    /// Stable position within the source, when known.
    pub source_position: Option<u32>,
    /// Source-local actor, role, or speaker, when known.
    pub source_role: Option<String>,
    /// Explicit memory context boundary, when provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<MemoryScope>,
    /// Event timestamp in milliseconds since the Unix epoch.
    pub created_at_ms: u64,
}

/// Derived assertion about an entity or concept.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Claim {
    /// Stable claim identifier.
    pub id: String,
    /// Event identifier that created this claim.
    pub event_id: String,
    /// Entity or concept the assertion is about.
    pub subject: String,
    /// Relationship or attribute being asserted.
    pub predicate: String,
    /// Assertion value.
    pub object: String,
    /// Optional source episode that supports this assertion.
    pub source_episode_id: Option<String>,
    /// Caller-provided confidence after clamping to the `0.0..=1.0` range.
    pub confidence: f32,
    /// Explicit memory context boundary, when provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<MemoryScope>,
    /// Event timestamp in milliseconds since the Unix epoch.
    pub created_at_ms: u64,
}

/// Compatibility alias for the original assertion model name.
///
/// New public code should prefer [`Claim`]. This alias remains for record-ledger
/// compatibility and migration from the initial Rust foundation.
pub type Fact = Claim;

/// Derived typed connection between two entities or concepts.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Link {
    /// Stable link identifier.
    pub id: String,
    /// Event identifier that created this link.
    pub event_id: String,
    /// Source endpoint of the relation.
    pub from: String,
    /// Relation label.
    pub relation: String,
    /// Target endpoint of the relation.
    pub to: String,
    /// Optional source episode that supports this relation.
    pub source_episode_id: Option<String>,
    /// Caller-provided confidence after clamping to the `0.0..=1.0` range.
    pub confidence: f32,
    /// Explicit memory context boundary, when provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<MemoryScope>,
    /// Event timestamp in milliseconds since the Unix epoch.
    pub created_at_ms: u64,
}

/// Compatibility alias for the original relation model name.
///
/// New public code should prefer [`Link`]. This alias remains for record-ledger
/// compatibility and migration from the initial Rust foundation.
pub type Relation = Link;

/// Kind of reusable procedure.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProcedureKind {
    /// A reusable operational rule or workflow.
    Procedure,
    /// A behavioral preference that should guide future work.
    Preference,
}

/// Reusable behavioral rule or preference.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Procedure {
    /// Stable procedure identifier.
    pub id: String,
    /// Event identifier that created this procedure.
    pub event_id: String,
    /// Whether this memory is a procedure or a preference.
    pub kind: ProcedureKind,
    /// Human-readable procedure or preference name.
    pub name: String,
    /// Operational instruction or preference body.
    pub body: String,
    /// Optional source episode that supports this procedure.
    pub source_episode_id: Option<String>,
    /// Caller-provided confidence after clamping to the `0.0..=1.0` range.
    pub confidence: f32,
    /// Explicit memory context boundary, when provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<MemoryScope>,
    /// Event timestamp in milliseconds since the Unix epoch.
    pub created_at_ms: u64,
}

/// Intention category.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IntentionKind {
    /// A concrete task.
    Task,
    /// A broader goal.
    Goal,
    /// A reminder for future attention.
    Reminder,
}

/// Intention priority.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IntentionPriority {
    /// Low urgency.
    Low,
    /// Normal urgency.
    Medium,
    /// High urgency.
    High,
    /// Critical urgency.
    Critical,
}

/// Current lifecycle state for an intention.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IntentionStatus {
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

/// Future work, goal, reminder, or commitment.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Intention {
    /// Stable intention identifier.
    pub id: String,
    /// Event identifier that created this intention.
    pub event_id: String,
    /// Event identifier that last changed this intention.
    pub updated_event_id: String,
    /// Intention category.
    pub kind: IntentionKind,
    /// Current lifecycle state.
    pub status: IntentionStatus,
    /// Priority for recall and operator attention.
    pub priority: IntentionPriority,
    /// What should happen.
    pub description: String,
    /// Optional source episode that supports this intention.
    pub source_episode_id: Option<String>,
    /// Optional reason attached to the last lifecycle change.
    pub status_reason: Option<String>,
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
    /// Event timestamp in milliseconds since the Unix epoch.
    pub created_at_ms: u64,
    /// Timestamp for the latest lifecycle event.
    pub updated_at_ms: u64,
}

/// Operator-approved review decision over a self-inspection finding.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ReviewDecision {
    /// Stable review-decision identifier.
    pub id: String,
    /// Event identifier that created this review decision.
    pub event_id: String,
    /// Operator review item identifier that was handled.
    pub review_id: String,
    /// Self-inspection finding identifier that produced the review item.
    pub finding_id: String,
    /// Action that the operator reviewed.
    pub action: ReviewDecisionAction,
    /// Outcome selected by the operator.
    pub outcome: ReviewDecisionOutcome,
    /// Operator-supplied resolution note.
    pub note: String,
    /// Event or memory identifiers covered by this review decision.
    pub evidence_ids: Vec<String>,
    /// Explicit memory context boundary, when provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<MemoryScope>,
    /// Event timestamp in milliseconds since the Unix epoch.
    pub created_at_ms: u64,
}

/// Review action represented by an operator decision.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDecisionAction {
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

/// Review decision outcome.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDecisionOutcome {
    /// The operator resolved the finding and recorded the resolution.
    Resolved,
}

/// Kind of memory item returned by recall.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    /// A named entity.
    Entity,
    /// A ground-truth episode.
    Episode,
    /// A canonical derived assertion.
    Claim,
    /// A canonical typed connection.
    Link,
    /// A reusable behavioral rule or preference.
    Procedure,
    /// Future work, goal, reminder, or commitment.
    Intention,
    /// Compatibility kind for older callers; new recall results use `Claim`.
    Fact,
    /// Compatibility kind for older callers; new recall results use `Link`.
    Relation,
}

/// Query-local trust classification for one recall result.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecallResultTrustMode {
    /// This specific result is backed by source evidence and no result-local
    /// health signal currently weakens it.
    Certify,
    /// This result is observable but should not be treated as a supported
    /// assertion on its own.
    Advisory,
    /// This result is relevant but missing support or affected by medium-risk
    /// result-local health signals.
    Warn,
    /// This result is affected by high-risk result-local health signals.
    Block,
}

/// Query-local trust context for one recall result.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RecallResultTrust {
    /// Trust mode for this specific result, independent of store-level
    /// authority.
    pub mode: RecallResultTrustMode,
    /// Numeric result trust score in the `0.0..=1.0` range.
    pub score: f32,
    /// Whether callers may rely on this specific result without adding
    /// uncertainty.
    pub can_trust: bool,
    /// Human-readable reasons for this result trust decision.
    pub reasons: Vec<String>,
    /// Result-local health signal kinds that drove the decision.
    pub signal_kinds: Vec<String>,
}

/// Scored recall candidate.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RecallResult {
    /// Type of memory item that matched the query.
    pub kind: MemoryKind,
    /// Identifier of the matched memory item.
    pub id: String,
    /// Deterministic lexical score, including confidence and evidence bonuses
    /// for facts and relations.
    pub score: f32,
    /// Human-readable excerpt for display or agent context.
    pub excerpt: String,
    /// Optional evidence episode identifier.
    pub evidence_id: Option<String>,
    /// Normalized query terms found in the matched item.
    pub matched_terms: Vec<String>,
    /// Explicit memory context boundary, when provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<MemoryScope>,
    /// Query-local trust context, attached when callers request authority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust: Option<RecallResultTrust>,
}
