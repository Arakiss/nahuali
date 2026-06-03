use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::{
    AuthorityDecision, HealthSeverity, HealthSignalKind, Intention, IntentionPriority,
    IntentionReconciliationIssueKind, IntentionReconciliationOptions,
    IntentionReconciliationPriority, IntentionStatus, KnowledgeHealth, MemoryData, NahualiError,
    OperatorReviewItem, OperatorReviewOptions, Result, ReviewDecisionOutcome,
    SelfInspectionReviewAction, SelfInspectionReviewPriority, SelfInspectionWriteBackPolicy,
    event::{ReviewRecorded, ReviewRecordedAction, ReviewRecordedOutcome},
    intention, operator_review, self_inspection,
};

/// Current proactive report format version.
pub const MEMORY_PROACTIVE_REPORT_VERSION: u32 = 1;

/// Current deadline report format version.
pub const MEMORY_DEADLINE_REPORT_VERSION: u32 = 1;

/// Current anomaly report format version.
pub const MEMORY_ANOMALY_REPORT_VERSION: u32 = 1;

/// Current anomaly acknowledgement report format version.
pub const MEMORY_ANOMALY_ACKNOWLEDGEMENT_VERSION: u32 = 1;

/// Default proactive deadline horizon in milliseconds.
pub const DEFAULT_PROACTIVE_DEADLINE_HORIZON_MS: u64 = 7 * 24 * 60 * 60 * 1000;

/// Options for non-mutating proactive reports.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProactiveOptions {
    /// Timestamp in milliseconds used for deadline and staleness checks.
    pub now_ms: u64,
    /// Future window considered actionable for deadline review.
    pub deadline_horizon_ms: u64,
    /// Active intentions older than this are surfaced for review. Zero disables staleness.
    pub stale_after_ms: u64,
    /// Maximum high-risk review items included in the proactive report.
    pub review_limit: usize,
}

impl Default for ProactiveOptions {
    fn default() -> Self {
        Self {
            now_ms: now_ms(),
            deadline_horizon_ms: DEFAULT_PROACTIVE_DEADLINE_HORIZON_MS,
            stale_after_ms: crate::DEFAULT_INTENTION_STALE_AFTER_MS,
            review_limit: 20,
        }
    }
}

/// Non-mutating proactive operator report.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct MemoryProactiveReport {
    /// Report format version.
    pub version: u32,
    /// Timestamp in milliseconds when the report was generated.
    pub generated_at_ms: u64,
    /// Number of source events represented by the reviewed projection.
    pub event_count: usize,
    /// Authority decision computed from the same projection.
    pub authority: AuthorityDecision,
    /// Aggregate proactive counts.
    pub summary: ProactiveSummary,
    /// Deadline report derived from intention metadata.
    pub deadlines: DeadlineReport,
    /// Anomaly report derived from health and intention reconciliation.
    pub anomalies: AnomalyReport,
    /// Evidence capture opportunities from the review queue.
    pub capture_opportunities: Vec<CaptureOpportunity>,
    /// Critical or high-priority review items requiring attention.
    pub high_risk_review_items: Vec<OperatorReviewItem>,
    /// Explicit policy for automatic write-back.
    pub write_back_policy: SelfInspectionWriteBackPolicy,
}

/// Aggregate proactive report counts.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProactiveSummary {
    /// Number of deadline signals.
    pub deadline_count: usize,
    /// Number of overdue deadlines.
    pub overdue_deadline_count: usize,
    /// Number of deadlines due within the configured horizon.
    pub due_soon_deadline_count: usize,
    /// Number of actionable anomaly alerts.
    pub anomaly_count: usize,
    /// Number of critical anomaly alerts.
    pub critical_anomaly_count: usize,
    /// Number of high-priority anomaly alerts.
    pub high_anomaly_count: usize,
    /// Number of evidence capture opportunities.
    pub capture_opportunity_count: usize,
    /// Number of high-risk review items included.
    pub high_risk_review_count: usize,
    /// Whether the caller should pause before trusting or continuing work.
    pub should_pause: bool,
}

/// Deadline-only report derived from intention metadata.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct DeadlineReport {
    /// Report format version.
    pub version: u32,
    /// Timestamp in milliseconds when the report was generated.
    pub generated_at_ms: u64,
    /// Timestamp in milliseconds used for deadline checks.
    pub now_ms: u64,
    /// Future window considered actionable for deadline review.
    pub horizon_ms: u64,
    /// Aggregate deadline counts.
    pub summary: DeadlineSummary,
    /// Deadline signals.
    pub deadlines: Vec<DeadlineSignal>,
}

