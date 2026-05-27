use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::{Intention, IntentionStatus, MemoryData};

/// Current intention reconciliation report format version.
pub const INTENTION_RECONCILIATION_VERSION: u32 = 1;

/// Current goal-progress report format version.
pub const GOAL_PROGRESS_VERSION: u32 = 1;

/// Default stale active-intention threshold in milliseconds.
pub const DEFAULT_INTENTION_STALE_AFTER_MS: u64 = 30 * 24 * 60 * 60 * 1000;

/// Options for updating intention metadata.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IntentionUpdateOptions {
    /// New intention description, when changed.
    pub description: Option<String>,
    /// New intention priority, when changed.
    pub priority: Option<crate::IntentionPriority>,
    /// Deadline update. `Some(None)` clears the deadline.
    pub deadline_at_ms: Option<Option<u64>>,
    /// Full dependency replacement. An empty vector clears dependencies.
    pub depends_on: Option<Vec<String>>,
    /// Parent goal update. `Some(None)` clears the parent goal.
    pub goal_id: Option<Option<String>>,
    /// Progress update. `Some(None)` clears progress.
    pub progress_percent: Option<Option<u8>>,
}

/// Options for building a non-mutating intention reconciliation report.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct IntentionReconciliationOptions {
    /// Timestamp in milliseconds used for deadline and staleness checks.
    pub now_ms: u64,
    /// Active intentions older than this are surfaced for review. Zero disables staleness.
    pub stale_after_ms: u64,
}

impl Default for IntentionReconciliationOptions {
    fn default() -> Self {
        Self {
            now_ms: now_ms(),
            stale_after_ms: DEFAULT_INTENTION_STALE_AFTER_MS,
        }
    }
}

/// Non-mutating reconciliation report for active work.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct IntentionReconciliationReport {
    /// Report format version.
    pub version: u32,
    /// Timestamp in milliseconds when the report was generated.
    pub generated_at_ms: u64,
    /// Number of intentions inspected.
    pub intention_count: usize,
    /// Number of issues returned.
    pub issue_count: usize,
    /// Non-mutating reconciliation issues.
    pub issues: Vec<IntentionReconciliationIssue>,
}

/// Reconciliation issue for a single intention.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct IntentionReconciliationIssue {
    /// Stable issue identifier.
    pub id: String,
    /// Intention that needs attention.
    pub intention_id: String,
    /// Issue category.
    pub kind: IntentionReconciliationIssueKind,
    /// Operational priority for the issue.
    pub priority: IntentionReconciliationPriority,
    /// Human-readable title.
    pub title: String,
    /// Specific non-mutating detail.
    pub detail: String,
    /// Evidence event or memory identifiers.
    pub evidence_ids: Vec<String>,
}

/// Reconciliation issue category.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IntentionReconciliationIssueKind {
    /// Active intention deadline is in the past.
    Overdue,
    /// Active intention is waiting on another non-completed intention.
    WaitingOnDependency,
    /// Intention references a dependency that does not exist.
    MissingDependency,
    /// Intention is blocked and should be reviewed.
    Blocked,
    /// Intention is deferred and should be reviewed.
    Deferred,
    /// Active intention has not changed within the configured threshold.
    Stale,
    /// Goal appears ready for operator completion review.
    GoalReadyForReview,
}

/// Reconciliation issue priority.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum IntentionReconciliationPriority {
    /// Low-priority review.
    Low,
    /// Medium-priority review.
    Medium,
    /// High-priority review.
    High,
    /// Critical review.
    Critical,
}

/// Non-mutating goal progress report.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct GoalProgressReport {
    /// Report format version.
    pub version: u32,
    /// Timestamp in milliseconds when the report was generated.
    pub generated_at_ms: u64,
    /// Number of goal rows returned.
    pub goal_count: usize,
    /// Goal progress rows.
    pub goals: Vec<GoalProgress>,
}

/// Progress aggregate for one goal intention.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct GoalProgress {
    /// Goal intention identifier.
    pub goal_id: String,
    /// Goal description.
    pub description: String,
    /// Current goal lifecycle status.
    pub status: IntentionStatus,
    /// Operator-provided goal progress, when available.
    pub explicit_progress_percent: Option<u8>,
    /// Derived child-completion progress from linked child intentions.
    pub derived_progress_percent: u8,
    /// Child intention count.
    pub child_count: usize,
    /// Completed child count.
    pub completed_count: usize,
    /// Active child count.
    pub active_count: usize,
    /// Blocked child count.
    pub blocked_count: usize,
    /// Deferred child count.
    pub deferred_count: usize,
    /// Abandoned child count.
    pub abandoned_count: usize,
    /// Child intention identifiers.
    pub child_ids: Vec<String>,
}

