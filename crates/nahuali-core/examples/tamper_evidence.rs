//! Narrated walkthrough of Nahuali's tamper-evidence wedge.
//!
//! Run it with:
//!
//! ```text
//! cargo run -p nahuali-core --example tamper_evidence --features attestation
//! ```
//!
//! It tells the story in four parts, entirely in memory and offline:
//!   1. an honest, hash-chained ledger and its tip;
//!   2. the operator signs that tip into a portable receipt;
//!   3. an attacker rewrites a historical event *and recomputes its checksum* —
//!      the self-contained checksum is fooled, but the chain catches it;
//!   4. the attacker goes further and re-chains the whole suffix so no link is
//!      broken — the chain is now fooled, but the signed receipt is not, because
//!      the tip changed and forging a fresh signature needs the private key.
//!
//! The events here are built directly with the public `EventEnvelope` API to keep
//! the demo dependency-free; on disk the engine writes exactly these chained
//! records.

use nahuali_core::{
    EpisodeRecorded, EventEnvelope, MemoryEvent, sign_chain_tip, verify_chain_tip,
    verify_event_chain,
};

// A fixed, non-secret demo seed. In production, generate one with
// `openssl rand -hex 32` and keep it off the machine that holds the ledger.
const DEMO_SEED_HEX: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn episode(id: &str, content: &str) -> MemoryEvent {
    MemoryEvent::EpisodeRecorded(EpisodeRecorded {
        id: id.to_string(),
        content: content.to_string(),
        tags: vec!["demo".to_string()],
        mentions: Vec::new(),
        source_id: None,
        source_position: None,
        source_role: None,
        scope: None,
    })
}

fn chained_ledger() -> Vec<EventEnvelope> {
    let contents = [
        "Lena owns the release notes.",
        "Aaron reviews the changelog every Friday.",
        "The 0.3 beta ships behind a feature flag.",
        "Support escalations route to the on-call engineer.",
    ];
    let mut events: Vec<EventEnvelope> = Vec::new();
    for (index, content) in contents.iter().enumerate() {
        let sequence = (index + 1) as u64;
        let previous = events.last().map(EventEnvelope::chain_hash);
        events.push(EventEnvelope::with_chain(
            sequence,
            1_000 + sequence,
            episode(&format!("episode_{sequence}"), content),
            previous.as_deref(),
        ));
    }
    events
}

fn tip(events: &[EventEnvelope]) -> (u64, String) {
    let last = events.last().expect("ledger is not empty");
    (last.sequence, last.chain_hash())
}

fn main() {
    println!("== Nahuali tamper-evidence demo ==\n");

    // Part 1 — an honest, chained ledger.
    let ledger = chained_ledger();
    let (tip_sequence, tip_hash) = tip(&ledger);
    println!(
        "1. An append-only ledger of {} chained events.",
        ledger.len()
    );
    println!("   Each event binds the previous event's chained hash.");
    println!("   chain intact: {}", verify_event_chain(&ledger).is_none());
    println!("   tip: seq {tip_sequence} {tip_hash}\n");

    // Part 2 — the operator signs the tip.
    let receipt =
        sign_chain_tip(DEMO_SEED_HEX, tip_sequence, &tip_hash).expect("signing the tip succeeds");
    println!("2. The operator signs that tip with an Ed25519 key (the receipt).");
    println!("   public key: {}", receipt.public_key);
    println!(
        "   receipt verifies against the live tip: {}\n",
        verify_chain_tip(&receipt, tip_sequence, &tip_hash).unwrap()
    );

    // Part 3 — in-place rewrite with a recomputed checksum.
    let mut rewritten = ledger.clone();
    let original = &rewritten[1];
    let mut forged = EventEnvelope::new(
        original.sequence,
        original.timestamp_ms,
        episode("episode_2", "TAMPERED: attacker-substituted content."),
    );
    // Keep the original chain link so only the body changed — the in-place
    // rewrite a checksum-only model cannot catch.
    forged.prev_hash = original.prev_hash.clone();
    rewritten[1] = forged;

    println!("3. An attacker rewrites event 2 and recomputes its own checksum.");
    println!(
        "   per-event checksum still valid (checksum-only model fooled): {}",
        rewritten[1].validate_checksum()
    );
    match verify_event_chain(&rewritten) {
        Some(chain_break) => println!(
            "   the chain catches it: broken link at record {} (seq {}).\n",
            chain_break.record, chain_break.sequence
        ),
        None => println!("   (unexpected) chain reported intact\n"),
    }

    // Part 4 — full suffix re-chain, then the signature catches it.
    let mut rechained: Vec<EventEnvelope> = Vec::new();
    for (index, event) in ledger.iter().enumerate() {
        let previous = rechained.last().map(EventEnvelope::chain_hash);
        let payload = if index == 1 {
            episode("episode_2", "TAMPERED then re-chained.")
        } else {
            event.payload.clone()
        };
        rechained.push(EventEnvelope::with_chain(
            event.sequence,
            event.timestamp_ms,
            payload,
            previous.as_deref(),
        ));
    }
    let (new_sequence, new_tip) = tip(&rechained);

    println!("4. The attacker re-chains the entire suffix to repair every link.");
    println!(
        "   chain now reports intact: {}",
        verify_event_chain(&rechained).is_none()
    );
    println!("   but the tip changed: seq {new_sequence} {new_tip}");
    println!(
        "   the signed receipt no longer verifies: {}",
        verify_chain_tip(&receipt, new_sequence, &new_tip).unwrap()
    );
    println!("   forging a fresh receipt would require the operator's private key.\n");

    println!("Checksum proves an event is internally consistent.");
    println!("The chain proves the history was not rewritten in place.");
    println!("The signed tip proves the history was not rewritten at all.");
}
