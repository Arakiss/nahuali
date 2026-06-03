use nahuali_core::{
    GoalProgress, GoalProgressReport, IntentionReconciliationIssue, IntentionReconciliationReport,
};
use rmcp::schemars;
use serde::Serialize;

use super::json_string;

/// Reconciliation issue for a single intention.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct IntentionReconciliationIssueView {
    id: String,
    intention_id: String,
    kind: String,
    priority: String,
    title: String,
    detail: String,
    evidence_ids: Vec<String>,
}

impl From<IntentionReconciliationIssue> for IntentionReconciliationIssueView {
    fn from(issue: IntentionReconciliationIssue) -> Self {
        Self {
            kind: json_string(&issue.kind),
            priority: json_string(&issue.priority),
            id: issue.id,
            intention_id: issue.intention_id,
            title: issue.title,
            detail: issue.detail,
            evidence_ids: issue.evidence_ids,
        }
    }
}

/// Non-mutating reconciliation report for active work.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct IntentionReconciliationReportView {
    version: u32,
    generated_at_ms: u64,
    intention_count: usize,
    issue_count: usize,
    issues: Vec<IntentionReconciliationIssueView>,
}

impl From<IntentionReconciliationReport> for IntentionReconciliationReportView {
    fn from(report: IntentionReconciliationReport) -> Self {
        Self {
            version: report.version,
            generated_at_ms: report.generated_at_ms,
            intention_count: report.intention_count,
            issue_count: report.issue_count,
            issues: report
                .issues
                .into_iter()
                .map(IntentionReconciliationIssueView::from)
                .collect(),
        }
    }
}

/// Progress aggregate for one goal intention.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct GoalProgressView {
    goal_id: String,
    description: String,
    status: String,
    explicit_progress_percent: Option<u8>,
    derived_progress_percent: u8,
    child_count: usize,
    completed_count: usize,
    active_count: usize,
    blocked_count: usize,
    deferred_count: usize,
    abandoned_count: usize,
    child_ids: Vec<String>,
}

impl From<GoalProgress> for GoalProgressView {
    fn from(goal: GoalProgress) -> Self {
        Self {
            status: json_string(&goal.status),
            goal_id: goal.goal_id,
            description: goal.description,
            explicit_progress_percent: goal.explicit_progress_percent,
            derived_progress_percent: goal.derived_progress_percent,
            child_count: goal.child_count,
            completed_count: goal.completed_count,
            active_count: goal.active_count,
            blocked_count: goal.blocked_count,
            deferred_count: goal.deferred_count,
            abandoned_count: goal.abandoned_count,
            child_ids: goal.child_ids,
        }
    }
}

/// Non-mutating goal progress report.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct GoalProgressReportView {
    version: u32,
    generated_at_ms: u64,
    goal_count: usize,
    goals: Vec<GoalProgressView>,
}

impl From<GoalProgressReport> for GoalProgressReportView {
    fn from(report: GoalProgressReport) -> Self {
        Self {
            version: report.version,
            generated_at_ms: report.generated_at_ms,
            goal_count: report.goal_count,
            goals: report
                .goals
                .into_iter()
                .map(GoalProgressView::from)
                .collect(),
        }
    }
}
