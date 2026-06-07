//! The `explore` command: open the store and hand a trust-first snapshot of
//! its memory to the nahuali-ui governance cockpit.
//!
//! Nahuali is agent-first, but a human supervises what the agent stored. This
//! reduces the engine's memory to a plain `Snapshot` of display strings — each
//! item carrying a provenance signal (observed / evidenced / no source) and a
//! few detail fields — plus store-level governance signals, and launches the
//! interactive TUI.

use std::path::Path;

use nahuali_core::{BriefingOptions, LedgerAuditOptions, MemoryEngine, MemoryScope};
use nahuali_ui::theme::{self, Rgb};
use nahuali_ui::tui::{Integrity, Item, Signal, Snapshot};

pub(crate) fn explore(memory: &mut MemoryEngine, database: &Path) -> anyhow::Result<()> {
    let briefing = memory.briefing_with_options(BriefingOptions {
        episode_limit: 0,
        intention_limit: 0,
        review_limit: 0,
        graph_seed_limit: 0,
    });
    let store_label = crate::style::authority_label(&briefing.authority.mode).to_string();
    let store_color = crate::style::authority_color(&briefing.authority.mode);
    let store_score = briefing.authority.score;

    // The tamper-evidence posture — the differentiator, surfaced honestly. In a
    // build without the hash-chain feature, merkle_root is None (chain off).
    let audit = memory.audit_ledger(&LedgerAuditOptions::default());
    let integrity = Integrity {
        verified: audit.integrity.verified,
        records: briefing.event_count,
        chain_intact: audit.integrity.chain_intact,
        merkle_root: audit.integrity.merkle_root.clone(),
    };

    let data = memory.data();
    let mut items = Vec::new();

    for episode in &data.episodes {
        items.push(Item {
            kind: "episode".to_string(),
            title: excerpt(&episode.content, 64),
            detail: episode.content.clone(),
            trust: Some(("observed".to_string(), theme::GREEN)),
            evidence: episode.source_id.clone(),
            meta: meta(None, &episode.scope, &episode.id),
        });
    }
    for claim in &data.claims {
        items.push(triple(
            "claim",
            &claim.subject,
            &claim.predicate,
            &claim.object,
            &claim.source_episode_id,
            meta(Some(claim.confidence), &claim.scope, &claim.id),
        ));
    }
    for fact in &data.facts {
        items.push(triple(
            "fact",
            &fact.subject,
            &fact.predicate,
            &fact.object,
            &fact.source_episode_id,
            meta(Some(fact.confidence), &fact.scope, &fact.id),
        ));
    }
    for link in &data.links {
        items.push(triple(
            "link",
            &link.from,
            &link.relation,
            &link.to,
            &link.source_episode_id,
            meta(Some(link.confidence), &link.scope, &link.id),
        ));
    }
    for relation in &data.relations {
        items.push(triple(
            "relation",
            &relation.from,
            &relation.relation,
            &relation.to,
            &relation.source_episode_id,
            meta(Some(relation.confidence), &relation.scope, &relation.id),
        ));
    }
    for procedure in &data.procedures {
        items.push(Item {
            kind: "procedure".to_string(),
            title: procedure.name.clone(),
            detail: procedure.body.clone(),
            trust: Some(provenance(procedure.source_episode_id.is_some())),
            evidence: procedure.source_episode_id.clone(),
            meta: meta(Some(procedure.confidence), &procedure.scope, &procedure.id),
        });
    }
    for intention in &data.intentions {
        let mut detail_meta = vec![
            ("priority".to_string(), format!("{:?}", intention.priority)),
            ("status".to_string(), format!("{:?}", intention.status)),
        ];
        detail_meta.extend(meta(None, &intention.scope, &intention.id));
        items.push(Item {
            kind: "intention".to_string(),
            title: excerpt(&intention.description, 64),
            detail: intention.description.clone(),
            trust: Some((format!("{:?}", intention.status), theme::BLUE)),
            evidence: intention.source_episode_id.clone(),
            meta: detail_meta,
        });
    }
    for entity in &data.entities {
        items.push(Item {
            kind: "entity".to_string(),
            title: format!("{} (×{})", entity.name, entity.mention_count),
            detail: format!("Entity, mentioned {} time(s).", entity.mention_count),
            trust: None,
            evidence: None,
            meta: vec![
                ("mentions".to_string(), entity.mention_count.to_string()),
                ("id".to_string(), entity.id.clone()),
            ],
        });
    }

    let snapshot = Snapshot {
        database: database.display().to_string(),
        store_trust_label: store_label,
        store_trust_color: store_color,
        store_trust_score: store_score,
        integrity,
        signals: signals(memory, &briefing),
        items,
    };

    nahuali_ui::tui::run(snapshot)?;
    Ok(())
}

