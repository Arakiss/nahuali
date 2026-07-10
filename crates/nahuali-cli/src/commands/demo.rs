//! `nahuali demo` — a zero-dependency, no-Docker first look at the trust wedge.
//!
//! The whole story runs in memory using the public `EventEnvelope` API, so a
//! freshly installed binary can show "memory you can prove was not altered" in
//! one command, with no database and no services. It mirrors the
//! `tamper_evidence` core example, dressed for a newcomer.

/// Run the narrated tamper-evidence demo.
///
/// `attestation` is a default feature, so a plain `cargo build` / `cargo install`
/// runs the full story here. The `--no-default-features` build keeps a minimal
/// pointer instead of the apology-free full demo.
pub(crate) fn demo() -> anyhow::Result<()> {
    #[cfg(feature = "attestation")]
    {
        print!("{}", run());
    }
    #[cfg(not(feature = "attestation"))]
    {
        let s = Style::detect();
        println!(
            "{}nahuali demo{} showcases the tamper-evidence trust layer, which needs the {}attestation{} build feature.",
            s.bold, s.reset, s.accent, s.reset
        );
        println!(
            "This binary was built with {}--no-default-features{}.",
            s.dim, s.reset
        );
        println!(
            "For the full story, build the default binary: {}cargo run -p nahuali-cli -- demo{}",
            s.dim, s.reset
        );
        println!(
            "Or install a release: {}https://github.com/Arakiss/nahuali/releases{}",
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
fn run() -> String {
    use std::fmt::Write as _;

    use nahuali_core::{
        EpisodeRecorded, EventEnvelope, MemoryEvent, sign_chain_tip, verify_chain_tip,
        verify_event_chain,
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

    out.push('\n');
    let _ = writeln!(
        out,
        "{}Nahuali{} — memory you can prove was not altered.",
        s.accent, s.reset
    );
    let _ = writeln!(
        out,
        "{}A 20-second look at the wedge, entirely in memory — no database, no Docker.{}",
        s.dim, s.reset
    );
    out.push('\n');

    // Part 1 — an honest, chained ledger.
    let ledger = chained_ledger();
    let last = ledger.last().expect("ledger is not empty");
    let (tip_sequence, tip_hash) = (last.sequence, last.chain_hash());
    let _ = writeln!(
        out,
        "{}1 · An append-only ledger of agent memory.{}",
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
        s.yes(verify_event_chain(&ledger).is_none()),
        short(&tip_hash)
    );
    out.push('\n');

    // Part 2 — the operator signs the tip.
    let receipt =
        sign_chain_tip(DEMO_SEED_HEX, tip_sequence, &tip_hash).expect("signing the tip succeeds");
    let _ = writeln!(
        out,
        "{}2 · The operator signs that tip — a portable receipt.{}",
        s.bold, s.reset
    );
    let _ = writeln!(
        out,
        "    receipt verifies against the live history: {}",
        s.yes(verify_chain_tip(&receipt, tip_sequence, &tip_hash).unwrap())
    );
    out.push('\n');

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
    let _ = writeln!(
        out,
        "{}3 · An attacker rewrites event 2 and recomputes its checksum.{}",
        s.bold, s.reset
    );
    let _ = writeln!(
        out,
        "    per-event checksum still valid (a checksum-only store is fooled): {}",
        s.yes(rewritten[1].validate_checksum())
    );
    match verify_event_chain(&rewritten) {
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
    let _ = writeln!(
        out,
        "{}4 · The attacker re-chains the whole history to repair every link.{}",
        s.bold, s.reset
    );
    let _ = writeln!(
        out,
        "    chain now reports intact: {}   but the tip changed: {}",
        s.yes(verify_event_chain(&rechained).is_none()),
        short(&new_tip)
    );
    let _ = writeln!(
        out,
        "    {}the signed receipt still refuses{}: verifies = {} (forging one needs the private key).",
        s.green,
        s.reset,
        s.yes(verify_chain_tip(&receipt, new_sequence, &new_tip).unwrap())
    );
    out.push('\n');

    let _ = writeln!(out, "{}What you just saw{}", s.bold, s.reset);
    let _ = writeln!(
        out,
        "    The checksum proves an event is internally consistent."
    );
    let _ = writeln!(
        out,
        "    The chain proves the history was not rewritten in place."
    );
    let _ = writeln!(
        out,
        "    The signed tip proves the history was not rewritten at all."
    );
    out.push('\n');
    let _ = writeln!(out, "{}Next{}", s.bold, s.reset);
    let _ = writeln!(
        out,
        "    Run the full engine on real memory:  {}nahuali --database memory trust-report{}",
        s.accent, s.reset
    );
    let _ = writeln!(
        out,
        "    (needs the local stack — see {}https://github.com/Arakiss/nahuali{})",
        s.dim, s.reset
    );
    out.push('\n');

    out
}

#[cfg(all(test, feature = "attestation"))]
mod tests {
    use super::run;

    /// C3: on a default (attestation) build the demo renders the full story — the
    /// four narrated parts and the closing — with no source-build apology.
    #[test]
    fn default_build_demo_runs_the_full_story() {
        // SAFETY: single-threaded test; strip ANSI so the assertions match the
        // plain text regardless of terminal detection.
        unsafe {
            std::env::set_var("NO_COLOR", "1");
        }
        let story = run();

        assert!(story.contains("memory you can prove was not altered"));
        assert!(story.contains("1 · An append-only ledger of agent memory."));
        assert!(story.contains("2 · The operator signs that tip"));
        assert!(story.contains("3 · An attacker rewrites event 2"));
        assert!(story.contains("the chain catches it"));
        assert!(story.contains("4 · The attacker re-chains the whole history"));
        assert!(story.contains("the signed receipt still refuses"));
        assert!(story.contains("What you just saw"));
        // No apology / source-build guidance in the full story.
        assert!(!story.contains("was built with"));
        assert!(!story.contains("--no-default-features"));
    }
}
