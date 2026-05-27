use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{
    IngestEpisode, IngestSource, MEMORY_INGEST_DOCUMENT_VERSION, MemoryIngestDocument, MemoryScope,
    SourceKind,
};

/// Current text-ingestion adapter version.
pub const TEXT_INGEST_ADAPTER_VERSION: u32 = 1;

/// Default target size for text chunks created by the text adapter.
pub const DEFAULT_TEXT_CHUNK_BYTES: usize = 32 * 1024;

/// Chunking strategy used by the text ingestion adapter.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TextChunking {
    /// Preserve the input as one document, then split only when the chunk limit requires it.
    Document,
    /// Split on blank-line-separated paragraphs.
    Paragraphs,
    /// Split on non-empty lines.
    Lines,
}

/// Options for building a source-neutral ingestion document from text.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct TextIngestOptions {
    /// Source category to attach to the generated ingestion document.
    pub source_kind: SourceKind,
    /// Human-readable title for the source.
    pub title: Option<String>,
    /// Source URI, path, or stable locator.
    pub uri: Option<String>,
    /// Source provenance metadata preserved on the generated source record.
    pub metadata: BTreeMap<String, String>,
    /// Explicit memory context boundary for the generated source document.
    pub scope: Option<MemoryScope>,
    /// Tags copied onto every generated episode.
    pub tags: Vec<String>,
    /// Explicit mentions copied onto every generated episode.
    pub mentions: Vec<String>,
    /// Source-local role or speaker copied onto every generated episode.
    pub source_role: Option<String>,
    /// Chunking strategy for generated episodes.
    pub chunking: TextChunking,
    /// Target maximum bytes for each generated episode chunk.
    pub max_chunk_bytes: usize,
}

impl Default for TextIngestOptions {
    fn default() -> Self {
        Self {
            source_kind: SourceKind::Document,
            title: None,
            uri: None,
            metadata: BTreeMap::new(),
            scope: None,
            tags: Vec::new(),
            mentions: Vec::new(),
            source_role: None,
            chunking: TextChunking::Document,
            max_chunk_bytes: DEFAULT_TEXT_CHUNK_BYTES,
        }
    }
}

/// Report returned by the text ingestion adapter.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct TextIngestBuildReport {
    /// Adapter report version.
    pub version: u32,
    /// Whether a document was generated.
    pub valid: bool,
    /// Number of source bytes supplied to the adapter.
    pub source_byte_len: u64,
    /// Number of episodes generated when the report is valid.
    pub episode_count: usize,
    /// Generated ingestion document, when validation passed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document: Option<MemoryIngestDocument>,
    /// Adapter validation issues.
    pub issues: Vec<TextIngestIssue>,
}

/// Validation issue returned by the text ingestion adapter.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct TextIngestIssue {
    /// Machine-readable issue kind.
    pub kind: TextIngestIssueKind,
    /// Adapter input path for the invalid field.
    pub path: String,
    /// Human-readable issue message.
    pub message: String,
}

/// Machine-readable text adapter issue kind.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TextIngestIssueKind {
    /// Text content was empty after trimming.
    EmptyContent,
    /// Source title and URI were both empty.
    EmptySourceLocator,
    /// Chunk byte limit was zero.
    InvalidChunkLimit,
}

/// Build a source-neutral ingestion document from explicit source text.
///
/// The adapter preserves source material as episodes only. It intentionally does
/// not infer claims, links, procedures, intentions, or entities from text.
pub fn build_text_ingest_document(
    content: &str,
    options: TextIngestOptions,
) -> TextIngestBuildReport {
    let mut issues = Vec::new();
    let source_byte_len = content.len() as u64;
    let title = clean_optional(&options.title);
    let uri = clean_optional(&options.uri);

    if content.trim().is_empty() {
        issues.push(issue(
            TextIngestIssueKind::EmptyContent,
            "content",
            "content cannot be empty",
        ));
    }

    if title.is_none() && uri.is_none() {
        issues.push(issue(
            TextIngestIssueKind::EmptySourceLocator,
            "source",
            "source must include a title or uri",
        ));
    }

    if options.max_chunk_bytes == 0 {
        issues.push(issue(
            TextIngestIssueKind::InvalidChunkLimit,
            "max_chunk_bytes",
            "max_chunk_bytes must be greater than zero",
        ));
    }

    if !issues.is_empty() {
        return TextIngestBuildReport {
            version: TEXT_INGEST_ADAPTER_VERSION,
            valid: false,
            source_byte_len,
            episode_count: 0,
            document: None,
            issues,
        };
    }

    let chunks = chunk_text(content, &options.chunking, options.max_chunk_bytes);
    let tags = clean_strings(&options.tags);
    let mentions = clean_strings(&options.mentions);
    let source_role = clean_optional(&options.source_role);
    let episodes = chunks
        .into_iter()
        .enumerate()
        .map(|(index, chunk)| IngestEpisode {
            ref_id: Some(format!("chunk-{}", index + 1)),
            content: chunk,
            tags: tags.clone(),
            mentions: mentions.clone(),
            source_position: Some((index + 1) as u32),
            source_role: source_role.clone(),
        })
        .collect::<Vec<_>>();
    let episode_count = episodes.len();
    let document = MemoryIngestDocument {
        version: MEMORY_INGEST_DOCUMENT_VERSION,
        source: IngestSource {
            kind: options.source_kind,
            title,
            uri,
            metadata: clean_metadata(&options.metadata),
            scope: options.scope,
        },
        episodes,
        claims: Vec::new(),
        links: Vec::new(),
        procedures: Vec::new(),
        intentions: Vec::new(),
    };

    TextIngestBuildReport {
        version: TEXT_INGEST_ADAPTER_VERSION,
        valid: true,
        source_byte_len,
        episode_count,
        document: Some(document),
        issues,
    }
}

