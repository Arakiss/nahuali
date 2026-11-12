//! An embedder with no model, for tests: each word lands in one bucket, so
//! two texts are close exactly when they share words. Words named as the
//! same share a bucket, which is how a test stands in for what a real model
//! knows about saying one thing in other words.

use crate::embedder::{EmbedError, Embedder};

/// Words a test treats as one: a stand-in for meaning.
const SAME: &[&[&str]] = &[
    &["audit", "inspection", "auditoria"],
    &["migrate", "migration", "alembic"],
    &["logo", "brand", "artwork"],
    &["worktree", "checkout"],
];

/// A bag-of-words embedder of the real model's width.
#[derive(Debug, Clone, Copy, Default)]
pub struct WordEmbedder;

impl WordEmbedder {
    fn bucket(word: &str) -> usize {
        let word = SAME.iter().find(|group| group.contains(&word)).map_or(word, |group| group[0]);
        let hash = word
            .bytes()
            .fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3));
        (hash % 384) as usize
    }

    fn embed(text: &str) -> Vec<f32> {
        let mut v = vec![0.0_f32; 384];
        for word in text.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| w.len() > 2) {
            v[Self::bucket(word)] = 1.0;
        }
        let length = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if length > 0.0 {
            v.iter_mut().for_each(|x| *x /= length);
        }
        v
    }
}

impl Embedder for WordEmbedder {
    fn model(&self) -> &str {
        "words-for-tests"
    }

    fn dimension(&self) -> usize {
        384
    }

    fn embed_query(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        Ok(Self::embed(text))
    }

    fn embed_passages(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbedError> {
        Ok(texts.iter().map(|t| Self::embed(t)).collect())
    }
}
