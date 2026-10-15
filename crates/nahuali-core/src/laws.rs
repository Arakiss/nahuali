//! Machine-checked invariants of the memory engine.
//!
//! `LAWS.md` states each law in plain language, its verification bounds, and
//! the Kani harness or property test behind it. Every law is written once as a
//! `check_*` function; Kani drives it with exhaustive symbolic inputs inside a
//! small bound, and proptest drives the same check with larger random inputs.

use std::collections::BTreeMap;

use crate::{
    error::Result,
    event::{
        EpisodeRecorded, EventEnvelope, FactAsserted, IntentionUpdated, MemoryEvent,
        RedactedRecord, RepairApplied, RepairClaimMaterialization, RepairMaterialization,
        ReviewRecorded, ReviewRecordedAction, ReviewRecordedOutcome, SourceRecorded,
        SourceRecordedKind,
    },
    forget::{ForgetOptions, ForgetPlan, derived_from, memory_ids},
    model::{
        Claim, ClaimLifecycle, ClaimStatus, MemoryKind, MemoryScope, MemoryScopeKind, RecallResult,
    },
    recall::{Candidate, is_superseded},
    self_repair::{AutonomyLevel, RepairKind},
};

/// Law 3: ranking is a permutation that never places a superseded claim ahead
/// of current memory.
fn check_superseded_ranked_last(input_ids: &[&str], ranked: &[RecallResult]) {
    assert!(ranked.len() == input_ids.len());
    for id in input_ids {
        assert!(ranked.iter().filter(|result| result.id == *id).count() == 1);
    }
    for (position, earlier) in ranked.iter().enumerate() {
        for later in &ranked[position + 1..] {
            assert!(!(is_superseded(earlier) && !is_superseded(later)));
        }
    }
}

/// Law 4: a superseded claim always names a current, strictly newer,
/// evidence-backed replacement for the same scope, subject, and predicate.
fn check_supersession_points_to_current(
    claims: &[Claim],
    lifecycles: &BTreeMap<String, ClaimLifecycle>,
) {
    assert!(lifecycles.len() == claims.len());
    for retired in claims {
        let lifecycle = &lifecycles[&retired.id];
        let Some(winner_id) = lifecycle.superseded_by.as_deref() else {
            assert!(lifecycle.status != ClaimStatus::Superseded);
            assert!(lifecycle.superseded_at_ms.is_none());
            continue;
        };
        assert!(lifecycle.status == ClaimStatus::Superseded);
        let winner = claims
            .iter()
            .find(|claim| claim.id == winner_id)
            .expect("superseding claim exists");
        let winner_lifecycle = &lifecycles[winner_id];
        assert!(winner_lifecycle.status == ClaimStatus::Current);
        assert!(winner_lifecycle.supersedes.contains(&retired.id));
        assert!(lifecycle.superseded_at_ms == Some(winner.created_at_ms));
        assert!(winner.scope == retired.scope);
        assert!(winner.subject == retired.subject && winner.predicate == retired.predicate);
        assert!(winner.object != retired.object);
        assert!(winner.created_at_ms > retired.created_at_ms);
        assert!(winner.source_episode_id.is_some());
        assert!(winner.source_episode_id != retired.source_episode_id);
    }
}

