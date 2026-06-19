use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::inspection::{evidence_reviewed, resolved_review_evidence};
use crate::{
    AuthorityDecision, Claim, HealthDimension, HealthSeverity, HealthSignal, HealthSignalKind,
    IntentionPriority, IntentionStatus, KnowledgeHealth, Link, MemoryData,
};

#[path = "self_inspection_source.rs"]
mod source;

/// Current non-mutating self-inspection report format version.
pub const SELF_INSPECTION_REPORT_VERSION: u32 = 2;

const DAY_MS: u64 = 24 * 60 * 60 * 1000;
const STALE_INTENTION_AFTER_DAYS: u64 = 30;
/// Minimum items of a kind before an overconfidence finding may be raised.
const CONFIDENCE_ALIGNMENT_MIN_SAMPLES: usize = 5;
/// Supplied confidence at or above this, with no source episode, is overconfident.
const OVERCONFIDENT_CONFIDENCE: f32 = 0.75;
/// Supplied confidence at or below this, despite a source episode, is conservative.
const CONSERVATIVE_CONFIDENCE: f32 = 0.25;

/// Non-mutating self-inspection report for the current memory projection.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct SelfInspectionReport {
    /// Report format version.
    pub version: u32,
    /// Timestamp in milliseconds when the report was generated.
    pub generated_at_ms: u64,
    /// Number of source events represented by the inspected projection.
    pub event_count: usize,
    /// Knowledge-health report used as the base inspection pass.
    pub health: KnowledgeHealth,
    /// Authority decision computed from the health report.
    pub authority: AuthorityDecision,
    /// Aggregate finding counts.
    pub summary: SelfInspectionSummary,
    /// Deterministic, informational signal of how much repair work is available.
    /// This is self-repair step 0: it detects and surfaces repair candidates but
    /// never writes. Acting on it requires an explicit `nahuali repair` proposal.
    pub repair_signal: RepairNeedSignal,
    /// Non-mutating audit of how well supplied confidence tracks evidence presence.
    pub confidence_alignment: ConfidenceProvenanceAlignment,
    /// Findings that should be reviewed before memory is trusted or improved.
    pub findings: Vec<SelfInspectionFinding>,
    /// Proposed operator review queue. These items do not mutate memory.
    pub review_queue: Vec<SelfInspectionReviewItem>,
    /// Explicit policy for automatic write-back.
    pub write_back_policy: SelfInspectionWriteBackPolicy,
}

/// Aggregate counts for a self-inspection report.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SelfInspectionSummary {
    /// Total number of findings.
    pub finding_count: usize,
    /// Number of contradiction findings.
    pub contradiction_count: usize,
    /// Number of stale-memory findings.
    pub stale_memory_count: usize,
    /// Number of blind-spot findings.
    pub blind_spot_count: usize,
    /// Number of weak-evidence findings.
    pub weak_evidence_count: usize,
    /// Number of source-coverage findings.
    pub source_coverage_count: usize,
    /// Number of low-confidence findings.
    pub low_confidence_count: usize,
    /// Number of overconfident-unsourced (confidence-provenance mismatch) findings.
    pub confidence_mismatch_count: usize,
    /// Number of consolidation opportunity findings.
    pub consolidation_opportunity_count: usize,
    /// Number of latent-intention findings.
    pub latent_intention_count: usize,
    /// Number of high or critical review items.
    pub high_priority_review_count: usize,
}

/// Deterministic, informational signal of how much self-repair work the engine
/// can already detect.
///
/// This is self-repair step 0: it surfaces repair candidates from the same
/// deterministic findings that drive the review queue, but it never writes.
/// Acting on a candidate requires an explicit `nahuali repair` proposal, which
/// an LLM authors and the engine then validates, classifies, and gates.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct RepairNeedSignal {
    /// Repeated-pattern findings an LLM could consolidate into a claim.
    pub consolidation_candidate_count: usize,
    /// Isolated entities an LLM could link to related memory.
    pub link_candidate_count: usize,
    /// Total deterministic repair candidates surfaced.
    pub candidate_count: usize,
    /// Human-readable guidance. Self-repair never runs automatically.
    pub guidance: String,
}

/// A single self-inspection finding.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SelfInspectionFinding {
    /// Stable identifier within this report.
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

/// Self-inspection finding category.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SelfInspectionFindingKind {
    /// Memory contains contradictory claims.
    Contradiction,
    /// Memory is old enough to need refresh.
    StaleMemory,
    /// Memory coverage or connectivity is incomplete.
    BlindSpot,
    /// A memory item is detached from sufficient evidence.
    WeakEvidence,
    /// Source provenance coverage is incomplete.
    SourceCoverage,
    /// A memory item has low confidence.
    LowConfidence,
    /// Assertional memory states high confidence but carries no source episode.
    ConfidenceProvenanceMismatch,
    /// Multiple records suggest a useful consolidation opportunity.
    ConsolidationOpportunity,
    /// An active intention needs review or evidence before it should guide work.
    LatentIntention,
}

/// Proposed operator review item.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SelfInspectionReviewItem {
    /// Stable identifier within this report.
    pub id: String,
    /// Finding identifier that created this review item.
    pub finding_id: String,
    /// Review priority.
    pub priority: SelfInspectionReviewPriority,
    /// Proposed review action.
    pub action: SelfInspectionReviewAction,
    /// Review lifecycle status.
    pub status: SelfInspectionReviewStatus,
    /// Short review title.
    pub title: String,
    /// Review detail.
    pub detail: String,
    /// Event or memory identifiers supporting the review item.
    pub evidence_ids: Vec<String>,
}

