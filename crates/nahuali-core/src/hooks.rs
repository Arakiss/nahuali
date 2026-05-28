use serde::{Deserialize, Serialize};

use crate::{
    AuthorityDecision, AuthorityMode, AuthorityRecall, BriefingOptions, KnowledgeHealth,
    MemoryBriefingReport, MemoryData, MemoryReflectionReport, MemorySleepReport, RecallResult,
    ReflectionOptions, SelfInspectionReport, SleepModeOptions, briefing,
    error::{NahualiError, Result},
    recall, reflection, self_inspection, sleep,
};

/// Current memory hook report format version.
pub const MEMORY_HOOK_REPORT_VERSION: u32 = 1;

/// Host execution point where Nahuali should provide deterministic memory context.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryHookKind {
    /// Start of an agent or operator session.
    SessionStart,
    /// Immediately before a user prompt or task is sent to a model.
    PrePrompt,
    /// Immediately after a tool call, action, or external step finishes.
    PostAction,
    /// End of an agent or operator session.
    SessionClose,
    /// Background consolidation pass analogous to sleep.
    SleepCycle,
}

/// Options for building a memory hook report.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct MemoryHookOptions {
    /// Hook execution point.
    pub kind: MemoryHookKind,
    /// Prompt, action summary, or host-provided context for input-sensitive hooks.
    pub input: Option<String>,
    /// Maximum lexical recall results returned for input-sensitive hooks.
    pub recall_limit: usize,
    /// Options used when a hook includes a briefing report.
    pub briefing: BriefingOptions,
    /// Options used when a hook includes a reflection report.
    pub reflection: ReflectionOptions,
    /// Options used when a hook includes a Sleep Mode report.
    pub sleep: SleepModeOptions,
}

impl Default for MemoryHookOptions {
    fn default() -> Self {
        Self {
            kind: MemoryHookKind::SessionStart,
            input: None,
            recall_limit: 10,
            briefing: BriefingOptions::default(),
            reflection: ReflectionOptions::default(),
            sleep: SleepModeOptions::default(),
        }
    }
}

/// Deterministic report returned by a memory hook invocation.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct MemoryHookReport {
    /// Report format version.
    pub version: u32,
    /// Timestamp in milliseconds when the report was generated.
    pub generated_at_ms: u64,
    /// Hook execution point.
    pub kind: MemoryHookKind,
    /// Trimmed host input used for input-sensitive hook work.
    pub input: Option<String>,
    /// Number of source events represented by the projection.
    pub event_count: usize,
    /// Projection-level authority decision.
    pub authority: AuthorityDecision,
    /// Aggregate hook counts.
    pub summary: MemoryHookSummary,
    /// Host-facing obligations derived from authority, recall, and inspection.
    pub directives: Vec<MemoryHookDirective>,
    /// Session briefing included for hooks that need continuity context.
    pub briefing: Option<MemoryBriefingReport>,
    /// Recall context included for prompt or action-sensitive hooks.
    pub recall: Option<AuthorityRecall>,
    /// Reflection report included for close or sleep hooks.
    pub reflection: Option<MemoryReflectionReport>,
    /// Self-inspection report included for close or sleep hooks.
    pub self_inspection: Option<SelfInspectionReport>,
    /// Sleep Mode report included for sleep-cycle hooks.
    pub sleep: Option<MemorySleepReport>,
}

/// Aggregate counts for a memory hook report.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct MemoryHookSummary {
    /// Number of lexical recall results returned.
    pub recall_count: usize,
    /// Number of recent episodes returned by the briefing.
    pub briefing_episode_count: usize,
    /// Number of active intentions returned by the briefing.
    pub briefing_intention_count: usize,
    /// Number of review items visible through briefing or self-inspection.
    pub review_item_count: usize,
    /// Number of reflection cycles returned.
    pub reflection_cycle_count: usize,
    /// Number of self-inspection findings returned.
    pub self_inspection_finding_count: usize,
    /// Number of Sleep Mode stages returned.
    pub sleep_stage_count: usize,
    /// Number of Sleep Mode consolidation candidates returned.
    pub sleep_candidate_count: usize,
    /// Whether hook output authorizes automatic write-back.
    pub automatic_write_back: bool,
    /// Whether hosts should pause or add uncertainty before trusting memory.
    pub should_pause_for_review: bool,
}

