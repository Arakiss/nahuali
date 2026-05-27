use std::collections::{HashMap, HashSet};

use crate::{
    Result,
    model::{IntentionStatus, MemoryData, ProcedureKind},
    store::{ImportEvidenceContext, MemoryEngine, SourceEpisodeOptions, SourceRecordOptions},
};

use super::{
    InterchangeClaim, InterchangeEpisode, InterchangeImportCounts, InterchangeImportReport,
    InterchangeIntention, InterchangeIssue, InterchangeIssueKind, InterchangeLink,
    InterchangeProcedure, InterchangeSource, MEMORY_INTERCHANGE_VERSION, MemoryInterchange,
    clean_optional, preflight, readiness,
};

pub(crate) fn export(data: &MemoryData) -> MemoryInterchange {
    MemoryInterchange {
        version: MEMORY_INTERCHANGE_VERSION,
        sources: data
            .sources
            .iter()
            .map(|source| InterchangeSource {
                ref_id: source.id.clone(),
                kind: source.kind.clone(),
                title: source.title.clone(),
                uri: source.uri.clone(),
                content_checksum: Some(source.content_checksum.clone()),
                byte_len: source.byte_len,
                metadata: source.metadata.clone(),
                scope: source.scope.clone(),
                timestamp_ms: Some(source.created_at_ms),
            })
            .collect(),
        episodes: data
            .episodes
            .iter()
            .map(|episode| InterchangeEpisode {
                ref_id: Some(episode.id.clone()),
                content: episode.content.clone(),
                tags: episode.tags.clone(),
                mentions: episode.mentions.clone(),
                source_role: episode.source_role.clone(),
                source_ref: episode.source_id.clone(),
                source_position: episode.source_position,
                scope: episode.scope.clone(),
                timestamp_ms: Some(episode.created_at_ms),
            })
            .collect(),
        claims: data
            .claims
            .iter()
            .map(|claim| InterchangeClaim {
                subject: claim.subject.clone(),
                predicate: claim.predicate.clone(),
                object: claim.object.clone(),
                source_episode_ref: claim.source_episode_id.clone(),
                confidence: claim.confidence,
                scope: claim.scope.clone(),
                timestamp_ms: Some(claim.created_at_ms),
            })
            .collect(),
        links: data
            .links
            .iter()
            .map(|link| InterchangeLink {
                from: link.from.clone(),
                relation: link.relation.clone(),
                to: link.to.clone(),
                source_episode_ref: link.source_episode_id.clone(),
                confidence: link.confidence,
                scope: link.scope.clone(),
                timestamp_ms: Some(link.created_at_ms),
            })
            .collect(),
        procedures: data
            .procedures
            .iter()
            .map(|procedure| InterchangeProcedure {
                kind: procedure.kind.clone(),
                name: procedure.name.clone(),
                body: procedure.body.clone(),
                source_episode_ref: procedure.source_episode_id.clone(),
                confidence: procedure.confidence,
                scope: procedure.scope.clone(),
                timestamp_ms: Some(procedure.created_at_ms),
            })
            .collect(),
        intentions: data
            .intentions
            .iter()
            .map(|intention| InterchangeIntention {
                kind: intention.kind.clone(),
                priority: intention.priority.clone(),
                status: intention.status.clone(),
                description: intention.description.clone(),
                source_episode_ref: intention.source_episode_id.clone(),
                status_reason: intention.status_reason.clone(),
                scope: intention.scope.clone(),
                timestamp_ms: Some(intention.created_at_ms),
                status_timestamp_ms: (intention.status != IntentionStatus::Active)
                    .then_some(intention.updated_at_ms),
            })
            .collect(),
    }
}

pub(crate) fn import(
    memory: &mut MemoryEngine,
    document: &MemoryInterchange,
    dry_run: bool,
) -> Result<InterchangeImportReport> {
    let mut report = validate(document, dry_run);
    if !report.valid || dry_run {
        return Ok(report);
    }

    let mut source_refs = HashMap::new();
    let mut episode_refs = HashMap::new();
    import_sources(memory, document, &mut report, &mut source_refs)?;
    import_episodes(
        memory,
        document,
        &mut report,
        &source_refs,
        &mut episode_refs,
    )?;
    import_claims(memory, document, &mut report, &episode_refs)?;
    import_links(memory, document, &mut report, &episode_refs)?;
    import_procedures(memory, document, &mut report, &episode_refs)?;
    import_intentions(memory, document, &mut report, &episode_refs)?;

    Ok(report)
}