/// Law 5: a forget plan erases every target, leaves no surviving record citing
/// erased memory or an erased record, never redacts the audit trail, and names
/// exactly the memory it erases.
fn check_forget_closure(
    events: &[EventEnvelope],
    options: &ForgetOptions,
    planned: Result<ForgetPlan>,
) {
    let live_ids = events
        .iter()
        .flat_map(|event| memory_ids(&event.payload))
        .collect::<Vec<_>>();
    let requested = options
        .targets
        .iter()
        .map(|target| target.trim())
        .filter(|target| !target.is_empty())
        .collect::<Vec<_>>();
    let satisfiable = !options.reason.trim().is_empty()
        && !requested.is_empty()
        && requested.iter().all(|target| live_ids.contains(target));

    let plan = match planned {
        Ok(plan) => plan,
        Err(error) => {
            // Under Kani, dropping a `NahualiError` pulls io and database error
            // drop glue into the verified program; leaking it keeps it out.
            std::mem::forget(error);
            assert!(!satisfiable);
            return;
        }
    };
    assert!(satisfiable);

    let forgotten = |id: &str| plan.forgotten_ids.iter().any(|forgotten| forgotten == id);
    let redacted_event = |id: &str| {
        plan.redacted_indexes
            .iter()
            .any(|&index| events[index].id == id)
    };

    for target in &requested {
        assert!(forgotten(target));
    }
    for (index, event) in events.iter().enumerate() {
        let ids = memory_ids(&event.payload);
        if plan.redacted_indexes.contains(&index) {
            assert!(!ids.is_empty());
            continue;
        }
        assert!(ids.iter().all(|id| !forgotten(id)));
        assert!(
            derived_from(&event.payload)
                .iter()
                .all(|parent| !forgotten(parent) && !redacted_event(parent))
        );
    }
    for id in &plan.forgotten_ids {
        assert!(
            plan.redacted_indexes
                .iter()
                .any(|&index| memory_ids(&events[index].payload).contains(&id.as_str()))
        );
    }
}

fn claim(
    id: &str,
    event_id: &str,
    subject: &str,
    object: &str,
    source_episode_id: Option<&str>,
    scope_key: Option<&str>,
    created_at_ms: u64,
) -> Claim {
    Claim {
        id: id.to_string(),
        event_id: event_id.to_string(),
        subject: subject.to_string(),
        predicate: "p".to_string(),
        object: object.to_string(),
        source_episode_id: source_episode_id.map(str::to_string),
        confidence: 0.5,
        scope: scope_key.map(|key| MemoryScope {
            kind: MemoryScopeKind::Project,
            name: key.to_string(),
            key: key.to_string(),
        }),
        created_at_ms,
    }
}

fn lifecycle(choice: u8) -> Option<ClaimLifecycle> {
    let status = match choice % 4 {
        0 => return None,
        1 => ClaimStatus::Current,
        2 => ClaimStatus::Superseded,
        _ => ClaimStatus::Contested,
    };
    Some(ClaimLifecycle {
        status,
        ..ClaimLifecycle::current()
    })
}

fn recall_result(id: &str) -> RecallResult {
    RecallResult {
        kind: MemoryKind::Claim,
        id: id.to_string(),
        score: 0.0,
        excerpt: String::new(),
        evidence_id: None,
        matched_terms: Vec::new(),
        scope: None,
        trust: None,
        created_at_ms: None,
        updated_at_ms: None,
        observed_at_ms: None,
        confidence: None,
        lifecycle: None,
        score_breakdown: None,
    }
}

/// Number of record shapes [`record`] can build.
const RECORD_KINDS: u8 = 7;