/// Review priority for a self-inspection finding.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SelfInspectionReviewPriority {
    /// Requires immediate review before trusting the affected memory.
    Critical,
    /// High-priority review.
    High,
    /// Medium-priority review.
    Medium,
    /// Low-priority review.
    Low,
}

/// Proposed review action.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SelfInspectionReviewAction {
    /// Record or attach missing evidence.
    CaptureEvidence,
    /// Resolve a contradiction.
    ResolveContradiction,
    /// Refresh stale memory.
    RefreshMemory,
    /// Add or strengthen graph links.
    LinkMemory,
    /// Consolidate repeated observations into a reusable claim, link, or procedure.
    ConsolidatePattern,
    /// Review an active intention.
    ReviewIntention,
}

/// Review lifecycle status.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SelfInspectionReviewStatus {
    /// Proposed by a non-mutating report and not yet accepted.
    Proposed,
}

/// Explicit policy for automatic write-back from self-inspection.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SelfInspectionWriteBackPolicy {
    /// Whether this report automatically writes new memory records.
    pub automatic_write_back: bool,
    /// Whether an operator must approve any proposed mutation.
    pub requires_operator_review: bool,
    /// Human-readable policy message.
    pub message: String,
}

/// Non-mutating audit of how well supplied confidence tracks evidence presence,
/// reported separately per memory kind.
///
/// This is an evidence-PRESENCE audit, NOT calibration against truth: a claim at
/// 0.9 confidence with a source episode scores as aligned even if the claim is
/// false, and a correct claim with no attached episode scores as overconfident.
/// `evidence_backed` is already reported as the health supported/unsupported
/// fact counts; this report adds only the confidence-versus-provenance
/// bucketing, no new ground truth about correctness. Kinds are separated because
/// procedures and preferences are operational and legitimately lack a source
/// episode, so an overconfidence *finding* is only raised for assertional claims
/// and links, never procedures.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ConfidenceProvenanceAlignment {
    /// Alignment over the projected claims.
    pub claims: ConfidenceProvenanceKindReport,
    /// Alignment over the projected links.
    pub links: ConfidenceProvenanceKindReport,
    /// Alignment over procedures and preferences (reported, never a finding).
    pub procedures: ConfidenceProvenanceKindReport,
}

/// Per-kind confidence-versus-provenance alignment numbers.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ConfidenceProvenanceKindReport {
    /// Number of items of this kind in the projection.
    pub sample_count: usize,
    /// Number of items that carry a source episode.
    pub evidence_backed_count: usize,
    /// Mean squared difference between supplied confidence and evidence
    /// backedness (1.0 with a source episode, 0.0 without), rounded to two
    /// decimals. 0.0 is perfect alignment; values near 1.0 are maximally
    /// misaligned. This is an evidence-presence error, not a calibration or
    /// accuracy metric.
    pub evidence_alignment_error: f32,
    /// Items asserting high confidence (>= 0.75) with no source episode.
    pub overconfident_count: usize,
    /// Items asserting low confidence (<= 0.25) despite carrying a source episode.
    pub conservative_count: usize,
    /// True when `sample_count` is below the reliability threshold; the ratios
    /// above are reported but must not be read as a trustworthy signal, and no
    /// overconfidence finding is raised.
    pub insufficient_samples: bool,
}

impl ConfidenceProvenanceKindReport {
    /// Provenance-coverage rate for this kind: the fraction of items that carry
    /// a source episode (`evidence_backed_count / sample_count`), rounded to two
    /// decimals, or `None` when there are no items. This is the compliance-facing
    /// number -- how much of this memory is traceable back to a recorded
    /// observation -- and is recomputable by any caller from the same projection.
    /// Read it together with `insufficient_samples`, which flags a `sample_count`
    /// too small for the rate to be a trustworthy signal.
    pub fn provenance_coverage_rate(&self) -> Option<f32> {
        if self.sample_count == 0 {
            return None;
        }
        Some(round2(
            self.evidence_backed_count as f32 / self.sample_count as f32,
        ))
    }

    /// Overconfidence rate for this kind: the fraction of items that assert high
    /// confidence (>= 0.75) with no source episode (`overconfident_count /
    /// sample_count`), rounded to two decimals, or `None` when there are no
    /// items. The inverse risk signal to coverage: high coverage with a low
    /// overconfidence rate is the healthy state.
    pub fn overconfidence_rate(&self) -> Option<f32> {
        if self.sample_count == 0 {
            return None;
        }
        Some(round2(
            self.overconfident_count as f32 / self.sample_count as f32,
        ))
    }
}

struct KindAlignment {
    report: ConfidenceProvenanceKindReport,
    overconfident_event_ids: Vec<String>,
}

pub(crate) fn self_inspect(data: &MemoryData) -> SelfInspectionReport {
    self_inspect_at(data, now_ms())
}

