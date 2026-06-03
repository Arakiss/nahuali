//! Local, reproducible integrity-detection measurement.
//!
//! LIVR -- a Ledger Integrity Verification Rate -- is a *computed* detection
//! rate, not an asserted headline number. This test injects a fixed corpus of
//! tampering into a synthetic chained ledger, runs the real ledger validators
//! over each case, and computes `TP / (TP + FN)` for three detector tiers of
//! increasing strength:
//!
//! * `checksum-only`   -- the self-contained per-event checksum (the naive baseline);
//! * `replay-chain`    -- checksum + sequence contiguity + the hash chain;
//! * `attestation-tip` -- replay plus an externally anchored, signed tip receipt.
//!
//! Honest limits: it measures detection against a fixed synthetic injection
//! method only; a passing rate proves self-consistency detection, not the
//! absence of all tampering. The replay tier cannot catch a fully re-chained
//! suffix on its own -- that gap is exactly what the anchored-tip tier closes,
//! and the per-tier breakdown below makes the gap visible rather than averaging
//! it away.
//!
//! Gated on the `attestation` feature so all three tiers are available; the file
//! compiles to nothing on a default build.
#![cfg(feature = "attestation")]

use nahuali_core::{
    EpisodeRecorded, EventEnvelope, LedgerAttestation, MemoryEvent, sign_chain_tip,
    verify_chain_tip, verify_event_chain,
};

/// Fixed signing seed for the synthetic attestation receipt. The corpus is
/// synthetic and reproducible, so a constant seed is intentional.
const SEED_HEX: &str = "0101010101010101010101010101010101010101010101010101010101010101";
/// Number of events in the synthetic ledger.
const LEDGER_LEN: u64 = 6;
/// Zero-based index of the middle event the injectors target.
const TARGET: usize = 2;

/// A tampering pattern injected into the synthetic ledger, or the clean control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AttackClass {
    /// No tampering: the control. A tier that flags it scores a false positive.
    Clean,
    /// A middle event's payload is rewritten and its own checksum recomputed,
    /// keeping the recorded chain link -- the strongest attack on a
    /// checksum-only model.
    InPlaceRewrite,
    /// A middle event is rewritten and the whole suffix re-chained, so every
    /// link is internally consistent again.
    SuffixRechain,
    /// A middle event is dropped, leaving a sequence discontinuity.
    SequenceGap,
    /// A single event's stored checksum is corrupted.
    ChecksumMutation,
}