/// One ledger record of shape `kind`, carrying memory `id` and citing `parent`.
/// Checksums are left empty: forget planning reads only ids and citations.
fn record(
    sequence: u64,
    event_id: &str,
    repair_id: &str,
    kind: u8,
    id: &str,
    parent: Option<&str>,
) -> EventEnvelope {
    let id = id.to_string();
    let parent = parent.map(str::to_string);
    let payload = match kind % RECORD_KINDS {
        0 => MemoryEvent::SourceRecorded(SourceRecorded {
            id,
            kind: SourceRecordedKind::Note,
            title: None,
            uri: None,
            content_checksum: String::new(),
            byte_len: 0,
            metadata: Default::default(),
            scope: None,
        }),
        1 => MemoryEvent::EpisodeRecorded(EpisodeRecorded {
            id,
            content: String::new(),
            tags: Vec::new(),
            mentions: Vec::new(),
            source_id: parent,
            source_position: None,
            source_role: None,
            scope: None,
        }),
        2 => MemoryEvent::FactAsserted(FactAsserted {
            id,
            subject: String::new(),
            predicate: String::new(),
            object: String::new(),
            source_episode_id: parent,
            confidence: 0.5,
            scope: None,
        }),
        3 => MemoryEvent::ReviewRecorded(ReviewRecorded {
            id,
            review_id: String::new(),
            finding_id: String::new(),
            action: ReviewRecordedAction::CaptureEvidence,
            outcome: ReviewRecordedOutcome::Resolved,
            note: String::new(),
            evidence_ids: parent.into_iter().collect(),
            scope: None,
        }),
        4 => MemoryEvent::IntentionUpdated(IntentionUpdated {
            id,
            description: None,
            priority: None,
            deadline_at_ms: None,
            depends_on: None,
            goal_id: None,
            progress_percent: None,
        }),
        5 => MemoryEvent::RepairApplied(RepairApplied {
            id: repair_id.to_string(),
            kind: RepairKind::ConsolidateClaim,
            evidence_ids: parent.into_iter().collect(),
            proposed_by: String::new(),
            rationale: String::new(),
            autonomy_level: AutonomyLevel::Auto,
            operator_override: false,
            materialized: RepairMaterialization::Claim(RepairClaimMaterialization {
                id,
                subject: String::new(),
                predicate: String::new(),
                object: String::new(),
                source_episode_id: None,
                confidence: 0.5,
                scope: None,
            }),
        }),
        _ => MemoryEvent::Redacted(RedactedRecord {
            forget_id: String::new(),
            kind: String::new(),
            chain_hash: None,
        }),
    };
    EventEnvelope {
        version: 2,
        id: event_id.to_string(),
        sequence,
        timestamp_ms: 0,
        checksum: String::new(),
        prev_hash: None,
        payload,
    }
}

fn candidate(
    relevance: f32,
    confidence: Option<f32>,
    has_evidence: bool,
    created_at_ms: u64,
    updated_at_ms: Option<u64>,
    lifecycle: Option<ClaimLifecycle>,
) -> Candidate {
    Candidate {
        relevance,
        confidence,
        has_evidence,
        created_at_ms,
        updated_at_ms,
        lifecycle,
    }
}

#[cfg(kani)]
mod proofs {
    use super::{
        RECORD_KINDS, candidate, check_forget_closure, check_superseded_ranked_last,
        check_supersession_points_to_current, claim, lifecycle, recall_result, record,
    };
    use crate::{
        forget::{ForgetOptions, plan_forget},
        inspection::claim_lifecycles,
        model::{MemoryData, RecallScoreBreakdown, admitted_confidence},
        recall::{RecallRankingPolicy, rank, score_breakdown},
    };

    fn pick<T: Copy>(options: &[T]) -> T {
        let index: usize = kani::any();
        kani::assume(index < options.len());
        options[index]
    }

    /// Any policy `RecallRankingPolicy::validate` accepts. The rule is mirrored
    /// inline because the rejection path formats floats, which Kani cannot
    /// model cheaply; `validate_agrees_with_the_kani_policy_domain` pins the
    /// two together.
    fn any_valid_policy() -> RecallRankingPolicy {
        let policy = RecallRankingPolicy {
            confidence_weight: kani::any(),
            evidence_bonus: kani::any(),
            recency_weight: kani::any(),
            recency_half_life_ms: kani::any(),
        };
        kani::assume(super::policy_in_kani_domain(&policy));
        policy
    }

    fn any_relevance() -> f32 {
        let relevance: f32 = kani::any();
        kani::assume(relevance.is_finite() && relevance >= 0.0);
        relevance
    }

    #[kani::proof]
    fn law_confidence_admission() {
        let value: f32 = kani::any();
        match admitted_confidence(value) {
            Some(admitted) => {
                assert!(!value.is_nan());
                assert!(admitted.is_finite());
                assert!((0.0..=1.0).contains(&admitted));
                if (0.0..=1.0).contains(&value) {
                    assert!(admitted == value);
                }
            }
            None => assert!(value.is_nan()),
        }
    }

    fn any_recency(policy: &RecallRankingPolicy) -> f32 {
        let recency: f32 = kani::any();
        kani::assume(0.0 <= recency && recency <= policy.recency_weight);
        recency
    }

