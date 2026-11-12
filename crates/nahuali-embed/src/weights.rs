//! A safetensors file read in place: tensors on demand, rows by offset.
//!
//! The word table holds 90% of a small multilingual model's parameters, and a
//! query touches about twenty of its 250,000 rows. Reading only those rows is
//! what lets each `nahuali recall` process load the model in milliseconds.

use std::collections::HashMap;
use std::fs::File;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};

use candle_core::{Device, Tensor};
use serde::Deserialize;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dtype {
    F32,
    Bf16,
}

impl Dtype {
    fn width(self) -> usize {
        match self {
            Dtype::F32 => 4,
            Dtype::Bf16 => 2,
        }
    }
}

#[derive(Deserialize)]
struct RawEntry {
    dtype: String,
    shape: Vec<usize>,
    data_offsets: (u64, u64),
}

struct Entry {
    dtype: Dtype,
    shape: Vec<usize>,
    start: u64,
}

/// An open safetensors file.
pub struct Weights {
    file: File,
    path: PathBuf,
    entries: HashMap<String, Entry>,
}

fn decode(bytes: &[u8], dtype: Dtype) -> Vec<f32> {
    match dtype {
        Dtype::F32 => bytes.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b)).collect(),
        Dtype::Bf16 => bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| f32::from_bits(u32::from(u16::from_le_bytes(*b)) << 16))
            .collect(),
    }
}

impl Weights {
    /// Open and read the header.
    pub fn open(path: &Path) -> Result<Weights> {
        let file = File::open(path).map_err(|e| Error::file(path, e))?;
        let mut len = [0_u8; 8];
        file.read_exact_at(&mut len, 0).map_err(|e| Error::file(path, e))?;
        let len = u64::from_le_bytes(len);
        let mut header = vec![0_u8; len as usize];
        file.read_exact_at(&mut header, 8).map_err(|e| Error::file(path, e))?;
        let raw: HashMap<String, serde_json::Value> =
            serde_json::from_slice(&header).map_err(|e| Error::Model(format!("safetensors header: {e}")))?;
        let mut entries = HashMap::with_capacity(raw.len());
        for (name, value) in raw {
            // `__metadata__`, and integer buffers no forward pass reads.
            let Ok(entry) = serde_json::from_value::<RawEntry>(value) else { continue };
            let dtype = match entry.dtype.as_str() {
                "F32" => Dtype::F32,
                "BF16" => Dtype::Bf16,
                _ => continue,
            };
            entries.insert(name, Entry { dtype, shape: entry.shape, start: 8 + len + entry.data_offsets.0 });
        }
        Ok(Weights { file, path: path.to_path_buf(), entries })
    }

    fn entry(&self, name: &str) -> Result<&Entry> {
        self.entries
            .get(name)
            .ok_or_else(|| Error::Model(format!("{} has no tensor {name}", self.path.display())))
    }

    fn read(&self, offset: u64, len: usize) -> Result<Vec<u8>> {
        let mut bytes = vec![0_u8; len];
        self.file.read_exact_at(&mut bytes, offset).map_err(|e| Error::file(&self.path, e))?;
        Ok(bytes)
    }

    /// A whole tensor as f32.
    pub fn tensor(&self, name: &str) -> Result<Tensor> {
        let entry = self.entry(name)?;
        let elements: usize = entry.shape.iter().product();
        let bytes = self.read(entry.start, elements * entry.dtype.width())?;
        Ok(Tensor::from_vec(decode(&bytes, entry.dtype), entry.shape.as_slice(), &Device::Cpu)?)
    }

    /// Rows `ids` of a 2-D tensor, concatenated as f32.
    pub fn rows(&self, name: &str, ids: &[u32]) -> Result<Vec<f32>> {
        let entry = self.entry(name)?;
        let [rows, width] = entry.shape[..] else {
            return Err(Error::Model(format!("{name} is not a matrix")));
        };
        let stride = width * entry.dtype.width();
        let mut out = Vec::with_capacity(ids.len() * width);
        for &id in ids {
            if id as usize >= rows {
                return Err(Error::Model(format!("token {id} is outside {name}")));
            }
            let bytes = self.read(entry.start + u64::from(id) * stride as u64, stride)?;
            out.extend(decode(&bytes, entry.dtype));
        }
        Ok(out)
    }

    /// Length of a matrix's rows.
    pub fn width(&self, name: &str) -> Result<usize> {
        self.entry(name)?.shape.last().copied().ok_or_else(|| Error::Model(format!("{name} has no shape")))
    }
}