pub(crate) fn self_inspect_at(data: &MemoryData, now_ms: u64) -> SelfInspectionReport {
    let health = KnowledgeHealth::inspect_at(data, now_ms);
    let authority = AuthorityDecision::evaluate(&health);
    let mut findings = health
        .signals
        .iter()
        .map(finding_from_health_signal)
        .collect::<Vec<_>>();

    append_consolidation_opportunities(data, &mut findings);
    source::append_source_coverage_findings(data, &mut findings);
    append_latent_intentions(data, now_ms, &mut findings);

    let claim_align = align_kind(projected_claims(data).iter().map(|claim| {
        (
            claim.event_id.as_str(),
            claim.confidence,
            claim.source_episode_id.is_some(),
        )
    }));
    let link_align = align_kind(projected_links(data).iter().map(|link| {
        (
            link.event_id.as_str(),
            link.confidence,
            link.source_episode_id.is_some(),
        )
    }));
    let procedure_align = align_kind(data.procedures.iter().map(|procedure| {
        (
            procedure.event_id.as_str(),
            procedure.confidence,
            procedure.source_episode_id.is_some(),
        )
    }));

    // Only assertional kinds (claims, links) can be "overconfident"; procedures
    // and preferences are operational and legitimately lack a source episode.
    append_confidence_mismatch_finding(data, "claim", &claim_align, &mut findings);
    append_confidence_mismatch_finding(data, "link", &link_align, &mut findings);

    let confidence_alignment = ConfidenceProvenanceAlignment {
        claims: claim_align.report,
        links: link_align.report,
        procedures: procedure_align.report,
    };

    let review_queue = findings
        .iter()
        .map(review_item_from_finding)
        .collect::<Vec<_>>();
    let summary = summarize(&findings, &review_queue);
    let repair_signal = repair_need_signal(&summary, &health);

    SelfInspectionReport {
        version: SELF_INSPECTION_REPORT_VERSION,
        generated_at_ms: now_ms,
        event_count: data.event_count,
        health,
        authority,
        summary,
        repair_signal,
        confidence_alignment,
        findings,
        review_queue,
        write_back_policy: SelfInspectionWriteBackPolicy {
            automatic_write_back: false,
            requires_operator_review: true,
            message:
                "self-inspection reports are non-mutating; write-back requires an explicit operator-reviewed action"
                    .to_string(),
        },
    }
}

fn finding_from_health_signal(signal: &HealthSignal) -> SelfInspectionFinding {
    let (kind, title, suggested_action) = match signal.kind {
        HealthSignalKind::NoEpisodes => (
            SelfInspectionFindingKind::BlindSpot,
            "No observed episodes",
            "Record source episodes before deriving memory.",
        ),
        HealthSignalKind::UnsupportedFact => (
            SelfInspectionFindingKind::WeakEvidence,
            "Memory without source evidence",
            "Attach an episode or record evidence that supports this memory.",
        ),
        HealthSignalKind::LowConfidenceFact => (
            SelfInspectionFindingKind::LowConfidence,
            "Low-confidence memory",
            "Verify the memory and raise confidence only after evidence improves.",
        ),
        HealthSignalKind::ConflictingFact => (
            SelfInspectionFindingKind::Contradiction,
            "Contradictory memory",
            "Review the conflicting values and record the resolution as new evidence.",
        ),
        HealthSignalKind::StaleFact => (
            SelfInspectionFindingKind::StaleMemory,
            "Stale memory",
            "Refresh or retire the stale memory with a new observed episode.",
        ),
        HealthSignalKind::StaleEpisode => (
            SelfInspectionFindingKind::StaleMemory,
            "Dormant memory store",
            "The store's most recent memory is old; record a current observation to refresh it.",
        ),
        HealthSignalKind::SupersededFact => (
            SelfInspectionFindingKind::StaleMemory,
            "Superseded memory",
            "Retire or archive the older value now that a newer evidence-backed fact replaced it.",
        ),
        HealthSignalKind::IsolatedEntity => (
            SelfInspectionFindingKind::BlindSpot,
            "Isolated entity",
            "Connect the entity with evidence-backed links or record more context.",
        ),
    };
    let id = finding_id(&kind, &signal.evidence_ids, &signal.message);

    SelfInspectionFinding {
        id,
        kind,
        severity: signal.severity.clone(),
        title: title.to_string(),
        detail: signal.message.clone(),
        dimensions: signal.dimensions.clone(),
        evidence_ids: signal.evidence_ids.clone(),
        suggested_action: suggested_action.to_string(),
    }
}

fn append_consolidation_opportunities(
    data: &MemoryData,
    findings: &mut Vec<SelfInspectionFinding>,
) {
    let mut tags = BTreeMap::<String, Vec<String>>::new();
    for episode in &data.episodes {
        for tag in &episode.tags {
            tags.entry(tag.trim().to_ascii_lowercase())
                .or_default()
                .push(episode.id.clone());
        }
    }

    for (tag, episode_ids) in tags {
        if tag.is_empty() || episode_ids.len() < 3 {
            continue;
        }
        let detail = format!(
            "Tag '{tag}' appears in {} episodes and may deserve a consolidated claim, link, or procedure.",
            episode_ids.len()
        );
        findings.push(SelfInspectionFinding {
            id: finding_id(
                &SelfInspectionFindingKind::ConsolidationOpportunity,
                &episode_ids,
                &detail,
            ),
            kind: SelfInspectionFindingKind::ConsolidationOpportunity,
            severity: HealthSeverity::Low,
            title: "Repeated episode pattern".to_string(),
            detail,
            dimensions: vec![HealthDimension::Completeness, HealthDimension::Connectivity],
            evidence_ids: episode_ids,
            suggested_action:
                "Review repeated observations and consolidate only if the pattern is durable."
                    .to_string(),
        });
    }
}