    #[kani::proof]
    fn law_score_components_bounded() {
        let policy = any_valid_policy();
        let relevance = any_relevance();
        let recency = any_recency(&policy);
        let has_evidence: bool = kani::any();
        let confidence: Option<f32> = kani::any();

        let breakdown = score_breakdown(
            &policy,
            &candidate(relevance, confidence, has_evidence, 0, None, None),
            recency,
        );

        assert!(breakdown.relevance == relevance && breakdown.recency == recency);
        assert!(0.0 <= breakdown.confidence && breakdown.confidence <= policy.confidence_weight);
        let evidence = if has_evidence {
            policy.evidence_bonus
        } else {
            0.0
        };
        assert!(breakdown.evidence == evidence);
        if confidence.is_none_or(f32::is_nan) {
            assert!(breakdown.confidence == 0.0);
        }
    }

    #[kani::proof]
    fn law_score_confidence_component_monotone() {
        let policy = any_valid_policy();
        let lower: f32 = kani::any();
        let higher: f32 = kani::any();
        kani::assume(!lower.is_nan() && !higher.is_nan() && lower <= higher);
        let component = |confidence| {
            score_breakdown(
                &policy,
                &candidate(0.0, Some(confidence), false, 0, None, None),
                0.0,
            )
            .confidence
        };

        assert!(component(lower) <= component(higher));
    }

    #[kani::proof]
    fn law_score_total_monotone_and_at_least_relevance() {
        let policy = any_valid_policy();
        let lower: f32 = kani::any();
        let higher: f32 = kani::any();
        kani::assume(0.0 <= lower && lower <= higher && higher <= policy.confidence_weight);
        let low = RecallScoreBreakdown {
            relevance: any_relevance(),
            confidence: lower,
            evidence: if kani::any() {
                policy.evidence_bonus
            } else {
                0.0
            },
            recency: any_recency(&policy),
        };
        let high = RecallScoreBreakdown {
            confidence: higher,
            ..low.clone()
        };

        assert!(low.total() >= low.relevance);
        assert!(low.total() <= high.total());
    }

    #[kani::proof]
    #[kani::unwind(5)]
    fn law_superseded_never_outranks_current() {
        const IDS: [&str; 3] = ["a", "b", "c"];
        let policy = any_valid_policy();
        let candidates = IDS
            .iter()
            .map(|id| {
                (
                    candidate(
                        any_relevance(),
                        kani::any(),
                        kani::any(),
                        kani::any(),
                        kani::any(),
                        lifecycle(kani::any()),
                    ),
                    recall_result(id),
                )
            })
            .collect();

        check_superseded_ranked_last(&IDS, &rank(candidates, &policy));
    }

    #[kani::proof]
    #[kani::unwind(4)]
    fn law_supersession_points_to_current_claim() {
        let any_claim = |id, event_id| {
            claim(
                id,
                event_id,
                "s",
                pick(&["x", "y"]),
                pick(&[None, Some("1"), Some("2")]),
                pick(&[None, Some("a")]),
                kani::any(),
            )
        };
        let claims = vec![
            any_claim("0", "e0"),
            any_claim("1", "e1"),
            any_claim("2", "e2"),
        ];
        let data = MemoryData {
            claims: claims.clone(),
            ..MemoryData::default()
        };

        check_supersession_points_to_current(&claims, &claim_lifecycles(&data));
    }

