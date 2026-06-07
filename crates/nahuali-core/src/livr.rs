//! Reproducible ledger-integrity measurement (LIVR).
//!
//! LIVR -- a Ledger Integrity Verification Rate -- is a *computed* detection
//! rate, not an asserted headline number. [`run_livr`] injects a fixed corpus
//! of tampering into a synthetic chained ledger, runs the real ledger
//! validators over each case, and computes `TP / (TP + FN)` for three detector
//! tiers of increasing strength:
//!
//! * checksum-only   -- the self-contained per-event checksum (the naive baseline);
//! * replay-chain    -- checksum + sequence contiguity + the hash chain;
//! * attestation-tip -- replay plus an externally anchored, signed tip receipt.
//!
//! The point of exposing this as a library function rather than burying it in a
//! test is that the headline governance number is *reproducible by the caller*:
//! anyone can run [`run_livr`] and recompute the same per-tier detection rates
//! the project publishes, instead of trusting an asserted figure.
//!
//! Honest limits: it measures detection against a fixed synthetic injection
//! method only; a passing rate proves self-consistency detection, not the
//! absence of all tampering. The replay tier cannot catch a fully re-chained
//! suffix on its own -- that gap is exactly what the anchored-tip tier closes,
//! and the per-tier breakdown makes the gap visible rather than averaging it
//! away.
//!
//! Available only under the `attestation` feature, so all three tiers exist.

use serde::Serialize;

use crate::{
    EpisodeRecorded, EventEnvelope, LedgerAttestation, MemoryEvent, sign_chain_tip,
    verify_chain_tip, verify_event_chain,
};

/// Current [`LivrReport`] schema version.
pub const LIVR_REPORT_VERSION: u32 = 1;

/// Fixed signing seed for the synthetic attestation receipt. The corpus is
/// synthetic and reproducible, so a constant seed is intentional.
const SEED_HEX: &str = "0101010101010101010101010101010101010101010101010101010101010101";
/// Number of events in the synthetic ledger.
const LEDGER_LEN: u64 = 6;
/// Zero-based index of the middle event the injectors target.
const TARGET: usize = 2;

/// A tampering pattern injected into the synthetic ledger, or the clean control.
///
/// The corpus is grouped by the weakest tier that catches each class, so the
/// per-tier detection rates stay honest and stable as classes are added: two
/// checksum-catchable classes, four that need the replay chain, and two fully
/// re-chained suffixes that only the anchored tip can see.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LivrAttackClass {
    /// No tampering: the control. A tier that flags it scores a false positive.
    Clean,
    /// A single event's stored checksum is corrupted.
    ChecksumMutation,
    /// A middle event's payload is edited but its stored checksum is left
    /// untouched, so the self-contained checksum no longer matches the body.
    PayloadEditNoRecompute,
    /// A middle event's payload is rewritten and its own checksum recomputed,
    /// keeping the recorded chain link -- the strongest attack on a
    /// checksum-only model.
    InPlaceRewrite,
    /// A middle event's timestamp is skewed and its checksum recomputed, keeping
    /// the recorded chain link. The forged checksum is self-consistent, but the
    /// timestamp is bound into the chain hash, so the next link no longer matches.
    TimestampSkew,
    /// A middle event is dropped, leaving a sequence discontinuity.
    SequenceGap,
    /// A middle event is replaced by an event grafted from a different ledger:
    /// its own checksum is valid, but its chain link belongs to a foreign chain.
    CrossLedgerGraft,
    /// A middle event is rewritten and the whole suffix re-chained, so every
    /// link is internally consistent again.
    SuffixRechain,
    /// A middle event's payload is truncated (content redacted) and the whole
    /// suffix re-chained, so the ledger stays internally consistent -- a
    /// redaction the replay chain cannot see, only the anchored tip can.
    PayloadTruncationRechain,
}

impl LivrAttackClass {
    /// Whether this class injects tampering (everything but [`Self::Clean`]).
    fn is_tampered(self) -> bool {
        !matches!(self, LivrAttackClass::Clean)
    }
}

/// A ledger-integrity detector of increasing strength.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LivrDetectorTier {
    /// The self-contained per-event checksum only (the naive baseline).
    ChecksumOnly,
    /// Checksum plus sequence contiguity plus the hash chain.
    ReplayChain,
    /// Replay plus an externally anchored, signed tip receipt.
    AttestationTip,
}

