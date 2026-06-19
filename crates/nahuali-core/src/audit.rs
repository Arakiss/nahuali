//! Non-mutating ledger audit/diff.
//!
//! The `memory_record` ledger is append-only and monotonic by `sequence`, so a
//! diff between two points is the contiguous slice of events after a lower bound
//! up to an upper bound. [`audit_events`] summarizes what was appended in that
//! range and restates the integrity of the history through the upper bound, so
//! an audit is self-contained evidence rather than a bare changelog.

use serde::{Deserialize, Serialize};

use crate::event::{EventEnvelope, MemoryEvent, RepairMaterialization};
use crate::model::MemoryScope;
use crate::store::MemoryEngine;

const SUMMARY_MAX_CHARS: usize = 100;

/// Bounds for a ledger audit. Sequence bounds anchor the range; the lower bound
/// is exclusive so "since the last checkpoint" excludes the checkpoint event
/// itself. Optional timestamp bounds narrow the range further.
#[derive(Debug, Clone, Default)]
pub struct LedgerAuditOptions {
    /// Exclusive lower sequence bound. `None` audits from the genesis event.
    pub from_sequence: Option<u64>,
    /// Inclusive upper sequence bound. `None` audits through the latest event.
    pub to_sequence: Option<u64>,
    /// Inclusive lower timestamp bound in milliseconds since the Unix epoch.
    pub since_ms: Option<u64>,
    /// Inclusive upper timestamp bound in milliseconds since the Unix epoch.
    pub until_ms: Option<u64>,
}

/// Ledger event category, mirroring [`MemoryEvent`] variants one to one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LedgerAuditEventKind {
    /// An external source was registered for provenance.
    SourceRecorded,
    /// A ground-truth episode was recorded.
    EpisodeRecorded,
    /// A derived fact (claim) was asserted.
    FactAsserted,
    /// A derived relation (link) was recorded.
    RelationRecorded,
    /// A procedure or preference was recorded.
    ProcedureRecorded,
    /// An intention was recorded.
    IntentionRecorded,
    /// An intention's metadata was updated.
    IntentionUpdated,
    /// An intention's lifecycle status was changed.
    IntentionStatusChanged,
    /// An operator review item was resolved.
    ReviewRecorded,
    /// An LLM-proposed repair was applied.
    RepairApplied,
}

/// A single change appended within the audited range.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerAuditEntry {
    /// Monotonic sequence number of the event.
    pub sequence: u64,
    /// Stable event identifier.
    pub id: String,
    /// Event timestamp in milliseconds since the Unix epoch.
    pub timestamp_ms: u64,
    /// Event category.
    pub kind: LedgerAuditEventKind,
    /// Short human-readable descriptor of the change.
    pub summary: String,
    /// Memory scope carried by the event, when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<MemoryScope>,
}

/// Per-category counts of changes within the audited range.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LedgerAuditCounts {
    /// Sources registered for provenance.
    pub sources_recorded: usize,
    /// Ground-truth episodes recorded.
    pub episodes_recorded: usize,
    /// Derived facts (claims) asserted.
    pub facts_asserted: usize,
    /// Derived relations (links) recorded.
    pub relations_recorded: usize,
    /// Procedures or preferences recorded.
    pub procedures_recorded: usize,
    /// Intentions recorded.
    pub intentions_recorded: usize,
    /// Intention metadata updates.
    pub intentions_updated: usize,
    /// Intention lifecycle status changes.
    pub intention_status_changes: usize,
    /// Operator review items resolved.
    pub reviews_recorded: usize,
    /// LLM-proposed repairs applied.
    pub repairs_applied: usize,
}