    #[kani::proof]
    #[kani::unwind(8)]
    fn law_forget_leaves_no_orphan_provenance() {
        const EVENT_IDS: [&str; 3] = ["e0", "e1", "e2"];
        const REPAIR_IDS: [&str; 3] = ["r0", "r1", "r2"];
        let any_record = |index: usize| {
            let kind: u8 = kani::any();
            kani::assume(kind < RECORD_KINDS);
            record(
                index as u64 + 1,
                EVENT_IDS[index],
                REPAIR_IDS[index],
                kind,
                pick(&["m0", "m1", "m2"]),
                pick(&[
                    None,
                    Some("m0"),
                    Some("m1"),
                    Some("m2"),
                    Some("e0"),
                    Some("e1"),
                    Some("e2"),
                ]),
            )
        };
        let events = vec![any_record(0), any_record(1), any_record(2)];
        let options = ForgetOptions {
            targets: vec![
                pick(&["m0", "m1", "r1", " m2 ", "x"]).to_string(),
                pick(&["m0", "m2", "r2", ""]).to_string(),
            ],
            reason: "law".to_string(),
            requested_by: None,
            dry_run: false,
        };

        check_forget_closure(&events, &options, plan_forget(&events, &options));
    }
}

/// The policy domain the Kani harnesses quantify over: exactly the policies
/// `RecallRankingPolicy::validate` accepts.
fn policy_in_kani_domain(policy: &crate::recall::RecallRankingPolicy) -> bool {
    [
        policy.confidence_weight,
        policy.evidence_bonus,
        policy.recency_weight,
    ]
    .iter()
    .all(|weight| weight.is_finite() && *weight >= 0.0)
        && policy.recency_half_life_ms > 0
}

#[cfg(test)]
mod properties {
    use std::path::PathBuf;

    use proptest::prelude::*;

    use super::{
        RECORD_KINDS, candidate, check_forget_closure, check_superseded_ranked_last,
        check_supersession_points_to_current, claim, lifecycle, policy_in_kani_domain,
        recall_result, record,
    };
    use crate::{
        MemoryEngine, NahualiError,
        event::{EventEnvelope, verify_redactions},
        forget::{ForgetOptions, plan_forget},
        inspection::claim_lifecycles,
        model::MemoryData,
        recall::{RecallRankingPolicy, rank, score_breakdown},
    };

    fn policy() -> impl Strategy<Value = RecallRankingPolicy> {
        let weight = prop_oneof![
            0.0_f32..=10.0,
            Just(0.0),
            Just(f32::MIN_POSITIVE),
            Just(1.0e-40),
            Just(f32::MAX),
        ];
        (weight.clone(), weight.clone(), weight, 1_u64..=u64::MAX).prop_map(
            |(confidence_weight, evidence_bonus, recency_weight, recency_half_life_ms)| {
                RecallRankingPolicy {
                    confidence_weight,
                    evidence_bonus,
                    recency_weight,
                    recency_half_life_ms,
                }
            },
        )
    }

    fn any_confidence() -> impl Strategy<Value = Option<f32>> {
        prop_oneof![
            Just(None),
            Just(Some(f32::NAN)),
            Just(Some(f32::INFINITY)),
            Just(Some(-1.0)),
            proptest::num::f32::ANY.prop_map(Some),
            (0.0_f32..=1.0).prop_map(Some),
        ]
    }

    #[test]
    fn validate_agrees_with_the_kani_policy_domain() {
        let weights = [0.0, 1.0, f32::MAX, -0.0, -1.0, f32::NAN, f32::INFINITY];
        for confidence_weight in weights {
            for evidence_bonus in weights {
                for recency_weight in weights {
                    for recency_half_life_ms in [0, 1, u64::MAX] {
                        let policy = RecallRankingPolicy {
                            confidence_weight,
                            evidence_bonus,
                            recency_weight,
                            recency_half_life_ms,
                        };
                        assert_eq!(policy.validate().is_ok(), policy_in_kani_domain(&policy));
                    }
                }
            }
        }
    }

