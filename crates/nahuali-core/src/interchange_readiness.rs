use std::collections::HashMap;

use crate::{
    model::{Claim, Episode, Intention, Link, MemoryData, Procedure, SourceDocument},
    self_inspection,
};

use super::{InterchangeImportReadiness, MemoryInterchange, clean_optional};

const FORECAST_NOW_MS: u64 = 0;

pub(super) fn forecast(document: &MemoryInterchange) -> InterchangeImportReadiness {
    let data = projected_data(document);
    let report = self_inspection::self_inspect_at(&data, FORECAST_NOW_MS);

    InterchangeImportReadiness {
        self_inspection_summary: report.summary,
        review_item_count: report.review_queue.len(),
        write_back_policy: report.write_back_policy,
    }
}

fn projected_data(document: &MemoryInterchange) -> MemoryData {
    let source_refs = source_refs(document);
    let episode_refs = episode_refs(document);
    let claims = claims(document, &episode_refs);
    let links = links(document, &episode_refs);

    MemoryData {
        event_count: event_count(document),
        sources: sources(document),
        episodes: episodes(document, &source_refs),
        facts: claims.clone(),
        relations: links.clone(),
        claims,
        links,
        procedures: procedures(document, &episode_refs),
        intentions: intentions(document, &episode_refs),
        ..MemoryData::default()
    }
}

fn source_refs(document: &MemoryInterchange) -> HashMap<String, String> {
    document
        .sources
        .iter()
        .enumerate()
        .filter_map(|(index, source)| {
            clean_optional(&Some(source.ref_id.clone())).map(|ref_id| (ref_id, source_id(index)))
        })
        .collect()
}

fn episode_refs(document: &MemoryInterchange) -> HashMap<String, String> {
    document
        .episodes
        .iter()
        .enumerate()
        .filter_map(|(index, episode)| {
            clean_optional(&episode.ref_id).map(|ref_id| (ref_id, episode_id(index)))
        })
        .collect()
}

fn sources(document: &MemoryInterchange) -> Vec<SourceDocument> {
    document
        .sources
        .iter()
        .enumerate()
        .map(|(index, source)| SourceDocument {
            id: source_id(index),
            event_id: event_id(index),
            kind: source.kind.clone(),
            title: source.title.clone(),
            uri: source.uri.clone(),
            content_checksum: source
                .content_checksum
                .clone()
                .unwrap_or_else(|| format!("forecast_source_checksum_{}", index + 1)),
            byte_len: source.byte_len,
            metadata: source.metadata.clone(),
            scope: source.scope.clone(),
            created_at_ms: source.timestamp_ms.unwrap_or(FORECAST_NOW_MS),
        })
        .collect()
}

fn episodes(document: &MemoryInterchange, source_refs: &HashMap<String, String>) -> Vec<Episode> {
    document
        .episodes
        .iter()
        .enumerate()
        .map(|(index, episode)| Episode {
            id: episode_id(index),
            event_id: event_id(document.sources.len() + index),
            content: episode.content.trim().to_string(),
            tags: clean_strings(&episode.tags),
            mentions: clean_strings(&episode.mentions),
            source_id: clean_optional(&episode.source_ref)
                .and_then(|ref_id| source_refs.get(&ref_id).cloned()),
            source_position: episode.source_position,
            source_role: episode.source_role.clone(),
            scope: episode.scope.clone(),
            created_at_ms: episode.timestamp_ms.unwrap_or(FORECAST_NOW_MS),
        })
        .collect()
}

fn claims(document: &MemoryInterchange, episode_refs: &HashMap<String, String>) -> Vec<Claim> {
    let offset = document.sources.len() + document.episodes.len();
    document
        .claims
        .iter()
        .enumerate()
        .map(|(index, claim)| Claim {
            id: format!("forecast_claim_{}", index + 1),
            event_id: event_id(offset + index),
            subject: claim.subject.trim().to_string(),
            predicate: claim.predicate.trim().to_string(),
            object: claim.object.trim().to_string(),
            source_episode_id: resolve_episode_ref(&claim.source_episode_ref, episode_refs),
            confidence: claim.confidence,
            scope: claim.scope.clone(),
            created_at_ms: claim.timestamp_ms.unwrap_or(FORECAST_NOW_MS),
        })
        .collect()
}

