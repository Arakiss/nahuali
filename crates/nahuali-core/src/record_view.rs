//! Exact-id lookup behind `nahuali show`: one record in full, the ledger events
//! that wrote it, and its evidence links in both directions.
//!
//! The lookup is read-only and matches ids exactly. It searches the projection
//! first, then the ledger, then the derived operator review queue, so any id
//! the CLI prints can be pasted back without knowing which kind it is.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    audit::{self, LedgerAuditEventKind},
    error::{NahualiError, Result},
    event::{EventEnvelope, MemoryEvent},
    inspection,
    model::{
        Claim, ClaimLifecycle, Entity, Episode, ForgottenRecord, Intention, Link, MemoryData,
        MemoryRepair, MemoryScope, Procedure, ReviewDecision, SourceDocument,
    },
    operator_review::{self, OperatorReviewItem, OperatorReviewOptions},
};

/// Current record view format version.
pub const RECORD_VIEW_VERSION: u32 = 1;

/// Id prefixes the lookup understands, as printed by the CLI.
pub const RECORD_ID_KINDS: &[&str] = &[
    "source_",
    "episode_",
    "claim_",
    "fact_",
    "link_",
    "relation_",
    "procedure_",
    "preference_",
    "intention_",
    "entity_",
    "review_ (open review item)",
    "review_decision_",
    "repair_",
    "forget_",
    "event_",
];

/// Kind of record a lookup resolved to.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecordViewKind {
    /// Registered source material.
    Source,
    /// Projected entity.
    Entity,
    /// Observed episode.
    Episode,
    /// Evidence-backed claim (or compatibility fact).
    Claim,
    /// Typed link (or compatibility relation).
    Link,
    /// Procedure or preference.
    Procedure,
    /// Task, goal, or reminder.
    Intention,
    /// Open operator review item derived from self-inspection.
    ReviewItem,
    /// Recorded operator review decision.
    ReviewDecision,
    /// Applied self-repair.
    Repair,
    /// Explicit forget request.
    ForgetRequest,
    /// Memory erased by a forget request; only the audit trace remains.
    Forgotten,
    /// Raw ledger event.
    Event,
}

impl RecordViewKind {
    /// Stable token used by transport and operator interfaces.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Entity => "entity",
            Self::Episode => "episode",
            Self::Claim => "claim",
            Self::Link => "link",
            Self::Procedure => "procedure",
            Self::Intention => "intention",
            Self::ReviewItem => "review_item",
            Self::ReviewDecision => "review_decision",
            Self::Repair => "repair",
            Self::ForgetRequest => "forget_request",
            Self::Forgotten => "forgotten",
            Self::Event => "event",
        }
    }
}

/// The record itself, serialized exactly as the projection or ledger holds it.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum RecordBody {
    /// Registered source material.
    Source(SourceDocument),
    /// Projected entity.
    Entity(Entity),
    /// Observed episode.
    Episode(Episode),
    /// Claim.
    Claim(Claim),
    /// Link.
    Link(Link),
    /// Procedure or preference.
    Procedure(Procedure),
    /// Intention.
    Intention(Intention),
    /// Open operator review item.
    ReviewItem(OperatorReviewItem),
    /// Recorded review decision.
    ReviewDecision(ReviewDecision),
    /// Applied self-repair.
    Repair(MemoryRepair),
    /// Forget request (also the audit trace of forgotten memory).
    ForgetRequest(ForgottenRecord),
    /// Raw ledger event.
    Event(EventEnvelope),
}

/// One ledger event that wrote or changed the record.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct RecordHistoryEntry {
    /// Ledger sequence of the event.
    pub sequence: u64,
    /// Ledger event identifier.
    pub event_id: String,
    /// Event timestamp in milliseconds since the Unix epoch.
    pub timestamp_ms: u64,
    /// Event category.
    pub kind: LedgerAuditEventKind,
    /// What the event did to the record, including lifecycle reasons.
    pub summary: String,
}

/// An evidence link between the shown record and another id.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct RecordReference {
    /// Field on the citing record that holds the link, e.g. `source_episode`.
    pub relation: String,
    /// Referenced identifier.
    pub id: String,
    /// Kind of the referenced record; `None` when the id does not resolve.
    pub kind: Option<RecordViewKind>,
    /// One-line description of the referenced record, when it resolves.
    pub summary: Option<String>,
}

