//! The forward pass of a post-LN BERT encoder, which covers both the BERT and
//! the XLM-R checkpoints this crate runs.

use std::path::Path;

use candle_core::{D, Device, Tensor};
use candle_nn::{LayerNorm, Linear, Module, ops::softmax_last_dim};
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::weights::Weights;

const WORDS: &str = "embeddings.word_embeddings.weight";

/// How a text's vector is read from its token states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pooling {
    /// The state of the first token.
    Cls,
    /// The mean over real tokens.
    Mean,
}

#[derive(Deserialize)]
struct Config {
    hidden_size: usize,
    num_attention_heads: usize,
    num_hidden_layers: usize,
    layer_norm_eps: f64,
    max_position_embeddings: usize,
}

struct Layer {
    query: Linear,
    key: Linear,
    value: Linear,
    attention_out: Linear,
    attention_norm: LayerNorm,
    intermediate: Linear,
    output: Linear,
    output_norm: LayerNorm,
}

/// A loaded encoder. All but the word table is in memory.
pub struct Encoder {
    weights: Weights,
    positions: Tensor,
    token_type: Tensor,
    norm: LayerNorm,
    layers: Vec<Layer>,
    heads: usize,
    hidden: usize,
    pooling: Pooling,
    position_offset: usize,
    pad: u32,
    max_tokens: usize,
}

fn linear(w: &Weights, prefix: &str) -> Result<Linear> {
    Ok(Linear::new(w.tensor(&format!("{prefix}.weight"))?, Some(w.tensor(&format!("{prefix}.bias"))?)))
}

fn norm(w: &Weights, prefix: &str, eps: f64) -> Result<LayerNorm> {
    Ok(LayerNorm::new(w.tensor(&format!("{prefix}.weight"))?, w.tensor(&format!("{prefix}.bias"))?, eps))
}

impl Encoder {
    /// Load `config.json` and `model.safetensors` from `dir`. `position_offset`
    /// is where position ids start (2 in XLM-R, 0 in BERT).
    pub fn open(dir: &Path, pooling: Pooling, position_offset: usize, pad: u32) -> Result<Encoder> {
        let config_path = dir.join("config.json");
        let config = std::fs::read(&config_path).map_err(|e| Error::file(&config_path, e))?;
        let config: Config =
            serde_json::from_slice(&config).map_err(|e| Error::Model(format!("config.json: {e}")))?;
        let w = Weights::open(&dir.join("model.safetensors"))?;
        let eps = config.layer_norm_eps;
        let mut layers = Vec::with_capacity(config.num_hidden_layers);
        for i in 0..config.num_hidden_layers {
            let p = format!("encoder.layer.{i}");
            layers.push(Layer {
                query: linear(&w, &format!("{p}.attention.self.query"))?,
                key: linear(&w, &format!("{p}.attention.self.key"))?,
                value: linear(&w, &format!("{p}.attention.self.value"))?,
                attention_out: linear(&w, &format!("{p}.attention.output.dense"))?,
                attention_norm: norm(&w, &format!("{p}.attention.output.LayerNorm"), eps)?,
                intermediate: linear(&w, &format!("{p}.intermediate.dense"))?,
                output: linear(&w, &format!("{p}.output.dense"))?,
                output_norm: norm(&w, &format!("{p}.output.LayerNorm"), eps)?,
            });
        }
        let positions = w.tensor("embeddings.position_embeddings.weight")?;
        let token_type = w.tensor("embeddings.token_type_embeddings.weight")?.narrow(0, 0, 1)?;
        let norm = norm(&w, "embeddings.LayerNorm", eps)?;
        let hidden = w.width(WORDS)?;
        if hidden != config.hidden_size {
            return Err(Error::Model("config.json and the word table disagree on the width".into()));
        }
        let max_tokens = config.max_position_embeddings - position_offset;
        Ok(Encoder {
            weights: w,
            positions,
            token_type,
            norm,
            layers,
            heads: config.num_attention_heads,
            hidden,
            pooling,
            position_offset,
            pad,
            max_tokens,
        })
    }

    /// Longest input in tokens.
    pub fn max_tokens(&self) -> usize {
        self.max_tokens
    }

    /// Vector length.
    pub fn hidden(&self) -> usize {
        self.hidden
    }

    /// Unit-length vectors of token sequences, padded to the longest.
    pub fn forward(&self, batch: &[Vec<u32>]) -> Result<Vec<Vec<f32>>> {
        let (b, t) = (batch.len(), batch.iter().map(Vec::len).max().unwrap_or(0));
        if b == 0 || t == 0 {
            return Ok(Vec::new());
        }
        let mut ids = Vec::with_capacity(b * t);
        let mut keep = Vec::with_capacity(b * t);
        for row in batch {
            ids.extend_from_slice(row);
            ids.extend(std::iter::repeat_n(self.pad, t - row.len()));
            keep.extend(std::iter::repeat_n(1.0_f32, row.len()));
            keep.extend(std::iter::repeat_n(0.0_f32, t - row.len()));
        }
        let device = Device::Cpu;
        let keep = Tensor::from_vec(keep, (b, t), &device)?;
        let words = Tensor::from_vec(self.weights.rows(WORDS, &ids)?, (b, t, self.hidden), &device)?;
        let positions = self.positions.narrow(0, self.position_offset, t)?;
        let x = words.broadcast_add(&positions)?.broadcast_add(&self.token_type)?;
        let mut x = self.norm.forward(&x)?;
        // Padding only exists in batches with uneven lengths.
        let bias = if batch.iter().all(|r| r.len() == t) {
            None
        } else {
            Some(((&keep - 1.0)? * 1e9)?.reshape((b, 1, 1, t))?)
        };
        for layer in &self.layers {
            x = self.layer(layer, &x, bias.as_ref())?;
        }
        let pooled = match self.pooling {
            Pooling::Cls => x.narrow(1, 0, 1)?.squeeze(1)?,
            Pooling::Mean => {
                let mask = keep.unsqueeze(D::Minus1)?;
                x.broadcast_mul(&mask)?.sum(1)?.broadcast_div(&mask.sum(1)?)?
            }
        };
        let length = pooled.sqr()?.sum_keepdim(1)?.sqrt()?;
        Ok(pooled.broadcast_div(&length)?.to_vec2()?)
    }

    fn layer(&self, l: &Layer, x: &Tensor, bias: Option<&Tensor>) -> Result<Tensor> {
        let (b, t, h) = x.dims3()?;
        let width = h / self.heads;
        let split = |y: Tensor| -> Result<Tensor> {
            Ok(y.reshape((b, t, self.heads, width))?.transpose(1, 2)?.contiguous()?)
        };
        let q = split(l.query.forward(x)?)?;
        let k = split(l.key.forward(x)?)?;
        let v = split(l.value.forward(x)?)?;
        let mut scores = (q.matmul(&k.transpose(2, 3)?.contiguous()?)? / (width as f64).sqrt())?;
        if let Some(bias) = bias {
            scores = scores.broadcast_add(bias)?;
        }
        let context = softmax_last_dim(&scores)?.matmul(&v)?.transpose(1, 2)?.reshape((b, t, h))?;
        let attended = l.attention_norm.forward(&(l.attention_out.forward(&context)? + x)?)?;
        let inner = l.intermediate.forward(&attended)?.gelu_erf()?;
        Ok(l.output_norm.forward(&(l.output.forward(&inner)? + attended)?)?)
    }
}
