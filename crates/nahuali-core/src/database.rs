//! Shared SurrealDB connection and configuration resolution.
//!
//! Every database, namespace, and endpoint value the workspace uses is resolved
//! here so the CLI, the MCP server, and the core connection layer agree on one
//! precedence rule — command-line flag, then environment variable, then built-in
//! default — and on one identifier policy. Nothing else should read the
//! `NAHUALI_DB_*` variables for resolution; doing so is what let a flag be
//! silently overridden by the environment.

use std::path::Path;

use serde::Serialize;

pub(crate) const SURREAL_NAMESPACE: &str = "nahuali";
const DEFAULT_SURREAL_ENDPOINT: &str = "localhost:18000";
const SURREAL_DATABASE: &str = "memory";
/// The documented read-only archive default. Its hyphen predates the strict
/// identifier rule, so it is accepted verbatim and mapped to the identifier it
/// has always normalized to.
const DEFAULT_ARCHIVE_DATABASE: &str = "ts-archive";
/// SurrealDB identifier the legacy `ts-archive` default resolves to. Kept stable
/// so archive stores created before the strict identifier rule keep resolving.
const LEGACY_ARCHIVE_IDENTIFIER: &str = "ts_archive";

/// Which precedence tier a resolved configuration value came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfigSource {
    /// A command-line flag — the highest precedence.
    Flag,
    /// An environment variable.
    Env,
    /// The built-in default — the lowest precedence.
    Default,
}

impl ConfigSource {
    /// Stable lowercase label used in human and JSON output.
    pub fn as_str(self) -> &'static str {
        match self {
            ConfigSource::Flag => "flag",
            ConfigSource::Env => "env",
            ConfigSource::Default => "default",
        }
    }
}

/// A single resolved configuration value and the precedence tier it came from.
#[derive(Clone, Debug, Serialize)]
pub struct ResolvedValue {
    /// The effective value in use.
    pub value: String,
    /// Whether the value came from a flag, environment variable, or default.
    pub source: ConfigSource,
}

/// The fully resolved effective configuration, free of any secret material.
#[derive(Clone, Debug, Serialize)]
pub struct ResolvedConfig {
    /// SurrealDB endpoint with any scheme prefix stripped, e.g. `localhost:18000`.
    pub endpoint: ResolvedValue,
    /// SurrealDB namespace.
    pub namespace: ResolvedValue,
    /// Effective memory database identifier.
    pub database: ResolvedValue,
    /// Effective read-only archive database identifier.
    pub archive_database: ResolvedValue,
}

/// A database name was refused because it is not a valid SurrealDB identifier and
/// normalizing it would silently connect to a *different* database than the one
/// the caller typed.
#[derive(Clone, Debug, thiserror::Error)]
#[error(
    "database name `{provided}` is not a valid identifier. Allowed characters are [A-Za-z0-9_] \
     (a leading digit is prefixed with `tenant_`). Nahuali refuses to silently rewrite it into a \
     different database. Use `{suggestion}` if that is what you meant, or pick a name of your own."
)]
pub struct DatabaseNameError {
    /// The rejected name exactly as provided.
    pub provided: String,
    /// The normalized identifier the name would otherwise have become.
    pub suggestion: String,
}

/// Resolve the effective memory database name with strict precedence:
/// `flag` (`--database`) beats `env` (`NAHUALI_DB_DATABASE`) beats the built-in
/// default (`memory`). A name that normalization would change is refused rather
/// than silently mangled.
pub fn resolve_database_name(flag: Option<&str>) -> Result<ResolvedValue, DatabaseNameError> {
    let env = std::env::var("NAHUALI_DB_DATABASE").ok();
    resolve_database_name_from(flag, env.as_deref())
}

