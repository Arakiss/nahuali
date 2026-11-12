//! The supported models and the [`Embedder`] that runs them.

use std::path::Path;

use crate::embedder::{EmbedError, Embedder};

use crate::encoder::{Encoder, Pooling};
use crate::error::{Error, Result};
use crate::unigram::{INDEX_FILE, Unigram};

/// Tokens per forward pass when embedding many texts.
const BATCH_TOKENS: usize = 4096;

/// One downloadable file: name and its pinned SHA-256.
pub type ModelFile = (&'static str, &'static str);

/// A model this build can run.
#[derive(Debug, Clone, Copy)]
pub struct ModelSpec {
    /// Id, also the directory under `models/`.
    pub id: &'static str,
    /// Hugging Face repository.
    pub repo: &'static str,
    /// Pinned revision.
    pub revision: &'static str,
    /// Files, with their hashes.
    pub files: &'static [ModelFile],
    /// Prepended to queries.
    pub query_prefix: &'static str,
    /// Prepended to stored text.
    pub passage_prefix: &'static str,
    pooling: Pooling,
    position_offset: usize,
    pad: u32,
}

/// Models this build can run.
pub const MODELS: &[ModelSpec] = &[
    ModelSpec {
        id: "granite-107m-multilingual",
        repo: "ibm-granite/granite-embedding-107m-multilingual",
        revision: "d6cffd338414d6a1c1f5decfad5fec62eebc90d5",
        files: &[
            ("config.json", "aa3b6a5d1d6efeacdb844fb8ba773556463af2195003ad5e70a6f014d11be9cf"),
            ("tokenizer.json", "7fc9e475f2ac473f070052d92b0c46ec2b91d9cd672935feb6fd837a65d33f78"),
            ("model.safetensors", "ae6f20cc5dc433041118c900b652d87532e1335dade78c37765f0b94d1161283"),
        ],
        query_prefix: "",
        passage_prefix: "",
        pooling: Pooling::Cls,
        position_offset: 2,
        pad: 1,
    },
    ModelSpec {
        id: "multilingual-e5-small",
        repo: "intfloat/multilingual-e5-small",
        revision: "614241f622f53c4eeff9890bdc4f31cfecc418b3",
        files: &[
            ("config.json", "69137736cab8b8903a07fe8afaafdda25aac55415a12a55d1bffa9f581abf959"),
            ("tokenizer.json", "0b44a9d7b51c3c62626640cda0e2c2f70fdacdc25bbbd68038369d14ebdf4c39"),
            ("model.safetensors", "1a55775f53449dac10a2bcbc312469fac40b96d53198c407081a831f81c98477"),
        ],
        query_prefix: "query: ",
        passage_prefix: "passage: ",
        pooling: Pooling::Mean,
        position_offset: 0,
        pad: 1,
    },
];

/// The model `embed setup` installs by default.
pub const DEFAULT_MODEL: &str = "granite-107m-multilingual";

/// A model by id.
pub fn spec(id: &str) -> Result<&'static ModelSpec> {
    MODELS.iter().find(|m| m.id == id).ok_or_else(|| Error::Unknown(id.to_string()))
}

/// A model loaded from `models_dir`.
pub struct LocalEmbedder {
    spec: &'static ModelSpec,
    tokenizer: Unigram,
    encoder: Encoder,
}

impl LocalEmbedder {
    /// Load an installed model.
    pub fn open(models_dir: &Path, id: &str) -> Result<LocalEmbedder> {
        let spec = spec(id)?;
        if !crate::install::installed(models_dir, spec) {
            return Err(Error::NotInstalled(id.to_string()));
        }
        let dir = models_dir.join(spec.id);
        let tokenizer = Unigram::open(&dir.join(INDEX_FILE))?;
        let encoder = Encoder::open(&dir, spec.pooling, spec.position_offset, spec.pad)?;
        Ok(LocalEmbedder { spec, tokenizer, encoder })
    }

    fn encode(&self, prefix: &str, text: &str) -> Vec<u32> {
        self.tokenizer.encode(&format!("{prefix}{text}"), self.encoder.max_tokens())
    }
}

impl Embedder for LocalEmbedder {
    fn model(&self) -> &str {
        self.spec.id
    }

    fn dimension(&self) -> usize {
        self.encoder.hidden()
    }

    fn embed_query(&self, text: &str) -> std::result::Result<Vec<f32>, EmbedError> {
        let ids = self.encode(self.spec.query_prefix, text);
        let vectors = self.encoder.forward(&[ids]).map_err(|e| EmbedError(e.to_string()))?;
        vectors.into_iter().next().ok_or_else(|| EmbedError("the model returned no vector".into()))
    }

    fn embed_passages(&self, texts: &[&str]) -> std::result::Result<Vec<Vec<f32>>, EmbedError> {
        let mut order: Vec<(usize, Vec<u32>)> =
            texts.iter().enumerate().map(|(i, t)| (i, self.encode(self.spec.passage_prefix, t))).collect();
        // Similar lengths together, so little is spent on padding.
        order.sort_by_key(|(_, ids)| ids.len());
        let mut out = vec![Vec::new(); texts.len()];
        let mut at = 0;
        while at < order.len() {
            let mut end = at + 1;
            while end < order.len() && order[end].1.len() * (end + 1 - at) <= BATCH_TOKENS {
                end += 1;
            }
            let batch: Vec<Vec<u32>> = order[at..end].iter().map(|(_, ids)| ids.clone()).collect();
            let vectors = self.encoder.forward(&batch).map_err(|e| EmbedError(e.to_string()))?;
            for ((index, _), vector) in order[at..end].iter().zip(vectors) {
                out[*index] = vector;
            }
            at = end;
        }
        Ok(out)
    }
}
