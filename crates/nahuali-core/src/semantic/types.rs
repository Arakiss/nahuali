/// Semantic index schema version stored in Qdrant payloads.
pub const SEMANTIC_INDEX_SCHEMA_VERSION: u32 = 4;
/// Default local Qdrant REST endpoint for the OSS Docker stack.
pub const DEFAULT_QDRANT_URL: &str = "http://localhost:16333";
/// Default collection used for memory-item vectors.
pub const DEFAULT_SEMANTIC_COLLECTION: &str = "nahuali_memory_items";
/// Default dimensionality for the deterministic local embedding provider.
pub const DEFAULT_EMBEDDING_DIMENSIONS: usize = 64;

// Bumped from `-token-v1` when the deterministic embedder moved from whole-token
// hashing to character n-grams: that changed the vector space, so the recorded
// embedding identity must change too, or an index built by the old embedder
// would be queried by the new one under the same name and silently mismatch.
const LOCAL_EMBEDDING_MODEL: &str = "nahuali/deterministic-ngram-v1";

/// Configuration for Qdrant-backed semantic indexing.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SemanticConfig {
    /// Qdrant REST endpoint.
    pub qdrant_url: String,
    /// Qdrant collection name for projected memory items.
    pub collection_name: String,
    /// Embedding provider configuration.
    pub embedding: EmbeddingProviderConfig,
    /// Filesystem path to a local static embedding model directory, used only
    /// when the provider is the optional local model. Ignored otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding_model_path: Option<String>,
}

impl SemanticConfig {
    /// Build a local deterministic semantic configuration from environment.
    ///
    /// Supported variables:
    ///
    /// - `NAHUALI_QDRANT_URL`
    /// - `NAHUALI_QDRANT_COLLECTION`
    /// - `NAHUALI_EMBEDDING_PROVIDER`
    /// - `NAHUALI_EMBEDDING_DIMENSIONS`
    /// - `NAHUALI_QDRANT_API_KEY`
    pub fn from_env() -> Result<Self> {
        let qdrant_url =
            non_empty_env("NAHUALI_QDRANT_URL").unwrap_or_else(|| DEFAULT_QDRANT_URL.to_string());
        let collection_name = non_empty_env("NAHUALI_QDRANT_COLLECTION")
            .unwrap_or_else(|| DEFAULT_SEMANTIC_COLLECTION.to_string());
        let dimensions = match non_empty_env("NAHUALI_EMBEDDING_DIMENSIONS") {
            Some(raw) => raw
                .parse::<usize>()
                .map_err(|_| NahualiError::InvalidSemanticConfig {
                    message: format!(
                        "NAHUALI_EMBEDDING_DIMENSIONS must be a positive integer, got {raw}"
                    ),
                })?,
            None => DEFAULT_EMBEDDING_DIMENSIONS,
        };
        if dimensions == 0 {
            return Err(NahualiError::InvalidSemanticConfig {
                message: "NAHUALI_EMBEDDING_DIMENSIONS must be greater than zero".to_string(),
            });
        }

        let provider = non_empty_env("NAHUALI_EMBEDDING_PROVIDER")
            .unwrap_or_else(|| "deterministic".to_string());
        let embedding = EmbeddingProviderConfig::from_provider_name(provider, dimensions);
        let embedding_model_path = if embedding.kind == EmbeddingProviderKind::LocalModel {
            non_empty_env("NAHUALI_LOCAL_EMBEDDING_MODEL_PATH")
        } else {
            None
        };

        Ok(Self {
            qdrant_url: normalize_qdrant_url(&qdrant_url),
            collection_name: normalize_collection_name(&collection_name)?,
            embedding,
            embedding_model_path,
        })
    }

    /// Return the default local deterministic configuration.
    pub fn default_local() -> Self {
        Self {
            qdrant_url: DEFAULT_QDRANT_URL.to_string(),
            collection_name: DEFAULT_SEMANTIC_COLLECTION.to_string(),
            embedding: EmbeddingProviderConfig::deterministic(DEFAULT_EMBEDDING_DIMENSIONS),
            embedding_model_path: None,
        }
    }

    /// Return the default local configuration with a caller-provided collection.
    pub fn local_with_collection(collection_name: impl Into<String>) -> Result<Self> {
        Ok(Self {
            collection_name: normalize_collection_name(&collection_name.into())?,
            ..Self::default_local()
        })
    }

