//! The Unigram tokenizer of the XLM-R family, loadable in a few milliseconds.
//!
//! `nahuali recall` is a new process per query, and parsing `tokenizer.json`
//! into the `tokenizers` crate takes about 270 ms, more than the whole recall
//! budget. So `embed setup` compiles the vocabulary once into [`INDEX_FILE`],
//! an open-addressing table of piece hashes that is read and used as bytes,
//! with no parsing at all. Encoding matches `tokenizers` id for id
//! (`tests/parity.rs`).
//!
//! Layout, little endian: magic, then `slots`, `unk`, `bos`, `eos` and
//! `max_piece` as u32, `min_score` as f32, the charsmap length as u32, the
//! charsmap, and `slots` entries of (hash u64, id u32, score f32).

use std::fs;
use std::path::Path;

use serde::Deserialize;
use spm_precompiled::Precompiled;

use crate::error::{Error, Result};

/// The compiled vocabulary, next to `tokenizer.json`.
pub const INDEX_FILE: &str = "vocab.idx";

const MAGIC: &[u8; 8] = b"NHLVOC01";
const HEADER: usize = 8 + 7 * 4;
const SLOT: usize = 16;
const EMPTY: u32 = u32::MAX;
/// Score penalty of an unknown character below the worst piece, as in
/// SentencePiece.
const UNK_PENALTY: f32 = 10.0;
/// Metaspace: how a space looks inside a piece.
const SPACE: char = '▁';

#[derive(Deserialize)]
struct TokenizerJson {
    added_tokens: Vec<AddedToken>,
    normalizer: Normalizers,
    model: Model,
}

#[derive(Deserialize)]
struct AddedToken {
    id: u32,
    content: String,
}

