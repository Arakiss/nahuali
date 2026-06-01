use std::path::PathBuf;

/// Error type returned by `nahuali-core` operations.
#[derive(Debug, thiserror::Error)]
pub enum NahualiError {
    /// Empty memory content, fact field, or relation field was provided.
    #[error("memory content cannot be empty")]
    EmptyContent,
    /// Empty recall query was provided.
    #[error("query cannot be empty")]
    EmptyQuery,
    /// Scope metadata was malformed or empty.
    #[error("invalid memory scope: {message}")]
    InvalidScope {
        /// Human-readable scope validation failure.
        message: String,
    },
    /// The requested intention does not exist in the current projection.
    #[error("unknown intention: {id}")]
    UnknownIntention {
        /// Requested intention identifier.
        id: String,
    },
    /// An intention update request is malformed or has no effect.
    #[error("invalid intention update: {message}")]
    InvalidIntentionUpdate {
        /// Human-readable validation failure.
        message: String,
    },
    /// The requested operator review item does not exist in the current queue.
    #[error("unknown operator review item: {id}")]
    UnknownReviewItem {
        /// Requested review item identifier.
        id: String,
    },
    /// The requested source does not exist in the current projection.
    #[error("unknown source: {id}")]
    UnknownSource {
        /// Requested source identifier.
        id: String,
    },
    /// The SurrealDB memory store could not be opened or queried.
    #[error("failed to use SurrealDB memory store at {path}: {source}")]
    Database {
        /// Database path that failed.
        path: PathBuf,
        /// Underlying SurrealDB error.
        source: Box<surrealdb::Error>,
    },
    /// The memory database path could not be represented for SurrealDB.
    #[error("memory database path is not valid UTF-8: {path}")]
    InvalidDatabasePath {
        /// Database path that could not be used.
        path: PathBuf,
    },
    /// A short-lived runtime could not be created for embedded database work.
    #[error("failed to create async runtime: {source}")]
    Runtime {
        /// Underlying runtime creation error.
        source: std::io::Error,
    },
    /// A snapshot file could not be read.
    #[error("failed to read memory snapshot at {path}: {source}")]
    ReadSnapshot {
        /// Snapshot path that could not be read.
        path: PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },
    /// A snapshot file could not be written.
    #[error("failed to write memory snapshot at {path}: {source}")]
    WriteSnapshot {
        /// Snapshot path that could not be written.
        path: PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },
    /// A backup file could not be read.
    #[error("failed to read memory backup at {path}: {source}")]
    ReadBackup {
        /// Backup path that could not be read.
        path: PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },
    /// A backup file could not be written.
    #[error("failed to write memory backup at {path}: {source}")]
    WriteBackup {
        /// Backup path that could not be written.
        path: PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },
    /// The SurrealDB record ledger failed integrity validation.
    #[error("invalid memory record ledger at {path}: record {record}: {message}")]
    InvalidRecordLedger {
        /// Database path containing the invalid record.
        path: PathBuf,
        /// One-based record position containing the invalid event.
        record: usize,
        /// Human-readable validation failure.
        message: String,
    },
    /// A snapshot could not be encoded before writing.
    #[error("failed to encode memory snapshot: {0}")]
    EncodeSnapshot(serde_json::Error),
    /// A backup could not be encoded before writing.
    #[error("failed to encode memory backup: {0}")]
    EncodeBackup(serde_json::Error),
    /// A backup file could not be decoded.
    #[error("failed to decode memory backup at {path}: {source}")]
    DecodeBackup {
        /// Backup path that could not be decoded.
        path: PathBuf,
        /// Underlying JSON decoding error.
        source: serde_json::Error,
    },
    /// A memory record could not be encoded before writing it to SurrealDB.
    #[error("failed to encode memory record: {0}")]
    EncodeRecord(serde_json::Error),
    /// A memory record read from SurrealDB could not be decoded.
    #[error("failed to decode memory record at {path}, record {record}: {source}")]
    DecodeRecord {
        /// Database path containing the invalid record.
        path: PathBuf,
        /// One-based record position containing the invalid event.
        record: usize,
        /// Underlying JSON decoding error.
        source: serde_json::Error,
    },
    /// Semantic tier configuration is invalid or unsupported by this build.
    #[error("invalid semantic configuration: {message}")]
    InvalidSemanticConfig {
        /// Human-readable configuration failure.
        message: String,
    },
    /// A local embedding model could not be loaded or produced no vectors.
    #[error("local embedding model error: {message}")]
    LocalEmbeddingModel {
        /// Human-readable load or inference failure.
        message: String,
    },
    /// The Qdrant semantic tier could not be reached.
    #[error("semantic tier HTTP error at {endpoint}: {source}")]
    SemanticHttp {
        /// Qdrant REST endpoint that failed.
        endpoint: String,
        /// Underlying HTTP client error.
        source: Box<reqwest::Error>,
    },
    /// Qdrant returned a non-successful API response.
    #[error("semantic tier API error at {endpoint}: status {status}: {message}")]
    SemanticApi {
        /// Qdrant REST endpoint that failed.
        endpoint: String,
        /// HTTP response status code.
        status: u16,
        /// Response body or Qdrant status payload.
        message: String,
    },
    /// A Qdrant response could not be decoded.
    #[error("semantic tier decode error at {endpoint}: {source}")]
    SemanticDecode {
        /// Qdrant REST endpoint that returned malformed JSON.
        endpoint: String,
        /// Underlying JSON decoding error.
        source: serde_json::Error,
    },
}

/// Result alias used by `nahuali-core`.
pub type Result<T> = std::result::Result<T, NahualiError>;