pub(crate) fn reconcile_intentions(
    data: &MemoryData,
    options: IntentionReconciliationOptions,
) -> IntentionReconciliationReport {
    let mut issues = Vec::new();

    for intention in &data.intentions {
        if matches!(intention.status, IntentionStatus::Active) {
            if let Some(deadline_at_ms) = intention.deadline_at_ms
                && deadline_at_ms < options.now_ms
            {
                issues.push(issue(
                    intention,
                    IntentionReconciliationIssueKind::Overdue,
                    IntentionReconciliationPriority::High,
                    "Overdue intention",
                    format!(
                        "Deadline {deadline_at_ms} is before reconciliation time {}.",
                        options.now_ms
                    ),
                ));
            }

            for dependency_id in &intention.depends_on {
                match data
                    .intentions
                    .iter()
                    .find(|candidate| candidate.id == *dependency_id)
                {
                    Some(dependency)
                        if !matches!(dependency.status, IntentionStatus::Completed) =>
                    {
                        issues.push(issue(
                            intention,
                            IntentionReconciliationIssueKind::WaitingOnDependency,
                            IntentionReconciliationPriority::Medium,
                            "Waiting on dependency",
                            format!(
                                "Dependency {} is {:?}, not completed.",
                                dependency.id, dependency.status
                            ),
                        ));
                    }
                    None => issues.push(issue(
                        intention,
                        IntentionReconciliationIssueKind::MissingDependency,
                        IntentionReconciliationPriority::High,
                        "Missing dependency",
                        format!("Dependency {dependency_id} does not exist."),
                    )),
                    _ => {}
                }
            }

            if options.stale_after_ms > 0
                && intention
                    .updated_at_ms
                    .saturating_add(options.stale_after_ms)
                    < options.now_ms
            {
                issues.push(issue(
                    intention,
                    IntentionReconciliationIssueKind::Stale,
                    IntentionReconciliationPriority::Low,
                    "Stale active intention",
                    format!(
                        "Intention has not changed since {}.",
                        intention.updated_at_ms
                    ),
                ));
            }
        }

        if matches!(intention.status, IntentionStatus::Blocked) {
            issues.push(issue(
                intention,
                IntentionReconciliationIssueKind::Blocked,
                IntentionReconciliationPriority::Medium,
                "Blocked intention",
                intention
                    .status_reason
                    .clone()
                    .unwrap_or_else(|| "Blocked intention needs operator review.".to_string()),
            ));
        }

        if matches!(intention.status, IntentionStatus::Deferred) {
            issues.push(issue(
                intention,
                IntentionReconciliationIssueKind::Deferred,
                IntentionReconciliationPriority::Low,
                "Deferred intention",
                intention
                    .status_reason
                    .clone()
                    .unwrap_or_else(|| "Deferred intention should be reviewed later.".to_string()),
            ));
        }
    }

    for goal in goal_progress(data).goals {
        if goal.child_count > 0
            && goal.completed_count == goal.child_count
            && !matches!(goal.status, IntentionStatus::Completed)
            && let Some(intention) = data
                .intentions
                .iter()
                .find(|intention| intention.id == goal.goal_id)
        {
            issues.push(issue(
                intention,
                IntentionReconciliationIssueKind::GoalReadyForReview,
                IntentionReconciliationPriority::Medium,
                "Goal ready for completion review",
                "All child intentions are completed; operator should review the parent goal."
                    .to_string(),
            ));
        }
    }

    issues.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.intention_id.cmp(&right.intention_id))
            .then_with(|| left.id.cmp(&right.id))
    });

    IntentionReconciliationReport {
        version: INTENTION_RECONCILIATION_VERSION,
        generated_at_ms: options.now_ms,
        intention_count: data.intentions.len(),
        issue_count: issues.len(),
        issues,
    }
}

pub(crate) fn goal_progress(data: &MemoryData) -> GoalProgressReport {
    let mut goals = data
        .intentions
        .iter()
        .filter(|intention| matches!(intention.kind, crate::IntentionKind::Goal))
        .map(|goal| {
            let children = data
                .intentions
                .iter()
                .filter(|intention| intention.goal_id.as_deref() == Some(goal.id.as_str()))
                .collect::<Vec<_>>();
            let child_count = children.len();
            let completed_count = children
                .iter()
                .filter(|intention| matches!(intention.status, IntentionStatus::Completed))
                .count();
            let active_count = children
                .iter()
                .filter(|intention| matches!(intention.status, IntentionStatus::Active))
                .count();
            let blocked_count = children
                .iter()
                .filter(|intention| matches!(intention.status, IntentionStatus::Blocked))
                .count();
            let deferred_count = children
                .iter()
                .filter(|intention| matches!(intention.status, IntentionStatus::Deferred))
                .count();
            let abandoned_count = children
                .iter()
                .filter(|intention| matches!(intention.status, IntentionStatus::Abandoned))
                .count();
            let derived_progress_percent = completed_count
                .saturating_mul(100)
                .checked_div(child_count)
                .map(|progress| progress as u8)
                .unwrap_or_else(|| goal.progress_percent.unwrap_or(0));
            let child_ids = children
                .into_iter()
                .map(|intention| intention.id.clone())
                .collect::<Vec<_>>();

            GoalProgress {
                goal_id: goal.id.clone(),
                description: goal.description.clone(),
                status: goal.status.clone(),
                explicit_progress_percent: goal.progress_percent,
                derived_progress_percent,
                child_count,
                completed_count,
                active_count,
                blocked_count,
                deferred_count,
                abandoned_count,
                child_ids,
            }
        })
        .collect::<Vec<_>>();
    goals.sort_by(|left, right| left.goal_id.cmp(&right.goal_id));

    GoalProgressReport {
        version: GOAL_PROGRESS_VERSION,
        generated_at_ms: now_ms(),
        goal_count: goals.len(),
        goals,
    }
}

