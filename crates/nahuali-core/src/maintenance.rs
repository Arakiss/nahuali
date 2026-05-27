use std::{fs, path::Path, time::UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::{
    EVENT_ENVELOPE_VERSION, EventEnvelope, MEMORY_DATA_VERSION, MemoryData,
    error::{NahualiError, Result},
};

/// Current optional snapshot file format version.
pub const MEMORY_SNAPSHOT_VERSION: u32 = 1;

/// Optional cached projection produced from a validated record-ledger replay.
///
/// A snapshot is a maintenance artifact only. It is never the authoritative
/// memory store, and opening [`crate::MemoryEngine`] always validates and replays
/// the record ledger instead of trusting this structure.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct MemorySnapshot {
    /// Snapshot file format version.
    pub version: u32,
    /// Timestamp in milliseconds when the snapshot was generated.
    pub generated_at_ms: u64,
    /// Projection schema version included in `data`.
    pub projection_version: u32,
    /// Event-envelope version used by the source record ledger.
    pub event_envelope_version: u32,
    /// Number of source events represented by the snapshot.
    pub event_count: usize,
    /// Identifier of the last source event represented by the snapshot.
    pub last_event_id: Option<String>,
    /// Deterministic checksum of the source event envelopes.
    pub record_ledger_checksum: String,
    /// Projected memory data produced by replaying the source record ledger.
    pub data: MemoryData,
    /// Deterministic checksum of the snapshot body excluding this field.
    pub checksum: String,
}

impl MemorySnapshot {
    /// Return a compact summary suitable for CLI and maintenance output.
    pub fn summary(&self) -> SnapshotSummary {
        SnapshotSummary {
            version: self.version,
            generated_at_ms: self.generated_at_ms,
            projection_version: self.projection_version,
            event_envelope_version: self.event_envelope_version,
            event_count: self.event_count,
            last_event_id: self.last_event_id.clone(),
            record_ledger_checksum: self.record_ledger_checksum.clone(),
            checksum: self.checksum.clone(),
        }
    }

    /// Return whether the stored snapshot checksum matches the snapshot body.
    pub fn checksum_valid(&self) -> bool {
        self.checksum == snapshot_checksum(self)
    }
}

/// Compact snapshot metadata without projected memory contents.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SnapshotSummary {
    /// Snapshot file format version.
    pub version: u32,
    /// Timestamp in milliseconds when the snapshot was generated.
    pub generated_at_ms: u64,
    /// Projection schema version included in the snapshot.
    pub projection_version: u32,
    /// Event-envelope version used by the source record ledger.
    pub event_envelope_version: u32,
    /// Number of source events represented by the snapshot.
    pub event_count: usize,
    /// Identifier of the last source event represented by the snapshot.
    pub last_event_id: Option<String>,
    /// Deterministic checksum of the source event envelopes.
    pub record_ledger_checksum: String,
    /// Deterministic checksum of the snapshot body.
    pub checksum: String,
}

/// Non-destructive local maintenance report.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct MaintenanceReport {
    /// Number of validated events in the authoritative record ledger.
    pub event_count: usize,
    /// Identifier of the last validated event in the authoritative record ledger.
    pub last_event_id: Option<String>,
    /// Whether this build can produce optional snapshots.
    pub snapshot_supported: bool,
    /// Whether creating a snapshot would currently produce a non-empty artifact.
    pub snapshot_recommended: bool,
    /// Whether destructive compaction is supported by this build.
    pub compaction_supported: bool,
    /// Human-readable policy describing the current compaction boundary.
    pub compaction_policy: String,
    /// Non-destructive actions an operator can take now.
    pub actions: Vec<String>,
}

/// Result of validating a snapshot against a fresh record-ledger replay.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SnapshotValidation {
    /// Whether the snapshot is valid for the current record ledger and projection.
    pub valid: bool,
    /// Snapshot format version, if the snapshot could be parsed.
    pub snapshot_version: Option<u32>,
    /// Event count recorded in the snapshot, if the snapshot could be parsed.
    pub snapshot_event_count: Option<usize>,
    /// Last event ID recorded in the snapshot, if the snapshot could be parsed.
    pub snapshot_last_event_id: Option<String>,
    /// Record-ledger checksum recorded in the snapshot, if the snapshot could be parsed.
    pub snapshot_record_ledger_checksum: Option<String>,
    /// Number of events in the current authoritative record ledger.
    pub current_event_count: usize,
    /// Last event ID in the current authoritative record ledger.
    pub current_last_event_id: Option<String>,
    /// Record-ledger checksum computed from the current authoritative record ledger.
    pub current_record_ledger_checksum: String,
    /// Whether the snapshot body checksum is valid.
    pub checksum_valid: bool,
    /// Whether snapshot data equals a fresh projection of the current record ledger.
    pub replay_equivalent: bool,
    /// Validation issues found while checking the snapshot.
    pub issues: Vec<SnapshotIssue>,
}

