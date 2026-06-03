use nahuali_core::{
    AnomalyAcknowledgementReport, AnomalyAlert, AnomalyReport, AnomalySummary, CaptureOpportunity,
    DeadlineReport, DeadlineSignal, DeadlineSummary, MemoryProactiveReport, ProactiveSummary,
};
use rmcp::schemars;
use serde::Serialize;

use super::{AuthorityDecisionView, OperatorReviewItemView, WriteBackPolicyView, json_string};

/// Aggregate counts for a proactive operator report.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct ProactiveSummaryView {
    deadline_count: usize,
    overdue_deadline_count: usize,
    due_soon_deadline_count: usize,
    anomaly_count: usize,
    critical_anomaly_count: usize,
    high_anomaly_count: usize,
    capture_opportunity_count: usize,
    high_risk_review_count: usize,
    should_pause: bool,
}

impl From<ProactiveSummary> for ProactiveSummaryView {
    fn from(summary: ProactiveSummary) -> Self {
        Self {
            deadline_count: summary.deadline_count,
            overdue_deadline_count: summary.overdue_deadline_count,
            due_soon_deadline_count: summary.due_soon_deadline_count,
            anomaly_count: summary.anomaly_count,
            critical_anomaly_count: summary.critical_anomaly_count,
            high_anomaly_count: summary.high_anomaly_count,
            capture_opportunity_count: summary.capture_opportunity_count,
            high_risk_review_count: summary.high_risk_review_count,
            should_pause: summary.should_pause,
        }
    }
}

/// A proactive evidence-capture opportunity.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct CaptureOpportunityView {
    id: String,
    review_id: String,
    priority: String,
    title: String,
    detail: String,
    evidence_ids: Vec<String>,
    suggested_action: String,
}

impl From<CaptureOpportunity> for CaptureOpportunityView {
    fn from(opportunity: CaptureOpportunity) -> Self {
        Self {
            priority: json_string(&opportunity.priority),
            id: opportunity.id,
            review_id: opportunity.review_id,
            title: opportunity.title,
            detail: opportunity.detail,
            evidence_ids: opportunity.evidence_ids,
            suggested_action: opportunity.suggested_action,
        }
    }
}

/// Aggregate deadline counts.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct DeadlineSummaryView {
    deadline_count: usize,
    overdue_count: usize,
    due_soon_count: usize,
    scheduled_count: usize,
}

impl From<DeadlineSummary> for DeadlineSummaryView {
    fn from(summary: DeadlineSummary) -> Self {
        Self {
            deadline_count: summary.deadline_count,
            overdue_count: summary.overdue_count,
            due_soon_count: summary.due_soon_count,
            scheduled_count: summary.scheduled_count,
        }
    }
}

/// A single deadline signal derived from intention metadata.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct DeadlineSignalView {
    id: String,
    intention_id: String,
    description: String,
    intention_priority: String,
    status: String,
    deadline_at_ms: u64,
    state: String,
    priority: String,
    detail: String,
    evidence_ids: Vec<String>,
}

impl From<DeadlineSignal> for DeadlineSignalView {
    fn from(signal: DeadlineSignal) -> Self {
        Self {
            intention_priority: json_string(&signal.intention_priority),
            status: json_string(&signal.status),
            state: json_string(&signal.state),
            priority: json_string(&signal.priority),
            id: signal.id,
            intention_id: signal.intention_id,
            description: signal.description,
            deadline_at_ms: signal.deadline_at_ms,
            detail: signal.detail,
            evidence_ids: signal.evidence_ids,
        }
    }
}

/// Deadline-only report derived from intention metadata.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct DeadlineReportView {
    version: u32,
    generated_at_ms: u64,
    now_ms: u64,
    horizon_ms: u64,
    summary: DeadlineSummaryView,
    deadlines: Vec<DeadlineSignalView>,
}

impl From<DeadlineReport> for DeadlineReportView {
    fn from(report: DeadlineReport) -> Self {
        Self {
            version: report.version,
            generated_at_ms: report.generated_at_ms,
            now_ms: report.now_ms,
            horizon_ms: report.horizon_ms,
            summary: DeadlineSummaryView::from(report.summary),
            deadlines: report
                .deadlines
                .into_iter()
                .map(DeadlineSignalView::from)
                .collect(),
        }
    }
}