fn append_latent_intentions(
    data: &MemoryData,
    now_ms: u64,
    findings: &mut Vec<SelfInspectionFinding>,
) {
    let stale_before_ms = now_ms.saturating_sub(STALE_INTENTION_AFTER_DAYS * DAY_MS);
    for intention in data
        .intentions
        .iter()
        .filter(|intention| intention.status == IntentionStatus::Active)
    {
        let missing_evidence = intention.source_episode_id.is_none();
        let stale = intention.updated_at_ms < stale_before_ms;
        if !missing_evidence && !stale {
            continue;
        }

        let severity = match intention.priority {
            IntentionPriority::Critical | IntentionPriority::High => HealthSeverity::Medium,
            IntentionPriority::Medium | IntentionPriority::Low => HealthSeverity::Low,
        };
        let mut detail = format!("Active intention '{}' needs review.", intention.description);
        if missing_evidence {
            detail.push_str(" It has no source episode.");
        }
        if stale {
            detail.push_str(" It has not changed recently.");
        }

        let evidence_ids = vec![intention.event_id.clone()];
        findings.push(SelfInspectionFinding {
            id: finding_id(
                &SelfInspectionFindingKind::LatentIntention,
                &evidence_ids,
                &detail,
            ),
            kind: SelfInspectionFindingKind::LatentIntention,
            severity,
            title: "Latent active intention".to_string(),
            detail,
            dimensions: vec![HealthDimension::Completeness, HealthDimension::BlindSpot],
            evidence_ids,
            suggested_action:
                "Review the intention, attach evidence, update status, or convert it into current work."
                    .to_string(),
        });
    }
}

fn align_kind<'a>(items: impl Iterator<Item = (&'a str, f32, bool)>) -> KindAlignment {
    let items = items.collect::<Vec<_>>();
    let sample_count = items.len();
    let evidence_backed_count = items.iter().filter(|(_, _, backed)| *backed).count();
    let evidence_alignment_error = if sample_count == 0 {
        0.0
    } else {
        let total = items
            .iter()
            .map(|(_, confidence, backed)| {
                let target = if *backed { 1.0 } else { 0.0 };
                let delta = confidence - target;
                delta * delta
            })
            .sum::<f32>();
        round2(total / sample_count as f32)
    };
    let overconfident_event_ids = items
        .iter()
        .filter(|(_, confidence, backed)| *confidence >= OVERCONFIDENT_CONFIDENCE && !*backed)
        .map(|(event_id, _, _)| event_id.to_string())
        .collect::<Vec<_>>();
    let conservative_count = items
        .iter()
        .filter(|(_, confidence, backed)| *confidence <= CONSERVATIVE_CONFIDENCE && *backed)
        .count();
    KindAlignment {
        report: ConfidenceProvenanceKindReport {
            sample_count,
            evidence_backed_count,
            evidence_alignment_error,
            overconfident_count: overconfident_event_ids.len(),
            conservative_count,
            insufficient_samples: sample_count < CONFIDENCE_ALIGNMENT_MIN_SAMPLES,
        },
        overconfident_event_ids,
    }
}

fn append_confidence_mismatch_finding(
    data: &MemoryData,
    kind_label: &str,
    align: &KindAlignment,
    findings: &mut Vec<SelfInspectionFinding>,
) {
    if align.report.insufficient_samples || align.overconfident_event_ids.is_empty() {
        return;
    }
    let resolved = resolved_review_evidence(data);
    let unresolved = align
        .overconfident_event_ids
        .iter()
        .filter(|event_id| !evidence_reviewed(&[event_id.as_str()], &resolved))
        .cloned()
        .collect::<Vec<_>>();
    if unresolved.is_empty() {
        return;
    }
    let detail = format!(
        "{} {kind_label}(s) state confidence at or above 0.75 with no source episode.",
        unresolved.len()
    );
    findings.push(SelfInspectionFinding {
        id: finding_id(
            &SelfInspectionFindingKind::ConfidenceProvenanceMismatch,
            &unresolved,
            &detail,
        ),
        kind: SelfInspectionFindingKind::ConfidenceProvenanceMismatch,
        severity: HealthSeverity::Low,
        title: "Overconfident unsourced memory".to_string(),
        detail,
        dimensions: vec![HealthDimension::Confidence, HealthDimension::UnsupportedMemory],
        evidence_ids: unresolved,
        suggested_action:
            "Attach a source episode or lower the stated confidence for high-confidence memory that has no evidence."
                .to_string(),
    });
}

fn projected_claims(data: &MemoryData) -> &[Claim] {
    if data.claims.is_empty() {
        &data.facts
    } else {
        &data.claims
    }
}

fn projected_links(data: &MemoryData) -> &[Link] {
    if data.links.is_empty() {
        &data.relations
    } else {
        &data.links
    }
}

fn round2(value: f32) -> f32 {
    (value * 100.0).round() / 100.0
}

fn review_item_from_finding(finding: &SelfInspectionFinding) -> SelfInspectionReviewItem {
    SelfInspectionReviewItem {
        id: review_id(&finding.id),
        finding_id: finding.id.clone(),
        priority: review_priority(finding),
        action: review_action(&finding.kind),
        status: SelfInspectionReviewStatus::Proposed,
        title: finding.title.clone(),
        detail: finding.suggested_action.clone(),
        evidence_ids: finding.evidence_ids.clone(),
    }
}

fn review_priority(finding: &SelfInspectionFinding) -> SelfInspectionReviewPriority {
    match (&finding.kind, &finding.severity) {
        (SelfInspectionFindingKind::Contradiction, HealthSeverity::High) => {
            SelfInspectionReviewPriority::Critical
        }
        (_, HealthSeverity::High) => SelfInspectionReviewPriority::High,
        (_, HealthSeverity::Medium) => SelfInspectionReviewPriority::Medium,
        (_, HealthSeverity::Low) => SelfInspectionReviewPriority::Low,
    }
}