/// Snapshot validation issue.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SnapshotIssue {
    /// Machine-readable issue category.
    pub kind: SnapshotIssueKind,
    /// Issue severity.
    pub severity: SnapshotIssueSeverity,
    /// Human-readable diagnostic.
    pub message: String,
}

/// Snapshot validation issue category.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotIssueKind {
    /// The snapshot file is not valid JSON or does not match the snapshot schema.
    ParseError,
    /// The snapshot declares a version this build does not support.
    UnsupportedVersion,
    /// The snapshot body checksum does not match the stored checksum.
    ChecksumMismatch,
    /// Snapshot record-ledger metadata does not match the current record ledger.
    RecordLedgerMismatch,
    /// Snapshot projected data does not match a fresh record-ledger replay.
    ReplayMismatch,
}

/// Snapshot validation issue severity.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotIssueSeverity {
    /// Error that prevents trusting the snapshot.
    Error,
}

pub(crate) fn maintenance_report(events: &[EventEnvelope]) -> MaintenanceReport {
    let event_count = events.len();
    let last_event_id = events.last().map(|event| event.id.clone());
    let mut actions = vec!["validate the record ledger before trusting memory".to_string()];

    if event_count > 0 {
        actions.push("write an optional snapshot for later replay-equivalence checks".to_string());
    } else {
        actions.push("record memory before writing a snapshot".to_string());
    }

    MaintenanceReport {
        event_count,
        last_event_id,
        snapshot_supported: true,
        snapshot_recommended: event_count > 0,
        compaction_supported: false,
        compaction_policy:
            "record ledger remains authoritative; destructive compaction requires a future versioned format with replay-equivalence validation"
                .to_string(),
        actions,
    }
}

pub(crate) fn create_snapshot(events: &[EventEnvelope], data: &MemoryData) -> MemorySnapshot {
    let mut snapshot = MemorySnapshot {
        version: MEMORY_SNAPSHOT_VERSION,
        generated_at_ms: now_ms(),
        projection_version: MEMORY_DATA_VERSION,
        event_envelope_version: EVENT_ENVELOPE_VERSION,
        event_count: events.len(),
        last_event_id: events.last().map(|event| event.id.clone()),
        record_ledger_checksum: record_ledger_checksum(events),
        data: data.clone(),
        checksum: String::new(),
    };
    snapshot.checksum = snapshot_checksum(&snapshot);
    snapshot
}

pub(crate) fn write_snapshot_file(path: &Path, snapshot: &MemorySnapshot) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|source| NahualiError::WriteSnapshot {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let encoded = serde_json::to_string_pretty(snapshot).map_err(NahualiError::EncodeSnapshot)?;
    fs::write(path, format!("{encoded}\n")).map_err(|source| NahualiError::WriteSnapshot {
        path: path.to_path_buf(),
        source,
    })
}

pub(crate) fn validate_snapshot_file(
    path: &Path,
    events: &[EventEnvelope],
    data: &MemoryData,
) -> Result<SnapshotValidation> {
    let raw = fs::read_to_string(path).map_err(|source| NahualiError::ReadSnapshot {
        path: path.to_path_buf(),
        source,
    })?;
    let current_record_ledger_checksum = record_ledger_checksum(events);
    let current_event_count = events.len();
    let current_last_event_id = events.last().map(|event| event.id.clone());

    let snapshot: MemorySnapshot = match serde_json::from_str(&raw) {
        Ok(snapshot) => snapshot,
        Err(source) => {
            return Ok(SnapshotValidation {
                valid: false,
                snapshot_version: None,
                snapshot_event_count: None,
                snapshot_last_event_id: None,
                snapshot_record_ledger_checksum: None,
                current_event_count,
                current_last_event_id,
                current_record_ledger_checksum,
                checksum_valid: false,
                replay_equivalent: false,
                issues: vec![SnapshotIssue {
                    kind: SnapshotIssueKind::ParseError,
                    severity: SnapshotIssueSeverity::Error,
                    message: format!("invalid snapshot JSON: {source}"),
                }],
            });
        }
    };

    Ok(validate_snapshot(
        &snapshot,
        events,
        data,
        current_record_ledger_checksum,
    ))
}

