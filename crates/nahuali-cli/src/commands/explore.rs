//! The `explore` command: open the store and hand a trust-first snapshot of
//! its memory to the nahuali-ui governance cockpit.
//!
//! Nahuali is agent-first, but a human supervises what the agent stored. This
//! reduces the engine's memory to a plain `Snapshot` of display strings — each
//! item carrying a provenance signal (observed / evidenced / no source) — and
//! launches the interactive TUI.

use std::path::Path;

use nahuali_core::{BriefingOptions, MemoryEngine};
use nahuali_ui::theme::{self, Rgb};
use nahuali_ui::tui::{Item, Snapshot};

pub(crate) fn explore(memory: &mut MemoryEngine, database: &Path) -> anyhow::Result<()> {
    // Only the store-level authority is needed from the briefing; items come
    // from the full projected data below.
    let briefing = memory.briefing_with_options(BriefingOptions {
        episode_limit: 0,
        intention_limit: 0,
        review_limit: 0,
        graph_seed_limit: 0,
    });
    let store_label = crate::style::authority_label(&briefing.authority.mode).to_string();
    let store_color = crate::style::authority_color(&briefing.authority.mode);
    let store_score = briefing.authority.score;

    let data = memory.data();
    let mut items = Vec::new();

    for episode in &data.episodes {
        items.push(Item {
            kind: "episode".to_string(),
            title: excerpt(&episode.content, 64),
            detail: episode.content.clone(),
            trust: Some(("observed".to_string(), theme::GREEN)),
            evidence: episode.source_id.clone(),
        });
    }
    for claim in &data.claims {
        items.push(triple(
            "claim",
            &claim.subject,
            &claim.predicate,
            &claim.object,
            &claim.source_episode_id,
        ));
    }
    for fact in &data.facts {
        items.push(triple(
            "fact",
            &fact.subject,
            &fact.predicate,
            &fact.object,
            &fact.source_episode_id,
        ));
    }
    for link in &data.links {
        items.push(triple(
            "link",
            &link.from,
            &link.relation,
            &link.to,
            &link.source_episode_id,
        ));
    }
    for relation in &data.relations {
        items.push(triple(
            "relation",
            &relation.from,
            &relation.relation,
            &relation.to,
            &relation.source_episode_id,
        ));
    }
    for procedure in &data.procedures {
        items.push(Item {
            kind: "procedure".to_string(),
            title: procedure.name.clone(),
            detail: procedure.body.clone(),
            trust: Some(provenance(procedure.source_episode_id.is_some())),
            evidence: procedure.source_episode_id.clone(),
        });
    }
    for intention in &data.intentions {
        items.push(Item {
            kind: "intention".to_string(),
            title: excerpt(&intention.description, 64),
            detail: format!(
                "{}\n[{:?} · {:?}]",
                intention.description, intention.priority, intention.status
            ),
            trust: Some((format!("{:?}", intention.status), theme::BLUE)),
            evidence: intention.source_episode_id.clone(),
        });
    }
    for entity in &data.entities {
        items.push(Item {
            kind: "entity".to_string(),
            title: format!("{} (×{})", entity.name, entity.mention_count),
            detail: format!("Entity, mentioned {} time(s).", entity.mention_count),
            trust: None,
            evidence: None,
        });
    }

    let snapshot = Snapshot {
        database: database.display().to_string(),
        store_trust_label: store_label,
        store_trust_color: store_color,
        store_trust_score: store_score,
        items,
    };

    nahuali_ui::tui::run(snapshot)?;
    Ok(())
}

/// Provenance signal: evidenced (sourced) in green, no-source in amber.
fn provenance(sourced: bool) -> (String, Rgb) {
    if sourced {
        ("evidenced".to_string(), theme::GREEN)
    } else {
        ("no source".to_string(), theme::AMBER)
    }
}

/// Build a triple item (claim/fact/link/relation) with its provenance signal.
fn triple(kind: &str, a: &str, rel: &str, b: &str, source: &Option<String>) -> Item {
    let text = format!("{a} {rel} {b}");
    Item {
        kind: kind.to_string(),
        title: excerpt(&text, 64),
        detail: text,
        trust: Some(provenance(source.is_some())),
        evidence: source.clone(),
    }
}

/// Shorten text to a single-line title of at most `max` characters.
fn excerpt(text: &str, max: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= max {
        flat
    } else {
        let head: String = flat.chars().take(max).collect();
        format!("{}\u{2026}", head.trim_end())
    }
}