/// Precedence core, split out from the environment read so it can be unit-tested
/// deterministically (this is the "a flag can never be silently overridden by the
/// environment" test).
fn resolve_database_name_from(
    flag: Option<&str>,
    env: Option<&str>,
) -> Result<ResolvedValue, DatabaseNameError> {
    if let Some(flag) = flag.map(str::trim).filter(|value| !value.is_empty()) {
        return Ok(ResolvedValue {
            value: validate_database_name(flag)?,
            source: ConfigSource::Flag,
        });
    }
    if let Some(env) = env.map(str::trim).filter(|value| !value.is_empty()) {
        return Ok(ResolvedValue {
            value: validate_database_name(env)?,
            source: ConfigSource::Env,
        });
    }
    Ok(ResolvedValue {
        value: SURREAL_DATABASE.to_string(),
        source: ConfigSource::Default,
    })
}

/// Validate an explicit database name. Returns the SurrealDB identifier when the
/// name is already valid (or is the legacy `ts-archive` default), and a typed
/// [`DatabaseNameError`] when normalization would silently change it.
pub fn validate_database_name(raw: &str) -> Result<String, DatabaseNameError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(SURREAL_DATABASE.to_string());
    }
    if trimmed == DEFAULT_ARCHIVE_DATABASE {
        return Ok(LEGACY_ARCHIVE_IDENTIFIER.to_string());
    }
    let normalized = normalize_database_name(trimmed);
    if normalized == trimmed {
        Ok(normalized)
    } else {
        Err(DatabaseNameError {
            provided: trimmed.to_string(),
            suggestion: normalized,
        })
    }
}

/// Resolve the SurrealDB endpoint (`NAHUALI_DB_URL` env, else default), with any
/// `ws(s)://` or `http(s)://` scheme stripped.
pub fn resolve_endpoint() -> ResolvedValue {
    match std::env::var("NAHUALI_DB_URL") {
        Ok(url) if !url.trim().is_empty() => ResolvedValue {
            value: strip_endpoint_scheme(&url),
            source: ConfigSource::Env,
        },
        _ => ResolvedValue {
            value: DEFAULT_SURREAL_ENDPOINT.to_string(),
            source: ConfigSource::Default,
        },
    }
}

/// Resolve the SurrealDB namespace (`NAHUALI_DB_NAMESPACE` env, else default).
pub fn resolve_namespace() -> ResolvedValue {
    match std::env::var("NAHUALI_DB_NAMESPACE") {
        Ok(namespace) if !namespace.trim().is_empty() => ResolvedValue {
            value: namespace.trim().to_string(),
            source: ConfigSource::Env,
        },
        _ => ResolvedValue {
            value: SURREAL_NAMESPACE.to_string(),
            source: ConfigSource::Default,
        },
    }
}

/// Resolve the read-only archive database (`NAHUALI_ARCHIVE_DB` env, else the
/// documented `ts-archive` default). Unlike the primary database this never
/// refuses: the archive is a secondary reference store, so an unusual name is
/// normalized to a SurrealDB identifier rather than rejected.
pub fn resolve_archive_database() -> ResolvedValue {
    match std::env::var("NAHUALI_ARCHIVE_DB") {
        Ok(name) if !name.trim().is_empty() => ResolvedValue {
            value: normalize_database_name(name.trim()),
            source: ConfigSource::Env,
        },
        _ => ResolvedValue {
            value: normalize_database_name(DEFAULT_ARCHIVE_DATABASE),
            source: ConfigSource::Default,
        },
    }
}

/// Resolve the full effective configuration (no secret material) for
/// `nahuali config`. The primary database is validated; a refusal surfaces here
/// as the same typed error the store-opening path would raise.
pub fn resolve_config(database_flag: Option<&str>) -> Result<ResolvedConfig, DatabaseNameError> {
    Ok(ResolvedConfig {
        endpoint: resolve_endpoint(),
        namespace: resolve_namespace(),
        database: resolve_database_name(database_flag)?,
        archive_database: resolve_archive_database(),
    })
}

pub(crate) fn normalized_endpoint() -> String {
    resolve_endpoint().value
}

pub(crate) fn resolved_namespace() -> String {
    resolve_namespace().value
}

