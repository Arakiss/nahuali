pub(crate) fn rebuild_index(
    data: &MemoryData,
    config: &SemanticConfig,
) -> Result<SemanticIndexReport> {
    let embedder = config.embedder()?;
    let embedding = EmbeddingProviderConfig {
        dimensions: embedder.dimensions(),
        ..config.embedding.clone()
    };
    let client = QdrantRestClient::new(config);
    let deleted_existing_collection =
        client.delete_collection_if_exists(&config.collection_name)?;
    client.create_collection(&config.collection_name, &embedding)?;

    let documents = semantic_documents(data);
    let points = documents
        .iter()
        .map(|document| {
            let vector = embedder.embed(&document.text);
            QdrantPoint {
                id: document.point_id,
                vector,
                payload: document.payload.clone(),
            }
        })
        .collect::<Vec<_>>();
    if !points.is_empty() {
        client.upsert_points(&config.collection_name, &points)?;
    }

    Ok(SemanticIndexReport {
        collection_name: config.collection_name.clone(),
        qdrant_url: config.qdrant_url.clone(),
        embedding,
        source_event_count: data.event_count,
        indexed_point_count: points.len(),
        deleted_existing_collection,
        points: documents
            .into_iter()
            .map(|document| SemanticPointSummary {
                point_id: document.point_id,
                kind: document.payload.kind(),
                id: document.payload.id,
                event_id: document.payload.event_id,
                event_ids: document.payload.event_ids,
                surreal_table: document.payload.surreal_table,
                surreal_id: document.payload.surreal_id,
                scope_key: document.payload.scope_key,
                entity_names: document.payload.entity_names,
                source_ids: document.payload.source_ids,
                text: document.text,
            })
            .collect(),
    })
}

/// Synchronize the Qdrant semantic index with the current projection *without*
/// dropping the collection.
///
/// [`rebuild_index`] deletes and recreates the collection, which leaves a window
/// with no index and means recall silently degrades after every write until a
/// caller runs a full rebuild. `sync_index` instead ensures the collection
/// exists and upserts the current points by their deterministic IDs: it is
/// idempotent and safe to run after each batch of writes to keep recall current
/// with no gap. Because it never drops the collection it cannot change the
/// vector space — after switching the embedder (which changes the dimensions),
/// run [`rebuild_index`] instead.
pub(crate) fn sync_index(data: &MemoryData, config: &SemanticConfig) -> Result<SemanticIndexReport> {
    let embedder = config.embedder()?;
    let embedding = EmbeddingProviderConfig {
        dimensions: embedder.dimensions(),
        ..config.embedding.clone()
    };
    let client = QdrantRestClient::new(config);
    if !client.collection_exists(&config.collection_name)? {
        client.create_collection(&config.collection_name, &embedding)?;
    }

    let documents = semantic_documents(data);
    let points = documents
        .iter()
        .map(|document| {
            let vector = embedder.embed(&document.text);
            QdrantPoint {
                id: document.point_id,
                vector,
                payload: document.payload.clone(),
            }
        })
        .collect::<Vec<_>>();
    if !points.is_empty() {
        client.upsert_points(&config.collection_name, &points)?;
    }

    Ok(SemanticIndexReport {
        collection_name: config.collection_name.clone(),
        qdrant_url: config.qdrant_url.clone(),
        embedding,
        source_event_count: data.event_count,
        indexed_point_count: points.len(),
        deleted_existing_collection: false,
        points: documents
            .into_iter()
            .map(|document| SemanticPointSummary {
                point_id: document.point_id,
                kind: document.payload.kind(),
                id: document.payload.id,
                event_id: document.payload.event_id,
                event_ids: document.payload.event_ids,
                surreal_table: document.payload.surreal_table,
                surreal_id: document.payload.surreal_id,
                scope_key: document.payload.scope_key,
                entity_names: document.payload.entity_names,
                source_ids: document.payload.source_ids,
                text: document.text,
            })
            .collect(),
    })
}