/// Aggregate anomaly counts.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct AnomalySummaryView {
    critical_count: usize,
    high_count: usize,
    medium_count: usize,
    low_count: usize,
}

impl From<AnomalySummary> for AnomalySummaryView {
    fn from(summary: AnomalySummary) -> Self {
        Self {
            critical_count: summary.critical_count,
            high_count: summary.high_count,
            medium_count: summary.medium_count,
            low_count: summary.low_count,
        }
    }
}

/// A single proactive anomaly alert.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct AnomalyAlertView {
    id: String,
    kind: String,
    priority: String,
    title: String,
    detail: String,
    evidence_ids: Vec<String>,
    source_id: String,
    review_id: Option<String>,
    suggested_action: String,
}

impl From<AnomalyAlert> for AnomalyAlertView {
    fn from(alert: AnomalyAlert) -> Self {
        Self {
            kind: json_string(&alert.kind),
            priority: json_string(&alert.priority),
            id: alert.id,
            title: alert.title,
            detail: alert.detail,
            evidence_ids: alert.evidence_ids,
            source_id: alert.source_id,
            review_id: alert.review_id,
            suggested_action: alert.suggested_action,
        }
    }
}

/// Non-mutating anomaly report surfacing actionable alerts.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct AnomalyReportView {
    version: u32,
    generated_at_ms: u64,
    summary: AnomalySummaryView,
    alert_count: usize,
    alerts: Vec<AnomalyAlertView>,
}

impl From<AnomalyReport> for AnomalyReportView {
    fn from(report: AnomalyReport) -> Self {
        Self {
            version: report.version,
            generated_at_ms: report.generated_at_ms,
            summary: AnomalySummaryView::from(report.summary),
            alert_count: report.alert_count,
            alerts: report
                .alerts
                .into_iter()
                .map(AnomalyAlertView::from)
                .collect(),
        }
    }
}

/// Structured proactive operator report bundling deadlines and anomalies.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct MemoryProactiveReportView {
    version: u32,
    generated_at_ms: u64,
    event_count: usize,
    authority: AuthorityDecisionView,
    summary: ProactiveSummaryView,
    deadlines: DeadlineReportView,
    anomalies: AnomalyReportView,
    capture_opportunities: Vec<CaptureOpportunityView>,
    high_risk_review_items: Vec<OperatorReviewItemView>,
    write_back_policy: WriteBackPolicyView,
}

impl From<MemoryProactiveReport> for MemoryProactiveReportView {
    fn from(report: MemoryProactiveReport) -> Self {
        Self {
            version: report.version,
            generated_at_ms: report.generated_at_ms,
            event_count: report.event_count,
            authority: AuthorityDecisionView::from(report.authority),
            summary: ProactiveSummaryView::from(report.summary),
            deadlines: DeadlineReportView::from(report.deadlines),
            anomalies: AnomalyReportView::from(report.anomalies),
            capture_opportunities: report
                .capture_opportunities
                .into_iter()
                .map(CaptureOpportunityView::from)
                .collect(),
            high_risk_review_items: report
                .high_risk_review_items
                .into_iter()
                .map(OperatorReviewItemView::from)
                .collect(),
            write_back_policy: WriteBackPolicyView::from(report.write_back_policy),
        }
    }
}

/// Result of planning or applying an anomaly acknowledgement.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct AnomalyAcknowledgementReportView {
    version: u32,
    dry_run: bool,
    applied: bool,
    anomaly_id: String,
    note: String,
    alert: AnomalyAlertView,
    evidence_ids: Vec<String>,
    decision_id: Option<String>,
    event_id: Option<String>,
    policy: String,
}

impl From<AnomalyAcknowledgementReport> for AnomalyAcknowledgementReportView {
    fn from(report: AnomalyAcknowledgementReport) -> Self {
        Self {
            version: report.version,
            dry_run: report.dry_run,
            applied: report.applied,
            anomaly_id: report.anomaly_id,
            note: report.note,
            alert: AnomalyAlertView::from(report.alert),
            evidence_ids: report.evidence_ids,
            decision_id: report.decision_id,
            event_id: report.event_id,
            policy: report.policy,
        }
    }
}
