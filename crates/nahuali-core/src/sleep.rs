use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    AuthorityDecision, KnowledgeHealth, MemoryData, MemoryReflectionReport, ReflectionOptions,
    SelfInspectionReport, SelfInspectionReviewAction, SelfInspectionReviewItem,
    SelfInspectionReviewPriority, SelfInspectionWriteBackPolicy, reflection, self_inspection,
};

/// Current Sleep Mode report format version.
pub const MEMORY_SLEEP_REPORT_VERSION: u32 = 1;

/// Options for building a non-mutating Sleep Mode report.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SleepModeOptions {
    /// Maximum recent episodes included in replay order.
    pub recent_episode_limit: usize,
    /// Maximum consolidation candidates returned.
    pub candidate_limit: usize,
    /// Reflection options used by the sleep pass.
    pub reflection: ReflectionOptions,
}

impl Default for SleepModeOptions {
    fn default() -> Self {
        Self {
            recent_episode_limit: 8,
            candidate_limit: 12,
            reflection: ReflectionOptions::default(),
        }
    }
}

/// Non-mutating report for a memory sleep/consolidation pass.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct MemorySleepReport {
    /// Report format version.
    pub version: u32,
    /// Timestamp in milliseconds when the report was generated.
    pub generated_at_ms: u64,
    /// Number of source events represented by the projection.
    pub event_count: usize,
    /// Projection-level authority decision.
    pub authority: AuthorityDecision,
    /// Knowledge-health report used by the sleep pass.
    pub health: KnowledgeHealth,
    /// Aggregate Sleep Mode counts.
    pub summary: SleepModeSummary,
    /// Deterministic stages of this sleep pass.
    pub stages: Vec<SleepStage>,
    /// Recent episodes replayed by this sleep pass.
    pub recent_episodes: Vec<SleepEpisodeReplay>,
    /// Candidate consolidation work derived from existing evidence.
    pub consolidation_candidates: Vec<SleepConsolidationCandidate>,
    /// Proposed review items that require explicit operator approval.
    pub review_items: Vec<SelfInspectionReviewItem>,
    /// Reflection report used to group self-inspection findings.
    pub reflection: MemoryReflectionReport,
    /// Self-inspection report used by the sleep pass.
    pub self_inspection: SelfInspectionReport,
    /// Explicit policy for automatic write-back.
    pub write_back_policy: SelfInspectionWriteBackPolicy,
}

/// Aggregate counts for a Sleep Mode report.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SleepModeSummary {
    /// Number of recent episodes replayed.
    pub replayed_episode_count: usize,
    /// Number of self-inspection findings considered.
    pub finding_count: usize,
    /// Number of reflection cycles returned.
    pub reflection_cycle_count: usize,
    /// Number of consolidation candidates returned.
    pub consolidation_candidate_count: usize,
    /// Number of proposed review items returned.
    pub review_item_count: usize,
    /// Number of stages with pending work.
    pub pending_stage_count: usize,
    /// Whether this sleep pass authorizes automatic write-back.
    pub automatic_write_back: bool,
}

/// Deterministic stage in a Sleep Mode pass.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SleepStage {
    /// Stable stage identifier.
    pub id: String,
    /// Stage status.
    pub status: SleepStageStatus,
    /// Human-readable title.
    pub title: String,
    /// Stage detail.
    pub detail: String,
    /// Event or memory identifiers supporting this stage.
    pub evidence_ids: Vec<String>,
}

/// Sleep stage status.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SleepStageStatus {
    /// The stage has work ready for review.
    Ready,
    /// The stage found no immediate work.
    Clear,
}

/// Episode replayed by Sleep Mode.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SleepEpisodeReplay {
    /// Stable episode identifier.
    pub id: String,
    /// Event identifier that created this episode.
    pub event_id: String,
    /// Natural-language episode content.
    pub content: String,
    /// Episode tags.
    pub tags: Vec<String>,
    /// Explicit entity mentions.
    pub mentions: Vec<String>,
    /// Source record that produced this episode, when available.
    pub source_id: Option<String>,
    /// Event timestamp in milliseconds since the Unix epoch.
    pub created_at_ms: u64,
}