fn review_action(kind: &SelfInspectionFindingKind) -> SelfInspectionReviewAction {
    match kind {
        SelfInspectionFindingKind::Contradiction => {
            SelfInspectionReviewAction::ResolveContradiction
        }
        SelfInspectionFindingKind::StaleMemory => SelfInspectionReviewAction::RefreshMemory,
        SelfInspectionFindingKind::BlindSpot => SelfInspectionReviewAction::LinkMemory,
        SelfInspectionFindingKind::WeakEvidence => SelfInspectionReviewAction::CaptureEvidence,
        SelfInspectionFindingKind::SourceCoverage => SelfInspectionReviewAction::CaptureEvidence,
        SelfInspectionFindingKind::LowConfidence => SelfInspectionReviewAction::CaptureEvidence,
        SelfInspectionFindingKind::ConfidenceProvenanceMismatch => {
            SelfInspectionReviewAction::CaptureEvidence
        }
        SelfInspectionFindingKind::ConsolidationOpportunity => {
            SelfInspectionReviewAction::ConsolidatePattern
        }
        SelfInspectionFindingKind::LatentIntention => SelfInspectionReviewAction::ReviewIntention,
    }
}

/// Derive the informational self-repair step-0 signal from the deterministic
/// findings: repeated-pattern findings are consolidation candidates, isolated
/// entities are link candidates. This detects and surfaces; it never writes.
fn repair_need_signal(
    summary: &SelfInspectionSummary,
    health: &KnowledgeHealth,
) -> RepairNeedSignal {
    let consolidation_candidate_count = summary.consolidation_opportunity_count;
    let link_candidate_count = health.isolated_entity_count;
    let candidate_count = consolidation_candidate_count + link_candidate_count;
    let guidance = if candidate_count == 0 {
        "No deterministic repair candidates detected.".to_string()
    } else {
        format!(
            "{candidate_count} deterministic repair candidate(s): {consolidation_candidate_count} consolidation, {link_candidate_count} isolated entity. Propose repairs with `nahuali repair`; the engine validates and gates each one. Self-repair never runs automatically."
        )
    };
    RepairNeedSignal {
        consolidation_candidate_count,
        link_candidate_count,
        candidate_count,
        guidance,
    }
}

fn summarize(
    findings: &[SelfInspectionFinding],
    review_queue: &[SelfInspectionReviewItem],
) -> SelfInspectionSummary {
    SelfInspectionSummary {
        finding_count: findings.len(),
        contradiction_count: count_kind(findings, SelfInspectionFindingKind::Contradiction),
        stale_memory_count: count_kind(findings, SelfInspectionFindingKind::StaleMemory),
        blind_spot_count: count_kind(findings, SelfInspectionFindingKind::BlindSpot),
        weak_evidence_count: count_kind(findings, SelfInspectionFindingKind::WeakEvidence),
        source_coverage_count: count_kind(findings, SelfInspectionFindingKind::SourceCoverage),
        low_confidence_count: count_kind(findings, SelfInspectionFindingKind::LowConfidence),
        confidence_mismatch_count: count_kind(
            findings,
            SelfInspectionFindingKind::ConfidenceProvenanceMismatch,
        ),
        consolidation_opportunity_count: count_kind(
            findings,
            SelfInspectionFindingKind::ConsolidationOpportunity,
        ),
        latent_intention_count: count_kind(findings, SelfInspectionFindingKind::LatentIntention),
        high_priority_review_count: review_queue
            .iter()
            .filter(|item| {
                item.priority == SelfInspectionReviewPriority::Critical
                    || item.priority == SelfInspectionReviewPriority::High
            })
            .count(),
    }
}

fn count_kind(findings: &[SelfInspectionFinding], kind: SelfInspectionFindingKind) -> usize {
    findings
        .iter()
        .filter(|finding| finding.kind == kind)
        .count()
}

fn finding_id(kind: &SelfInspectionFindingKind, evidence_ids: &[String], detail: &str) -> String {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(kind_slug(kind).as_bytes());
    bytes.push(0);
    if evidence_ids.is_empty() {
        bytes.extend_from_slice(detail.as_bytes());
    } else {
        for evidence_id in evidence_ids {
            bytes.extend_from_slice(evidence_id.as_bytes());
            bytes.push(0);
        }
    }
    format!("finding_{}_{:08x}", kind_slug(kind), fnv1a32(&bytes))
}

fn review_id(finding_id: &str) -> String {
    format!(
        "review_{}",
        finding_id.strip_prefix("finding_").unwrap_or(finding_id)
    )
}

fn kind_slug(kind: &SelfInspectionFindingKind) -> &'static str {
    match kind {
        SelfInspectionFindingKind::Contradiction => "contradiction",
        SelfInspectionFindingKind::StaleMemory => "stale_memory",
        SelfInspectionFindingKind::BlindSpot => "blind_spot",
        SelfInspectionFindingKind::WeakEvidence => "weak_evidence",
        SelfInspectionFindingKind::SourceCoverage => "source_coverage",
        SelfInspectionFindingKind::LowConfidence => "low_confidence",
        SelfInspectionFindingKind::ConfidenceProvenanceMismatch => "confidence_provenance_mismatch",
        SelfInspectionFindingKind::ConsolidationOpportunity => "consolidation_opportunity",
        SelfInspectionFindingKind::LatentIntention => "latent_intention",
    }
}