/// Aggregate deadline counts.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct DeadlineSummary {
    /// Total deadlines included.
    pub deadline_count: usize,
    /// Overdue deadlines.
    pub overdue_count: usize,
    /// Deadlines due within the configured horizon.
    pub due_soon_count: usize,
    /// Scheduled deadlines outside the configured horizon.
    pub scheduled_count: usize,
}

/// A single deadline signal.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct DeadlineSignal {
    /// Stable signal identifier.
    pub id: String,
    /// Intention carrying the deadline.
    pub intention_id: String,
    /// Intention description.
    pub description: String,
    /// Intention priority.
    pub intention_priority: IntentionPriority,
    /// Intention lifecycle status.
    pub status: IntentionStatus,
    /// Deadline timestamp in milliseconds since the Unix epoch.
    pub deadline_at_ms: u64,
    /// Deadline state at report generation.
    pub state: DeadlineState,
    /// Proactive priority assigned to the signal.
    pub priority: ProactivePriority,
    /// Human-readable detail.
    pub detail: String,
    /// Event or memory identifiers supporting the signal.
    pub evidence_ids: Vec<String>,
}

/// Deadline state at report generation.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeadlineState {
    /// Deadline is before the report timestamp.
    Overdue,
    /// Deadline is within the configured horizon.
    DueSoon,
    /// Deadline is outside the configured horizon.
    Scheduled,
}

/// Non-mutating anomaly report.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AnomalyReport {
    /// Report format version.
    pub version: u32,
    /// Timestamp in milliseconds when the report was generated.
    pub generated_at_ms: u64,
    /// Aggregate anomaly counts.
    pub summary: AnomalySummary,
    /// Number of alerts included.
    pub alert_count: usize,
    /// Actionable alerts.
    pub alerts: Vec<AnomalyAlert>,
}

/// Aggregate anomaly counts.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AnomalySummary {
    /// Critical alerts.
    pub critical_count: usize,
    /// High-priority alerts.
    pub high_count: usize,
    /// Medium-priority alerts.
    pub medium_count: usize,
    /// Low-priority alerts.
    pub low_count: usize,
}

/// A single proactive anomaly alert.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AnomalyAlert {
    /// Stable alert identifier.
    pub id: String,
    /// Alert category.
    pub kind: AnomalyKind,
    /// Proactive priority.
    pub priority: ProactivePriority,
    /// Short alert title.
    pub title: String,
    /// Specific non-mutating detail.
    pub detail: String,
    /// Event or memory identifiers supporting the alert.
    pub evidence_ids: Vec<String>,
    /// Source signal or issue identifier.
    pub source_id: String,
    /// Review identifier, when the alert maps to an operator-review item.
    pub review_id: Option<String>,
    /// Suggested explicit operator action.
    pub suggested_action: String,
}

/// Proactive anomaly category.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnomalyKind {
    /// No observed episodes exist.
    NoEpisodes,
    /// Contradictory memory exists.
    Contradiction,
    /// Memory is detached from source evidence.
    UnsupportedMemory,
    /// Memory confidence is low.
    LowConfidenceMemory,
    /// Memory is stale.
    StaleMemory,
    /// Memory was retired by a newer, evidence-backed value.
    SupersededMemory,
    /// Entity is disconnected from relations.
    IsolatedEntity,
    /// Deadline is overdue.
    OverdueDeadline,
    /// Intention is waiting on an incomplete dependency.
    WaitingOnDependency,
    /// Intention references a missing dependency.
    MissingDependency,
    /// Intention is blocked.
    BlockedIntention,
    /// Intention is deferred.
    DeferredIntention,
    /// Active intention is stale.
    StaleIntention,
    /// Goal appears ready for completion review.
    GoalReadyForReview,
    /// Evidence capture is needed.
    CaptureOpportunity,
}

/// Proactive priority.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ProactivePriority {
    /// Immediate attention recommended.
    Critical,
    /// High-priority attention.
    High,
    /// Medium-priority attention.
    Medium,
    /// Low-priority attention.
    Low,
}

/// A proactive evidence-capture opportunity.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct CaptureOpportunity {
    /// Stable opportunity identifier.
    pub id: String,
    /// Related operator review item.
    pub review_id: String,
    /// Opportunity priority.
    pub priority: ProactivePriority,
    /// Short title.
    pub title: String,
    /// Specific detail.
    pub detail: String,
    /// Event or memory identifiers that need evidence.
    pub evidence_ids: Vec<String>,
    /// Suggested explicit operator action.
    pub suggested_action: String,
}

/// Options for acknowledging an anomaly alert.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AnomalyAcknowledgementOptions {
    /// Alert identifier to acknowledge.
    pub anomaly_id: String,
    /// Operator-supplied acknowledgement note.
    pub note: String,
    /// Whether to preview the acknowledgement without writing a record.
    pub dry_run: bool,
}