/// One record in full, with its lifecycle and evidence links.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RecordView {
    /// Report format version.
    pub version: u32,
    /// The id that was looked up.
    pub id: String,
    /// Kind of record the id resolved to.
    pub kind: RecordViewKind,
    /// Lifecycle status, when the kind has one (intention status, claim
    /// lifecycle, review status, `forgotten`).
    pub status: Option<String>,
    /// One-line description of the record.
    pub summary: String,
    /// Explicit memory context boundary, when present.
    pub scope: Option<MemoryScope>,
    /// The record exactly as projected or recorded.
    pub record: RecordBody,
    /// Supersession and contest state for claims.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim_lifecycle: Option<ClaimLifecycle>,
    /// Ledger events that wrote or changed this record, oldest first.
    pub history: Vec<RecordHistoryEntry>,
    /// Ids this record cites.
    pub evidence: Vec<RecordReference>,
    /// Records that cite this record.
    pub cited_by: Vec<RecordReference>,
    /// Context the operator needs to read the result correctly.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

pub(crate) fn show_record(
    data: &MemoryData,
    events: &[EventEnvelope],
    id: &str,
) -> Result<RecordView> {
    let id = id.trim();
    if id.is_empty() {
        return Err(NahualiError::EmptyQuery);
    }
    let review_items = operator_review::operator_review(
        data,
        OperatorReviewOptions {
            limit: usize::MAX,
            min_priority: None,
            ..OperatorReviewOptions::default()
        },
    )
    .items;
    let (kind, record, notes) =
        locate(data, events, &review_items, id).ok_or_else(|| NahualiError::UnknownRecord {
            id: id.to_string(),
            known_kinds: RECORD_ID_KINDS.join(", "),
        })?;

    let claim_lifecycle = match &record {
        RecordBody::Claim(claim) => Some(
            inspection::claim_lifecycles(data)
                .remove(&claim.id)
                .unwrap_or_else(ClaimLifecycle::current),
        ),
        _ => None,
    };
    let status = match (&record, kind) {
        (_, RecordViewKind::Forgotten) => Some("forgotten".to_string()),
        (RecordBody::Intention(intention), _) => Some(audit::token(&intention.status)),
        (RecordBody::ReviewItem(item), _) => Some(audit::token(&item.status)),
        (RecordBody::ReviewDecision(decision), _) => Some(audit::token(&decision.outcome)),
        _ => claim_lifecycle
            .as_ref()
            .map(|lifecycle| audit::token(&lifecycle.status)),
    };
    let history = history(events, &record);
    let evidence = evidence(data, events, kind, &record);
    let cited_by = cited_by(data, &review_items, kind, &record);

    Ok(RecordView {
        version: RECORD_VIEW_VERSION,
        id: id.to_string(),
        kind,
        status,
        summary: body_summary(&record),
        scope: body_scope(&record),
        record,
        claim_lifecycle,
        history,
        evidence,
        cited_by,
        notes,
    })
}

fn locate(
    data: &MemoryData,
    events: &[EventEnvelope],
    review_items: &[OperatorReviewItem],
    id: &str,
) -> Option<(RecordViewKind, RecordBody, Vec<String>)> {
    use RecordViewKind as Kind;

    let found = |kind: Kind, body: RecordBody| Some((kind, body, Vec::new()));
    if let Some(record) = data.sources.iter().find(|record| record.id == id) {
        return found(Kind::Source, RecordBody::Source(record.clone()));
    }
    if let Some(record) = data.episodes.iter().find(|record| record.id == id) {
        return found(Kind::Episode, RecordBody::Episode(record.clone()));
    }
    if let Some(record) = data.claims.iter().find(|record| record.id == id) {
        return found(Kind::Claim, RecordBody::Claim(record.clone()));
    }
    if let Some(record) = data.links.iter().find(|record| record.id == id) {
        return found(Kind::Link, RecordBody::Link(record.clone()));
    }
    if let Some(record) = data.procedures.iter().find(|record| record.id == id) {
        return found(Kind::Procedure, RecordBody::Procedure(record.clone()));
    }
    if let Some(record) = data.intentions.iter().find(|record| record.id == id) {
        return found(Kind::Intention, RecordBody::Intention(record.clone()));
    }
    if let Some(record) = data.entities.iter().find(|record| record.id == id) {
        return found(Kind::Entity, RecordBody::Entity(record.clone()));
    }
    if let Some(record) = data.review_decisions.iter().find(|record| record.id == id) {
        return found(
            Kind::ReviewDecision,
            RecordBody::ReviewDecision(record.clone()),
        );
    }
    if let Some(record) = data.repairs.iter().find(|record| record.id == id) {
        return found(Kind::Repair, RecordBody::Repair(record.clone()));
    }
    if let Some(record) = data.forgotten.iter().find(|record| record.id == id) {
        return found(
            Kind::ForgetRequest,
            RecordBody::ForgetRequest(record.clone()),
        );
    }
    if let Some(event) = events.iter().find(|event| event.id == id) {
        return found(Kind::Event, RecordBody::Event(event.clone()));
    }
    if let Some(item) = review_items.iter().find(|item| item.id == id) {
        return found(Kind::ReviewItem, RecordBody::ReviewItem(item.clone()));
    }
    // A resolved review item leaves the queue; its decision is the durable record.
    if let Some(decision) = data
        .review_decisions
        .iter()
        .rev()
        .find(|decision| decision.review_id == id)
    {
        return Some((
            Kind::ReviewDecision,
            RecordBody::ReviewDecision(decision.clone()),
            vec![format!(
                "review item {id} is no longer open: it was resolved by {}",
                decision.id
            )],
        ));
    }
    if let Some(request) = data.forgotten.iter().find(|request| {
        request
            .forgotten_ids
            .iter()
            .any(|forgotten| forgotten == id)
    }) {
        return Some((
            Kind::Forgotten,
            RecordBody::ForgetRequest(request.clone()),
            vec![format!(
                "{id} was erased by forget request {}; only this audit trace remains",
                request.id
            )],
        ));
    }
    None
}

