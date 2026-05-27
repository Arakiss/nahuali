use std::collections::{BTreeMap, BTreeSet};

use crate::{
    event::{
        EventEnvelope, IntentionRecordedKind, IntentionRecordedPriority, IntentionRecordedStatus,
        MemoryEvent, ProcedureRecordedKind, ReviewRecordedAction, ReviewRecordedOutcome,
        SourceRecordedKind,
    },
    model::{
        Claim, Entity, Episode, Intention, IntentionKind, IntentionPriority, IntentionStatus, Link,
        MemoryData, MemoryScope, Procedure, ProcedureKind, ReviewDecision, ReviewDecisionAction,
        ReviewDecisionOutcome, SourceDocument, SourceKind,
    },
};

pub(crate) fn project(events: &[EventEnvelope]) -> MemoryData {
    let mut data = MemoryData::default();
    let mut entities = BTreeMap::new();

    for event in events {
        data.event_count += 1;
        data.last_event_id = Some(event.id.clone());

        match &event.payload {
            MemoryEvent::SourceRecorded(payload) => {
                data.sources.push(SourceDocument {
                    id: payload.id.clone(),
                    event_id: event.id.clone(),
                    kind: source_kind(&payload.kind),
                    title: payload.title.clone(),
                    uri: payload.uri.clone(),
                    content_checksum: payload.content_checksum.clone(),
                    byte_len: payload.byte_len,
                    metadata: payload.metadata.clone(),
                    scope: payload.scope.clone(),
                    created_at_ms: event.timestamp_ms,
                });
            }
            MemoryEvent::EpisodeRecorded(payload) => {
                data.episodes.push(Episode {
                    id: payload.id.clone(),
                    event_id: event.id.clone(),
                    content: payload.content.clone(),
                    tags: payload.tags.clone(),
                    mentions: cleaned_names(&payload.mentions),
                    source_id: payload.source_id.clone(),
                    source_position: payload.source_position,
                    source_role: payload.source_role.clone(),
                    scope: payload.scope.clone(),
                    created_at_ms: event.timestamp_ms,
                });
                for mention in &payload.mentions {
                    record_entity(
                        &mut entities,
                        mention,
                        event.timestamp_ms,
                        &event.id,
                        payload.scope.as_ref(),
                    );
                }
            }
            MemoryEvent::FactAsserted(payload) => {
                let claim = Claim {
                    id: payload.id.clone(),
                    event_id: event.id.clone(),
                    subject: payload.subject.clone(),
                    predicate: payload.predicate.clone(),
                    object: payload.object.clone(),
                    source_episode_id: payload.source_episode_id.clone(),
                    confidence: payload.confidence,
                    scope: payload.scope.clone(),
                    created_at_ms: event.timestamp_ms,
                };
                record_entity(
                    &mut entities,
                    &claim.subject,
                    event.timestamp_ms,
                    &event.id,
                    claim.scope.as_ref(),
                );
                record_entity(
                    &mut entities,
                    &claim.object,
                    event.timestamp_ms,
                    &event.id,
                    claim.scope.as_ref(),
                );
                data.claims.push(claim.clone());
                data.facts.push(claim);
            }
            MemoryEvent::RelationRecorded(payload) => {
                let link = Link {
                    id: payload.id.clone(),
                    event_id: event.id.clone(),
                    from: payload.from.clone(),
                    relation: payload.relation.clone(),
                    to: payload.to.clone(),
                    source_episode_id: payload.source_episode_id.clone(),
                    confidence: payload.confidence,
                    scope: payload.scope.clone(),
                    created_at_ms: event.timestamp_ms,
                };
                record_entity(
                    &mut entities,
                    &link.from,
                    event.timestamp_ms,
                    &event.id,
                    link.scope.as_ref(),
                );
                record_entity(
                    &mut entities,
                    &link.to,
                    event.timestamp_ms,
                    &event.id,
                    link.scope.as_ref(),
                );
                data.links.push(link.clone());
                data.relations.push(link);
            }
            MemoryEvent::ProcedureRecorded(payload) => {
                data.procedures.push(Procedure {
                    id: payload.id.clone(),
                    event_id: event.id.clone(),
                    kind: procedure_kind(&payload.kind),
                    name: payload.name.clone(),
                    body: payload.body.clone(),
                    source_episode_id: payload.source_episode_id.clone(),
                    confidence: payload.confidence,
                    scope: payload.scope.clone(),
                    created_at_ms: event.timestamp_ms,
                });
            }
            MemoryEvent::IntentionRecorded(payload) => {
                data.intentions.push(Intention {
                    id: payload.id.clone(),
                    event_id: event.id.clone(),
                    updated_event_id: event.id.clone(),
                    kind: intention_kind(&payload.kind),
                    status: IntentionStatus::Active,
                    priority: intention_priority(&payload.priority),
                    description: payload.description.clone(),
                    source_episode_id: payload.source_episode_id.clone(),
                    status_reason: None,
                    deadline_at_ms: payload.deadline_at_ms,
                    depends_on: payload.depends_on.clone(),
                    goal_id: payload.goal_id.clone(),
                    progress_percent: payload.progress_percent,
                    scope: payload.scope.clone(),
                    created_at_ms: event.timestamp_ms,
                    updated_at_ms: event.timestamp_ms,
                });
            }
            MemoryEvent::IntentionUpdated(payload) => {
                if let Some(intention) = data
                    .intentions
                    .iter_mut()
                    .find(|intention| intention.id == payload.id)
                {
                    if let Some(description) = &payload.description {
                        intention.description = description.clone();
                    }
                    if let Some(priority) = &payload.priority {
                        intention.priority = intention_priority(priority);
                    }
                    if let Some(deadline_at_ms) = payload.deadline_at_ms {
                        intention.deadline_at_ms = deadline_at_ms;
                    }
                    if let Some(depends_on) = &payload.depends_on {
                        intention.depends_on = depends_on.clone();
                    }
                    if let Some(goal_id) = &payload.goal_id {
                        intention.goal_id = goal_id.clone();
                    }
                    if let Some(progress_percent) = payload.progress_percent {
                        intention.progress_percent = progress_percent;
                    }
                    intention.updated_event_id = event.id.clone();
                    intention.updated_at_ms = event.timestamp_ms;
                }
            }
            MemoryEvent::IntentionStatusChanged(payload) => {
                if let Some(intention) = data
                    .intentions
                    .iter_mut()
                    .find(|intention| intention.id == payload.id)
                {
                    intention.status = intention_status(&payload.status);
                    intention.status_reason = payload.reason.clone();
                    intention.updated_event_id = event.id.clone();
                    intention.updated_at_ms = event.timestamp_ms;
                }
            }
            MemoryEvent::ReviewRecorded(payload) => {
                data.review_decisions.push(ReviewDecision {
                    id: payload.id.clone(),
                    event_id: event.id.clone(),
                    review_id: payload.review_id.clone(),
                    finding_id: payload.finding_id.clone(),
                    action: review_action(&payload.action),
                    outcome: review_outcome(&payload.outcome),
                    note: payload.note.clone(),
                    evidence_ids: payload.evidence_ids.clone(),
                    scope: payload.scope.clone(),
                    created_at_ms: event.timestamp_ms,
                });
            }
        }
    }

    data.entities = entities.into_values().map(Entity::from).collect();
    data
}

