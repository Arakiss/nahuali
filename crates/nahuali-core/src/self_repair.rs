//! Self-repair contract: an LLM *proposes* a memory repair, and this
//! deterministic engine *validates, classifies, and records* it.
//!
//! The trust kernel never calls an LLM. A [`RepairProposal`] enters as
//! structured JSON produced by an agent at the edge; everything in this module
//! is deterministic, offline, and append-only. The six contract rules:
//!
//! 1. The LLM proposes; the deterministic engine validates and records. The
//!    trust kernel uses no LLM.
//! 2. Always evidence-anchored: without a real source episode, there is no
//!    repair.
//! 3. Additive, never destructive: append-only; a bad repair is reversible by
//!    supersession.
//! 4. The repair is itself an audited event in the tamper-evident ledger.
//! 5. The trust verdict *gates* the repair (no longer only for recall).
//! 6. The core stays LLM-free, offline, deterministic; self-repair is an opt-in
//!    edge layer.
//!
//! This module owns the contract types and the deterministic autonomy
//! classifier ([`classify_autonomy`]). Validation and write-back live in the
//! engine service layer, which builds on the [`crate::event::RepairApplied`]
//! event so that a single append materializes the claim or link *and* its audit
//! atomically.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    authority::{AuthorityDecision, AuthorityMode},
    error::{NahualiError, Result},
    event::{
        RepairApplied, RepairClaimMaterialization, RepairLinkMaterialization, RepairMaterialization,
    },
    inspection::KnowledgeHealth,
    model::{Claim, Episode, MemoryData, MemoryScope},
};

/// Current self-repair report format version.
pub const SELF_REPAIR_REPORT_VERSION: u32 = 1;

/// Non-mutating policy statement included with every repair report.
pub(crate) const SELF_REPAIR_POLICY: &str = "self-repair is opt-in: an LLM proposes, the deterministic engine validates, classifies, and records. The trust kernel uses no LLM; every repair is evidence-anchored and appended as an audited, reversible event.";

/// Category of repair an LLM proposed.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RepairKind {
    /// Consolidate homogeneous episodes into a single derived claim.
    ConsolidateClaim,
    /// Link two entities that both already appear in memory.
    LinkEntities,
}

/// Deterministic autonomy level the engine assigns to a proposed repair.
///
/// The classifier — never the LLM — decides this from the current projection.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AutonomyLevel {
    /// Safe to apply with no operator approval: deterministic, evidence-anchored,
    /// and reversible.
    Auto,
    /// Requires explicit operator approval (`--approve`) because it needs
    /// judgment.
    Queue,
    /// Must never be applied automatically; a trust engine does not mask
    /// contradictions, so this is raised to the operator as a warning.
    NeverAuto,
}

/// Claim materialization carried by a [`RepairPayload::ConsolidateClaim`].
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RepairClaim {
    /// Entity or concept the consolidated assertion is about.
    pub subject: String,
    /// Relationship or attribute being asserted.
    pub predicate: String,
    /// Assertion value.
    pub object: String,
    /// Caller-provided confidence; clamped to the `0.0..=1.0` range on write.
    #[serde(default)]
    pub confidence: f32,
    /// Explicit memory context boundary, when provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<MemoryScope>,
}

/// Link materialization carried by a [`RepairPayload::LinkEntities`].
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RepairLink {
    /// Source endpoint of the relation. Must already exist in the projection.
    pub from: String,
    /// Relation label.
    pub relation: String,
    /// Target endpoint of the relation. Must already exist in the projection.
    pub to: String,
    /// Caller-provided confidence; clamped to the `0.0..=1.0` range on write.
    #[serde(default)]
    pub confidence: f32,
    /// Explicit memory context boundary, when provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<MemoryScope>,
}

/// The materializable operation an LLM proposed.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RepairPayload {
    /// Consolidate homogeneous evidence episodes into one derived claim.
    ConsolidateClaim(RepairClaim),
    /// Link two entities already present in the projection.
    LinkEntities(RepairLink),
}

/// A structured repair an LLM proposed.
///
/// This is the only thing that crosses the boundary from the LLM edge into the
/// deterministic engine. The engine never trusts it blindly: every field is
/// validated and the operation is classified before anything is recorded.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RepairProposal {
    /// The materializable operation requested.
    pub payload: RepairPayload,
    /// Source episodes that justify the repair. Must be non-empty and real;
    /// a fabricated citation is rejected, never minted into evidence-backed
    /// memory.
    pub evidence_episode_ids: Vec<String>,
    /// Identifier of the model that proposed the repair, recorded as provenance.
    pub proposed_by: String,
    /// Natural-language justification recorded with the repair.
    #[serde(default)]
    pub rationale: String,
}

