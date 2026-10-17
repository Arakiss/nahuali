//! Explicit, audited erasure of memory (right to be forgotten).
//!
//! Forgetting is an operator command, never an automatic repair. It replaces
//! the payload of every affected ledger record with a [`RedactedRecord`] stub
//! and appends one [`MemoryForgotten`] event naming who asked, why, and which
//! records were redacted. Envelope ids, checksums, and chain links are kept, so
//! the hash chain, Merkle roots, and signed checkpoints over the history stay
//! valid while the erased content is gone from the ledger and every tier
//! derived from it.
//!
//! [`RedactedRecord`]: crate::event::RedactedRecord

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    error::{NahualiError, Result},
    event::{
        EventEnvelope, MemoryEvent, MemoryForgotten, RedactedRecordRef, RepairMaterialization,
    },
};

/// Request to forget memory by identifier.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ForgetOptions {
    /// Memory identifiers to forget: sources, episodes, claims, links,
    /// procedures, preferences, intentions, review decisions, or repairs.
    pub targets: Vec<String>,
    /// Why the memory is being forgotten. Recorded in the audit trail, so it
    /// must not repeat the erased content.
    pub reason: String,
    /// Person or process that requested the erasure.
    pub requested_by: Option<String>,
    /// Plan the erasure and report it without writing anything.
    pub dry_run: bool,
}

/// Outcome of a forget request.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ForgetReport {
    /// Forget-request identifier, absent on a dry run.
    pub id: Option<String>,
    /// Event identifier of the recorded forget request, absent on a dry run.
    pub event_id: Option<String>,
    /// Whether this was a non-mutating preview.
    pub dry_run: bool,
    /// When the memory was forgotten, absent on a dry run.
    pub forgotten_at_ms: Option<u64>,
    /// Operator-supplied reason.
    pub reason: String,
    /// Person or process that requested the erasure.
    pub requested_by: Option<String>,
    /// Memory identifiers the operator asked to forget.
    pub targets: Vec<String>,
    /// Every memory identifier erased, including derived memory cascaded from the targets.
    pub forgotten_ids: Vec<String>,
    /// Number of ledger records whose content is (or would be) redacted.
    pub redacted_event_count: usize,
    /// Ledger records whose content is (or would be) redacted.
    pub redacted_events: Vec<RedactedRecordRef>,
    /// Whether the graph projection was rebuilt without the erased memory.
    pub graph_projection_rebuilt: bool,
    /// Whether the Qdrant semantic index was rebuilt without the erased memory.
    pub semantic_index_rebuilt: bool,
}

#[derive(Debug)]
pub(crate) struct ForgetPlan {
    pub(crate) targets: Vec<String>,
    pub(crate) reason: String,
    pub(crate) requested_by: Option<String>,
    pub(crate) forgotten_ids: Vec<String>,
    pub(crate) redacted_indexes: Vec<usize>,
}

impl ForgetPlan {
    pub(crate) fn redacted_refs(&self, events: &[EventEnvelope]) -> Vec<RedactedRecordRef> {
        self.redacted_indexes
            .iter()
            .map(|&index| {
                let event = &events[index];
                RedactedRecordRef {
                    sequence: event.sequence,
                    event_id: event.id.clone(),
                    kind: event.payload.kind(),
                    chain_hash: event.link_hash(),
                }
            })
            .collect()
    }

    pub(crate) fn event(&self, id: String, events: &[EventEnvelope]) -> MemoryForgotten {
        MemoryForgotten {
            id,
            reason: self.reason.clone(),
            requested_by: self.requested_by.clone(),
            targets: self.targets.clone(),
            forgotten_ids: self.forgotten_ids.clone(),
            redacted_records: self.redacted_refs(events),
        }
    }

    pub(crate) fn preview(&self, events: &[EventEnvelope]) -> ForgetReport {
        let redacted_events = self.redacted_refs(events);
        ForgetReport {
            id: None,
            event_id: None,
            dry_run: true,
            forgotten_at_ms: None,
            reason: self.reason.clone(),
            requested_by: self.requested_by.clone(),
            targets: self.targets.clone(),
            forgotten_ids: self.forgotten_ids.clone(),
            redacted_event_count: redacted_events.len(),
            redacted_events,
            graph_projection_rebuilt: false,
            semantic_index_rebuilt: false,
        }
    }
}