pub(crate) fn validate(document: &MemoryInterchange, dry_run: bool) -> InterchangeImportReport {
    let counts = InterchangeImportCounts {
        sources: document.sources.len(),
        episodes: document.episodes.len(),
        claims: document.claims.len(),
        links: document.links.len(),
        procedures: document.procedures.len(),
        intentions: document.intentions.len(),
        intention_status_updates: document
            .intentions
            .iter()
            .filter(|intention| intention.status != IntentionStatus::Active)
            .count(),
    };
    let mut issues = Vec::new();
    let mut source_refs = HashSet::new();
    let mut episode_refs = HashSet::new();

    validate_version(document, &mut issues);
    validate_sources(document, &mut issues, &mut source_refs);
    validate_episodes(document, &mut issues, &source_refs, &mut episode_refs);
    validate_claims(document, &mut issues, &episode_refs);
    validate_links(document, &mut issues, &episode_refs);
    validate_procedures(document, &mut issues, &episode_refs);
    validate_intentions(document, &mut issues, &episode_refs);

    let valid = issues.is_empty();
    let preflight = preflight::preflight(document, &counts);
    let readiness = readiness::forecast(document);
    InterchangeImportReport {
        version: document.version,
        valid,
        dry_run,
        appendable_event_count: counts.event_count(),
        imported_event_count: 0,
        counts,
        preflight,
        readiness,
        issues,
    }
}

fn import_sources(
    memory: &mut MemoryEngine,
    document: &MemoryInterchange,
    report: &mut InterchangeImportReport,
    source_refs: &mut HashMap<String, String>,
) -> Result<()> {
    for source in &document.sources {
        let options = SourceRecordOptions {
            kind: source.kind.clone(),
            title: source.title.clone(),
            uri: source.uri.clone(),
            content_checksum: source_checksum(source),
            byte_len: source.byte_len,
            metadata: source.metadata.clone(),
            scope: source.scope.clone(),
        };
        let imported = if let Some(timestamp_ms) = source.timestamp_ms {
            memory.import_source_at(options, timestamp_ms)?
        } else {
            memory.record_source_with_options(options)?
        };
        source_refs.insert(source.ref_id.trim().to_string(), imported.id);
        report.imported_event_count += 1;
    }
    Ok(())
}

fn import_episodes(
    memory: &mut MemoryEngine,
    document: &MemoryInterchange,
    report: &mut InterchangeImportReport,
    source_refs: &HashMap<String, String>,
    episode_refs: &mut HashMap<String, String>,
) -> Result<()> {
    for episode in &document.episodes {
        let source_id = resolve_ref(source_refs, &episode.source_ref);
        let imported = if let Some(source_id) = source_id {
            import_source_episode(memory, episode, source_id)?
        } else if let Some(timestamp_ms) = episode.timestamp_ms {
            memory.import_episode_at(
                episode.content.trim(),
                clean_strings(&episode.tags),
                clean_strings(&episode.mentions),
                episode.scope.clone(),
                episode.source_role.clone(),
                timestamp_ms,
            )?
        } else if let Some(scope) = episode.scope.clone() {
            memory.remember_with_mentions_scoped(
                episode.content.trim(),
                clean_strings(&episode.tags),
                clean_strings(&episode.mentions),
                scope,
            )?
        } else {
            memory.remember_with_mentions(
                episode.content.trim(),
                clean_strings(&episode.tags),
                clean_strings(&episode.mentions),
            )?
        };
        if let Some(ref_id) = clean_optional(&episode.ref_id) {
            episode_refs.insert(ref_id, imported.id);
        }
        report.imported_event_count += 1;
    }
    Ok(())
}

fn import_source_episode(
    memory: &mut MemoryEngine,
    episode: &InterchangeEpisode,
    source_id: String,
) -> Result<crate::model::Episode> {
    let options = SourceEpisodeOptions {
        content: episode.content.trim().to_string(),
        tags: clean_strings(&episode.tags),
        mentions: clean_strings(&episode.mentions),
        source_id,
        source_position: episode.source_position,
        source_role: episode.source_role.clone(),
        scope: episode.scope.clone(),
    };
    if let Some(timestamp_ms) = episode.timestamp_ms {
        memory.import_source_episode_at(options, timestamp_ms)
    } else {
        memory.remember_source_episode_with_options(options)
    }
}