/// Result of planning or applying an anomaly acknowledgement.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AnomalyAcknowledgementReport {
    /// Report format version.
    pub version: u32,
    /// Whether this report was generated without writing a record.
    pub dry_run: bool,
    /// Whether an append-only review decision was written.
    pub applied: bool,
    /// Alert identifier that was handled.
    pub anomaly_id: String,
    /// Operator-supplied acknowledgement note.
    pub note: String,
    /// Alert that was acknowledged.
    pub alert: AnomalyAlert,
    /// Event or memory identifiers covered by this acknowledgement.
    pub evidence_ids: Vec<String>,
    /// Review decision identifier, when known.
    pub decision_id: Option<String>,
    /// Event identifier appended to the record ledger, if applied.
    pub event_id: Option<String>,
    /// Non-mutating policy statement included for agent clients.
    pub policy: String,
}

pub(crate) struct PreparedAnomalyAcknowledgement {
    pub report: AnomalyAcknowledgementReport,
    pub event: ReviewRecorded,
}

pub(crate) fn proactive_report(
    data: &MemoryData,
    options: ProactiveOptions,
) -> MemoryProactiveReport {
    let deadlines = deadline_report(data, options.clone());
    let anomalies = anomaly_report(data, options.clone());
    let review = operator_review_at(
        data,
        &options,
        OperatorReviewOptions {
            limit: options.review_limit,
            min_priority: Some(SelfInspectionReviewPriority::High),
            ..OperatorReviewOptions::default()
        },
    );
    let capture_opportunities = capture_opportunities(data, options.clone());
    let authority = AuthorityDecision::evaluate(&KnowledgeHealth::inspect_at(data, options.now_ms));
    let summary = ProactiveSummary {
        deadline_count: deadlines.summary.deadline_count,
        overdue_deadline_count: deadlines.summary.overdue_count,
        due_soon_deadline_count: deadlines.summary.due_soon_count,
        anomaly_count: anomalies.alert_count,
        critical_anomaly_count: anomalies.summary.critical_count,
        high_anomaly_count: anomalies.summary.high_count,
        capture_opportunity_count: capture_opportunities.len(),
        high_risk_review_count: review.displayed_items,
        should_pause: anomalies.summary.critical_count > 0
            || anomalies.summary.high_count > 0
            || deadlines.summary.overdue_count > 0
            || review.summary.critical_count > 0
            || review.summary.high_count > 0,
    };

    MemoryProactiveReport {
        version: MEMORY_PROACTIVE_REPORT_VERSION,
        generated_at_ms: options.now_ms,
        event_count: data.event_count,
        authority,
        summary,
        deadlines,
        anomalies,
        capture_opportunities,
        high_risk_review_items: review.items,
        write_back_policy: SelfInspectionWriteBackPolicy {
            automatic_write_back: false,
            requires_operator_review: true,
            message:
                "proactive reports are non-mutating; acknowledgement and write-back require explicit operator commands"
                    .to_string(),
        },
    }
}

pub(crate) fn deadline_report(data: &MemoryData, options: ProactiveOptions) -> DeadlineReport {
    let mut deadlines = data
        .intentions
        .iter()
        .filter(|intention| !matches!(intention.status, IntentionStatus::Completed))
        .filter_map(|intention| deadline_signal(intention, &options))
        .collect::<Vec<_>>();
    deadlines.sort_by(|left, right| {
        priority_rank(&left.priority)
            .cmp(&priority_rank(&right.priority))
            .then_with(|| left.deadline_at_ms.cmp(&right.deadline_at_ms))
            .then_with(|| left.intention_id.cmp(&right.intention_id))
    });

    let summary = DeadlineSummary {
        deadline_count: deadlines.len(),
        overdue_count: deadlines
            .iter()
            .filter(|deadline| deadline.state == DeadlineState::Overdue)
            .count(),
        due_soon_count: deadlines
            .iter()
            .filter(|deadline| deadline.state == DeadlineState::DueSoon)
            .count(),
        scheduled_count: deadlines
            .iter()
            .filter(|deadline| deadline.state == DeadlineState::Scheduled)
            .count(),
    };

    DeadlineReport {
        version: MEMORY_DEADLINE_REPORT_VERSION,
        generated_at_ms: options.now_ms,
        now_ms: options.now_ms,
        horizon_ms: options.deadline_horizon_ms,
        summary,
        deadlines,
    }
}

