//! `nahuali demo` is a zero-dependency first look at governed agent memory.
//!
//! The whole story runs through public, read-only projection APIs and the public
//! `EventEnvelope` API. A freshly installed binary can show evidence-aware
//! recall, self-inspection, and tamper detection in one command with no database
//! or services.

/// Run the narrated governed-memory demo.
///
/// `attestation` is a default feature, so a plain `cargo build` / `cargo install`
/// runs the full story here. The `--no-default-features` build keeps a minimal
/// pointer instead of the full history-integrity half of the demo.
pub(crate) fn demo(json: bool) -> anyhow::Result<()> {
    let (narrative, evidence) = run();
    if json {
        crate::output::print_json(&evidence)?;
    } else {
        print!("{narrative}");
    }
    Ok(())
}

struct Style {
    bold: &'static str,
    dim: &'static str,
    green: &'static str,
    red: &'static str,
    accent: &'static str,
    reset: &'static str,
}

impl Style {
    fn detect() -> Self {
        use std::io::IsTerminal;
        let on = std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal();
        if on {
            Self {
                bold: "\x1b[1m",
                dim: "\x1b[2m",
                green: "\x1b[32m",
                red: "\x1b[31m",
                accent: "\x1b[36m",
                reset: "\x1b[0m",
            }
        } else {
            Self {
                bold: "",
                dim: "",
                green: "",
                red: "",
                accent: "",
                reset: "",
            }
        }
    }

    fn yes(&self, value: bool) -> String {
        if value {
            format!("{}yes{}", self.green, self.reset)
        } else {
            format!("{}no{}", self.red, self.reset)
        }
    }
}

fn governed_events(now_ms: u64) -> Vec<nahuali_core::EventEnvelope> {
    use nahuali_core::{EpisodeRecorded, EventEnvelope, FactAsserted, MemoryEvent};

    let payloads = vec![
        MemoryEvent::EpisodeRecorded(EpisodeRecorded {
            id: "episode_release_notes".to_string(),
            content: "Lena owns the release notes.".to_string(),
            tags: vec!["release".to_string()],
            mentions: vec!["Lena".to_string()],
            source_id: None,
            source_position: None,
            source_role: Some("operator".to_string()),
            scope: None,
        }),
        MemoryEvent::FactAsserted(FactAsserted {
            id: "claim_release_owner".to_string(),
            subject: "Lena".to_string(),
            predicate: "owns".to_string(),
            object: "release notes".to_string(),
            source_episode_id: Some("episode_release_notes".to_string()),
            confidence: 0.95,
            scope: None,
        }),
        MemoryEvent::FactAsserted(FactAsserted {
            id: "claim_deployment_owner".to_string(),
            subject: "Mateo".to_string(),
            predicate: "owns".to_string(),
            object: "deployment keys".to_string(),
            source_episode_id: None,
            confidence: 0.9,
            scope: None,
        }),
        MemoryEvent::EpisodeRecorded(EpisodeRecorded {
            id: "episode_launch_tuesday".to_string(),
            content: "The release review set launch day to Tuesday.".to_string(),
            tags: vec!["release".to_string()],
            mentions: Vec::new(),
            source_id: None,
            source_position: None,
            source_role: Some("release review".to_string()),
            scope: None,
        }),
        MemoryEvent::EpisodeRecorded(EpisodeRecorded {
            id: "episode_launch_friday".to_string(),
            content: "The incident review set launch day to Friday.".to_string(),
            tags: vec!["incident".to_string()],
            mentions: Vec::new(),
            source_id: None,
            source_position: None,
            source_role: Some("incident review".to_string()),
            scope: None,
        }),
        MemoryEvent::FactAsserted(FactAsserted {
            id: "claim_launch_tuesday".to_string(),
            subject: "Launch".to_string(),
            predicate: "day".to_string(),
            object: "Tuesday".to_string(),
            source_episode_id: Some("episode_launch_tuesday".to_string()),
            confidence: 0.9,
            scope: None,
        }),
        MemoryEvent::FactAsserted(FactAsserted {
            id: "claim_launch_friday".to_string(),
            subject: "Launch".to_string(),
            predicate: "day".to_string(),
            object: "Friday".to_string(),
            source_episode_id: Some("episode_launch_friday".to_string()),
            confidence: 0.9,
            scope: None,
        }),
    ];

    payloads
        .into_iter()
        .enumerate()
        .map(|(index, payload)| {
            let sequence = (index + 1) as u64;
            EventEnvelope::new(sequence, now_ms, payload)
        })
        .collect()
}