fn import_claims(
    memory: &mut MemoryEngine,
    document: &MemoryInterchange,
    report: &mut InterchangeImportReport,
    episode_refs: &HashMap<String, String>,
) -> Result<()> {
    for claim in &document.claims {
        let source_episode_id = resolve_ref(episode_refs, &claim.source_episode_ref);
        if let Some(timestamp_ms) = claim.timestamp_ms {
            memory.import_claim_at(
                claim.subject.trim(),
                claim.predicate.trim(),
                claim.object.trim(),
                ImportEvidenceContext {
                    source_episode_id,
                    confidence: claim.confidence,
                    scope: claim.scope.clone(),
                    timestamp_ms,
                },
            )?;
        } else if let Some(scope) = claim.scope.clone() {
            memory.add_claim_scoped(
                claim.subject.trim(),
                claim.predicate.trim(),
                claim.object.trim(),
                source_episode_id,
                claim.confidence,
                scope,
            )?;
        } else {
            memory.add_claim(
                claim.subject.trim(),
                claim.predicate.trim(),
                claim.object.trim(),
                source_episode_id,
                claim.confidence,
            )?;
        }
        report.imported_event_count += 1;
    }
    Ok(())
}

fn import_links(
    memory: &mut MemoryEngine,
    document: &MemoryInterchange,
    report: &mut InterchangeImportReport,
    episode_refs: &HashMap<String, String>,
) -> Result<()> {
    for link in &document.links {
        let source_episode_id = resolve_ref(episode_refs, &link.source_episode_ref);
        if let Some(timestamp_ms) = link.timestamp_ms {
            memory.import_link_at(
                link.from.trim(),
                link.relation.trim(),
                link.to.trim(),
                ImportEvidenceContext {
                    source_episode_id,
                    confidence: link.confidence,
                    scope: link.scope.clone(),
                    timestamp_ms,
                },
            )?;
        } else if let Some(scope) = link.scope.clone() {
            memory.add_link_scoped(
                link.from.trim(),
                link.relation.trim(),
                link.to.trim(),
                source_episode_id,
                link.confidence,
                scope,
            )?;
        } else {
            memory.add_link(
                link.from.trim(),
                link.relation.trim(),
                link.to.trim(),
                source_episode_id,
                link.confidence,
            )?;
        }
        report.imported_event_count += 1;
    }
    Ok(())
}

fn import_procedures(
    memory: &mut MemoryEngine,
    document: &MemoryInterchange,
    report: &mut InterchangeImportReport,
    episode_refs: &HashMap<String, String>,
) -> Result<()> {
    for procedure in &document.procedures {
        let source_episode_id = resolve_ref(episode_refs, &procedure.source_episode_ref);
        if let Some(timestamp_ms) = procedure.timestamp_ms {
            memory.import_procedure_at(
                procedure.kind.clone(),
                procedure.name.trim(),
                procedure.body.trim(),
                ImportEvidenceContext {
                    source_episode_id,
                    confidence: procedure.confidence,
                    scope: procedure.scope.clone(),
                    timestamp_ms,
                },
            )?;
        } else {
            import_procedure_without_timestamp(memory, procedure, source_episode_id)?;
        }
        report.imported_event_count += 1;
    }
    Ok(())
}

fn import_procedure_without_timestamp(
    memory: &mut MemoryEngine,
    procedure: &InterchangeProcedure,
    source_episode_id: Option<String>,
) -> Result<()> {
    match &procedure.kind {
        ProcedureKind::Procedure => {
            if let Some(scope) = procedure.scope.clone() {
                memory.add_procedure_scoped(
                    procedure.name.trim(),
                    procedure.body.trim(),
                    source_episode_id,
                    procedure.confidence,
                    scope,
                )?;
            } else {
                memory.add_procedure(
                    procedure.name.trim(),
                    procedure.body.trim(),
                    source_episode_id,
                    procedure.confidence,
                )?;
            }
        }
        ProcedureKind::Preference => {
            if let Some(scope) = procedure.scope.clone() {
                memory.add_preference_scoped(
                    procedure.name.trim(),
                    procedure.body.trim(),
                    source_episode_id,
                    procedure.confidence,
                    scope,
                )?;
            } else {
                memory.add_preference(
                    procedure.name.trim(),
                    procedure.body.trim(),
                    source_episode_id,
                    procedure.confidence,
                )?;
            }
        }
    }
    Ok(())
}

