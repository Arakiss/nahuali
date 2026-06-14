use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    path::Path,
};

use anyhow::{Context, bail};
use nahuali_core::{
    IntentionKind, IntentionPriority, IntentionStatus, InterchangeClaim, InterchangeEpisode,
    InterchangeIntention, InterchangeLink, InterchangeProcedure, InterchangeSource,
    MemoryInterchange, MemoryScope, ProcedureKind, SourceKind,
};
use serde_json::{Map, Value};

use crate::commands::{
    migration_timestamps::timestamp_value,
    migration_values::{
        array_field_any, claim_object, clean_string_value, confidence_field, episode_content,
        episode_tags, intention_description, lifecycle_status, metadata_field, procedure_body,
        projection_object, record_keys, record_ref, source_kind, source_record_keys, source_ref,
        source_uri, stable_source_key, string_array_field, synthetic_source_checksum, text_field,
        text_field_any, u32_value, u64_field_any, value_field_any, value_keys,
    },
    scope::parse_scope,
};
use crate::output;

pub(crate) fn convert_projection_export(
    path: &Path,
    output: &Path,
    scope: Option<String>,
    json: bool,
) -> anyhow::Result<bool> {
    let raw =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let value: Value = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    let scope = parse_scope(scope)?;
    let conversion = convert_projection_value(&value, scope)?;
    emit_conversion_result(
        "projection_export_path",
        "Projection export",
        path,
        output,
        json,
        &conversion,
        None,
    )
}

pub(crate) fn convert_projection_value(
    value: &Value,
    scope: Option<MemoryScope>,
) -> anyhow::Result<ProjectionConversion> {
    ProjectionConversion::from_value(value, scope)
}

pub(crate) fn emit_conversion_result(
    input_path_key: &str,
    input_label: &str,
    input_path: &Path,
    output: &Path,
    json: bool,
    conversion: &ProjectionConversion,
    detected_format: Option<&str>,
) -> anyhow::Result<bool> {
    let encoded = serde_json::to_string_pretty(&conversion.interchange)?;

    if let Some(parent) = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(output, format!("{encoded}\n"))
        .with_context(|| format!("failed to write {}", output.display()))?;

    if json {
        let mut payload = Map::new();
        payload.insert(
            input_path_key.to_string(),
            Value::String(input_path.display().to_string()),
        );
        payload.insert(
            "interchange_path".to_string(),
            Value::String(output.display().to_string()),
        );
        payload.insert("summary".to_string(), conversion.summary());
        payload.insert(
            "issues".to_string(),
            serde_json::to_value(&conversion.issues)?,
        );
        if let Some(detected_format) = detected_format {
            payload.insert(
                "detected_format".to_string(),
                Value::String(detected_format.to_string()),
            );
        }
        output::print_json(&Value::Object(payload))?;
    } else {
        println!("{input_label}: {}", input_path.display());
        println!("Interchange: {}", output.display());
        println!("Status: converted");
        if let Some(detected_format) = detected_format {
            println!("Detected format: {detected_format}");
        }
        println!("Sources: {}", conversion.interchange.sources.len());
        println!("Episodes: {}", conversion.interchange.episodes.len());
        println!("Claims: {}", conversion.interchange.claims.len());
        println!("Links: {}", conversion.interchange.links.len());
        println!("Procedures: {}", conversion.interchange.procedures.len());
        println!("Intentions: {}", conversion.interchange.intentions.len());
        if !conversion.issues.is_empty() {
            println!("Issues:");
            for issue in &conversion.issues {
                println!("- {}: {}", issue.path, issue.message);
            }
        }
    }

    Ok(true)
}

pub(crate) struct ProjectionConversion {
    interchange: MemoryInterchange,
    issues: Vec<ProjectionConversionIssue>,
    source_counts: BTreeMap<&'static str, usize>,
}

impl ProjectionConversion {
    fn from_value(value: &Value, scope: Option<MemoryScope>) -> anyhow::Result<Self> {
        let object = value
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("migration export must be a JSON object"))?;
        let object = projection_object(object);
        let mut converter = ProjectionConverter::new(scope);

