use nahuali_core::{LedgerAudit, LedgerAuditCounts, LedgerAuditEntry, LedgerAuditIntegrity};
use rmcp::schemars;
use serde::Serialize;

#[cfg(feature = "tamper-evidence")]
use super::LedgerChainStatusView;
use super::{ScopeView, json_string};

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct AuditResult {
    from_sequence: u64,
    to_sequence: u64,
    total_event_count: usize,
    range_event_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    from_timestamp_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    to_timestamp_ms: Option<u64>,
    #[cfg(feature = "tamper-evidence")]
    #[serde(skip_serializing_if = "Option::is_none")]
    from_tip: Option<String>,
    #[cfg(feature = "tamper-evidence")]
    #[serde(skip_serializing_if = "Option::is_none")]
    to_tip: Option<String>,
    integrity: LedgerAuditIntegrityView,
    counts: LedgerAuditCountsView,
    entries: Vec<LedgerAuditEntryView>,
}

impl From<LedgerAudit> for AuditResult {
    fn from(audit: LedgerAudit) -> Self {
        Self {
            from_sequence: audit.from_sequence,
            to_sequence: audit.to_sequence,
            total_event_count: audit.total_event_count,
            range_event_count: audit.range_event_count,
            from_timestamp_ms: audit.from_timestamp_ms,
            to_timestamp_ms: audit.to_timestamp_ms,
            #[cfg(feature = "tamper-evidence")]
            from_tip: audit.from_tip,
            #[cfg(feature = "tamper-evidence")]
            to_tip: audit.to_tip,
            integrity: audit.integrity.into(),
            counts: audit.counts.into(),
            entries: audit
                .entries
                .into_iter()
                .map(LedgerAuditEntryView::from)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct LedgerAuditIntegrityView {
    checksums_valid: bool,
    sequence_contiguous: bool,
    #[cfg(feature = "tamper-evidence")]
    chain_intact: bool,
    #[cfg(feature = "tamper-evidence")]
    chain_status: LedgerChainStatusView,
    #[cfg(feature = "tamper-evidence")]
    #[serde(skip_serializing_if = "Option::is_none")]
    merkle_root: Option<String>,
    verified: bool,
}

impl From<LedgerAuditIntegrity> for LedgerAuditIntegrityView {
    fn from(integrity: LedgerAuditIntegrity) -> Self {
        Self {
            checksums_valid: integrity.checksums_valid,
            sequence_contiguous: integrity.sequence_contiguous,
            #[cfg(feature = "tamper-evidence")]
            chain_intact: integrity.chain_intact,
            #[cfg(feature = "tamper-evidence")]
            chain_status: integrity.chain_status.into(),
            #[cfg(feature = "tamper-evidence")]
            merkle_root: integrity.merkle_root,
            verified: integrity.verified,
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct LedgerAuditCountsView {
    sources_recorded: usize,
    episodes_recorded: usize,
    facts_asserted: usize,
    relations_recorded: usize,
    procedures_recorded: usize,
    intentions_recorded: usize,
    intentions_updated: usize,
    intention_status_changes: usize,
    reviews_recorded: usize,
}

impl From<LedgerAuditCounts> for LedgerAuditCountsView {
    fn from(counts: LedgerAuditCounts) -> Self {
        Self {
            sources_recorded: counts.sources_recorded,
            episodes_recorded: counts.episodes_recorded,
            facts_asserted: counts.facts_asserted,
            relations_recorded: counts.relations_recorded,
            procedures_recorded: counts.procedures_recorded,
            intentions_recorded: counts.intentions_recorded,
            intentions_updated: counts.intentions_updated,
            intention_status_changes: counts.intention_status_changes,
            reviews_recorded: counts.reviews_recorded,
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct LedgerAuditEntryView {
    sequence: u64,
    id: String,
    timestamp_ms: u64,
    kind: String,
    summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<ScopeView>,
}

impl From<LedgerAuditEntry> for LedgerAuditEntryView {
    fn from(entry: LedgerAuditEntry) -> Self {
        Self {
            sequence: entry.sequence,
            id: entry.id,
            timestamp_ms: entry.timestamp_ms,
            kind: json_string(&entry.kind),
            summary: entry.summary,
            scope: entry.scope.map(ScopeView::from),
        }
    }
}
