//! The `explore` command: open the store and hand a trust-first snapshot of
//! its memory to the nahuali-ui governance cockpit.
//!
//! A human can supervise what an agent stored. This reduces the engine's memory
//! to a plain `Snapshot` of display strings — each
//! item carrying a provenance signal (observed / evidenced / no source) and a
//! few detail fields — plus store-level governance signals, and launches the
//! interactive TUI.

use std::path::Path;

#[cfg(feature = "attestation")]
use std::path::PathBuf;

#[cfg(feature = "attestation")]
use nahuali_core::CheckpointMatchMode;
#[cfg(feature = "tamper-evidence")]
use nahuali_core::LedgerChainStatus;
use nahuali_core::{BriefingOptions, LedgerAuditOptions, MemoryEngine, MemoryScope};
use nahuali_ui::theme::{self, Rgb};
use nahuali_ui::tui::{Anchor, AnchorStatus, Integrity, Item, LedgerStatus, Signal, Snapshot};

pub(crate) struct ExploreOptions {
    #[cfg(feature = "attestation")]
    pub(crate) checkpoint: Option<PathBuf>,
    #[cfg(feature = "attestation")]
    pub(crate) policy: Option<PathBuf>,
    #[cfg(feature = "attestation")]
    pub(crate) checkpoint_mode: Option<CheckpointMatchMode>,
}

pub(crate) fn explore(
    memory: &mut MemoryEngine,
    _database: &Path,
    options: ExploreOptions,
) -> anyhow::Result<()> {
    let briefing = memory.briefing_with_options(BriefingOptions {
        episode_limit: 0,
        intention_limit: 0,
        review_limit: 0,
        graph_seed_limit: 0,
    });
    let (store_label, store_color) = if briefing.event_count == 0 {
        (
            "EMPTY · ready for the first memory".to_string(),
            theme::INK_DIM,
        )
    } else {
        (
            crate::style::authority_label(&briefing.authority.mode).to_string(),
            crate::style::authority_color(&briefing.authority.mode),
        )
    };

    // The tamper-evidence posture is kept separate from content authority.
    let audit = memory.audit_ledger(&LedgerAuditOptions::default());
    #[cfg(feature = "tamper-evidence")]
    let ledger_status = match audit.integrity.chain_status {
        LedgerChainStatus::Empty => LedgerStatus::Empty,
        LedgerChainStatus::Verified => LedgerStatus::Verified,
        LedgerChainStatus::Legacy => LedgerStatus::Legacy,
        LedgerChainStatus::Broken => LedgerStatus::Broken,
    };
    #[cfg(not(feature = "tamper-evidence"))]
    let ledger_status = LedgerStatus::Unavailable;
    #[cfg(feature = "tamper-evidence")]
    let merkle_root = audit.integrity.merkle_root.clone();
    #[cfg(not(feature = "tamper-evidence"))]
    let merkle_root = None;
    let integrity = Integrity {
        records: briefing.event_count,
        checksums_valid: audit.integrity.checksums_valid,
        sequence_contiguous: audit.integrity.sequence_contiguous,
        status: ledger_status,
        merkle_root,
    };
    let anchor = checkpoint_anchor(memory, &options);

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
        if data.claims.iter().any(|claim| {
            claim.subject == fact.subject
                && claim.predicate == fact.predicate
                && claim.object == fact.object
        }) {
            continue;
        }
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
        if data.links.iter().any(|link| {
            link.from == relation.from
                && link.relation == relation.relation
                && link.to == relation.to
        }) {
            continue;
        }
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
            meta: vec![("mentions".to_string(), entity.mention_count.to_string())],
        });
    }

    let snapshot = Snapshot {
        store_trust_label: store_label,
        store_trust_color: store_color,
        integrity,
        anchor,
        signals: signals(memory, &briefing),
        items,
    };

    nahuali_ui::tui::run(snapshot)?;
    Ok(())
}