fn record_id(record: &RecordBody) -> &str {
    match record {
        RecordBody::Source(record) => &record.id,
        RecordBody::Entity(record) => &record.id,
        RecordBody::Episode(record) => &record.id,
        RecordBody::Claim(record) => &record.id,
        RecordBody::Link(record) => &record.id,
        RecordBody::Procedure(record) => &record.id,
        RecordBody::Intention(record) => &record.id,
        RecordBody::ReviewItem(record) => &record.id,
        RecordBody::ReviewDecision(record) => &record.id,
        RecordBody::Repair(record) => &record.id,
        RecordBody::ForgetRequest(record) => &record.id,
        RecordBody::Event(record) => &record.id,
    }
}

fn record_event_id(record: &RecordBody) -> Option<&str> {
    match record {
        RecordBody::Source(record) => Some(&record.event_id),
        RecordBody::Episode(record) => Some(&record.event_id),
        RecordBody::Claim(record) => Some(&record.event_id),
        RecordBody::Link(record) => Some(&record.event_id),
        RecordBody::Procedure(record) => Some(&record.event_id),
        RecordBody::Intention(record) => Some(&record.event_id),
        RecordBody::ReviewDecision(record) => Some(&record.event_id),
        RecordBody::Repair(record) => Some(&record.event_id),
        RecordBody::ForgetRequest(record) => Some(&record.event_id),
        RecordBody::Entity(_) | RecordBody::ReviewItem(_) | RecordBody::Event(_) => None,
    }
}

fn body_summary(record: &RecordBody) -> String {
    match record {
        RecordBody::Source(record) => source_summary(record),
        RecordBody::Entity(record) => record.name.clone(),
        RecordBody::Episode(record) => record.content.clone(),
        RecordBody::Claim(record) => claim_summary(record),
        RecordBody::Link(record) => link_summary(record),
        RecordBody::Procedure(record) => procedure_summary(record),
        RecordBody::Intention(record) => record.description.clone(),
        RecordBody::ReviewItem(record) => record.title.clone(),
        RecordBody::ReviewDecision(record) => decision_summary(record),
        RecordBody::Repair(record) => repair_summary(record),
        RecordBody::ForgetRequest(record) => format!(
            "forgot {} memory id(s): {}",
            record.forgotten_ids.len(),
            record.reason
        ),
        RecordBody::Event(record) => audit::summarize(&record.payload),
    }
}

fn source_summary(source: &SourceDocument) -> String {
    source
        .title
        .clone()
        .or_else(|| source.uri.clone())
        .unwrap_or_else(|| source.id.clone())
}

fn claim_summary(claim: &Claim) -> String {
    format!("{} {} {}", claim.subject, claim.predicate, claim.object)
}

fn link_summary(link: &Link) -> String {
    format!("{} {} {}", link.from, link.relation, link.to)
}

fn procedure_summary(procedure: &Procedure) -> String {
    format!("{}: {}", procedure.name, procedure.body)
}

fn decision_summary(decision: &ReviewDecision) -> String {
    format!("{} resolved: {}", decision.review_id, decision.note)
}

