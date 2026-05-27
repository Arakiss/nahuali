use super::{
    InterchangeClaim, InterchangeEpisode, InterchangeIssueKind, InterchangeProcedure,
    InterchangeSource, MEMORY_INTERCHANGE_VERSION, MemoryInterchange, validate,
};
use crate::{MemoryScope, ProcedureKind, SourceKind};

#[test]
fn validates_supported_document_before_import() {
    let scope = MemoryScope::parse("project:Nahuali").expect("scope parses");
    let document = MemoryInterchange {
        version: MEMORY_INTERCHANGE_VERSION,
        sources: vec![InterchangeSource {
            ref_id: "conversation-a".to_string(),
            kind: SourceKind::Conversation,
            title: Some("Release review".to_string()),
            uri: None,
            content_checksum: Some("checksum-a".to_string()),
            byte_len: 128,
            metadata: Default::default(),
            scope: Some(scope.clone()),
            timestamp_ms: None,
        }],
        episodes: vec![InterchangeEpisode {
            ref_id: Some("episode-a".to_string()),
            content: "Lena owns release notes".to_string(),
            tags: Vec::new(),
            mentions: Vec::new(),
            source_role: None,
            source_ref: Some("conversation-a".to_string()),
            source_position: Some(1),
            scope: Some(scope.clone()),
            timestamp_ms: None,
        }],
        claims: vec![InterchangeClaim {
            subject: "Lena".to_string(),
            predicate: "owns".to_string(),
            object: "release notes".to_string(),
            source_episode_ref: Some("episode-a".to_string()),
            confidence: 0.9,
            scope: Some(scope),
            timestamp_ms: None,
        }],
        links: Vec::new(),
        procedures: Vec::new(),
        intentions: Vec::new(),
    };

    let report = validate(&document, true);

    assert!(report.valid);
    assert!(report.dry_run);
    assert_eq!(report.appendable_event_count, 3);
    assert_eq!(report.imported_event_count, 0);
    assert_eq!(report.counts.sources, 1);
    assert_eq!(report.preflight.source_count, 1);
    assert_eq!(report.preflight.sourced_episode_count, 1);
    assert_eq!(report.preflight.unsourced_episode_count, 0);
    assert_eq!(report.preflight.derived_record_count, 1);
    assert_eq!(report.preflight.evidence_linked_record_count, 1);
    assert_eq!(report.preflight.evidence_gap_count, 0);
    assert_eq!(report.preflight.referenced_episode_count, 1);
    assert_eq!(report.preflight.unreferenced_episode_count, 0);
    assert_eq!(report.preflight.scoped_record_count, 3);
    assert_eq!(report.preflight.unscoped_record_count, 0);
    assert_eq!(report.preflight.scope_keys, vec!["project:nahuali"]);
    assert_eq!(
        report
            .readiness
            .self_inspection_summary
            .source_coverage_count,
        0
    );
    assert!(!report.readiness.write_back_policy.automatic_write_back);
}

#[test]
fn forecasts_source_coverage_review_before_import() {
    let document = MemoryInterchange {
        version: MEMORY_INTERCHANGE_VERSION,
        sources: vec![InterchangeSource {
            ref_id: "conversation-a".to_string(),
            kind: SourceKind::Conversation,
            title: Some("Release review".to_string()),
            uri: None,
            content_checksum: Some("checksum-a".to_string()),
            byte_len: 128,
            metadata: Default::default(),
            scope: None,
            timestamp_ms: None,
        }],
        episodes: vec![InterchangeEpisode {
            ref_id: Some("episode-a".to_string()),
            content: "Lena owns release notes".to_string(),
            tags: Vec::new(),
            mentions: Vec::new(),
            source_role: None,
            source_ref: Some("conversation-a".to_string()),
            source_position: Some(1),
            scope: None,
            timestamp_ms: None,
        }],
        claims: vec![InterchangeClaim {
            subject: "Lena".to_string(),
            predicate: "owns".to_string(),
            object: "release notes".to_string(),
            source_episode_ref: Some("episode-a".to_string()),
            confidence: 0.9,
            scope: None,
            timestamp_ms: None,
        }],
        links: Vec::new(),
        procedures: vec![InterchangeProcedure {
            kind: ProcedureKind::Procedure,
            name: "Release notes".to_string(),
            body: "Keep release notes concise.".to_string(),
            source_episode_ref: None,
            confidence: 0.8,
            scope: None,
            timestamp_ms: None,
        }],
        intentions: Vec::new(),
    };

    let report = validate(&document, true);

    assert!(report.valid);
    assert_eq!(report.preflight.unsourced_episode_count, 0);
    assert_eq!(report.preflight.evidence_gap_count, 1);
    assert_eq!(
        report
            .readiness
            .self_inspection_summary
            .source_coverage_count,
        1
    );
    assert!(report.readiness.review_item_count >= 1);
    assert!(report.readiness.write_back_policy.requires_operator_review);
}

#[test]
fn reports_invalid_documents_without_importing() {
    let document = MemoryInterchange {
        version: 999,
        sources: vec![
            InterchangeSource {
                ref_id: "duplicate-source".to_string(),
                kind: SourceKind::Other,
                title: None,
                uri: None,
                content_checksum: None,
                byte_len: 0,
                metadata: Default::default(),
                scope: None,
                timestamp_ms: None,
            },
            InterchangeSource {
                ref_id: "duplicate-source".to_string(),
                kind: SourceKind::Other,
                title: None,
                uri: None,
                content_checksum: None,
                byte_len: 0,
                metadata: Default::default(),
                scope: None,
                timestamp_ms: None,
            },
        ],
        episodes: vec![
            InterchangeEpisode {
                ref_id: Some("duplicate".to_string()),
                content: String::new(),
                tags: Vec::new(),
                mentions: Vec::new(),
                source_role: None,
                source_ref: Some("missing-source".to_string()),
                source_position: None,
                scope: None,
                timestamp_ms: None,
            },
            InterchangeEpisode {
                ref_id: Some("duplicate".to_string()),
                content: "Valid content".to_string(),
                tags: Vec::new(),
                mentions: Vec::new(),
                source_role: None,
                source_ref: None,
                source_position: None,
                scope: None,
                timestamp_ms: None,
            },
        ],
        claims: vec![InterchangeClaim {
            subject: "Lena".to_string(),
            predicate: "owns".to_string(),
            object: "release notes".to_string(),
            source_episode_ref: Some("missing".to_string()),
            confidence: 0.9,
            scope: None,
            timestamp_ms: None,
        }],
        links: Vec::new(),
        procedures: Vec::new(),
        intentions: Vec::new(),
    };

    let report = validate(&document, false);

    assert!(!report.valid);
    assert_eq!(report.imported_event_count, 0);
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == InterchangeIssueKind::UnsupportedVersion)
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == InterchangeIssueKind::EmptyField)
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == InterchangeIssueKind::DuplicateReference)
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == InterchangeIssueKind::UnknownSourceReference)
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == InterchangeIssueKind::UnknownSourceDocumentReference)
    );
}