/// Restated integrity of the history through the upper bound of the audit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerAuditIntegrity {
    /// Every event through the upper bound passes its per-event checksum.
    pub checksums_valid: bool,
    /// Sequences through the upper bound are contiguous and ordered from 1.
    pub sequence_contiguous: bool,
    /// The tamper-evident hash chain through the upper bound is intact.
    #[cfg(feature = "tamper-evidence")]
    pub chain_intact: bool,
    /// Merkle commitment over the chained prefix through the upper bound: one
    /// root summarizing that these events existed in this order. A commitment
    /// for anchoring and inclusion proofs, not itself a proof and not a trust
    /// gate. `None` for an unchained or empty ledger.
    #[cfg(feature = "tamper-evidence")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merkle_root: Option<String>,
    /// Overall verdict over the checks that apply to this build.
    pub verified: bool,
}

/// A non-mutating diff of what changed in the ledger between two points.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerAudit {
    /// Resolved exclusive lower sequence bound (`0` audits from genesis).
    pub from_sequence: u64,
    /// Resolved inclusive upper sequence bound (`0` for an empty ledger).
    pub to_sequence: u64,
    /// Total events in the whole ledger.
    pub total_event_count: usize,
    /// Events that fall within the audited range.
    pub range_event_count: usize,
    /// Timestamp of the first event in the range, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_timestamp_ms: Option<u64>,
    /// Timestamp of the last event in the range, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_timestamp_ms: Option<u64>,
    /// Chain hash anchoring the lower bound, when the ledger is chained.
    #[cfg(feature = "tamper-evidence")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_tip: Option<String>,
    /// Chain hash anchoring the upper bound, when the ledger is chained.
    #[cfg(feature = "tamper-evidence")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_tip: Option<String>,
    /// Restated integrity of the history through the upper bound.
    pub integrity: LedgerAuditIntegrity,
    /// Per-category change counts within the range.
    pub counts: LedgerAuditCounts,
    /// The individual changes within the range, in ledger order.
    pub entries: Vec<LedgerAuditEntry>,
}

impl MemoryEngine {
    /// Audit what changed in the ledger between two points without mutating it.
    pub fn audit_ledger(&self, options: &LedgerAuditOptions) -> LedgerAudit {
        audit_events(self.events(), options)
    }
}

/// Audit a slice of validated event envelopes. Pure over its inputs so it can be
/// exercised with synthetic and deliberately broken ledgers in tests.
pub fn audit_events(events: &[EventEnvelope], options: &LedgerAuditOptions) -> LedgerAudit {
    let latest_sequence = events.last().map(|event| event.sequence).unwrap_or(0);
    let from_sequence = options.from_sequence.unwrap_or(0);
    let to_sequence = options
        .to_sequence
        .unwrap_or(latest_sequence)
        .min(latest_sequence);

    let in_range = |event: &EventEnvelope| -> bool {
        event.sequence > from_sequence
            && event.sequence <= to_sequence
            && options
                .since_ms
                .is_none_or(|since| event.timestamp_ms >= since)
            && options
                .until_ms
                .is_none_or(|until| event.timestamp_ms <= until)
    };

    let mut counts = LedgerAuditCounts::default();
    let mut entries = Vec::new();
    for event in events.iter().filter(|event| in_range(event)) {
        count_event(&mut counts, &event.payload);
        entries.push(LedgerAuditEntry {
            sequence: event.sequence,
            id: event.id.clone(),
            timestamp_ms: event.timestamp_ms,
            kind: event_kind(&event.payload),
            summary: summarize(&event.payload),
            scope: event_scope(&event.payload),
        });
    }

    let from_timestamp_ms = entries.first().map(|entry| entry.timestamp_ms);
    let to_timestamp_ms = entries.last().map(|entry| entry.timestamp_ms);

    let prefix_len = events
        .iter()
        .take_while(|event| event.sequence <= to_sequence)
        .count();
    let integrity = audit_integrity(&events[..prefix_len]);

    LedgerAudit {
        from_sequence,
        to_sequence,
        total_event_count: events.len(),
        range_event_count: entries.len(),
        from_timestamp_ms,
        to_timestamp_ms,
        #[cfg(feature = "tamper-evidence")]
        from_tip: tip_at(events, from_sequence),
        #[cfg(feature = "tamper-evidence")]
        to_tip: tip_at(events, to_sequence),
        integrity,
        counts,
        entries,
    }
}