fn repair_summary(repair: &MemoryRepair) -> String {
    format!(
        "{} repair materialized {}",
        audit::token(&repair.kind),
        repair.materialized_id
    )
}

fn body_scope(record: &RecordBody) -> Option<MemoryScope> {
    match record {
        RecordBody::Source(record) => record.scope.clone(),
        RecordBody::Entity(record) => record.scope.clone(),
        RecordBody::Episode(record) => record.scope.clone(),
        RecordBody::Claim(record) => record.scope.clone(),
        RecordBody::Link(record) => record.scope.clone(),
        RecordBody::Procedure(record) => record.scope.clone(),
        RecordBody::Intention(record) => record.scope.clone(),
        RecordBody::ReviewDecision(record) => record.scope.clone(),
        RecordBody::Repair(record) => record.scope.clone(),
        RecordBody::ReviewItem(_) | RecordBody::ForgetRequest(_) | RecordBody::Event(_) => None,
    }
}

fn history(events: &[EventEnvelope], record: &RecordBody) -> Vec<RecordHistoryEntry> {
    match record {
        RecordBody::Event(event) => vec![history_entry(event)],
        RecordBody::Entity(entity) => {
            let mentions = entity
                .source_event_ids
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>();
            events
                .iter()
                .filter(|event| mentions.contains(event.id.as_str()))
                .map(history_entry)
                .collect()
        }
        // A forgotten id resolves to its forget request, whose event is the
        // only history left: the original records are redacted.
        _ => events
            .iter()
            .filter(|event| event_touches(&event.payload, record_id(record)))
            .map(history_entry)
            .collect(),
    }
}

/// Whether a ledger event wrote or changed the memory with this id.
fn event_touches(payload: &MemoryEvent, id: &str) -> bool {
    match payload {
        MemoryEvent::SourceRecorded(event) => event.id == id,
        MemoryEvent::EpisodeRecorded(event) => event.id == id,
        MemoryEvent::FactAsserted(event) => event.id == id,
        MemoryEvent::RelationRecorded(event) => event.id == id,
        MemoryEvent::ProcedureRecorded(event) => event.id == id,
        MemoryEvent::IntentionRecorded(event) => event.id == id,
        MemoryEvent::IntentionUpdated(event) => event.id == id,
        MemoryEvent::IntentionStatusChanged(event) => event.id == id,
        MemoryEvent::ReviewRecorded(event) => event.id == id || event.review_id == id,
        MemoryEvent::RepairApplied(event) => event.id == id || event.materialized_id() == id,
        MemoryEvent::MemoryForgotten(event) => {
            event.id == id || event.forgotten_ids.iter().any(|forgotten| forgotten == id)
        }
        MemoryEvent::Redacted(_) => false,
    }
}

fn history_entry(event: &EventEnvelope) -> RecordHistoryEntry {
    RecordHistoryEntry {
        sequence: event.sequence,
        event_id: event.id.clone(),
        timestamp_ms: event.timestamp_ms,
        kind: audit::event_kind(&event.payload),
        summary: history_summary(&event.payload),
    }
}

/// Audit summary, with the lifecycle detail the audit line leaves out: the
/// reason behind a status change and which fields an update touched.
fn history_summary(payload: &MemoryEvent) -> String {
    match payload {
        MemoryEvent::IntentionStatusChanged(event) => {
            let status = audit::token(&event.status);
            match event.reason.as_deref().map(str::trim) {
                Some(reason) if !reason.is_empty() => format!("status -> {status}: {reason}"),
                _ => format!("status -> {status}"),
            }
        }
        MemoryEvent::IntentionUpdated(event) => {
            let mut changed = Vec::new();
            if let Some(description) = &event.description {
                changed.push(format!("description: {description}"));
            }
            if let Some(priority) = &event.priority {
                changed.push(format!("priority: {}", audit::token(priority)));
            }
            match event.deadline_at_ms {
                Some(Some(deadline)) => changed.push(format!("deadline_at_ms: {deadline}")),
                Some(None) => changed.push("deadline cleared".to_string()),
                None => {}
            }
            match &event.depends_on {
                Some(dependencies) if dependencies.is_empty() => {
                    changed.push("dependencies cleared".to_string());
                }
                Some(dependencies) => {
                    changed.push(format!("depends_on: {}", dependencies.join(", ")));
                }
                None => {}
            }
            match &event.goal_id {
                Some(Some(goal)) => changed.push(format!("goal: {goal}")),
                Some(None) => changed.push("goal cleared".to_string()),
                None => {}
            }
            match event.progress_percent {
                Some(Some(progress)) => changed.push(format!("progress: {progress}%")),
                Some(None) => changed.push("progress cleared".to_string()),
                None => {}
            }
            if changed.is_empty() {
                "metadata updated".to_string()
            } else {
                format!("updated {}", changed.join("; "))
            }
        }
        other => audit::summarize(other),
    }
}