impl RepairProposal {
    /// Repair category for this proposal.
    pub fn kind(&self) -> RepairKind {
        match self.payload {
            RepairPayload::ConsolidateClaim(_) => RepairKind::ConsolidateClaim,
            RepairPayload::LinkEntities(_) => RepairKind::LinkEntities,
        }
    }
}

/// Deterministic verdict for a proposed repair.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct RepairVerdict {
    /// Autonomy level the deterministic classifier assigned.
    pub autonomy_level: AutonomyLevel,
    /// Whether the engine will materialize this proposal given sufficient
    /// authority. `false` only for [`AutonomyLevel::NeverAuto`].
    pub valid: bool,
    /// Human-readable reasons for the decision.
    pub reasons: Vec<String>,
    /// When the repair is blocked, the conflicting evidence that blocked it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_by: Option<String>,
}

/// Result of planning or applying a repair proposal.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RepairReport {
    /// Report format version.
    pub version: u32,
    /// Whether this report was generated without writing a record.
    pub dry_run: bool,
    /// Whether an append-only repair event was written.
    pub applied: bool,
    /// Repair category that was handled.
    pub kind: RepairKind,
    /// Autonomy level assigned by the deterministic classifier.
    pub autonomy_level: AutonomyLevel,
    /// Full deterministic verdict.
    pub verdict: RepairVerdict,
    /// Source episodes that anchor the repair.
    pub evidence_ids: Vec<String>,
    /// Model identifier that proposed the repair.
    pub proposed_by: String,
    /// Natural-language justification.
    pub rationale: String,
    /// Whether an operator explicitly approved a queued repair.
    pub operator_override: bool,
    /// Repair-decision identifier, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repair_id: Option<String>,
    /// Event identifier appended to the record ledger, if applied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resulting_event_id: Option<String>,
    /// Identifier of the claim or link this repair materialized, if applied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materialized_id: Option<String>,
    /// Non-mutating policy statement included for agent clients.
    pub policy: String,
}

/// Classify a proposed repair into a deterministic autonomy verdict.
///
/// This is the gradient table from the self-repair contract, decided entirely
/// by the engine — never by the LLM:
///
/// | Repair | Policy |
/// |---|---|
/// | Consolidate homogeneous (tag + scope) episodes into a cited claim | [`AutonomyLevel::Auto`] |
/// | Link two entities both already present in memory | [`AutonomyLevel::Auto`] |
/// | Ambiguous consolidation (no homogeneous tag/scope) | [`AutonomyLevel::Queue`] |
/// | A claim that contradicts an existing one (same subject+predicate, different value) | [`AutonomyLevel::NeverAuto`] |
///
/// Rule 5 of the contract — the trust verdict gates the repair — is realized
/// here too: when the store as a whole cannot be trusted (its authority mode is
/// [`AuthorityMode::Block`]), an otherwise [`AutonomyLevel::Auto`] repair is
/// degraded to [`AutonomyLevel::Queue`] so a contaminated store never gets an
/// unattended write.
///
/// This function assumes the proposal is already structurally valid (real
/// evidence episodes, present link endpoints). Structural validation and write
/// gating live in the engine service layer.
pub fn classify_autonomy(data: &MemoryData, proposal: &RepairProposal) -> RepairVerdict {
    let mut reasons = Vec::new();

    let (mut level, blocked_by) = match &proposal.payload {
        RepairPayload::ConsolidateClaim(claim) => {
            if let Some(conflict) = contradicting_claim(data, claim) {
                reasons.push(format!(
                    "'{} {}' is already recorded as '{}'; a contradicting consolidation must not be applied automatically.",
                    claim.subject, claim.predicate, conflict.object
                ));
                reasons.push(
                    "Resolve the contradiction through an explicit operator review instead."
                        .to_string(),
                );
                let blocked = format!(
                    "claim {} ({} {} = '{}')",
                    conflict.id, conflict.subject, conflict.predicate, conflict.object
                );
                (AutonomyLevel::NeverAuto, Some(blocked))
            } else if evidence_shares_homogeneous_tag(data, &proposal.evidence_episode_ids) {
                reasons.push(
                    "Evidence episodes share one scope and a common tag; the consolidation is deterministic, anchored, and reversible by supersession."
                        .to_string(),
                );
                (AutonomyLevel::Auto, None)
            } else {
                reasons.push(
                    "Evidence episodes do not share a homogeneous tag and scope; the pattern needs operator judgment."
                        .to_string(),
                );
                (AutonomyLevel::Queue, None)
            }
        }
        RepairPayload::LinkEntities(link) => {
            reasons.push(format!(
                "Both endpoints ('{}', '{}') are present in memory and the link cites a real episode.",
                link.from, link.to
            ));
            (AutonomyLevel::Auto, None)
        }
    };

    // Rule 5: a store the engine cannot trust never receives an unattended
    // write. A blocking authority verdict degrades an Auto repair to Queue.
    if level == AutonomyLevel::Auto {
        let authority = AuthorityDecision::evaluate(&KnowledgeHealth::inspect(data));
        if authority.mode == AuthorityMode::Block {
            level = AutonomyLevel::Queue;
            reasons.push(
                "Store trust is currently Block; the repair is queued for explicit operator approval."
                    .to_string(),
            );
        }
    }

    RepairVerdict {
        autonomy_level: level,
        valid: level != AutonomyLevel::NeverAuto,
        reasons,
        blocked_by,
    }
}

