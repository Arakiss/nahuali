/// Semantic index schema version stored in Qdrant payloads.
pub const SEMANTIC_INDEX_SCHEMA_VERSION: u32 = 2;
/// Default local Qdrant REST endpoint for the OSS Docker stack.
pub const DEFAULT_QDRANT_URL: &str = "http://localhost:16333";
/// Default collection used for memory-item vectors.
pub const DEFAULT_SEMANTIC_COLLECTION: &str = "nahuali_memory_items";
/// Default dimensionality for the deterministic local embedding provider.
pub const DEFAULT_EMBEDDING_DIMENSIONS: usize = 64;

const LOCAL_EMBEDDING_MODEL: &str = "nahuali/deterministic-token-v1";

/// Configuration for Qdrant-backed semantic indexing.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SemanticConfig {
    /// Qdrant REST endpoint.
    pub qdrant_url: String,
    /// Qdrant collection name for projected memory items.
    pub collection_name: String,
    /// Embedding provider configuration.
    pub embedding: EmbeddingProviderConfig,
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

        Ok(Self {
            qdrant_url: normalize_qdrant_url(&qdrant_url),
            collection_name: normalize_collection_name(&collection_name)?,
            embedding,
        })
    }

    /// Return the default local deterministic configuration.
    pub fn default_local() -> Self {
        Self {
            qdrant_url: DEFAULT_QDRANT_URL.to_string(),
            collection_name: DEFAULT_SEMANTIC_COLLECTION.to_string(),
            embedding: EmbeddingProviderConfig::deterministic(DEFAULT_EMBEDDING_DIMENSIONS),
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

    fn embedder(&self) -> Result<DeterministicEmbedder> {
        if self.embedding.kind != EmbeddingProviderKind::DeterministicLocal {
            return Err(NahualiError::InvalidSemanticConfig {
                message: format!(
                    "embedding provider '{}' is configured but only the deterministic local provider is built into nahuali-core",
                    self.embedding.model
                ),
            });
        }
        Ok(DeterministicEmbedder {
            dimensions: self.embedding.dimensions,
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
    pub authority_score: f32,
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
    /// Projection-level authority decision used to contextualize recall.
    pub authority: AuthorityDecision,
    /// Lexical results considered before merging.
    pub lexical_results: Vec<RecallResult>,
    /// Semantic results returned by Qdrant before merging.
    pub semantic_results: Vec<SemanticMatch>,
    /// Merged recall results ordered by combined score.
    pub results: Vec<HybridRecallResult>,
}