fn evidence(
    data: &MemoryData,
    events: &[EventEnvelope],
    kind: RecordViewKind,
    record: &RecordBody,
) -> Vec<RecordReference> {
    if kind == RecordViewKind::Forgotten {
        return Vec::new();
    }
    let mut links: Vec<(&str, &str)> = Vec::new();
    match record {
        RecordBody::Episode(episode) => {
            links.extend(episode.source_id.as_deref().map(|id| ("source", id)));
        }
        RecordBody::Claim(claim) => {
            links.extend(
                claim
                    .source_episode_id
                    .as_deref()
                    .map(|id| ("source_episode", id)),
            );
        }
        RecordBody::Link(link) => {
            links.extend(
                link.source_episode_id
                    .as_deref()
                    .map(|id| ("source_episode", id)),
            );
        }
        RecordBody::Procedure(procedure) => {
            links.extend(
                procedure
                    .source_episode_id
                    .as_deref()
                    .map(|id| ("source_episode", id)),
            );
        }
        RecordBody::Intention(intention) => {
            links.extend(
                intention
                    .source_episode_id
                    .as_deref()
                    .map(|id| ("source_episode", id)),
            );
            links.extend(
                intention
                    .depends_on
                    .iter()
                    .map(|id| ("depends_on", id.as_str())),
            );
            links.extend(intention.goal_id.as_deref().map(|id| ("goal", id)));
        }
        RecordBody::ReviewItem(item) => {
            links.extend(item.evidence_ids.iter().map(|id| ("evidence", id.as_str())));
        }
        RecordBody::ReviewDecision(decision) => {
            links.extend(
                decision
                    .evidence_ids
                    .iter()
                    .map(|id| ("evidence", id.as_str())),
            );
        }
        RecordBody::Repair(repair) => {
            links.extend(
                repair
                    .evidence_ids
                    .iter()
                    .map(|id| ("evidence", id.as_str())),
            );
            links.push(("materialized", repair.materialized_id.as_str()));
        }
        RecordBody::Event(event) => {
            links.extend(event_subject(&event.payload).map(|id| ("records", id)));
        }
        RecordBody::Source(_) | RecordBody::Entity(_) | RecordBody::ForgetRequest(_) => {}
    }
    links
        .into_iter()
        .map(|(relation, id)| {
            let resolved = describe(data, events, id);
            RecordReference {
                relation: relation.to_string(),
                id: id.to_string(),
                kind: resolved.as_ref().map(|(kind, _)| *kind),
                summary: resolved.map(|(_, summary)| summary),
            }
        })
        .collect()
}

/// The memory id an event wrote, when it wrote one.
fn event_subject(payload: &MemoryEvent) -> Option<&str> {
    match payload {
        MemoryEvent::SourceRecorded(event) => Some(&event.id),
        MemoryEvent::EpisodeRecorded(event) => Some(&event.id),
        MemoryEvent::FactAsserted(event) => Some(&event.id),
        MemoryEvent::RelationRecorded(event) => Some(&event.id),
        MemoryEvent::ProcedureRecorded(event) => Some(&event.id),
        MemoryEvent::IntentionRecorded(event) => Some(&event.id),
        MemoryEvent::IntentionUpdated(event) => Some(&event.id),
        MemoryEvent::IntentionStatusChanged(event) => Some(&event.id),
        MemoryEvent::ReviewRecorded(event) => Some(&event.id),
        MemoryEvent::RepairApplied(event) => Some(&event.id),
        MemoryEvent::MemoryForgotten(event) => Some(&event.id),
        MemoryEvent::Redacted(_) => None,
    }
}