pub(crate) fn anomaly_report(data: &MemoryData, options: ProactiveOptions) -> AnomalyReport {
    let mut alerts = Vec::new();
    append_health_alerts(data, &options, &mut alerts);
    append_deadline_alerts(data, &options, &mut alerts);
    append_reconciliation_alerts(data, &options, &mut alerts);
    append_capture_alerts(data, &options, &mut alerts);

    alerts.retain(|alert| !alert_acknowledged(data, alert));
    alerts.sort_by(|left, right| {
        priority_rank(&left.priority)
            .cmp(&priority_rank(&right.priority))
            .then_with(|| left.kind_name().cmp(right.kind_name()))
            .then_with(|| left.id.cmp(&right.id))
    });
    alerts.dedup_by(|left, right| left.id == right.id);

    let summary = AnomalySummary {
        critical_count: count_priority(&alerts, ProactivePriority::Critical),
        high_count: count_priority(&alerts, ProactivePriority::High),
        medium_count: count_priority(&alerts, ProactivePriority::Medium),
        low_count: count_priority(&alerts, ProactivePriority::Low),
    };

    AnomalyReport {
        version: MEMORY_ANOMALY_REPORT_VERSION,
        generated_at_ms: options.now_ms,
        alert_count: alerts.len(),
        summary,
        alerts,
    }
}

pub(crate) fn prepare_anomaly_acknowledgement(
    data: &MemoryData,
    options: AnomalyAcknowledgementOptions,
    decision_id: String,
) -> Result<PreparedAnomalyAcknowledgement> {
    let anomaly_id = options.anomaly_id.trim().to_string();
    if anomaly_id.is_empty() {
        return Err(NahualiError::EmptyContent);
    }

    let note = options.note.trim().to_string();
    if note.is_empty() {
        return Err(NahualiError::EmptyContent);
    }

    let alert = anomaly_report(data, ProactiveOptions::default())
        .alerts
        .into_iter()
        .find(|alert| alert.id == anomaly_id)
        .ok_or_else(|| NahualiError::UnknownReviewItem {
            id: anomaly_id.clone(),
        })?;

    let event = ReviewRecorded {
        id: decision_id.clone(),
        review_id: alert.id.clone(),
        finding_id: alert.source_id.clone(),
        action: review_action(&alert.kind),
        outcome: ReviewRecordedOutcome::Resolved,
        note: note.clone(),
        evidence_ids: alert.evidence_ids.clone(),
        scope: None,
    };
    let report = AnomalyAcknowledgementReport {
        version: MEMORY_ANOMALY_ACKNOWLEDGEMENT_VERSION,
        dry_run: options.dry_run,
        applied: false,
        anomaly_id,
        note,
        evidence_ids: alert.evidence_ids.clone(),
        alert,
        decision_id: Some(decision_id),
        event_id: None,
        policy:
            "anomaly acknowledgements require explicit operator approval and are stored as append-only review decisions"
                .to_string(),
    };

    Ok(PreparedAnomalyAcknowledgement { report, event })
}

fn deadline_signal(intention: &Intention, options: &ProactiveOptions) -> Option<DeadlineSignal> {
    let deadline_at_ms = intention.deadline_at_ms?;
    let state = if deadline_at_ms < options.now_ms {
        DeadlineState::Overdue
    } else if deadline_at_ms <= options.now_ms.saturating_add(options.deadline_horizon_ms) {
        DeadlineState::DueSoon
    } else {
        DeadlineState::Scheduled
    };
    let priority = deadline_priority(&state, &intention.priority);
    let detail = match state {
        DeadlineState::Overdue => {
            format!(
                "Deadline {deadline_at_ms} is before proactive report time {}.",
                options.now_ms
            )
        }
        DeadlineState::DueSoon => {
            format!(
                "Deadline {deadline_at_ms} is within the next {} ms.",
                options.deadline_horizon_ms
            )
        }
        DeadlineState::Scheduled => format!("Deadline {deadline_at_ms} is scheduled."),
    };

    Some(DeadlineSignal {
        id: format!("deadline_{}_{}", intention.id, state_key(&state)),
        intention_id: intention.id.clone(),
        description: intention.description.clone(),
        intention_priority: intention.priority.clone(),
        status: intention.status.clone(),
        deadline_at_ms,
        state,
        priority,
        detail,
        evidence_ids: vec![intention.updated_event_id.clone()],
    })
}

