//! Shared SurrealDB connection and configuration resolution.
//!
//! Every database, namespace, and endpoint value the workspace uses is resolved
//! here so the CLI, the MCP server, and the core connection layer agree on one
//! precedence rule — command-line flag, then environment variable, then built-in
//! default — and on one identifier policy. Nothing else should read the
//! `NAHUALI_DB_*` variables for resolution; doing so is what let a flag be
//! silently overridden by the environment.

use std::{
    collections::HashMap,
    fs::OpenOptions,
    future::Future,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    thread,
    time::Duration,
};

use fs2::FileExt;
use serde::Serialize;
use surrealdb::{
    Surreal,
    engine::any::{Any, connect},
    opt::auth::Root,
};
use tokio::runtime::{Builder, Runtime};

use crate::error::{NahualiError, Result as CoreResult};

pub(crate) const SURREAL_NAMESPACE: &str = "nahuali";
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
    /// Effective embedded or remote SurrealDB endpoint, including its scheme.
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

/// Resolve the storage endpoint (`NAHUALI_DB_URL` env, else an embedded
/// SurrealKV store under `~/.nahuali/data`). Remote endpoints without a scheme
/// keep compatibility with the historical `host:port` form by using `ws://`.
pub fn resolve_endpoint() -> ResolvedValue {
    match std::env::var("NAHUALI_DB_URL") {
        Ok(url) if !url.trim().is_empty() => ResolvedValue {
            value: normalize_endpoint(&url),
            source: ConfigSource::Env,
        },
        _ => ResolvedValue {
            value: default_embedded_endpoint(),
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

fn default_embedded_endpoint() -> String {
    let root = std::env::var_os("NAHUALI_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".nahuali")))
        .unwrap_or_else(|| PathBuf::from(".nahuali"));
    format!("surrealkv://{}", root.join("data").display())
}

fn normalize_endpoint(endpoint: &str) -> String {
    let endpoint = endpoint.trim();
    if endpoint.contains("://") {
        endpoint.to_string()
    } else {
        format!("ws://{endpoint}")
    }
}

pub(crate) fn is_embedded_endpoint(endpoint: &str) -> bool {
    endpoint.starts_with("surrealkv://") || endpoint.starts_with("mem://")
}

static STORE_RUNTIMES: OnceLock<Mutex<HashMap<String, Arc<StoreRuntime>>>> = OnceLock::new();

struct StoreRuntime {
    endpoint: String,
    runtime: Option<Runtime>,
    root: Mutex<Option<Surreal<Any>>>,
}

/// A logical database session backed by one process-owned SurrealDB router.
///
/// Embedded SurrealKV permits a single owner for its physical directory. All
/// sessions in this process therefore share one router and runtime, while each
/// clone keeps an independent namespace/database selection.
#[derive(Clone)]
pub(crate) struct DatabaseSession {
    db: Surreal<Any>,
    owner: Arc<StoreRuntime>,
}

impl std::fmt::Debug for DatabaseSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DatabaseSession")
            .field("endpoint", &self.owner.endpoint)
            .finish_non_exhaustive()
    }
}

impl DatabaseSession {
    pub(crate) fn open(path: &Path) -> CoreResult<Self> {
        let endpoint = normalized_endpoint();
        let owner = store_runtime(&endpoint, path)?;
        let db = owner
            .root
            .lock()
            .expect("database root lock must not be poisoned")
            .as_ref()
            .expect("database root exists while the runtime is alive")
            .clone();
        let namespace = resolved_namespace();
        let database = database_name(path);
        let logical_path = path.to_path_buf();
        let selected = db.clone();
        let selected = run_on(&owner, async move {
            selected
                .use_ns(namespace)
                .use_db(database)
                .await
                .map_err(|source| database_error(&logical_path, source))?;
            Ok(selected)
        })?;
        Ok(Self {
            db: selected,
            owner,
        })
    }
}

impl std::ops::Deref for DatabaseSession {
    type Target = Surreal<Any>;

    fn deref(&self) -> &Self::Target {
        &self.db
    }
}

fn store_runtime(endpoint: &str, path: &Path) -> CoreResult<Arc<StoreRuntime>> {
    let runtimes = STORE_RUNTIMES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut runtimes = runtimes
        .lock()
        .expect("store runtime registry must not be poisoned");
    if let Some(runtime) = runtimes.get(endpoint) {
        return Ok(Arc::clone(runtime));
    }

    let endpoint_owned = endpoint.to_string();
    let path_owned = path.to_path_buf();
    let (runtime, root) = thread::spawn(move || -> CoreResult<_> {
        let runtime = Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|source| NahualiError::Runtime { source })?;
        let root = runtime
            .block_on(async { connect(&endpoint_owned).await })
            .map_err(|source| database_error(&path_owned, source))?;
        if !is_embedded_endpoint(&endpoint_owned) {
            let username =
                std::env::var("NAHUALI_DB_USERNAME").unwrap_or_else(|_| "root".to_string());
            let password =
                std::env::var("NAHUALI_DB_PASSWORD").unwrap_or_else(|_| "root".to_string());
            runtime
                .block_on(async { root.signin(Root { username, password }).await })
                .map_err(|source| database_error(&path_owned, source))?;
        }
        Ok((runtime, root))
    })
    .join()
    .unwrap_or_else(|payload| std::panic::resume_unwind(payload))?;
    let owner = Arc::new(StoreRuntime {
        endpoint: endpoint.to_string(),
        runtime: Some(runtime),
        root: Mutex::new(Some(root)),
    });
    runtimes.insert(endpoint.to_string(), Arc::clone(&owner));
    Ok(owner)
}

fn run_on<F, T>(owner: &Arc<StoreRuntime>, future: F) -> CoreResult<T>
where
    F: Future<Output = CoreResult<T>> + Send + 'static,
    T: Send + 'static,
{
    if tokio::runtime::Handle::try_current().is_ok() {
        let owner = Arc::clone(owner);
        return match thread::spawn(move || {
            owner
                .runtime
                .as_ref()
                .expect("database runtime exists while its owner is alive")
                .block_on(future)
        })
        .join()
        {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        };
    }
    owner
        .runtime
        .as_ref()
        .expect("database runtime exists while its owner is alive")
        .block_on(future)
}

impl Drop for StoreRuntime {
    fn drop(&mut self) {
        let root = self
            .root
            .get_mut()
            .expect("database root lock must not be poisoned")
            .take();
        let runtime = self
            .runtime
            .take()
            .expect("database runtime exists until its owner is dropped");
        let lock_path = self
            .endpoint
            .strip_prefix("surrealkv://")
            .map(|path| Path::new(path).join("LOCK"));
        let _ = thread::spawn(move || {
            drop(root);
            runtime.shutdown_timeout(Duration::from_secs(2));
            if let Some(lock_path) = lock_path {
                for _ in 0..200 {
                    if let Ok(file) = OpenOptions::new().read(true).write(true).open(&lock_path)
                        && file.try_lock_exclusive().is_ok()
                    {
                        let _ = file.unlock();
                        break;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
            }
        })
        .join();
    }
}

fn database_error(path: &Path, source: surrealdb::Error) -> NahualiError {
    NahualiError::Database {
        path: path.to_path_buf(),
        source: Box::new(source),
    }
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
        ConfigSource, database_name, normalize_database_name, normalize_endpoint,
        resolve_database_name_from, validate_database_name,
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

    #[test]
    fn endpoint_normalization_preserves_engine_and_transport_schemes() {
        assert_eq!(
            normalize_endpoint("localhost:18000"),
            "ws://localhost:18000"
        );
        assert_eq!(normalize_endpoint("ws://db:8000"), "ws://db:8000");
        assert_eq!(normalize_endpoint("wss://db.example"), "wss://db.example");
        assert_eq!(
            normalize_endpoint("surrealkv:///tmp/nahuali"),
            "surrealkv:///tmp/nahuali"
        );
    }
}