fn validate_snapshot(
    snapshot: &MemorySnapshot,
    events: &[EventEnvelope],
    data: &MemoryData,
    current_record_ledger_checksum: String,
) -> SnapshotValidation {
    let current_event_count = events.len();
    let current_last_event_id = events.last().map(|event| event.id.clone());
    let checksum_valid = snapshot.checksum_valid();
    let record_ledger_matches = snapshot.event_count == current_event_count
        && snapshot.last_event_id == current_last_event_id
        && snapshot.record_ledger_checksum == current_record_ledger_checksum;
    let replay_equivalent = snapshot.data == *data;
    let mut issues = Vec::new();

    if snapshot.version != MEMORY_SNAPSHOT_VERSION {
        issues.push(SnapshotIssue {
            kind: SnapshotIssueKind::UnsupportedVersion,
            severity: SnapshotIssueSeverity::Error,
            message: format!(
                "unsupported snapshot version {}, supported version is {}",
                snapshot.version, MEMORY_SNAPSHOT_VERSION
            ),
        });
    }

    if snapshot.projection_version != MEMORY_DATA_VERSION {
        issues.push(SnapshotIssue {
            kind: SnapshotIssueKind::UnsupportedVersion,
            severity: SnapshotIssueSeverity::Error,
            message: format!(
                "unsupported projection version {}, supported version is {}",
                snapshot.projection_version, MEMORY_DATA_VERSION
            ),
        });
    }

    if snapshot.event_envelope_version != EVENT_ENVELOPE_VERSION {
        issues.push(SnapshotIssue {
            kind: SnapshotIssueKind::UnsupportedVersion,
            severity: SnapshotIssueSeverity::Error,
            message: format!(
                "unsupported event envelope version {}, supported version is {}",
                snapshot.event_envelope_version, EVENT_ENVELOPE_VERSION
            ),
        });
    }

    if !checksum_valid {
        issues.push(SnapshotIssue {
            kind: SnapshotIssueKind::ChecksumMismatch,
            severity: SnapshotIssueSeverity::Error,
            message: "snapshot checksum mismatch".to_string(),
        });
    }

    if !record_ledger_matches {
        issues.push(SnapshotIssue {
            kind: SnapshotIssueKind::RecordLedgerMismatch,
            severity: SnapshotIssueSeverity::Error,
            message: "snapshot record-ledger metadata does not match the current record ledger"
                .to_string(),
        });
    }

    if !replay_equivalent {
        issues.push(SnapshotIssue {
            kind: SnapshotIssueKind::ReplayMismatch,
            severity: SnapshotIssueSeverity::Error,
            message: "snapshot data does not match a fresh record-ledger replay".to_string(),
        });
    }

    SnapshotValidation {
        valid: issues.is_empty(),
        snapshot_version: Some(snapshot.version),
        snapshot_event_count: Some(snapshot.event_count),
        snapshot_last_event_id: snapshot.last_event_id.clone(),
        snapshot_record_ledger_checksum: Some(snapshot.record_ledger_checksum.clone()),
        current_event_count,
        current_last_event_id,
        current_record_ledger_checksum,
        checksum_valid,
        replay_equivalent,
        issues,
    }
}

fn snapshot_checksum(snapshot: &MemorySnapshot) -> String {
    let body = SnapshotChecksumBody {
        version: snapshot.version,
        generated_at_ms: snapshot.generated_at_ms,
        projection_version: snapshot.projection_version,
        event_envelope_version: snapshot.event_envelope_version,
        event_count: snapshot.event_count,
        last_event_id: &snapshot.last_event_id,
        record_ledger_checksum: &snapshot.record_ledger_checksum,
        data: &snapshot.data,
    };
    checksum_json(&body)
}

pub(crate) fn record_ledger_checksum(events: &[EventEnvelope]) -> String {
    checksum_json(events)
}

pub(crate) fn checksum_json(value: &(impl Serialize + ?Sized)) -> String {
    let encoded = serde_json::to_vec(value).expect("memory maintenance data must serialize");
    format!("{:016x}", fnv1a64(&encoded))
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    bytes.iter().fold(FNV_OFFSET, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
    })
}

fn now_ms() -> u64 {
    UNIX_EPOCH
        .elapsed()
        .expect("system clock is after epoch")
        .as_millis() as u64
}

#[derive(Serialize)]
struct SnapshotChecksumBody<'a> {
    version: u32,
    generated_at_ms: u64,
    projection_version: u32,
    event_envelope_version: u32,
    event_count: usize,
    last_event_id: &'a Option<String>,
    record_ledger_checksum: &'a str,
    data: &'a MemoryData,
}