pub(crate) fn index_status(config: &SemanticConfig) -> Result<SemanticIndexStatus> {
    let client = QdrantRestClient::new(config);
    let collection_exists = client.collection_exists(&config.collection_name)?;
    let point_count = if collection_exists {
        client.count_points(&config.collection_name)?
    } else {
        0
    };

    Ok(SemanticIndexStatus {
        collection_name: config.collection_name.clone(),
        qdrant_url: config.qdrant_url.clone(),
        collection_exists,
        point_count,
    })
}

pub(crate) fn hybrid_recall(
    data: &MemoryData,
    query: &str,
    limit: usize,
    authority: AuthorityDecision,
    config: &SemanticConfig,
) -> Result<HybridRecallReport> {
    hybrid_recall_with_options(data, query, limit, RecallOptions::default(), authority, config)
}

pub(crate) fn hybrid_recall_with_options(
    data: &MemoryData,
    query: &str,
    limit: usize,
    options: RecallOptions,
    authority: AuthorityDecision,
    config: &SemanticConfig,
) -> Result<HybridRecallReport> {
    let embedder = config.embedder()?;
    let embedding = EmbeddingProviderConfig {
        dimensions: embedder.dimensions(),
        ..config.embedding.clone()
    };
    let client = QdrantRestClient::new(config);
    let bounded_limit = limit.max(1);
    let candidate_limit = bounded_limit.saturating_mul(3).max(bounded_limit);
    let lexical_results = recall::recall_with_options(
        data,
        query,
        RecallOptions {
            limit: candidate_limit,
            ..options.clone()
        },
    );
    let query_vector = embedder.embed(query);
    let semantic_results = client.query_points(
        &config.collection_name,
        &query_vector,
        candidate_limit,
        Some(SemanticQueryFilter::from_options(&options)),
    )?;
    let mut merged = BTreeMap::<String, HybridAccumulator>::new();

    for result in &lexical_results {
        let key = result_key(&result.kind, &result.id);
        merged
            .entry(key)
            .or_insert_with(|| HybridAccumulator::from_lexical(result))
            .merge_lexical(result);
    }

    for result in &semantic_results {
        let key = result_key(&result.kind, &result.id);
        merged
            .entry(key)
            .or_insert_with(|| HybridAccumulator::from_semantic(result))
            .merge_semantic(result);
    }

    let mut results = merged
        .into_values()
        .map(|accumulator| accumulator.finish(&authority))
        .collect::<Vec<_>>();
    results.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.id.cmp(&right.id))
    });
    results.truncate(bounded_limit);

    Ok(HybridRecallReport {
        query: query.to_string(),
        limit: bounded_limit,
        collection_name: config.collection_name.clone(),
        embedding,
        authority,
        lexical_results,
        semantic_results,
        results,
    })
}

/// Converts memory-item and query text into a dense vector for semantic recall.
///
/// This is the single seam behind which embedding strategies vary. The default
/// build ships only [`DeterministicEmbedder`]; optional adapters (e.g. a local
/// static model) implement the same contract behind a feature flag.
pub(crate) trait Embedder {
    /// Embed `text` into a vector of [`Embedder::dimensions`] length.
    fn embed(&self, text: &str) -> Vec<f32>;
    /// Vector dimensionality produced by this embedder.
    fn dimensions(&self) -> usize;
}

struct DeterministicEmbedder {
    dimensions: usize,
}

/// Hash `key` into one signed bucket of `vector`, the shared accumulation step
/// for the deterministic embedder's tokens and character n-grams.
fn deterministic_bump(vector: &mut [f32], key: &str) {
    let hash = fnv1a64(key.as_bytes());
    let index = (hash as usize) % vector.len();
    vector[index] += if (hash >> 63) == 0 { 1.0 } else { -1.0 };
}

