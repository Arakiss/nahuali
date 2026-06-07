#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::{
        AuthorityDecision, AuthorityMode, EmbeddingProviderKind, MemoryData, SemanticConfig,
        SemanticIndexStatus,
        semantic::{DeterministicEmbedder, Embedder, hybrid_recall, index_status, rebuild_index},
    };

    #[test]
    fn deterministic_embedding_is_normalized() {
        let embedder = DeterministicEmbedder { dimensions: 16 };

        let vector = embedder.embed("Lena owns release notes");

        assert_eq!(vector.len(), 16);
        let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.001);
    }

    #[test]
    fn semantic_filter_emits_a_created_at_range() {
        use crate::RecallOptions;

        let filter = super::SemanticQueryFilter::from_options(&RecallOptions {
            as_of_ms: Some(500),
            since_ms: Some(100),
            ..RecallOptions::default()
        });
        let json = filter
            .to_qdrant_filter()
            .expect("a bounded window yields a filter");
        let must = json["must"].as_array().expect("must clause is an array");
        let range = must
            .iter()
            .find(|clause| clause["key"] == "created_at_ms")
            .expect("a created_at_ms range clause is present");
        assert_eq!(range["range"]["lte"], serde_json::json!(500));
        assert_eq!(range["range"]["gte"], serde_json::json!(100));

        // No temporal bounds (and nothing else set) yields no filter at all.
        let empty = super::SemanticQueryFilter::from_options(&RecallOptions::default());
        assert!(empty.to_qdrant_filter().is_none());
    }

    #[test]
    fn deterministic_embedder_clusters_shared_word_shapes() {
        let embedder = DeterministicEmbedder {
            dimensions: crate::DEFAULT_EMBEDDING_DIMENSIONS,
        };
        let dot = |a: &[f32], b: &[f32]| a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>();

        // "releasing"/"release" and "product"/"products" share no whole word, so
        // the old token-only hash scored these near-orthogonal. Character
        // n-grams give them shared word shapes, so the related text now ranks
        // clearly above an unrelated one.
        let anchor = embedder.embed("the team is releasing the product");
        let related = embedder.embed("release of our products");
        let unrelated = embedder.embed("quarterly budget committee meeting");

        let related_score = dot(&anchor, &related);
        let unrelated_score = dot(&anchor, &unrelated);
        assert!(
            related_score > unrelated_score + 0.05,
            "shared word shapes {related_score:.3} should beat unrelated {unrelated_score:.3}"
        );
        assert!(
            related_score > 0.1,
            "morphological variants should carry real similarity, got {related_score:.3}"
        );
    }

    #[test]
    fn semantic_config_rejects_invalid_collection_names() {
        let error = SemanticConfig::local_with_collection("bad/name")
            .expect_err("invalid collection fails");

        assert!(format!("{error}").contains("collection name"));
    }

    #[test]
    fn semantic_config_scopes_collection_by_database_name() {
        let scoped = SemanticConfig::local_with_collection("nahuali_memory_items")
            .expect("base collection is valid")
            .scoped_to_database("tenant alpha/01")
            .expect("database-scoped collection is valid");

        assert_eq!(
            scoped.collection_name,
            "nahuali_memory_items__tenant_alpha_01"
        );
    }

    #[test]
    fn hosted_embedding_configuration_is_explicit_but_not_builtin() {
        let mut config = SemanticConfig::default_local();
        config.embedding.kind = EmbeddingProviderKind::Hosted;
        config.embedding.model = "hosted/example".to_string();

        let data = MemoryData::default();
        let error = rebuild_index(&data, &config).expect_err("hosted provider requires adapter");

        assert!(format!("{error}").contains("only the deterministic local provider"));
    }

    #[test]
    fn status_reports_missing_collection_without_error_when_qdrant_is_available() {
        let Some(config) = qdrant_test_config("missing") else {
            return;
        };

        let status = index_status(&config).expect("status checks qdrant");

        assert_eq!(
            status,
            SemanticIndexStatus {
                collection_name: config.collection_name,
                qdrant_url: config.qdrant_url,
                collection_exists: false,
                point_count: 0,
            }
        );
    }

    #[test]
    fn hybrid_recall_requires_existing_qdrant_collection_when_available() {
        let Some(config) = qdrant_test_config("hybrid-missing") else {
            return;
        };
        let data = MemoryData::default();
        let authority = AuthorityDecision {
            mode: AuthorityMode::Certify,
            score: 1.0,
            can_trust: true,
            reasons: Vec::new(),
            signal_kinds: Vec::new(),
        };

        let error = hybrid_recall(&data, "release notes", 10, authority, &config)
            .expect_err("missing collection fails");

        assert!(format!("{error}").contains("/points/query"));
    }

    #[cfg(not(feature = "local-embeddings"))]
    #[test]
    fn local_model_requires_local_embeddings_feature() {
        let mut config = SemanticConfig::default_local();
        config.embedding.kind = EmbeddingProviderKind::LocalModel;
        config.embedding.model = "model2vec".to_string();

        let data = MemoryData::default();
        let error = rebuild_index(&data, &config).expect_err("local model requires the feature");

        assert!(format!("{error}").contains("local-embeddings"));
    }

    #[cfg(feature = "local-embeddings")]
    #[test]
    fn local_model_embeds_with_model_dimensions_when_present() {
        let Some(path) = std::env::var("NAHUALI_LOCAL_EMBEDDING_MODEL_PATH")
            .ok()
            .filter(|value| !value.is_empty())
        else {
            return;
        };
        let mut config = SemanticConfig::default_local();
        config.embedding.kind = EmbeddingProviderKind::LocalModel;
        config.embedding.model = "model2vec".to_string();
        config.embedding_model_path = Some(path);

        let embedder = config.embedder().expect("local model loads from disk");
        let vector = embedder.embed("Lena owns the release notes");

        assert!(embedder.dimensions() > 0);
        assert_eq!(vector.len(), embedder.dimensions());
    }

    /// Reproducible proof that the local model embedder carries real meaning the
    /// deterministic token-hash embedder cannot. The synonym phrasing below shares
    /// only stopwords with the query, so the deterministic embedder — which hashes
    /// tokens — cannot tell it apart from an unrelated sentence: both land at a
    /// similar middling score. A model2vec model places the synonym far above the
    /// unrelated sentence. What matters for recall ranking is that *separation*,
    /// and the model's is several times the deterministic embedder's. That is the
    /// "stronger semantic recall" the README promises.
    ///
    /// Runs only when `NAHUALI_LOCAL_EMBEDDING_MODEL_PATH` points at a local
    /// model directory; otherwise it is a no-op so default CI stays offline.
    #[cfg(feature = "local-embeddings")]
    #[test]
    fn local_model_separates_meaning_where_deterministic_cannot() {
        let Some(path) = std::env::var("NAHUALI_LOCAL_EMBEDDING_MODEL_PATH")
            .ok()
            .filter(|value| !value.is_empty())
        else {
            return;
        };

        // The synonym shares no content words with the query, only stopwords.
        let query = "a car travels along the street";
        let synonym = "the automobile drives down the road";
        let unrelated = "the committee approved the quarterly budget";

        let mut config = SemanticConfig::default_local();
        config.embedding.kind = EmbeddingProviderKind::LocalModel;
        config.embedding.model = "model2vec".to_string();
        config.embedding_model_path = Some(path);
        let model = config.embedder().expect("local model loads from disk");
        let deterministic = DeterministicEmbedder {
            dimensions: crate::DEFAULT_EMBEDDING_DIMENSIONS,
        };

        let model_synonym = cosine(&model.embed(query), &model.embed(synonym));
        let model_unrelated = cosine(&model.embed(query), &model.embed(unrelated));
        let det_synonym = cosine(&deterministic.embed(query), &deterministic.embed(synonym));
        let det_unrelated = cosine(&deterministic.embed(query), &deterministic.embed(unrelated));
        let model_separation = model_synonym - model_unrelated;
        let det_separation = det_synonym - det_unrelated;

        eprintln!(
            "model2vec:     synonym={model_synonym:.3} unrelated={model_unrelated:.3} \
             separation={model_separation:.3}"
        );
        eprintln!(
            "deterministic: synonym={det_synonym:.3} unrelated={det_unrelated:.3} \
             separation={det_separation:.3}"
        );

        // The model ranks the synonym clearly above an unrelated sentence.
        assert!(
            model_synonym > model_unrelated + 0.2,
            "model synonym {model_synonym:.3} should clear unrelated {model_unrelated:.3}"
        );
        // And it separates meaning far better than the token-hash embedder, which
        // is what makes recall ranking actually reflect meaning.
        assert!(
            model_separation > det_separation + 0.2,
            "model separation {model_separation:.3} should beat deterministic {det_separation:.3}"
        );
    }

    #[cfg(feature = "local-embeddings")]
    fn cosine(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
        let norm = |v: &[f32]| v.iter().map(|x| x * x).sum::<f32>().sqrt();
        let denominator = norm(a) * norm(b);
        if denominator == 0.0 {
            0.0
        } else {
            dot / denominator
        }
    }

    fn qdrant_test_config(suffix: &str) -> Option<SemanticConfig> {
        let config = SemanticConfig::local_with_collection(format!(
            "nahuali_core_semantic_test_{}_{}_{}",
            suffix.replace('-', "_"),
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock is after epoch")
                .as_nanos()
        ))
        .expect("test collection name is valid");
        let status = index_status(&config).ok()?;
        if status.collection_exists {
            return None;
        }
        Some(config)
    }
}
