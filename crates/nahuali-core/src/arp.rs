//! Attestation Recovery Profile (ARP) — a reproducible key-lifecycle coverage check.
//!
//! Where [`crate::run_livr`] measures integrity detection on the *ledger* side,
//! ARP measures the *key* side: does the attestation keyring behave correctly
//! across the full lifecycle of a signing key? It drives a deterministic matrix
//! over a synthetic chained ledger and asserts the honored/rejected verdict at
//! each step:
//!
//! * `valid_receipt`    — a receipt under an active key for the live tip is honored;
//! * `rechained_suffix` — a re-chain that moves the tip voids the old receipt;
//! * `rotated_key`      — after rotation, a receipt under the new active key is honored;
//! * `revoked_key`      — a cryptographically valid receipt under a revoked key is rejected;
//! * `foreign_ledger`   — a receipt over a different ledger's tip is rejected here.
//!
//! Reported as a coverage table (pass/fail per scenario), not a single rate, so
//! the result is a profile rather than an average. Like LIVR it is reproducible
//! from the library: any caller can recompute [`run_arp`]. Available only under
//! the `attestation` feature.

use serde::Serialize;

use crate::{
    AttestationKey, AttestationKeyStatus, AttestationKeyring, EpisodeRecorded, EventEnvelope,
    MemoryEvent, sign_chain_tip, verify_attestation_with_keyring,
};

/// Current [`ArpReport`] schema version.
pub const ARP_REPORT_VERSION: u32 = 1;

/// Active signing key seed (32 bytes of 0x01), the original key.
const SEED_A: &str = "0101010101010101010101010101010101010101010101010101010101010101";
/// Rotation target seed (32 bytes of 0x02), a distinct key.
const SEED_B: &str = "0202020202020202020202020202020202020202020202020202020202020202";
/// Foreign-ledger seed (32 bytes of 0x03) for the cross-ledger case.
const SEED_FOREIGN: &str = "0303030303030303030303030303030303030303030303030303030303030303";
/// Number of events in the synthetic ledger.
const LEDGER_LEN: u64 = 4;
/// Zero-based index of the middle event the re-chain rewrites.
const TARGET: usize = 1;

/// Outcome of one key-lifecycle scenario.
#[derive(Clone, Debug, Serialize)]
pub struct ArpCase {
    /// Scenario name.
    pub scenario: String,
    /// Whether the receipt should be honored in this scenario.
    pub expected_trusted: bool,
    /// Whether the keyring-aware verifier actually honored it.
    pub actual_trusted: bool,
    /// The receipt's `(sequence, tip)` matched the expected checkpoint.
    pub matches_tip: bool,
    /// The signature verified against the recorded public key.
    pub signature_valid: bool,
    /// The signing key was present and active in the keyring.
    pub key_trusted: bool,
    /// The signing key was present and revoked in the keyring.
    pub key_revoked: bool,
    /// `actual_trusted == expected_trusted`.
    pub pass: bool,
}

/// A computed ARP coverage report across the key-lifecycle scenarios.
#[derive(Clone, Debug, Serialize)]
pub struct ArpReport {
    /// Report schema version.
    pub version: u32,
    /// Number of scenarios in the matrix.
    pub case_count: usize,
    /// Number of scenarios whose verdict matched expectation.
    pub passed: usize,
    /// Whether every scenario matched expectation.
    pub all_pass: bool,
    /// Per-scenario outcomes.
    pub cases: Vec<ArpCase>,
}

/// Run the key-lifecycle matrix and return the computed coverage report.
///
/// Deterministic and side-effect free: it builds an in-memory synthetic ledger,
/// signs it with fixed seeds, and checks the keyring-aware verifier's verdict in
/// each scenario. No database, filesystem, or network access.
pub fn run_arp() -> ArpReport {
    let ledger = clean_ledger(LEDGER_LEN);
    let (sequence, tip) = tip_of(&ledger);
    let receipt_a = sign_chain_tip(SEED_A, sequence, &tip).expect("seed A signs");
    let public_key_a = receipt_a.public_key.clone();

    // A re-chained suffix changes the tip; the old receipt no longer matches.
    let rechained = rechained_ledger(&ledger);
    let (rechained_sequence, rechained_tip) = tip_of(&rechained);

    // Rotation: a second key signs the same live tip.
    let receipt_b = sign_chain_tip(SEED_B, sequence, &tip).expect("seed B signs");
    let public_key_b = receipt_b.public_key.clone();

    // A receipt signed over a *different* ledger's tip.
    let foreign = foreign_ledger();
    let (foreign_sequence, foreign_tip) = tip_of(&foreign);
    let receipt_foreign =
        sign_chain_tip(SEED_FOREIGN, foreign_sequence, &foreign_tip).expect("foreign seed signs");
    let public_key_foreign = receipt_foreign.public_key.clone();

    let keyring_initial = AttestationKeyring {
        keys: vec![active("primary", &public_key_a)],
    };
    // After rotation: the old key is revoked, the new key is active. The foreign
    // key is also trusted here, to prove the tip check — not just key trust —
    // rejects the cross-ledger receipt.
    let keyring_rotated = AttestationKeyring {
        keys: vec![
            revoked("retired", &public_key_a),
            active("current", &public_key_b),
            active("foreign", &public_key_foreign),
        ],
    };
    let keyring_foreign = AttestationKeyring {
        keys: vec![active("foreign", &public_key_foreign)],
    };

    let cases = vec![
        evaluate(
            "valid_receipt",
            true,
            &receipt_a,
            sequence,
            &tip,
            &keyring_initial,
        ),
        evaluate(
            "rechained_suffix",
            false,
            &receipt_a,
            rechained_sequence,
            &rechained_tip,
            &keyring_initial,
        ),
        evaluate(
            "rotated_key",
            true,
            &receipt_b,
            sequence,
            &tip,
            &keyring_rotated,
        ),
        evaluate(
            "revoked_key",
            false,
            &receipt_a,
            sequence,
            &tip,
            &keyring_rotated,
        ),
        evaluate(
            "foreign_ledger",
            false,
            &receipt_foreign,
            sequence,
            &tip,
            &keyring_foreign,
        ),
    ];

    let passed = cases.iter().filter(|case| case.pass).count();
    ArpReport {
        version: ARP_REPORT_VERSION,
        case_count: cases.len(),
        passed,
        all_pass: passed == cases.len(),
        cases,
    }
}