impl Embedder for DeterministicEmbedder {
    fn embed(&self, text: &str) -> Vec<f32> {
        let mut vector = vec![0.0; self.dimensions];
        for token in tokenize(text) {
            // The whole token preserves exact-match strength: identical words
            // still land in the same bucket.
            deterministic_bump(&mut vector, &token);
            // Character trigrams over a boundary-padded token (#token#) capture
            // shared word shapes — morphology, common roots, substrings, and
            // typos — so "release" and "releasing", or "product" and "products",
            // overlap instead of being orthogonal the way whole-token hashing
            // alone left them. This does not bridge true synonyms with no shared
            // characters (car / automobile); the optional local model does that.
            let padded = format!("#{token}#");
            let chars = padded.chars().collect::<Vec<_>>();
            for window in chars.windows(3) {
                let gram = window.iter().collect::<String>();
                deterministic_bump(&mut vector, &gram);
            }
        }

        let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
        if norm == 0.0 {
            vector[0] = 1.0;
            return vector;
        }
        for value in &mut vector {
            *value /= norm;
        }
        vector
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }
}

/// Local static-embedding adapter backed by a model2vec model loaded from disk.
///
/// Available only under `--features local-embeddings`. The model is a frozen
/// token→vector lookup table plus mean pooling: deterministic, CPU-only, and
/// fully offline (bytes are read from disk, never fetched).
#[cfg(feature = "local-embeddings")]
struct LocalModelEmbedder {
    model: model2vec_rs::model::StaticModel,
    dimensions: usize,
}

#[cfg(feature = "local-embeddings")]
impl LocalModelEmbedder {
    /// Load the model from a directory holding `tokenizer.json`,
    /// `model.safetensors` and `config.json`.
    fn load(path: &str) -> Result<Self> {
        let dir = std::path::Path::new(path);
        let read = |file: &str| -> Result<Vec<u8>> {
            std::fs::read(dir.join(file)).map_err(|source| NahualiError::LocalEmbeddingModel {
                message: format!("failed to read {file} under {path}: {source}"),
            })
        };
        let tokenizer = read("tokenizer.json")?;
        let model_bytes = read("model.safetensors")?;
        let config = read("config.json")?;
        let model =
            model2vec_rs::model::StaticModel::from_bytes(tokenizer, model_bytes, config, Some(true))
                .map_err(|source| NahualiError::LocalEmbeddingModel {
                    message: format!("failed to load local embedding model from {path}: {source}"),
                })?;
        let dimensions = model.encode_single("nahuali").len();
        if dimensions == 0 {
            return Err(NahualiError::LocalEmbeddingModel {
                message: format!("local embedding model at {path} produced an empty vector"),
            });
        }
        Ok(Self { model, dimensions })
    }
}

#[cfg(feature = "local-embeddings")]
impl Embedder for LocalModelEmbedder {
    fn embed(&self, text: &str) -> Vec<f32> {
        self.model.encode_single(text)
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }
}