fn append_health_alerts(
    data: &MemoryData,
    options: &ProactiveOptions,
    alerts: &mut Vec<AnomalyAlert>,
) {
    for signal in KnowledgeHealth::inspect_at(data, options.now_ms).signals {
        let (kind, title, suggested_action) = match signal.kind {
            HealthSignalKind::NoEpisodes => (
                AnomalyKind::NoEpisodes,
                "No observed episodes",
                "Record source episodes before deriving or trusting memory.",
            ),
            HealthSignalKind::UnsupportedFact => (
                AnomalyKind::UnsupportedMemory,
                "Memory lacks source evidence",
                "Record or attach source evidence before relying on this memory.",
            ),
            HealthSignalKind::LowConfidenceFact => (
                AnomalyKind::LowConfidenceMemory,
                "Low-confidence memory",
                "Verify the memory with evidence before increasing trust.",
            ),
            HealthSignalKind::ConflictingFact => (
                AnomalyKind::Contradiction,
                "Contradictory memory",
                "Review conflicting evidence and record an explicit resolution.",
            ),
            HealthSignalKind::StaleFact => (
                AnomalyKind::StaleMemory,
                "Stale memory",
                "Record a fresh observation before relying on this memory.",
            ),
            HealthSignalKind::SupersededFact => (
                AnomalyKind::SupersededMemory,
                "Superseded memory",
                "A newer evidence-backed value replaced this one; retire or archive the older value.",
            ),
            HealthSignalKind::IsolatedEntity => (
                AnomalyKind::IsolatedEntity,
                "Disconnected entity",
                "Add evidence-backed links or record more context.",
            ),
        };
        let source_id = alert_source_id(kind_key(&kind), &signal.evidence_ids, &signal.message);
        alerts.push(AnomalyAlert {
            id: format!("anomaly_{source_id}"),
            kind,
            priority: priority_from_health(&signal.severity),
            title: title.to_string(),
            detail: signal.message,
            evidence_ids: signal.evidence_ids,
            source_id,
            review_id: None,
            suggested_action: suggested_action.to_string(),
        });
    }
}

fn append_deadline_alerts(
    data: &MemoryData,
    options: &ProactiveOptions,
    alerts: &mut Vec<AnomalyAlert>,
) {
    for deadline in deadline_report(data, options.clone()).deadlines {
        if deadline.state != DeadlineState::Overdue {
            continue;
        }
        alerts.push(AnomalyAlert {
            id: format!("anomaly_{}", deadline.id),
            kind: AnomalyKind::OverdueDeadline,
            priority: deadline.priority,
            title: "Overdue deadline".to_string(),
            detail: deadline.detail,
            evidence_ids: deadline.evidence_ids,
            source_id: deadline.id,
            review_id: None,
            suggested_action:
                "Review the intention, update the deadline, block/defer it, or complete it explicitly."
                    .to_string(),
        });
    }
}

fn append_reconciliation_alerts(
    data: &MemoryData,
    options: &ProactiveOptions,
    alerts: &mut Vec<AnomalyAlert>,
) {
    let report = intention::reconcile_intentions(
        data,
        IntentionReconciliationOptions {
            now_ms: options.now_ms,
            stale_after_ms: options.stale_after_ms,
        },
    );
    for issue in report.issues {
        if issue.kind == IntentionReconciliationIssueKind::Overdue {
            continue;
        }
        let kind = anomaly_kind_from_reconciliation(&issue.kind);
        alerts.push(AnomalyAlert {
            id: format!("anomaly_{}", issue.id),
            kind,
            priority: priority_from_reconciliation(&issue.priority),
            title: issue.title,
            detail: issue.detail,
            evidence_ids: issue.evidence_ids,
            source_id: issue.id,
            review_id: None,
            suggested_action: "Review and update the affected intention explicitly.".to_string(),
        });
    }
}

fn append_capture_alerts(
    data: &MemoryData,
    options: &ProactiveOptions,
    alerts: &mut Vec<AnomalyAlert>,
) {
    for opportunity in capture_opportunities(data, options.clone()) {
        alerts.push(AnomalyAlert {
            id: format!("anomaly_{}", opportunity.id),
            kind: AnomalyKind::CaptureOpportunity,
            priority: opportunity.priority,
            title: opportunity.title,
            detail: opportunity.detail,
            evidence_ids: opportunity.evidence_ids,
            source_id: opportunity.id,
            review_id: Some(opportunity.review_id),
            suggested_action: opportunity.suggested_action,
        });
    }
}

fn capture_opportunities(data: &MemoryData, options: ProactiveOptions) -> Vec<CaptureOpportunity> {
    let mut opportunities = operator_review_at(
        data,
        &options,
        OperatorReviewOptions {
            limit: usize::MAX,
            action: Some(SelfInspectionReviewAction::CaptureEvidence),
            ..OperatorReviewOptions::default()
        },
    )
    .items
    .into_iter()
    .map(|item| CaptureOpportunity {
        id: format!("capture_{}", item.id),
        review_id: item.id,
        priority: priority_from_review(&item.priority),
        title: item.title,
        detail: item.detail,
        evidence_ids: item.evidence_ids,
        suggested_action: item.operator_guidance,
    })
    .filter(|opportunity| !capture_acknowledged(data, opportunity))
    .collect::<Vec<_>>();

    opportunities.sort_by(|left, right| {
        priority_rank(&left.priority)
            .cmp(&priority_rank(&right.priority))
            .then_with(|| left.id.cmp(&right.id))
    });
    opportunities.truncate(options.review_limit.max(1));
    opportunities
}

