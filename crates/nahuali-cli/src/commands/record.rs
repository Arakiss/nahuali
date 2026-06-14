use anyhow::{Context, bail};
use nahuali_core::{
    IntentionKind, IntentionPriority, IntentionStatus, IntentionUpdateOptions, MemoryEngine,
};

use crate::commands::scope::parse_scope;
use crate::{
    cli::{CliIntentionKind, CliIntentionPriority, CliIntentionStatus},
    output,
};

pub(crate) fn remember(
    memory: &mut MemoryEngine,
    content: Vec<String>,
    tags: Vec<String>,
    mentions: Vec<String>,
    scope: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    let scope = parse_scope(scope)?;
    let episode = if let Some(scope) = scope {
        memory.remember_with_mentions_scoped(content.join(" "), tags, mentions, scope)?
    } else {
        memory.remember_with_mentions(content.join(" "), tags, mentions)?
    };
    if json {
        output::print_json(&episode)?;
    } else {
        let mut summary = format!("\"{}\"", excerpt(&episode.content, 72));
        if !episode.tags.is_empty() {
            summary.push_str(&format!(" · tags: {}", episode.tags.join(", ")));
        }
        if !episode.mentions.is_empty() {
            summary.push_str(&format!(" · mentions: {}", episode.mentions.join(", ")));
        }
        println!(
            "{}",
            crate::style::confirm("Episode recorded", &summary, &episode.id)
        );
    }
    Ok(())
}

pub(crate) fn claim(
    memory: &mut MemoryEngine,
    args: EvidenceArgs,
    json: bool,
) -> anyhow::Result<()> {
    let source_episode_id =
        resolve_source_episode_id(memory, args.source_episode_id, args.source_last)?;
    let scope = parse_scope(args.scope)?;
    let claim = if let Some(scope) = scope {
        memory.add_claim_scoped(
            args.subject,
            args.predicate,
            args.object.join(" "),
            source_episode_id,
            args.confidence,
            scope,
        )?
    } else {
        memory.add_claim(
            args.subject,
            args.predicate,
            args.object.join(" "),
            source_episode_id,
            args.confidence,
        )?
    };
    if json {
        output::print_json(&claim)?;
    } else {
        println!(
            "{}",
            crate::style::confirm(
                "Claim asserted",
                &assertion_summary(
                    &claim.subject,
                    &claim.predicate,
                    &claim.object,
                    claim.confidence,
                    claim.source_episode_id.is_some()
                ),
                &claim.id
            )
        );
    }
    Ok(())
}

pub(crate) fn fact(
    memory: &mut MemoryEngine,
    args: EvidenceArgs,
    json: bool,
) -> anyhow::Result<()> {
    let source_episode_id =
        resolve_source_episode_id(memory, args.source_episode_id, args.source_last)?;
    let scope = parse_scope(args.scope)?;
    let fact = if let Some(scope) = scope {
        memory.add_fact_scoped(
            args.subject,
            args.predicate,
            args.object.join(" "),
            source_episode_id,
            args.confidence,
            scope,
        )?
    } else {
        memory.add_fact(
            args.subject,
            args.predicate,
            args.object.join(" "),
            source_episode_id,
            args.confidence,
        )?
    };
    if json {
        output::print_json(&fact)?;
    } else {
        println!(
            "{}",
            crate::style::confirm(
                "Fact asserted",
                &assertion_summary(
                    &fact.subject,
                    &fact.predicate,
                    &fact.object,
                    fact.confidence,
                    fact.source_episode_id.is_some()
                ),
                &fact.id
            )
        );
    }
    Ok(())
}

pub(crate) fn link(memory: &mut MemoryEngine, args: LinkArgs, json: bool) -> anyhow::Result<()> {
    let source_episode_id =
        resolve_source_episode_id(memory, args.source_episode_id, args.source_last)?;
    let scope = parse_scope(args.scope)?;
    let link = if let Some(scope) = scope {
        memory.add_link_scoped(
            args.from,
            args.relation,
            args.to.join(" "),
            source_episode_id,
            args.confidence,
            scope,
        )?
    } else {
        memory.add_link(
            args.from,
            args.relation,
            args.to.join(" "),
            source_episode_id,
            args.confidence,
        )?
    };
    if json {
        output::print_json(&link)?;
    } else {
        println!(
            "{}",
            crate::style::confirm(
                "Link recorded",
                &assertion_summary(
                    &link.from,
                    &link.relation,
                    &link.to,
                    link.confidence,
                    link.source_episode_id.is_some()
                ),
                &link.id
            )
        );
    }
    Ok(())
}