/// Projected canonical claims, falling back to the compatibility mirror.
pub(crate) fn projected_claims(data: &MemoryData) -> &[Claim] {
    if data.claims.is_empty() {
        &data.facts
    } else {
        &data.claims
    }
}

/// Find an existing claim that the proposed claim would contradict: same scope,
/// subject, and predicate but a different object value.
fn contradicting_claim<'a>(data: &'a MemoryData, claim: &RepairClaim) -> Option<&'a Claim> {
    let scope_key = claim.scope.as_ref().map(|scope| scope.key.as_str());
    projected_claims(data).iter().find(|existing| {
        existing.scope.as_ref().map(|scope| scope.key.as_str()) == scope_key
            && existing.subject == claim.subject
            && existing.predicate == claim.predicate
            && existing.object != claim.object
    })
}

/// Whether the cited evidence episodes are homogeneous enough to consolidate
/// without operator judgment: at least two episodes, all in one scope, sharing
/// at least one common tag.
pub(crate) fn evidence_shares_homogeneous_tag(data: &MemoryData, evidence_ids: &[String]) -> bool {
    let episodes: Vec<&Episode> = evidence_ids
        .iter()
        .filter_map(|id| data.episodes.iter().find(|episode| &episode.id == id))
        .collect();
    if episodes.len() < 2 {
        return false;
    }

    let scope_keys: BTreeSet<Option<&str>> = episodes
        .iter()
        .map(|episode| episode.scope.as_ref().map(|scope| scope.key.as_str()))
        .collect();
    if scope_keys.len() != 1 {
        return false;
    }

    let mut common: Option<BTreeSet<String>> = None;
    for episode in &episodes {
        let tags: BTreeSet<String> = episode
            .tags
            .iter()
            .map(|tag| tag.trim().to_ascii_lowercase())
            .filter(|tag| !tag.is_empty())
            .collect();
        common = Some(match common {
            None => tags,
            Some(current) => current.intersection(&tags).cloned().collect(),
        });
    }
    common.map(|tags| !tags.is_empty()).unwrap_or(false)
}

/// Options that gate how a prepared repair would be written.
pub(crate) struct RepairOptions {
    /// Preview the verdict without writing a record.
    pub dry_run: bool,
    /// Whether an operator explicitly approved a queued repair.
    pub approve: bool,
}

/// A validated, classified repair ready to write (or report on a dry run).
pub(crate) struct PreparedRepair {
    /// Report describing the verdict and what would be (or was) written.
    pub report: RepairReport,
    /// The append-only event that materializes the repair and its audit.
    pub event: RepairApplied,
}

