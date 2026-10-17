//! Explicit bulk closure of intentions, behind `reconcile-intentions --close`.
//!
//! Closure never selects intentions on its own: the caller names every id.
//! Planning validates the whole selection before anything is written, and
//! each closure is then an ordinary lifecycle event with the operator's
//! reason, so the ledger audit shows it like any single status change.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{IntentionStatus, MemoryData, NahualiError, Result};

/// Current intention closure report format version.
pub const INTENTION_CLOSURE_VERSION: u32 = 1;

/// Options for closing an explicit set of intentions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntentionClosureOptions {
    /// Intention ids to close, as the operator selected them.
    pub ids: Vec<String>,
    /// Closing status: [`IntentionStatus::Completed`] or
    /// [`IntentionStatus::Abandoned`].
    pub status: IntentionStatus,
    /// Reason recorded on every closure event. Required.
    pub reason: String,
    /// Preview the closure without writing anything.
    pub dry_run: bool,
}

/// What a bulk closure changed, or would change in a dry run.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct IntentionClosureReport {
    /// Report format version.
    pub version: u32,
    /// Whether this was a preview; nothing is written when true.
    pub dry_run: bool,
    /// Closing status applied to every change.
    pub status: IntentionStatus,
    /// Reason recorded on every closure event.
    pub reason: String,
    /// Number of distinct ids requested.
    pub requested_count: usize,
    /// Intentions closed (or that would be closed), in request order.
    pub changes: Vec<IntentionClosureChange>,
    /// Requested intentions left untouched because they were already closed.
    pub skipped: Vec<IntentionClosureSkip>,
    /// Write failure that stopped an applied closure part-way, when any.
    /// `changes` then lists only what was written before it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<IntentionClosureFailure>,
}

/// One intention whose lifecycle changed (or would change).
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct IntentionClosureChange {
    /// Intention identifier.
    pub id: String,
    /// Intention description.
    pub description: String,
    /// Status before the closure.
    pub previous_status: IntentionStatus,
    /// Status after the closure.
    pub status: IntentionStatus,
    /// When the intention last changed before the closure.
    pub previous_updated_at_ms: u64,
    /// Ledger event that recorded the closure; `None` in a dry run.
    pub event_id: Option<String>,
}

/// One requested intention the closure did not touch.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct IntentionClosureSkip {
    /// Intention identifier.
    pub id: String,
    /// Intention description.
    pub description: String,
    /// Current status, already closed.
    pub status: IntentionStatus,
}

/// The write that failed and the ids not attempted because of it.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct IntentionClosureFailure {
    /// Intention whose closure event could not be written.
    pub id: String,
    /// Error reported by the store.
    pub message: String,
    /// Ids from the failing one onward that were not closed.
    pub not_closed: Vec<String>,
}

/// Validate the selection and plan the closure without writing anything.
///
/// Fails before any write when the status is not a closing one, the reason
/// is empty, no ids are given, or any id is unknown.
pub(crate) fn plan_intention_closure(
    data: &MemoryData,
    options: &IntentionClosureOptions,
) -> Result<IntentionClosureReport> {
    if !is_closed(&options.status) {
        return Err(NahualiError::InvalidIntentionUpdate {
            message: "bulk closure only marks intentions completed or abandoned".to_string(),
        });
    }
    let reason = options.reason.trim().to_string();
    if reason.is_empty() {
        return Err(NahualiError::InvalidIntentionUpdate {
            message: "a reason is required to close intentions in bulk".to_string(),
        });
    }
    let mut seen = BTreeSet::new();
    let ids = options
        .ids
        .iter()
        .map(|id| id.trim())
        .filter(|id| !id.is_empty() && seen.insert(*id))
        .collect::<Vec<_>>();
    if ids.is_empty() {
        return Err(NahualiError::InvalidIntentionUpdate {
            message: "no intention ids given; closure never selects intentions by itself"
                .to_string(),
        });
    }
    let unknown = ids
        .iter()
        .filter(|id| !data.intentions.iter().any(|intention| intention.id == **id))
        .copied()
        .collect::<Vec<_>>();
    if !unknown.is_empty() {
        return Err(NahualiError::UnknownIntention {
            id: unknown.join(", "),
        });
    }

    let mut changes = Vec::new();
    let mut skipped = Vec::new();
    for id in &ids {
        let intention = data
            .intentions
            .iter()
            .find(|intention| intention.id == *id)
            .expect("unknown ids were rejected above");
        if is_closed(&intention.status) {
            skipped.push(IntentionClosureSkip {
                id: intention.id.clone(),
                description: intention.description.clone(),
                status: intention.status.clone(),
            });
        } else {
            changes.push(IntentionClosureChange {
                id: intention.id.clone(),
                description: intention.description.clone(),
                previous_status: intention.status.clone(),
                status: options.status.clone(),
                previous_updated_at_ms: intention.updated_at_ms,
                event_id: None,
            });
        }
    }

    Ok(IntentionClosureReport {
        version: INTENTION_CLOSURE_VERSION,
        dry_run: options.dry_run,
        status: options.status.clone(),
        reason,
        requested_count: ids.len(),
        changes,
        skipped,
        failure: None,
    })
}