        converter.convert_sources(array_field_any(
            object,
            &["sources", "source", "sourceDocuments", "documents"],
        ));
        converter.index_entities(array_field_any(object, &["entities", "entity"]));
        converter.convert_episodes(array_field_any(object, &["episodes", "episode"]));
        converter.convert_entities();
        converter.convert_relations(array_field_any(object, &["relations", "relates_to"]));
        converter.convert_procedures(array_field_any(object, &["procedures", "procedure"]));
        converter.convert_intentions(array_field_any(object, &["intentions", "intention"]));

        let conversion = converter.finish();
        if conversion.interchange.episodes.is_empty()
            && conversion.interchange.claims.is_empty()
            && conversion.interchange.links.is_empty()
            && conversion.interchange.procedures.is_empty()
            && conversion.interchange.intentions.is_empty()
        {
            bail!("migration export did not contain convertible memory records");
        }
        Ok(conversion)
    }

    pub(crate) fn summary(&self) -> Value {
        serde_json::json!({
            "source_counts": self.source_counts,
            "source_count": self.interchange.sources.len(),
            "episode_count": self.interchange.episodes.len(),
            "claim_count": self.interchange.claims.len(),
            "link_count": self.interchange.links.len(),
            "procedure_count": self.interchange.procedures.len(),
            "intention_count": self.interchange.intentions.len(),
            "issue_count": self.issues.len(),
        })
    }
}

#[derive(serde::Serialize)]
struct ProjectionConversionIssue {
    path: String,
    message: String,
}

const EPISODE_TIMESTAMPS: &[&str] = &["timestamp_ms", "timestamp", "created_at_ms", "createdAt"];
const RECORD_TIMESTAMPS: &[&str] = &["timestamp_ms", "created_at_ms", "createdAt", "timestamp"];
const STATUS_TIMESTAMPS: &[&str] = &["status_timestamp_ms", "completedAt", "updatedAt"];

struct ProjectionConverter {
    interchange: MemoryInterchange,
    issues: Vec<ProjectionConversionIssue>,
    source_counts: BTreeMap<&'static str, usize>,
    entity_names: HashMap<String, String>,
    entity_source_refs: HashMap<String, String>,
    source_refs: HashMap<String, String>,
    entities: Vec<Value>,
    scope: Option<MemoryScope>,
}

impl ProjectionConverter {
    fn new(scope: Option<MemoryScope>) -> Self {
        Self {
            interchange: MemoryInterchange::default(),
            issues: Vec::new(),
            source_counts: BTreeMap::new(),
            entity_names: HashMap::new(),
            entity_source_refs: HashMap::new(),
            source_refs: HashMap::new(),
            entities: Vec::new(),
            scope,
        }
    }

    fn convert_sources(&mut self, sources: &[Value]) {
        self.source_counts.insert("sources", sources.len());
        for (index, source) in sources.iter().enumerate() {
            let Some(ref_id) = source_ref(source) else {
                self.issue(
                    format!("sources[{index}].ref"),
                    "source reference is missing",
                );
                continue;
            };
            if self.source_refs.contains_key(&ref_id) {
                continue;
            }
            let timestamp_ms =
                self.record_timestamp(source, &format!("sources[{index}]"), RECORD_TIMESTAMPS);
            let source = InterchangeSource {
                ref_id: ref_id.clone(),
                kind: source_kind(source),
                title: text_field_any(source, &["title", "name", "summary", "source"]),
                uri: text_field_any(source, &["uri", "url", "path", "locator"]),
                content_checksum: text_field_any(
                    source,
                    &["content_checksum", "contentChecksum", "checksum", "hash"],
                )
                .or_else(|| Some(synthetic_source_checksum(&ref_id))),
                byte_len: u64_field_any(source, &["byte_len", "byteLen", "contentLength", "size"])
                    .unwrap_or_default(),
                metadata: metadata_field(source),
                scope: self.scope.clone(),
                timestamp_ms,
            };
            self.register_source_keys(&source.ref_id, source_record_keys(&source));
            self.interchange.sources.push(source);
        }
    }

    fn index_entities(&mut self, entities: &[Value]) {
        self.source_counts.insert("entities", entities.len());
        self.entities = entities.to_vec();
        for entity in entities {
            let Some(name) = text_field(entity, "name") else {
                continue;
            };
            for key in record_keys(entity) {
                self.entity_names.insert(key, name.clone());
            }
        }
    }