fn chunk_text(content: &str, chunking: &TextChunking, max_chunk_bytes: usize) -> Vec<String> {
    let chunks = match chunking {
        TextChunking::Document => vec![content.trim().to_string()],
        TextChunking::Paragraphs => paragraph_chunks(content),
        TextChunking::Lines => content
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                (!line.is_empty()).then(|| line.to_string())
            })
            .collect(),
    };

    chunks
        .into_iter()
        .flat_map(|chunk| split_by_byte_limit(&chunk, max_chunk_bytes))
        .collect()
}

fn paragraph_chunks(content: &str) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = Vec::new();

    for line in content.lines() {
        if line.trim().is_empty() {
            if !current.is_empty() {
                chunks.push(current.join("\n"));
                current.clear();
            }
            continue;
        }
        current.push(line.trim().to_string());
    }

    if !current.is_empty() {
        chunks.push(current.join("\n"));
    }

    chunks
}

fn split_by_byte_limit(content: &str, max_chunk_bytes: usize) -> Vec<String> {
    if content.len() <= max_chunk_bytes {
        return vec![content.to_string()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();

    for character in content.chars() {
        if !current.is_empty() && current.len() + character.len_utf8() > max_chunk_bytes {
            chunks.push(current.trim().to_string());
            current.clear();
        }
        current.push(character);
    }

    if !current.trim().is_empty() {
        chunks.push(current.trim().to_string());
    }

    chunks
}

fn clean_optional(value: &Option<String>) -> Option<String> {
    value.as_ref().and_then(|value| {
        let value = value.trim().to_string();
        if value.is_empty() { None } else { Some(value) }
    })
}

fn clean_strings(values: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .iter()
        .filter_map(|value| {
            let value = value.trim().to_string();
            if value.is_empty() || !seen.insert(value.clone()) {
                None
            } else {
                Some(value)
            }
        })
        .collect()
}

fn clean_metadata(metadata: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    metadata
        .iter()
        .filter_map(|(key, value)| {
            let key = key.trim().to_string();
            let value = value.trim().to_string();
            if key.is_empty() || value.is_empty() {
                None
            } else {
                Some((key, value))
            }
        })
        .collect()
}

fn issue(
    kind: TextIngestIssueKind,
    path: impl Into<String>,
    message: impl Into<String>,
) -> TextIngestIssue {
    TextIngestIssue {
        kind,
        path: path.into(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::{TextChunking, TextIngestIssueKind, TextIngestOptions, build_text_ingest_document};

    #[test]
    fn text_adapter_splits_paragraphs_and_preserves_source_metadata() {
        let mut options = TextIngestOptions {
            title: Some("  Field notes  ".to_string()),
            uri: Some(" file:///tmp/notes.md ".to_string()),
            chunking: TextChunking::Paragraphs,
            tags: vec![" notes ".to_string(), "notes".to_string(), "".to_string()],
            mentions: vec!["Lena".to_string()],
            source_role: Some(" author ".to_string()),
            ..TextIngestOptions::default()
        };
        options
            .metadata
            .insert(" source ".to_string(), " local ".to_string());

        let report = build_text_ingest_document("Alpha\n\nBeta\nGamma\n", options);

        assert!(report.valid);
        assert_eq!(report.episode_count, 2);
        let document = report.document.expect("document is generated");
        assert_eq!(document.source.title.as_deref(), Some("Field notes"));
        assert_eq!(document.source.uri.as_deref(), Some("file:///tmp/notes.md"));
        assert_eq!(document.source.metadata["source"], "local");
        assert_eq!(document.episodes[0].ref_id.as_deref(), Some("chunk-1"));
        assert_eq!(document.episodes[0].content, "Alpha");
        assert_eq!(document.episodes[1].content, "Beta\nGamma");
        assert_eq!(document.episodes[0].tags, vec!["notes"]);
        assert_eq!(document.episodes[0].mentions, vec!["Lena"]);
        assert_eq!(document.episodes[0].source_role.as_deref(), Some("author"));
        assert!(document.claims.is_empty());
        assert!(document.links.is_empty());
        assert!(document.procedures.is_empty());
        assert!(document.intentions.is_empty());
    }

    #[test]
    fn text_adapter_splits_large_chunks_without_breaking_utf8() {
        let report = build_text_ingest_document(
            "abécd",
            TextIngestOptions {
                title: Some("Unicode".to_string()),
                max_chunk_bytes: 3,
                ..TextIngestOptions::default()
            },
        );

        assert!(report.valid);
        let document = report.document.expect("document is generated");
        assert_eq!(document.episodes.len(), 3);
        assert_eq!(document.episodes[0].content, "ab");
        assert_eq!(document.episodes[1].content, "éc");
        assert_eq!(document.episodes[2].content, "d");
    }

    #[test]
    fn text_adapter_reports_invalid_boundary_inputs() {
        let report = build_text_ingest_document(
            "   ",
            TextIngestOptions {
                max_chunk_bytes: 0,
                ..TextIngestOptions::default()
            },
        );

        assert!(!report.valid);
        assert!(report.document.is_none());
        assert_eq!(report.episode_count, 0);
        assert_eq!(report.issues.len(), 3);
        assert_eq!(report.issues[0].kind, TextIngestIssueKind::EmptyContent);
        assert_eq!(
            report.issues[1].kind,
            TextIngestIssueKind::EmptySourceLocator
        );
        assert_eq!(
            report.issues[2].kind,
            TextIngestIssueKind::InvalidChunkLimit
        );
    }
}
