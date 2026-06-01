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