fn issue(
    intention: &Intention,
    kind: IntentionReconciliationIssueKind,
    priority: IntentionReconciliationPriority,
    title: impl Into<String>,
    detail: impl Into<String>,
) -> IntentionReconciliationIssue {
    let kind_key = format!("{kind:?}").to_ascii_lowercase();
    IntentionReconciliationIssue {
        id: format!("intention_reconcile_{}_{}", intention.id, kind_key),
        intention_id: intention.id.clone(),
        kind,
        priority,
        title: title.into(),
        detail: detail.into(),
        evidence_ids: vec![intention.updated_event_id.clone()],
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use crate::{
        Intention, IntentionKind, IntentionPriority, IntentionStatus, MemoryData,
        intention::{
            IntentionReconciliationIssueKind, IntentionReconciliationOptions, goal_progress,
            reconcile_intentions,
        },
    };

    #[test]
    fn reconciles_overdue_and_dependency_blocked_intentions() {
        let data = MemoryData {
            intentions: vec![
                intention("goal_1", IntentionKind::Goal, IntentionStatus::Active),
                Intention {
                    id: "task_1".to_string(),
                    description: "Ship release notes".to_string(),
                    deadline_at_ms: Some(50),
                    depends_on: vec!["task_2".to_string(), "missing".to_string()],
                    goal_id: Some("goal_1".to_string()),
                    updated_at_ms: 10,
                    ..intention("task_1", IntentionKind::Task, IntentionStatus::Active)
                },
                intention("task_2", IntentionKind::Task, IntentionStatus::Blocked),
            ],
            ..MemoryData::default()
        };

        let report = reconcile_intentions(
            &data,
            IntentionReconciliationOptions {
                now_ms: 100,
                stale_after_ms: 20,
            },
        );

        assert_eq!(report.intention_count, 3);
        assert!(report.issues.iter().any(|issue| {
            issue.kind == IntentionReconciliationIssueKind::Overdue
                && issue.intention_id == "task_1"
        }));
        assert!(report.issues.iter().any(|issue| {
            issue.kind == IntentionReconciliationIssueKind::WaitingOnDependency
                && issue.intention_id == "task_1"
        }));
        assert!(report.issues.iter().any(|issue| {
            issue.kind == IntentionReconciliationIssueKind::MissingDependency
                && issue.intention_id == "task_1"
        }));
    }

    #[test]
    fn reports_goal_progress_from_child_intentions() {
        let data = MemoryData {
            intentions: vec![
                intention("goal_1", IntentionKind::Goal, IntentionStatus::Active),
                Intention {
                    goal_id: Some("goal_1".to_string()),
                    ..intention("task_1", IntentionKind::Task, IntentionStatus::Completed)
                },
                Intention {
                    goal_id: Some("goal_1".to_string()),
                    ..intention("task_2", IntentionKind::Task, IntentionStatus::Active)
                },
            ],
            ..MemoryData::default()
        };

        let report = goal_progress(&data);

        assert_eq!(report.goal_count, 1);
        assert_eq!(report.goals[0].child_count, 2);
        assert_eq!(report.goals[0].completed_count, 1);
        assert_eq!(report.goals[0].derived_progress_percent, 50);
    }

    fn intention(id: &str, kind: IntentionKind, status: IntentionStatus) -> Intention {
        Intention {
            id: id.to_string(),
            event_id: format!("event_{id}"),
            updated_event_id: format!("event_{id}"),
            kind,
            status,
            priority: IntentionPriority::Medium,
            description: id.to_string(),
            source_episode_id: None,
            status_reason: None,
            deadline_at_ms: None,
            depends_on: Vec::new(),
            goal_id: None,
            progress_percent: None,
            scope: None,
            created_at_ms: 0,
            updated_at_ms: 0,
        }
    }
}