/// Validate and classify a repair proposal, returning the report and the event
/// that would materialize it — without writing anything.
///
/// Validation is deterministic and rejects structurally invalid proposals:
/// a missing model id, no evidence, a fabricated evidence episode (contract
/// rule 2), empty fields, or a link whose endpoints are not present in memory.
/// The `repair_id` and `materialized_id` are minted by the caller (the engine),
/// mirroring how review write-back receives its decision id.
pub(crate) fn prepare_repair(
    data: &MemoryData,
    proposal: RepairProposal,
    options: RepairOptions,
    repair_id: String,
    materialized_id: String,
) -> Result<PreparedRepair> {
    let proposed_by = proposal.proposed_by.trim().to_string();
    if proposed_by.is_empty() {
        return Err(NahualiError::InvalidRepairProposal {
            message: "a repair must name the model that proposed it".to_string(),
        });
    }
    let rationale = proposal.rationale.trim().to_string();

    // Evidence must be real. A fabricated citation is never minted into
    // evidence-backed memory (contract rule 2).
    let evidence_ids = clean_ids(&proposal.evidence_episode_ids);
    if evidence_ids.is_empty() {
        return Err(NahualiError::InvalidRepairProposal {
            message: "a repair must cite at least one evidence episode".to_string(),
        });
    }
    for id in &evidence_ids {
        if !data.episodes.iter().any(|episode| &episode.id == id) {
            return Err(NahualiError::UnknownSourceEpisode { id: id.clone() });
        }
    }
    let anchor_episode_id = evidence_ids[0].clone();

    let cleaned_payload = match &proposal.payload {
        RepairPayload::ConsolidateClaim(claim) => {
            let subject = claim.subject.trim().to_string();
            let predicate = claim.predicate.trim().to_string();
            let object = claim.object.trim().to_string();
            if subject.is_empty() || predicate.is_empty() || object.is_empty() {
                return Err(NahualiError::InvalidRepairProposal {
                    message: "a consolidated claim needs a subject, predicate, and object"
                        .to_string(),
                });
            }
            RepairPayload::ConsolidateClaim(RepairClaim {
                subject,
                predicate,
                object,
                confidence: claim.confidence.clamp(0.0, 1.0),
                scope: claim.scope.clone(),
            })
        }
        RepairPayload::LinkEntities(link) => {
            let from = link.from.trim().to_string();
            let relation = link.relation.trim().to_string();
            let to = link.to.trim().to_string();
            if from.is_empty() || relation.is_empty() || to.is_empty() {
                return Err(NahualiError::InvalidRepairProposal {
                    message: "a link needs a source, relation, and target".to_string(),
                });
            }
            // A link may only connect entities already present in the projection;
            // self-repair never invents a new entity.
            if !entity_present(data, &from, link.scope.as_ref()) {
                return Err(NahualiError::InvalidRepairProposal {
                    message: format!("link endpoint '{from}' is not present in memory"),
                });
            }
            if !entity_present(data, &to, link.scope.as_ref()) {
                return Err(NahualiError::InvalidRepairProposal {
                    message: format!("link endpoint '{to}' is not present in memory"),
                });
            }
            RepairPayload::LinkEntities(RepairLink {
                from,
                relation,
                to,
                confidence: link.confidence.clamp(0.0, 1.0),
                scope: link.scope.clone(),
            })
        }
    };

    let cleaned = RepairProposal {
        payload: cleaned_payload,
        evidence_episode_ids: evidence_ids.clone(),
        proposed_by: proposed_by.clone(),
        rationale: rationale.clone(),
    };

    let verdict = classify_autonomy(data, &cleaned);
    let operator_override = verdict.autonomy_level == AutonomyLevel::Queue && options.approve;
    let kind = cleaned.kind();

    // Anchor the materialized claim or link to the first cited episode so it
    // passes the same recall-trust evidence bar as any directly written memory.
    let materialized = match &cleaned.payload {
        RepairPayload::ConsolidateClaim(claim) => {
            RepairMaterialization::Claim(RepairClaimMaterialization {
                id: materialized_id,
                subject: claim.subject.clone(),
                predicate: claim.predicate.clone(),
                object: claim.object.clone(),
                source_episode_id: Some(anchor_episode_id),
                confidence: claim.confidence,
                scope: claim.scope.clone(),
            })
        }
        RepairPayload::LinkEntities(link) => {
            RepairMaterialization::Link(RepairLinkMaterialization {
                id: materialized_id,
                from: link.from.clone(),
                relation: link.relation.clone(),
                to: link.to.clone(),
                source_episode_id: Some(anchor_episode_id),
                confidence: link.confidence,
                scope: link.scope.clone(),
            })
        }
    };

    let event = RepairApplied {
        id: repair_id.clone(),
        kind,
        evidence_ids: evidence_ids.clone(),
        proposed_by: proposed_by.clone(),
        rationale: rationale.clone(),
        autonomy_level: verdict.autonomy_level,
        operator_override,
        materialized,
    };

    let report = RepairReport {
        version: SELF_REPAIR_REPORT_VERSION,
        dry_run: options.dry_run,
        applied: false,
        kind,
        autonomy_level: verdict.autonomy_level,
        verdict,
        evidence_ids,
        proposed_by,
        rationale,
        operator_override,
        repair_id: Some(repair_id),
        resulting_event_id: None,
        materialized_id: None,
        policy: SELF_REPAIR_POLICY.to_string(),
    };

    Ok(PreparedRepair { report, event })
}

