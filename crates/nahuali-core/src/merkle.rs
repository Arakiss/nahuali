//! Merkle commitment and inclusion proofs over ledger hashes.
//!
//! A Merkle tree summarizes any list of leaf hashes into a single root. An
//! *inclusion proof* then proves one leaf is committed under that root with only
//! O(log n) sibling hashes, so a caller can prove a specific record was in the
//! recorded history without replaying or revealing the rest of the ledger. This
//! is the selective, logarithmic-size verification a linear hash chain cannot
//! give on its own: the chain proves the whole history all-or-nothing, while a
//! Merkle proof answers "is *this* record under the committed root?" cheaply.
//!
//! The construction is domain-separated and length-prefixed -- a leaf hash can
//! never be read as an internal-node hash, and adjacent fields cannot be
//! confused -- and an odd node is promoted unchanged to the next level rather
//! than duplicated, which avoids the duplicate-leaf ambiguity of naive
//! Bitcoin-style trees. It is the same transparency-log primitive Certificate
//! Transparency uses, scoped to a single-owner ledger; it is a verifiable
//! inclusion proof, not a "blockchain".
//!
//! Gated on the `tamper-evidence` feature, alongside the hash chain it commits
//! over.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::event::EventEnvelope;

const LEAF_DOMAIN: &[u8] = b"nahuali.ledger.merkle.leaf.v1";
const NODE_DOMAIN: &[u8] = b"nahuali.ledger.merkle.node.v1";

/// One sibling on the path from a proven leaf up to the Merkle root.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MerkleSibling {
    /// The sibling node's hash (hex).
    pub hash: String,
    /// Whether the sibling sits on the right, so the proven node is the left
    /// child at that level.
    pub on_right: bool,
}

/// An inclusion proof: the sibling path that ties one leaf to a Merkle root.
///
/// It is portable -- a holder can keep it next to a record and later prove, to
/// anyone who knows only the root, that the record was committed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MerkleProof {
    /// Zero-based index of the proven leaf within the committed list.
    pub index: usize,
    /// Total number of leaves committed under the root.
    pub leaf_count: usize,
    /// Sibling hashes from the leaf up to the root, in bottom-up order.
    pub siblings: Vec<MerkleSibling>,
}

/// Compute the Merkle root over `leaves`, or `None` for an empty list.
pub fn merkle_root(leaves: &[String]) -> Option<String> {
    if leaves.is_empty() {
        return None;
    }
    let mut level: Vec<String> = leaves.iter().map(|leaf| leaf_hash(leaf)).collect();
    while level.len() > 1 {
        level = fold_level(&level);
    }
    level.into_iter().next()
}

/// Produce an inclusion proof for the leaf at `index`, or `None` if out of range.
pub fn merkle_proof(leaves: &[String], index: usize) -> Option<MerkleProof> {
    if index >= leaves.len() {
        return None;
    }
    let leaf_count = leaves.len();
    let mut level: Vec<String> = leaves.iter().map(|leaf| leaf_hash(leaf)).collect();
    let mut idx = index;
    let mut siblings = Vec::new();

    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        let mut i = 0;
        let mut new_idx = idx;
        while i < level.len() {
            let parent_pos = next.len();
            if i + 1 < level.len() {
                if idx == i {
                    siblings.push(MerkleSibling {
                        hash: level[i + 1].clone(),
                        on_right: true,
                    });
                    new_idx = parent_pos;
                } else if idx == i + 1 {
                    siblings.push(MerkleSibling {
                        hash: level[i].clone(),
                        on_right: false,
                    });
                    new_idx = parent_pos;
                }
                next.push(node_hash(&level[i], &level[i + 1]));
                i += 2;
            } else {
                // Odd node: promoted unchanged to the next level, no sibling.
                if idx == i {
                    new_idx = parent_pos;
                }
                next.push(level[i].clone());
                i += 1;
            }
        }
        idx = new_idx;
        level = next;
    }

    Some(MerkleProof {
        index,
        leaf_count,
        siblings,
    })
}

/// Verify that `leaf` is committed under `root` using `proof`.
pub fn verify_merkle_proof(leaf: &str, proof: &MerkleProof, root: &str) -> bool {
    let mut computed = leaf_hash(leaf);
    for sibling in &proof.siblings {
        computed = if sibling.on_right {
            node_hash(&computed, &sibling.hash)
        } else {
            node_hash(&sibling.hash, &computed)
        };
    }
    computed == root
}

/// Merkle root over a chained ledger's per-event chain hashes.
pub fn ledger_merkle_root(events: &[EventEnvelope]) -> Option<String> {
    let leaves: Vec<String> = events.iter().map(EventEnvelope::chain_hash).collect();
    merkle_root(&leaves)
}