#[derive(Clone, Debug)]
struct SemanticDocument {
    point_id: u64,
    text: String,
    payload: SemanticPayload,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct SemanticPayload {
    schema_version: u32,
    projection_version: u32,
    kind: String,
    id: String,
    event_id: String,
    event_ids: Vec<String>,
    surreal_table: String,
    surreal_id: String,
    text: String,
    excerpt: String,
    evidence_id: Option<String>,
    has_evidence: bool,
    created_at_ms: u64,
    confidence: Option<f32>,
    health_score: Option<f32>,
    scope_key: Option<String>,
    scope_kind: Option<String>,
    scope_name: Option<String>,
    entity_names: Vec<String>,
    source_ids: Vec<String>,
}

impl SemanticPayload {
    fn kind(&self) -> MemoryKind {
        memory_kind_from_name(&self.kind).expect("semantic payload kind is created by nahuali")
    }
}

fn semantic_documents(data: &MemoryData) -> Vec<SemanticDocument> {
    let mut documents = Vec::new();

    for entity in &data.entities {
        let text = entity.name.clone();
        documents.push(document(DocumentInput {
            kind: MemoryKind::Entity,
            id: &entity.id,
            event_id: entity
                .source_event_ids
                .first()
                .map(String::as_str)
                .unwrap_or(""),
            text: text.clone(),
            excerpt: text,
            evidence_id: entity.source_event_ids.first().cloned(),
            created_at_ms: entity.last_seen_at_ms,
            confidence: None,
            scope: entity.scope.as_ref(),
            entity_names: vec![entity.name.clone()],
            source_ids: entity.source_event_ids.clone(),
            health_score: None,
        }));
    }

    for episode in &data.episodes {
        let text = format!("{} {}", episode.content, episode.tags.join(" "));
        documents.push(document(DocumentInput {
            kind: MemoryKind::Episode,
            id: &episode.id,
            event_id: &episode.event_id,
            text,
            excerpt: episode.content.clone(),
            evidence_id: Some(episode.id.clone()),
            created_at_ms: episode.created_at_ms,
            confidence: None,
            scope: episode.scope.as_ref(),
            entity_names: episode.mentions.clone(),
            source_ids: episode
                .source_id
                .iter()
                .cloned()
                .chain(std::iter::once(episode.id.clone()))
                .collect(),
            health_score: None,
        }));
    }

    for claim in &data.claims {
        let text = format!("{} {} {}", claim.subject, claim.predicate, claim.object);
        documents.push(document(DocumentInput {
            kind: MemoryKind::Claim,
            id: &claim.id,
            event_id: &claim.event_id,
            text: text.clone(),
            excerpt: text,
            evidence_id: claim.source_episode_id.clone(),
            created_at_ms: claim.created_at_ms,
            confidence: Some(claim.confidence),
            scope: claim.scope.as_ref(),
            entity_names: vec![claim.subject.clone(), claim.object.clone()],
            source_ids: claim.source_episode_id.iter().cloned().collect(),
            health_score: Some(claim.confidence),
        }));
    }

    for link in &data.links {
        let text = format!("{} {} {}", link.from, link.relation, link.to);
        documents.push(document(DocumentInput {
            kind: MemoryKind::Link,
            id: &link.id,
            event_id: &link.event_id,
            text: text.clone(),
            excerpt: text,
            evidence_id: link.source_episode_id.clone(),
            created_at_ms: link.created_at_ms,
            confidence: Some(link.confidence),
            scope: link.scope.as_ref(),
            entity_names: vec![link.from.clone(), link.to.clone()],
            source_ids: link.source_episode_id.iter().cloned().collect(),
            health_score: Some(link.confidence),
        }));
    }

    for procedure in &data.procedures {
        let text = format!("{:?} {} {}", procedure.kind, procedure.name, procedure.body);
        documents.push(document(DocumentInput {
            kind: MemoryKind::Procedure,
            id: &procedure.id,
            event_id: &procedure.event_id,
            text,
            excerpt: format!("{}: {}", procedure.name, procedure.body),
            evidence_id: procedure.source_episode_id.clone(),
            created_at_ms: procedure.created_at_ms,
            confidence: Some(procedure.confidence),
            scope: procedure.scope.as_ref(),
            entity_names: Vec::new(),
            source_ids: procedure.source_episode_id.iter().cloned().collect(),
            health_score: Some(procedure.confidence),
        }));
    }

    for intention in &data.intentions {
        let text = format!(
            "{:?} {:?} {:?} {}",
            intention.kind, intention.status, intention.priority, intention.description
        );
        documents.push(document(DocumentInput {
            kind: MemoryKind::Intention,
            id: &intention.id,
            event_id: &intention.event_id,
            text,
            excerpt: intention.description.clone(),
            evidence_id: intention.source_episode_id.clone(),
            created_at_ms: intention.created_at_ms,
            confidence: None,
            scope: intention.scope.as_ref(),
            entity_names: Vec::new(),
            source_ids: intention.source_episode_id.iter().cloned().collect(),
            health_score: None,
        }));
    }

    documents
}

struct DocumentInput<'a> {
    kind: MemoryKind,
    id: &'a str,
    event_id: &'a str,
    text: String,
    excerpt: String,
    evidence_id: Option<String>,
    created_at_ms: u64,
    confidence: Option<f32>,
    scope: Option<&'a MemoryScope>,
    entity_names: Vec<String>,
    source_ids: Vec<String>,
    health_score: Option<f32>,
}

