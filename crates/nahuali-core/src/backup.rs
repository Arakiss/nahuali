use std::{fs, path::Path, time::UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::{
    EVENT_ENVELOPE_VERSION, EventEnvelope,
    error::{NahualiError, Result},
    maintenance::{checksum_json, record_ledger_checksum},
    semantic::DEFAULT_QDRANT_URL,
};

/// Current local backup manifest format version.
pub const MEMORY_BACKUP_VERSION: u32 = 1;

/// Versioned local backup document for the authoritative record ledger.
///
/// A backup preserves SurrealDB record envelopes exactly. Semantic vectors are
/// treated as a derived Qdrant tier and are rebuilt from these records after
/// restore in the current release boundary.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct MemoryBackup {
    /// Backup manifest format version.
    pub version: u32,
    /// Timestamp in milliseconds when the backup was generated.
    pub created_at_ms: u64,
    /// `nahuali-core` crate version that generated the backup.
    pub engine_version: String,
    /// Operator-facing source database selector.
    pub source_database: String,
    /// Event-envelope version used by all included records.
    pub event_envelope_version: u32,
    /// Number of authoritative records included in this backup.
    pub record_count: usize,
    /// Identifier of the last included record.
    pub last_event_id: Option<String>,
    /// Deterministic checksum of the authoritative record ledger.
    pub record_ledger_checksum: String,
    /// Metadata for the derived semantic tier.
    pub semantic_tier: SemanticTierBackup,
    /// Authoritative event envelopes to restore into a new SurrealDB database.
    pub records: Vec<EventEnvelope>,
    /// Deterministic checksum of the backup body excluding this field.
    pub checksum: String,
}

impl MemoryBackup {
    /// Return compact backup metadata suitable for CLI output.
    pub fn summary(&self) -> BackupSummary {
        BackupSummary {
            version: self.version,
            created_at_ms: self.created_at_ms,
            engine_version: self.engine_version.clone(),
            source_database: self.source_database.clone(),
            event_envelope_version: self.event_envelope_version,
            record_count: self.record_count,
            last_event_id: self.last_event_id.clone(),
            record_ledger_checksum: self.record_ledger_checksum.clone(),
            semantic_tier: self.semantic_tier.clone(),
            checksum: self.checksum.clone(),
        }
    }

    /// Return whether the stored backup checksum matches the backup body.
    pub fn checksum_valid(&self) -> bool {
        self.checksum == backup_checksum(self)
    }
}

/// Compact backup metadata without the record payloads.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct BackupSummary {
    /// Backup manifest format version.
    pub version: u32,
    /// Timestamp in milliseconds when the backup was generated.
    pub created_at_ms: u64,
    /// `nahuali-core` crate version that generated the backup.
    pub engine_version: String,
    /// Operator-facing source database selector.
    pub source_database: String,
    /// Event-envelope version used by the backup.
    pub event_envelope_version: u32,
    /// Number of authoritative records included in the backup.
    pub record_count: usize,
    /// Identifier of the last included record.
    pub last_event_id: Option<String>,
    /// Deterministic checksum of the authoritative record ledger.
    pub record_ledger_checksum: String,
    /// Metadata for the derived semantic tier.
    pub semantic_tier: SemanticTierBackup,
    /// Deterministic checksum of the backup body.
    pub checksum: String,
}

/// Metadata for a backed-up semantic tier.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SemanticTierBackup {
    /// Semantic storage provider.
    pub provider: SemanticTierProvider,
    /// Whether semantic data is derived from authoritative records.
    pub derived: bool,
    /// Qdrant endpoint observed when the backup was created, if configured.
    pub endpoint: Option<String>,
    /// Qdrant collections coordinated by the backup contract.
    pub collections: Vec<String>,
    /// Whether collection snapshots are included in this backup.
    pub snapshot_status: SemanticTierSnapshotStatus,
    /// Restore policy for semantic data.
    pub restore_policy: SemanticTierRestorePolicy,
    /// Human-readable operational note.
    pub message: String,
}

/// Semantic storage provider.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SemanticTierProvider {
    /// Qdrant vector database.
    Qdrant,
}

/// Snapshot status for the semantic tier.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SemanticTierSnapshotStatus {
    /// Vector snapshots are not included because the tier is derived.
    NotIncluded,
}

/// Restore policy for semantic data.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SemanticTierRestorePolicy {
    /// Rebuild vectors from restored authoritative records.
    RebuildFromRecords,
}