/// Whether an entity with this name (and scope) is present in the projection.
fn entity_present(data: &MemoryData, name: &str, scope: Option<&MemoryScope>) -> bool {
    let target = normalized_name(name);
    if target.is_empty() {
        return false;
    }
    let scope_key = scope.map(|scope| scope.key.as_str());
    data.entities.iter().any(|entity| {
        normalized_name(&entity.name) == target
            && entity.scope.as_ref().map(|scope| scope.key.as_str()) == scope_key
    })
}

/// Normalize an entity name exactly like the projection: collapse internal
/// whitespace and lowercase, so presence checks match the stored entity key.
fn normalized_name(name: &str) -> String {
    name.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

/// Trim, drop empty, and de-duplicate identifiers while preserving order.
fn clean_ids(ids: &[String]) -> Vec<String> {
    let mut cleaned = Vec::new();
    for id in ids {
        let id = id.trim().to_string();
        if !id.is_empty() && !cleaned.contains(&id) {
            cleaned.push(id);
        }
    }
    cleaned
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Claim, Episode, MemoryScope, MemoryScopeKind};

    fn episode(id: &str, tags: &[&str], scope: Option<MemoryScope>) -> Episode {
        Episode {
            id: id.to_string(),
            event_id: format!("event_{id}"),
            content: format!("content for {id}"),
            tags: tags.iter().map(|tag| tag.to_string()).collect(),
            mentions: Vec::new(),
            source_id: None,
            source_position: None,
            source_role: None,
            scope,
            created_at_ms: 1000,
        }
    }

    fn claim(id: &str, subject: &str, predicate: &str, object: &str) -> Claim {
        Claim {
            id: id.to_string(),
            event_id: format!("event_{id}"),
            subject: subject.to_string(),
            predicate: predicate.to_string(),
            object: object.to_string(),
            source_episode_id: Some("episode_1".to_string()),
            confidence: 0.9,
            scope: None,
            created_at_ms: 1000,
        }
    }

    fn consolidate(
        subject: &str,
        predicate: &str,
        object: &str,
        evidence: &[&str],
    ) -> RepairProposal {
        RepairProposal {
            payload: RepairPayload::ConsolidateClaim(RepairClaim {
                subject: subject.to_string(),
                predicate: predicate.to_string(),
                object: object.to_string(),
                confidence: 0.9,
                scope: None,
            }),
            evidence_episode_ids: evidence.iter().map(|id| id.to_string()).collect(),
            proposed_by: "claude-opus-4-8".to_string(),
            rationale: "repeated observations".to_string(),
        }
    }

    #[test]
    fn homogeneous_consolidation_is_auto() {
        let data = MemoryData {
            episodes: vec![
                episode("episode_1", &["release"], None),
                episode("episode_2", &["release"], None),
            ],
            ..MemoryData::default()
        };

        let verdict = classify_autonomy(
            &data,
            &consolidate("Lena", "owns", "release notes", &["episode_1", "episode_2"]),
        );

        assert_eq!(verdict.autonomy_level, AutonomyLevel::Auto);
        assert!(verdict.valid);
        assert!(verdict.blocked_by.is_none());
    }

    #[test]
    fn ambiguous_consolidation_without_common_tag_is_queue() {
        let data = MemoryData {
            episodes: vec![
                episode("episode_1", &["release"], None),
                episode("episode_2", &["billing"], None),
            ],
            ..MemoryData::default()
        };

        let verdict = classify_autonomy(
            &data,
            &consolidate("Lena", "owns", "release notes", &["episode_1", "episode_2"]),
        );

        assert_eq!(verdict.autonomy_level, AutonomyLevel::Queue);
        assert!(verdict.valid);
    }

    #[test]
    fn single_episode_consolidation_is_queue() {
        let data = MemoryData {
            episodes: vec![episode("episode_1", &["release"], None)],
            ..MemoryData::default()
        };

        let verdict = classify_autonomy(
            &data,
            &consolidate("Lena", "owns", "release notes", &["episode_1"]),
        );

        assert_eq!(verdict.autonomy_level, AutonomyLevel::Queue);
    }

    #[test]
    fn contradicting_consolidation_is_never_auto() {
        let data = MemoryData {
            episodes: vec![
                episode("episode_1", &["release"], None),
                episode("episode_2", &["release"], None),
            ],
            claims: vec![claim("claim_1", "Lena", "owns", "the roadmap")],
            ..MemoryData::default()
        };

        let verdict = classify_autonomy(
            &data,
            &consolidate("Lena", "owns", "release notes", &["episode_1", "episode_2"]),
        );

        assert_eq!(verdict.autonomy_level, AutonomyLevel::NeverAuto);
        assert!(!verdict.valid);
        assert!(verdict.blocked_by.is_some());
    }

    #[test]
    fn same_value_consolidation_is_not_a_contradiction() {
        let data = MemoryData {
            episodes: vec![
                episode("episode_1", &["release"], None),
                episode("episode_2", &["release"], None),
            ],
            claims: vec![claim("claim_1", "Lena", "owns", "release notes")],
            ..MemoryData::default()
        };

        let verdict = classify_autonomy(
            &data,
            &consolidate("Lena", "owns", "release notes", &["episode_1", "episode_2"]),
        );

        assert_eq!(verdict.autonomy_level, AutonomyLevel::Auto);
    }

    #[test]
    fn link_entities_is_auto() {
        // A non-empty, healthy store: both endpoints co-occur in real episodes.
        let data = MemoryData {
            episodes: vec![
                episode("episode_1", &["graph"], None),
                episode("episode_2", &["graph"], None),
            ],
            ..MemoryData::default()
        };
        let proposal = RepairProposal {
            payload: RepairPayload::LinkEntities(RepairLink {
                from: "Lena".to_string(),
                relation: "owns".to_string(),
                to: "Release Notes".to_string(),
                confidence: 0.9,
                scope: None,
            }),
            evidence_episode_ids: vec!["episode_1".to_string()],
            proposed_by: "claude-opus-4-8".to_string(),
            rationale: "both entities co-occur".to_string(),
        };

        let verdict = classify_autonomy(&data, &proposal);

        assert_eq!(verdict.autonomy_level, AutonomyLevel::Auto);
    }

    #[test]
    fn auto_is_degraded_to_queue_when_store_trust_is_block() {
        // An unrelated High-severity contradiction (Lena owns X vs Y) drives the
        // store authority to Block. A homogeneous consolidation about a different
        // subject is then queued rather than applied unattended (rule 5).
        let data = MemoryData {
            episodes: vec![
                episode("episode_1", &["status"], None),
                episode("episode_2", &["status"], None),
            ],
            claims: vec![
                claim("claim_a", "Lena", "owns", "the roadmap"),
                claim("claim_b", "Lena", "owns", "the backlog"),
            ],
            ..MemoryData::default()
        };

        // Sanity: the store is in Block.
        let authority = AuthorityDecision::evaluate(&KnowledgeHealth::inspect(&data));
        assert_eq!(authority.mode, AuthorityMode::Block);

        let verdict = classify_autonomy(
            &data,
            &consolidate("Bruno", "status", "done", &["episode_1", "episode_2"]),
        );

        assert_eq!(verdict.autonomy_level, AutonomyLevel::Queue);
    }

    #[test]
    fn scoped_episodes_in_distinct_scopes_are_not_homogeneous() {
        let personal = MemoryScope::new(MemoryScopeKind::Personal, "Lena").unwrap();
        let project = MemoryScope::new(MemoryScopeKind::Project, "Nahuali").unwrap();
        let data = MemoryData {
            episodes: vec![
                episode("episode_1", &["release"], Some(personal)),
                episode("episode_2", &["release"], Some(project)),
            ],
            ..MemoryData::default()
        };

        assert!(!evidence_shares_homogeneous_tag(
            &data,
            &["episode_1".to_string(), "episode_2".to_string()]
        ));
    }
}