    fn convert_episodes(&mut self, episodes: &[Value]) {
        self.source_counts.insert("episodes", episodes.len());
        for (index, episode) in episodes.iter().enumerate() {
            let Some(content) = episode_content(episode) else {
                self.issue(
                    format!("episodes[{index}]"),
                    "episode has no summary or content",
                );
                continue;
            };
            let tags = episode_tags(episode);
            let ref_id = record_ref(episode);
            let mentions = self.episode_mentions(episode);
            let source_ref = self.episode_source_ref(episode);
            let source_position = self.source_position(episode, &format!("episodes[{index}]"));
            let timestamp_ms =
                self.record_timestamp(episode, &format!("episodes[{index}]"), EPISODE_TIMESTAMPS);
            if let Some(ref_id) = &ref_id {
                for mention in &mentions {
                    self.entity_source_refs
                        .entry(mention.clone())
                        .or_insert_with(|| ref_id.clone());
                }
            }
            self.interchange.episodes.push(InterchangeEpisode {
                ref_id,
                content,
                tags,
                mentions,
                source_role: text_field_any(episode, &["sourceRole", "source_role", "operator"]),
                source_ref,
                source_position,
                scope: self.scope.clone(),
                timestamp_ms,
            });
        }
    }

    fn convert_entities(&mut self) {
        let entities = self.entities.clone();
        for (index, entity) in entities.iter().enumerate() {
            let Some(name) = text_field(entity, "name") else {
                self.issue(format!("entities[{index}].name"), "entity name is missing");
                continue;
            };
            let source_episode_ref = self.entity_source_refs.get(&name).cloned();
            let timestamp_ms =
                self.record_timestamp(entity, &format!("entities[{index}]"), RECORD_TIMESTAMPS);

            if let Some(entity_type) = text_field(entity, "type") {
                self.add_claim(
                    &name,
                    "type",
                    entity_type,
                    source_episode_ref.clone(),
                    timestamp_ms,
                );
            }
            for alias in string_array_field(entity, "aliases") {
                self.add_claim(
                    &name,
                    "alias",
                    alias,
                    source_episode_ref.clone(),
                    timestamp_ms,
                );
            }
            if let Some(attributes) = entity.get("attributes").and_then(Value::as_object) {
                for (key, value) in attributes {
                    if let Some(object) = claim_object(value) {
                        self.add_claim(
                            &name,
                            key,
                            object,
                            source_episode_ref.clone(),
                            timestamp_ms,
                        );
                    }
                }
            }
        }
    }

    fn convert_relations(&mut self, relations: &[Value]) {
        self.source_counts.insert("relations", relations.len());
        for (index, relation) in relations.iter().enumerate() {
            let from_value = value_field_any(
                relation,
                &["in", "from", "fromEntity", "source", "sourceEntity"],
            );
            let to_value = value_field_any(relation, &["out", "to", "toEntity", "target"]);
            let Some(from) = from_value.and_then(|value| self.resolve_entity_ref(value)) else {
                self.issue(
                    format!("relations[{index}].from"),
                    "relation source is unresolved",
                );
                continue;
            };
            let Some(to) = to_value.and_then(|value| self.resolve_entity_ref(value)) else {
                self.issue(
                    format!("relations[{index}].to"),
                    "relation target is unresolved",
                );
                continue;
            };
            let relation_name = text_field(relation, "customType")
                .or_else(|| text_field(relation, "relationType"))
                .or_else(|| text_field(relation, "type"))
                .unwrap_or_else(|| "related_to".to_string());
            let timestamp_ms =
                self.record_timestamp(relation, &format!("relations[{index}]"), RECORD_TIMESTAMPS);

            self.interchange.links.push(InterchangeLink {
                source_episode_ref: self
                    .entity_source_refs
                    .get(&from)
                    .or_else(|| self.entity_source_refs.get(&to))
                    .cloned(),
                from,
                relation: relation_name,
                to,
                confidence: confidence_field(relation),
                scope: self.scope.clone(),
                timestamp_ms,
            });
        }
    }