/// Candidate consolidation work produced by Sleep Mode.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SleepConsolidationCandidate {
    /// Stable candidate identifier within this report.
    pub id: String,
    /// Candidate category.
    pub kind: SleepConsolidationCandidateKind,
    /// Candidate priority.
    pub priority: SelfInspectionReviewPriority,
    /// Proposed operator action.
    pub action: SelfInspectionReviewAction,
    /// Human-readable title.
    pub title: String,
    /// Why this candidate matters.
    pub rationale: String,
    /// Event or memory identifiers supporting the candidate.
    pub evidence_ids: Vec<String>,
}

/// Candidate category for Sleep Mode consolidation work.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SleepConsolidationCandidateKind {
    /// Candidate derived from a grouped reflection cycle.
    ReflectionCycle,
    /// Candidate derived from a self-inspection review item.
    ReviewItem,
    /// Repeated episode tag suggests a pattern worth reviewing.
    RepeatedEpisodeTag,
    /// Repeated mention suggests an entity worth linking or summarizing.
    RepeatedMention,
    /// Source or evidence coverage needs operator attention.
    SourceCoverageGap,
}

pub(crate) fn sleep_mode(data: &MemoryData, options: SleepModeOptions) -> MemorySleepReport {
    sleep_mode_at(data, options, now_ms())
}

pub(crate) fn sleep_mode_at(
    data: &MemoryData,
    options: SleepModeOptions,
    generated_at_ms: u64,
) -> MemorySleepReport {
    let recent_episode_limit = options.recent_episode_limit.max(1);
    let candidate_limit = options.candidate_limit.max(1);
    let reflection = reflection::reflect(data, options.reflection);
    let self_inspection = self_inspection::self_inspect_at(data, generated_at_ms);
    let recent_episodes = recent_episodes(data, recent_episode_limit);
    let mut consolidation_candidates =
        consolidation_candidates(data, &reflection, &self_inspection, candidate_limit);
    let stages = sleep_stages(
        data,
        &reflection,
        &self_inspection,
        &recent_episodes,
        &consolidation_candidates,
    );
    let summary = SleepModeSummary {
        replayed_episode_count: recent_episodes.len(),
        finding_count: self_inspection.findings.len(),
        reflection_cycle_count: reflection.cycles.len(),
        consolidation_candidate_count: consolidation_candidates.len(),
        review_item_count: self_inspection.review_queue.len(),
        pending_stage_count: stages
            .iter()
            .filter(|stage| stage.status == SleepStageStatus::Ready)
            .count(),
        automatic_write_back: self_inspection.write_back_policy.automatic_write_back,
    };

    consolidation_candidates.truncate(candidate_limit);

    MemorySleepReport {
        version: MEMORY_SLEEP_REPORT_VERSION,
        generated_at_ms,
        event_count: data.event_count,
        authority: self_inspection.authority.clone(),
        health: self_inspection.health.clone(),
        summary,
        stages,
        recent_episodes,
        consolidation_candidates,
        review_items: self_inspection.review_queue.clone(),
        reflection,
        self_inspection: self_inspection.clone(),
        write_back_policy: self_inspection.write_back_policy,
    }
}

fn recent_episodes(data: &MemoryData, limit: usize) -> Vec<SleepEpisodeReplay> {
    let mut episodes = data.episodes.clone();
    episodes.sort_by(|left, right| {
        right
            .created_at_ms
            .cmp(&left.created_at_ms)
            .then_with(|| left.id.cmp(&right.id))
    });

    episodes
        .into_iter()
        .take(limit)
        .map(|episode| SleepEpisodeReplay {
            id: episode.id,
            event_id: episode.event_id,
            content: episode.content,
            tags: episode.tags,
            mentions: episode.mentions,
            source_id: episode.source_id,
            created_at_ms: episode.created_at_ms,
        })
        .collect()
}

fn consolidation_candidates(
    data: &MemoryData,
    reflection: &MemoryReflectionReport,
    self_inspection: &SelfInspectionReport,
    candidate_limit: usize,
) -> Vec<SleepConsolidationCandidate> {
    let mut candidates = Vec::new();
    append_reflection_candidates(reflection, &mut candidates);
    append_review_candidates(self_inspection, &mut candidates);
    append_repeated_tag_candidates(data, &mut candidates);
    append_repeated_mention_candidates(data, &mut candidates);
    append_source_coverage_candidate(data, reflection, &mut candidates);
    dedupe_sort_and_limit(candidates, candidate_limit)
}

