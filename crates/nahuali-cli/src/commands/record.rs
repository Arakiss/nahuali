use anyhow::{Context, bail};
use nahuali_core::{
    IntentionKind, IntentionPriority, IntentionStatus, IntentionUpdateOptions, MemoryEngine,
};

use crate::cli::{CliIntentionKind, CliIntentionPriority, CliIntentionStatus};
use crate::commands::scope::parse_scope;

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
        println!("{}", serde_json::to_string(&episode)?);
    } else {
        println!("remembered {}", episode.id);
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
        println!("{}", serde_json::to_string(&claim)?);
    } else {
        println!("claimed {}", claim.id);
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
        println!("{}", serde_json::to_string(&fact)?);
    } else {
        println!("asserted {}", fact.id);
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
        println!("{}", serde_json::to_string(&link)?);
    } else {
        println!("linked {}", link.id);
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
        println!("{}", serde_json::to_string(&relation)?);
    } else {
        println!("related {}", relation.id);
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
        println!("{}", serde_json::to_string(&procedure)?);
    } else {
        println!("recorded {}", procedure.id);
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
        println!("{}", serde_json::to_string(&preference)?);
    } else {
        println!("recorded {}", preference.id);
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
        println!("{}", serde_json::to_string(&intention)?);
    } else {
        println!("recorded {}", intention.id);
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
        println!("{}", serde_json::to_string(&intention)?);
    } else {
        println!("updated {} {:?}", intention.id, intention.status);
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
        println!("{}", serde_json::to_string(&intention)?);
    } else {
        println!("updated {}", intention.id);
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
        println!("{}", serde_json::to_string(&intention)?);
    } else {
        println!("updated {} {:?}", intention.id, intention.status);
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