fn audit_integrity(prefix: &[EventEnvelope]) -> LedgerAuditIntegrity {
    let checksums_valid = prefix.iter().all(EventEnvelope::validate_checksum);
    let sequence_contiguous = prefix
        .iter()
        .enumerate()
        .all(|(index, event)| event.sequence == index as u64 + 1);

    #[cfg(feature = "tamper-evidence")]
    let chain_intact = crate::verify_event_chain(prefix).is_none();
    #[cfg(feature = "tamper-evidence")]
    let merkle_root = crate::ledger_merkle_root(prefix);

    #[cfg(feature = "tamper-evidence")]
    let verified = checksums_valid && sequence_contiguous && chain_intact;
    #[cfg(not(feature = "tamper-evidence"))]
    let verified = checksums_valid && sequence_contiguous;

    LedgerAuditIntegrity {
        checksums_valid,
        sequence_contiguous,
        #[cfg(feature = "tamper-evidence")]
        chain_intact,
        #[cfg(feature = "tamper-evidence")]
        merkle_root,
        verified,
    }
}

/// Chain hash anchoring `sequence`, or `None` for the genesis anchor (`0`) or an
/// unchained ledger (a default-build or legacy store).
#[cfg(feature = "tamper-evidence")]
fn tip_at(events: &[EventEnvelope], sequence: u64) -> Option<String> {
    if sequence == 0 || !events.iter().any(EventEnvelope::is_chained) {
        return None;
    }
    events
        .iter()
        .find(|event| event.sequence == sequence)
        .map(EventEnvelope::chain_hash)
}

fn count_event(counts: &mut LedgerAuditCounts, payload: &MemoryEvent) {
    match payload {
        MemoryEvent::SourceRecorded(_) => counts.sources_recorded += 1,
        MemoryEvent::EpisodeRecorded(_) => counts.episodes_recorded += 1,
        MemoryEvent::FactAsserted(_) => counts.facts_asserted += 1,
        MemoryEvent::RelationRecorded(_) => counts.relations_recorded += 1,
        MemoryEvent::ProcedureRecorded(_) => counts.procedures_recorded += 1,
        MemoryEvent::IntentionRecorded(_) => counts.intentions_recorded += 1,
        MemoryEvent::IntentionUpdated(_) => counts.intentions_updated += 1,
        MemoryEvent::IntentionStatusChanged(_) => counts.intention_status_changes += 1,
        MemoryEvent::ReviewRecorded(_) => counts.reviews_recorded += 1,
        MemoryEvent::RepairApplied(_) => counts.repairs_applied += 1,
    }
}

fn event_kind(payload: &MemoryEvent) -> LedgerAuditEventKind {
    match payload {
        MemoryEvent::SourceRecorded(_) => LedgerAuditEventKind::SourceRecorded,
        MemoryEvent::EpisodeRecorded(_) => LedgerAuditEventKind::EpisodeRecorded,
        MemoryEvent::FactAsserted(_) => LedgerAuditEventKind::FactAsserted,
        MemoryEvent::RelationRecorded(_) => LedgerAuditEventKind::RelationRecorded,
        MemoryEvent::ProcedureRecorded(_) => LedgerAuditEventKind::ProcedureRecorded,
        MemoryEvent::IntentionRecorded(_) => LedgerAuditEventKind::IntentionRecorded,
        MemoryEvent::IntentionUpdated(_) => LedgerAuditEventKind::IntentionUpdated,
        MemoryEvent::IntentionStatusChanged(_) => LedgerAuditEventKind::IntentionStatusChanged,
        MemoryEvent::ReviewRecorded(_) => LedgerAuditEventKind::ReviewRecorded,
        MemoryEvent::RepairApplied(_) => LedgerAuditEventKind::RepairApplied,
    }
}

