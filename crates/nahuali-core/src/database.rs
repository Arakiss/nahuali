//! Shared SurrealDB connection and database-name helpers.

use std::path::Path;

pub(crate) const SURREAL_NAMESPACE: &str = "nahuali";
const DEFAULT_SURREAL_ENDPOINT: &str = "localhost:18000";
const SURREAL_DATABASE: &str = "memory";

pub(crate) fn normalized_endpoint() -> String {
    let endpoint =
        std::env::var("NAHUALI_DB_URL").unwrap_or_else(|_| DEFAULT_SURREAL_ENDPOINT.to_string());
    endpoint
        .trim()
        .trim_start_matches("ws://")
        .trim_start_matches("wss://")
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .to_string()
}

pub(crate) fn database_name(path: &Path) -> String {
    let explicit = std::env::var("NAHUALI_DB_DATABASE").ok();
    let raw = explicit
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .or_else(|| path.to_str().map(str::to_string))
        .unwrap_or_else(|| SURREAL_DATABASE.to_string());
    normalize_database_name(&raw)
}

fn normalize_database_name(raw: &str) -> String {
    let mut name = raw
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    if name.is_empty() {
        name = SURREAL_DATABASE.to_string();
    }
    if name
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_digit())
    {
        name.insert_str(0, "tenant_");
    }
    name
}

#[cfg(test)]
mod tests {
    use super::normalize_database_name;

    #[test]
    fn normalizes_operator_database_inputs_to_surrealdb_identifiers() {
        assert_eq!(normalize_database_name("memory"), "memory");
        assert_eq!(normalize_database_name(".nahuali-demo"), "_nahuali_demo");
        assert_eq!(
            normalize_database_name(".nahuali-oss/memory"),
            "_nahuali_oss_memory"
        );
        assert_eq!(
            normalize_database_name("project:Nahuali"),
            "project_Nahuali"
        );
        assert_eq!(normalize_database_name("123"), "tenant_123");
        assert_eq!(normalize_database_name(""), "memory");
    }
}
