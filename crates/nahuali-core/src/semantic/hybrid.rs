#[derive(Debug)]
struct HybridAccumulator {
    kind: MemoryKind,
    id: String,
    lexical_score: Option<f32>,
    semantic_score: Option<f32>,
    excerpt: String,
    evidence_id: Option<String>,
    matched_terms: Vec<String>,
}

impl HybridAccumulator {
    fn from_lexical(result: &RecallResult) -> Self {
        Self {
            kind: result.kind.clone(),
            id: result.id.clone(),
            lexical_score: None,
            semantic_score: None,
            excerpt: result.excerpt.clone(),
            evidence_id: result.evidence_id.clone(),
            matched_terms: result.matched_terms.clone(),
        }
    }

    fn from_semantic(result: &SemanticMatch) -> Self {
        Self {
            kind: result.kind.clone(),
            id: result.id.clone(),
            lexical_score: None,
            semantic_score: None,
            excerpt: result.excerpt.clone(),
            evidence_id: result.evidence_id.clone(),
            matched_terms: Vec::new(),
        }
    }

    fn merge_lexical(&mut self, result: &RecallResult) {
        self.lexical_score = Some(result.score);
        self.excerpt = result.excerpt.clone();
        self.evidence_id = result
            .evidence_id
            .clone()
            .or_else(|| self.evidence_id.clone());
        self.matched_terms = result.matched_terms.clone();
    }

    fn merge_semantic(&mut self, result: &SemanticMatch) {
        self.semantic_score = Some(result.score);
        if self.excerpt.is_empty() {
            self.excerpt = result.excerpt.clone();
        }
        self.evidence_id = self
            .evidence_id
            .clone()
            .or_else(|| result.evidence_id.clone());
    }

    fn finish(self, authority: &AuthorityDecision) -> HybridRecallResult {
        let evidence_bonus = if self.evidence_id.is_some() {
            0.25
        } else {
            0.0
        };
        let score =
            self.lexical_score.unwrap_or(0.0) + self.semantic_score.unwrap_or(0.0) + evidence_bonus;
        let mut explanations = Vec::new();
        if let Some(score) = self.lexical_score {
            explanations.push(format!("lexical_score={score:.3}"));
        }
        if let Some(score) = self.semantic_score {
            explanations.push(format!("semantic_score={score:.3}"));
        }
        if self.evidence_id.is_some() {
            explanations.push("evidence_bonus=0.250".to_string());
        }
        explanations.push(format!(
            "authority={}(score={:.2})",
            authority_mode_name(&authority.mode),
            authority.score
        ));

        HybridRecallResult {
            kind: self.kind,
            id: self.id,
            score,
            lexical_score: self.lexical_score,
            semantic_score: self.semantic_score,
            authority_score: authority.score,
            excerpt: self.excerpt,
            evidence_id: self.evidence_id,
            matched_terms: self.matched_terms,
            explanations,
        }
    }
}

fn semantic_http_error(endpoint: &str, source: reqwest::Error) -> NahualiError {
    NahualiError::SemanticHttp {
        endpoint: endpoint.to_string(),
        source: Box::new(source),
    }
}

fn normalize_qdrant_url(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}

fn normalize_collection_name(name: &str) -> Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(NahualiError::InvalidSemanticConfig {
            message: "semantic collection name cannot be empty".to_string(),
        });
    }
    if !trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err(NahualiError::InvalidSemanticConfig {
            message: format!(
                "semantic collection name can only contain ASCII letters, digits, '_' or '-', got {trimmed}"
            ),
        });
    }
    Ok(trimmed.to_string())
}

fn normalize_database_suffix(name: &str) -> String {
    let mut suffix = name
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if suffix.is_empty() {
        suffix = "memory".to_string();
    }
    if suffix
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_digit())
    {
        suffix.insert_str(0, "tenant_");
    }
    suffix
}

fn non_empty_env(key: &str) -> Option<String> {
    std::env::var(key).ok().and_then(|value| {
        let value = value.trim().to_string();
        if value.is_empty() { None } else { Some(value) }
    })
}

fn result_key(kind: &MemoryKind, id: &str) -> String {
    format!("{}:{id}", memory_kind_name(kind))
}

fn point_id_for(kind: &str, id: &str) -> u64 {
    fnv1a64(format!("{kind}:{id}").as_bytes())
}

fn memory_kind_name(kind: &MemoryKind) -> &'static str {
    match kind {
        MemoryKind::Entity => "entity",
        MemoryKind::Episode => "episode",
        MemoryKind::Claim => "claim",
        MemoryKind::Link => "link",
        MemoryKind::Procedure => "procedure",
        MemoryKind::Intention => "intention",
        MemoryKind::Fact => "fact",
        MemoryKind::Relation => "relation",
    }
}

fn memory_kind_from_name(kind: &str) -> Option<MemoryKind> {
    match kind {
        "entity" => Some(MemoryKind::Entity),
        "episode" => Some(MemoryKind::Episode),
        "claim" => Some(MemoryKind::Claim),
        "link" => Some(MemoryKind::Link),
        "procedure" => Some(MemoryKind::Procedure),
        "intention" => Some(MemoryKind::Intention),
        "fact" => Some(MemoryKind::Fact),
        "relation" => Some(MemoryKind::Relation),
        _ => None,
    }
}

fn surreal_table_for(kind: &MemoryKind) -> &'static str {
    match kind {
        MemoryKind::Entity => "entity",
        MemoryKind::Episode => "episode",
        MemoryKind::Claim | MemoryKind::Fact => "claim",
        MemoryKind::Link | MemoryKind::Relation => "relates_to",
        MemoryKind::Procedure => "procedure",
        MemoryKind::Intention => "intention",
    }
}

fn authority_mode_name(mode: &AuthorityMode) -> &'static str {
    match mode {
        AuthorityMode::Advisory => "advisory",
        AuthorityMode::Warn => "warn",
        AuthorityMode::Block => "block",
        AuthorityMode::Certify => "certify",
    }
}

fn tokenize(input: &str) -> Vec<String> {
    input
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .filter(|term| term.len() > 1)
        .map(ToOwned::to_owned)
        .collect()
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    bytes.iter().fold(FNV_OFFSET, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
    })
}