/// Host-facing action derived from a hook report.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct MemoryHookDirective {
    /// Stable directive identifier within this report.
    pub id: String,
    /// Directive priority.
    pub priority: MemoryHookDirectivePriority,
    /// Short title for host logs and UIs.
    pub title: String,
    /// Detailed instruction for the host or agent runtime.
    pub detail: String,
    /// Event or memory identifiers supporting the directive.
    pub evidence_ids: Vec<String>,
}

/// Priority for a hook directive.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryHookDirectivePriority {
    /// Requires immediate attention before memory should be trusted.
    Critical,
    /// High-priority host obligation.
    High,
    /// Useful host obligation for this execution point.
    Medium,
    /// Informational directive.
    Low,
}

pub(crate) fn memory_hook(
    data: &MemoryData,
    options: MemoryHookOptions,
) -> Result<MemoryHookReport> {
    memory_hook_at(data, options, now_ms())
}

pub(crate) fn memory_hook_at(
    data: &MemoryData,
    options: MemoryHookOptions,
    generated_at_ms: u64,
) -> Result<MemoryHookReport> {
    let input = normalized_input(&options)?;
    let health = KnowledgeHealth::inspect(data);
    let authority = AuthorityDecision::evaluate(&health);
    let briefing = briefing_for_hook(data, &options, generated_at_ms);
    let recall = recall_for_hook(data, &options, &input, &authority, &health);
    let self_inspection = self_inspection_for_hook(data, &options, generated_at_ms);
    let reflection = reflection_for_hook(data, &options);
    let sleep = sleep_for_hook(data, &options, generated_at_ms);
    let summary = summarize(
        &briefing,
        &recall,
        &reflection,
        &self_inspection,
        &sleep,
        &authority,
    );
    let directives = directives_for_hook(
        &options.kind,
        &authority,
        &summary,
        &input,
        recall.as_ref().map(|report| report.results.as_slice()),
        self_inspection.as_ref(),
    );

    Ok(MemoryHookReport {
        version: MEMORY_HOOK_REPORT_VERSION,
        generated_at_ms,
        kind: options.kind,
        input,
        event_count: data.event_count,
        authority,
        summary,
        directives,
        briefing,
        recall,
        reflection,
        self_inspection,
        sleep,
    })
}

fn normalized_input(options: &MemoryHookOptions) -> Result<Option<String>> {
    let input = options
        .input
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if matches!(
        options.kind,
        MemoryHookKind::PrePrompt | MemoryHookKind::PostAction
    ) && input.is_none()
    {
        return Err(NahualiError::EmptyQuery);
    }

    Ok(input.map(ToOwned::to_owned))
}

fn briefing_for_hook(
    data: &MemoryData,
    options: &MemoryHookOptions,
    generated_at_ms: u64,
) -> Option<MemoryBriefingReport> {
    if matches!(
        options.kind,
        MemoryHookKind::SessionStart | MemoryHookKind::SessionClose
    ) {
        return Some(briefing::briefing_at(
            data,
            options.briefing.clone(),
            generated_at_ms,
        ));
    }

    None
}

fn recall_for_hook(
    data: &MemoryData,
    options: &MemoryHookOptions,
    input: &Option<String>,
    authority: &AuthorityDecision,
    health: &KnowledgeHealth,
) -> Option<AuthorityRecall> {
    if !matches!(
        options.kind,
        MemoryHookKind::PrePrompt | MemoryHookKind::PostAction
    ) {
        return None;
    }

    let query = input.as_ref()?;
    let mut results = recall::recall(data, query, options.recall_limit.max(1));
    recall::attach_result_trust(data, health, &mut results);
    Some(AuthorityRecall {
        results,
        authority: authority.clone(),
        health: health.clone(),
    })
}

fn reflection_for_hook(
    data: &MemoryData,
    options: &MemoryHookOptions,
) -> Option<MemoryReflectionReport> {
    if matches!(
        options.kind,
        MemoryHookKind::SessionClose | MemoryHookKind::SleepCycle
    ) {
        return Some(reflection::reflect(data, options.reflection.clone()));
    }

    None
}