fn import_intentions(
    memory: &mut MemoryEngine,
    document: &MemoryInterchange,
    report: &mut InterchangeImportReport,
    episode_refs: &HashMap<String, String>,
) -> Result<()> {
    for intention in &document.intentions {
        let source_episode_id = resolve_ref(episode_refs, &intention.source_episode_ref);
        let imported = import_intention(memory, intention, source_episode_id)?;
        report.imported_event_count += 1;
        if intention.status != IntentionStatus::Active {
            set_intention_status(memory, intention, imported.id)?;
            report.imported_event_count += 1;
        }
    }
    Ok(())
}

fn import_intention(
    memory: &mut MemoryEngine,
    intention: &InterchangeIntention,
    source_episode_id: Option<String>,
) -> Result<crate::model::Intention> {
    if let Some(timestamp_ms) = intention.timestamp_ms {
        memory.import_intention_at(
            intention.description.trim(),
            intention.kind.clone(),
            intention.priority.clone(),
            source_episode_id,
            intention.scope.clone(),
            timestamp_ms,
        )
    } else if let Some(scope) = intention.scope.clone() {
        memory.add_intention_scoped(
            intention.description.trim(),
            intention.kind.clone(),
            intention.priority.clone(),
            source_episode_id,
            scope,
        )
    } else {
        memory.add_intention(
            intention.description.trim(),
            intention.kind.clone(),
            intention.priority.clone(),
            source_episode_id,
        )
    }
}

fn set_intention_status(
    memory: &mut MemoryEngine,
    intention: &InterchangeIntention,
    id: String,
) -> Result<()> {
    if let Some(timestamp_ms) = intention.status_timestamp_ms.or(intention.timestamp_ms) {
        memory.import_intention_status_at(
            id,
            intention.status.clone(),
            clean_optional(&intention.status_reason),
            timestamp_ms,
        )?;
    } else {
        memory.set_intention_status(
            id,
            intention.status.clone(),
            clean_optional(&intention.status_reason),
        )?;
    }
    Ok(())
}

fn validate_version(document: &MemoryInterchange, issues: &mut Vec<InterchangeIssue>) {
    if document.version != MEMORY_INTERCHANGE_VERSION {
        issues.push(issue(
            InterchangeIssueKind::UnsupportedVersion,
            "version",
            format!(
                "unsupported interchange version {}, supported version is {}",
                document.version, MEMORY_INTERCHANGE_VERSION
            ),
        ));
    }
}

fn validate_sources(
    document: &MemoryInterchange,
    issues: &mut Vec<InterchangeIssue>,
    source_refs: &mut HashSet<String>,
) {
    for (index, source) in document.sources.iter().enumerate() {
        require_text(issues, format!("sources[{index}].ref"), &source.ref_id);
        let ref_id = source.ref_id.trim().to_string();
        if !ref_id.is_empty() && !source_refs.insert(ref_id.clone()) {
            issues.push(issue(
                InterchangeIssueKind::DuplicateReference,
                format!("sources[{index}].ref"),
                format!("duplicate source reference {ref_id}"),
            ));
        }
    }
}

fn validate_episodes(
    document: &MemoryInterchange,
    issues: &mut Vec<InterchangeIssue>,
    source_refs: &HashSet<String>,
    episode_refs: &mut HashSet<String>,
) {
    for (index, episode) in document.episodes.iter().enumerate() {
        require_text(
            issues,
            format!("episodes[{index}].content"),
            &episode.content,
        );
        if let Some(ref_id) = clean_optional(&episode.ref_id)
            && !episode_refs.insert(ref_id.clone())
        {
            issues.push(issue(
                InterchangeIssueKind::DuplicateReference,
                format!("episodes[{index}].ref"),
                format!("duplicate episode reference {ref_id}"),
            ));
        }
        require_known_source_ref(
            issues,
            format!("episodes[{index}].source_ref"),
            &episode.source_ref,
            source_refs,
        );
    }
}