#[cfg(feature = "attestation")]
fn run() -> (String, serde_json::Value) {
    use std::fmt::Write as _;
    use std::time::{SystemTime, UNIX_EPOCH};

    use nahuali_core::{
        EpisodeRecorded, EventEnvelope, MemoryEvent, RecallOptions, project_validated_events,
        recall_projection_with_authority, self_inspect_projection, sign_chain_tip,
        verify_chain_tip, verify_event_chain,
    };

    let mut out = String::new();

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

    let s = Style::detect();
    let short = |hash: &str| -> String { hash.chars().take(16).collect::<String>() + "..." };
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is after the Unix epoch")
        .as_millis() as u64;
    let mut ledger: Vec<EventEnvelope> = Vec::new();
    for event in governed_events(now_ms) {
        let previous = ledger.last().map(EventEnvelope::chain_hash);
        ledger.push(EventEnvelope::with_chain(
            event.sequence,
            event.timestamp_ms,
            event.payload,
            previous.as_deref(),
        ));
    }
    let memory = project_validated_events(&ledger);
    let supported =
        recall_projection_with_authority(&memory, "Lena release notes", RecallOptions::default())
            .expect("supported recall succeeds");
    let supported_claim = supported
        .results
        .iter()
        .find(|result| result.id == "claim_release_owner")
        .expect("supported claim is recalled");
    let supported_trust = supported_claim
        .trust
        .as_ref()
        .expect("authority recall attaches result trust");
    let unsupported = recall_projection_with_authority(
        &memory,
        "Mateo deployment keys",
        RecallOptions::default(),
    )
    .expect("unsupported recall succeeds");
    let unsupported_claim = unsupported
        .results
        .iter()
        .find(|result| result.id == "claim_deployment_owner")
        .expect("unsupported claim is recalled");
    let unsupported_trust = unsupported_claim
        .trust
        .as_ref()
        .expect("authority recall attaches result trust");
    let inspection = self_inspect_projection(&memory);

    out.push('\n');
    let _ = writeln!(
        out,
        "{}Nahuali{}  memory that shows its work",
        s.accent, s.reset
    );
    let _ = writeln!(
        out,
        "{}A zero-setup tour of recall trust, self-inspection, and history integrity.{}",
        s.dim, s.reset
    );
    out.push('\n');

    let _ = writeln!(
        out,
        "{}1 · Recall returns evidence and a verdict.{}",
        s.bold, s.reset
    );
    let _ = writeln!(
        out,
        "    {}CERTIFY{}  {}",
        s.green, s.reset, supported_claim.excerpt
    );
    let _ = writeln!(
        out,
        "             evidence: {}   can trust: {}",
        supported_claim.evidence_id.as_deref().unwrap_or("none"),
        s.yes(supported_trust.can_trust)
    );
    let _ = writeln!(
        out,
        "    {}WARN{}     {}",
        s.red, s.reset, unsupported_claim.excerpt
    );
    let _ = writeln!(
        out,
        "             evidence: {}   can trust: {}",
        unsupported_claim.evidence_id.as_deref().unwrap_or("none"),
        s.yes(unsupported_trust.can_trust)
    );
    out.push('\n');

    let _ = writeln!(
        out,
        "{}2 · The store inspects itself before anything is repaired.{}",
        s.bold, s.reset
    );
    let _ = writeln!(
        out,
        "    unsupported claims: {}   contradictions: {}   review required: {}",
        inspection.health.unsupported_fact_count,
        inspection.summary.contradiction_count,
        s.yes(!inspection.review_queue.is_empty())
    );
    let _ = writeln!(
        out,
        "    overall authority: {}   automatic write-back: {}",
        format!("{:?}", inspection.authority.mode).to_ascii_uppercase(),
        s.yes(inspection.write_back_policy.automatic_write_back)
    );
    let _ = writeln!(
        out,
        "    the supported result still certifies even while unrelated memory needs review"
    );
    out.push('\n');

    // Part 3: an honest, chained ledger.
    let last = ledger.last().expect("ledger is not empty");
    let (tip_sequence, tip_hash) = (last.sequence, last.chain_hash());
    let chain_intact = verify_event_chain(&ledger).is_none();
    let _ = writeln!(
        out,
        "{}3 · The same memory lives in an append-only ledger.{}",
        s.bold, s.reset
    );
    let _ = writeln!(
        out,
        "    {} events, each binding the previous event's chained hash.",
        ledger.len()
    );
    let _ = writeln!(
        out,
        "    chain intact: {}   tip: seq {tip_sequence} {}",
        s.yes(chain_intact),
        short(&tip_hash)
    );
    out.push('\n');

    // Part 4: the operator signs the tip.
    let receipt =
        sign_chain_tip(DEMO_SEED_HEX, tip_sequence, &tip_hash).expect("signing the tip succeeds");
    let receipt_verifies = verify_chain_tip(&receipt, tip_sequence, &tip_hash).unwrap();
    let _ = writeln!(
        out,
        "{}4 · The operator signs that tip and keeps the receipt separately.{}",
        s.bold, s.reset
    );
    let _ = writeln!(
        out,
        "    receipt verifies against the live history: {}",
        s.yes(receipt_verifies)
    );
    out.push('\n');

    // Part 5: in-place rewrite with a recomputed checksum.
    let mut rewritten = ledger.clone();
    let original = &rewritten[1];
    let mut forged = EventEnvelope::new(
        original.sequence,
        original.timestamp_ms,
        episode("episode_2", "TAMPERED: attacker-substituted content."),
    );
    forged.prev_hash = original.prev_hash.clone();
    rewritten[1] = forged;
    let local_checksum_valid = rewritten[1].validate_checksum();
    let _ = writeln!(
        out,
        "{}5 · An attacker rewrites event 2 and recomputes its checksum.{}",
        s.bold, s.reset
    );
    let _ = writeln!(
        out,
        "    per-event checksum still valid (a checksum-only store is fooled): {}",
        s.yes(local_checksum_valid)
    );
    let in_place_break = verify_event_chain(&rewritten);
    match &in_place_break {
        Some(brk) => {
            let _ = writeln!(
                out,
                "    {}the chain catches it{}: broken link at record {} (seq {}).",
                s.green, s.reset, brk.record, brk.sequence
            );
        }
        None => {
            let _ = writeln!(out, "    (unexpected) chain reported intact");
        }
    }
    out.push('\n');

    // Part 6: full suffix re-chain, then the signature catches it.
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
    let new_last = rechained.last().expect("ledger is not empty");
    let (new_sequence, new_tip) = (new_last.sequence, new_last.chain_hash());
    let rechained_intact = verify_event_chain(&rechained).is_none();
    let rewritten_receipt_verifies = verify_chain_tip(&receipt, new_sequence, &new_tip).unwrap();
    let _ = writeln!(
        out,
        "{}6 · The attacker re-chains the whole history to repair every link.{}",
        s.bold, s.reset
    );
    let _ = writeln!(
        out,
        "    chain now reports intact: {}   but the tip changed: {}",
        s.yes(rechained_intact),
        short(&new_tip)
    );
    let _ = writeln!(
        out,
        "    {}the signed receipt still refuses{}: verifies = {} (forging one needs the private key).",
        s.green,
        s.reset,
        s.yes(rewritten_receipt_verifies)
    );
    out.push('\n');

    let _ = writeln!(out, "{}What you just saw{}", s.bold, s.reset);
    let _ = writeln!(
        out,
        "    Recall can certify a supported result and warn on a weak one."
    );
    let _ = writeln!(
        out,
        "    Self-inspection turns memory problems into review work without rewriting memory."
    );
    let _ = writeln!(
        out,
        "    The chain detects in-place edits; an operator-held signed checkpoint detects a full re-chain."
    );
    out.push('\n');
    let _ = writeln!(out, "{}Next{}", s.bold, s.reset);
    let _ = writeln!(
        out,
        "    Wire your agent to the same trust policy:  {}nahuali init{}",
        s.accent, s.reset
    );
    let _ = writeln!(
        out,
        "    Record persistent memory now:          {}nahuali remember \"What happened\"{}",
        s.dim, s.reset
    );
    out.push('\n');

    let evidence = serde_json::json!({
        "supported_recall": {
            "verdict": format!("{:?}", supported_trust.mode).to_ascii_lowercase(),
            "can_trust": supported_trust.can_trust,
            "evidence_id": supported_claim.evidence_id,
        },
        "unsupported_recall": {
            "verdict": format!("{:?}", unsupported_trust.mode).to_ascii_lowercase(),
            "can_trust": unsupported_trust.can_trust,
            "evidence_id": unsupported_claim.evidence_id,
        },
        "inspection": {
            "unsupported_claim_count": inspection.health.unsupported_fact_count,
            "contradiction_count": inspection.summary.contradiction_count,
            "review_required": !inspection.review_queue.is_empty(),
            "automatic_write_back": inspection.write_back_policy.automatic_write_back,
        },
        "history_integrity": {
            "event_count": ledger.len(),
            "chain_intact": chain_intact,
            "checkpoint_verifies_original_tip": receipt_verifies,
            "rewritten_event_checksum_valid": local_checksum_valid,
            "in_place_rewrite_detected": in_place_break.is_some(),
            "full_rechain_intact": rechained_intact,
            "tip_changed_after_rechain": tip_hash != new_tip,
            "checkpoint_rejects_rechain": !rewritten_receipt_verifies,
            "external_checkpoint": true,
        }
    });

    (out, evidence)
}