fn self_inspection_for_hook(
    data: &MemoryData,
    options: &MemoryHookOptions,
    generated_at_ms: u64,
) -> Option<SelfInspectionReport> {
    if matches!(
        options.kind,
        MemoryHookKind::SessionClose | MemoryHookKind::SleepCycle
    ) {
        return Some(self_inspection::self_inspect_at(data, generated_at_ms));
    }

    None
}

fn sleep_for_hook(
    data: &MemoryData,
    options: &MemoryHookOptions,
    generated_at_ms: u64,
) -> Option<MemorySleepReport> {
    if options.kind == MemoryHookKind::SleepCycle {
        return Some(sleep::sleep_mode_at(
            data,
            options.sleep.clone(),
            generated_at_ms,
        ));
    }

    None
}

fn summarize(
    briefing: &Option<MemoryBriefingReport>,
    recall: &Option<AuthorityRecall>,
    reflection: &Option<MemoryReflectionReport>,
    self_inspection: &Option<SelfInspectionReport>,
    sleep: &Option<MemorySleepReport>,
    authority: &AuthorityDecision,
) -> MemoryHookSummary {
    let review_item_count = briefing
        .as_ref()
        .map(|report| report.review_items.len())
        .unwrap_or_default()
        + self_inspection
            .as_ref()
            .map(|report| report.review_queue.len())
            .unwrap_or_default();
    let automatic_write_back = self_inspection
        .as_ref()
        .map(|report| report.write_back_policy.automatic_write_back)
        .unwrap_or(false);

    MemoryHookSummary {
        recall_count: recall
            .as_ref()
            .map(|report| report.results.len())
            .unwrap_or_default(),
        briefing_episode_count: briefing
            .as_ref()
            .map(|report| report.recent_episodes.len())
            .unwrap_or_default(),
        briefing_intention_count: briefing
            .as_ref()
            .map(|report| report.active_intentions.len())
            .unwrap_or_default(),
        review_item_count,
        reflection_cycle_count: reflection
            .as_ref()
            .map(|report| report.cycles.len())
            .unwrap_or_default(),
        self_inspection_finding_count: self_inspection
            .as_ref()
            .map(|report| report.findings.len())
            .unwrap_or_default(),
        sleep_stage_count: sleep
            .as_ref()
            .map(|report| report.stages.len())
            .unwrap_or_default(),
        sleep_candidate_count: sleep
            .as_ref()
            .map(|report| report.consolidation_candidates.len())
            .unwrap_or_default(),
        automatic_write_back,
        should_pause_for_review: matches!(authority.mode, AuthorityMode::Block)
            || review_item_count > 0,
    }
}

fn directives_for_hook(
    kind: &MemoryHookKind,
    authority: &AuthorityDecision,
    summary: &MemoryHookSummary,
    input: &Option<String>,
    recall_results: Option<&[RecallResult]>,
    self_inspection: Option<&SelfInspectionReport>,
) -> Vec<MemoryHookDirective> {
    let mut directives = Vec::new();
    push_authority_directive(&mut directives, authority);

    match kind {
        MemoryHookKind::SessionStart => {
            directives.push(directive(
                "session-start-briefing",
                MemoryHookDirectivePriority::High,
                "Load session memory",
                "Use the briefing before planning new work; it contains recent episodes, active intentions, review items, and graph seeds.",
                Vec::new(),
            ));
        }
        MemoryHookKind::PrePrompt => {
            push_recall_directive(&mut directives, input, recall_results);
        }
        MemoryHookKind::PostAction => {
            push_recall_directive(&mut directives, input, recall_results);
            directives.push(directive(
                "post-action-record",
                MemoryHookDirectivePriority::Medium,
                "Record durable outcomes explicitly",
                "If the action changed project state, record an episode, claim, link, procedure, or intention through an explicit memory command.",
                Vec::new(),
            ));
        }
        MemoryHookKind::SessionClose => {
            directives.push(directive(
                "session-close-review",
                MemoryHookDirectivePriority::High,
                "Close the session deliberately",
                "Review reflection cycles and record durable outcomes before ending the session; hook output never writes memory automatically.",
                review_evidence(self_inspection),
            ));
        }
        MemoryHookKind::SleepCycle => {
            directives.push(directive(
                "sleep-cycle-consolidation",
                MemoryHookDirectivePriority::High,
                "Review consolidation work",
                "Use the reflection and self-inspection reports as a consolidation queue; operator approval is required before write-back.",
                review_evidence(self_inspection),
            ));
        }
    }

    if summary.review_item_count > 0 {
        directives.push(directive(
            "operator-review-required",
            MemoryHookDirectivePriority::High,
            "Operator review is pending",
            "Resolve high-priority memory review items before treating derived memory as settled.",
            review_evidence(self_inspection),
        ));
    }

    directives
}