/// Options for validating a local backup document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackupValidationOptions {
    /// Require every included record to carry a tamper-evident hash-chain link.
    ///
    /// Fail-closed by default: a backup with stripped chain links (even with a
    /// recomputed backup checksum) is rejected. Use
    /// [`BackupValidationOptions::legacy_permissive`] to accept unchained legacy
    /// records.
    pub require_chained: bool,
}

impl Default for BackupValidationOptions {
    fn default() -> Self {
        Self {
            require_chained: true,
        }
    }
}

impl BackupValidationOptions {
    /// Fail-closed validation: every included record must be chained. Default.
    pub fn fail_closed() -> Self {
        Self::default()
    }

    /// Legacy-permissive validation: accept unchained records instead of failing
    /// closed. Use only for backups of legacy ledgers written before the
    /// tamper-evident chain existed.
    pub fn legacy_permissive() -> Self {
        Self {
            require_chained: false,
        }
    }
}

/// Result of validating a local backup file.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct BackupValidation {
    /// Whether the backup can be trusted for restore.
    pub valid: bool,
    /// Backup manifest version, if the document could be parsed.
    pub backup_version: Option<u32>,
    /// Event count recorded in the backup, if the document could be parsed.
    pub backup_record_count: Option<usize>,
    /// Last event ID recorded in the backup, if the document could be parsed.
    pub backup_last_event_id: Option<String>,
    /// Record-ledger checksum recorded in the backup, if the document could be parsed.
    pub backup_record_ledger_checksum: Option<String>,
    /// Whether the backup body checksum is valid.
    pub checksum_valid: bool,
    /// Whether every included record has valid sequence, version, and checksum metadata.
    pub records_valid: bool,
    /// Whether hash-chain links are valid for the included records under the
    /// requested validation mode.
    #[cfg(feature = "tamper-evidence")]
    pub chain_valid: bool,
    /// Whether this validation required every included record to be chained.
    #[cfg(feature = "tamper-evidence")]
    pub require_chained: bool,
    /// Semantic-tier metadata, if the document could be parsed.
    pub semantic_tier: Option<SemanticTierBackup>,
    /// Validation issues found while checking the backup.
    pub issues: Vec<BackupIssue>,
}

/// Result of restoring a backup into a target database.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct BackupRestoreReport {
    /// Whether the backup was valid and the target database was safe to use.
    pub valid: bool,
    /// Whether this was a dry-run restore.
    pub dry_run: bool,
    /// Backup file path used for restore.
    pub backup_path: String,
    /// Target database selector used for restore.
    pub target_database: String,
    /// Number of records that can be restored from the backup.
    pub appendable_event_count: usize,
    /// Number of records written to the target database.
    pub restored_event_count: usize,
    /// Whether the target database had no records before restore.
    pub target_was_empty: bool,
    /// Deterministic checksum of the restored record ledger.
    pub record_ledger_checksum: Option<String>,
    /// Semantic-tier policy that must run after record restore.
    pub semantic_restore_policy: Option<SemanticTierRestorePolicy>,
    /// Validation or restore issues found before or during restore.
    pub issues: Vec<BackupIssue>,
}

/// Result of a non-mutating backup recovery drill.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct BackupDrillReport {
    /// Whether the drill found a valid backup and a restore-ready target.
    pub valid: bool,
    /// Backup file path used for the drill.
    pub backup_path: String,
    /// Target database selector used for the restore dry-run.
    pub target_database: String,
    /// Backup validation report.
    pub backup_validation: BackupValidation,
    /// Dry-run restore report.
    pub restore_dry_run: BackupRestoreReport,
    /// Whether semantic indexes must be rebuilt after an actual restore.
    pub semantic_rebuild_required: bool,
    /// Non-mutating next actions for the operator.
    pub operator_next_actions: Vec<String>,
}

/// Backup validation or restore issue.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct BackupIssue {
    /// One-based record number, if the issue belongs to a concrete record.
    pub record: Option<usize>,
    /// Machine-readable issue category.
    pub kind: BackupIssueKind,
    /// Issue severity.
    pub severity: BackupIssueSeverity,
    /// Human-readable diagnostic.
    pub message: String,
}