impl AttackClass {
    fn is_tampered(self) -> bool {
        !matches!(self, AttackClass::Clean)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DetectorTier {
    ChecksumOnly,
    ReplayChain,
    AttestationTip,
}

struct TierResult {
    tier: DetectorTier,
    true_positives: usize,
    false_negatives: usize,
    false_positives: usize,
    detection_rate: f32,
    undetected: Vec<AttackClass>,
}

const CLASSES: [AttackClass; 5] = [
    AttackClass::Clean,
    AttackClass::InPlaceRewrite,
    AttackClass::SuffixRechain,
    AttackClass::SequenceGap,
    AttackClass::ChecksumMutation,
];

const TIERS: [DetectorTier; 3] = [
    DetectorTier::ChecksumOnly,
    DetectorTier::ReplayChain,
    DetectorTier::AttestationTip,
];

/// Run the corpus through every tier and compute per-tier detection.
fn run_livr() -> Vec<TierResult> {
    let clean = clean_ledger(LEDGER_LEN);
    let receipt = attest_tip(&clean);

    TIERS
        .iter()
        .map(|&tier| {
            let mut true_positives = 0;
            let mut false_negatives = 0;
            let mut false_positives = 0;
            let mut undetected = Vec::new();
            for &class in &CLASSES {
                let ledger = inject(&clean, class);
                let flagged = detect(tier, &ledger, &receipt);
                if class.is_tampered() {
                    if flagged {
                        true_positives += 1;
                    } else {
                        false_negatives += 1;
                        undetected.push(class);
                    }
                } else if flagged {
                    false_positives += 1;
                }
            }
            let detection_rate = if true_positives + false_negatives == 0 {
                0.0
            } else {
                round2(true_positives as f32 / (true_positives + false_negatives) as f32)
            };
            TierResult {
                tier,
                true_positives,
                false_negatives,
                false_positives,
                detection_rate,
                undetected,
            }
        })
        .collect()
}

fn detect(tier: DetectorTier, events: &[EventEnvelope], receipt: &LedgerAttestation) -> bool {
    match tier {
        DetectorTier::ChecksumOnly => checksum_flags(events),
        DetectorTier::ReplayChain => replay_chain_flags(events),
        DetectorTier::AttestationTip => attestation_tip_flags(events, receipt),
    }
}

fn checksum_flags(events: &[EventEnvelope]) -> bool {
    events.iter().any(|event| !event.validate_checksum())
}

fn sequence_contiguous(events: &[EventEnvelope]) -> bool {
    events
        .iter()
        .enumerate()
        .all(|(index, event)| event.sequence == index as u64 + 1)
}

fn replay_chain_flags(events: &[EventEnvelope]) -> bool {
    checksum_flags(events) || !sequence_contiguous(events) || verify_event_chain(events).is_some()
}

fn attestation_tip_flags(events: &[EventEnvelope], receipt: &LedgerAttestation) -> bool {
    if replay_chain_flags(events) {
        return true;
    }
    // The replay tier passed: the ledger is internally consistent. The anchored
    // receipt is the only thing that can still catch a fully re-chained suffix,
    // because its tip no longer matches the signed one.
    let tip = events
        .last()
        .map(EventEnvelope::chain_hash)
        .unwrap_or_default();
    let sequence = events
        .last()
        .map(|event| event.sequence)
        .unwrap_or_default();
    !verify_chain_tip(receipt, sequence, &tip).unwrap_or(false)
}

fn clean_ledger(count: u64) -> Vec<EventEnvelope> {
    let mut events: Vec<EventEnvelope> = Vec::new();
    for sequence in 1..=count {
        let prev = events.last().map(EventEnvelope::chain_hash);
        events.push(EventEnvelope::with_chain(
            sequence,
            1000 + sequence,
            episode_event(sequence, &format!("synthetic event {sequence}")),
            prev.as_deref(),
        ));
    }
    events
}

fn attest_tip(events: &[EventEnvelope]) -> LedgerAttestation {
    let tip = events
        .last()
        .map(EventEnvelope::chain_hash)
        .unwrap_or_default();
    let sequence = events
        .last()
        .map(|event| event.sequence)
        .unwrap_or_default();
    sign_chain_tip(SEED_HEX, sequence, &tip).expect("synthetic attestation signs")
}

fn inject(clean: &[EventEnvelope], class: AttackClass) -> Vec<EventEnvelope> {
    let mut events = clean.to_vec();
    match class {
        AttackClass::Clean => {}
        AttackClass::InPlaceRewrite => {
            let original = &events[TARGET];
            // Rewrite the body and forge a valid per-event checksum (new()
            // recomputes it), preserving the recorded chain link.
            let mut forged = EventEnvelope::new(
                original.sequence,
                original.timestamp_ms,
                episode_event(original.sequence, "in-place rewrite"),
            );
            forged.prev_hash = original.prev_hash.clone();
            assert!(forged.validate_checksum(), "the forged checksum is valid");
            events[TARGET] = forged;
        }
        AttackClass::SuffixRechain => {
            let mut rechained: Vec<EventEnvelope> = Vec::new();
            for (index, event) in clean.iter().enumerate() {
                let prev = rechained.last().map(EventEnvelope::chain_hash);
                let payload = if index == TARGET {
                    episode_event(event.sequence, "suffix rechain")
                } else {
                    event.payload.clone()
                };
                rechained.push(EventEnvelope::with_chain(
                    event.sequence,
                    event.timestamp_ms,
                    payload,
                    prev.as_deref(),
                ));
            }
            events = rechained;
        }
        AttackClass::SequenceGap => {
            events.remove(TARGET);
        }
        AttackClass::ChecksumMutation => {
            events[TARGET].checksum = "0000000000000000".to_string();
        }
    }
    events
}

fn episode_event(sequence: u64, content: &str) -> MemoryEvent {
    MemoryEvent::EpisodeRecorded(EpisodeRecorded {
        id: format!("episode_{sequence}"),
        content: content.to_string(),
        tags: vec!["livr".to_string()],
        mentions: Vec::new(),
        source_id: None,
        source_position: None,
        source_role: None,
        scope: None,
    })
}

fn round2(value: f32) -> f32 {
    (value * 100.0).round() / 100.0
}

fn tier(results: &[TierResult], tier: DetectorTier) -> &TierResult {
    results
        .iter()
        .find(|result| result.tier == tier)
        .expect("tier present in report")
}

/// The headline result: each tier's detection rate is computed from the corpus,
/// and the per-tier breakdown shows exactly which class each tier misses.
#[test]
fn livr_detection_rate_is_computed_per_tier() {
    let results = run_livr();

    // Four tampered classes plus the clean control.
    let tampered = CLASSES.iter().filter(|class| class.is_tampered()).count();
    assert_eq!(tampered, 4);

    // The naive baseline catches only the corrupted checksum (1 of 4).
    let checksum = tier(&results, DetectorTier::ChecksumOnly);
    assert_eq!(checksum.detection_rate, 0.25);
    assert_eq!(checksum.true_positives, 1);
    assert_eq!(checksum.false_negatives, 3);
    assert_eq!(checksum.false_positives, 0);
    assert_eq!(
        checksum.undetected,
        vec![
            AttackClass::InPlaceRewrite,
            AttackClass::SuffixRechain,
            AttackClass::SequenceGap,
        ]
    );

    // Replaying the chain catches the in-place rewrite and the sequence gap that
    // the checksum alone cannot, but it is blind to a fully re-chained suffix.
    let replay = tier(&results, DetectorTier::ReplayChain);
    assert_eq!(replay.detection_rate, 0.75);
    assert_eq!(replay.true_positives, 3);
    assert_eq!(replay.false_negatives, 1);
    assert_eq!(replay.false_positives, 0);
    assert_eq!(replay.undetected, vec![AttackClass::SuffixRechain]);

    // The anchored tip receipt closes that one blind spot: full detection, and
    // still no false positive on the clean control.
    let attestation = tier(&results, DetectorTier::AttestationTip);
    assert_eq!(attestation.detection_rate, 1.0);
    assert_eq!(attestation.true_positives, 4);
    assert_eq!(attestation.false_negatives, 0);
    assert_eq!(attestation.false_positives, 0);
    assert!(attestation.undetected.is_empty());
}

/// A clean, unchained legacy ledger (records written before the hash chain
/// existed) must not be mistaken for tampering: the replay tier skips unchained
/// events rather than reporting a broken chain.
#[test]
fn legacy_unchained_clean_ledger_is_not_flagged() {
    let legacy: Vec<EventEnvelope> = (1..=4)
        .map(|sequence| {
            EventEnvelope::new(sequence, 1000 + sequence, episode_event(sequence, "legacy"))
        })
        .collect();

    assert!(legacy.iter().all(|event| !event.is_chained()));
    assert!(!replay_chain_flags(&legacy));
}