#[derive(Deserialize)]
struct Normalizers {
    normalizers: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct Model {
    unk_id: u32,
    vocab: Vec<(String, f64)>,
}

fn fnv(hash: u64, byte: u8) -> u64 {
    (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3)
}

const FNV_START: u64 = 0xcbf2_9ce4_8422_2325;

/// Compile `tokenizer.json` into `index`.
pub fn compile(tokenizer_json: &Path, index: &Path) -> Result<()> {
    let text = fs::read_to_string(tokenizer_json).map_err(|e| Error::file(tokenizer_json, e))?;
    let parsed: TokenizerJson =
        serde_json::from_str(&text).map_err(|e| Error::Model(format!("tokenizer.json: {e}")))?;
    let charsmap = parsed
        .normalizer
        .normalizers
        .iter()
        .find_map(|n| n.get("precompiled_charsmap").and_then(|c| c.as_str()))
        .ok_or_else(|| Error::Model("tokenizer.json has no precompiled charsmap".into()))?;
    let charsmap = base64(charsmap)?;
    let token = |name: &str| {
        parsed
            .added_tokens
            .iter()
            .find(|t| t.content == name)
            .map(|t| t.id)
            .ok_or_else(|| Error::Model(format!("tokenizer.json has no {name}")))
    };
    let (bos, eos) = (token("<s>")?, token("</s>")?);
    let reserved: Vec<u32> = parsed.added_tokens.iter().map(|t| t.id).collect();

    let slots = (parsed.model.vocab.len() * 2).next_power_of_two();
    let mut table = vec![(0_u64, EMPTY, 0.0_f32); slots];
    let (mut max_piece, mut min_score) = (0_usize, f32::MAX);
    for (id, (piece, score)) in parsed.model.vocab.iter().enumerate() {
        let id = id as u32;
        // Control tokens are placed by position, never found in text.
        if reserved.contains(&id) {
            continue;
        }
        let score = *score as f32;
        min_score = min_score.min(score);
        max_piece = max_piece.max(piece.len());
        let hash = piece.bytes().fold(FNV_START, fnv);
        let mut slot = hash as usize & (slots - 1);
        while table[slot].1 != EMPTY {
            slot = (slot + 1) & (slots - 1);
        }
        table[slot] = (hash, id, score);
    }

    let mut out = Vec::with_capacity(HEADER + charsmap.len() + slots * SLOT);
    out.extend_from_slice(MAGIC);
    for field in [slots as u32, parsed.model.unk_id, bos, eos, max_piece as u32] {
        out.extend_from_slice(&field.to_le_bytes());
    }
    out.extend_from_slice(&min_score.to_le_bytes());
    out.extend_from_slice(&(charsmap.len() as u32).to_le_bytes());
    out.extend_from_slice(&charsmap);
    for (hash, id, score) in table {
        out.extend_from_slice(&hash.to_le_bytes());
        out.extend_from_slice(&id.to_le_bytes());
        out.extend_from_slice(&score.to_le_bytes());
    }
    fs::write(index, out).map_err(|e| Error::file(index, e))
}

fn base64(text: &str) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let (mut acc, mut bits) = (0_u32, 0_u32);
    for byte in text.bytes().filter(|b| *b != b'=') {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return Err(Error::Model("tokenizer.json: bad base64 in the charsmap".into())),
        };
        acc = (acc << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Ok(out)
}

/// A compiled vocabulary.
pub struct Unigram {
    normalizer: Precompiled,
    table: Vec<u8>,
    mask: usize,
    unk: u32,
    bos: u32,
    eos: u32,
    max_piece: usize,
    unk_score: f32,
}

#[derive(Clone, Copy)]
struct Best {
    from: usize,
    id: u32,
    score: f32,
    reached: bool,
}

impl Unigram {
    /// Load a compiled vocabulary.
    pub fn open(index: &Path) -> Result<Unigram> {
        let bytes = fs::read(index).map_err(|e| Error::file(index, e))?;
        let bad = || Error::Model(format!("{} is not a vocabulary index", index.display()));
        if bytes.len() < HEADER || &bytes[..8] != MAGIC {
            return Err(bad());
        }
        let field = |i: usize| {
            let at = 8 + i * 4;
            u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
        };
        let (slots, charsmap_len) = (field(0) as usize, field(6) as usize);
        if bytes.len() != HEADER + charsmap_len + slots * SLOT || !slots.is_power_of_two() {
            return Err(bad());
        }
        let normalizer = Precompiled::from(&bytes[HEADER..HEADER + charsmap_len])
            .map_err(|e| Error::Model(format!("charsmap: {e}")))?;
        let (unk, bos, eos, max_piece) = (field(1), field(2), field(3), field(4) as usize);
        let min_score = f32::from_bits(field(5));
        let table = bytes[HEADER + charsmap_len..].to_vec();
        Ok(Unigram {
            normalizer,
            table,
            mask: slots - 1,
            unk,
            bos,
            eos,
            max_piece,
            unk_score: min_score - UNK_PENALTY,
        })
    }

    fn find(&self, hash: u64) -> Option<(u32, f32)> {
        let mut slot = hash as usize & self.mask;
        loop {
            let at = slot * SLOT;
            let row = &self.table[at..at + SLOT];
            let id = u32::from_le_bytes([row[8], row[9], row[10], row[11]]);
            if id == EMPTY {
                return None;
            }
            if u64::from_le_bytes([row[0], row[1], row[2], row[3], row[4], row[5], row[6], row[7]]) == hash {
                return Some((id, f32::from_le_bytes([row[12], row[13], row[14], row[15]])));
            }
            slot = (slot + 1) & self.mask;
        }
    }

    /// Token ids of `text` between `<s>` and `</s>`, at most `max` in all.
    pub fn encode(&self, text: &str, max: usize) -> Vec<u32> {
        let normalized = self.normalizer.normalize_string(text);
        let mut ids = vec![self.bos];
        // Runs of spaces collapse, then Metaspace: one mark per space, plus
        // one at the start, and pieces never cross a mark.
        let mut spaced = String::with_capacity(normalized.len() + 1);
        for ch in normalized.chars() {
            if ch != ' ' || !spaced.ends_with(' ') {
                spaced.push(ch);
            }
        }
        if !spaced.is_empty() {
            for word in spaced.strip_prefix(' ').unwrap_or(&spaced).split(' ') {
                self.viterbi(&format!("{SPACE}{word}"), &mut ids);
                if ids.len() >= max {
                    break;
                }
            }
        }
        ids.truncate(max.saturating_sub(1));
        ids.push(self.eos);
        ids
    }

