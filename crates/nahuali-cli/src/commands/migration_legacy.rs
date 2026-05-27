use std::{fs, path::Path};

use anyhow::{Context, bail};
use serde_json::{Map, Value};

use crate::commands::scope::parse_scope;

use super::migration::{convert_projection_value, emit_conversion_result};

#[derive(Clone, Copy)]
enum LegacyExportFormat {
    Json,
    Surql,
}

impl LegacyExportFormat {
    fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Surql => "surql",
        }
    }
}

pub(crate) fn convert_legacy_export(
    path: &Path,
    output: &Path,
    scope: Option<String>,
    json: bool,
) -> anyhow::Result<bool> {
    let raw =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let (value, format) =
        load_legacy_export(&raw).with_context(|| format!("failed to decode {}", path.display()))?;
    let scope = parse_scope(scope)?;
    let conversion = convert_projection_value(&value, scope)?;

    emit_conversion_result(
        "legacy_export_path",
        "Legacy export",
        path,
        output,
        json,
        &conversion,
        Some(format.as_str()),
    )
}

fn load_legacy_export(raw: &str) -> anyhow::Result<(Value, LegacyExportFormat)> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        bail!("legacy export is empty");
    }

    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return Ok((value, LegacyExportFormat::Json));
    }

    Ok((parse_surql_export(trimmed)?, LegacyExportFormat::Surql))
}

fn parse_surql_export(raw: &str) -> anyhow::Result<Value> {
    let mut exported_at = None;
    let mut object = Map::new();
    let mut statement_count = 0usize;

    for (index, line) in raw.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(timestamp) = line.strip_prefix("-- Nahuali Export ") {
            let timestamp = timestamp.trim();
            if !timestamp.is_empty() {
                exported_at = Some(timestamp.to_string());
            }
            continue;
        }
        if line.starts_with("--") {
            continue;
        }

        let Some(rest) = line.strip_prefix("INSERT INTO ") else {
            bail!(
                "unsupported legacy SurrealQL statement at line {}: expected `INSERT INTO`",
                index + 1
            );
        };
        let Some(separator) = rest.find(char::is_whitespace) else {
            bail!(
                "unsupported legacy SurrealQL statement at line {}: missing table separator",
                index + 1
            );
        };
        let (table, payload) = rest.split_at(separator);
        let payload = payload.trim_start();
        let Some(payload) = payload.strip_suffix(';') else {
            bail!(
                "unsupported legacy SurrealQL statement at line {}: missing trailing semicolon",
                index + 1
            );
        };
        let record: Value = serde_json::from_str(payload).with_context(|| {
            format!(
                "legacy SurrealQL payload at line {} is not valid JSON",
                index + 1
            )
        })?;

        let collection_key = match table {
            "entity" => "entities",
            "episode" => "episodes",
            "relates_to" | "relation" => "relations",
            "procedure" => "procedures",
            "intention" => "intentions",
            _ => {
                bail!(
                    "unsupported legacy SurrealQL table `{table}` at line {}",
                    index + 1
                )
            }
        };

        object
            .entry(collection_key.to_string())
            .or_insert_with(|| Value::Array(Vec::new()))
            .as_array_mut()
            .expect("legacy export collections are arrays")
            .push(record);
        statement_count += 1;
    }

    if statement_count == 0 {
        bail!("legacy SurrealQL export did not contain convertible INSERT statements");
    }

    if let Some(exported_at) = exported_at {
        object.insert("exportedAt".to_string(), Value::String(exported_at));
    }

    Ok(Value::Object(object))
}