fn document(input: DocumentInput<'_>) -> SemanticDocument {
    let kind_name = memory_kind_name(&input.kind).to_string();
    let point_id = point_id_for(&kind_name, input.id);
    let event_ids = if input.event_id.is_empty() {
        Vec::new()
    } else {
        vec![input.event_id.to_string()]
    };
    let scope_key = input.scope.map(|scope| scope.key.clone());
    let scope_kind = input.scope.map(|scope| scope.kind.as_key().to_string());
    let scope_name = input.scope.map(|scope| scope.name.clone());
    SemanticDocument {
        point_id,
        text: input.text.clone(),
        payload: SemanticPayload {
            schema_version: SEMANTIC_INDEX_SCHEMA_VERSION,
            projection_version: MEMORY_DATA_VERSION,
            kind: kind_name,
            id: input.id.to_string(),
            event_id: input.event_id.to_string(),
            event_ids,
            surreal_table: surreal_table_for(&input.kind).to_string(),
            surreal_id: input.id.to_string(),
            text: input.text,
            excerpt: input.excerpt,
            has_evidence: input.evidence_id.is_some(),
            evidence_id: input.evidence_id,
            created_at_ms: input.created_at_ms,
            confidence: input.confidence,
            health_score: input.health_score,
            scope_key,
            scope_kind,
            scope_name,
            entity_names: input.entity_names,
            source_ids: input.source_ids,
        },
    }
}

#[derive(Clone, Debug, Default)]
struct SemanticQueryFilter {
    scope_key: Option<String>,
    kinds: Vec<String>,
    require_evidence: bool,
    as_of_ms: Option<u64>,
    since_ms: Option<u64>,
}

impl SemanticQueryFilter {
    fn from_options(options: &RecallOptions) -> Self {
        Self {
            scope_key: options.scope.as_ref().map(|scope| scope.key.clone()),
            kinds: options
                .kinds
                .iter()
                .map(memory_kind_name)
                .map(str::to_string)
                .collect(),
            require_evidence: options.require_evidence,
            as_of_ms: options.as_of_ms,
            since_ms: options.since_ms,
        }
    }

    fn to_qdrant_filter(&self) -> Option<serde_json::Value> {
        let mut must = Vec::new();
        if let Some(scope_key) = &self.scope_key {
            must.push(serde_json::json!({
                "key": "scope_key",
                "match": { "value": scope_key }
            }));
        }
        if !self.kinds.is_empty() {
            must.push(serde_json::json!({
                "key": "kind",
                "match": { "any": self.kinds }
            }));
        }
        if self.require_evidence {
            must.push(serde_json::json!({
                "key": "has_evidence",
                "match": { "value": true }
            }));
        }
        if self.as_of_ms.is_some() || self.since_ms.is_some() {
            let mut range = serde_json::Map::new();
            if let Some(as_of_ms) = self.as_of_ms {
                range.insert("lte".to_string(), serde_json::json!(as_of_ms));
            }
            if let Some(since_ms) = self.since_ms {
                range.insert("gte".to_string(), serde_json::json!(since_ms));
            }
            must.push(serde_json::json!({
                "key": "created_at_ms",
                "range": serde_json::Value::Object(range)
            }));
        }

        if must.is_empty() {
            None
        } else {
            Some(serde_json::json!({ "must": must }))
        }
    }
}
