//! `nahuali demo` — a zero-dependency, no-Docker first look at the trust wedge.
//!
//! The whole story runs in memory using the public `EventEnvelope` API, so a
//! freshly installed binary can show "memory you can prove was not altered" in
//! one command, with no database and no services. It mirrors the
//! `tamper_evidence` core example, dressed for a newcomer.

/// Run the narrated tamper-evidence demo.
pub(crate) fn demo() -> anyhow::Result<()> {
    #[cfg(feature = "attestation")]
    {
        run();
    }
    #[cfg(not(feature = "attestation"))]
    {
        let s = Style::detect();
        println!(
            "{}nahuali demo{} showcases the tamper-evidence trust layer, which needs the {}attestation{} build feature.",
            s.bold, s.reset, s.accent, s.reset
        );
        println!("The official release binary ships with it already.");
        println!(
            "From source: {}cargo run -p nahuali-cli --features attestation -- demo{}",
            s.dim, s.reset
        );
    }
    Ok(())
}

// `green`/`red`/`yes` are exercised only by the attestation-feature demo body.
#[allow(dead_code)]
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

    #[allow(dead_code)]
    fn yes(&self, value: bool) -> String {
        if value {
            format!("{}yes{}", self.green, self.reset)
        } else {
            format!("{}no{}", self.red, self.reset)
        }
    }
}

#[cfg(feature = "attestation")]
fn run() {
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

    let s = Style::detect();
    let short = |hash: &str| -> String { hash.chars().take(16).collect::<String>() + "..." };

    println!();
    println!(
        "{}Nahuali{} — memory you can prove was not altered.",
        s.accent, s.reset
    );
    println!(
        "{}A 20-second look at the wedge, entirely in memory — no database, no Docker.{}",
        s.dim, s.reset
    );
    println!();

    // Part 1 — an honest, chained ledger.
    let ledger = chained_ledger();
    let last = ledger.last().expect("ledger is not empty");
    let (tip_sequence, tip_hash) = (last.sequence, last.chain_hash());
    println!(
        "{}1 · An append-only ledger of agent memory.{}",
        s.bold, s.reset
    );
    println!(
        "    {} events, each binding the previous event's chained hash.",
        ledger.len()
    );
    println!(
        "    chain intact: {}   tip: seq {tip_sequence} {}",
        s.yes(verify_event_chain(&ledger).is_none()),
        short(&tip_hash)
    );
    println!();

    // Part 2 — the operator signs the tip.
    let receipt =
        sign_chain_tip(DEMO_SEED_HEX, tip_sequence, &tip_hash).expect("signing the tip succeeds");
    println!(
        "{}2 · The operator signs that tip — a portable receipt.{}",
        s.bold, s.reset
    );
    println!(
        "    receipt verifies against the live history: {}",
        s.yes(verify_chain_tip(&receipt, tip_sequence, &tip_hash).unwrap())
    );
    println!();

    // Part 3 — in-place rewrite with a recomputed checksum.
    let mut rewritten = ledger.clone();
    let original = &rewritten[1];
    let mut forged = EventEnvelope::new(
        original.sequence,
        original.timestamp_ms,
        episode("episode_2", "TAMPERED: attacker-substituted content."),
    );
    forged.prev_hash = original.prev_hash.clone();
    rewritten[1] = forged;
    println!(
        "{}3 · An attacker rewrites event 2 and recomputes its checksum.{}",
        s.bold, s.reset
    );
    println!(
        "    per-event checksum still valid (a checksum-only store is fooled): {}",
        s.yes(rewritten[1].validate_checksum())
    );
    match verify_event_chain(&rewritten) {
        Some(brk) => println!(
            "    {}the chain catches it{}: broken link at record {} (seq {}).",
            s.green, s.reset, brk.record, brk.sequence
        ),
        None => println!("    (unexpected) chain reported intact"),
    }
    println!();

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
    let new_last = rechained.last().expect("ledger is not empty");
    let (new_sequence, new_tip) = (new_last.sequence, new_last.chain_hash());
    println!(
        "{}4 · The attacker re-chains the whole history to repair every link.{}",
        s.bold, s.reset
    );
    println!(
        "    chain now reports intact: {}   but the tip changed: {}",
        s.yes(verify_event_chain(&rechained).is_none()),
        short(&new_tip)
    );
    println!(
        "    {}the signed receipt still refuses{}: verifies = {} (forging one needs the private key).",
        s.green,
        s.reset,
        s.yes(verify_chain_tip(&receipt, new_sequence, &new_tip).unwrap())
    );
    println!();

    println!("{}What you just saw{}", s.bold, s.reset);
    println!("    The checksum proves an event is internally consistent.");
    println!("    The chain proves the history was not rewritten in place.");
    println!("    The signed tip proves the history was not rewritten at all.");
    println!();
    println!("{}Next{}", s.bold, s.reset);
    println!(
        "    Run the full engine on real memory:  {}nahuali --database ./memory trust-report{}",
        s.accent, s.reset
    );
    println!(
        "    (needs the local stack — see {}https://github.com/Arakiss/nahuali{})",
        s.dim, s.reset
    );
    println!();
}