    /// Return this configuration with its collection scoped to a database name.
    pub fn scoped_to_database(&self, database_name: impl AsRef<str>) -> Result<Self> {
        let database_suffix = normalize_database_suffix(database_name.as_ref());
        Ok(Self {
            collection_name: normalize_collection_name(&format!(
                "{}__{}",
                self.collection_name, database_suffix
            ))?,
            ..self.clone()
        })
    }

    fn embedder(&self) -> Result<Box<dyn Embedder>> {
        match self.embedding.kind {
            EmbeddingProviderKind::DeterministicLocal => Ok(Box::new(DeterministicEmbedder {
                dimensions: self.embedding.dimensions,
            })),
            EmbeddingProviderKind::LocalModel => self.local_model_embedder(),
            EmbeddingProviderKind::Hosted => Err(NahualiError::InvalidSemanticConfig {
                message: format!(
                    "embedding provider '{}' is configured but only the deterministic local provider is built into nahuali-core",
                    self.embedding.model
                ),
            }),
        }
    }

    #[cfg(feature = "local-embeddings")]
    fn local_model_embedder(&self) -> Result<Box<dyn Embedder>> {
        let path = self.embedding_model_path.as_deref().ok_or_else(|| {
            NahualiError::InvalidSemanticConfig {
                message: "local-model embedding provider requires NAHUALI_LOCAL_EMBEDDING_MODEL_PATH \
                          to point at a directory with tokenizer.json, model.safetensors and config.json"
                    .to_string(),
            }
        })?;
        Ok(Box::new(LocalModelEmbedder::load(path)?))
    }

    #[cfg(not(feature = "local-embeddings"))]
    fn local_model_embedder(&self) -> Result<Box<dyn Embedder>> {
        Err(NahualiError::InvalidSemanticConfig {
            message: "local-model embedding provider requires building nahuali-core with \
                      --features local-embeddings"
                .to_string(),
        })
    }
}

impl Default for SemanticConfig {
    fn default() -> Self {
        Self::default_local()
    }
}

/// Embedding provider configuration used by the semantic tier.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct EmbeddingProviderConfig {
    /// Provider kind.
    pub kind: EmbeddingProviderKind,
    /// Provider or model identifier.
    pub model: String,
    /// Vector dimensionality.
    pub dimensions: usize,
}

impl EmbeddingProviderConfig {
    /// Return the built-in deterministic local provider.
    pub fn deterministic(dimensions: usize) -> Self {
        Self {
            kind: EmbeddingProviderKind::DeterministicLocal,
            model: LOCAL_EMBEDDING_MODEL.to_string(),
            dimensions,
        }
    }

    fn from_provider_name(provider: String, dimensions: usize) -> Self {
        let normalized = provider.trim().to_ascii_lowercase();
        if normalized == "deterministic"
            || normalized == "local"
            || normalized == LOCAL_EMBEDDING_MODEL
        {
            Self::deterministic(dimensions)
        } else if normalized == "local-model" || normalized == "model2vec" {
            Self {
                kind: EmbeddingProviderKind::LocalModel,
                model: normalized,
                dimensions,
            }
        } else {
            Self {
                kind: EmbeddingProviderKind::Hosted,
                model: provider,
                dimensions,
            }
        }
    }
}

/// Embedding provider category.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingProviderKind {
    /// Built-in deterministic local token-hash embedding.
    DeterministicLocal,
    /// Optional local static embedding model (model2vec) loaded from disk.
    LocalModel,
    /// Explicit hosted or external provider configured above the OSS core.
    Hosted,
}

/// Result of rebuilding a Qdrant semantic index from the current projection.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct SemanticIndexReport {
    /// Qdrant collection name.
    pub collection_name: String,
    /// Qdrant REST endpoint.
    pub qdrant_url: String,
    /// Embedding provider used for vectors.
    pub embedding: EmbeddingProviderConfig,
    /// Number of source events represented by the projection.
    pub source_event_count: usize,
    /// Number of memory-item points written to Qdrant.
    pub indexed_point_count: usize,
    /// Whether an existing collection was deleted before rebuild.
    pub deleted_existing_collection: bool,
    /// Indexed point summaries.
    pub points: Vec<SemanticPointSummary>,
}