fn event_scope(payload: &MemoryEvent) -> Option<MemoryScope> {
    match payload {
        MemoryEvent::SourceRecorded(event) => event.scope.clone(),
        MemoryEvent::EpisodeRecorded(event) => event.scope.clone(),
        MemoryEvent::FactAsserted(event) => event.scope.clone(),
        MemoryEvent::RelationRecorded(event) => event.scope.clone(),
        MemoryEvent::ProcedureRecorded(event) => event.scope.clone(),
        MemoryEvent::IntentionRecorded(event) => event.scope.clone(),
        MemoryEvent::ReviewRecorded(event) => event.scope.clone(),
        MemoryEvent::RepairApplied(event) => match &event.materialized {
            RepairMaterialization::Claim(claim) => claim.scope.clone(),
            RepairMaterialization::Link(link) => link.scope.clone(),
        },
        MemoryEvent::IntentionUpdated(_) | MemoryEvent::IntentionStatusChanged(_) => None,
    }
}

fn summarize(payload: &MemoryEvent) -> String {
    match payload {
        MemoryEvent::SourceRecorded(event) => {
            truncate(event.title.clone().unwrap_or_else(|| event.id.clone()))
        }
        MemoryEvent::EpisodeRecorded(event) => truncate(event.content.clone()),
        MemoryEvent::FactAsserted(event) => truncate(format!(
            "{} {} {}",
            event.subject, event.predicate, event.object
        )),
        MemoryEvent::RelationRecorded(event) => {
            truncate(format!("{} {} {}", event.from, event.relation, event.to))
        }
        MemoryEvent::ProcedureRecorded(event) => truncate(event.name.clone()),
        MemoryEvent::IntentionRecorded(event) => truncate(event.description.clone()),
        MemoryEvent::IntentionUpdated(event) => format!("intention {}", event.id),
        MemoryEvent::IntentionStatusChanged(event) => {
            format!("intention {} -> {}", event.id, token(&event.status))
        }
        MemoryEvent::ReviewRecorded(event) => {
            truncate(format!("{} {}", event.review_id, token(&event.action)))
        }
        MemoryEvent::RepairApplied(event) => match &event.materialized {
            RepairMaterialization::Claim(claim) => truncate(format!(
                "repair {} {} {}",
                claim.subject, claim.predicate, claim.object
            )),
            RepairMaterialization::Link(link) => truncate(format!(
                "repair {} {} {}",
                link.from, link.relation, link.to
            )),
        },
    }
}

