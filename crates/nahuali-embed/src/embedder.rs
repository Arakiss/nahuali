//! The seam between recall and whatever turns text into vectors.
//!
//! Vectors are derived data: they never enter an event or an export, and
//! they can always be computed again from the text.

use thiserror::Error;

/// Text could not be embedded.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{0}")]
pub struct EmbedError(pub String);

/// Turns text into unit-length vectors. Queries and passages are separate
/// calls because some models mark them differently.
pub trait Embedder: Send + Sync {
    /// Stable model id, stored with each vector.
    fn model(&self) -> &str;
    /// Vector length.
    fn dimension(&self) -> usize;
    /// Embed a search query.
    fn embed_query(&self, text: &str) -> Result<Vec<f32>, EmbedError>;
    /// Embed stored texts, in order.
    fn embed_passages(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbedError>;
}