    proptest! {
        #[test]
        fn law_recency_bounded_and_non_increasing_in_age(
            policy in policy(),
            younger in any::<u64>(),
            extra in any::<u64>(),
        ) {
            let older = younger.saturating_add(extra);
            let newest = policy.recency(0);
            let young = policy.recency(younger);
            let old = policy.recency(older);
            prop_assert_eq!(newest, policy.recency_weight);
            prop_assert!(0.0 <= old && old <= young && young <= policy.recency_weight);
        }

        #[test]
        fn law_score_bounded_and_monotone_in_confidence(
            policy in policy(),
            relevance in 0.0_f32..1.0e6,
            has_evidence in any::<bool>(),
            age in any::<u64>(),
            lower in any_confidence(),
            higher in any_confidence(),
        ) {
            let recency = policy.recency(age);
            let scored = |confidence| {
                score_breakdown(
                    &policy,
                    &candidate(relevance, confidence, has_evidence, 0, None, None),
                    recency,
                )
            };
            let (low, high) = (scored(lower), scored(higher));
            prop_assert!(0.0 <= low.confidence && low.confidence <= policy.confidence_weight);
            prop_assert!(low.total() >= relevance);
            let admitted = |confidence: Option<f32>| {
                confidence.and_then(crate::model::admitted_confidence).unwrap_or(0.0)
            };
            if admitted(lower) <= admitted(higher) {
                prop_assert!(low.total() <= high.total());
            }
        }

        #[test]
        fn law_superseded_never_outranks_current(
            policy in policy(),
            inputs in proptest::collection::vec(
                (
                    0.0_f32..100.0,
                    any_confidence(),
                    any::<bool>(),
                    any::<u64>(),
                    any::<Option<u64>>(),
                    any::<u8>(),
                ),
                0..64,
            ),
        ) {
            let ids = (0..inputs.len()).map(|index| format!("r{index}")).collect::<Vec<_>>();
            let candidates = inputs
                .into_iter()
                .zip(&ids)
                .map(|((relevance, confidence, evidence, created, updated, status), id)| {
                    (
                        candidate(relevance, confidence, evidence, created, updated, lifecycle(status)),
                        recall_result(id),
                    )
                })
                .collect();
            let ranked = rank(candidates, &policy);
            let id_refs = ids.iter().map(String::as_str).collect::<Vec<_>>();
            check_superseded_ranked_last(&id_refs, &ranked);
            for pair in ranked.windows(2) {
                if crate::recall::is_superseded(&pair[0]) == crate::recall::is_superseded(&pair[1]) {
                    prop_assert!(pair[0].score >= pair[1].score);
                }
            }
        }

        #[test]
        fn law_supersession_points_to_current_claim(
            inputs in proptest::collection::vec(
                (0_usize..2, 0_usize..3, 0_usize..4, 0_usize..3, 0_u64..6),
                0..12,
            ),
        ) {
            const SUBJECTS: [&str; 2] = ["lena", "petru"];
            const OBJECTS: [&str; 3] = ["x", "y", "z"];
            const EPISODES: [Option<&str>; 4] = [None, Some("episode_1"), Some("episode_2"), Some("episode_3")];
            const SCOPES: [Option<&str>; 3] = [None, Some("project:a"), Some("project:b")];
            let claims = inputs
                .iter()
                .enumerate()
                .map(|(index, &(subject, object, episode, scope, created))| {
                    claim(
                        &format!("c{index}"),
                        &format!("e{index}"),
                        SUBJECTS[subject],
                        OBJECTS[object],
                        EPISODES[episode],
                        SCOPES[scope],
                        created,
                    )
                })
                .collect::<Vec<_>>();
            let data = MemoryData { claims: claims.clone(), ..MemoryData::default() };
            check_supersession_points_to_current(&claims, &claim_lifecycles(&data));
        }

        #[test]
        fn law_forget_leaves_no_orphan_provenance(
            inputs in proptest::collection::vec(
                (0..RECORD_KINDS, 0_usize..6, proptest::option::of(0_usize..16)),
                0..10,
            ),
            targets in proptest::collection::vec(0_usize..14, 1..3),
        ) {
            const TARGETS: [&str; 14] = [
                "m0", "m1", "m2", "m3", "m4", "m5", "repair_0", "repair_3",
                " m1 ", "", "unknown", "event_0", "m2", "m0",
            ];
            let parents = (0..6)
                .map(|index| format!("m{index}"))
                .chain((0..10).map(|index| format!("event_{index}")))
                .collect::<Vec<_>>();
            let events = inputs
                .iter()
                .enumerate()
                .map(|(index, &(kind, id, parent))| {
                    record(
                        index as u64 + 1,
                        &format!("event_{index}"),
                        &format!("repair_{index}"),
                        kind,
                        &format!("m{id}"),
                        parent.map(|parent| parents[parent].as_str()),
                    )
                })
                .collect::<Vec<_>>();
            let options = ForgetOptions {
                targets: targets.iter().map(|&target| TARGETS[target].to_string()).collect(),
                reason: "law".to_string(),
                requested_by: None,
                dry_run: false,
            };
            check_forget_closure(&events, &options, plan_forget(&events, &options));
        }
    }