fn evaluate(
    scenario: &str,
    expected_trusted: bool,
    receipt: &crate::LedgerAttestation,
    sequence: u64,
    tip: &str,
    keyring: &AttestationKeyring,
) -> ArpCase {
    let verdict = verify_attestation_with_keyring(receipt, sequence, tip, keyring)
        .expect("synthetic receipt verifies without malformed fields");
    let actual_trusted = verdict.is_trusted();
    ArpCase {
        scenario: scenario.to_string(),
        expected_trusted,
        actual_trusted,
        matches_tip: verdict.matches_tip,
        signature_valid: verdict.signature_valid,
        key_trusted: verdict.key_trusted,
        key_revoked: verdict.key_revoked,
        pass: actual_trusted == expected_trusted,
    }
}

fn active(key_id: &str, public_key: &str) -> AttestationKey {
    AttestationKey {
        key_id: Some(key_id.to_string()),
        public_key: public_key.to_string(),
        status: AttestationKeyStatus::Active,
    }
}

fn revoked(key_id: &str, public_key: &str) -> AttestationKey {
    AttestationKey {
        key_id: Some(key_id.to_string()),
        public_key: public_key.to_string(),
        status: AttestationKeyStatus::Revoked,
    }
}

fn tip_of(events: &[EventEnvelope]) -> (u64, String) {
    let sequence = events
        .last()
        .map(|event| event.sequence)
        .unwrap_or_default();
    let tip = events
        .last()
        .map(EventEnvelope::chain_hash)
        .unwrap_or_default();
    (sequence, tip)
}

fn clean_ledger(count: u64) -> Vec<EventEnvelope> {
    let mut events: Vec<EventEnvelope> = Vec::new();
    for sequence in 1..=count {
        let prev = events.last().map(EventEnvelope::chain_hash);
        events.push(EventEnvelope::with_chain(
            sequence,
            1000 + sequence,
            episode_event(sequence, &format!("arp event {sequence}")),
            prev.as_deref(),
        ));
    }
    events
}

fn rechained_ledger(clean: &[EventEnvelope]) -> Vec<EventEnvelope> {
    let mut rechained: Vec<EventEnvelope> = Vec::new();
    for (index, event) in clean.iter().enumerate() {
        let prev = rechained.last().map(EventEnvelope::chain_hash);
        let payload = if index == TARGET {
            episode_event(event.sequence, "rechained edit")
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
    rechained
}

fn foreign_ledger() -> Vec<EventEnvelope> {
    let mut events: Vec<EventEnvelope> = Vec::new();
    for sequence in 1..=LEDGER_LEN {
        let prev = events.last().map(EventEnvelope::chain_hash);
        events.push(EventEnvelope::with_chain(
            sequence,
            5000 + sequence,
            episode_event(sequence, &format!("foreign event {sequence}")),
            prev.as_deref(),
        ));
    }
    events
}

fn episode_event(sequence: u64, content: &str) -> MemoryEvent {
    MemoryEvent::EpisodeRecorded(EpisodeRecorded {
        id: format!("episode_{sequence}"),
        content: content.to_string(),
        tags: vec!["arp".to_string()],
        mentions: Vec::new(),
        source_id: None,
        source_position: None,
        source_role: None,
        scope: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arp_covers_the_key_lifecycle_with_every_scenario_passing() {
        let report = run_arp();
        assert_eq!(report.version, ARP_REPORT_VERSION);
        assert_eq!(report.case_count, 5);
        assert_eq!(report.passed, 5);
        assert!(report.all_pass);

        // The honored scenarios are exactly the live-key, current-tip ones.
        let honored: Vec<&str> = report
            .cases
            .iter()
            .filter(|case| case.actual_trusted)
            .map(|case| case.scenario.as_str())
            .collect();
        assert_eq!(honored, vec!["valid_receipt", "rotated_key"]);

        // The revoked-key case has a valid signature but is still rejected.
        let revoked = report
            .cases
            .iter()
            .find(|case| case.scenario == "revoked_key")
            .expect("revoked_key scenario present");
        assert!(revoked.signature_valid && revoked.key_revoked && !revoked.actual_trusted);
    }
}