/// Plan which ledger records a forget request redacts.
///
/// Forgetting cascades to memory derived from a forgotten item: a source takes
/// its episodes, an episode takes the claims, links, procedures, intentions,
/// repairs, and review decisions that cite it, and an intention takes its later
/// updates. Unrelated memory that merely mentions the same entity is kept.
pub(crate) fn plan_forget(events: &[EventEnvelope], options: &ForgetOptions) -> Result<ForgetPlan> {
    let reason = options.reason.trim().to_string();
    if reason.is_empty() {
        return Err(NahualiError::InvalidForgetRequest {
            message: "a reason is required".to_string(),
        });
    }
    let mut targets = Vec::new();
    for target in &options.targets {
        let target = target.trim();
        if !target.is_empty() && !targets.iter().any(|existing| existing == target) {
            targets.push(target.to_string());
        }
    }
    if targets.is_empty() {
        return Err(NahualiError::InvalidForgetRequest {
            message: "at least one memory id is required".to_string(),
        });
    }

    let live: Vec<LiveRecord<&str>> = events
        .iter()
        .enumerate()
        .map(|(index, event)| LiveRecord {
            index,
            event_id: event.id.as_str(),
            ids: memory_ids(&event.payload),
            parents: derived_from(&event.payload),
        })
        .filter(|record| !record.ids.is_empty())
        .collect();
    let known: BTreeSet<&str> = live
        .iter()
        .flat_map(|record| record.ids.iter().copied())
        .collect();
    if let Some(unknown) = targets
        .iter()
        .find(|target| !known.contains(target.as_str()))
    {
        return Err(NahualiError::UnknownForgetTarget {
            id: unknown.clone(),
        });
    }

    let target_ids = targets.iter().map(String::as_str).collect::<Vec<_>>();
    let (forgotten, redacted) = redaction_closure::<_, _, BTreeSet<_>>(&live, &target_ids);
    let forgotten_ids = forgotten.into_iter().map(str::to_string).collect();
    let redacted_indexes = live
        .iter()
        .zip(redacted)
        .filter_map(|(record, redacted)| redacted.then_some(record.index))
        .collect();

    Ok(ForgetPlan {
        targets,
        reason,
        requested_by: options
            .requested_by
            .as_deref()
            .map(str::trim)
            .filter(|requested_by| !requested_by.is_empty())
            .map(str::to_string),
        forgotten_ids,
        redacted_indexes,
    })
}

/// A ledger record that holds live memory, reduced to what the cascade reads.
///
/// The id lists are generic so Kani can use fixed arrays: nested heap
/// allocations make every access expensive to model.
pub(crate) struct LiveRecord<Id, List = Vec<Id>> {
    pub(crate) index: usize,
    pub(crate) event_id: Id,
    pub(crate) ids: List,
    pub(crate) parents: List,
}

/// The two set operations the forget cascade uses.
///
/// The cascade is generic over its id set so Kani can verify it with a bitset:
/// `BTreeSet` node operations on symbolic keys exhaust a bounded model checker
/// even at three records.
pub(crate) trait IdSet<Id>: Default {
    fn contains(&self, id: &Id) -> bool;
    fn insert(&mut self, id: Id);
}

impl<Id: Ord> IdSet<Id> for BTreeSet<Id> {
    fn contains(&self, id: &Id) -> bool {
        BTreeSet::contains(self, id)
    }

    fn insert(&mut self, id: Id) {
        BTreeSet::insert(self, id);
    }
}

