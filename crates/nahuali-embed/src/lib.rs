//! Local multilingual text embeddings: no server, no cloud, no network at
//! query time. `nahuali setup --meaning` downloads a model with [`install`];
//! [`LocalEmbedder`] runs it in process on the CPU.

#![warn(missing_docs)]

mod embedder;
mod encoder;
mod error;
mod install;
mod local;
pub mod testing;
#[doc(hidden)]
pub mod unigram;
mod weights;

pub use embedder::{EmbedError, Embedder};
pub use error::{Error, Result};
pub use install::{install, installed};
pub use local::{DEFAULT_MODEL, LocalEmbedder, MODELS, ModelSpec, spec};
#[doc(hidden)]
pub use unigram::{Unigram, compile};