fn links(document: &MemoryInterchange, episode_refs: &HashMap<String, String>) -> Vec<Link> {
    let offset = document.sources.len() + document.episodes.len() + document.claims.len();
    document
        .links
        .iter()
        .enumerate()
        .map(|(index, link)| Link {
            id: format!("forecast_link_{}", index + 1),
            event_id: event_id(offset + index),
            from: link.from.trim().to_string(),
            relation: link.relation.trim().to_string(),
            to: link.to.trim().to_string(),
            source_episode_id: resolve_episode_ref(&link.source_episode_ref, episode_refs),
            confidence: link.confidence,
            scope: link.scope.clone(),
            created_at_ms: link.timestamp_ms.unwrap_or(FORECAST_NOW_MS),
        })
        .collect()
}

fn procedures(
    document: &MemoryInterchange,
    episode_refs: &HashMap<String, String>,
) -> Vec<Procedure> {
    let offset = document.sources.len()
        + document.episodes.len()
        + document.claims.len()
        + document.links.len();
    document
        .procedures
        .iter()
        .enumerate()
        .map(|(index, procedure)| Procedure {
            id: format!("forecast_procedure_{}", index + 1),
            event_id: event_id(offset + index),
            kind: procedure.kind.clone(),
            name: procedure.name.trim().to_string(),
            body: procedure.body.trim().to_string(),
            source_episode_id: resolve_episode_ref(&procedure.source_episode_ref, episode_refs),
            confidence: procedure.confidence,
            scope: procedure.scope.clone(),
            created_at_ms: procedure.timestamp_ms.unwrap_or(FORECAST_NOW_MS),
        })
        .collect()
}

fn intentions(
    document: &MemoryInterchange,
    episode_refs: &HashMap<String, String>,
) -> Vec<Intention> {
    let offset = document.sources.len()
        + document.episodes.len()
        + document.claims.len()
        + document.links.len()
        + document.procedures.len();
    document
        .intentions
        .iter()
        .enumerate()
        .map(|(index, intention)| {
            let created_at_ms = intention.timestamp_ms.unwrap_or(FORECAST_NOW_MS);
            Intention {
                id: format!("forecast_intention_{}", index + 1),
                event_id: event_id(offset + index),
                updated_event_id: event_id(offset + index),
                kind: intention.kind.clone(),
                status: intention.status.clone(),
                priority: intention.priority.clone(),
                description: intention.description.trim().to_string(),
                source_episode_id: resolve_episode_ref(&intention.source_episode_ref, episode_refs),
                status_reason: intention.status_reason.clone(),
                deadline_at_ms: None,
                depends_on: Vec::new(),
                goal_id: None,
                progress_percent: None,
                scope: intention.scope.clone(),
                created_at_ms,
                updated_at_ms: intention.status_timestamp_ms.unwrap_or(created_at_ms),
            }
        })
        .collect()
}

fn resolve_episode_ref(
    source_episode_ref: &Option<String>,
    episode_refs: &HashMap<String, String>,
) -> Option<String> {
    clean_optional(source_episode_ref).and_then(|ref_id| episode_refs.get(&ref_id).cloned())
}

fn clean_strings(values: &[String]) -> Vec<String> {
    values
        .iter()
        .filter_map(|value| {
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_string())
        })
        .collect()
}

fn event_count(document: &MemoryInterchange) -> usize {
    document.sources.len()
        + document.episodes.len()
        + document.claims.len()
        + document.links.len()
        + document.procedures.len()
        + document.intentions.len()
}

fn source_id(index: usize) -> String {
    format!("forecast_source_{}", index + 1)
}

fn episode_id(index: usize) -> String {
    format!("forecast_episode_{}", index + 1)
}

fn event_id(index: usize) -> String {
    format!("forecast_event_{}", index + 1)
}