/// Kind and one-line summary of a projected record or ledger event, without
/// computing the review queue.
fn describe(
    data: &MemoryData,
    events: &[EventEnvelope],
    id: &str,
) -> Option<(RecordViewKind, String)> {
    use RecordViewKind as Kind;

    data.sources
        .iter()
        .find(|record| record.id == id)
        .map(|record| (Kind::Source, source_summary(record)))
        .or_else(|| {
            data.episodes
                .iter()
                .find(|record| record.id == id)
                .map(|record| (Kind::Episode, record.content.clone()))
        })
        .or_else(|| {
            data.claims
                .iter()
                .find(|record| record.id == id)
                .map(|record| (Kind::Claim, claim_summary(record)))
        })
        .or_else(|| {
            data.links
                .iter()
                .find(|record| record.id == id)
                .map(|record| (Kind::Link, link_summary(record)))
        })
        .or_else(|| {
            data.procedures
                .iter()
                .find(|record| record.id == id)
                .map(|record| (Kind::Procedure, procedure_summary(record)))
        })
        .or_else(|| {
            data.intentions
                .iter()
                .find(|record| record.id == id)
                .map(|record| (Kind::Intention, record.description.clone()))
        })
        .or_else(|| {
            data.entities
                .iter()
                .find(|record| record.id == id)
                .map(|record| (Kind::Entity, record.name.clone()))
        })
        .or_else(|| {
            data.review_decisions
                .iter()
                .find(|record| record.id == id)
                .map(|record| (Kind::ReviewDecision, decision_summary(record)))
        })
        .or_else(|| {
            data.repairs
                .iter()
                .find(|record| record.id == id)
                .map(|record| (Kind::Repair, repair_summary(record)))
        })
        .or_else(|| {
            events
                .iter()
                .find(|event| event.id == id)
                .map(|event| (Kind::Event, audit::summarize(&event.payload)))
        })
}

fn cited_by(
    data: &MemoryData,
    review_items: &[OperatorReviewItem],
    kind: RecordViewKind,
    record: &RecordBody,
) -> Vec<RecordReference> {
    use RecordViewKind as Kind;

    let reference = |relation: &str, kind: Kind, id: &str, summary: String| RecordReference {
        relation: relation.to_string(),
        id: id.to_string(),
        kind: Some(kind),
        summary: Some(summary),
    };
    let mut references = Vec::new();

    if let RecordBody::Entity(entity) = record {
        let mentions = entity
            .source_event_ids
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let mentioned = |event_id: &str| mentions.contains(event_id);
        for episode in data.episodes.iter().filter(|r| mentioned(&r.event_id)) {
            references.push(reference(
                "mentions",
                Kind::Episode,
                &episode.id,
                episode.content.clone(),
            ));
        }
        for claim in data.claims.iter().filter(|r| mentioned(&r.event_id)) {
            references.push(reference(
                "mentions",
                Kind::Claim,
                &claim.id,
                claim_summary(claim),
            ));
        }
        for link in data.links.iter().filter(|r| mentioned(&r.event_id)) {
            references.push(reference(
                "mentions",
                Kind::Link,
                &link.id,
                link_summary(link),
            ));
        }
        return references;
    }
    if matches!(kind, Kind::Forgotten | Kind::ForgetRequest | Kind::Event) {
        return references;
    }

    let id = record_id(record);
    // Review evidence cites either the memory id or the event that wrote it.
    let targets = std::iter::once(id)
        .chain(record_event_id(record))
        .collect::<BTreeSet<_>>();
    let cites = |evidence_ids: &[String]| {
        evidence_ids
            .iter()
            .any(|evidence| targets.contains(evidence.as_str()))
    };

    for episode in data
        .episodes
        .iter()
        .filter(|r| r.source_id.as_deref() == Some(id))
    {
        references.push(reference(
            "source",
            Kind::Episode,
            &episode.id,
            episode.content.clone(),
        ));
    }
    let cites_episode = |source: &Option<String>| source.as_deref() == Some(id);
    for claim in data
        .claims
        .iter()
        .filter(|r| cites_episode(&r.source_episode_id))
    {
        references.push(reference(
            "source_episode",
            Kind::Claim,
            &claim.id,
            claim_summary(claim),
        ));
    }
    for link in data
        .links
        .iter()
        .filter(|r| cites_episode(&r.source_episode_id))
    {
        references.push(reference(
            "source_episode",
            Kind::Link,
            &link.id,
            link_summary(link),
        ));
    }
    for procedure in data
        .procedures
        .iter()
        .filter(|r| cites_episode(&r.source_episode_id))
    {
        references.push(reference(
            "source_episode",
            Kind::Procedure,
            &procedure.id,
            procedure_summary(procedure),
        ));
    }
    for intention in &data.intentions {
        if intention.id == id {
            continue;
        }
        let summary = || intention.description.clone();
        if cites_episode(&intention.source_episode_id) {
            references.push(reference(
                "source_episode",
                Kind::Intention,
                &intention.id,
                summary(),
            ));
        }
        if intention
            .depends_on
            .iter()
            .any(|dependency| dependency == id)
        {
            references.push(reference(
                "depends_on",
                Kind::Intention,
                &intention.id,
                summary(),
            ));
        }
        if intention.goal_id.as_deref() == Some(id) {
            references.push(reference("goal", Kind::Intention, &intention.id, summary()));
        }
    }
    for decision in data
        .review_decisions
        .iter()
        .filter(|r| r.id != id && cites(&r.evidence_ids))
    {
        references.push(reference(
            "evidence",
            Kind::ReviewDecision,
            &decision.id,
            decision_summary(decision),
        ));
    }
    for repair in data.repairs.iter().filter(|r| r.id != id) {
        if cites(&repair.evidence_ids) {
            references.push(reference(
                "evidence",
                Kind::Repair,
                &repair.id,
                repair_summary(repair),
            ));
        }
        if repair.materialized_id == id {
            references.push(reference(
                "materialized",
                Kind::Repair,
                &repair.id,
                repair_summary(repair),
            ));
        }
    }
    for item in review_items
        .iter()
        .filter(|r| r.id != id && cites(&r.evidence_ids))
    {
        references.push(reference(
            "evidence",
            Kind::ReviewItem,
            &item.id,
            item.title.clone(),
        ));
    }
    references
}