#[cfg(feature = "attestation")]
fn checkpoint_anchor(memory: &mut MemoryEngine, options: &ExploreOptions) -> Anchor {
    let (Some(checkpoint), Some(policy)) =
        (options.checkpoint.as_deref(), options.policy.as_deref())
    else {
        return Anchor {
            status: AnchorStatus::NotChecked,
            newer_updates: 0,
        };
    };
    let mode = options
        .checkpoint_mode
        .unwrap_or(CheckpointMatchMode::Current);
    match super::checkpoint::verification_verdict(memory, checkpoint, policy, mode) {
        Ok(verdict) if verdict.trusted && mode == CheckpointMatchMode::Current => Anchor {
            status: AnchorStatus::TrustedCurrent,
            newer_updates: 0,
        },
        Ok(verdict) if verdict.trusted => Anchor {
            status: AnchorStatus::TrustedHistorical,
            newer_updates: verdict.appended_event_count,
        },
        Ok(_) => Anchor {
            status: AnchorStatus::Untrusted,
            newer_updates: 0,
        },
        Err(_) => Anchor {
            status: AnchorStatus::Invalid,
            newer_updates: 0,
        },
    }
}

#[cfg(not(feature = "attestation"))]
fn checkpoint_anchor(_memory: &mut MemoryEngine, _options: &ExploreOptions) -> Anchor {
    Anchor {
        status: AnchorStatus::Unavailable,
        newer_updates: 0,
    }
}

/// Store-level governance signals — what a human supervisor watches.
fn signals(memory: &MemoryEngine, briefing: &nahuali_core::MemoryBriefingReport) -> Vec<Signal> {
    let data = memory.data();
    // Provenance coverage follows the rows a person actually sees. Compatibility
    // facts and relations that duplicate a claim or link count only once.
    let visible_facts = data.facts.iter().filter(|fact| {
        !data.claims.iter().any(|claim| {
            claim.subject == fact.subject
                && claim.predicate == fact.predicate
                && claim.object == fact.object
        })
    });
    let visible_relations = data.relations.iter().filter(|relation| {
        !data.links.iter().any(|link| {
            link.from == relation.from
                && link.relation == relation.relation
                && link.to == relation.to
        })
    });
    let total = data.claims.len()
        + visible_facts.clone().count()
        + data.links.len()
        + visible_relations.clone().count()
        + data.procedures.len();
    let evidenced = data
        .claims
        .iter()
        .filter(|c| c.source_episode_id.is_some())
        .count()
        + visible_facts
            .filter(|fact| fact.source_episode_id.is_some())
            .count()
        + data
            .links
            .iter()
            .filter(|l| l.source_episode_id.is_some())
            .count()
        + visible_relations
            .filter(|relation| relation.source_episode_id.is_some())
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

    let empty = briefing.event_count == 0;
    let health = if empty {
        0
    } else {
        briefing.health.signal_count
    };
    let review = if empty {
        0
    } else {
        briefing.summary.high_priority_review_count
    };

    vec![
        Signal {
            label: "updates".to_string(),
            value: briefing.event_count.to_string(),
            color: theme::INK_DIM,
        },
        Signal {
            label: "needs attention".to_string(),
            value: health.to_string(),
            color: if health == 0 {
                theme::GREEN
            } else {
                theme::AMBER
            },
        },
        Signal {
            label: "to review".to_string(),
            value: review.to_string(),
            color: if review == 0 {
                theme::GREEN
            } else {
                theme::AMBER
            },
        },
        Signal {
            label: "open tasks".to_string(),
            value: briefing.summary.active_intention_count.to_string(),
            color: theme::BLUE,
        },
        Signal {
            label: "with evidence".to_string(),
            value: format!("{evidenced}/{total}"),
            color: provenance_color,
        },
    ]
}

/// Human-readable detail fields for an item. Internal ids stay out of the TUI.
fn meta(confidence: Option<f32>, scope: &Option<MemoryScope>, _id: &str) -> Vec<(String, String)> {
    let mut fields = Vec::new();
    if let Some(confidence) = confidence {
        fields.push((
            "confidence".to_string(),
            format!("{:.0}%", confidence * 100.0),
        ));
    }
    if let Some(scope) = scope {
        fields.push(("context".to_string(), scope.name.clone()));
    }
    fields
}

/// Provenance signal: evidenced (sourced) in green, no-source in amber.
fn provenance(sourced: bool) -> (String, Rgb) {
    if sourced {
        ("with evidence".to_string(), theme::GREEN)
    } else {
        ("needs evidence".to_string(), theme::AMBER)
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