fn append_reflection_candidates(
    reflection: &MemoryReflectionReport,
    candidates: &mut Vec<SleepConsolidationCandidate>,
) {
    for cycle in &reflection.cycles {
        candidates.push(SleepConsolidationCandidate {
            id: format!("sleep_reflection_{}", cycle.id),
            kind: SleepConsolidationCandidateKind::ReflectionCycle,
            priority: cycle.priority.clone(),
            action: cycle.action.clone(),
            title: cycle.title.clone(),
            rationale: cycle.rationale.clone(),
            evidence_ids: cycle.evidence_ids.clone(),
        });
    }
}

fn append_review_candidates(
    self_inspection: &SelfInspectionReport,
    candidates: &mut Vec<SleepConsolidationCandidate>,
) {
    for item in &self_inspection.review_queue {
        candidates.push(SleepConsolidationCandidate {
            id: format!("sleep_review_{}", item.id),
            kind: SleepConsolidationCandidateKind::ReviewItem,
            priority: item.priority.clone(),
            action: item.action.clone(),
            title: item.title.clone(),
            rationale: item.detail.clone(),
            evidence_ids: item.evidence_ids.clone(),
        });
    }
}

fn append_repeated_tag_candidates(
    data: &MemoryData,
    candidates: &mut Vec<SleepConsolidationCandidate>,
) {
    let mut tags = BTreeMap::<String, Vec<String>>::new();
    for episode in &data.episodes {
        for tag in &episode.tags {
            let normalized = normalize_key(tag);
            if normalized.is_empty() {
                continue;
            }
            tags.entry(normalized).or_default().push(episode.id.clone());
        }
    }

    for (tag, evidence_ids) in tags {
        if evidence_ids.len() < 2 {
            continue;
        }
        candidates.push(SleepConsolidationCandidate {
            id: format!("sleep_repeated_tag_{}", slug(&tag)),
            kind: SleepConsolidationCandidateKind::RepeatedEpisodeTag,
            priority: SelfInspectionReviewPriority::Medium,
            action: SelfInspectionReviewAction::ConsolidatePattern,
            title: format!("Repeated episode tag: {tag}"),
            rationale: format!(
                "Tag '{tag}' appears in {} episodes and may deserve operator-reviewed consolidation.",
                evidence_ids.len()
            ),
            evidence_ids,
        });
    }
}

fn append_repeated_mention_candidates(
    data: &MemoryData,
    candidates: &mut Vec<SleepConsolidationCandidate>,
) {
    let mut mentions = BTreeMap::<String, Vec<String>>::new();
    for episode in &data.episodes {
        for mention in &episode.mentions {
            let normalized = normalize_key(mention);
            if normalized.is_empty() {
                continue;
            }
            mentions
                .entry(normalized)
                .or_default()
                .push(episode.id.clone());
        }
    }

    for (mention, evidence_ids) in mentions {
        if evidence_ids.len() < 2 {
            continue;
        }
        candidates.push(SleepConsolidationCandidate {
            id: format!("sleep_repeated_mention_{}", slug(&mention)),
            kind: SleepConsolidationCandidateKind::RepeatedMention,
            priority: SelfInspectionReviewPriority::Low,
            action: SelfInspectionReviewAction::LinkMemory,
            title: format!("Repeated mention: {mention}"),
            rationale: format!(
                "Mention '{mention}' appears in {} episodes and may need stronger graph context.",
                evidence_ids.len()
            ),
            evidence_ids,
        });
    }
}