pub(crate) fn relate(memory: &mut MemoryEngine, args: LinkArgs, json: bool) -> anyhow::Result<()> {
    let source_episode_id =
        resolve_source_episode_id(memory, args.source_episode_id, args.source_last)?;
    let scope = parse_scope(args.scope)?;
    let relation = if let Some(scope) = scope {
        memory.relate_scoped(
            args.from,
            args.relation,
            args.to.join(" "),
            source_episode_id,
            args.confidence,
            scope,
        )?
    } else {
        memory.relate(
            args.from,
            args.relation,
            args.to.join(" "),
            source_episode_id,
            args.confidence,
        )?
    };
    if json {
        output::print_json(&relation)?;
    } else {
        println!(
            "{}",
            crate::style::confirm(
                "Relation recorded",
                &assertion_summary(
                    &relation.from,
                    &relation.relation,
                    &relation.to,
                    relation.confidence,
                    relation.source_episode_id.is_some()
                ),
                &relation.id
            )
        );
    }
    Ok(())
}

pub(crate) fn procedure(
    memory: &mut MemoryEngine,
    args: ProcedureArgs,
    json: bool,
) -> anyhow::Result<()> {
    let source_episode_id =
        resolve_source_episode_id(memory, args.source_episode_id, args.source_last)?;
    let scope = parse_scope(args.scope)?;
    let procedure = if let Some(scope) = scope {
        memory.add_procedure_scoped(
            args.name,
            args.body.join(" "),
            source_episode_id,
            args.confidence,
            scope,
        )?
    } else {
        memory.add_procedure(
            args.name,
            args.body.join(" "),
            source_episode_id,
            args.confidence,
        )?
    };
    if json {
        output::print_json(&procedure)?;
    } else {
        let summary = format!("{} · {}", procedure.name, excerpt(&procedure.body, 56));
        println!(
            "{}",
            crate::style::confirm("Procedure recorded", &summary, &procedure.id)
        );
    }
    Ok(())
}

pub(crate) fn preference(
    memory: &mut MemoryEngine,
    args: ProcedureArgs,
    json: bool,
) -> anyhow::Result<()> {
    let source_episode_id =
        resolve_source_episode_id(memory, args.source_episode_id, args.source_last)?;
    let scope = parse_scope(args.scope)?;
    let preference = if let Some(scope) = scope {
        memory.add_preference_scoped(
            args.name,
            args.body.join(" "),
            source_episode_id,
            args.confidence,
            scope,
        )?
    } else {
        memory.add_preference(
            args.name,
            args.body.join(" "),
            source_episode_id,
            args.confidence,
        )?
    };
    if json {
        output::print_json(&preference)?;
    } else {
        let summary = format!("{} · {}", preference.name, excerpt(&preference.body, 56));
        println!(
            "{}",
            crate::style::confirm("Preference recorded", &summary, &preference.id)
        );
    }
    Ok(())
}

pub(crate) fn intention(
    memory: &mut MemoryEngine,
    args: IntentionArgs,
    json: bool,
) -> anyhow::Result<()> {
    let source_episode_id =
        resolve_source_episode_id(memory, args.source_episode_id, args.source_last)?;
    let scope = parse_scope(args.scope)?;
    let intention = if let Some(scope) = scope {
        memory.add_intention_scoped(
            args.description.join(" "),
            IntentionKind::from(args.kind),
            IntentionPriority::from(args.priority),
            source_episode_id,
            scope,
        )?
    } else {
        memory.add_intention(
            args.description.join(" "),
            IntentionKind::from(args.kind),
            IntentionPriority::from(args.priority),
            source_episode_id,
        )?
    };
    if json {
        output::print_json(&intention)?;
    } else {
        let summary = format!(
            "\"{}\" · {:?}/{:?}",
            excerpt(&intention.description, 60),
            intention.kind,
            intention.priority
        );
        println!(
            "{}",
            crate::style::confirm("Intention noted", &summary, &intention.id)
        );
    }
    Ok(())
}

pub(crate) fn intention_status(
    memory: &mut MemoryEngine,
    id: String,
    status: CliIntentionStatus,
    reason: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    let intention = memory.set_intention_status(id, IntentionStatus::from(status), reason)?;
    if json {
        output::print_json(&intention)?;
    } else {
        println!(
            "{}",
            crate::style::confirm(
                &format!("Intention {:?}", intention.status),
                "",
                &intention.id
            )
        );
    }
    Ok(())
}