#[cfg(not(feature = "attestation"))]
fn run() -> (String, serde_json::Value) {
    use std::fmt::Write as _;
    use std::time::{SystemTime, UNIX_EPOCH};

    use nahuali_core::{
        RecallOptions, project_validated_events, recall_projection_with_authority,
        self_inspect_projection,
    };

    let s = Style::detect();
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is after the Unix epoch")
        .as_millis() as u64;
    let memory = project_validated_events(&governed_events(now_ms));
    let supported =
        recall_projection_with_authority(&memory, "Lena release notes", RecallOptions::default())
            .expect("supported recall succeeds");
    let supported_claim = supported
        .results
        .iter()
        .find(|result| result.id == "claim_release_owner")
        .expect("supported claim is recalled");
    let supported_trust = supported_claim
        .trust
        .as_ref()
        .expect("authority recall attaches result trust");
    let unsupported = recall_projection_with_authority(
        &memory,
        "Mateo deployment keys",
        RecallOptions::default(),
    )
    .expect("unsupported recall succeeds");
    let unsupported_claim = unsupported
        .results
        .iter()
        .find(|result| result.id == "claim_deployment_owner")
        .expect("unsupported claim is recalled");
    let unsupported_trust = unsupported_claim
        .trust
        .as_ref()
        .expect("authority recall attaches result trust");
    let inspection = self_inspect_projection(&memory);

    let mut out = String::new();
    let _ = writeln!(
        out,
        "\n{}Nahuali{}  memory that shows its work",
        s.accent, s.reset
    );
    let _ = writeln!(
        out,
        "{}A zero-setup tour of recall trust and self-inspection.{}\n",
        s.dim, s.reset
    );
    let _ = writeln!(
        out,
        "{}1 · Recall returns evidence and a verdict.{}",
        s.bold, s.reset
    );
    let _ = writeln!(
        out,
        "    {}CERTIFY{}  {}",
        s.green, s.reset, supported_claim.excerpt
    );
    let _ = writeln!(
        out,
        "             evidence: {}   can trust: {}",
        supported_claim.evidence_id.as_deref().unwrap_or("none"),
        s.yes(supported_trust.can_trust)
    );
    let _ = writeln!(
        out,
        "    {}WARN{}     {}",
        s.red, s.reset, unsupported_claim.excerpt
    );
    let _ = writeln!(
        out,
        "             evidence: {}   can trust: {}\n",
        unsupported_claim.evidence_id.as_deref().unwrap_or("none"),
        s.yes(unsupported_trust.can_trust)
    );
    let _ = writeln!(
        out,
        "{}2 · The store inspects itself before anything is repaired.{}",
        s.bold, s.reset
    );
    let _ = writeln!(
        out,
        "    unsupported claims: {}   contradictions: {}   review required: {}",
        inspection.health.unsupported_fact_count,
        inspection.summary.contradiction_count,
        s.yes(!inspection.review_queue.is_empty())
    );
    let _ = writeln!(
        out,
        "    overall authority: {}   automatic write-back: {}\n",
        format!("{:?}", inspection.authority.mode).to_ascii_uppercase(),
        s.yes(inspection.write_back_policy.automatic_write_back)
    );
    let _ = writeln!(
        out,
        "{}History integrity proof unavailable in this legacy build.{}",
        s.bold, s.reset
    );
    let _ = writeln!(
        out,
        "Build with the default features to add the hash-chain and signed-checkpoint attack story."
    );
    let _ = writeln!(
        out,
        "{}cargo run -p nahuali-cli -- demo{}\n",
        s.accent, s.reset
    );
    let evidence = serde_json::json!({
        "supported_recall": {
            "verdict": format!("{:?}", supported_trust.mode).to_ascii_lowercase(),
            "can_trust": supported_trust.can_trust,
            "evidence_id": supported_claim.evidence_id,
        },
        "unsupported_recall": {
            "verdict": format!("{:?}", unsupported_trust.mode).to_ascii_lowercase(),
            "can_trust": unsupported_trust.can_trust,
            "evidence_id": unsupported_claim.evidence_id,
        },
        "inspection": {
            "unsupported_claim_count": inspection.health.unsupported_fact_count,
            "contradiction_count": inspection.summary.contradiction_count,
            "review_required": !inspection.review_queue.is_empty(),
            "automatic_write_back": inspection.write_back_policy.automatic_write_back,
        },
        "history_integrity": {
            "available": false,
            "external_checkpoint": false,
        }
    });
    (out, evidence)
}