fn operator_review_at(
    data: &MemoryData,
    options: &ProactiveOptions,
    review_options: OperatorReviewOptions,
) -> crate::OperatorReviewReport {
    operator_review::operator_review_from_self_inspection(
        self_inspection::self_inspect_at(data, options.now_ms),
        review_options,
    )
}

fn alert_acknowledged(data: &MemoryData, alert: &AnomalyAlert) -> bool {
    data.review_decisions
        .iter()
        .filter(|decision| decision.outcome == ReviewDecisionOutcome::Resolved)
        .any(|decision| {
            decision.review_id == alert.id
                || decision.finding_id == alert.id
                || decision.finding_id == alert.source_id
                || alert
                    .review_id
                    .as_deref()
                    .map(|review_id| decision.review_id == review_id)
                    .unwrap_or(false)
                || evidence_covered(&alert.evidence_ids, &decision.evidence_ids)
        })
}

fn capture_acknowledged(data: &MemoryData, opportunity: &CaptureOpportunity) -> bool {
    data.review_decisions
        .iter()
        .filter(|decision| decision.outcome == ReviewDecisionOutcome::Resolved)
        .any(|decision| {
            decision.review_id == opportunity.review_id
                || decision.finding_id == opportunity.id
                || evidence_covered(&opportunity.evidence_ids, &decision.evidence_ids)
        })
}

fn evidence_covered(alert_evidence: &[String], decision_evidence: &[String]) -> bool {
    !alert_evidence.is_empty()
        && alert_evidence
            .iter()
            .all(|evidence_id| decision_evidence.iter().any(|id| id == evidence_id))
}

fn deadline_priority(state: &DeadlineState, priority: &IntentionPriority) -> ProactivePriority {
    match (state, priority) {
        (DeadlineState::Overdue, IntentionPriority::Critical) => ProactivePriority::Critical,
        (DeadlineState::Overdue, IntentionPriority::High) => ProactivePriority::High,
        (DeadlineState::Overdue, _) => ProactivePriority::Medium,
        (DeadlineState::DueSoon, IntentionPriority::Critical | IntentionPriority::High) => {
            ProactivePriority::High
        }
        (DeadlineState::DueSoon, _) => ProactivePriority::Medium,
        (DeadlineState::Scheduled, _) => ProactivePriority::Low,
    }
}

fn priority_from_health(severity: &HealthSeverity) -> ProactivePriority {
    match severity {
        HealthSeverity::High => ProactivePriority::Critical,
        HealthSeverity::Medium => ProactivePriority::High,
        HealthSeverity::Low => ProactivePriority::Low,
    }
}

fn priority_from_reconciliation(priority: &IntentionReconciliationPriority) -> ProactivePriority {
    match priority {
        IntentionReconciliationPriority::Critical => ProactivePriority::Critical,
        IntentionReconciliationPriority::High => ProactivePriority::High,
        IntentionReconciliationPriority::Medium => ProactivePriority::Medium,
        IntentionReconciliationPriority::Low => ProactivePriority::Low,
    }
}

fn priority_from_review(priority: &SelfInspectionReviewPriority) -> ProactivePriority {
    match priority {
        SelfInspectionReviewPriority::Critical => ProactivePriority::Critical,
        SelfInspectionReviewPriority::High => ProactivePriority::High,
        SelfInspectionReviewPriority::Medium => ProactivePriority::Medium,
        SelfInspectionReviewPriority::Low => ProactivePriority::Low,
    }
}

fn priority_rank(priority: &ProactivePriority) -> u8 {
    match priority {
        ProactivePriority::Critical => 0,
        ProactivePriority::High => 1,
        ProactivePriority::Medium => 2,
        ProactivePriority::Low => 3,
    }
}

fn count_priority(alerts: &[AnomalyAlert], priority: ProactivePriority) -> usize {
    alerts
        .iter()
        .filter(|alert| alert.priority == priority)
        .count()
}

fn anomaly_kind_from_reconciliation(kind: &IntentionReconciliationIssueKind) -> AnomalyKind {
    match kind {
        IntentionReconciliationIssueKind::Overdue => AnomalyKind::OverdueDeadline,
        IntentionReconciliationIssueKind::WaitingOnDependency => AnomalyKind::WaitingOnDependency,
        IntentionReconciliationIssueKind::MissingDependency => AnomalyKind::MissingDependency,
        IntentionReconciliationIssueKind::Blocked => AnomalyKind::BlockedIntention,
        IntentionReconciliationIssueKind::Deferred => AnomalyKind::DeferredIntention,
        IntentionReconciliationIssueKind::Stale => AnomalyKind::StaleIntention,
        IntentionReconciliationIssueKind::GoalReadyForReview => AnomalyKind::GoalReadyForReview,
    }
}