fn append_source_coverage_candidate(
    data: &MemoryData,
    reflection: &MemoryReflectionReport,
    candidates: &mut Vec<SleepConsolidationCandidate>,
) {
    let coverage = &reflection.source_coverage;
    if coverage.unsourced_episode_count == 0 && coverage.unsupported_memory_count == 0 {
        return;
    }

    let mut evidence_ids = data
        .episodes
        .iter()
        .filter(|episode| episode.source_id.is_none())
        .map(|episode| episode.id.clone())
        .collect::<Vec<_>>();
    evidence_ids.truncate(8);

    let priority = if coverage.unsupported_memory_count > 0 {
        SelfInspectionReviewPriority::High
    } else {
        SelfInspectionReviewPriority::Medium
    };
    candidates.push(SleepConsolidationCandidate {
        id: "sleep_source_coverage_gap".to_string(),
        kind: SleepConsolidationCandidateKind::SourceCoverageGap,
        priority,
        action: SelfInspectionReviewAction::CaptureEvidence,
        title: "Source and evidence coverage gap".to_string(),
        rationale: format!(
            "{} unsourced episode(s) and {} unsupported derived item(s) need evidence review.",
            coverage.unsourced_episode_count, coverage.unsupported_memory_count
        ),
        evidence_ids,
    });
}

fn dedupe_sort_and_limit(
    candidates: Vec<SleepConsolidationCandidate>,
    limit: usize,
) -> Vec<SleepConsolidationCandidate> {
    let mut seen = BTreeSet::new();
    let mut deduped = candidates
        .into_iter()
        .filter(|candidate| seen.insert(candidate.id.clone()))
        .collect::<Vec<_>>();
    deduped.sort_by(|left, right| {
        priority_rank(&left.priority)
            .cmp(&priority_rank(&right.priority))
            .then_with(|| kind_rank(&left.kind).cmp(&kind_rank(&right.kind)))
            .then_with(|| left.id.cmp(&right.id))
    });
    deduped.truncate(limit);
    deduped
}

fn sleep_stages(
    data: &MemoryData,
    reflection: &MemoryReflectionReport,
    self_inspection: &SelfInspectionReport,
    recent_episodes: &[SleepEpisodeReplay],
    candidates: &[SleepConsolidationCandidate],
) -> Vec<SleepStage> {
    vec![
        SleepStage {
            id: "replay_recent_episodes".to_string(),
            status: if recent_episodes.is_empty() {
                SleepStageStatus::Clear
            } else {
                SleepStageStatus::Ready
            },
            title: "Replay recent episodes".to_string(),
            detail: format!(
                "Replayed {} of {} projected episode(s).",
                recent_episodes.len(),
                data.episodes.len()
            ),
            evidence_ids: recent_episodes
                .iter()
                .map(|episode| episode.id.clone())
                .collect(),
        },
        SleepStage {
            id: "inspect_memory_health".to_string(),
            status: if self_inspection.findings.is_empty() {
                SleepStageStatus::Clear
            } else {
                SleepStageStatus::Ready
            },
            title: "Inspect memory health".to_string(),
            detail: format!(
                "Found {} self-inspection finding(s).",
                self_inspection.findings.len()
            ),
            evidence_ids: self_inspection
                .findings
                .iter()
                .flat_map(|finding| finding.evidence_ids.iter().cloned())
                .collect(),
        },
        SleepStage {
            id: "plan_consolidation".to_string(),
            status: if candidates.is_empty() {
                SleepStageStatus::Clear
            } else {
                SleepStageStatus::Ready
            },
            title: "Plan consolidation".to_string(),
            detail: format!(
                "Prepared {} candidate(s) across {} reflection cycle(s).",
                candidates.len(),
                reflection.cycles.len()
            ),
            evidence_ids: candidates
                .iter()
                .flat_map(|candidate| candidate.evidence_ids.iter().cloned())
                .collect(),
        },
        SleepStage {
            id: "queue_operator_review".to_string(),
            status: if self_inspection.review_queue.is_empty() {
                SleepStageStatus::Clear
            } else {
                SleepStageStatus::Ready
            },
            title: "Queue operator review".to_string(),
            detail: format!(
                "{} proposed review item(s) require explicit approval before write-back.",
                self_inspection.review_queue.len()
            ),
            evidence_ids: self_inspection
                .review_queue
                .iter()
                .flat_map(|item| item.evidence_ids.iter().cloned())
                .collect(),
        },
    ]
}