fn is_closed(status: &IntentionStatus) -> bool {
    matches!(
        status,
        IntentionStatus::Completed | IntentionStatus::Abandoned
    )
}

#[cfg(test)]
mod tests {
    use super::{IntentionClosureOptions, plan_intention_closure};
    use crate::{
        Intention, IntentionKind, IntentionPriority, IntentionStatus, MemoryData, NahualiError,
    };

    fn intention(id: &str, status: IntentionStatus) -> Intention {
        Intention {
            id: id.to_string(),
            event_id: format!("event_{id}"),
            updated_event_id: format!("event_{id}"),
            kind: IntentionKind::Task,
            status,
            priority: IntentionPriority::Medium,
            description: format!("{id} description"),
            source_episode_id: None,
            status_reason: None,
            deadline_at_ms: None,
            depends_on: Vec::new(),
            goal_id: None,
            progress_percent: None,
            scope: None,
            created_at_ms: 0,
            updated_at_ms: 7,
        }
    }

    fn data() -> MemoryData {
        MemoryData {
            intentions: vec![
                intention("active", IntentionStatus::Active),
                intention("blocked", IntentionStatus::Blocked),
                intention("done", IntentionStatus::Completed),
                intention("untouched", IntentionStatus::Active),
            ],
            ..MemoryData::default()
        }
    }

    fn options(ids: &[&str], status: IntentionStatus, reason: &str) -> IntentionClosureOptions {
        IntentionClosureOptions {
            ids: ids.iter().map(|id| id.to_string()).collect(),
            status,
            reason: reason.to_string(),
            dry_run: true,
        }
    }

    #[test]
    fn plans_only_the_named_open_intentions_and_skips_closed_ones() {
        let report = plan_intention_closure(
            &data(),
            &options(
                &[" active ", "blocked", "done", "active", ""],
                IntentionStatus::Abandoned,
                "  stale cleanup ",
            ),
        )
        .expect("plan builds");

        assert!(report.dry_run);
        assert_eq!(report.reason, "stale cleanup");
        assert_eq!(report.requested_count, 3);
        let changed = report
            .changes
            .iter()
            .map(|change| (change.id.as_str(), change.previous_status.clone()))
            .collect::<Vec<_>>();
        assert_eq!(
            changed,
            vec![
                ("active", IntentionStatus::Active),
                ("blocked", IntentionStatus::Blocked),
            ]
        );
        assert!(report.changes.iter().all(|change| {
            change.status == IntentionStatus::Abandoned && change.event_id.is_none()
        }));
        assert_eq!(report.skipped.len(), 1);
        assert_eq!(report.skipped[0].id, "done");
    }

    #[test]
    fn rejects_unknown_ids_before_planning_anything() {
        let error = plan_intention_closure(
            &data(),
            &options(
                &["active", "ghost", "phantom"],
                IntentionStatus::Completed,
                "done",
            ),
        )
        .expect_err("unknown ids fail");

        match error {
            NahualiError::UnknownIntention { id } => assert_eq!(id, "ghost, phantom"),
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn requires_ids_a_reason_and_a_closing_status() {
        let empty_ids = plan_intention_closure(
            &data(),
            &options(&[" "], IntentionStatus::Completed, "done"),
        );
        assert!(matches!(
            empty_ids,
            Err(NahualiError::InvalidIntentionUpdate { .. })
        ));
        let no_reason = plan_intention_closure(
            &data(),
            &options(&["active"], IntentionStatus::Completed, "  "),
        );
        assert!(matches!(
            no_reason,
            Err(NahualiError::InvalidIntentionUpdate { .. })
        ));
        let not_closing = plan_intention_closure(
            &data(),
            &options(&["active"], IntentionStatus::Deferred, "later"),
        );
        assert!(matches!(
            not_closing,
            Err(NahualiError::InvalidIntentionUpdate { .. })
        ));
    }
}