#[cfg(test)]
mod tests {
    use super::{RecordBody, RecordViewKind, show_record};
    use crate::{
        EpisodeRecorded, EventEnvelope, FactAsserted, IntentionRecorded, IntentionRecordedKind,
        IntentionRecordedPriority, IntentionRecordedStatus, IntentionStatusChanged,
        IntentionUpdated, LedgerAuditEventKind, MemoryEvent, NahualiError,
        project_validated_events,
    };

    fn ledger(payloads: Vec<MemoryEvent>) -> Vec<EventEnvelope> {
        payloads
            .into_iter()
            .enumerate()
            .map(|(index, payload)| {
                let sequence = index as u64 + 1;
                EventEnvelope::new(sequence, 1_000 * sequence, payload)
            })
            .collect()
    }

    fn episode(id: &str, content: &str, mentions: &[&str]) -> MemoryEvent {
        MemoryEvent::EpisodeRecorded(EpisodeRecorded {
            id: id.to_string(),
            content: content.to_string(),
            tags: Vec::new(),
            mentions: mentions.iter().map(|name| name.to_string()).collect(),
            source_id: None,
            source_position: None,
            source_role: None,
            scope: None,
        })
    }

    fn fact(id: &str, object: &str, source: &str) -> MemoryEvent {
        MemoryEvent::FactAsserted(FactAsserted {
            id: id.to_string(),
            subject: "Arnold".to_string(),
            predicate: "editor".to_string(),
            object: object.to_string(),
            source_episode_id: Some(source.to_string()),
            confidence: 0.9,
            scope: None,
        })
    }

    fn intention(id: &str, description: &str, source: Option<&str>) -> MemoryEvent {
        MemoryEvent::IntentionRecorded(IntentionRecorded {
            id: id.to_string(),
            kind: IntentionRecordedKind::Task,
            priority: IntentionRecordedPriority::High,
            description: description.to_string(),
            source_episode_id: source.map(str::to_string),
            deadline_at_ms: None,
            depends_on: Vec::new(),
            goal_id: None,
            progress_percent: None,
            scope: None,
        })
    }

    fn fixture() -> Vec<EventEnvelope> {
        ledger(vec![
            episode("episode_1", "Arnold uses Vim", &["Arnold"]),
            fact("claim_1", "Vim", "episode_1"),
            episode("episode_2", "Arnold switched to Helix", &["Arnold"]),
            fact("claim_2", "Helix", "episode_2"),
            intention("intention_goal", "Ship the editor guide", None),
            intention("intention_1", "Write the Helix section", Some("episode_2")),
            MemoryEvent::IntentionUpdated(IntentionUpdated {
                id: "intention_1".to_string(),
                description: None,
                priority: None,
                deadline_at_ms: None,
                depends_on: None,
                goal_id: Some(Some("intention_goal".to_string())),
                progress_percent: Some(Some(40)),
            }),
            MemoryEvent::IntentionStatusChanged(IntentionStatusChanged {
                id: "intention_1".to_string(),
                status: IntentionRecordedStatus::Completed,
                reason: Some("merged in the guide".to_string()),
            }),
        ])
    }

