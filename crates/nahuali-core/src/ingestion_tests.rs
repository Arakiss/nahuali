use super::{
    IngestClaim, IngestEpisode, IngestSource, IngestionIssueKind, MEMORY_INGEST_DOCUMENT_VERSION,
    MemoryIngestDocument, validate,
};
use crate::SourceKind;

#[test]
fn validates_supported_document_before_ingestion() {
    let document = MemoryIngestDocument {
        version: MEMORY_INGEST_DOCUMENT_VERSION,
        source: IngestSource {
            kind: SourceKind::Conversation,
            title: Some("Release review".to_string()),
            uri: None,
            metadata: Default::default(),
            scope: None,
        },
        episodes: vec![IngestEpisode {
            ref_id: Some("message-1".to_string()),
            content: "Lena owns release notes".to_string(),
            tags: Vec::new(),
            mentions: vec!["Lena".to_string()],
            source_position: Some(1),
            source_role: Some("user".to_string()),
        }],
        claims: vec![
            IngestClaim {
                subject: "Lena".to_string(),
                predicate: "owns".to_string(),
                object: "release notes".to_string(),
                source_episode_ref: Some("message-1".to_string()),
                confidence: 0.9,
            },
            IngestClaim {
                subject: "Release notes".to_string(),
                predicate: "style".to_string(),
                object: "concise".to_string(),
                source_episode_ref: None,
                confidence: 0.7,
            },
        ],
        links: Vec::new(),
        procedures: Vec::new(),
        intentions: Vec::new(),
    };

    let report = validate(&document, true);

    assert!(report.valid);
    assert!(report.dry_run);
    assert_eq!(report.appendable_event_count, 4);
    assert_eq!(report.preflight.derived_record_count, 2);
    assert_eq!(report.preflight.evidence_linked_record_count, 1);
    assert_eq!(report.preflight.evidence_gap_count, 1);
    assert_eq!(report.preflight.referenced_episode_count, 1);
    assert_eq!(report.preflight.unreferenced_episode_count, 0);
}

#[test]
fn reports_invalid_documents_without_ingesting() {
    let document = MemoryIngestDocument {
        version: 999,
        source: IngestSource {
            kind: SourceKind::Document,
            title: None,
            uri: None,
            metadata: Default::default(),
            scope: None,
        },
        episodes: Vec::new(),
        claims: Vec::new(),
        links: Vec::new(),
        procedures: Vec::new(),
        intentions: Vec::new(),
    };

    let report = validate(&document, true);

    assert!(!report.valid);
    assert_eq!(report.issues.len(), 3);
    assert_eq!(
        report.issues[0].kind,
        IngestionIssueKind::UnsupportedVersion
    );
    assert_eq!(
        report.issues[1].kind,
        IngestionIssueKind::EmptySourceLocator
    );
    assert_eq!(report.issues[2].kind, IngestionIssueKind::NoEpisodes);
}
