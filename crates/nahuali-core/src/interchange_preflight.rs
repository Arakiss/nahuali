use std::collections::{BTreeSet, HashSet};

use serde::{Deserialize, Serialize};

use super::{InterchangeImportCounts, MemoryInterchange, clean_optional};

/// Boundary and evidence summary computed before an interchange import writes.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct InterchangeImportPreflight {
    /// Source records represented by the import.
    pub source_count: usize,
    /// Episodes linked to a source record.
    pub sourced_episode_count: usize,
    /// Episodes without a source record.
    pub unsourced_episode_count: usize,
    /// Total bytes of episode content inspected by import.
    pub episode_byte_len: u64,
    /// Explicit derived records included in the document.
    pub derived_record_count: usize,
    /// Derived records that cite a source episode reference.
    pub evidence_linked_record_count: usize,
    /// Derived records that do not cite source evidence.
    pub evidence_gap_count: usize,
    /// Unique source episodes referenced by derived records.
    pub referenced_episode_count: usize,
    /// Source episodes not referenced by any derived record.
    pub unreferenced_episode_count: usize,
    /// Import records with an explicit memory scope.
    pub scoped_record_count: usize,
    /// Import records without an explicit memory scope.
    pub unscoped_record_count: usize,
    /// Unique normalized scope keys represented by this import.
    pub scope_keys: Vec<String>,
}

pub(super) fn preflight(
    document: &MemoryInterchange,
    counts: &InterchangeImportCounts,
) -> InterchangeImportPreflight {
    let derived_record_count = counts.claims + counts.links + counts.procedures + counts.intentions;
    let evidence_linked_record_count = evidence_linked_record_count(document);
    let referenced_episode_count = referenced_episode_count(document);
    let scoped_record_count = scoped_record_count(document);
    let record_count = counts.sources
        + counts.episodes
        + counts.claims
        + counts.links
        + counts.procedures
        + counts.intentions;
    let sourced_episode_count = sourced_episode_count(document);

    InterchangeImportPreflight {
        source_count: document.sources.len(),
        sourced_episode_count,
        unsourced_episode_count: document
            .episodes
            .len()
            .saturating_sub(sourced_episode_count),
        episode_byte_len: episode_byte_len(document),
        derived_record_count,
        evidence_linked_record_count,
        evidence_gap_count: derived_record_count.saturating_sub(evidence_linked_record_count),
        referenced_episode_count,
        unreferenced_episode_count: document
            .episodes
            .len()
            .saturating_sub(referenced_episode_count),
        scoped_record_count,
        unscoped_record_count: record_count.saturating_sub(scoped_record_count),
        scope_keys: scope_keys(document),
    }
}

fn episode_byte_len(document: &MemoryInterchange) -> u64 {
    document
        .episodes
        .iter()
        .map(|episode| episode.content.len() as u64)
        .sum()
}

fn evidence_linked_record_count(document: &MemoryInterchange) -> usize {
    document
        .claims
        .iter()
        .filter(|record| clean_optional(&record.source_episode_ref).is_some())
        .count()
        + document
            .links
            .iter()
            .filter(|record| clean_optional(&record.source_episode_ref).is_some())
            .count()
        + document
            .procedures
            .iter()
            .filter(|record| clean_optional(&record.source_episode_ref).is_some())
            .count()
        + document
            .intentions
            .iter()
            .filter(|record| clean_optional(&record.source_episode_ref).is_some())
            .count()
}

fn referenced_episode_count(document: &MemoryInterchange) -> usize {
    let known_refs = document
        .episodes
        .iter()
        .filter_map(|episode| clean_optional(&episode.ref_id))
        .collect::<HashSet<_>>();
    let mut referenced_refs = HashSet::new();

    for ref_id in document
        .claims
        .iter()
        .filter_map(|record| clean_optional(&record.source_episode_ref))
        .chain(
            document
                .links
                .iter()
                .filter_map(|record| clean_optional(&record.source_episode_ref)),
        )
        .chain(
            document
                .procedures
                .iter()
                .filter_map(|record| clean_optional(&record.source_episode_ref)),
        )
        .chain(
            document
                .intentions
                .iter()
                .filter_map(|record| clean_optional(&record.source_episode_ref)),
        )
    {
        if known_refs.contains(&ref_id) {
            referenced_refs.insert(ref_id);
        }
    }

    referenced_refs.len()
}

fn scoped_record_count(document: &MemoryInterchange) -> usize {
    document
        .sources
        .iter()
        .filter(|record| record.scope.is_some())
        .count()
        + document
            .episodes
            .iter()
            .filter(|record| record.scope.is_some())
            .count()
        + document
            .claims
            .iter()
            .filter(|record| record.scope.is_some())
            .count()
        + document
            .links
            .iter()
            .filter(|record| record.scope.is_some())
            .count()
        + document
            .procedures
            .iter()
            .filter(|record| record.scope.is_some())
            .count()
        + document
            .intentions
            .iter()
            .filter(|record| record.scope.is_some())
            .count()
}

fn scope_keys(document: &MemoryInterchange) -> Vec<String> {
    document
        .sources
        .iter()
        .filter_map(|record| record.scope.as_ref())
        .chain(
            document
                .episodes
                .iter()
                .filter_map(|record| record.scope.as_ref()),
        )
        .chain(
            document
                .claims
                .iter()
                .filter_map(|record| record.scope.as_ref()),
        )
        .chain(
            document
                .links
                .iter()
                .filter_map(|record| record.scope.as_ref()),
        )
        .chain(
            document
                .procedures
                .iter()
                .filter_map(|record| record.scope.as_ref()),
        )
        .chain(
            document
                .intentions
                .iter()
                .filter_map(|record| record.scope.as_ref()),
        )
        .map(|scope| scope.key.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn sourced_episode_count(document: &MemoryInterchange) -> usize {
    document
        .episodes
        .iter()
        .filter(|record| clean_optional(&record.source_ref).is_some())
        .count()
}