fn review_action(kind: &AnomalyKind) -> ReviewRecordedAction {
    match kind {
        AnomalyKind::Contradiction => ReviewRecordedAction::ResolveContradiction,
        AnomalyKind::StaleMemory | AnomalyKind::SupersededMemory => {
            ReviewRecordedAction::RefreshMemory
        }
        AnomalyKind::IsolatedEntity => ReviewRecordedAction::LinkMemory,
        AnomalyKind::NoEpisodes
        | AnomalyKind::UnsupportedMemory
        | AnomalyKind::LowConfidenceMemory
        | AnomalyKind::CaptureOpportunity => ReviewRecordedAction::CaptureEvidence,
        AnomalyKind::OverdueDeadline
        | AnomalyKind::WaitingOnDependency
        | AnomalyKind::MissingDependency
        | AnomalyKind::BlockedIntention
        | AnomalyKind::DeferredIntention
        | AnomalyKind::StaleIntention
        | AnomalyKind::GoalReadyForReview => ReviewRecordedAction::ReviewIntention,
    }
}

fn kind_key(kind: &AnomalyKind) -> &'static str {
    match kind {
        AnomalyKind::NoEpisodes => "no_episodes",
        AnomalyKind::Contradiction => "contradiction",
        AnomalyKind::UnsupportedMemory => "unsupported_memory",
        AnomalyKind::LowConfidenceMemory => "low_confidence_memory",
        AnomalyKind::StaleMemory => "stale_memory",
        AnomalyKind::SupersededMemory => "superseded_memory",
        AnomalyKind::IsolatedEntity => "isolated_entity",
        AnomalyKind::OverdueDeadline => "overdue_deadline",
        AnomalyKind::WaitingOnDependency => "waiting_on_dependency",
        AnomalyKind::MissingDependency => "missing_dependency",
        AnomalyKind::BlockedIntention => "blocked_intention",
        AnomalyKind::DeferredIntention => "deferred_intention",
        AnomalyKind::StaleIntention => "stale_intention",
        AnomalyKind::GoalReadyForReview => "goal_ready_for_review",
        AnomalyKind::CaptureOpportunity => "capture_opportunity",
    }
}

impl AnomalyAlert {
    fn kind_name(&self) -> &'static str {
        kind_key(&self.kind)
    }
}

fn state_key(state: &DeadlineState) -> &'static str {
    match state {
        DeadlineState::Overdue => "overdue",
        DeadlineState::DueSoon => "due_soon",
        DeadlineState::Scheduled => "scheduled",
    }
}

fn alert_source_id(kind: &str, evidence_ids: &[String], detail: &str) -> String {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(kind.as_bytes());
    bytes.push(0);
    if evidence_ids.is_empty() {
        bytes.extend_from_slice(detail.as_bytes());
    } else {
        for evidence_id in evidence_ids {
            bytes.extend_from_slice(evidence_id.as_bytes());
            bytes.push(0);
        }
    }
    format!("{kind}_{:08x}", fnv1a32(&bytes))
}

