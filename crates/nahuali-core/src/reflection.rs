use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    AuthorityDecision, HealthDimension, HealthSeverity, KnowledgeHealth, MemoryData,
    SelfInspectionFinding, SelfInspectionFindingKind, SelfInspectionReviewAction,
    SelfInspectionReviewPriority, SelfInspectionWriteBackPolicy, self_inspection,
};

/// Current reflection-cycle report format version.
pub const MEMORY_REFLECTION_VERSION: u32 = 1;

/// Options for building a non-mutating reflection cycle.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ReflectionOptions {
    /// Maximum grouped cycles returned.
    pub cycle_limit: usize,
    /// Maximum evidence identifiers returned per grouped cycle.
    pub evidence_limit: usize,
}

impl Default for ReflectionOptions {
    fn default() -> Self {
        Self {
            cycle_limit: 8,
            evidence_limit: 8,
        }
    }
}

/// Non-mutating reflection report for operator-approved consolidation planning.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct MemoryReflectionReport {
    /// Report format version.
    pub version: u32,
    /// Timestamp in milliseconds when the report was generated.
    pub generated_at_ms: u64,
    /// Number of source events represented by the projection.
    pub event_count: usize,
    /// Projection-level authority decision.
    pub authority: AuthorityDecision,
    /// Knowledge-health report used to derive reflection work.
    pub health: KnowledgeHealth,
    /// Aggregate counts for the reflection pass.
    pub summary: ReflectionSummary,
    /// Source and evidence coverage across the projection.
    pub source_coverage: ReflectionSourceCoverage,
    /// Grouped operator cycles derived from self-inspection findings.
    pub cycles: Vec<ReflectionCycle>,
    /// Explicit policy for automatic write-back.
    pub write_back_policy: SelfInspectionWriteBackPolicy,
}

/// Aggregate counts for a reflection report.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ReflectionSummary {
    /// Number of self-inspection findings considered.
    pub finding_count: usize,
    /// Number of grouped cycles before display limiting.
    pub total_cycle_count: usize,
    /// Number of grouped cycles returned.
    pub displayed_cycle_count: usize,
    /// Number of critical cycles before display limiting.
    pub critical_cycle_count: usize,
    /// Number of high cycles before display limiting.
    pub high_cycle_count: usize,
    /// Number of medium cycles before display limiting.
    pub medium_cycle_count: usize,
    /// Number of low cycles before display limiting.
    pub low_cycle_count: usize,
    /// Number of unique evidence identifiers across returned cycles.
    pub evidence_id_count: usize,
}

/// Source and evidence coverage across projected memory.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ReflectionSourceCoverage {
    /// Number of projected source records.
    pub source_count: usize,
    /// Number of projected episodes.
    pub episode_count: usize,
    /// Number of episodes linked to a source record.
    pub sourced_episode_count: usize,
    /// Number of episodes without a source record.
    pub unsourced_episode_count: usize,
    /// Canonical derived memory item count.
    pub derived_memory_count: usize,
    /// Derived memory items that cite a source episode.
    pub evidence_backed_memory_count: usize,
    /// Derived memory items without a source episode.
    pub unsupported_memory_count: usize,
    /// Ratio of episodes linked to source records, rounded to two decimals.
    pub source_coverage_ratio: f32,
    /// Ratio of derived memory that cites source episodes, rounded to two decimals.
    pub evidence_coverage_ratio: f32,
}

/// Grouped reflection work for one operator-approved cycle.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ReflectionCycle {
    /// Stable cycle identifier within this report shape.
    pub id: String,
    /// Highest priority represented by the grouped findings.
    pub priority: SelfInspectionReviewPriority,
    /// Operator action this cycle plans.
    pub action: SelfInspectionReviewAction,
    /// Human-readable cycle title.
    pub title: String,
    /// Why this cycle matters.
    pub rationale: String,
    /// Number of findings grouped into this cycle.
    pub finding_count: usize,
    /// Event or memory identifiers supporting this cycle.
    pub evidence_ids: Vec<String>,
    /// Findings grouped into this cycle.
    pub findings: Vec<ReflectionFinding>,
}