fn priority_rank(priority: &SelfInspectionReviewPriority) -> u8 {
    match priority {
        SelfInspectionReviewPriority::Critical => 0,
        SelfInspectionReviewPriority::High => 1,
        SelfInspectionReviewPriority::Medium => 2,
        SelfInspectionReviewPriority::Low => 3,
    }
}

fn kind_rank(kind: &SleepConsolidationCandidateKind) -> u8 {
    match kind {
        SleepConsolidationCandidateKind::ReflectionCycle => 0,
        SleepConsolidationCandidateKind::ReviewItem => 1,
        SleepConsolidationCandidateKind::SourceCoverageGap => 2,
        SleepConsolidationCandidateKind::RepeatedEpisodeTag => 3,
        SleepConsolidationCandidateKind::RepeatedMention => 4,
    }
}

fn normalize_key(input: &str) -> String {
    input.trim().to_ascii_lowercase()
}

fn slug(input: &str) -> String {
    input
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use crate::{Claim, Episode, MemoryData, SelfInspectionReviewPriority};

    use super::{
        MEMORY_SLEEP_REPORT_VERSION, SleepConsolidationCandidateKind, SleepModeOptions,
        SleepStageStatus, sleep_mode_at,
    };

    #[test]
    fn sleep_mode_replays_recent_episodes_and_plans_consolidation() {
        let data = data_with_repeated_patterns();

        let report = sleep_mode_at(
            &data,
            SleepModeOptions {
                recent_episode_limit: 2,
                candidate_limit: 8,
                ..SleepModeOptions::default()
            },
            123,
        );

        assert_eq!(report.version, MEMORY_SLEEP_REPORT_VERSION);
        assert_eq!(report.generated_at_ms, 123);
        assert_eq!(report.recent_episodes.len(), 2);
        assert_eq!(report.recent_episodes[0].id, "episode_3");
        assert!(report.consolidation_candidates.iter().any(|candidate| {
            candidate.kind == SleepConsolidationCandidateKind::RepeatedEpisodeTag
                && candidate.evidence_ids.len() == 3
        }));
        assert!(report.stages.iter().any(
            |stage| stage.id == "plan_consolidation" && stage.status == SleepStageStatus::Ready
        ));
        assert!(!report.write_back_policy.automatic_write_back);
    }

    #[test]
    fn sleep_mode_surfaces_review_items_without_mutating() {
        let data = data_with_unsupported_claim();

        let report = sleep_mode_at(&data, SleepModeOptions::default(), 123);

        assert!(report.summary.review_item_count >= 1);
        assert!(
            report
                .review_items
                .iter()
                .any(|item| item.priority == SelfInspectionReviewPriority::High)
        );
        assert!(
            report
                .consolidation_candidates
                .iter()
                .any(|candidate| candidate.kind == SleepConsolidationCandidateKind::ReviewItem)
        );
        assert!(!report.summary.automatic_write_back);
    }

    fn data_with_repeated_patterns() -> MemoryData {
        MemoryData {
            event_count: 3,
            last_event_id: Some("event_3".to_string()),
            episodes: vec![
                episode("episode_1", "event_1", 1),
                episode("episode_2", "event_2", 2),
                episode("episode_3", "event_3", 3),
            ],
            ..MemoryData::default()
        }
    }

    fn data_with_unsupported_claim() -> MemoryData {
        MemoryData {
            event_count: 1,
            last_event_id: Some("event_1".to_string()),
            claims: vec![Claim {
                id: "claim_1".to_string(),
                event_id: "event_1".to_string(),
                subject: "Lena".to_string(),
                predicate: "owns".to_string(),
                object: "release notes".to_string(),
                source_episode_id: None,
                confidence: 0.8,
                scope: None,
                created_at_ms: 1,
            }],
            ..MemoryData::default()
        }
    }

    fn episode(id: &str, event_id: &str, created_at_ms: u64) -> Episode {
        Episode {
            id: id.to_string(),
            event_id: event_id.to_string(),
            content: "Lena discussed release notes.".to_string(),
            tags: vec!["product".to_string()],
            mentions: vec!["Lena".to_string()],
            source_id: None,
            source_position: None,
            source_role: None,
            scope: None,
            created_at_ms,
        }
    }
}