pub(crate) fn intention_update(
    memory: &mut MemoryEngine,
    args: IntentionUpdateArgs,
    json: bool,
) -> anyhow::Result<()> {
    if args.clear_dependencies && !args.depends_on.is_empty() {
        bail!("--clear-dependencies cannot be used with --depends-on");
    }

    let intention = memory.update_intention(
        args.id,
        IntentionUpdateOptions {
            description: args.description,
            priority: args.priority.map(IntentionPriority::from),
            deadline_at_ms: if args.clear_deadline {
                Some(None)
            } else {
                args.deadline_at_ms.map(Some)
            },
            depends_on: if args.clear_dependencies {
                Some(Vec::new())
            } else if args.depends_on.is_empty() {
                None
            } else {
                Some(args.depends_on)
            },
            goal_id: if args.clear_goal {
                Some(None)
            } else {
                args.goal.map(Some)
            },
            progress_percent: if args.clear_progress {
                Some(None)
            } else {
                args.progress.map(Some)
            },
        },
    )?;
    if json {
        output::print_json(&intention)?;
    } else {
        println!(
            "{}",
            crate::style::confirm("Intention updated", "", &intention.id)
        );
    }
    Ok(())
}

pub(crate) fn intention_complete(
    memory: &mut MemoryEngine,
    id: String,
    reason: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    let intention = memory.complete_intention(id, reason)?;
    print_intention_status_update(intention, json)
}

pub(crate) fn intention_block(
    memory: &mut MemoryEngine,
    id: String,
    reason: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    let intention = memory.block_intention(id, reason)?;
    print_intention_status_update(intention, json)
}

pub(crate) fn intention_defer(
    memory: &mut MemoryEngine,
    id: String,
    reason: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    let intention = memory.defer_intention(id, reason)?;
    print_intention_status_update(intention, json)
}

fn print_intention_status_update(
    intention: nahuali_core::Intention,
    json: bool,
) -> anyhow::Result<()> {
    if json {
        output::print_json(&intention)?;
    } else {
        println!(
            "{}",
            crate::style::confirm(
                &format!("Intention {:?}", intention.status),
                "",
                &intention.id
            )
        );
    }
    Ok(())
}

pub(crate) struct EvidenceArgs {
    pub(crate) subject: String,
    pub(crate) predicate: String,
    pub(crate) object: Vec<String>,
    pub(crate) source_episode_id: Option<String>,
    pub(crate) source_last: bool,
    pub(crate) confidence: f32,
    pub(crate) scope: Option<String>,
}

pub(crate) struct LinkArgs {
    pub(crate) from: String,
    pub(crate) relation: String,
    pub(crate) to: Vec<String>,
    pub(crate) source_episode_id: Option<String>,
    pub(crate) source_last: bool,
    pub(crate) confidence: f32,
    pub(crate) scope: Option<String>,
}

pub(crate) struct ProcedureArgs {
    pub(crate) name: String,
    pub(crate) body: Vec<String>,
    pub(crate) source_episode_id: Option<String>,
    pub(crate) source_last: bool,
    pub(crate) confidence: f32,
    pub(crate) scope: Option<String>,
}

pub(crate) struct IntentionArgs {
    pub(crate) description: Vec<String>,
    pub(crate) kind: CliIntentionKind,
    pub(crate) priority: CliIntentionPriority,
    pub(crate) source_episode_id: Option<String>,
    pub(crate) source_last: bool,
    pub(crate) scope: Option<String>,
}

pub(crate) struct IntentionUpdateArgs {
    pub(crate) id: String,
    pub(crate) description: Option<String>,
    pub(crate) priority: Option<CliIntentionPriority>,
    pub(crate) deadline_at_ms: Option<u64>,
    pub(crate) clear_deadline: bool,
    pub(crate) depends_on: Vec<String>,
    pub(crate) clear_dependencies: bool,
    pub(crate) goal: Option<String>,
    pub(crate) clear_goal: bool,
    pub(crate) progress: Option<u8>,
    pub(crate) clear_progress: bool,
}

/// Truncate `text` to at most `max` characters for a one-line confirmation.
fn excerpt(text: &str, max: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max {
        trimmed.to_string()
    } else {
        let head: String = trimmed.chars().take(max).collect();
        format!("{head}\u{2026}")
    }
}

/// Human summary for an asserted triple (claim, fact, link, or relation),
/// noting confidence and whether it carries source evidence.
fn assertion_summary(a: &str, rel: &str, b: &str, confidence: f32, sourced: bool) -> String {
    format!(
        "{a} {rel} {b} (confidence {confidence:.2}, {})",
        if sourced { "sourced" } else { "unsourced" }
    )
}

fn resolve_source_episode_id(
    memory: &MemoryEngine,
    source_episode_id: Option<String>,
    source_last: bool,
) -> anyhow::Result<Option<String>> {
    if source_episode_id.is_some() && source_last {
        bail!("--source-last cannot be used with --source-episode");
    }

    if source_last {
        let episode = memory
            .data()
            .episodes
            .last()
            .context(
                "--source-last requires at least one episode in the selected database; run `nahuali remember ...` first or pass --source-episode",
            )?;
        return Ok(Some(episode.id.clone()));
    }

    Ok(source_episode_id)
}