/// Finding included in a grouped reflection cycle.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ReflectionFinding {
    /// Stable finding identifier.
    pub id: String,
    /// Finding category.
    pub kind: SelfInspectionFindingKind,
    /// Finding severity.
    pub severity: HealthSeverity,
    /// Short human-readable title.
    pub title: String,
    /// Detailed explanation.
    pub detail: String,
    /// Health dimensions affected by this finding.
    pub dimensions: Vec<HealthDimension>,
    /// Event or memory identifiers supporting the finding.
    pub evidence_ids: Vec<String>,
    /// Suggested human or agent action.
    pub suggested_action: String,
}

pub(crate) fn reflect(data: &MemoryData, options: ReflectionOptions) -> MemoryReflectionReport {
    let cycle_limit = options.cycle_limit.max(1);
    let evidence_limit = options.evidence_limit.max(1);
    let self_inspection = self_inspection::self_inspect(data);
    let mut cycles = grouped_cycles(&self_inspection.findings, evidence_limit);
    let total_cycle_count = cycles.len();
    let critical_cycle_count = cycles
        .iter()
        .filter(|cycle| cycle.priority == SelfInspectionReviewPriority::Critical)
        .count();
    let high_cycle_count = cycles
        .iter()
        .filter(|cycle| cycle.priority == SelfInspectionReviewPriority::High)
        .count();
    let medium_cycle_count = cycles
        .iter()
        .filter(|cycle| cycle.priority == SelfInspectionReviewPriority::Medium)
        .count();
    let low_cycle_count = cycles
        .iter()
        .filter(|cycle| cycle.priority == SelfInspectionReviewPriority::Low)
        .count();

    cycles.truncate(cycle_limit);
    let evidence_id_count = cycles
        .iter()
        .flat_map(|cycle| cycle.evidence_ids.iter().cloned())
        .collect::<BTreeSet<_>>()
        .len();
    let summary = ReflectionSummary {
        finding_count: self_inspection.findings.len(),
        total_cycle_count,
        displayed_cycle_count: cycles.len(),
        critical_cycle_count,
        high_cycle_count,
        medium_cycle_count,
        low_cycle_count,
        evidence_id_count,
    };

    MemoryReflectionReport {
        version: MEMORY_REFLECTION_VERSION,
        generated_at_ms: self_inspection.generated_at_ms,
        event_count: data.event_count,
        authority: self_inspection.authority,
        health: self_inspection.health,
        summary,
        source_coverage: source_coverage(data),
        cycles,
        write_back_policy: self_inspection.write_back_policy,
    }
}

fn grouped_cycles(
    findings: &[SelfInspectionFinding],
    evidence_limit: usize,
) -> Vec<ReflectionCycle> {
    let mut grouped =
        BTreeMap::<&'static str, (SelfInspectionReviewAction, Vec<&SelfInspectionFinding>)>::new();
    for finding in findings {
        let action = action_for_kind(&finding.kind);
        grouped
            .entry(action_slug(&action))
            .or_insert_with(|| (action, Vec::new()))
            .1
            .push(finding);
    }

    let mut cycles = grouped
        .into_iter()
        .map(|(_, (action, findings))| cycle_for_action(action, findings, evidence_limit))
        .collect::<Vec<_>>();
    cycles.sort_by(|left, right| {
        priority_rank(&left.priority)
            .cmp(&priority_rank(&right.priority))
            .then_with(|| right.finding_count.cmp(&left.finding_count))
            .then_with(|| action_slug(&left.action).cmp(action_slug(&right.action)))
    });
    cycles
}