/// Store-level governance signals — what a human supervisor watches.
fn signals(memory: &MemoryEngine, briefing: &nahuali_core::MemoryBriefingReport) -> Vec<Signal> {
    let data = memory.data();
    // Provenance coverage: derived items (claims/facts/links/relations/procedures)
    // that cite a source episode, over the total.
    let total = data.claims.len()
        + data.facts.len()
        + data.links.len()
        + data.relations.len()
        + data.procedures.len();
    let evidenced = data
        .claims
        .iter()
        .filter(|c| c.source_episode_id.is_some())
        .count()
        + data
            .facts
            .iter()
            .filter(|f| f.source_episode_id.is_some())
            .count()
        + data
            .links
            .iter()
            .filter(|l| l.source_episode_id.is_some())
            .count()
        + data
            .relations
            .iter()
            .filter(|r| r.source_episode_id.is_some())
            .count()
        + data
            .procedures
            .iter()
            .filter(|p| p.source_episode_id.is_some())
            .count();
    let provenance_color = if total == 0 || evidenced == total {
        theme::GREEN
    } else if evidenced * 2 >= total {
        theme::AMBER
    } else {
        theme::RED
    };

    let health = briefing.health.blind_spot_count;
    let review = briefing.summary.high_priority_review_count;

    vec![
        Signal {
            label: "events".to_string(),
            value: briefing.event_count.to_string(),
            color: theme::INK_DIM,
        },
        Signal {
            label: "health signals".to_string(),
            value: health.to_string(),
            color: if health == 0 {
                theme::GREEN
            } else {
                theme::AMBER
            },
        },
        Signal {
            label: "review".to_string(),
            value: review.to_string(),
            color: if review == 0 {
                theme::GREEN
            } else {
                theme::AMBER
            },
        },
        Signal {
            label: "active intentions".to_string(),
            value: briefing.summary.active_intention_count.to_string(),
            color: theme::BLUE,
        },
        Signal {
            label: "evidenced".to_string(),
            value: format!("{evidenced}/{total}"),
            color: provenance_color,
        },
    ]
}

/// Common detail fields for an item: confidence (if any), scope (if any), id.
fn meta(confidence: Option<f32>, scope: &Option<MemoryScope>, id: &str) -> Vec<(String, String)> {
    let mut fields = Vec::new();
    if let Some(confidence) = confidence {
        fields.push(("conf".to_string(), format!("{confidence:.2}")));
    }
    if let Some(scope) = scope {
        fields.push(("scope".to_string(), format!("{scope:?}")));
    }
    fields.push(("id".to_string(), id.to_string()));
    fields
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
fn triple(
    kind: &str,
    a: &str,
    rel: &str,
    b: &str,
    source: &Option<String>,
    meta: Vec<(String, String)>,
) -> Item {
    let text = format!("{a} {rel} {b}");
    Item {
        kind: kind.to_string(),
        title: excerpt(&text, 64),
        detail: text,
        trust: Some(provenance(source.is_some())),
        evidence: source.clone(),
        meta,
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