/// Every memory id that forgetting `targets` erases, and whether each live
/// record is redacted: the least set of records closed under the cascade.
///
/// A record is redacted when it holds a forgotten id or cites a forgotten id or
/// the event id of a redacted record.
pub(crate) fn redaction_closure<Id: Copy, List: AsRef<[Id]>, Set: IdSet<Id>>(
    live: &[LiveRecord<Id, List>],
    targets: &[Id],
) -> (Set, Vec<bool>) {
    let mut forgotten = Set::default();
    for &target in targets {
        forgotten.insert(target);
    }
    let mut forgotten_event_ids = Set::default();
    let mut redacted = vec![false; live.len()];
    // Every pass but the last redacts at least one more record.
    for _ in 0..=live.len() {
        let mut changed = false;
        for (position, record) in live.iter().enumerate() {
            if redacted[position] {
                continue;
            }
            let hit = record.ids.as_ref().iter().any(|id| forgotten.contains(id))
                || record.parents.as_ref().iter().any(|parent| {
                    forgotten.contains(parent) || forgotten_event_ids.contains(parent)
                });
            if hit {
                redacted[position] = true;
                for &id in record.ids.as_ref() {
                    forgotten.insert(id);
                }
                forgotten_event_ids.insert(record.event_id);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    (forgotten, redacted)
}

/// Memory identifiers whose content a record carries.
pub(crate) fn memory_ids(payload: &MemoryEvent) -> Vec<&str> {
    match payload {
        MemoryEvent::SourceRecorded(event) => vec![&event.id],
        MemoryEvent::EpisodeRecorded(event) => vec![&event.id],
        MemoryEvent::FactAsserted(event) => vec![&event.id],
        MemoryEvent::RelationRecorded(event) => vec![&event.id],
        MemoryEvent::ProcedureRecorded(event) => vec![&event.id],
        MemoryEvent::IntentionRecorded(event) => vec![&event.id],
        MemoryEvent::IntentionUpdated(event) => vec![&event.id],
        MemoryEvent::IntentionStatusChanged(event) => vec![&event.id],
        MemoryEvent::ReviewRecorded(event) => vec![&event.id],
        MemoryEvent::RepairApplied(event) => vec![&event.id, event.materialized_id()],
        MemoryEvent::MemoryForgotten(_) | MemoryEvent::Redacted(_) => Vec::new(),
    }
}

/// Identifiers a record's memory was derived from or cites as evidence.
pub(crate) fn derived_from(payload: &MemoryEvent) -> Vec<&str> {
    match payload {
        MemoryEvent::EpisodeRecorded(event) => event.source_id.as_deref().into_iter().collect(),
        MemoryEvent::FactAsserted(event) => {
            event.source_episode_id.as_deref().into_iter().collect()
        }
        MemoryEvent::RelationRecorded(event) => {
            event.source_episode_id.as_deref().into_iter().collect()
        }
        MemoryEvent::ProcedureRecorded(event) => {
            event.source_episode_id.as_deref().into_iter().collect()
        }
        MemoryEvent::IntentionRecorded(event) => {
            event.source_episode_id.as_deref().into_iter().collect()
        }
        MemoryEvent::ReviewRecorded(event) => {
            event.evidence_ids.iter().map(String::as_str).collect()
        }
        MemoryEvent::RepairApplied(event) => {
            let materialized_source = match &event.materialized {
                RepairMaterialization::Claim(claim) => claim.source_episode_id.as_deref(),
                RepairMaterialization::Link(link) => link.source_episode_id.as_deref(),
            };
            event
                .evidence_ids
                .iter()
                .map(String::as_str)
                .chain(materialized_source)
                .collect()
        }
        MemoryEvent::SourceRecorded(_)
        | MemoryEvent::IntentionUpdated(_)
        | MemoryEvent::IntentionStatusChanged(_)
        | MemoryEvent::MemoryForgotten(_)
        | MemoryEvent::Redacted(_) => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::{ForgetOptions, plan_forget};
    use crate::{
        NahualiError,
        event::{
            EpisodeRecorded, EventEnvelope, FactAsserted, MemoryEvent, ReviewRecorded,
            ReviewRecordedAction, ReviewRecordedOutcome, SourceRecorded, SourceRecordedKind,
            verify_redactions,
        },
        projection::project,
    };

    fn ledger(payloads: Vec<MemoryEvent>) -> Vec<EventEnvelope> {
        let mut events: Vec<EventEnvelope> = Vec::new();
        for (index, payload) in payloads.into_iter().enumerate() {
            let sequence = index as u64 + 1;
            let previous = events.last().and_then(EventEnvelope::link_hash);
            let mut envelope = EventEnvelope::new(sequence, 1_000 + sequence, payload);
            envelope.prev_hash = Some(previous.unwrap_or_default());
            events.push(envelope);
        }
        events
    }

    fn source(id: &str) -> MemoryEvent {
        MemoryEvent::SourceRecorded(SourceRecorded {
            id: id.to_string(),
            kind: SourceRecordedKind::Conversation,
            title: Some("Chat with Petru".to_string()),
            uri: None,
            content_checksum: "abc".to_string(),
            byte_len: 3,
            metadata: Default::default(),
            scope: None,
        })
    }

    fn episode(id: &str, content: &str, source_id: Option<&str>) -> MemoryEvent {
        MemoryEvent::EpisodeRecorded(EpisodeRecorded {
            id: id.to_string(),
            content: content.to_string(),
            tags: Vec::new(),
            mentions: vec!["Petru".to_string()],
            source_id: source_id.map(str::to_string),
            source_position: None,
            source_role: None,
            scope: None,
        })
    }

    fn claim(id: &str, object: &str, source_episode_id: Option<&str>) -> MemoryEvent {
        MemoryEvent::FactAsserted(FactAsserted {
            id: id.to_string(),
            subject: "Petru".to_string(),
            predicate: "address".to_string(),
            object: object.to_string(),
            source_episode_id: source_episode_id.map(str::to_string),
            confidence: 0.9,
            scope: None,
        })
    }

    fn review(id: &str, evidence: &str) -> MemoryEvent {
        MemoryEvent::ReviewRecorded(ReviewRecorded {
            id: id.to_string(),
            review_id: "review_1".to_string(),
            finding_id: "finding_1".to_string(),
            action: ReviewRecordedAction::CaptureEvidence,
            outcome: ReviewRecordedOutcome::Resolved,
            note: "Checked the address".to_string(),
            evidence_ids: vec![evidence.to_string()],
            scope: None,
        })
    }

    fn options(targets: &[&str]) -> ForgetOptions {
        ForgetOptions {
            targets: targets.iter().map(|target| (*target).to_string()).collect(),
            reason: "GDPR art. 17 request".to_string(),
            requested_by: Some(" petru ".to_string()),
            dry_run: false,
        }
    }

    fn apply(events: &[EventEnvelope], targets: &[&str]) -> Vec<EventEnvelope> {
        let plan = plan_forget(events, &options(targets)).unwrap();
        let forgotten = plan.event("forget_1".to_string(), events);
        let mut redacted: Vec<EventEnvelope> = events.to_vec();
        for &index in &plan.redacted_indexes {
            redacted[index] = events[index].redacted("forget_1");
        }
        let sequence = events.len() as u64 + 1;
        let previous = redacted.last().and_then(EventEnvelope::link_hash);
        let mut envelope =
            EventEnvelope::new(sequence, 9_000, MemoryEvent::MemoryForgotten(forgotten));
        envelope.prev_hash = Some(previous.unwrap_or_default());
        redacted.push(envelope);
        redacted
    }

    #[test]
    fn forgetting_a_source_cascades_to_episodes_claims_and_reviews() {
        let events = ledger(vec![
            source("source_1"),
            episode(
                "episode_1",
                "Petru lives at Calle Luna 12",
                Some("source_1"),
            ),
            claim("claim_1", "Calle Luna 12", Some("episode_1")),
            review("decision_1", "claim_1"),
            episode("episode_2", "Petru likes hiking", None),
            claim("claim_2", "hiking", Some("episode_2")),
        ]);

        let plan = plan_forget(&events, &options(&["source_1"])).unwrap();

        assert_eq!(
            plan.forgotten_ids,
            vec!["claim_1", "decision_1", "episode_1", "source_1"]
        );
        assert_eq!(plan.redacted_indexes, vec![0, 1, 2, 3]);
        assert_eq!(plan.requested_by.as_deref(), Some("petru"));
    }

    #[test]
    fn forgetting_a_claim_keeps_its_source_episode() {
        let events = ledger(vec![
            episode("episode_1", "Petru is 41", None),
            claim("claim_1", "41", Some("episode_1")),
        ]);

        let plan = plan_forget(&events, &options(&["claim_1"])).unwrap();

        assert_eq!(plan.forgotten_ids, vec!["claim_1"]);
        assert_eq!(plan.redacted_indexes, vec![1]);
    }

    #[test]
    fn rejects_unknown_targets_missing_reason_and_already_forgotten_ids() {
        let events = ledger(vec![episode("episode_1", "Petru is 41", None)]);

        assert!(matches!(
            plan_forget(&events, &options(&["episode_404"])),
            Err(NahualiError::UnknownForgetTarget { id }) if id == "episode_404"
        ));
        assert!(matches!(
            plan_forget(
                &events,
                &ForgetOptions {
                    reason: "  ".to_string(),
                    ..options(&["episode_1"])
                }
            ),
            Err(NahualiError::InvalidForgetRequest { .. })
        ));
        assert!(matches!(
            plan_forget(&events, &options(&[" ", ""])),
            Err(NahualiError::InvalidForgetRequest { .. })
        ));

        let forgotten = apply(&events, &["episode_1"]);
        assert!(matches!(
            plan_forget(&forgotten, &options(&["episode_1"])),
            Err(NahualiError::UnknownForgetTarget { .. })
        ));
    }

    #[test]
    fn redaction_erases_content_and_keeps_the_chain_and_audit_trail() {
        let events = ledger(vec![
            episode("episode_1", "Petru lives at Calle Luna 12", None),
            claim("claim_1", "Calle Luna 12", Some("episode_1")),
            episode("episode_2", "Petru likes hiking", None),
        ]);

        let forgotten = apply(&events, &["episode_1"]);

        let encoded = serde_json::to_string(&forgotten).unwrap();
        assert!(!encoded.contains("Calle Luna"), "{encoded}");
        assert_eq!(verify_redactions(&forgotten), None);
        assert!(forgotten.iter().all(EventEnvelope::validate_checksum));
        for (original, kept) in events.iter().zip(&forgotten) {
            assert_eq!(original.id, kept.id);
            assert_eq!(original.link_hash(), kept.link_hash());
        }
        #[cfg(feature = "tamper-evidence")]
        {
            assert_eq!(crate::verify_event_chain(&forgotten), None);
            assert_eq!(
                crate::ledger_merkle_root(&forgotten[..3]),
                crate::ledger_merkle_root(&events)
            );
        }

        let data = project(&forgotten);
        assert_eq!(data.episodes.len(), 1);
        assert!(data.claims.is_empty());
        assert_eq!(data.forgotten.len(), 1);
        assert_eq!(
            data.forgotten[0].forgotten_ids,
            vec!["claim_1", "episode_1"]
        );
        assert_eq!(data.forgotten[0].redacted_event_count, 2);
        assert_eq!(data.forgotten[0].forgotten_at_ms, 9_000);
    }

    #[test]
    fn unauthorized_or_reverted_redactions_fail_closed() {
        let events = ledger(vec![
            episode("episode_1", "Petru lives at Calle Luna 12", None),
            episode("episode_2", "Petru likes hiking", None),
        ]);

        let mut silent = events.clone();
        silent[1] = events[1].redacted("forget_404");
        let violation = verify_redactions(&silent).expect("unauthorized stub is rejected");
        assert_eq!(violation.sequence, 2);

        let mut restored = apply(&events, &["episode_1"]);
        restored[0] = events[0].clone();
        let violation = verify_redactions(&restored).expect("un-forgotten content is rejected");
        assert_eq!(violation.sequence, 1);

        let mut swapped = apply(&events, &["episode_1"]);
        swapped[1] = events[1].redacted("forget_1");
        assert!(verify_redactions(&swapped).is_some());
    }
}
