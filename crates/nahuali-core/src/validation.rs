use std::{collections::BTreeSet, future::Future, path::Path, thread};

use serde::{Deserialize, Serialize};
use surrealdb::{
    Surreal,
    engine::remote::ws::{Client, Ws},
    opt::auth::Root,
};
use tokio::runtime::Builder;

use crate::{
    EVENT_ENVELOPE_VERSION, EventEnvelope,
    database::{SURREAL_NAMESPACE, database_name, normalized_endpoint},
    error::{NahualiError, Result},
    schema::MEMORY_RECORD_SCHEMA,
};

/// Non-destructive SurrealDB record-ledger validation report.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct RecordLedgerValidation {
    /// Whether the record ledger can be opened and projected by this version.
    pub valid: bool,
    /// Number of validated record envelopes.
    pub event_count: usize,
    /// Record-envelope version supported by this crate.
    pub supported_event_version: u32,
    /// Record-envelope versions observed in SurrealDB.
    pub observed_event_versions: Vec<u32>,
    /// Number of accepted pre-public envelopes that require migration.
    pub legacy_event_count: usize,
    /// Whether callers should plan a non-destructive migration.
    pub migration_required: bool,
    /// Validation issues found while scanning records.
    pub issues: Vec<RecordLedgerIssue>,
}

impl RecordLedgerValidation {
    /// Return a valid empty report.
    pub fn empty() -> Self {
        Self {
            valid: true,
            event_count: 0,
            supported_event_version: EVENT_ENVELOPE_VERSION,
            observed_event_versions: Vec::new(),
            legacy_event_count: 0,
            migration_required: false,
            issues: Vec::new(),
        }
    }
}

/// Record-ledger validation issue.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct RecordLedgerIssue {
    /// One-based source record number, if the issue belongs to a concrete record.
    pub line: Option<usize>,
    /// Machine-readable issue category.
    pub kind: RecordLedgerIssueKind,
    /// Issue severity.
    pub severity: RecordLedgerIssueSeverity,
    /// Human-readable diagnostic.
    pub message: String,
}

/// Record-ledger validation issue category.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecordLedgerIssueKind {
    /// A pre-public envelope shape is no longer accepted.
    LegacyEnvelope,
    /// A record did not match the envelope schema.
    ParseError,
    /// Sequence numbers were not contiguous from `1`.
    OutOfOrderSequence,
    /// The record declares an envelope version this crate cannot open.
    UnsupportedVersion,
    /// The checksum does not match the event body.
    ChecksumMismatch,
    /// The tamper-evident hash chain is broken at this record: the recorded
    /// `prev_hash` does not match the previous event's chained hash.
    ///
    /// Only produced when the `tamper-evidence` feature is enabled and the
    /// record carries a chain link; default-build and legacy unchained records
    /// never raise it.
    HashChainBroken,
}

/// Record-ledger validation issue severity.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecordLedgerIssueSeverity {
    /// Informational issue that does not prevent opening records today.
    Info,
    /// Error that prevents opening records.
    Error,
}

#[derive(Clone, Debug, Deserialize)]
struct MemoryRecord {
    sequence: u64,
    envelope: EventEnvelope,
}