fn validate_claims(
    document: &MemoryInterchange,
    issues: &mut Vec<InterchangeIssue>,
    episode_refs: &HashSet<String>,
) {
    for (index, claim) in document.claims.iter().enumerate() {
        require_text(issues, format!("claims[{index}].subject"), &claim.subject);
        require_text(
            issues,
            format!("claims[{index}].predicate"),
            &claim.predicate,
        );
        require_text(issues, format!("claims[{index}].object"), &claim.object);
        require_known_ref(
            issues,
            format!("claims[{index}].source_episode_ref"),
            &claim.source_episode_ref,
            episode_refs,
        );
    }
}

fn validate_links(
    document: &MemoryInterchange,
    issues: &mut Vec<InterchangeIssue>,
    episode_refs: &HashSet<String>,
) {
    for (index, link) in document.links.iter().enumerate() {
        require_text(issues, format!("links[{index}].from"), &link.from);
        require_text(issues, format!("links[{index}].relation"), &link.relation);
        require_text(issues, format!("links[{index}].to"), &link.to);
        require_known_ref(
            issues,
            format!("links[{index}].source_episode_ref"),
            &link.source_episode_ref,
            episode_refs,
        );
    }
}

fn validate_procedures(
    document: &MemoryInterchange,
    issues: &mut Vec<InterchangeIssue>,
    episode_refs: &HashSet<String>,
) {
    for (index, procedure) in document.procedures.iter().enumerate() {
        require_text(issues, format!("procedures[{index}].name"), &procedure.name);
        require_text(issues, format!("procedures[{index}].body"), &procedure.body);
        require_known_ref(
            issues,
            format!("procedures[{index}].source_episode_ref"),
            &procedure.source_episode_ref,
            episode_refs,
        );
    }
}

fn validate_intentions(
    document: &MemoryInterchange,
    issues: &mut Vec<InterchangeIssue>,
    episode_refs: &HashSet<String>,
) {
    for (index, intention) in document.intentions.iter().enumerate() {
        require_text(
            issues,
            format!("intentions[{index}].description"),
            &intention.description,
        );
        require_known_ref(
            issues,
            format!("intentions[{index}].source_episode_ref"),
            &intention.source_episode_ref,
            episode_refs,
        );
    }
}

fn require_text(issues: &mut Vec<InterchangeIssue>, path: String, value: &str) {
    if value.trim().is_empty() {
        issues.push(issue(
            InterchangeIssueKind::EmptyField,
            path,
            "field cannot be empty",
        ));
    }
}

fn require_known_ref(
    issues: &mut Vec<InterchangeIssue>,
    path: String,
    value: &Option<String>,
    episode_refs: &HashSet<String>,
) {
    if let Some(ref_id) = clean_optional(value)
        && !episode_refs.contains(&ref_id)
    {
        issues.push(issue(
            InterchangeIssueKind::UnknownSourceReference,
            path,
            format!("unknown episode reference {ref_id}"),
        ));
    }
}

fn require_known_source_ref(
    issues: &mut Vec<InterchangeIssue>,
    path: String,
    value: &Option<String>,
    source_refs: &HashSet<String>,
) {
    if let Some(ref_id) = clean_optional(value)
        && !source_refs.contains(&ref_id)
    {
        issues.push(issue(
            InterchangeIssueKind::UnknownSourceDocumentReference,
            path,
            format!("unknown source reference {ref_id}"),
        ));
    }
}

fn issue(
    kind: InterchangeIssueKind,
    path: impl Into<String>,
    message: impl Into<String>,
) -> InterchangeIssue {
    InterchangeIssue {
        kind,
        path: path.into(),
        message: message.into(),
    }
}

fn resolve_ref(episode_refs: &HashMap<String, String>, value: &Option<String>) -> Option<String> {
    clean_optional(value).and_then(|ref_id| episode_refs.get(&ref_id).cloned())
}

fn clean_strings(values: &[String]) -> Vec<String> {
    values
        .iter()
        .filter_map(|value| {
            let value = value.trim().to_string();
            if value.is_empty() { None } else { Some(value) }
        })
        .collect()
}

fn source_checksum(source: &InterchangeSource) -> String {
    clean_optional(&source.content_checksum)
        .unwrap_or_else(|| format!("interchange-source:{}", source.ref_id.trim()))
}
