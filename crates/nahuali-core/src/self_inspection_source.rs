use crate::{
    HealthDimension, HealthSeverity, MemoryData, SelfInspectionFinding, SelfInspectionFindingKind,
};

pub(super) fn append_source_coverage_findings(
    data: &MemoryData,
    findings: &mut Vec<SelfInspectionFinding>,
) {
    let unsourced_episodes = data
        .episodes
        .iter()
        .filter(|episode| episode.source_id.is_none())
        .map(|episode| episode.id.clone())
        .collect::<Vec<_>>();
    let unsupported_derived = unsupported_derived_ids(data);
    if unsourced_episodes.is_empty() && unsupported_derived.is_empty() {
        return;
    }

    let mut evidence_ids = unsourced_episodes
        .iter()
        .chain(unsupported_derived.iter())
        .take(12)
        .cloned()
        .collect::<Vec<_>>();
    evidence_ids.sort();

    let severity = if !unsupported_derived.is_empty() {
        HealthSeverity::Medium
    } else {
        HealthSeverity::Low
    };
    let detail = format!(
        "{} episode(s) lack source records and {} derived memory item(s) lack source episode evidence.",
        unsourced_episodes.len(),
        unsupported_derived.len()
    );

    findings.push(SelfInspectionFinding {
        id: super::finding_id(
            &SelfInspectionFindingKind::SourceCoverage,
            &evidence_ids,
            &detail,
        ),
        kind: SelfInspectionFindingKind::SourceCoverage,
        severity,
        title: "Source coverage gap".to_string(),
        detail,
        dimensions: vec![HealthDimension::Completeness, HealthDimension::UnsupportedMemory],
        evidence_ids,
        suggested_action:
            "Attach source episodes or record source-backed observations before relying on this memory."
                .to_string(),
    });
}

fn unsupported_derived_ids(data: &MemoryData) -> Vec<String> {
    data.claims
        .iter()
        .filter(|claim| claim.source_episode_id.is_none())
        .map(|claim| claim.id.clone())
        .chain(
            data.links
                .iter()
                .filter(|link| link.source_episode_id.is_none())
                .map(|link| link.id.clone()),
        )
        .chain(
            data.procedures
                .iter()
                .filter(|procedure| procedure.source_episode_id.is_none())
                .map(|procedure| procedure.id.clone()),
        )
        .chain(
            data.intentions
                .iter()
                .filter(|intention| intention.source_episode_id.is_none())
                .map(|intention| intention.id.clone()),
        )
        .collect()
}