/// Detection outcome for a single detector tier over the corpus.
#[derive(Clone, Debug, Serialize)]
pub struct LivrTierResult {
    /// The detector tier these counts describe.
    pub tier: LivrDetectorTier,
    /// Tampered classes the tier flagged.
    pub true_positives: usize,
    /// Tampered classes the tier missed.
    pub false_negatives: usize,
    /// Clean controls the tier wrongly flagged.
    pub false_positives: usize,
    /// `TP / (TP + FN)`, rounded to two decimals.
    pub detection_rate: f32,
    /// The specific tampered classes this tier did not catch.
    pub undetected: Vec<LivrAttackClass>,
}

/// A computed LIVR report across every detector tier.
#[derive(Clone, Debug, Serialize)]
pub struct LivrReport {
    /// Report schema version.
    pub version: u32,
    /// Number of events in the synthetic ledger.
    pub ledger_len: u64,
    /// Number of tampered attack classes in the corpus (excludes the clean control).
    pub tampered_class_count: usize,
    /// Per-tier detection outcomes, weakest tier first.
    pub tiers: Vec<LivrTierResult>,
}

const CLASSES: [LivrAttackClass; 9] = [
    LivrAttackClass::Clean,
    // checksum-catchable
    LivrAttackClass::ChecksumMutation,
    LivrAttackClass::PayloadEditNoRecompute,
    // need the replay chain
    LivrAttackClass::InPlaceRewrite,
    LivrAttackClass::TimestampSkew,
    LivrAttackClass::SequenceGap,
    LivrAttackClass::CrossLedgerGraft,
    // only the anchored tip
    LivrAttackClass::SuffixRechain,
    LivrAttackClass::PayloadTruncationRechain,
];

const TIERS: [LivrDetectorTier; 3] = [
    LivrDetectorTier::ChecksumOnly,
    LivrDetectorTier::ReplayChain,
    LivrDetectorTier::AttestationTip,
];

/// Run the fixed tampering corpus through every detector tier and return the
/// computed per-tier detection report.
///
/// Deterministic and side-effect free: it builds an in-memory synthetic ledger,
/// signs its tip with a fixed seed, injects each tampering class, and measures
/// what each tier catches. No database, filesystem, or network access.
pub fn run_livr() -> LivrReport {
    let tiers = run_tiers();
    let tampered_class_count = CLASSES.iter().filter(|class| class.is_tampered()).count();
    LivrReport {
        version: LIVR_REPORT_VERSION,
        ledger_len: LEDGER_LEN,
        tampered_class_count,
        tiers,
    }
}