fn push_authority_directive(
    directives: &mut Vec<MemoryHookDirective>,
    authority: &AuthorityDecision,
) {
    let (priority, title, detail) = match authority.mode {
        AuthorityMode::Block => (
            MemoryHookDirectivePriority::Critical,
            "Memory authority is blocked",
            "Do not rely on memory without resolving the cited health signals.",
        ),
        AuthorityMode::Warn => (
            MemoryHookDirectivePriority::High,
            "Memory authority requires caution",
            "Use memory with explicit uncertainty and cite evidence when acting on it.",
        ),
        AuthorityMode::Advisory => (
            MemoryHookDirectivePriority::Medium,
            "Memory authority is advisory",
            "Memory is usable, but hosts should keep health warnings visible.",
        ),
        AuthorityMode::Certify => (
            MemoryHookDirectivePriority::Low,
            "Memory authority is certified",
            "No health signals currently require attention.",
        ),
    };

    directives.push(directive(
        "memory-authority",
        priority,
        title,
        detail,
        Vec::new(),
    ));
}

fn push_recall_directive(
    directives: &mut Vec<MemoryHookDirective>,
    input: &Option<String>,
    recall_results: Option<&[RecallResult]>,
) {
    let recall_results = recall_results.unwrap_or_default();
    if recall_results.is_empty() {
        directives.push(directive(
            "memory-recall-empty",
            MemoryHookDirectivePriority::Medium,
            "No matching memory was recalled",
            "Proceed without inventing durable facts; record new evidence explicitly if this input reveals important context.",
            Vec::new(),
        ));
        return;
    }

    let evidence_ids = recall_results
        .iter()
        .filter_map(|result| result.evidence_id.clone())
        .collect();
    directives.push(directive(
        "memory-recall-required",
        MemoryHookDirectivePriority::High,
        "Use recalled memory before responding",
        format!(
            "The hook recalled {} item(s) for '{}'; incorporate the relevant evidence before acting.",
            recall_results.len(),
            input.as_deref().unwrap_or("the host input")
        ),
        evidence_ids,
    ));
}

fn review_evidence(self_inspection: Option<&SelfInspectionReport>) -> Vec<String> {
    self_inspection
        .map(|report| {
            report
                .review_queue
                .iter()
                .flat_map(|item| item.evidence_ids.iter().cloned())
                .collect()
        })
        .unwrap_or_default()
}