    /// Every earlier ledger state is an unchanged prefix of the later one: same
    /// envelopes, same chain links, and a Merkle consistency proof between the
    /// two roots. Only an authorized forget may swap a payload for a stub.
    fn check_append_only(before: &[EventEnvelope], after: &[EventEnvelope], forgot: bool) {
        assert_eq!(
            after.len(),
            before.len() + 1,
            "each operation appends one record"
        );
        for (old, new) in before.iter().zip(after) {
            assert_eq!(
                (
                    &old.id,
                    old.sequence,
                    old.timestamp_ms,
                    &old.checksum,
                    &old.prev_hash
                ),
                (
                    &new.id,
                    new.sequence,
                    new.timestamp_ms,
                    &new.checksum,
                    &new.prev_hash
                ),
            );
            assert_eq!(old.link_hash(), new.link_hash());
            if old.payload != new.payload {
                assert!(forgot && new.is_redacted() && !old.is_redacted());
            }
        }
        assert_eq!(verify_redactions(after), None);
        #[cfg(feature = "tamper-evidence")]
        {
            use crate::merkle::{
                ledger_append_only, ledger_consistency_proof, ledger_merkle_root,
                verify_merkle_consistency_proof,
            };
            assert_eq!(crate::verify_event_chain(after), None);
            if let Some(old_root) = ledger_merkle_root(before) {
                let new_root = ledger_merkle_root(after).expect("non-empty ledger has a root");
                assert!(ledger_append_only(&old_root, before.len(), after).append_only);
                let proof = ledger_consistency_proof(after, before.len())
                    .expect("prefix has a consistency proof");
                assert!(verify_merkle_consistency_proof(
                    &old_root, &new_root, &proof
                ));
            }
        }
    }

    #[derive(Clone, Debug)]
    enum Operation {
        Remember(u8),
        Claim(u8, u8),
        Forget(usize),
    }

    fn operation() -> impl Strategy<Value = Operation> {
        prop_oneof![
            any::<u8>().prop_map(Operation::Remember),
            (any::<u8>(), any::<u8>())
                .prop_map(|(subject, object)| Operation::Claim(subject, object)),
            any::<usize>().prop_map(Operation::Forget),
        ]
    }

