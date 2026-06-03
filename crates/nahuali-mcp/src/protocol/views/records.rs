use nahuali_core::{
    Claim, Entity, Episode, Fact, Intention, Link, MemoryScope, Procedure, RecallResult,
    RecordLedgerIssue, Relation,
};
use rmcp::schemars;
use serde::Serialize;

use super::json_string;

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct RecordLedgerIssueView {
    kind: String,
    severity: String,
    line: Option<usize>,
    message: String,
}

impl From<RecordLedgerIssue> for RecordLedgerIssueView {
    fn from(issue: RecordLedgerIssue) -> Self {
        Self {
            kind: json_string(&issue.kind),
            severity: json_string(&issue.severity),
            line: issue.line,
            message: issue.message,
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct ScopeView {
    kind: String,
    name: String,
    key: String,
}

impl From<MemoryScope> for ScopeView {
    fn from(scope: MemoryScope) -> Self {
        Self {
            kind: json_string(&scope.kind),
            name: scope.name,
            key: scope.key,
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct EpisodeView {
    id: String,
    event_id: String,
    content: String,
    tags: Vec<String>,
    mentions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<ScopeView>,
    created_at_ms: u64,
}

impl From<Episode> for EpisodeView {
    fn from(episode: Episode) -> Self {
        Self {
            id: episode.id,
            event_id: episode.event_id,
            content: episode.content,
            tags: episode.tags,
            mentions: episode.mentions,
            scope: episode.scope.map(ScopeView::from),
            created_at_ms: episode.created_at_ms,
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct EntityView {
    id: String,
    name: String,
    mention_count: usize,
    first_seen_at_ms: u64,
    last_seen_at_ms: u64,
    source_event_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<ScopeView>,
}

impl From<Entity> for EntityView {
    fn from(entity: Entity) -> Self {
        Self {
            id: entity.id,
            name: entity.name,
            mention_count: entity.mention_count,
            first_seen_at_ms: entity.first_seen_at_ms,
            last_seen_at_ms: entity.last_seen_at_ms,
            source_event_ids: entity.source_event_ids,
            scope: entity.scope.map(ScopeView::from),
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct ClaimView {
    id: String,
    event_id: String,
    subject: String,
    predicate: String,
    object: String,
    source_episode_id: Option<String>,
    confidence: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<ScopeView>,
    created_at_ms: u64,
}

impl From<Claim> for ClaimView {
    fn from(claim: Claim) -> Self {
        Self {
            id: claim.id,
            event_id: claim.event_id,
            subject: claim.subject,
            predicate: claim.predicate,
            object: claim.object,
            source_episode_id: claim.source_episode_id,
            confidence: claim.confidence,
            scope: claim.scope.map(ScopeView::from),
            created_at_ms: claim.created_at_ms,
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct FactView {
    id: String,
    event_id: String,
    subject: String,
    predicate: String,
    object: String,
    source_episode_id: Option<String>,
    confidence: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<ScopeView>,
    created_at_ms: u64,
}

impl From<Fact> for FactView {
    fn from(fact: Fact) -> Self {
        Self {
            id: fact.id,
            event_id: fact.event_id,
            subject: fact.subject,
            predicate: fact.predicate,
            object: fact.object,
            source_episode_id: fact.source_episode_id,
            confidence: fact.confidence,
            scope: fact.scope.map(ScopeView::from),
            created_at_ms: fact.created_at_ms,
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct LinkView {
    id: String,
    event_id: String,
    from: String,
    relation: String,
    to: String,
    source_episode_id: Option<String>,
    confidence: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<ScopeView>,
    created_at_ms: u64,
}

impl From<Link> for LinkView {
    fn from(link: Link) -> Self {
        Self {
            id: link.id,
            event_id: link.event_id,
            from: link.from,
            relation: link.relation,
            to: link.to,
            source_episode_id: link.source_episode_id,
            confidence: link.confidence,
            scope: link.scope.map(ScopeView::from),
            created_at_ms: link.created_at_ms,
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct RelationView {
    id: String,
    event_id: String,
    from: String,
    relation: String,
    to: String,
    source_episode_id: Option<String>,
    confidence: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<ScopeView>,
    created_at_ms: u64,
}

impl From<Relation> for RelationView {
    fn from(relation: Relation) -> Self {
        Self {
            id: relation.id,
            event_id: relation.event_id,
            from: relation.from,
            relation: relation.relation,
            to: relation.to,
            source_episode_id: relation.source_episode_id,
            confidence: relation.confidence,
            scope: relation.scope.map(ScopeView::from),
            created_at_ms: relation.created_at_ms,
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct ProcedureView {
    id: String,
    event_id: String,
    kind: String,
    name: String,
    body: String,
    source_episode_id: Option<String>,
    confidence: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<ScopeView>,
    created_at_ms: u64,
}

impl From<Procedure> for ProcedureView {
    fn from(procedure: Procedure) -> Self {
        Self {
            kind: json_string(&procedure.kind),
            id: procedure.id,
            event_id: procedure.event_id,
            name: procedure.name,
            body: procedure.body,
            source_episode_id: procedure.source_episode_id,
            confidence: procedure.confidence,
            scope: procedure.scope.map(ScopeView::from),
            created_at_ms: procedure.created_at_ms,
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct IntentionView {
    id: String,
    event_id: String,
    updated_event_id: String,
    kind: String,
    status: String,
    priority: String,
    description: String,
    source_episode_id: Option<String>,
    status_reason: Option<String>,
    deadline_at_ms: Option<u64>,
    depends_on: Vec<String>,
    goal_id: Option<String>,
    progress_percent: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<ScopeView>,
    created_at_ms: u64,
    updated_at_ms: u64,
}

impl From<Intention> for IntentionView {
    fn from(intention: Intention) -> Self {
        Self {
            kind: json_string(&intention.kind),
            status: json_string(&intention.status),
            priority: json_string(&intention.priority),
            id: intention.id,
            event_id: intention.event_id,
            updated_event_id: intention.updated_event_id,
            description: intention.description,
            source_episode_id: intention.source_episode_id,
            status_reason: intention.status_reason,
            deadline_at_ms: intention.deadline_at_ms,
            depends_on: intention.depends_on,
            goal_id: intention.goal_id,
            progress_percent: intention.progress_percent,
            scope: intention.scope.map(ScopeView::from),
            created_at_ms: intention.created_at_ms,
            updated_at_ms: intention.updated_at_ms,
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct RecallResultView {
    kind: String,
    id: String,
    score: f32,
    excerpt: String,
    evidence_id: Option<String>,
    matched_terms: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<ScopeView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    trust: Option<RecallResultTrustView>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct RecallResultTrustView {
    mode: String,
    score: f32,
    can_trust: bool,
    reasons: Vec<String>,
    signal_kinds: Vec<String>,
}

impl From<RecallResult> for RecallResultView {
    fn from(result: RecallResult) -> Self {
        Self {
            kind: json_string(&result.kind),
            id: result.id,
            score: result.score,
            excerpt: result.excerpt,
            evidence_id: result.evidence_id,
            matched_terms: result.matched_terms,
            scope: result.scope.map(ScopeView::from),
            trust: result.trust.map(|trust| RecallResultTrustView {
                mode: json_string(&trust.mode),
                score: trust.score,
                can_trust: trust.can_trust,
                reasons: trust.reasons,
                signal_kinds: trust.signal_kinds,
            }),
        }
    }
}