fn cycle_for_action(
    action: SelfInspectionReviewAction,
    findings: Vec<&SelfInspectionFinding>,
    evidence_limit: usize,
) -> ReflectionCycle {
    let priority = findings
        .iter()
        .map(|finding| priority_for_finding(finding))
        .min_by_key(priority_rank)
        .unwrap_or(SelfInspectionReviewPriority::Low);
    let mut evidence_ids = findings
        .iter()
        .flat_map(|finding| finding.evidence_ids.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    evidence_ids.truncate(evidence_limit);
    let mut cycle_findings = findings
        .into_iter()
        .map(|finding| ReflectionFinding {
            id: finding.id.clone(),
            kind: finding.kind.clone(),
            severity: finding.severity.clone(),
            title: finding.title.clone(),
            detail: finding.detail.clone(),
            dimensions: finding.dimensions.clone(),
            evidence_ids: finding.evidence_ids.clone(),
            suggested_action: finding.suggested_action.clone(),
        })
        .collect::<Vec<_>>();
    cycle_findings.sort_by(|left, right| {
        severity_rank(&left.severity)
            .cmp(&severity_rank(&right.severity))
            .then_with(|| left.title.cmp(&right.title))
            .then_with(|| left.id.cmp(&right.id))
    });

    ReflectionCycle {
        id: format!(
            "cycle_{}_{}",
            priority_slug(&priority),
            action_slug(&action)
        ),
        priority,
        title: action_title(&action).to_string(),
        rationale: action_rationale(&action).to_string(),
        action,
        finding_count: cycle_findings.len(),
        evidence_ids,
        findings: cycle_findings,
    }
}

fn source_coverage(data: &MemoryData) -> ReflectionSourceCoverage {
    let episode_count = data.episodes.len();
    let sourced_episode_count = data
        .episodes
        .iter()
        .filter(|episode| episode.source_id.is_some())
        .count();
    let derived_memory_count =
        data.claims.len() + data.links.len() + data.procedures.len() + data.intentions.len();
    let evidence_backed_memory_count = data
        .claims
        .iter()
        .filter(|claim| claim.source_episode_id.is_some())
        .count()
        + data
            .links
            .iter()
            .filter(|link| link.source_episode_id.is_some())
            .count()
        + data
            .procedures
            .iter()
            .filter(|procedure| procedure.source_episode_id.is_some())
            .count()
        + data
            .intentions
            .iter()
            .filter(|intention| intention.source_episode_id.is_some())
            .count();

    ReflectionSourceCoverage {
        source_count: data.sources.len(),
        episode_count,
        sourced_episode_count,
        unsourced_episode_count: episode_count.saturating_sub(sourced_episode_count),
        derived_memory_count,
        evidence_backed_memory_count,
        unsupported_memory_count: derived_memory_count.saturating_sub(evidence_backed_memory_count),
        source_coverage_ratio: ratio(sourced_episode_count, episode_count),
        evidence_coverage_ratio: ratio(evidence_backed_memory_count, derived_memory_count),
    }
}

fn action_for_kind(kind: &SelfInspectionFindingKind) -> SelfInspectionReviewAction {
    match kind {
        SelfInspectionFindingKind::Contradiction => {
            SelfInspectionReviewAction::ResolveContradiction
        }
        SelfInspectionFindingKind::StaleMemory => SelfInspectionReviewAction::RefreshMemory,
        SelfInspectionFindingKind::BlindSpot => SelfInspectionReviewAction::LinkMemory,
        SelfInspectionFindingKind::WeakEvidence => SelfInspectionReviewAction::CaptureEvidence,
        SelfInspectionFindingKind::SourceCoverage => SelfInspectionReviewAction::CaptureEvidence,
        SelfInspectionFindingKind::LowConfidence => SelfInspectionReviewAction::CaptureEvidence,
        SelfInspectionFindingKind::ConsolidationOpportunity => {
            SelfInspectionReviewAction::ConsolidatePattern
        }
        SelfInspectionFindingKind::LatentIntention => SelfInspectionReviewAction::ReviewIntention,
    }
}

fn priority_for_finding(finding: &SelfInspectionFinding) -> SelfInspectionReviewPriority {
    match (&finding.kind, &finding.severity) {
        (SelfInspectionFindingKind::Contradiction, HealthSeverity::High) => {
            SelfInspectionReviewPriority::Critical
        }
        (_, HealthSeverity::High) => SelfInspectionReviewPriority::High,
        (_, HealthSeverity::Medium) => SelfInspectionReviewPriority::Medium,
        (_, HealthSeverity::Low) => SelfInspectionReviewPriority::Low,
    }
}

fn priority_rank(priority: &SelfInspectionReviewPriority) -> u8 {
    match priority {
        SelfInspectionReviewPriority::Critical => 0,
        SelfInspectionReviewPriority::High => 1,
        SelfInspectionReviewPriority::Medium => 2,
        SelfInspectionReviewPriority::Low => 3,
    }
}

fn severity_rank(severity: &HealthSeverity) -> u8 {
    match severity {
        HealthSeverity::High => 0,
        HealthSeverity::Medium => 1,
        HealthSeverity::Low => 2,
    }
}

fn priority_slug(priority: &SelfInspectionReviewPriority) -> &'static str {
    match priority {
        SelfInspectionReviewPriority::Critical => "critical",
        SelfInspectionReviewPriority::High => "high",
        SelfInspectionReviewPriority::Medium => "medium",
        SelfInspectionReviewPriority::Low => "low",
    }
}