/// Run the corpus through every tier and compute per-tier detection.
fn run_tiers() -> Vec<LivrTierResult> {
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
            LivrTierResult {
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

fn detect(tier: LivrDetectorTier, events: &[EventEnvelope], receipt: &LedgerAttestation) -> bool {
    match tier {
        LivrDetectorTier::ChecksumOnly => checksum_flags(events),
        LivrDetectorTier::ReplayChain => replay_chain_flags(events),
        LivrDetectorTier::AttestationTip => attestation_tip_flags(events, receipt),
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

fn inject(clean: &[EventEnvelope], class: LivrAttackClass) -> Vec<EventEnvelope> {
    let mut events = clean.to_vec();
    match class {
        LivrAttackClass::Clean => {}
        LivrAttackClass::InPlaceRewrite => {
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
        LivrAttackClass::SuffixRechain => {
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
        LivrAttackClass::SequenceGap => {
            events.remove(TARGET);
        }
        LivrAttackClass::ChecksumMutation => {
            events[TARGET].checksum = "0000000000000000".to_string();
        }
        LivrAttackClass::PayloadEditNoRecompute => {
            // Edit the body but leave the stored checksum as-is: the self-contained
            // checksum is now stale, so even the naive tier catches it.
            events[TARGET].payload = episode_event(events[TARGET].sequence, "uncomputed edit");
            assert!(
                !events[TARGET].validate_checksum(),
                "the stale checksum no longer matches the edited body"
            );
        }
        LivrAttackClass::TimestampSkew => {
            let original = &events[TARGET];
            // Skew the timestamp by a day and recompute the per-event checksum
            // (new() recomputes it), preserving the recorded chain link.
            let mut forged = EventEnvelope::new(
                original.sequence,
                original.timestamp_ms.wrapping_add(86_400_000),
                original.payload.clone(),
            );
            forged.prev_hash = original.prev_hash.clone();
            assert!(forged.validate_checksum(), "the skewed checksum is valid");
            events[TARGET] = forged;
        }
        LivrAttackClass::CrossLedgerGraft => {
            // Graft an event from a *different* ledger: its own checksum is valid,
            // but its chain link points at a foreign predecessor.
            let foreign_prev =
                EventEnvelope::with_chain(1, 999, episode_event(99, "foreign genesis"), None)
                    .chain_hash();
            let target = &events[TARGET];
            let graft = EventEnvelope::with_chain(
                target.sequence,
                target.timestamp_ms,
                episode_event(target.sequence, "cross-ledger graft"),
                Some(&foreign_prev),
            );
            assert!(graft.validate_checksum(), "the grafted checksum is valid");
            events[TARGET] = graft;
        }
        LivrAttackClass::PayloadTruncationRechain => {
            // Truncate the target's content, then re-chain the whole ledger so
            // every link is internally consistent again -- like SuffixRechain but
            // via a redaction rather than a replacement.
            let mut rechained: Vec<EventEnvelope> = Vec::new();
            for (index, event) in clean.iter().enumerate() {
                let prev = rechained.last().map(EventEnvelope::chain_hash);
                let payload = if index == TARGET {
                    episode_event(event.sequence, "")
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tier(report: &LivrReport, tier: LivrDetectorTier) -> &LivrTierResult {
        report
            .tiers
            .iter()
            .find(|result| result.tier == tier)
            .expect("tier present in report")
    }

    /// The headline result: each tier's detection rate is computed from the
    /// corpus, and the per-tier breakdown shows exactly which class each tier
    /// misses.
    #[test]
    fn livr_detection_rate_is_computed_per_tier() {
        let report = run_livr();
        assert_eq!(report.version, LIVR_REPORT_VERSION);
        assert_eq!(report.ledger_len, LEDGER_LEN);

        // Eight tampered classes plus the clean control.
        assert_eq!(report.tampered_class_count, 8);

        // The naive baseline catches only the two classes that leave a stale or
        // corrupted per-event checksum (2 of 8): everything else forges a valid one.
        let checksum = tier(&report, LivrDetectorTier::ChecksumOnly);
        assert_eq!(checksum.detection_rate, 0.25);
        assert_eq!(checksum.true_positives, 2);
        assert_eq!(checksum.false_negatives, 6);
        assert_eq!(checksum.false_positives, 0);
        assert_eq!(
            checksum.undetected,
            vec![
                LivrAttackClass::InPlaceRewrite,
                LivrAttackClass::TimestampSkew,
                LivrAttackClass::SequenceGap,
                LivrAttackClass::CrossLedgerGraft,
                LivrAttackClass::SuffixRechain,
                LivrAttackClass::PayloadTruncationRechain,
            ]
        );

        // Replaying the chain catches every in-place edit, the timestamp skew, the
        // sequence gap, and the cross-ledger graft that the checksum alone cannot
        // (6 of 8), but it is blind to a fully re-chained suffix in either form.
        let replay = tier(&report, LivrDetectorTier::ReplayChain);
        assert_eq!(replay.detection_rate, 0.75);
        assert_eq!(replay.true_positives, 6);
        assert_eq!(replay.false_negatives, 2);
        assert_eq!(replay.false_positives, 0);
        assert_eq!(
            replay.undetected,
            vec![
                LivrAttackClass::SuffixRechain,
                LivrAttackClass::PayloadTruncationRechain,
            ]
        );

        // The anchored tip receipt closes both blind spots: full detection, and
        // still no false positive on the clean control.
        let attestation = tier(&report, LivrDetectorTier::AttestationTip);
        assert_eq!(attestation.detection_rate, 1.0);
        assert_eq!(attestation.true_positives, 8);
        assert_eq!(attestation.false_negatives, 0);
        assert_eq!(attestation.false_positives, 0);
        assert!(attestation.undetected.is_empty());
    }

    /// A clean, unchained legacy ledger (records written before the hash chain
    /// existed) must not be mistaken for tampering: the replay tier skips
    /// unchained events rather than reporting a broken chain.
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
}
