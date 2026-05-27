use std::collections::{BTreeMap, BTreeSet};

use nahuali_core::{InterchangeSource, SourceKind};
use serde_json::Value;

pub(super) fn projection_object(
    object: &serde_json::Map<String, Value>,
) -> &serde_json::Map<String, Value> {
    ["data", "memory", "projection"]
        .iter()
        .find_map(|key| object.get(*key).and_then(Value::as_object))
        .unwrap_or(object)
}

pub(super) fn array_field_any<'a>(
    object: &'a serde_json::Map<String, Value>,
    keys: &[&str],
) -> &'a [Value] {
    keys.iter()
        .find_map(|key| {
            object
                .get(*key)
                .and_then(Value::as_array)
                .map(Vec::as_slice)
        })
        .unwrap_or(&[])
}

pub(super) fn record_keys(record: &Value) -> Vec<String> {
    let mut keys = Vec::new();
    for key in ["id", "recordId", "entityId"] {
        if let Some(id) = record.get(key) {
            keys.extend(value_keys(id));
        }
    }
    keys
}

pub(super) fn value_keys(value: &Value) -> Vec<String> {
    let mut keys = Vec::new();
    if let Some(text) = clean_string_value(value) {
        keys.push(text.clone());
        if let Some((table, id)) = text.split_once(':') {
            keys.push(id.to_string());
            keys.push(format!("{table}:{id}"));
        }
        return keys;
    }
    if let Some(object) = value.as_object()
        && let Some(id) = object.get("id").and_then(clean_string_value)
    {
        if let Some(table) = value_field_any(value, &["tb", "table"]).and_then(clean_string_value) {
            keys.push(format!("{table}:{id}"));
        }
        keys.push(id.clone());
    }
    keys
}

pub(super) fn record_ref(record: &Value) -> Option<String> {
    record
        .get("id")
        .and_then(|id| value_keys(id).into_iter().next())
        .or_else(|| text_field(record, "ref"))
}

pub(super) fn source_ref(record: &Value) -> Option<String> {
    record_ref(record)
        .or_else(|| text_field_any(record, &["ref", "source", "sourceId", "source_id"]))
        .or_else(|| text_field_any(record, &["uri", "url", "path", "title", "name"]))
}

pub(super) fn source_record_keys(record: &InterchangeSource) -> Vec<String> {
    let mut keys = vec![record.ref_id.clone()];
    if let Some(title) = &record.title {
        keys.push(title.clone());
    }
    if let Some(uri) = &record.uri {
        keys.push(uri.clone());
    }
    keys
}

pub(super) fn text_field(record: &Value, key: &str) -> Option<String> {
    record.get(key).and_then(clean_string_value)
}

pub(super) fn text_field_any(record: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| text_field(record, key))
}

pub(super) fn value_field_any<'a>(record: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| record.get(*key))
}

pub(super) fn clean_string_value(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub(super) fn u32_value(value: &Value) -> Option<u32> {
    value
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .or_else(|| clean_string_value(value).and_then(|value| value.parse::<u32>().ok()))
}

pub(super) fn u64_field_any(record: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| {
        record.get(*key).and_then(|value| {
            value
                .as_u64()
                .or_else(|| clean_string_value(value).and_then(|value| value.parse::<u64>().ok()))
        })
    })
}

pub(super) fn string_array_field(record: &Value, key: &str) -> Vec<String> {
    let Some(value) = record.get(key) else {
        return Vec::new();
    };
    string_list_value(value)
}

pub(super) fn string_list_value(value: &Value) -> Vec<String> {
    if let Some(text) = clean_string_value(value) {
        return vec![text];
    }
    value
        .as_array()
        .map(|values| values.iter().filter_map(clean_string_value).collect())
        .unwrap_or_default()
}

pub(super) fn episode_tags(episode: &Value) -> Vec<String> {
    let mut tags = BTreeSet::new();
    for key in ["tags", "emotions"] {
        for value in string_array_field(episode, key) {
            tags.insert(value);
        }
    }
    tags.into_iter().collect()
}

pub(super) fn episode_content(episode: &Value) -> Option<String> {
    let summary = text_field(episode, "summary").or_else(|| text_field(episode, "title"));
    let content = text_field(episode, "content")
        .or_else(|| text_field(episode, "body"))
        .or_else(|| text_field(episode, "text"));
    match (summary, content) {
        (Some(summary), Some(content)) if summary != content => {
            Some(format!("{summary}\n\n{content}"))
        }
        (Some(summary), _) => Some(summary),
        (_, Some(content)) => Some(content),
        _ => None,
    }
}

pub(super) fn claim_object(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(text) => {
            let trimmed = text.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        Value::Array(values) => {
            let items = values.iter().filter_map(claim_object).collect::<Vec<_>>();
            (!items.is_empty()).then(|| items.join(", "))
        }
        Value::Object(_) => Some(value.to_string()),
    }
}