fn fnv1a32(bytes: &[u8]) -> u32 {
    const FNV_OFFSET: u32 = 0x811c9dc5;
    const FNV_PRIME: u32 = 0x01000193;

    bytes.iter().fold(FNV_OFFSET, |hash, byte| {
        (hash ^ u32::from(*byte)).wrapping_mul(FNV_PRIME)
    })
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
        Claim, Intention, IntentionKind, IntentionPriority, IntentionStatus, MemoryData,
        proactive::{
            AnomalyKind, DeadlineState, ProactiveOptions, anomaly_report, deadline_report,
            prepare_anomaly_acknowledgement, proactive_report,
        },
    };

    #[test]
    fn reports_deadlines_anomalies_and_capture_opportunities() {
        let data = MemoryData {
            event_count: 3,
            intentions: vec![Intention {
                id: "intention_1".to_string(),
                event_id: "event_1".to_string(),
                updated_event_id: "event_1".to_string(),
                kind: IntentionKind::Task,
                status: IntentionStatus::Active,
                priority: IntentionPriority::High,
                description: "Ship release notes".to_string(),
                source_episode_id: None,
                status_reason: None,
                deadline_at_ms: Some(50),
                depends_on: vec!["missing_dependency".to_string()],
                goal_id: None,
                progress_percent: None,
                scope: None,
                created_at_ms: 0,
                updated_at_ms: 0,
            }],
            claims: vec![Claim {
                id: "claim_1".to_string(),
                event_id: "event_2".to_string(),
                subject: "Lena".to_string(),
                predicate: "owns".to_string(),
                object: "release notes".to_string(),
                source_episode_id: None,
                confidence: 0.9,
                scope: None,
                created_at_ms: 10,
            }],
            ..MemoryData::default()
        };

        let report = proactive_report(
            &data,
            ProactiveOptions {
                now_ms: 100,
                deadline_horizon_ms: 20,
                stale_after_ms: 0,
                review_limit: 20,
            },
        );

        assert_eq!(report.deadlines.summary.overdue_count, 1);
        assert_eq!(report.deadlines.deadlines[0].state, DeadlineState::Overdue);
        assert!(report.anomalies.alerts.iter().any(|alert| {
            alert.kind == AnomalyKind::OverdueDeadline
                && alert.evidence_ids == vec!["event_1".to_string()]
        }));
        assert!(report.anomalies.alerts.iter().any(|alert| {
            alert.kind == AnomalyKind::MissingDependency
                && alert.evidence_ids == vec!["event_1".to_string()]
        }));
        assert!(report.anomalies.alerts.iter().any(|alert| {
            alert.kind == AnomalyKind::UnsupportedMemory
                && alert.evidence_ids == vec!["event_2".to_string()]
        }));
        assert!(!report.capture_opportunities.is_empty());
        assert!(report.summary.should_pause);
    }

    #[test]
    fn reports_due_soon_deadlines_without_overdue_anomaly() {
        let data = MemoryData {
            intentions: vec![Intention {
                id: "intention_1".to_string(),
                event_id: "event_1".to_string(),
                updated_event_id: "event_1".to_string(),
                kind: IntentionKind::Task,
                status: IntentionStatus::Active,
                priority: IntentionPriority::Medium,
                description: "Prepare launch".to_string(),
                source_episode_id: Some("episode_1".to_string()),
                status_reason: None,
                deadline_at_ms: Some(150),
                depends_on: Vec::new(),
                goal_id: None,
                progress_percent: None,
                scope: None,
                created_at_ms: 0,
                updated_at_ms: 0,
            }],
            ..MemoryData::default()
        };

        let deadlines = deadline_report(
            &data,
            ProactiveOptions {
                now_ms: 100,
                deadline_horizon_ms: 100,
                stale_after_ms: 0,
                review_limit: 20,
            },
        );
        let anomalies = anomaly_report(
            &data,
            ProactiveOptions {
                now_ms: 100,
                deadline_horizon_ms: 100,
                stale_after_ms: 0,
                review_limit: 20,
            },
        );

        assert_eq!(deadlines.summary.due_soon_count, 1);
        assert_eq!(deadlines.deadlines[0].state, DeadlineState::DueSoon);
        assert!(
            !anomalies
                .alerts
                .iter()
                .any(|alert| alert.kind == AnomalyKind::OverdueDeadline)
        );
    }

    #[test]
    fn acknowledgement_prepares_review_decision_for_alert() {
        let data = MemoryData {
            intentions: vec![Intention {
                id: "intention_1".to_string(),
                event_id: "event_1".to_string(),
                updated_event_id: "event_1".to_string(),
                kind: IntentionKind::Task,
                status: IntentionStatus::Active,
                priority: IntentionPriority::High,
                description: "Ship release notes".to_string(),
                source_episode_id: None,
                status_reason: None,
                deadline_at_ms: Some(50),
                depends_on: Vec::new(),
                goal_id: None,
                progress_percent: None,
                scope: None,
                created_at_ms: 0,
                updated_at_ms: 0,
            }],
            ..MemoryData::default()
        };
        let alert_id = anomaly_report(
            &data,
            ProactiveOptions {
                now_ms: 100,
                deadline_horizon_ms: 0,
                stale_after_ms: 0,
                review_limit: 20,
            },
        )
        .alerts
        .iter()
        .find(|alert| alert.kind == AnomalyKind::OverdueDeadline)
        .expect("overdue alert exists")
        .id
        .clone();

        let prepared = prepare_anomaly_acknowledgement(
            &data,
            crate::AnomalyAcknowledgementOptions {
                anomaly_id: alert_id.clone(),
                note: "Reviewed".to_string(),
                dry_run: true,
            },
            "review_decision_1".to_string(),
        )
        .expect("acknowledgement prepares");

        assert_eq!(prepared.report.anomaly_id, alert_id);
        assert_eq!(prepared.event.review_id, prepared.report.alert.id);
        assert_eq!(prepared.event.evidence_ids, vec!["event_1".to_string()]);
    }
}