/// Backup issue category.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BackupIssueKind {
    /// The backup file is not valid JSON or does not match the backup schema.
    ParseError,
    /// The backup declares a version this build does not support.
    UnsupportedVersion,
    /// The backup body checksum does not match the stored checksum.
    ChecksumMismatch,
    /// Backup record-ledger metadata does not match the included records.
    RecordLedgerMismatch,
    /// An included record has a non-contiguous sequence number.
    RecordSequenceMismatch,
    /// An included record checksum does not match its body.
    RecordChecksumMismatch,
    /// An included record's hash-chain link does not match the previous
    /// record's chained hash.
    RecordHashChainBroken,
    /// An included record did not carry a hash-chain link while validation
    /// required every record to be chained.
    RecordHashChainMissing,
    /// The target database already contains records.
    TargetNotEmpty,
    /// The restored target did not match the backup after writing.
    RestoreVerificationMismatch,
}

/// Backup issue severity.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BackupIssueSeverity {
    /// Error that prevents trusting or applying the backup.
    Error,
}

pub(crate) fn create_backup(
    source_database: &Path,
    events: &[EventEnvelope],
    semantic_collections: Vec<String>,
) -> MemoryBackup {
    let mut backup = MemoryBackup {
        version: MEMORY_BACKUP_VERSION,
        created_at_ms: now_ms(),
        engine_version: env!("CARGO_PKG_VERSION").to_string(),
        source_database: source_database.display().to_string(),
        event_envelope_version: EVENT_ENVELOPE_VERSION,
        record_count: events.len(),
        last_event_id: events.last().map(|event| event.id.clone()),
        record_ledger_checksum: record_ledger_checksum(events),
        semantic_tier: semantic_tier_backup(semantic_collections),
        records: events.to_vec(),
        checksum: String::new(),
    };
    backup.checksum = backup_checksum(&backup);
    backup
}

pub(crate) fn write_backup_file(path: &Path, backup: &MemoryBackup) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|source| NahualiError::WriteBackup {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let encoded = serde_json::to_string_pretty(backup).map_err(NahualiError::EncodeBackup)?;
    fs::write(path, format!("{encoded}\n")).map_err(|source| NahualiError::WriteBackup {
        path: path.to_path_buf(),
        source,
    })
}