/// Inclusion proof for the event at `index` within a chained ledger. Verify it
/// with [`verify_merkle_proof`] against `event.chain_hash()` and the ledger root.
pub fn ledger_inclusion_proof(events: &[EventEnvelope], index: usize) -> Option<MerkleProof> {
    let leaves: Vec<String> = events.iter().map(EventEnvelope::chain_hash).collect();
    merkle_proof(&leaves, index)
}

fn fold_level(level: &[String]) -> Vec<String> {
    let mut next = Vec::with_capacity(level.len().div_ceil(2));
    let mut i = 0;
    while i < level.len() {
        if i + 1 < level.len() {
            next.push(node_hash(&level[i], &level[i + 1]));
            i += 2;
        } else {
            next.push(level[i].clone());
            i += 1;
        }
    }
    next
}

fn leaf_hash(data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(LEAF_DOMAIN);
    absorb(&mut hasher, data.as_bytes());
    encode_hex(&hasher.finalize())
}

fn node_hash(left: &str, right: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(NODE_DOMAIN);
    absorb(&mut hasher, left.as_bytes());
    absorb(&mut hasher, right.as_bytes());
    encode_hex(&hasher.finalize())
}

fn absorb(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn encode_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::{
        EventEnvelope, ledger_inclusion_proof, ledger_merkle_root, merkle_proof, merkle_root,
        verify_merkle_proof,
    };
    use crate::event::{EpisodeRecorded, MemoryEvent};

    fn leaves(count: usize) -> Vec<String> {
        (0..count).map(|index| format!("leaf-{index}")).collect()
    }

    #[test]
    fn empty_input_has_no_root_or_proof() {
        assert_eq!(merkle_root(&[]), None);
        assert_eq!(merkle_proof(&[], 0), None);
    }

    #[test]
    fn single_leaf_proof_is_empty_and_verifies() {
        let leaves = leaves(1);
        let root = merkle_root(&leaves).expect("root");
        let proof = merkle_proof(&leaves, 0).expect("proof");

        assert!(proof.siblings.is_empty());
        assert_eq!(proof.leaf_count, 1);
        assert!(verify_merkle_proof(&leaves[0], &proof, &root));
    }

    #[test]
    fn root_is_deterministic() {
        assert_eq!(merkle_root(&leaves(6)), merkle_root(&leaves(6)));
    }

    #[test]
    fn every_leaf_has_a_logarithmic_verifiable_proof() {
        // Odd counts exercise the promote-odd path at several levels.
        for count in [2usize, 3, 4, 5, 6, 7, 8] {
            let leaves = leaves(count);
            let root = merkle_root(&leaves).expect("root");
            let max_depth = count.next_power_of_two().trailing_zeros() as usize;
            for index in 0..count {
                let proof = merkle_proof(&leaves, index).expect("proof");
                assert_eq!(proof.index, index);
                assert_eq!(proof.leaf_count, count);
                assert!(proof.siblings.len() <= max_depth);
                assert!(
                    verify_merkle_proof(&leaves[index], &proof, &root),
                    "leaf {index} of {count} must verify"
                );
            }
        }
    }

    #[test]
    fn proof_rejects_a_tampered_leaf() {
        let leaves = leaves(5);
        let root = merkle_root(&leaves).expect("root");
        let proof = merkle_proof(&leaves, 2).expect("proof");

        assert!(verify_merkle_proof(&leaves[2], &proof, &root));
        assert!(!verify_merkle_proof("tampered value", &proof, &root));
    }

    #[test]
    fn a_proof_does_not_transfer_to_another_leaf() {
        let leaves = leaves(6);
        let root = merkle_root(&leaves).expect("root");
        let proof_for_one = merkle_proof(&leaves, 1).expect("proof");

        assert!(!verify_merkle_proof(&leaves[4], &proof_for_one, &root));
    }

    #[test]
    fn out_of_range_index_has_no_proof() {
        assert_eq!(merkle_proof(&leaves(4), 4), None);
    }

    #[test]
    fn ledger_events_have_verifiable_inclusion_proofs() {
        let mut events: Vec<EventEnvelope> = Vec::new();
        for sequence in 1..=5u64 {
            let prev = events.last().map(EventEnvelope::chain_hash);
            events.push(EventEnvelope::with_chain(
                sequence,
                1000 + sequence,
                MemoryEvent::EpisodeRecorded(EpisodeRecorded {
                    id: format!("episode_{sequence}"),
                    content: format!("event {sequence}"),
                    tags: Vec::new(),
                    mentions: Vec::new(),
                    source_id: None,
                    source_position: None,
                    source_role: None,
                    scope: None,
                }),
                prev.as_deref(),
            ));
        }

        let root = ledger_merkle_root(&events).expect("ledger root");
        for index in 0..events.len() {
            let proof = ledger_inclusion_proof(&events, index).expect("ledger proof");
            assert!(
                verify_merkle_proof(&events[index].chain_hash(), &proof, &root),
                "event {index} must prove inclusion"
            );
        }
    }
}