    fn convert_procedures(&mut self, procedures: &[Value]) {
        self.source_counts.insert("procedures", procedures.len());
        for (index, procedure) in procedures.iter().enumerate() {
            let Some(name) = text_field(procedure, "name") else {
                self.issue(
                    format!("procedures[{index}].name"),
                    "procedure name is missing",
                );
                continue;
            };
            let Some(body) = procedure_body(procedure) else {
                self.issue(
                    format!("procedures[{index}].body"),
                    "procedure body is empty",
                );
                continue;
            };
            let timestamp_ms = self.record_timestamp(
                procedure,
                &format!("procedures[{index}]"),
                RECORD_TIMESTAMPS,
            );
            self.interchange.procedures.push(InterchangeProcedure {
                kind: ProcedureKind::Procedure,
                name,
                body,
                source_episode_ref: None,
                confidence: confidence_field(procedure),
                scope: self.scope.clone(),
                timestamp_ms,
            });
        }
    }

    fn convert_intentions(&mut self, intentions: &[Value]) {
        self.source_counts.insert("intentions", intentions.len());
        for (index, intention) in intentions.iter().enumerate() {
            let Some(description) = intention_description(intention) else {
                self.issue(
                    format!("intentions[{index}].description"),
                    "intention description is missing",
                );
                continue;
            };
            let timestamp_ms = self.record_timestamp(
                intention,
                &format!("intentions[{index}]"),
                RECORD_TIMESTAMPS,
            );
            let status_timestamp_ms = self.record_timestamp(
                intention,
                &format!("intentions[{index}]"),
                STATUS_TIMESTAMPS,
            );
            self.interchange.intentions.push(InterchangeIntention {
                kind: intention_kind(text_field(intention, "type").as_deref()),
                priority: intention_priority(intention),
                status: intention_status(lifecycle_status(intention).as_deref()),
                description,
                source_episode_ref: None,
                status_reason: status_reason(intention),
                scope: self.scope.clone(),
                timestamp_ms,
                status_timestamp_ms,
            });
        }
    }

    fn episode_mentions(&self, episode: &Value) -> Vec<String> {
        let mut mentions = BTreeSet::new();
        for key in ["entities", "entityNames", "mentions"] {
            if let Some(values) = episode.get(key).and_then(Value::as_array) {
                for value in values {
                    if let Some(name) = self.resolve_entity_ref(value) {
                        mentions.insert(name);
                    }
                }
            }
        }
        mentions.into_iter().collect()
    }

    fn resolve_entity_ref(&self, value: &Value) -> Option<String> {
        if let Some(name) = value.get("name").and_then(clean_string_value) {
            return Some(name);
        }
        if let Some(name) =
            value_field_any(value, &["entityName", "label"]).and_then(clean_string_value)
        {
            return Some(name);
        }
        if let Some(entity_id) = value.get("entityId").and_then(clean_string_value) {
            return self
                .entity_names
                .get(&entity_id)
                .cloned()
                .or(Some(entity_id));
        }
        if let Some(entity_id) = value.get("entityId") {
            for key in value_keys(entity_id) {
                if let Some(name) = self.entity_names.get(&key) {
                    return Some(name.clone());
                }
            }
        }
        if let Some(id) = value.get("id") {
            for key in value_keys(id) {
                if let Some(name) = self.entity_names.get(&key) {
                    return Some(name.clone());
                }
            }
        }
        if let Some(text) = clean_string_value(value) {
            return self.entity_names.get(&text).cloned().or(Some(text));
        }
        None
    }

    fn episode_source_ref(&mut self, episode: &Value) -> Option<String> {
        for (key, kind) in [
            ("sourceRef", SourceKind::Other),
            ("source_ref", SourceKind::Other),
            ("sourceId", SourceKind::Other),
            ("source_id", SourceKind::Other),
            ("source", SourceKind::Other),
            ("conversationId", SourceKind::Conversation),
            ("conversation_id", SourceKind::Conversation),
        ] {
            if let Some(value) = episode.get(key)
                && let Some(ref_id) = self.resolve_source_ref(value, kind)
            {
                return Some(ref_id);
            }
        }
        None
    }

    fn resolve_source_ref(&mut self, value: &Value, kind: SourceKind) -> Option<String> {
        for key in value_keys(value) {
            if let Some(ref_id) = self.source_refs.get(&key) {
                return Some(ref_id.clone());
            }
        }
        clean_string_value(value).and_then(|label| self.ensure_synthetic_source(label, kind))
    }