/// Current Qdrant semantic index status.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SemanticIndexStatus {
    /// Qdrant collection name.
    pub collection_name: String,
    /// Qdrant REST endpoint.
    pub qdrant_url: String,
    /// Whether the collection exists.
    pub collection_exists: bool,
    /// Exact point count when the collection exists.
    pub point_count: usize,
    /// Number of points the current deterministic projection requires.
    pub expected_point_count: usize,
    /// Exact number of points currently stored in Qdrant.
    pub indexed_point_count: usize,
    /// Number of projected points absent from Qdrant.
    pub missing_point_count: usize,
    /// Number of Qdrant points that no longer belong to the projection.
    pub orphan_point_count: usize,
    /// Number of points whose payload or embedding identity is outdated.
    pub stale_point_count: usize,
    /// Whether the collection exactly matches the current projection and embedding identity.
    pub is_current: bool,
    /// Bounded examples of missing point identifiers.
    pub missing_point_ids: Vec<String>,
    /// Bounded examples of orphan point identifiers.
    pub orphan_point_ids: Vec<String>,
    /// Bounded examples of stale point identifiers.
    pub stale_point_ids: Vec<String>,
    /// Whether any drift identifier list was truncated.
    pub drift_details_truncated: bool,
}

/// Compact metadata for one indexed semantic point.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct SemanticPointSummary {
    /// Deterministic Qdrant point identifier.
    pub point_id: u64,
    /// Memory item kind.
    pub kind: MemoryKind,
    /// Memory item identifier.
    pub id: String,
    /// Source event identifier.
    pub event_id: String,
    /// Event identifiers represented by this point.
    pub event_ids: Vec<String>,
    /// SurrealDB graph projection table that owns this point.
    pub surreal_table: String,
    /// SurrealDB graph projection identifier for this point.
    pub surreal_id: String,
    /// Explicit memory scope key, when present.
    pub scope_key: Option<String>,
    /// Entity names attached to this point for filtering/debugging.
    pub entity_names: Vec<String>,
    /// Source records or evidence episodes attached to this point.
    pub source_ids: Vec<String>,
    /// Text embedded into Qdrant.
    pub text: String,
}

/// Semantic match returned by Qdrant.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct SemanticMatch {
    /// Memory item kind.
    pub kind: MemoryKind,
    /// Memory item identifier.
    pub id: String,
    /// Source event identifier.
    pub event_id: String,
    /// Qdrant cosine similarity score.
    pub score: f32,
    /// Human-readable excerpt.
    pub excerpt: String,
    /// Optional evidence episode identifier.
    pub evidence_id: Option<String>,
    /// Explicit memory scope key, when present.
    pub scope_key: Option<String>,
}

/// Explainable hybrid recall result.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct HybridRecallResult {
    /// Memory item kind.
    pub kind: MemoryKind,
    /// Memory item identifier.
    pub id: String,
    /// Combined score used for ordering.
    pub score: f32,
    /// Lexical score from deterministic local recall, when present.
    pub lexical_score: Option<f32>,
    /// Semantic score from Qdrant, when present.
    pub semantic_score: Option<f32>,
    /// Projection-level authority score at query time.
    ///
    /// Retained for compatibility; prefer the per-result [`Self::trust`] verdict,
    /// which is computed by the same policy as lexical recall.
    pub authority_score: f32,
    /// Per-result trust verdict, computed by the same policy lexical recall uses
    /// (`RecallResultTrust`). Populated when callers request authority context, so
    /// a record that certifies lexically certifies here too.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust: Option<RecallResultTrust>,
    /// Human-readable excerpt.
    pub excerpt: String,
    /// Optional evidence episode identifier.
    pub evidence_id: Option<String>,
    /// Normalized lexical terms found in the item.
    pub matched_terms: Vec<String>,
    /// Score components and trust notes.
    pub explanations: Vec<String>,
}

/// Explainable hybrid recall report.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct HybridRecallReport {
    /// Query text.
    pub query: String,
    /// Result limit requested by the caller.
    pub limit: usize,
    /// Qdrant collection name queried.
    pub collection_name: String,
    /// Embedding provider used for the query vector.
    pub embedding: EmbeddingProviderConfig,
    /// Query/scope-level authority decision used to contextualize recall.
    pub authority: AuthorityDecision,
    /// Store-wide authority, independent of query filters.
    pub store_authority: AuthorityDecision,
    /// Lexical results considered before merging.
    pub lexical_results: Vec<RecallResult>,
    /// Semantic results returned by Qdrant before merging.
    pub semantic_results: Vec<SemanticMatch>,
    /// Merged recall results ordered by combined score.
    pub results: Vec<HybridRecallResult>,
}