pub(super) fn confidence_field(record: &Value) -> f32 {
    record
        .get("confidence")
        .and_then(Value::as_f64)
        .unwrap_or(0.8)
        .clamp(0.0, 1.0) as f32
}

pub(super) fn source_kind(record: &Value) -> SourceKind {
    match text_field_any(record, &["kind", "type", "sourceKind", "source_kind"])
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "document" | "doc" => SourceKind::Document,
        "conversation" | "chat" => SourceKind::Conversation,
        "transcript" | "meeting" => SourceKind::Transcript,
        "web_page" | "webpage" | "web" | "url" => SourceKind::WebPage,
        "note" => SourceKind::Note,
        _ => SourceKind::Other,
    }
}

pub(super) fn metadata_field(record: &Value) -> BTreeMap<String, String> {
    let Some(metadata) = record.get("metadata").and_then(Value::as_object) else {
        return BTreeMap::new();
    };
    metadata
        .iter()
        .filter_map(|(key, value)| {
            let key = key.trim().to_string();
            let value = claim_object(value)?;
            if key.is_empty() || value.trim().is_empty() {
                None
            } else {
                Some((key, value))
            }
        })
        .collect()
}

pub(super) fn source_uri(label: &str) -> Option<String> {
    let label = label.trim();
    (label.contains("://") || label.starts_with("file:")).then(|| label.to_string())
}

pub(super) fn stable_source_key(label: &str) -> String {
    let key = label
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    if key.is_empty() {
        "source".to_string()
    } else {
        key
    }
}

pub(super) fn synthetic_source_checksum(ref_id: &str) -> String {
    format!("projected-source:{}", stable_source_key(ref_id))
}

pub(super) fn procedure_body(procedure: &Value) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(category) = text_field(procedure, "category") {
        parts.push(format!("Category: {category}"));
    }
    if let Some(priority) = procedure.get("priority").and_then(claim_object) {
        parts.push(format!("Priority: {priority}"));
    }
    if let Some(description) = text_field(procedure, "description") {
        parts.push(description);
    }
    append_list(&mut parts, "Rules", &string_array_field(procedure, "rules"));
    append_list(
        &mut parts,
        "Anti-patterns",
        &string_array_field(procedure, "antiPatterns"),
    );
    append_list(
        &mut parts,
        "Entity scope",
        &string_array_field(procedure, "entityScope"),
    );
    append_list(
        &mut parts,
        "Context scope",
        &string_array_field(procedure, "contextScope"),
    );
    append_record_list(&mut parts, "Triggers", procedure.get("triggers"));
    append_record_list(&mut parts, "Examples", procedure.get("examples"));
    let body = parts.join("\n\n");
    (!body.trim().is_empty()).then_some(body)
}

pub(super) fn intention_description(intention: &Value) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(description) = text_field(intention, "description")
        .or_else(|| text_field(intention, "summary"))
        .or_else(|| text_field(intention, "title"))
    {
        parts.push(description);
    }
    if let Some(context) = text_field(intention, "context") {
        parts.push(format!("Context: {context}"));
    }
    if let Some(target) = text_field(intention, "targetDate") {
        parts.push(format!("Target date: {target}"));
    }
    if let Some(reminder) = text_field(intention, "reminderDate") {
        parts.push(format!("Reminder date: {reminder}"));
    }
    append_list(&mut parts, "Notes", &string_array_field(intention, "notes"));
    append_list(&mut parts, "Tags", &string_array_field(intention, "tags"));
    append_list(
        &mut parts,
        "Entity names",
        &string_array_field(intention, "entityNames"),
    );
    append_list(
        &mut parts,
        "Dependencies",
        &string_array_field(intention, "dependencies"),
    );
    let body = parts.join("\n\n");
    (!body.trim().is_empty()).then_some(body)
}

pub(super) fn lifecycle_status(record: &Value) -> Option<String> {
    text_field(record, "state").or_else(|| text_field(record, "status"))
}

fn append_list(parts: &mut Vec<String>, title: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }
    let lines = values
        .iter()
        .map(|value| format!("- {value}"))
        .collect::<Vec<_>>()
        .join("\n");
    parts.push(format!("{title}:\n{lines}"));
}

fn append_record_list(parts: &mut Vec<String>, title: &str, value: Option<&Value>) {
    let Some(values) = value.and_then(Value::as_array) else {
        return;
    };
    let lines = values
        .iter()
        .filter_map(claim_object)
        .map(|value| format!("- {value}"))
        .collect::<Vec<_>>()
        .join("\n");
    if !lines.is_empty() {
        parts.push(format!("{title}:\n{lines}"));
    }
}