    fn temp_store(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("nahuali_law_{name}_{}_{nanos}", std::process::id()))
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 24, ..ProptestConfig::default() })]

        #[test]
        fn law_ledger_is_append_only_even_under_forget(
            operations in proptest::collection::vec(operation(), 1..8),
        ) {
            let path = temp_store("append_only");
            let mut memory = MemoryEngine::open(&path).unwrap();
            for operation in operations {
                let before = memory.events().to_vec();
                let mut forgot = false;
                match operation {
                    Operation::Remember(word) => {
                        memory.remember(format!("Lena noted item {word}"), Vec::new()).unwrap();
                    }
                    Operation::Claim(subject, object) => {
                        let episode = memory.data().episodes.last().map(|episode| episode.id.clone());
                        memory
                            .add_claim(format!("s{}", subject % 3), "status", format!("v{object}"), episode, 0.8)
                            .unwrap();
                    }
                    Operation::Forget(choice) => {
                        let data = memory.data();
                        let ids = data
                            .episodes
                            .iter()
                            .map(|episode| episode.id.clone())
                            .chain(data.claims.iter().map(|claim| claim.id.clone()))
                            .collect::<Vec<_>>();
                        if ids.is_empty() {
                            continue;
                        }
                        let report = memory
                            .forget(&ForgetOptions {
                                targets: vec![ids[choice % ids.len()].clone()],
                                reason: "law".to_string(),
                                requested_by: None,
                                dry_run: false,
                            })
                            .unwrap();
                        forgot = true;
                        let data = memory.data();
                        let erased = |id: &str| report.forgotten_ids.iter().any(|forgotten| forgotten == id);
                        prop_assert!(data.episodes.iter().all(|episode| !erased(&episode.id)));
                        let no_orphan_claims = data.claims.iter().all(|claim| {
                            !erased(&claim.id)
                                && claim.source_episode_id.as_deref().is_none_or(|episode| !erased(episode))
                        });
                        prop_assert!(no_orphan_claims);
                    }
                }
                check_append_only(&before, memory.events(), forgot);
            }
            drop(memory);
            let _ = std::fs::remove_dir_all(&path);
            let _ = std::fs::remove_file(&path);
        }
    }

    #[cfg(feature = "tamper-evidence")]
    #[test]
    fn law_merkle_commitments_prove_every_prefix_and_reject_rewrites() {
        use crate::merkle::{
            merkle_consistency_proof, merkle_proof, merkle_root, verify_append_only,
            verify_merkle_consistency_proof, verify_merkle_proof,
        };
        let leaves = (0..33)
            .map(|index| format!("leaf-{index}"))
            .collect::<Vec<_>>();
        for size in 1..=leaves.len() {
            let tree = &leaves[..size];
            let root = merkle_root(tree).unwrap();
            for index in 0..size {
                let proof = merkle_proof(tree, index).unwrap();
                assert!(verify_merkle_proof(&tree[index], &proof, &root));
            }
            for prefix in 1..=size {
                let old_root = merkle_root(&tree[..prefix]).unwrap();
                let proof = merkle_consistency_proof(tree, prefix).unwrap();
                assert!(verify_merkle_consistency_proof(&old_root, &root, &proof));
                assert!(verify_append_only(&old_root, prefix, tree).append_only);
                for rewritten in 0..prefix {
                    let mut forged = tree.to_vec();
                    forged[rewritten].push('!');
                    let forged_root = merkle_root(&forged).unwrap();
                    let forged_proof = merkle_consistency_proof(&forged, prefix).unwrap();
                    assert!(!verify_append_only(&old_root, prefix, &forged).append_only);
                    assert!(!verify_merkle_consistency_proof(
                        &old_root,
                        &forged_root,
                        &forged_proof
                    ));
                }
            }
        }
    }

    #[test]
    fn non_finite_confidence_is_rejected_before_it_reaches_the_ledger() {
        let path = temp_store("nan_confidence");
        let mut memory = MemoryEngine::open(&path).unwrap();
        let episode = memory
            .remember("Lena owns the release notes.", Vec::new())
            .unwrap();
        let invalid = |result: crate::Result<_>| {
            matches!(result, Err(NahualiError::InvalidConfidence { .. }))
        };

        assert!(invalid(
            memory
                .add_claim("Lena", "owns", "notes", Some(episode.id.clone()), f32::NAN)
                .map(drop)
        ));
        assert!(invalid(
            memory
                .add_link("Lena", "owns", "notes", Some(episode.id.clone()), f32::NAN)
                .map(drop)
        ));
        assert!(invalid(
            memory
                .add_procedure("Ship", "Run the gate", Some(episode.id.clone()), f32::NAN)
                .map(drop)
        ));
        assert_eq!(memory.events().len(), 1);

        let claim = memory
            .add_claim("Lena", "owns", "notes", Some(episode.id), f32::INFINITY)
            .unwrap();
        assert_eq!(claim.confidence, 1.0);
        drop(memory);

        let reopened = MemoryEngine::open(&path).expect("store stays openable");
        assert_eq!(reopened.events().len(), 2);
        drop(reopened);
        let _ = std::fs::remove_dir_all(&path);
        let _ = std::fs::remove_file(&path);
    }
}