/// Render a serde enum as its serialized string token without coupling to its
/// variants (e.g. `IntentionRecordedStatus::Completed` -> `"completed"`).
fn token<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn truncate(text: String) -> String {
    if text.chars().count() <= SUMMARY_MAX_CHARS {
        return text;
    }
    let head: String = text.chars().take(SUMMARY_MAX_CHARS).collect();
    format!("{head}...")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{EpisodeRecorded, FactAsserted, IntentionRecorded};

    fn episode(content: &str) -> MemoryEvent {
        MemoryEvent::EpisodeRecorded(EpisodeRecorded {
            id: format!("episode-{content}"),
            content: content.to_string(),
            tags: Vec::new(),
            mentions: Vec::new(),
            source_id: None,
            source_position: None,
            source_role: None,
            scope: None,
        })
    }

    fn fact(subject: &str) -> MemoryEvent {
        MemoryEvent::FactAsserted(FactAsserted {
            id: format!("fact-{subject}"),
            subject: subject.to_string(),
            predicate: "owns".to_string(),
            object: "the release notes".to_string(),
            source_episode_id: None,
            confidence: 0.9,
            scope: None,
        })
    }

    fn intention(description: &str) -> MemoryEvent {
        MemoryEvent::IntentionRecorded(IntentionRecorded {
            id: format!("intention-{description}"),
            kind: crate::IntentionRecordedKind::Task,
            priority: crate::IntentionRecordedPriority::High,
            description: description.to_string(),
            source_episode_id: None,
            deadline_at_ms: None,
            depends_on: Vec::new(),
            goal_id: None,
            progress_percent: None,
            scope: None,
        })
    }

    fn ledger(payloads: Vec<MemoryEvent>) -> Vec<EventEnvelope> {
        payloads
            .into_iter()
            .enumerate()
            .map(|(index, payload)| {
                EventEnvelope::new(index as u64 + 1, (index as u64 + 1) * 1000, payload)
            })
            .collect()
    }

    #[test]
    fn audits_an_empty_ledger() {
        let audit = audit_events(&[], &LedgerAuditOptions::default());
        assert_eq!(audit.to_sequence, 0);
        assert_eq!(audit.range_event_count, 0);
        assert_eq!(audit.total_event_count, 0);
        assert!(audit.integrity.verified);
        assert!(audit.entries.is_empty());
    }

    #[test]
    fn audits_the_full_range_by_default() {
        let events = ledger(vec![
            episode("Lena owns the release notes"),
            fact("Lena"),
            intention("Ship release notes"),
        ]);
        let audit = audit_events(&events, &LedgerAuditOptions::default());

        assert_eq!(audit.from_sequence, 0);
        assert_eq!(audit.to_sequence, 3);
        assert_eq!(audit.range_event_count, 3);
        assert_eq!(audit.counts.episodes_recorded, 1);
        assert_eq!(audit.counts.facts_asserted, 1);
        assert_eq!(audit.counts.intentions_recorded, 1);
        assert_eq!(audit.from_timestamp_ms, Some(1000));
        assert_eq!(audit.to_timestamp_ms, Some(3000));
        assert!(audit.integrity.verified);
        assert_eq!(audit.entries[1].kind, LedgerAuditEventKind::FactAsserted);
        assert_eq!(audit.entries[1].summary, "Lena owns the release notes");
    }

    #[test]
    fn excludes_events_at_or_before_the_exclusive_lower_bound() {
        let events = ledger(vec![episode("first"), fact("Lena"), intention("ship")]);
        let audit = audit_events(
            &events,
            &LedgerAuditOptions {
                from_sequence: Some(1),
                ..Default::default()
            },
        );

        assert_eq!(audit.from_sequence, 1);
        assert_eq!(audit.range_event_count, 2);
        assert_eq!(audit.counts.episodes_recorded, 0);
        assert_eq!(audit.entries[0].sequence, 2);
    }

    #[test]
    fn caps_the_range_at_the_upper_bound() {
        let events = ledger(vec![episode("first"), fact("Lena"), intention("ship")]);
        let audit = audit_events(
            &events,
            &LedgerAuditOptions {
                to_sequence: Some(2),
                ..Default::default()
            },
        );

        assert_eq!(audit.to_sequence, 2);
        assert_eq!(audit.range_event_count, 2);
        assert!(audit.integrity.verified);
    }

    #[test]
    fn filters_the_range_by_timestamp() {
        let events = ledger(vec![episode("first"), fact("Lena"), intention("ship")]);
        let audit = audit_events(
            &events,
            &LedgerAuditOptions {
                since_ms: Some(2000),
                until_ms: Some(2000),
                ..Default::default()
            },
        );

        assert_eq!(audit.range_event_count, 1);
        assert_eq!(audit.entries[0].sequence, 2);
    }

    #[cfg(feature = "tamper-evidence")]
    #[test]
    fn reports_intact_chain_and_anchoring_tips() {
        let first = EventEnvelope::with_chain(1, 1000, episode("first"), None);
        let second = EventEnvelope::with_chain(2, 2000, fact("Lena"), Some(&first.chain_hash()));
        let events = vec![first, second];

        let audit = audit_events(&events, &LedgerAuditOptions::default());

        assert!(audit.integrity.chain_intact);
        assert!(audit.integrity.verified);
        assert!(audit.from_tip.is_none());
        assert_eq!(audit.to_tip, Some(events[1].chain_hash()));
        // The audit surfaces the Merkle commitment over the chained prefix.
        assert!(audit.integrity.merkle_root.is_some());
        assert_eq!(
            audit.integrity.merkle_root,
            crate::ledger_merkle_root(&events)
        );
    }
}