#[cfg(all(test, feature = "attestation"))]
mod tests {
    use super::run;

    /// A default build proves both halves of the product promise with no services:
    /// governed recall/self-inspection and tamper-evident history.
    #[test]
    fn default_build_demo_runs_the_full_story() {
        // SAFETY: single-threaded test; strip ANSI so the assertions match the
        // plain text regardless of terminal detection.
        unsafe {
            std::env::set_var("NO_COLOR", "1");
        }
        let (story, evidence) = run();

        assert!(story.contains("memory that shows its work"));
        assert!(story.contains("1 · Recall returns evidence and a verdict."));
        assert!(story.contains("CERTIFY  Lena owns release notes"));
        assert!(story.contains("WARN     Mateo owns deployment keys"));
        assert!(story.contains("evidence: episode_release_notes   can trust: yes"));
        assert!(story.contains("evidence: none   can trust: no"));
        assert!(story.contains("2 · The store inspects itself"));
        assert!(story.contains("unsupported claims: 1"));
        assert!(story.contains("contradictions: 1"));
        assert!(story.contains("review required: yes"));
        assert!(story.contains("overall authority: BLOCK"));
        assert!(story.contains("automatic write-back: no"));
        assert!(story.contains("3 · The same memory lives in an append-only ledger."));
        assert!(story.contains("4 · The operator signs that tip"));
        assert!(story.contains("5 · An attacker rewrites event 2"));
        assert!(story.contains("the chain catches it"));
        assert!(story.contains("6 · The attacker re-chains the whole history"));
        assert!(story.contains("the signed receipt still refuses"));
        assert!(story.contains("Self-inspection turns memory problems into review work"));
        assert!(story.contains("What you just saw"));
        // No apology / source-build guidance in the full story.
        assert!(!story.contains("was built with"));
        assert!(!story.contains("--no-default-features"));
        assert_eq!(evidence["supported_recall"]["can_trust"], true);
        assert_eq!(
            evidence["history_integrity"]["in_place_rewrite_detected"],
            true
        );
        assert_eq!(
            evidence["history_integrity"]["checkpoint_rejects_rechain"],
            true
        );
    }
}

#[cfg(all(test, not(feature = "attestation")))]
mod legacy_tests {
    use super::run;

    #[test]
    fn legacy_build_demo_still_runs_governed_recall_and_inspection() {
        unsafe {
            std::env::set_var("NO_COLOR", "1");
        }
        let (story, evidence) = run();

        assert!(story.contains("CERTIFY  Lena owns release notes"));
        assert!(story.contains("WARN     Mateo owns deployment keys"));
        assert!(story.contains("unsupported claims: 1"));
        assert!(story.contains("contradictions: 1"));
        assert!(story.contains("History integrity proof unavailable"));
        assert_eq!(evidence["history_integrity"]["available"], false);
    }
}