fn strip_endpoint_scheme(endpoint: &str) -> String {
    endpoint
        .trim()
        .trim_start_matches("ws://")
        .trim_start_matches("wss://")
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .to_string()
}

/// The SurrealDB database identifier for an already-resolved database path.
///
/// Resolution and refusal happen once at the CLI/MCP boundary via
/// [`resolve_database_name`]; by the time a path reaches the connection layer it
/// is the resolved name, so this only maps it to a SurrealDB identifier. It no
/// longer reads `NAHUALI_DB_DATABASE`: that read moved into the single resolver
/// so a flag can never be silently overridden by the environment. Secondary
/// stores that do not pass through the strict boundary (the read-only archive,
/// restore/drill target databases) still normalize here.
pub(crate) fn database_name(path: &Path) -> String {
    path.to_str()
        .map(normalize_database_name)
        .unwrap_or_else(|| SURREAL_DATABASE.to_string())
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
    use super::{
        ConfigSource, database_name, normalize_database_name, resolve_database_name_from,
        validate_database_name,
    };
    use std::path::Path;

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

    #[test]
    fn resolution_prefers_flag_over_env_over_default() {
        // The override-no-longer-silent guarantee: a flag beats a set env var.
        let resolved = resolve_database_name_from(Some("flag_db"), Some("env_db"))
            .expect("clean flag resolves");
        assert_eq!(resolved.value, "flag_db");
        assert_eq!(resolved.source, ConfigSource::Flag);

        // Env beats the default when no flag is present.
        let resolved =
            resolve_database_name_from(None, Some("env_db")).expect("clean env resolves");
        assert_eq!(resolved.value, "env_db");
        assert_eq!(resolved.source, ConfigSource::Env);

        // Nothing set falls through to the built-in default.
        let resolved = resolve_database_name_from(None, None).expect("default resolves");
        assert_eq!(resolved.value, "memory");
        assert_eq!(resolved.source, ConfigSource::Default);

        // A blank env value does not count; the default still wins.
        let resolved =
            resolve_database_name_from(None, Some("   ")).expect("blank env falls through");
        assert_eq!(resolved.source, ConfigSource::Default);
    }

    #[test]
    fn resolution_refuses_a_path_like_flag_even_with_a_clean_env() {
        // A path-like flag is refused outright, never silently mangled, and a
        // clean env value cannot rescue it (flag precedence still applies).
        let error = resolve_database_name_from(Some("./memory"), Some("memory"))
            .expect_err("path-like flag is refused");
        assert_eq!(error.provided, "./memory");
        assert_eq!(error.suggestion, "__memory");
    }

    #[test]
    fn validate_rejects_path_like_names_and_accepts_clean_ones() {
        // Clean identifiers pass through unchanged.
        assert_eq!(validate_database_name("memory").expect("clean"), "memory");
        assert_eq!(
            validate_database_name("project_nahuali").expect("clean"),
            "project_nahuali"
        );

        // Anything normalization would change is refused.
        assert!(validate_database_name("./memory").is_err());
        assert!(validate_database_name("my-db").is_err());
        assert!(validate_database_name("project:nahuali").is_err());
        assert!(validate_database_name("has space").is_err());
        assert!(validate_database_name("123").is_err());
    }

    #[test]
    fn legacy_archive_default_keeps_resolving() {
        // Compatibility carve-out: the documented `ts-archive` default resolves
        // to the identifier it has always connected to.
        assert_eq!(
            validate_database_name("ts-archive").expect("legacy default"),
            "ts_archive"
        );
    }

    #[test]
    fn connection_layer_maps_resolved_path_without_reading_env() {
        // The connection layer only maps an already-resolved name to an
        // identifier; it does not consult NAHUALI_DB_DATABASE.
        assert_eq!(database_name(Path::new("memory")), "memory");
        assert_eq!(database_name(Path::new("ts-archive")), "ts_archive");
    }
}