#[derive(Debug)]
struct EntityAccumulator {
    id: String,
    name: String,
    mention_count: usize,
    first_seen_at_ms: u64,
    last_seen_at_ms: u64,
    source_event_ids: Vec<String>,
    scope: Option<MemoryScope>,
}

impl EntityAccumulator {
    fn new(name: String, timestamp_ms: u64, event_id: &str, scope: Option<MemoryScope>) -> Self {
        Self {
            id: entity_id(&name, scope.as_ref()),
            name,
            mention_count: 0,
            first_seen_at_ms: timestamp_ms,
            last_seen_at_ms: timestamp_ms,
            source_event_ids: vec![event_id.to_string()],
            scope,
        }
    }

    fn record(&mut self, timestamp_ms: u64, event_id: &str) {
        self.mention_count += 1;
        self.last_seen_at_ms = timestamp_ms;
        if !self.source_event_ids.iter().any(|id| id == event_id) {
            self.source_event_ids.push(event_id.to_string());
        }
    }
}

impl From<EntityAccumulator> for Entity {
    fn from(value: EntityAccumulator) -> Self {
        Self {
            id: value.id,
            name: value.name,
            mention_count: value.mention_count,
            first_seen_at_ms: value.first_seen_at_ms,
            last_seen_at_ms: value.last_seen_at_ms,
            source_event_ids: value.source_event_ids,
            scope: value.scope,
        }
    }
}