fn directive(
    id: impl Into<String>,
    priority: MemoryHookDirectivePriority,
    title: impl Into<String>,
    detail: impl Into<String>,
    evidence_ids: Vec<String>,
) -> MemoryHookDirective {
    MemoryHookDirective {
        id: id.into(),
        priority,
        title: title.into(),
        detail: detail.into(),
        evidence_ids,
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use crate::{
        Episode, Intention, IntentionKind, IntentionPriority, IntentionStatus, MemoryData,
        MemoryKind, SourceDocument, SourceKind,
    };

    use super::{MEMORY_HOOK_REPORT_VERSION, MemoryHookKind, MemoryHookOptions, memory_hook_at};

    #[test]
    fn memory_hook_session_start_includes_briefing() {
        let data = data_with_episode();

        let report = memory_hook_at(
            &data,
            MemoryHookOptions {
                kind: MemoryHookKind::SessionStart,
                ..MemoryHookOptions::default()
            },
            123,
        )
        .expect("hook builds");

        assert_eq!(report.version, MEMORY_HOOK_REPORT_VERSION);
        assert_eq!(report.generated_at_ms, 123);
        assert_eq!(report.kind, MemoryHookKind::SessionStart);
        assert!(report.briefing.is_some());
        assert!(report.recall.is_none());
        assert_eq!(report.summary.briefing_episode_count, 1);
        assert_eq!(report.summary.briefing_intention_count, 1);
        assert!(!report.summary.automatic_write_back);
    }

    #[test]
    fn memory_hook_pre_prompt_recalls_input_context() {
        let data = data_with_episode();

        let report = memory_hook_at(
            &data,
            MemoryHookOptions {
                kind: MemoryHookKind::PrePrompt,
                input: Some("release notes owner".to_string()),
                recall_limit: 5,
                ..MemoryHookOptions::default()
            },
            123,
        )
        .expect("hook builds");

        let recall = report.recall.expect("recall is included");
        assert_eq!(recall.results.len(), 2);
        assert!(
            recall
                .results
                .iter()
                .any(|result| result.kind == MemoryKind::Episode)
        );
        assert_eq!(report.summary.recall_count, 2);
        assert!(
            report
                .directives
                .iter()
                .any(|directive| directive.id == "memory-recall-required")
        );
    }

    #[test]
    fn memory_hook_rejects_empty_pre_prompt_input() {
        let data = MemoryData::default();

        let error = memory_hook_at(
            &data,
            MemoryHookOptions {
                kind: MemoryHookKind::PrePrompt,
                input: Some("   ".to_string()),
                ..MemoryHookOptions::default()
            },
            123,
        )
        .expect_err("empty input is invalid");

        assert_eq!(error.to_string(), "query cannot be empty");
    }

    #[test]
    fn memory_hook_sleep_cycle_includes_non_mutating_inspection() {
        let data = data_with_episode();

        let report = memory_hook_at(
            &data,
            MemoryHookOptions {
                kind: MemoryHookKind::SleepCycle,
                ..MemoryHookOptions::default()
            },
            123,
        )
        .expect("hook builds");

        assert!(report.briefing.is_none());
        assert!(report.recall.is_none());
        assert!(report.reflection.is_some());
        assert!(report.self_inspection.is_some());
        assert!(report.sleep.is_some());
        assert_eq!(report.summary.self_inspection_finding_count, 0);
        assert_eq!(report.summary.sleep_stage_count, 4);
        assert!(!report.summary.automatic_write_back);
        assert!(
            report
                .directives
                .iter()
                .any(|directive| directive.id == "sleep-cycle-consolidation")
        );
    }

    fn data_with_episode() -> MemoryData {
        MemoryData {
            event_count: 2,
            last_event_id: Some("event_2".to_string()),
            sources: vec![SourceDocument {
                id: "source_1".to_string(),
                event_id: "source_event_1".to_string(),
                kind: SourceKind::Conversation,
                title: Some("Release review".to_string()),
                uri: None,
                content_checksum: "checksum".to_string(),
                byte_len: 42,
                metadata: Default::default(),
                scope: None,
                created_at_ms: 0,
            }],
            episodes: vec![Episode {
                id: "episode_1".to_string(),
                event_id: "event_1".to_string(),
                content: "Lena owns the release notes.".to_string(),
                tags: vec!["product".to_string()],
                mentions: vec!["Lena".to_string(), "Release Notes".to_string()],
                source_id: Some("source_1".to_string()),
                source_position: Some(1),
                source_role: None,
                scope: None,
                created_at_ms: 1,
            }],
            intentions: vec![Intention {
                id: "intention_1".to_string(),
                event_id: "event_2".to_string(),
                updated_event_id: "event_2".to_string(),
                kind: IntentionKind::Task,
                status: IntentionStatus::Active,
                priority: IntentionPriority::High,
                description: "Ship release notes".to_string(),
                source_episode_id: Some("episode_1".to_string()),
                status_reason: None,
                deadline_at_ms: None,
                depends_on: Vec::new(),
                goal_id: None,
                progress_percent: None,
                scope: None,
                created_at_ms: 2,
                updated_at_ms: 2,
            }],
            ..MemoryData::default()
        }
    }
}