/// Validate the SurrealDB record ledger without mutating or projecting it.
pub fn validate_record_ledger(path: impl AsRef<Path>) -> Result<RecordLedgerValidation> {
    let path = path.as_ref();
    let read_path = path.to_path_buf();
    let records = block_on_database(async move { read_records(&read_path).await })?;
    let mut report = RecordLedgerValidation::empty();
    let mut observed_versions = BTreeSet::new();
    let mut expected_sequence = 1_u64;
    // Running chained hash of the last accepted event, used to verify the next
    // event's recorded `prev_hash`. Only consulted under `tamper-evidence`.
    #[cfg(feature = "tamper-evidence")]
    let mut last_chained: Option<String> = None;

    for (index, record) in records.into_iter().enumerate() {
        let record_number = index + 1;
        let event = record.envelope;
        observed_versions.insert(event.version);

        if record.sequence != event.sequence {
            report.valid = false;
            report.issues.push(error_issue(
                record_number,
                RecordLedgerIssueKind::OutOfOrderSequence,
                format!(
                    "record sequence {} does not match envelope sequence {}",
                    record.sequence, event.sequence
                ),
            ));
            continue;
        }

        if event.sequence != expected_sequence {
            report.valid = false;
            report.issues.push(error_issue(
                record_number,
                RecordLedgerIssueKind::OutOfOrderSequence,
                format!(
                    "expected sequence {expected_sequence}, found {}",
                    event.sequence
                ),
            ));
            continue;
        }

        if event.version != EVENT_ENVELOPE_VERSION {
            report.valid = false;
            report.migration_required = true;
            report.issues.push(error_issue(
                record_number,
                RecordLedgerIssueKind::UnsupportedVersion,
                format!(
                    "unsupported record envelope version {}, supported version is {}",
                    event.version, EVENT_ENVELOPE_VERSION
                ),
            ));
            continue;
        }

        if !event.validate_checksum() {
            report.valid = false;
            report.issues.push(error_issue(
                record_number,
                RecordLedgerIssueKind::ChecksumMismatch,
                "checksum mismatch".to_string(),
            ));
            continue;
        }

        // Tamper-evident hash-chain linkage. Mirrors the open/replay check in
        // `store/ledger.rs`: the per-event checksum cannot detect an in-place
        // rewrite that recomputes its own checksum, but the chain can, because
        // the next event's `prev_hash` no longer matches. Unchained records
        // (default-build / legacy) carry no link and are accepted as-is.
        #[cfg(feature = "tamper-evidence")]
        if event.is_chained() {
            let expected_prev = last_chained.clone().unwrap_or_default();
            let recorded_prev = event.prev_hash.clone().unwrap_or_default();
            if recorded_prev != expected_prev {
                report.valid = false;
                report.issues.push(error_issue(
                    record_number,
                    RecordLedgerIssueKind::HashChainBroken,
                    "hash-chain broken: recorded prev_hash does not match the \
                     previous event's chained hash"
                        .to_string(),
                ));
                continue;
            }
        }
        #[cfg(feature = "tamper-evidence")]
        {
            last_chained = Some(event.chain_hash());
        }

        report.event_count += 1;
        expected_sequence += 1;
    }

    report.observed_event_versions = observed_versions.into_iter().collect();
    Ok(report)
}

async fn read_records(path: &Path) -> Result<Vec<MemoryRecord>> {
    let db = open_database(path).await?;
    let mut response = db
        .query("SELECT sequence, envelope FROM memory_record ORDER BY sequence ASC")
        .await
        .map_err(|source| database_error(path, source))?;
    let rows: Vec<serde_json::Value> = response
        .take(0)
        .map_err(|source| database_error(path, source))?;
    rows.into_iter()
        .enumerate()
        .map(|(index, row)| decode_record(path, index + 1, row))
        .collect()
}

fn decode_record(path: &Path, record: usize, row: serde_json::Value) -> Result<MemoryRecord> {
    serde_json::from_value(row).map_err(|source| NahualiError::DecodeRecord {
        path: path.to_path_buf(),
        record,
        source,
    })
}

async fn open_database(path: &Path) -> Result<Surreal<Client>> {
    let endpoint = normalized_endpoint();
    let namespace =
        std::env::var("NAHUALI_DB_NAMESPACE").unwrap_or_else(|_| SURREAL_NAMESPACE.to_string());
    let database = database_name(path);
    let username = std::env::var("NAHUALI_DB_USERNAME").unwrap_or_else(|_| "root".to_string());
    let db_pass = std::env::var("NAHUALI_DB_PASSWORD").unwrap_or_else(|_| "root".to_string());
    let db = Surreal::new::<Ws>(&endpoint)
        .await
        .map_err(|source| database_error(path, source))?;
    db.signin(Root {
        username,
        password: db_pass,
    })
    .await
    .map_err(|source| database_error(path, source))?;
    db.use_ns(namespace)
        .use_db(database)
        .await
        .map_err(|source| database_error(path, source))?;
    db.query(MEMORY_RECORD_SCHEMA)
        .await
        .map_err(|source| database_error(path, source))?;
    Ok(db)
}