fn fnv1a32(bytes: &[u8]) -> u32 {
    const FNV_OFFSET: u32 = 0x811c9dc5;
    const FNV_PRIME: u32 = 0x01000193;

    bytes.iter().fold(FNV_OFFSET, |hash, byte| {
        (hash ^ u32::from(*byte)).wrapping_mul(FNV_PRIME)
    })
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time before Unix epoch")
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use crate::{
        AuthorityDecision, Claim, Entity, Episode, Intention, IntentionKind, IntentionPriority,
        IntentionStatus, MemoryData, Procedure, ProcedureKind, ReviewDecision,
        ReviewDecisionAction, ReviewDecisionOutcome, SelfInspectionFindingKind, SourceDocument,
        SourceKind,
        self_inspection::{DAY_MS, self_inspect_at},
    };

    #[test]
    fn provenance_coverage_and_overconfidence_rates_are_derived_from_counts() {
        use crate::self_inspection::ConfidenceProvenanceKindReport;

        // Eight claims: six sourced, and two of the unsourced ones assert high
        // confidence -> 0.75 coverage, 0.25 overconfidence.
        let report = ConfidenceProvenanceKindReport {
            sample_count: 8,
            evidence_backed_count: 6,
            evidence_alignment_error: 0.0,
            overconfident_count: 2,
            conservative_count: 0,
            insufficient_samples: false,
        };
        assert_eq!(report.provenance_coverage_rate(), Some(0.75));
        assert_eq!(report.overconfidence_rate(), Some(0.25));

        // An empty kind reports no rate rather than dividing by zero.
        let empty = ConfidenceProvenanceKindReport {
            sample_count: 0,
            evidence_backed_count: 0,
            evidence_alignment_error: 0.0,
            overconfident_count: 0,
            conservative_count: 0,
            insufficient_samples: true,
        };
        assert_eq!(empty.provenance_coverage_rate(), None);
        assert_eq!(empty.overconfidence_rate(), None);
    }

    #[test]
    fn reports_contradictions_staleness_and_blind_spots() {
        let now = 200 * DAY_MS;
        let data = MemoryData {
            event_count: 3,
            entities: vec![entity("Lena"), entity("Release Notes")],
            claims: vec![
                claim("claim_1", "event_1", "Lena", "role", "CTO", 0.4, 0),
                claim(
                    "claim_2",
                    "event_2",
                    "Lena",
                    "role",
                    "VP Engineering",
                    0.9,
                    1,
                ),
            ],
            facts: vec![
                claim("claim_1", "event_1", "Lena", "role", "CTO", 0.4, 0),
                claim(
                    "claim_2",
                    "event_2",
                    "Lena",
                    "role",
                    "VP Engineering",
                    0.9,
                    1,
                ),
            ],
            ..MemoryData::default()
        };

        let report = self_inspect_at(&data, now);

        assert!(!report.write_back_policy.automatic_write_back);
        assert!(has_kind(&report, SelfInspectionFindingKind::Contradiction));
        assert!(has_kind(&report, SelfInspectionFindingKind::StaleMemory));
        assert!(has_kind(&report, SelfInspectionFindingKind::WeakEvidence));
        assert!(has_kind(&report, SelfInspectionFindingKind::LowConfidence));
        assert!(has_kind(&report, SelfInspectionFindingKind::BlindSpot));
        assert_eq!(report.summary.contradiction_count, 1);
        assert_eq!(
            report.review_queue[0].status,
            crate::SelfInspectionReviewStatus::Proposed
        );
    }

    #[test]
    fn reports_consolidation_opportunities_from_repeated_episode_tags() {
        let data = MemoryData {
            event_count: 3,
            episodes: vec![
                episode(
                    "episode_1",
                    "event_1",
                    "Lena reviewed release notes",
                    "product",
                ),
                episode(
                    "episode_2",
                    "event_2",
                    "Lena edited release notes",
                    "product",
                ),
                episode(
                    "episode_3",
                    "event_3",
                    "Lena shipped release notes",
                    "product",
                ),
            ],
            ..MemoryData::default()
        };

        let report = self_inspect_at(&data, DAY_MS);

        assert!(has_kind(
            &report,
            SelfInspectionFindingKind::ConsolidationOpportunity
        ));
        assert_eq!(report.summary.consolidation_opportunity_count, 1);
        assert!(
            report
                .review_queue
                .iter()
                .any(|item| item.action == crate::SelfInspectionReviewAction::ConsolidatePattern)
        );
    }

    #[test]
    fn repair_signal_surfaces_consolidation_candidates_without_writing() {
        let data = MemoryData {
            event_count: 3,
            episodes: vec![
                episode("episode_1", "event_1", "Lena reviewed release notes", "product"),
                episode("episode_2", "event_2", "Lena edited release notes", "product"),
                episode("episode_3", "event_3", "Lena shipped release notes", "product"),
            ],
            ..MemoryData::default()
        };

        let report = self_inspect_at(&data, DAY_MS);

        // The repeated tag is a consolidation candidate, surfaced (not written).
        assert_eq!(report.repair_signal.consolidation_candidate_count, 1);
        assert_eq!(
            report.repair_signal.link_candidate_count,
            report.health.isolated_entity_count
        );
        assert_eq!(
            report.repair_signal.candidate_count,
            report.repair_signal.consolidation_candidate_count
                + report.repair_signal.link_candidate_count
        );
        assert!(report.repair_signal.candidate_count >= 1);
        assert!(report.repair_signal.guidance.contains("nahuali repair"));
        // Step 0 only informs: it never enables automatic write-back.
        assert!(!report.write_back_policy.automatic_write_back);
    }

    #[test]
    fn repair_signal_is_quiet_when_no_candidates_exist() {
        let report = self_inspect_at(&MemoryData::default(), DAY_MS);

        assert_eq!(report.repair_signal.candidate_count, 0);
        assert_eq!(
            report.repair_signal.guidance,
            "No deterministic repair candidates detected."
        );
    }

    #[test]
    fn reports_source_coverage_gaps() {
        let data = MemoryData {
            event_count: 2,
            episodes: vec![episode(
                "episode_1",
                "event_1",
                "Lena discussed release notes",
                "product",
            )],
            claims: vec![claim(
                "claim_1",
                "event_2",
                "Lena",
                "owns",
                "Release Notes",
                0.9,
                DAY_MS,
            )],
            ..MemoryData::default()
        };

        let report = self_inspect_at(&data, DAY_MS);

        assert!(has_kind(&report, SelfInspectionFindingKind::SourceCoverage));
        assert_eq!(report.summary.source_coverage_count, 1);
        assert!(report.review_queue.iter().any(|item| {
            item.action == crate::SelfInspectionReviewAction::CaptureEvidence
                && item.finding_id.contains("source_coverage")
        }));
    }

    #[test]
    fn skips_source_coverage_when_memory_is_sourced() {
        let mut backed_claim = claim(
            "claim_1",
            "event_3",
            "Lena",
            "owns",
            "Release Notes",
            0.9,
            DAY_MS,
        );
        backed_claim.source_episode_id = Some("episode_1".to_string());
        let mut backed_fact = backed_claim.clone();
        backed_fact.source_episode_id = Some("episode_1".to_string());
        let mut backed_episode = episode(
            "episode_1",
            "event_2",
            "Lena discussed release notes",
            "product",
        );
        backed_episode.source_id = Some("source_1".to_string());
        let data = MemoryData {
            event_count: 3,
            sources: vec![source("source_1", "event_1")],
            episodes: vec![backed_episode],
            claims: vec![backed_claim],
            facts: vec![backed_fact],
            ..MemoryData::default()
        };

        let report = self_inspect_at(&data, DAY_MS);

        assert!(!has_kind(
            &report,
            SelfInspectionFindingKind::SourceCoverage
        ));
        assert_eq!(report.summary.source_coverage_count, 0);
    }

    #[test]
    fn reports_latent_active_intentions() {
        let data = MemoryData {
            event_count: 1,
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
                deadline_at_ms: None,
                depends_on: Vec::new(),
                goal_id: None,
                progress_percent: None,
                scope: None,
                created_at_ms: 0,
                updated_at_ms: 0,
            }],
            ..MemoryData::default()
        };

        let report = self_inspect_at(&data, 40 * DAY_MS);

        assert!(has_kind(
            &report,
            SelfInspectionFindingKind::LatentIntention
        ));
        assert_eq!(report.summary.latent_intention_count, 1);
    }

    #[test]
    fn confidence_alignment_reports_exact_error_and_skips_small_samples() {
        let data = MemoryData {
            event_count: 2,
            claims: vec![
                conf_claim("c1", 0.8, Some("episode_1")),
                conf_claim("c2", 0.8, None),
            ],
            ..MemoryData::default()
        };

        let report = self_inspect_at(&data, DAY_MS);
        let claims = &report.confidence_alignment.claims;

        // ((0.8 - 1.0)^2 + (0.8 - 0.0)^2) / 2 = 0.34
        assert_eq!(claims.evidence_alignment_error, 0.34);
        assert_eq!(claims.sample_count, 2);
        assert_eq!(claims.evidence_backed_count, 1);
        assert_eq!(claims.overconfident_count, 1);
        assert!(claims.insufficient_samples);
        // A two-item bucket is too small to manufacture a finding.
        assert!(!has_kind(
            &report,
            SelfInspectionFindingKind::ConfidenceProvenanceMismatch
        ));
        assert_eq!(report.summary.confidence_mismatch_count, 0);
    }

    #[test]
    fn confidence_alignment_perfectly_aligned_is_zero() {
        let claims = (0..5)
            .map(|index| conf_claim(&format!("c{index}"), 1.0, Some("episode_1")))
            .collect::<Vec<_>>();
        let data = MemoryData {
            event_count: 5,
            claims,
            ..MemoryData::default()
        };

        let report = self_inspect_at(&data, DAY_MS);
        let claims = &report.confidence_alignment.claims;

        assert_eq!(claims.evidence_alignment_error, 0.0);
        assert_eq!(claims.overconfident_count, 0);
        assert!(!claims.insufficient_samples);
        assert!(!has_kind(
            &report,
            SelfInspectionFindingKind::ConfidenceProvenanceMismatch
        ));
    }

    #[test]
    fn confidence_alignment_maximal_misalignment_raises_finding() {
        let claims = (0..5)
            .map(|index| conf_claim(&format!("c{index}"), 1.0, None))
            .collect::<Vec<_>>();
        let data = MemoryData {
            event_count: 5,
            claims,
            ..MemoryData::default()
        };

        let report = self_inspect_at(&data, DAY_MS);
        let claims = &report.confidence_alignment.claims;

        // every item: (1.0 - 0.0)^2 = 1.0
        assert_eq!(claims.evidence_alignment_error, 1.0);
        assert_eq!(claims.overconfident_count, 5);
        assert!(!claims.insufficient_samples);
        assert!(has_kind(
            &report,
            SelfInspectionFindingKind::ConfidenceProvenanceMismatch
        ));
        assert_eq!(report.summary.confidence_mismatch_count, 1);
    }

    #[test]
    fn overconfident_preferences_are_reported_but_raise_no_finding() {
        let procedures = (0..5)
            .map(|index| preference(&format!("p{index}"), 0.9, None))
            .collect::<Vec<_>>();
        let data = MemoryData {
            event_count: 5,
            procedures,
            ..MemoryData::default()
        };

        let report = self_inspect_at(&data, DAY_MS);

        // Procedures are operational: reported, but never an overconfidence finding.
        assert_eq!(
            report.confidence_alignment.procedures.overconfident_count,
            5
        );
        assert!(!has_kind(
            &report,
            SelfInspectionFindingKind::ConfidenceProvenanceMismatch
        ));
        assert_eq!(report.summary.confidence_mismatch_count, 0);
    }

    #[test]
    fn review_resolved_overconfidence_raises_no_finding() {
        let claims = (0..5)
            .map(|index| conf_claim(&format!("c{index}"), 1.0, None))
            .collect::<Vec<_>>();
        let resolved_ids = (0..5).map(|index| format!("c{index}")).collect::<Vec<_>>();
        let data = MemoryData {
            event_count: 5,
            claims,
            review_decisions: vec![resolved_decision(resolved_ids)],
            ..MemoryData::default()
        };

        let report = self_inspect_at(&data, DAY_MS);

        // Still observed in the raw report...
        assert_eq!(report.confidence_alignment.claims.overconfident_count, 5);
        // ...but the operator already resolved it, so no finding fires.
        assert!(!has_kind(
            &report,
            SelfInspectionFindingKind::ConfidenceProvenanceMismatch
        ));
    }

    #[test]
    fn confidence_alignment_does_not_change_authority() {
        let claims = (0..5)
            .map(|index| conf_claim(&format!("c{index}"), 1.0, None))
            .collect::<Vec<_>>();
        let data = MemoryData {
            event_count: 5,
            claims,
            ..MemoryData::default()
        };

        let report = self_inspect_at(&data, DAY_MS);

        // The overconfidence finding lives in findings, never in health.signals,
        // so authority stays purely health-derived.
        assert!(has_kind(
            &report,
            SelfInspectionFindingKind::ConfidenceProvenanceMismatch
        ));
        assert_eq!(
            report.authority,
            AuthorityDecision::evaluate(&report.health)
        );
    }

    fn has_kind(report: &crate::SelfInspectionReport, kind: SelfInspectionFindingKind) -> bool {
        report.findings.iter().any(|finding| finding.kind == kind)
    }

    fn conf_claim(id: &str, confidence: f32, source: Option<&str>) -> Claim {
        Claim {
            id: id.to_string(),
            event_id: id.to_string(),
            subject: id.to_string(),
            predicate: "attribute".to_string(),
            object: "value".to_string(),
            source_episode_id: source.map(str::to_string),
            confidence,
            scope: None,
            created_at_ms: 0,
        }
    }

    fn preference(id: &str, confidence: f32, source: Option<&str>) -> Procedure {
        Procedure {
            id: id.to_string(),
            event_id: id.to_string(),
            kind: ProcedureKind::Preference,
            name: id.to_string(),
            body: "body".to_string(),
            source_episode_id: source.map(str::to_string),
            confidence,
            scope: None,
            created_at_ms: 0,
        }
    }

    fn resolved_decision(evidence_ids: Vec<String>) -> ReviewDecision {
        ReviewDecision {
            id: "decision_1".to_string(),
            event_id: "decision_event".to_string(),
            review_id: "review_1".to_string(),
            finding_id: "finding_1".to_string(),
            action: ReviewDecisionAction::CaptureEvidence,
            outcome: ReviewDecisionOutcome::Resolved,
            note: "resolved".to_string(),
            evidence_ids,
            scope: None,
            created_at_ms: 0,
        }
    }

    fn entity(name: &str) -> Entity {
        Entity {
            id: format!("entity_{}", name.to_ascii_lowercase().replace(' ', "_")),
            name: name.to_string(),
            mention_count: 1,
            first_seen_at_ms: 0,
            last_seen_at_ms: 0,
            source_event_ids: Vec::new(),
            scope: None,
        }
    }

    fn episode(id: &str, event_id: &str, content: &str, tag: &str) -> Episode {
        Episode {
            id: id.to_string(),
            event_id: event_id.to_string(),
            content: content.to_string(),
            tags: vec![tag.to_string()],
            mentions: Vec::new(),
            source_id: None,
            source_position: None,
            source_role: None,
            scope: None,
            created_at_ms: 0,
        }
    }

    fn claim(
        id: &str,
        event_id: &str,
        subject: &str,
        predicate: &str,
        object: &str,
        confidence: f32,
        created_at_ms: u64,
    ) -> Claim {
        Claim {
            id: id.to_string(),
            event_id: event_id.to_string(),
            subject: subject.to_string(),
            predicate: predicate.to_string(),
            object: object.to_string(),
            source_episode_id: None,
            confidence,
            scope: None,
            created_at_ms,
        }
    }

    fn source(id: &str, event_id: &str) -> SourceDocument {
        SourceDocument {
            id: id.to_string(),
            event_id: event_id.to_string(),
            kind: SourceKind::Conversation,
            title: Some("Release review".to_string()),
            uri: None,
            content_checksum: "checksum".to_string(),
            byte_len: 42,
            metadata: Default::default(),
            scope: None,
            created_at_ms: 0,
        }
    }
}