pub(crate) fn read_backup_file(path: &Path) -> Result<MemoryBackup> {
    let raw = fs::read_to_string(path).map_err(|source| NahualiError::ReadBackup {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&raw).map_err(|source| NahualiError::DecodeBackup {
        path: path.to_path_buf(),
        source,
    })
}

pub(crate) fn validate_backup_file(path: &Path) -> Result<BackupValidation> {
    validate_backup_file_with_options(path, &BackupValidationOptions::default())
}

pub(crate) fn validate_backup_file_with_options(
    path: &Path,
    options: &BackupValidationOptions,
) -> Result<BackupValidation> {
    let raw = fs::read_to_string(path).map_err(|source| NahualiError::ReadBackup {
        path: path.to_path_buf(),
        source,
    })?;

    let backup: MemoryBackup = match serde_json::from_str(&raw) {
        Ok(backup) => backup,
        Err(source) => {
            return Ok(BackupValidation {
                valid: false,
                backup_version: None,
                backup_record_count: None,
                backup_last_event_id: None,
                backup_record_ledger_checksum: None,
                checksum_valid: false,
                records_valid: false,
                #[cfg(feature = "tamper-evidence")]
                chain_valid: false,
                #[cfg(feature = "tamper-evidence")]
                require_chained: options.require_chained,
                semantic_tier: None,
                issues: vec![error_issue(
                    None,
                    BackupIssueKind::ParseError,
                    format!("invalid backup JSON: {source}"),
                )],
            });
        }
    };

    // Honor the caller's options directly. (Previously the permissive branch
    // fell back to `validate_backup`, whose default was permissive; now that the
    // default is fail-closed, routing through it would invert `--allow-unchained`.)
    Ok(validate_backup_with_options(&backup, options))
}

/// Validate an in-memory backup with the default (fail-closed) options.
///
/// Test-only helper (its callers are the tamper-evidence backup tests):
/// production paths call `validate_backup_file[_with_options]`, which now
/// forwards options directly rather than routing the permissive case through
/// here.
#[cfg(all(test, feature = "tamper-evidence"))]
pub(crate) fn validate_backup(backup: &MemoryBackup) -> BackupValidation {
    validate_backup_with_options(backup, &BackupValidationOptions::default())
}

pub(crate) fn validate_backup_with_options(
    backup: &MemoryBackup,
    options: &BackupValidationOptions,
) -> BackupValidation {
    #[cfg(not(feature = "tamper-evidence"))]
    let _ = options;

    let mut issues = Vec::new();
    let checksum_valid = backup.checksum_valid();
    let mut records_valid = true;
    #[cfg(feature = "tamper-evidence")]
    let mut chain_valid = true;
    #[cfg(feature = "tamper-evidence")]
    let mut last_chained: Option<String> = None;
    let actual_record_count = backup.records.len();
    let actual_last_event_id = backup.records.last().map(|event| event.id.clone());
    let actual_record_ledger_checksum = record_ledger_checksum(&backup.records);

    if backup.version != MEMORY_BACKUP_VERSION {
        issues.push(error_issue(
            None,
            BackupIssueKind::UnsupportedVersion,
            format!(
                "unsupported backup version {}, supported version is {}",
                backup.version, MEMORY_BACKUP_VERSION
            ),
        ));
    }

    if backup.event_envelope_version != EVENT_ENVELOPE_VERSION {
        issues.push(error_issue(
            None,
            BackupIssueKind::UnsupportedVersion,
            format!(
                "unsupported event envelope version {}, supported version is {}",
                backup.event_envelope_version, EVENT_ENVELOPE_VERSION
            ),
        ));
    }

    if !checksum_valid {
        issues.push(error_issue(
            None,
            BackupIssueKind::ChecksumMismatch,
            "backup checksum mismatch".to_string(),
        ));
    }

    if backup.record_count != actual_record_count
        || backup.last_event_id != actual_last_event_id
        || backup.record_ledger_checksum != actual_record_ledger_checksum
    {
        records_valid = false;
        issues.push(error_issue(
            None,
            BackupIssueKind::RecordLedgerMismatch,
            "backup record-ledger metadata does not match included records".to_string(),
        ));
    }

    for (index, event) in backup.records.iter().enumerate() {
        let expected_sequence = index as u64 + 1;
        let record = index + 1;

        if event.sequence != expected_sequence {
            records_valid = false;
            issues.push(error_issue(
                Some(record),
                BackupIssueKind::RecordSequenceMismatch,
                format!(
                    "expected sequence {expected_sequence}, found {}",
                    event.sequence
                ),
            ));
            continue;
        }

        // Legacy record versions (below the current one) are accepted: a backup of
        // a legacy ledger must stay restorable. Only a future/unknown version is
        // rejected. `validate_checksum` picks SHA-256 or legacy FNV per version.
        if event.version > EVENT_ENVELOPE_VERSION {
            records_valid = false;
            issues.push(error_issue(
                Some(record),
                BackupIssueKind::UnsupportedVersion,
                format!(
                    "unsupported record envelope version {}, supported version is {}",
                    event.version, EVENT_ENVELOPE_VERSION
                ),
            ));
            continue;
        }

        if !event.validate_checksum() {
            records_valid = false;
            issues.push(error_issue(
                Some(record),
                BackupIssueKind::RecordChecksumMismatch,
                "record checksum mismatch".to_string(),
            ));
            continue;
        }

        #[cfg(feature = "tamper-evidence")]
        if event.is_chained() {
            let expected_prev = last_chained.clone().unwrap_or_default();
            let recorded_prev = event.prev_hash.clone().unwrap_or_default();
            if recorded_prev != expected_prev {
                records_valid = false;
                chain_valid = false;
                issues.push(error_issue(
                    Some(record),
                    BackupIssueKind::RecordHashChainBroken,
                    "record hash-chain broken: recorded prev_hash does not match the previous record's chained hash"
                        .to_string(),
                ));
                continue;
            }
        } else if options.require_chained {
            records_valid = false;
            chain_valid = false;
            issues.push(error_issue(
                Some(record),
                BackupIssueKind::RecordHashChainMissing,
                "record hash-chain missing: backup validation requires every record to be chained"
                    .to_string(),
            ));
            continue;
        }

        #[cfg(feature = "tamper-evidence")]
        {
            last_chained = Some(event.chain_hash());
        }
    }

    BackupValidation {
        valid: issues.is_empty(),
        backup_version: Some(backup.version),
        backup_record_count: Some(backup.record_count),
        backup_last_event_id: backup.last_event_id.clone(),
        backup_record_ledger_checksum: Some(backup.record_ledger_checksum.clone()),
        checksum_valid,
        records_valid,
        #[cfg(feature = "tamper-evidence")]
        chain_valid,
        #[cfg(feature = "tamper-evidence")]
        require_chained: options.require_chained,
        semantic_tier: Some(backup.semantic_tier.clone()),
        issues,
    }
}

pub(crate) fn target_not_empty_issue(event_count: usize) -> BackupIssue {
    error_issue(
        None,
        BackupIssueKind::TargetNotEmpty,
        format!(
            "target database already contains {event_count} record(s); restore requires an empty database"
        ),
    )
}

pub(crate) fn restore_verification_issue() -> BackupIssue {
    error_issue(
        None,
        BackupIssueKind::RestoreVerificationMismatch,
        "restored database did not match backup checksum after write".to_string(),
    )
}

pub(crate) fn backup_drill_report(
    backup_path: &Path,
    target_database: &Path,
    backup_validation: BackupValidation,
    restore_dry_run: BackupRestoreReport,
) -> BackupDrillReport {
    let semantic_rebuild_required = restore_dry_run
        .semantic_restore_policy
        .as_ref()
        .is_some_and(|policy| *policy == SemanticTierRestorePolicy::RebuildFromRecords);
    let valid = backup_validation.valid && restore_dry_run.valid && restore_dry_run.dry_run;
    let operator_next_actions = backup_drill_actions(
        valid,
        semantic_rebuild_required,
        &backup_validation,
        &restore_dry_run,
    );

    BackupDrillReport {
        valid,
        backup_path: backup_path.display().to_string(),
        target_database: target_database.display().to_string(),
        backup_validation,
        restore_dry_run,
        semantic_rebuild_required,
        operator_next_actions,
    }
}

fn backup_drill_actions(
    valid: bool,
    semantic_rebuild_required: bool,
    backup_validation: &BackupValidation,
    restore_dry_run: &BackupRestoreReport,
) -> Vec<String> {
    if !backup_validation.valid {
        return vec![
            "Regenerate or repair the backup before attempting restore.".to_string(),
            "Run backup-validate again and require a valid report before proceeding.".to_string(),
        ];
    }

    if !restore_dry_run.target_was_empty {
        return vec![
            "Choose an empty target database for restore.".to_string(),
            "Run backup-drill again before any non-dry-run restore.".to_string(),
        ];
    }

    if !valid {
        return vec![
            "Resolve restore validation issues before attempting a non-dry-run restore."
                .to_string(),
            "Run backup-drill again after the target and backup are corrected.".to_string(),
        ];
    }

    let mut actions = vec![
        "Run restore without --dry-run only when the selected target database is intentionally empty."
            .to_string(),
        "Run validate against the restored database before promoting it.".to_string(),
    ];
    if semantic_rebuild_required {
        actions.push(
            "Run semantic-rebuild after restore because semantic indexes are derived state."
                .to_string(),
        );
    }
    actions
}

fn semantic_tier_backup(collections: Vec<String>) -> SemanticTierBackup {
    let endpoint = std::env::var("NAHUALI_QDRANT_URL")
        .ok()
        .and_then(|value| {
            let value = value.trim().to_string();
            if value.is_empty() { None } else { Some(value) }
        })
        .or_else(|| Some(DEFAULT_QDRANT_URL.to_string()));

    SemanticTierBackup {
        provider: SemanticTierProvider::Qdrant,
        derived: true,
        endpoint,
        collections,
        snapshot_status: SemanticTierSnapshotStatus::NotIncluded,
        restore_policy: SemanticTierRestorePolicy::RebuildFromRecords,
        message:
            "Qdrant vectors are derived from SurrealDB records and must be rebuilt after restore"
                .to_string(),
    }
}

fn backup_checksum(backup: &MemoryBackup) -> String {
    let body = BackupChecksumBody {
        version: backup.version,
        created_at_ms: backup.created_at_ms,
        engine_version: &backup.engine_version,
        source_database: &backup.source_database,
        event_envelope_version: backup.event_envelope_version,
        record_count: backup.record_count,
        last_event_id: &backup.last_event_id,
        record_ledger_checksum: &backup.record_ledger_checksum,
        semantic_tier: &backup.semantic_tier,
        records: &backup.records,
    };
    checksum_json(&body)
}

fn error_issue(record: Option<usize>, kind: BackupIssueKind, message: String) -> BackupIssue {
    BackupIssue {
        record,
        kind,
        severity: BackupIssueSeverity::Error,
        message,
    }
}

fn now_ms() -> u64 {
    UNIX_EPOCH
        .elapsed()
        .expect("system clock is after epoch")
        .as_millis() as u64
}

#[derive(Serialize)]
struct BackupChecksumBody<'a> {
    version: u32,
    created_at_ms: u64,
    engine_version: &'a str,
    source_database: &'a str,
    event_envelope_version: u32,
    record_count: usize,
    last_event_id: &'a Option<String>,
    record_ledger_checksum: &'a str,
    semantic_tier: &'a SemanticTierBackup,
    records: &'a [EventEnvelope],
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "tamper-evidence")]
    use super::*;
    #[cfg(feature = "tamper-evidence")]
    use crate::{EpisodeRecorded, MemoryEvent};

    #[cfg(feature = "tamper-evidence")]
    #[test]
    fn backup_validation_fails_closed_on_chain_stripping() {
        let mut backup = create_backup(
            Path::new("memory.surrealdb"),
            &chained_events(3),
            Vec::new(),
        );

        for record in &mut backup.records {
            record.prev_hash = None;
            assert!(
                record.validate_checksum(),
                "stripping prev_hash must not invalidate the per-event checksum"
            );
        }
        refresh_backup_checksums(&mut backup);

        // Default (fail-closed): the chain-stripped backup is rejected even with a
        // recomputed backup checksum.
        let default_report = validate_backup(&backup);
        assert!(!default_report.valid);
        assert!(!default_report.records_valid);
        assert!(!default_report.chain_valid);
        assert!(default_report.require_chained);
        assert!(
            default_report
                .issues
                .iter()
                .any(|issue| issue.kind == BackupIssueKind::RecordHashChainMissing),
            "expected a chain-missing issue, got {:?}",
            default_report.issues
        );
        assert!(BackupValidationOptions::default().require_chained);

        // Legacy-permissive escape hatch: the same backup is accepted.
        let permissive_report =
            validate_backup_with_options(&backup, &BackupValidationOptions::legacy_permissive());

        assert!(permissive_report.valid);
        assert!(permissive_report.records_valid);
        assert!(permissive_report.chain_valid);
        assert!(!permissive_report.require_chained);
    }

    #[cfg(feature = "tamper-evidence")]
    #[test]
    fn backup_validation_rejects_broken_chain_links_even_with_valid_manifest_checksums() {
        let mut backup = create_backup(
            Path::new("memory.surrealdb"),
            &chained_events(3),
            Vec::new(),
        );
        backup.records[1].prev_hash = Some("sha256:not-the-previous-hash".to_string());
        refresh_backup_checksums(&mut backup);

        let report = validate_backup(&backup);

        assert!(!report.valid);
        assert!(report.checksum_valid);
        assert!(!report.records_valid);
        assert!(!report.chain_valid);
        assert!(
            report.issues.iter().any(|issue| {
                issue.record == Some(2) && issue.kind == BackupIssueKind::RecordHashChainBroken
            }),
            "expected a chain-broken issue at record 2, got {:?}",
            report.issues
        );
    }

    #[cfg(feature = "tamper-evidence")]
    fn chained_events(count: u64) -> Vec<EventEnvelope> {
        let mut events: Vec<EventEnvelope> = Vec::new();
        for sequence in 1..=count {
            let prev = events.last().map(EventEnvelope::chain_hash);
            events.push(EventEnvelope::with_chain(
                sequence,
                1_700_000_000_000 + sequence,
                MemoryEvent::EpisodeRecorded(EpisodeRecorded {
                    id: format!("episode_{sequence}"),
                    content: format!("Backup validation event {sequence}"),
                    tags: vec!["backup".to_string()],
                    mentions: Vec::new(),
                    source_id: None,
                    source_position: None,
                    source_role: None,
                    scope: None,
                }),
                prev.as_deref(),
            ));
        }
        events
    }

    #[cfg(feature = "tamper-evidence")]
    fn refresh_backup_checksums(backup: &mut MemoryBackup) {
        backup.record_count = backup.records.len();
        backup.last_event_id = backup.records.last().map(|event| event.id.clone());
        backup.record_ledger_checksum = record_ledger_checksum(&backup.records);
        backup.checksum = backup_checksum(backup);
    }
}