    fn viterbi(&self, word: &str, ids: &mut Vec<u32>) {
        let bytes = word.as_bytes();
        let start = Best { from: 0, id: self.unk, score: 0.0, reached: false };
        let mut best = vec![start; bytes.len() + 1];
        best[0].reached = true;
        for (at, ch) in word.char_indices() {
            let till_here = best[at].score;
            let width = ch.len_utf8();
            let mut covered = false;
            let mut hash = FNV_START;
            for end in at..bytes.len().min(at + self.max_piece) {
                hash = fnv(hash, bytes[end]);
                if !word.is_char_boundary(end + 1) {
                    continue;
                }
                if let Some((id, score)) = self.find(hash) {
                    let node = &mut best[end + 1];
                    let candidate = till_here + score;
                    if !node.reached || candidate > node.score {
                        *node = Best { from: at, id, score: candidate, reached: true };
                    }
                    covered |= end + 1 - at == width;
                }
            }
            if !covered {
                let node = &mut best[at + width];
                let candidate = till_here + self.unk_score;
                if !node.reached || candidate > node.score {
                    *node = Best { from: at, id: self.unk, score: candidate, reached: true };
                }
            }
        }
        let mut path = Vec::new();
        let mut at = bytes.len();
        while at > 0 {
            let node = best[at];
            // Consecutive unknowns fuse into one.
            if !(node.id == self.unk && path.last() == Some(&self.unk)) {
                path.push(node.id);
            }
            at = node.from;
        }
        ids.extend(path.into_iter().rev());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A vocabulary with a charsmap that maps nothing: 256 empty double-array
    /// units behind their length.
    fn index(pieces: &[(&str, f64)]) -> (tempfile::TempDir, Unigram) {
        let dir = tempfile::tempdir().unwrap();
        let mut vocab = vec![
            serde_json::json!(["<s>", 0.0]),
            serde_json::json!(["<pad>", 0.0]),
            serde_json::json!(["</s>", 0.0]),
            serde_json::json!(["<unk>", 0.0]),
        ];
        vocab.extend(pieces.iter().map(|(p, s)| serde_json::json!([p, s])));
        let charsmap = format!("AAQA{}AAA=", "AAAA".repeat(341));
        let json = serde_json::json!({
            "added_tokens": [
                {"id": 0, "content": "<s>"}, {"id": 1, "content": "<pad>"},
                {"id": 2, "content": "</s>"}, {"id": 3, "content": "<unk>"}
            ],
            "normalizer": {"normalizers": [{"precompiled_charsmap": charsmap}]},
            "model": {"unk_id": 3, "vocab": vocab},
        });
        let (src, out) = (dir.path().join("tokenizer.json"), dir.path().join(INDEX_FILE));
        fs::write(&src, json.to_string()).unwrap();
        compile(&src, &out).unwrap();
        let unigram = Unigram::open(&out).unwrap();
        (dir, unigram)
    }

    #[test]
    fn picks_the_best_scoring_segmentation() {
        let (_d, u) = index(&[("▁ab", -1.0), ("▁a", -3.0), ("b", -3.0), ("c", -2.0), ("▁abc", -5.0)]);
        // ▁ab + c = -3 beats ▁abc = -5 and ▁a + b + c = -8.
        assert_eq!(u.encode("abc", 16), [0, 4, 7, 2]);
    }

    #[test]
    fn unknown_characters_fuse_into_one_token() {
        let (_d, u) = index(&[("▁", -2.0), ("a", -1.0)]);
        assert_eq!(u.encode("zzza", 16), [0, 4, 3, 5, 2]);
    }

    #[test]
    fn truncation_keeps_the_closing_token() {
        let (_d, u) = index(&[("▁a", -1.0)]);
        assert_eq!(u.encode("a a a a a a", 4), [0, 4, 4, 2]);
    }

    #[test]
    fn a_corrupt_index_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(INDEX_FILE);
        fs::write(&path, b"nope").unwrap();
        assert!(Unigram::open(&path).is_err());
    }

    #[test]
    fn base64_decodes() {
        assert_eq!(base64("aGk=").unwrap(), b"hi");
    }
}