fn database_error(path: &Path, source: surrealdb::Error) -> NahualiError {
    NahualiError::Database {
        path: path.to_path_buf(),
        source: Box::new(source),
    }
}

fn runtime() -> Result<tokio::runtime::Runtime> {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|source| NahualiError::Runtime { source })
}

fn block_on_database<F, T>(future: F) -> Result<T>
where
    F: Future<Output = Result<T>> + Send + 'static,
    T: Send + 'static,
{
    if tokio::runtime::Handle::try_current().is_ok() {
        return match thread::spawn(move || runtime()?.block_on(future)).join() {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        };
    }

    runtime()?.block_on(future)
}

fn error_issue(line: usize, kind: RecordLedgerIssueKind, message: String) -> RecordLedgerIssue {
    RecordLedgerIssue {
        line: Some(line),
        kind,
        severity: RecordLedgerIssueSeverity::Error,
        message,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::{EpisodeRecorded, EventEnvelope, MemoryEngine, MemoryEvent};

    use super::{
        RecordLedgerIssueKind, block_on_database, database_error, open_database,
        validate_record_ledger,
    };

    #[test]
    fn validates_missing_store_as_empty_records() {
        let path = temp_path("missing-store");
        let _ = fs::remove_dir_all(&path);

        let report = validate_record_ledger(&path).unwrap();

        assert!(report.valid);
        assert_eq!(report.event_count, 0);
        assert!(!report.migration_required);
    }

    #[test]
    fn validates_surrealdb_records() {
        let path = temp_path("valid-records");
        let _ = fs::remove_dir_all(&path);

        let mut memory = MemoryEngine::open(&path).unwrap();
        memory
            .remember("Lena prefers concise release notes.", Vec::new())
            .unwrap();

        let report = validate_record_ledger(&path).unwrap();

        assert!(report.valid);
        assert_eq!(report.event_count, 1);
        assert_eq!(report.observed_event_versions, vec![1]);

        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn rejects_record_sequence_mismatch() {
        let path = temp_path("sequence-mismatch");
        let _ = fs::remove_dir_all(&path);

        let event = test_envelope(2);
        write_raw_record(&path, 1, event);

        let report = validate_record_ledger(&path).unwrap();

        assert!(!report.valid);
        assert_eq!(report.event_count, 0);
        assert_eq!(report.observed_event_versions, vec![1]);
        assert_eq!(report.issues.len(), 1);
        assert_eq!(
            report.issues[0].kind,
            RecordLedgerIssueKind::OutOfOrderSequence
        );

        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn rejects_non_contiguous_sequences() {
        let path = temp_path("sequence-gap");
        let _ = fs::remove_dir_all(&path);

        let event = test_envelope(2);
        write_raw_record(&path, event.sequence, event);

        let report = validate_record_ledger(&path).unwrap();

        assert!(!report.valid);
        assert_eq!(report.event_count, 0);
        assert_eq!(report.observed_event_versions, vec![1]);
        assert_eq!(report.issues.len(), 1);
        assert_eq!(
            report.issues[0].kind,
            RecordLedgerIssueKind::OutOfOrderSequence
        );

        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn rejects_checksum_mismatches() {
        let path = temp_path("checksum-mismatch");
        let _ = fs::remove_dir_all(&path);

        let mut event = test_envelope(1);
        event.checksum = "fnv1a64:invalid".to_string();
        write_raw_record(&path, event.sequence, event);

        let report = validate_record_ledger(&path).unwrap();

        assert!(!report.valid);
        assert_eq!(report.event_count, 0);
        assert_eq!(report.observed_event_versions, vec![1]);
        assert_eq!(report.issues.len(), 1);
        assert_eq!(
            report.issues[0].kind,
            RecordLedgerIssueKind::ChecksumMismatch
        );

        let _ = fs::remove_dir_all(path);
    }

    /// (b) THE KEY PROOF, exercised through the real non-mutating replay path.
    /// A clean chained ledger validates; rewriting a middle event's payload and
    /// forging its own per-event checksum still passes the checksum-only check,
    /// yet `validate_record_ledger` reports `HashChainBroken` at the next record.
    #[cfg(feature = "tamper-evidence")]
    #[test]
    fn detects_hash_chain_break_after_in_place_rewrite() {
        let path = temp_path("hash-chain-break");
        let _ = fs::remove_dir_all(&path);

        // Build and persist a clean three-event chain.
        let mut chain: Vec<EventEnvelope> = Vec::new();
        for sequence in 1..=3u64 {
            let prev = chain.last().map(EventEnvelope::chain_hash);
            chain.push(EventEnvelope::with_chain(
                sequence,
                1_700_000_000_000 + sequence,
                MemoryEvent::EpisodeRecorded(EpisodeRecorded {
                    id: format!("episode_{sequence}"),
                    content: format!("Chained validation event {sequence}"),
                    tags: vec!["validation".to_string()],
                    mentions: Vec::new(),
                    source_id: None,
                    source_position: None,
                    source_role: None,
                    scope: None,
                }),
                prev.as_deref(),
            ));
        }

        // Tamper the middle event in place: new payload, freshly recomputed
        // checksum, original chain link preserved.
        let original = &chain[1];
        let mut forged = EventEnvelope::new(
            original.sequence,
            original.timestamp_ms,
            MemoryEvent::EpisodeRecorded(EpisodeRecorded {
                id: "episode_2".to_string(),
                content: "TAMPERED middle event".to_string(),
                tags: vec!["validation".to_string()],
                mentions: Vec::new(),
                source_id: None,
                source_position: None,
                source_role: None,
                scope: None,
            }),
        );
        forged.prev_hash = original.prev_hash.clone();
        assert!(
            forged.validate_checksum(),
            "forged checksum must pass the self-contained checksum check"
        );
        chain[1] = forged;

        for event in &chain {
            write_raw_record(&path, event.sequence, event.clone());
        }

        let report = validate_record_ledger(&path).unwrap();

        assert!(!report.valid);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.kind == RecordLedgerIssueKind::HashChainBroken),
            "expected a HashChainBroken issue, got {:?}",
            report.issues
        );
        // The break surfaces at the third record (it recorded the original
        // event 2's chained hash, which no longer matches the forged body).
        assert_eq!(
            report
                .issues
                .iter()
                .find(|issue| issue.kind == RecordLedgerIssueKind::HashChainBroken)
                .and_then(|issue| issue.line),
            Some(3)
        );

        let _ = fs::remove_dir_all(path);
    }

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("nahuali-validation-{name}-{nanos}.surrealdb"))
    }

    fn write_raw_record(path: &Path, sequence: u64, envelope: EventEnvelope) {
        let path = path.to_path_buf();
        block_on_database(async move {
            let db = open_database(&path).await?;
            let envelope =
                serde_json::to_value(envelope).map_err(super::NahualiError::EncodeRecord)?;
            let record = serde_json::json!({
                "sequence": sequence,
                "envelope": envelope,
            });
            db.query("CREATE memory_record CONTENT $record")
                .bind(("record", record))
                .await
                .map_err(|source| database_error(&path, source))?;
            Ok(())
        })
        .unwrap();
    }

    fn test_envelope(sequence: u64) -> EventEnvelope {
        EventEnvelope::new(
            sequence,
            1_700_000_000_000 + sequence,
            MemoryEvent::EpisodeRecorded(EpisodeRecorded {
                id: format!("episode_{sequence}"),
                content: format!("Synthetic validation event {sequence}"),
                tags: vec!["validation".to_string()],
                mentions: Vec::new(),
                source_id: None,
                source_position: None,
                source_role: None,
                scope: None,
            }),
        )
    }
}