fn action_slug(action: &SelfInspectionReviewAction) -> &'static str {
    match action {
        SelfInspectionReviewAction::CaptureEvidence => "capture_evidence",
        SelfInspectionReviewAction::ResolveContradiction => "resolve_contradiction",
        SelfInspectionReviewAction::RefreshMemory => "refresh_memory",
        SelfInspectionReviewAction::LinkMemory => "link_memory",
        SelfInspectionReviewAction::ConsolidatePattern => "consolidate_pattern",
        SelfInspectionReviewAction::ReviewIntention => "review_intention",
    }
}

fn action_title(action: &SelfInspectionReviewAction) -> &'static str {
    match action {
        SelfInspectionReviewAction::CaptureEvidence => "Capture missing evidence",
        SelfInspectionReviewAction::ResolveContradiction => "Resolve contradictions",
        SelfInspectionReviewAction::RefreshMemory => "Refresh stale memory",
        SelfInspectionReviewAction::LinkMemory => "Link disconnected memory",
        SelfInspectionReviewAction::ConsolidatePattern => "Consolidate repeated patterns",
        SelfInspectionReviewAction::ReviewIntention => "Review active intentions",
    }
}

fn action_rationale(action: &SelfInspectionReviewAction) -> &'static str {
    match action {
        SelfInspectionReviewAction::CaptureEvidence => {
            "Some derived memory needs stronger source support before it should guide work."
        }
        SelfInspectionReviewAction::ResolveContradiction => {
            "Contradictory memory should be resolved by an explicit operator decision before trust increases."
        }
        SelfInspectionReviewAction::RefreshMemory => {
            "Stale memory should be refreshed with a new observation before continued reliance."
        }
        SelfInspectionReviewAction::LinkMemory => {
            "Disconnected memory should be linked to surrounding context so future recall has better continuity."
        }
        SelfInspectionReviewAction::ConsolidatePattern => {
            "Repeated observations can become reusable memory only after the pattern is confirmed."
        }
        SelfInspectionReviewAction::ReviewIntention => {
            "Active intentions should be reviewed, evidenced, updated, or closed before they drive work."
        }
    }
}

fn ratio(numerator: usize, denominator: usize) -> f32 {
    if denominator == 0 {
        return 0.0;
    }
    (((numerator as f32 / denominator as f32) * 100.0).round()) / 100.0
}