    fn ensure_synthetic_source(&mut self, label: String, kind: SourceKind) -> Option<String> {
        let label = label.trim().to_string();
        if label.is_empty() {
            return None;
        }
        if let Some(ref_id) = self.source_refs.get(&label) {
            return Some(ref_id.clone());
        }
        let ref_id = format!("source:{}", stable_source_key(&label));
        if !self.source_refs.contains_key(&ref_id) {
            self.interchange.sources.push(InterchangeSource {
                ref_id: ref_id.clone(),
                kind,
                title: Some(label.clone()),
                uri: source_uri(&label),
                content_checksum: Some(synthetic_source_checksum(&ref_id)),
                byte_len: 0,
                metadata: BTreeMap::new(),
                scope: self.scope.clone(),
                timestamp_ms: None,
            });
        }
        self.register_source_keys(&ref_id, vec![label, ref_id.clone()]);
        Some(ref_id)
    }

    fn register_source_keys(&mut self, ref_id: &str, keys: Vec<String>) {
        for key in keys {
            if !key.trim().is_empty() {
                self.source_refs
                    .entry(key.trim().to_string())
                    .or_insert_with(|| ref_id.to_string());
            }
        }
    }

    fn source_position(&mut self, episode: &Value, path: &str) -> Option<u32> {
        for key in [
            "sourcePosition",
            "source_position",
            "position",
            "messageIndex",
            "index",
        ] {
            if let Some(value) = episode.get(key) {
                return match u32_value(value) {
                    Some(position) => Some(position),
                    None => {
                        self.issue(
                            format!("{path}.{key}"),
                            "source position must be an integer",
                        );
                        None
                    }
                };
            }
        }
        None
    }

    fn add_claim(
        &mut self,
        subject: &str,
        predicate: &str,
        object: String,
        source_episode_ref: Option<String>,
        timestamp_ms: Option<u64>,
    ) {
        self.interchange.claims.push(InterchangeClaim {
            subject: subject.to_string(),
            predicate: predicate.to_string(),
            object,
            source_episode_ref,
            confidence: 0.8,
            scope: self.scope.clone(),
            timestamp_ms,
        });
    }

    fn record_timestamp(&mut self, record: &Value, path: &str, keys: &[&str]) -> Option<u64> {
        for key in keys {
            if let Some(value) = record.get(*key) {
                return match timestamp_value(value) {
                    Ok(timestamp_ms) => timestamp_ms,
                    Err(message) => {
                        self.issue(format!("{path}.{key}"), &message);
                        None
                    }
                };
            }
        }
        None
    }

    fn issue(&mut self, path: String, message: &str) {
        self.issues.push(ProjectionConversionIssue {
            path,
            message: message.to_string(),
        });
    }

    fn finish(self) -> ProjectionConversion {
        ProjectionConversion {
            interchange: self.interchange,
            issues: self.issues,
            source_counts: self.source_counts,
        }
    }
}

fn status_reason(intention: &Value) -> Option<String> {
    let status = lifecycle_status(intention)?;
    (status != "active").then(|| "Migrated from projected state".to_string())
}

fn intention_kind(value: Option<&str>) -> IntentionKind {
    match value.unwrap_or_default() {
        "goal" | "milestone" | "project" | "objective" => IntentionKind::Goal,
        "reminder" | "deadline" | "habit" => IntentionKind::Reminder,
        _ => IntentionKind::Task,
    }
}

fn intention_priority(record: &Value) -> IntentionPriority {
    if let Some(value) = text_field(record, "priority") {
        return match value.as_str() {
            "critical" => IntentionPriority::Critical,
            "high" => IntentionPriority::High,
            "low" => IntentionPriority::Low,
            _ => IntentionPriority::Medium,
        };
    }
    match record
        .get("importance")
        .and_then(Value::as_f64)
        .unwrap_or_default()
    {
        value if value >= 0.9 => IntentionPriority::Critical,
        value if value >= 0.7 => IntentionPriority::High,
        value if value > 0.0 && value <= 0.3 => IntentionPriority::Low,
        _ => IntentionPriority::Medium,
    }
}

fn intention_status(value: Option<&str>) -> IntentionStatus {
    match value.unwrap_or_default() {
        "done" | "completed" => IntentionStatus::Completed,
        "cancelled" | "canceled" | "abandoned" => IntentionStatus::Abandoned,
        "blocked" => IntentionStatus::Blocked,
        "paused" | "deferred" => IntentionStatus::Deferred,
        _ => IntentionStatus::Active,
    }
}