fn record_entity(
    entities: &mut BTreeMap<String, EntityAccumulator>,
    name: &str,
    timestamp_ms: u64,
    event_id: &str,
    scope: Option<&MemoryScope>,
) {
    let Some(name) = clean_name(name) else {
        return;
    };
    let key = entity_key(&name, scope);
    entities
        .entry(key)
        .or_insert_with(|| EntityAccumulator::new(name, timestamp_ms, event_id, scope.cloned()))
        .record(timestamp_ms, event_id);
}

fn cleaned_names(names: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut cleaned = Vec::new();
    for name in names.iter().filter_map(|name| clean_name(name)) {
        if seen.insert(unscoped_entity_key(&name)) {
            cleaned.push(name);
        }
    }
    cleaned
}

fn clean_name(name: &str) -> Option<String> {
    let cleaned = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

fn unscoped_entity_key(name: &str) -> String {
    name.to_ascii_lowercase()
}

fn entity_key(name: &str, scope: Option<&MemoryScope>) -> String {
    let key = unscoped_entity_key(name);
    if let Some(scope) = scope {
        format!("{}::{key}", scope.key)
    } else {
        key
    }
}

fn entity_id(name: &str, scope: Option<&MemoryScope>) -> String {
    let key = unscoped_entity_key(name);
    let slug = key
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    let readable = if slug.is_empty() { "entity" } else { &slug };
    if let Some(scope) = scope {
        let scoped_key = format!("{}::{key}", scope.key);
        format!("entity_{readable}_{:08x}", fnv1a32(scoped_key.as_bytes()))
    } else {
        format!("entity_{readable}_{:08x}", fnv1a32(key.as_bytes()))
    }
}

fn fnv1a32(bytes: &[u8]) -> u32 {
    const FNV_OFFSET: u32 = 0x811c9dc5;
    const FNV_PRIME: u32 = 0x01000193;

    bytes.iter().fold(FNV_OFFSET, |hash, byte| {
        (hash ^ u32::from(*byte)).wrapping_mul(FNV_PRIME)
    })
}

fn source_kind(kind: &SourceRecordedKind) -> SourceKind {
    match kind {
        SourceRecordedKind::Document => SourceKind::Document,
        SourceRecordedKind::Conversation => SourceKind::Conversation,
        SourceRecordedKind::Transcript => SourceKind::Transcript,
        SourceRecordedKind::WebPage => SourceKind::WebPage,
        SourceRecordedKind::Note => SourceKind::Note,
        SourceRecordedKind::Other => SourceKind::Other,
    }
}

fn procedure_kind(kind: &ProcedureRecordedKind) -> ProcedureKind {
    match kind {
        ProcedureRecordedKind::Procedure => ProcedureKind::Procedure,
        ProcedureRecordedKind::Preference => ProcedureKind::Preference,
    }
}

fn intention_kind(kind: &IntentionRecordedKind) -> IntentionKind {
    match kind {
        IntentionRecordedKind::Task => IntentionKind::Task,
        IntentionRecordedKind::Goal => IntentionKind::Goal,
        IntentionRecordedKind::Reminder => IntentionKind::Reminder,
    }
}

fn intention_priority(priority: &IntentionRecordedPriority) -> IntentionPriority {
    match priority {
        IntentionRecordedPriority::Low => IntentionPriority::Low,
        IntentionRecordedPriority::Medium => IntentionPriority::Medium,
        IntentionRecordedPriority::High => IntentionPriority::High,
        IntentionRecordedPriority::Critical => IntentionPriority::Critical,
    }
}

fn intention_status(status: &IntentionRecordedStatus) -> IntentionStatus {
    match status {
        IntentionRecordedStatus::Active => IntentionStatus::Active,
        IntentionRecordedStatus::Completed => IntentionStatus::Completed,
        IntentionRecordedStatus::Abandoned => IntentionStatus::Abandoned,
        IntentionRecordedStatus::Blocked => IntentionStatus::Blocked,
        IntentionRecordedStatus::Deferred => IntentionStatus::Deferred,
    }
}

fn review_action(action: &ReviewRecordedAction) -> ReviewDecisionAction {
    match action {
        ReviewRecordedAction::CaptureEvidence => ReviewDecisionAction::CaptureEvidence,
        ReviewRecordedAction::ResolveContradiction => ReviewDecisionAction::ResolveContradiction,
        ReviewRecordedAction::RefreshMemory => ReviewDecisionAction::RefreshMemory,
        ReviewRecordedAction::LinkMemory => ReviewDecisionAction::LinkMemory,
        ReviewRecordedAction::ConsolidatePattern => ReviewDecisionAction::ConsolidatePattern,
        ReviewRecordedAction::ReviewIntention => ReviewDecisionAction::ReviewIntention,
    }
}

fn review_outcome(outcome: &ReviewRecordedOutcome) -> ReviewDecisionOutcome {
    match outcome {
        ReviewRecordedOutcome::Resolved => ReviewDecisionOutcome::Resolved,
    }
}

#[cfg(test)]
mod tests {
    use crate::event::{
        EpisodeRecorded, EventEnvelope, FactAsserted, IntentionRecorded, IntentionRecordedKind,
        IntentionRecordedPriority, IntentionRecordedStatus, IntentionStatusChanged,
        IntentionUpdated, MemoryEvent, ProcedureRecorded, ProcedureRecordedKind, RelationRecorded,
    };
    use crate::model::{
        IntentionKind, IntentionPriority, IntentionStatus, MEMORY_DATA_VERSION, MemoryScope,
        MemoryScopeKind, ProcedureKind,
    };

    use super::project;

    #[test]
    fn projection_is_deterministic() {
        let events = vec![EventEnvelope::new(
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
        )];

        let first = project(&events);
        let second = project(&events);

        assert_eq!(first, second);
        assert_eq!(first.version, MEMORY_DATA_VERSION);
        assert_eq!(first.event_count, 1);
        assert_eq!(first.last_event_id.as_deref(), Some(events[0].id.as_str()));
    }

    #[test]
    fn projects_entities_from_episode_mentions_claims_and_links() {
        let events = vec![
            EventEnvelope::new(
                1,
                1000,
                MemoryEvent::EpisodeRecorded(EpisodeRecorded {
                    id: "episode_1".to_string(),
                    content: "Lena owns the release notes.".to_string(),
                    tags: vec!["product".to_string()],
                    mentions: vec![" Lena ".to_string(), "Release Notes".to_string()],
                    source_id: None,
                    source_position: None,
                    source_role: None,
                    scope: None,
                }),
            ),
            EventEnvelope::new(
                2,
                1001,
                MemoryEvent::FactAsserted(FactAsserted {
                    id: "claim_1".to_string(),
                    subject: "Lena".to_string(),
                    predicate: "owns".to_string(),
                    object: "release notes".to_string(),
                    source_episode_id: Some("episode_1".to_string()),
                    confidence: 0.9,
                    scope: None,
                }),
            ),
            EventEnvelope::new(
                3,
                1002,
                MemoryEvent::RelationRecorded(RelationRecorded {
                    id: "link_1".to_string(),
                    from: "Lena".to_string(),
                    relation: "owns".to_string(),
                    to: "Release Notes".to_string(),
                    source_episode_id: Some("episode_1".to_string()),
                    confidence: 0.9,
                    scope: None,
                }),
            ),
        ];

        let data = project(&events);

        assert_eq!(data.entities.len(), 2);
        assert_eq!(data.entities[0].name, "Lena");
        assert_eq!(data.entities[0].mention_count, 3);
        assert_eq!(data.entities[1].name, "Release Notes");
        assert_eq!(data.entities[1].mention_count, 3);
        assert_eq!(data.claims, data.facts);
        assert_eq!(data.links, data.relations);
    }

    #[test]
    fn projects_procedures_and_preferences() {
        let events = vec![
            EventEnvelope::new(
                1,
                1000,
                MemoryEvent::ProcedureRecorded(ProcedureRecorded {
                    id: "procedure_1".to_string(),
                    kind: ProcedureRecordedKind::Procedure,
                    name: "Release notes".to_string(),
                    body: "Keep release notes concise.".to_string(),
                    source_episode_id: None,
                    confidence: 0.8,
                    scope: None,
                }),
            ),
            EventEnvelope::new(
                2,
                1001,
                MemoryEvent::ProcedureRecorded(ProcedureRecorded {
                    id: "preference_1".to_string(),
                    kind: ProcedureRecordedKind::Preference,
                    name: "Communication style".to_string(),
                    body: "Prefer direct Spanish updates.".to_string(),
                    source_episode_id: Some("episode_1".to_string()),
                    confidence: 0.95,
                    scope: None,
                }),
            ),
        ];

        let data = project(&events);

        assert_eq!(data.procedures.len(), 2);
        assert_eq!(data.procedures[0].kind, ProcedureKind::Procedure);
        assert_eq!(data.procedures[1].kind, ProcedureKind::Preference);
        assert_eq!(
            data.procedures[1].source_episode_id.as_deref(),
            Some("episode_1")
        );
    }

    #[test]
    fn projects_intention_lifecycle_state() {
        let events = vec![
            EventEnvelope::new(
                1,
                1000,
                MemoryEvent::IntentionRecorded(IntentionRecorded {
                    id: "intention_1".to_string(),
                    kind: IntentionRecordedKind::Goal,
                    priority: IntentionRecordedPriority::High,
                    description: "Ship the public release.".to_string(),
                    source_episode_id: None,
                    deadline_at_ms: None,
                    depends_on: Vec::new(),
                    goal_id: None,
                    progress_percent: None,
                    scope: None,
                }),
            ),
            EventEnvelope::new(
                2,
                1001,
                MemoryEvent::IntentionUpdated(IntentionUpdated {
                    id: "intention_1".to_string(),
                    description: Some("Ship the public release notes.".to_string()),
                    priority: Some(IntentionRecordedPriority::Medium),
                    deadline_at_ms: Some(Some(2000)),
                    depends_on: Some(vec!["intention_dependency".to_string()]),
                    goal_id: Some(Some("goal_public_release".to_string())),
                    progress_percent: Some(Some(40)),
                }),
            ),
            EventEnvelope::new(
                3,
                1002,
                MemoryEvent::IntentionStatusChanged(IntentionStatusChanged {
                    id: "intention_1".to_string(),
                    status: IntentionRecordedStatus::Blocked,
                    reason: Some("Waiting for release gate.".to_string()),
                }),
            ),
            EventEnvelope::new(
                4,
                1003,
                MemoryEvent::IntentionStatusChanged(IntentionStatusChanged {
                    id: "intention_1".to_string(),
                    status: IntentionRecordedStatus::Completed,
                    reason: None,
                }),
            ),
        ];

        let data = project(&events);

        assert_eq!(data.intentions.len(), 1);
        assert_eq!(data.intentions[0].kind, IntentionKind::Goal);
        assert_eq!(data.intentions[0].priority, IntentionPriority::Medium);
        assert_eq!(
            data.intentions[0].description,
            "Ship the public release notes."
        );
        assert_eq!(data.intentions[0].deadline_at_ms, Some(2000));
        assert_eq!(
            data.intentions[0].depends_on,
            vec!["intention_dependency".to_string()]
        );
        assert_eq!(
            data.intentions[0].goal_id.as_deref(),
            Some("goal_public_release")
        );
        assert_eq!(data.intentions[0].progress_percent, Some(40));
        assert_eq!(data.intentions[0].status, IntentionStatus::Completed);
        assert_eq!(data.intentions[0].updated_at_ms, 1003);
        assert_eq!(
            data.intentions[0].updated_event_id.as_str(),
            events[3].id.as_str()
        );
    }

    #[test]
    fn scoped_entities_do_not_merge_with_unscoped_entities() {
        let scope = MemoryScope::new(MemoryScopeKind::Project, "Nahuali").unwrap();
        let events = vec![
            EventEnvelope::new(
                1,
                1000,
                MemoryEvent::EpisodeRecorded(EpisodeRecorded {
                    id: "episode_unscoped".to_string(),
                    content: "Lena owns release notes.".to_string(),
                    tags: Vec::new(),
                    mentions: vec!["Lena".to_string()],
                    source_id: None,
                    source_position: None,
                    source_role: None,
                    scope: None,
                }),
            ),
            EventEnvelope::new(
                2,
                1001,
                MemoryEvent::EpisodeRecorded(EpisodeRecorded {
                    id: "episode_scoped".to_string(),
                    content: "Lena owns release notes.".to_string(),
                    tags: Vec::new(),
                    mentions: vec!["Lena".to_string()],
                    source_id: None,
                    source_position: None,
                    source_role: None,
                    scope: Some(scope.clone()),
                }),
            ),
        ];

        let data = project(&events);

        assert_eq!(data.entities.len(), 2);
        assert!(data.entities.iter().any(|entity| entity.scope.is_none()));
        assert!(
            data.entities
                .iter()
                .any(|entity| entity.scope == Some(scope.clone()))
        );
    }
}