    #[test]
    fn shows_an_intention_with_its_lifecycle_and_evidence() {
        let events = fixture();
        let data = project_validated_events(&events);

        let view = show_record(&data, &events, " intention_1 ").expect("intention resolves");

        assert_eq!(view.kind, RecordViewKind::Intention);
        assert_eq!(view.id, "intention_1");
        assert_eq!(view.status.as_deref(), Some("completed"));
        assert_eq!(view.summary, "Write the Helix section");
        assert!(matches!(view.record, RecordBody::Intention(_)));
        let kinds = view
            .history
            .iter()
            .map(|entry| entry.kind)
            .collect::<Vec<_>>();
        assert_eq!(
            kinds,
            vec![
                LedgerAuditEventKind::IntentionRecorded,
                LedgerAuditEventKind::IntentionUpdated,
                LedgerAuditEventKind::IntentionStatusChanged,
            ]
        );
        assert_eq!(
            view.history[1].summary,
            "updated goal: intention_goal; progress: 40%"
        );
        assert_eq!(
            view.history[2].summary,
            "status -> completed: merged in the guide"
        );
        let evidence = view
            .evidence
            .iter()
            .map(|reference| (reference.relation.as_str(), reference.id.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            evidence,
            vec![("source_episode", "episode_2"), ("goal", "intention_goal")]
        );
        assert_eq!(view.evidence[0].kind, Some(RecordViewKind::Episode));
        assert_eq!(
            view.evidence[0].summary.as_deref(),
            Some("Arnold switched to Helix")
        );

        let goal = show_record(&data, &events, "intention_goal").expect("goal resolves");
        let children = goal
            .cited_by
            .iter()
            .filter(|reference| reference.relation == "goal")
            .map(|reference| reference.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(children, vec!["intention_1"]);
    }

    #[test]
    fn shows_claim_lifecycle_and_what_an_episode_supports() {
        let events = fixture();
        let data = project_validated_events(&events);

        let vim = show_record(&data, &events, "claim_1").expect("claim resolves");
        assert_eq!(vim.kind, RecordViewKind::Claim);
        assert_eq!(vim.status.as_deref(), Some("superseded"));
        assert_eq!(
            vim.claim_lifecycle
                .as_ref()
                .and_then(|lifecycle| lifecycle.superseded_by.as_deref()),
            Some("claim_2")
        );

        let episode = show_record(&data, &events, "episode_2").expect("episode resolves");
        let supported = episode
            .cited_by
            .iter()
            .filter(|reference| reference.relation == "source_episode")
            .map(|reference| reference.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(supported, vec!["claim_2", "intention_1"]);
        // Open review items that cite the episode are listed too.
        assert!(
            episode
                .cited_by
                .iter()
                .filter(|reference| reference.kind == Some(RecordViewKind::ReviewItem))
                .all(|reference| reference.relation == "evidence")
        );
        assert_eq!(episode.history.len(), 1);
    }

    #[test]
    fn shows_an_entity_with_the_records_that_mention_it() {
        let events = fixture();
        let data = project_validated_events(&events);
        let entity_id = data
            .entities
            .iter()
            .find(|entity| entity.name == "Arnold")
            .map(|entity| entity.id.clone())
            .expect("Arnold is projected");

        let view = show_record(&data, &events, &entity_id).expect("entity resolves");

        assert_eq!(view.kind, RecordViewKind::Entity);
        assert!(
            view.cited_by
                .iter()
                .any(|reference| reference.id == "episode_1" && reference.relation == "mentions"),
            "{:?}",
            view.cited_by
        );
        assert!(!view.history.is_empty());
    }

    #[test]
    fn shows_a_raw_ledger_event_and_the_memory_it_wrote() {
        let events = fixture();
        let data = project_validated_events(&events);
        let event_id = events[1].id.clone();

        let view = show_record(&data, &events, &event_id).expect("event resolves");

        assert_eq!(view.kind, RecordViewKind::Event);
        assert_eq!(view.evidence.len(), 1);
        assert_eq!(view.evidence[0].relation, "records");
        assert_eq!(view.evidence[0].id, "claim_1");
    }

    #[test]
    fn unknown_ids_fail_and_name_the_id_kinds_that_exist() {
        let events = fixture();
        let data = project_validated_events(&events);

        let error = show_record(&data, &events, "intention_missing").expect_err("unknown id");

        assert!(matches!(error, NahualiError::UnknownRecord { .. }));
        let message = error.to_string();
        assert!(message.contains("intention_missing"), "{message}");
        assert!(message.contains("episode_"), "{message}");
        assert!(message.contains("review_decision_"), "{message}");
        assert!(matches!(
            show_record(&data, &events, "  "),
            Err(NahualiError::EmptyQuery)
        ));
    }
}
