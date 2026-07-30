//! Merkle commitment and inclusion proofs over ledger hashes.
//!
//! A Merkle tree summarizes any list of leaf hashes into a single root. An
//! *inclusion proof* then establishes that one leaf is committed under a supplied
//! root with only O(log n) sibling hashes. This gives selective,
//! logarithmic-size verification without replaying or revealing the rest of the
//! ledger. The root is not authenticated by the proof itself; a verifier needs an
//! externally authorized checkpoint before treating membership as third-party
//! evidence.
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
#[serde(deny_unknown_fields)]
pub struct MerkleSibling {
    /// The sibling node's hash (hex).
    pub hash: String,
    /// Whether the sibling sits on the right, so the proven node is the left
    /// child at that level.
    pub on_right: bool,
}

/// An inclusion proof: the sibling path that ties one leaf to a Merkle root.
///
/// It is portable: a holder can keep it next to a record and later establish
/// membership under the supplied root. The proof does not authenticate the root.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MerkleProof {
    /// Zero-based index of the proven leaf within the committed list.
    pub index: usize,
    /// Declared total number of leaves used to validate the proof topology.
    /// `nahuali-merkle-v1` roots do not independently authenticate this
    /// metadata when two sizes yield the same path; pair the proof with a
    /// signed checkpoint before treating the size as authoritative.
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
///
/// This strictly validates index range, path topology, sibling directions and
/// path length. A bare v1 root still does not authenticate size metadata when
/// two declared trees share the same topology; a signed checkpoint supplies
/// that external binding.
pub fn verify_merkle_proof(leaf: &str, proof: &MerkleProof, root: &str) -> bool {
    if proof.leaf_count == 0
        || proof.index >= proof.leaf_count
        || !is_sha256_hex(root)
        || proof
            .siblings
            .iter()
            .any(|sibling| !is_sha256_hex(&sibling.hash))
    {
        return false;
    }

    let mut computed = leaf_hash(leaf);
    let mut index = proof.index;
    let mut level_width = proof.leaf_count;
    let mut siblings = proof.siblings.iter();

    while level_width > 1 {
        let is_right_child = index % 2 == 1;
        let has_right_sibling = !is_right_child && index + 1 < level_width;

        if is_right_child || has_right_sibling {
            let Some(sibling) = siblings.next() else {
                return false;
            };
            let expected_on_right = has_right_sibling;
            if sibling.on_right != expected_on_right {
                return false;
            }
            computed = if expected_on_right {
                node_hash(&computed, &sibling.hash)
            } else {
                node_hash(&sibling.hash, &computed)
            };
        }

        index /= 2;
        level_width = level_width.div_ceil(2);
    }

    siblings.next().is_none() && computed == root
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

/// Maximum number of hashes accepted in a consistency proof over a `u64`
/// sized tree.
pub const MAX_CONSISTENCY_PROOF_HASHES: usize = u64::BITS as usize + 1;

/// Compact evidence that one `nahuali-merkle-v1` tree is an append-only prefix
/// of a later tree.
///
/// The proof shape follows the RFC 9162 consistency algorithm, but its hashes
/// deliberately remain Nahuali's historical domain-separated v1 hashes. It is
/// therefore RFC-shaped, not byte-compatible with Certificate Transparency.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MerkleConsistencyProof {
    /// Leaf count committed by the earlier root.
    pub old_leaf_count: u64,
    /// Leaf count committed by the later root.
    pub new_leaf_count: u64,
    /// Canonical lowercase subtree hashes in verification order.
    pub hashes: Vec<String>,
}

/// Build a compact consistency proof from an earlier prefix to the full tree.
///
/// Returns `None` for an empty prefix, a prefix larger than the tree, or a tree
/// whose size cannot be represented as `u64`. An unchanged non-empty tree has
/// a valid empty proof.
pub fn merkle_consistency_proof(
    leaves: &[String],
    old_leaf_count: usize,
) -> Option<MerkleConsistencyProof> {
    if old_leaf_count == 0 || old_leaf_count > leaves.len() {
        return None;
    }
    let old_size = u64::try_from(old_leaf_count).ok()?;
    let new_size = u64::try_from(leaves.len()).ok()?;
    let mut hashes = Vec::new();
    consistency_subproof(leaves, old_leaf_count, true, &mut hashes)?;
    if hashes.len() > MAX_CONSISTENCY_PROOF_HASHES {
        return None;
    }
    Some(MerkleConsistencyProof {
        old_leaf_count: old_size,
        new_leaf_count: new_size,
        hashes,
    })
}

/// Verify a compact append-only consistency proof from `old_root` to
/// `new_root`, using the two caller-supplied roots, their sizes, and the proof
/// hashes. Callers must authorize the roots separately before relying on them.
///
/// Sizes are taken from the proof so callers must bind the proof to signed
/// checkpoints before treating either size as authoritative.
pub fn verify_merkle_consistency_proof(
    old_root: &str,
    new_root: &str,
    proof: &MerkleConsistencyProof,
) -> bool {
    if proof.old_leaf_count == 0
        || proof.old_leaf_count > proof.new_leaf_count
        || !is_canonical_sha256_hex(old_root)
        || !is_canonical_sha256_hex(new_root)
        || proof.hashes.len() > MAX_CONSISTENCY_PROOF_HASHES
        || proof
            .hashes
            .iter()
            .any(|hash| !is_canonical_sha256_hex(hash))
    {
        return false;
    }

    if proof.old_leaf_count == proof.new_leaf_count {
        return proof.hashes.is_empty() && old_root == new_root;
    }

    let mut old_index = proof.old_leaf_count - 1;
    let mut new_index = proof.new_leaf_count - 1;
    while old_index & 1 == 1 {
        old_index >>= 1;
        new_index >>= 1;
    }

    let (mut old_hash, mut new_hash, mut proof_index) = if old_index == 0 {
        (old_root.to_string(), old_root.to_string(), 0usize)
    } else {
        let Some(first) = proof.hashes.first() else {
            return false;
        };
        (first.clone(), first.clone(), 1usize)
    };

    while let Some(hash) = proof.hashes.get(proof_index) {
        if new_index == 0 {
            return false;
        }

        if old_index & 1 == 1 || old_index == new_index {
            old_hash = node_hash(hash, &old_hash);
            new_hash = node_hash(hash, &new_hash);
            while old_index != 0 && old_index & 1 == 0 {
                old_index >>= 1;
                new_index >>= 1;
            }
        } else {
            new_hash = node_hash(&new_hash, hash);
        }

        old_index >>= 1;
        new_index >>= 1;
        proof_index += 1;
    }

    new_index == 0 && old_hash == old_root && new_hash == new_root
}

/// Compact consistency proof between an earlier event count and the current
/// ledger root, using each event's chain hash as the Merkle leaf.
pub fn ledger_consistency_proof(
    events: &[EventEnvelope],
    old_event_count: usize,
) -> Option<MerkleConsistencyProof> {
    let leaves: Vec<String> = events.iter().map(EventEnvelope::chain_hash).collect();
    merkle_consistency_proof(&leaves, old_event_count)
}

/// A verdict for whether one Merkle commitment is an append-only extension of an
/// earlier one: the earlier leaves are an unchanged prefix of the later ones.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsistencyVerdict {
    /// Leaf count of the earlier commitment.
    pub old_leaf_count: usize,
    /// Leaf count of the later commitment.
    pub new_leaf_count: usize,
    /// The earlier root recomputed from the later leaves' prefix, when checkable.
    pub recomputed_old_root: Option<String>,
    /// Whether the earlier commitment is an unchanged prefix of the later one,
    /// relative to the caller-supplied earlier root.
    pub append_only: bool,
}

/// Verify that the commitment over `new_leaves` is an append-only extension of an
/// earlier commitment `old_root` taken over its first `old_leaf_count` leaves:
/// the earlier prefix is unchanged and only new leaves were appended.
///
/// This is the leaf-backed check -- the caller holds the later ledger's leaves
/// and the earlier root, which is the local audit case (you have both states).
/// A match establishes that the supplied earlier root commits to the later
/// leaves' prefix; it does not authenticate that root. Use
/// [`verify_merkle_consistency_proof`] when the verifier holds only signed roots
/// and a compact proof rather than the complete later ledger.
pub fn verify_append_only(
    old_root: &str,
    old_leaf_count: usize,
    new_leaves: &[String],
) -> ConsistencyVerdict {
    let new_leaf_count = new_leaves.len();
    if old_leaf_count == 0 || old_leaf_count > new_leaf_count {
        return ConsistencyVerdict {
            old_leaf_count,
            new_leaf_count,
            recomputed_old_root: None,
            append_only: false,
        };
    }
    let recomputed_old_root = merkle_root(&new_leaves[..old_leaf_count]);
    let append_only = recomputed_old_root.as_deref() == Some(old_root);
    ConsistencyVerdict {
        old_leaf_count,
        new_leaf_count,
        recomputed_old_root,
        append_only,
    }
}

/// Append-only verdict between an earlier ledger root taken over `old_leaf_count`
/// events and the current `new_events`, using per-event chain hashes as leaves.
pub fn ledger_append_only(
    old_root: &str,
    old_leaf_count: usize,
    new_events: &[EventEnvelope],
) -> ConsistencyVerdict {
    let leaves: Vec<String> = new_events.iter().map(EventEnvelope::chain_hash).collect();
    verify_append_only(old_root, old_leaf_count, &leaves)
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

fn consistency_subproof(
    leaves: &[String],
    old_leaf_count: usize,
    complete_subtree: bool,
    hashes: &mut Vec<String>,
) -> Option<()> {
    if old_leaf_count == leaves.len() {
        if !complete_subtree {
            hashes.push(merkle_root(leaves)?);
        }
        return Some(());
    }

    let split = largest_power_of_two_less_than(leaves.len())?;
    if old_leaf_count <= split {
        consistency_subproof(&leaves[..split], old_leaf_count, complete_subtree, hashes)?;
        hashes.push(merkle_root(&leaves[split..])?);
    } else {
        consistency_subproof(&leaves[split..], old_leaf_count - split, false, hashes)?;
        hashes.push(merkle_root(&leaves[..split])?);
    }
    Some(())
}

fn largest_power_of_two_less_than(value: usize) -> Option<usize> {
    if value <= 1 {
        return None;
    }
    let exponent = usize::BITS - (value - 1).leading_zeros() - 1;
    Some(1usize << exponent)
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

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_canonical_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::{
        EventEnvelope, MAX_CONSISTENCY_PROOF_HASHES, ledger_append_only, ledger_consistency_proof,
        ledger_inclusion_proof, ledger_merkle_root, merkle_consistency_proof, merkle_proof,
        merkle_root, verify_append_only, verify_merkle_consistency_proof, verify_merkle_proof,
    };
    use crate::event::{EpisodeRecorded, MemoryEvent};

    fn leaves(count: usize) -> Vec<String> {
        (0..count).map(|index| format!("leaf-{index}")).collect()
    }

    fn consistency_topology_hash_count(
        old_leaf_count: u64,
        new_leaf_count: u64,
        complete_subtree: bool,
    ) -> usize {
        assert!(old_leaf_count > 0);
        assert!(old_leaf_count <= new_leaf_count);
        if old_leaf_count == new_leaf_count {
            return if complete_subtree { 0 } else { 1 };
        }

        let exponent = u64::BITS - (new_leaf_count - 1).leading_zeros() - 1;
        let split = 1u64 << exponent;
        if old_leaf_count <= split {
            consistency_topology_hash_count(old_leaf_count, split, complete_subtree) + 1
        } else {
            consistency_topology_hash_count(old_leaf_count - split, new_leaf_count - split, false)
                + 1
        }
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
    fn every_non_empty_prefix_has_a_compact_consistency_proof() {
        for new_count in 1usize..=128 {
            let new_leaves = leaves(new_count);
            let new_root = merkle_root(&new_leaves).expect("new root");
            for old_count in 1usize..=new_count {
                let old_root = merkle_root(&new_leaves[..old_count]).expect("old root");
                let proof = merkle_consistency_proof(&new_leaves, old_count)
                    .expect("valid prefix has a consistency proof");

                assert_eq!(proof.old_leaf_count, old_count as u64);
                assert_eq!(proof.new_leaf_count, new_count as u64);
                assert!(proof.hashes.len() <= MAX_CONSISTENCY_PROOF_HASHES);
                assert!(
                    verify_merkle_consistency_proof(&old_root, &new_root, &proof),
                    "prefix {old_count} of {new_count} must verify"
                );
            }
        }
    }

    #[test]
    fn consistency_proof_bound_covers_the_full_u64_topology() {
        let largest_tree = (1u64 << 63) + 1;
        let required = consistency_topology_hash_count(3, largest_tree, true);

        assert_eq!(required, 65);
        assert_eq!(MAX_CONSISTENCY_PROOF_HASHES, required);
    }

    #[test]
    fn unchanged_tree_has_an_empty_consistency_proof() {
        let leaves = leaves(9);
        let root = merkle_root(&leaves).expect("root");
        let proof = merkle_consistency_proof(&leaves, leaves.len()).expect("proof");

        assert!(proof.hashes.is_empty());
        assert!(verify_merkle_consistency_proof(&root, &root, &proof));
    }

    #[test]
    fn consistency_proof_rejects_rewrite_truncation_fork_and_tampering() {
        let original = leaves(11);
        let old_root = merkle_root(&original[..5]).expect("old root");
        let new_root = merkle_root(&original).expect("new root");
        let proof = merkle_consistency_proof(&original, 5).expect("proof");
        assert!(verify_merkle_consistency_proof(
            &old_root, &new_root, &proof
        ));

        let mut rewritten = original.clone();
        rewritten[2] = "rewritten-history".to_string();
        let rewritten_root = merkle_root(&rewritten).expect("rewritten root");
        assert!(!verify_merkle_consistency_proof(
            &old_root,
            &rewritten_root,
            &proof
        ));

        let truncated_root = merkle_root(&original[..8]).expect("truncated root");
        let mut truncated = proof.clone();
        truncated.new_leaf_count = 8;
        assert!(!verify_merkle_consistency_proof(
            &old_root,
            &truncated_root,
            &truncated
        ));

        let mut fork = original.clone();
        fork[9] = "forked-suffix".to_string();
        let fork_root = merkle_root(&fork).expect("fork root");
        assert!(!verify_merkle_consistency_proof(
            &old_root, &fork_root, &proof
        ));

        for index in 0..proof.hashes.len() {
            let mut tampered = proof.clone();
            tampered.hashes[index].replace_range(0..2, "ff");
            assert!(!verify_merkle_consistency_proof(
                &old_root, &new_root, &tampered
            ));
        }

        let mut missing = proof.clone();
        missing.hashes.pop();
        assert!(!verify_merkle_consistency_proof(
            &old_root, &new_root, &missing
        ));

        let mut extra = proof.clone();
        extra.hashes.push("00".repeat(32));
        assert!(!verify_merkle_consistency_proof(
            &old_root, &new_root, &extra
        ));
    }

    #[test]
    fn consistency_proof_rejects_invalid_sizes_and_noncanonical_hashes() {
        let leaves = leaves(4);
        let old_root = merkle_root(&leaves[..2]).expect("old root");
        let new_root = merkle_root(&leaves).expect("new root");
        let proof = merkle_consistency_proof(&leaves, 2).expect("proof");

        assert!(merkle_consistency_proof(&leaves, 0).is_none());
        assert!(merkle_consistency_proof(&leaves, 5).is_none());

        let mut zero = proof.clone();
        zero.old_leaf_count = 0;
        assert!(!verify_merkle_consistency_proof(
            &old_root, &new_root, &zero
        ));

        let mut reversed = proof.clone();
        reversed.old_leaf_count = 5;
        assert!(!verify_merkle_consistency_proof(
            &old_root, &new_root, &reversed
        ));

        assert!(!verify_merkle_consistency_proof(
            &old_root.to_uppercase(),
            &new_root,
            &proof
        ));
    }

    #[test]
    fn consistency_proof_has_a_stable_golden_vector() {
        let leaves = leaves(7);
        let proof = merkle_consistency_proof(&leaves, 3).expect("proof");
        assert_eq!(proof.old_leaf_count, 3);
        assert_eq!(proof.new_leaf_count, 7);
        assert_eq!(
            proof.hashes,
            [
                "aadb91271013ee1793fe30a2caf68638ad278783514515f4e345e3ae398d7da9",
                "5de32e6409d77202b85588f0420ef7a493e15c378b67f2f27b2f03ba02bd58c1",
                "6c5a1369fedb19c5d7355771eb0740714494e97672e3ff5fa57c3ab60b4a0370",
                "909c1ad504f79c381a8bdf02b418d041c41a064c7b77126cd3abd2f4dbf4afcc",
            ]
            .map(str::to_string)
        );
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
    fn proof_rejects_a_tampered_index_or_tree_shape() {
        let leaves = leaves(5);
        let root = merkle_root(&leaves).expect("root");
        let proof = merkle_proof(&leaves, 4).expect("proof");

        let mut wrong_index = proof.clone();
        wrong_index.index = 3;
        assert!(!verify_merkle_proof(&leaves[4], &wrong_index, &root));

        let mut impossible_size = proof.clone();
        impossible_size.leaf_count = 4;
        assert!(!verify_merkle_proof(&leaves[4], &impossible_size, &root));

        let mut different_shape = proof.clone();
        different_shape.leaf_count = 6;
        assert!(!verify_merkle_proof(&leaves[4], &different_shape, &root));
    }

    #[test]
    fn a_bare_v1_root_does_not_authenticate_ambiguous_size_metadata() {
        let leaves = leaves(3);
        let root = merkle_root(&leaves).expect("root");
        let mut proof = merkle_proof(&leaves, 2).expect("proof");

        // The right-most leaf in a three-leaf promoted-odd tree has the same
        // path shape as index one in a two-leaf tree. The root authenticates
        // the leaf and path, not this standalone metadata; SignedCheckpoint v2
        // binds the authoritative size.
        proof.index = 1;
        proof.leaf_count = 2;
        assert!(verify_merkle_proof(&leaves[2], &proof, &root));
    }

    #[test]
    fn proof_rejects_a_tampered_path_shape_or_hash() {
        let leaves = leaves(7);
        let root = merkle_root(&leaves).expect("root");
        let proof = merkle_proof(&leaves, 2).expect("proof");

        let mut wrong_direction = proof.clone();
        wrong_direction.siblings[0].on_right = !wrong_direction.siblings[0].on_right;
        assert!(!verify_merkle_proof(&leaves[2], &wrong_direction, &root));

        let mut missing_node = proof.clone();
        missing_node.siblings.pop();
        assert!(!verify_merkle_proof(&leaves[2], &missing_node, &root));

        let mut extra_node = proof.clone();
        extra_node.siblings.push(proof.siblings[0].clone());
        assert!(!verify_merkle_proof(&leaves[2], &extra_node, &root));

        let mut malformed_hash = proof.clone();
        malformed_hash.siblings[0].hash = "not-a-sha256-hash".to_string();
        assert!(!verify_merkle_proof(&leaves[2], &malformed_hash, &root));

        assert!(!verify_merkle_proof(&leaves[2], &proof, "not-a-root"));
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

    #[test]
    fn append_only_holds_when_history_only_grows() {
        let old = leaves(4);
        let old_root = merkle_root(&old).expect("old root");
        let mut new = old.clone();
        new.push("leaf-4".to_string());
        new.push("leaf-5".to_string());

        let verdict = verify_append_only(&old_root, old.len(), &new);

        assert!(verdict.append_only);
        assert_eq!(verdict.old_leaf_count, 4);
        assert_eq!(verdict.new_leaf_count, 6);
        assert_eq!(
            verdict.recomputed_old_root.as_deref(),
            Some(old_root.as_str())
        );
    }

    #[test]
    fn append_only_holds_for_an_unchanged_ledger() {
        let leaves = leaves(5);
        let root = merkle_root(&leaves).expect("root");

        assert!(verify_append_only(&root, leaves.len(), &leaves).append_only);
    }

    #[test]
    fn append_only_fails_when_a_prefix_leaf_was_rewritten() {
        let old = leaves(4);
        let old_root = merkle_root(&old).expect("old root");
        let mut new = old.clone();
        new.push("leaf-4".to_string());
        // Rewrite a historical leaf instead of only appending.
        new[1] = "rewritten".to_string();

        assert!(!verify_append_only(&old_root, old.len(), &new).append_only);
    }

    #[test]
    fn append_only_fails_when_the_ledger_shrank() {
        let old = leaves(6);
        let old_root = merkle_root(&old).expect("old root");
        let new = leaves(4);

        let verdict = verify_append_only(&old_root, old.len(), &new);

        assert!(!verdict.append_only);
        assert!(verdict.recomputed_old_root.is_none());
    }

    #[test]
    fn ledger_append_only_proves_an_appended_ledger() {
        let mut events: Vec<EventEnvelope> = Vec::new();
        for sequence in 1..=4u64 {
            let prev = events.last().map(EventEnvelope::chain_hash);
            events.push(EventEnvelope::with_chain(
                sequence,
                1000 + sequence,
                episode_event(sequence),
                prev.as_deref(),
            ));
        }
        let old_root = ledger_merkle_root(&events).expect("old ledger root");
        let old_count = events.len();

        for sequence in 5..=7u64 {
            let prev = events.last().map(EventEnvelope::chain_hash);
            events.push(EventEnvelope::with_chain(
                sequence,
                1000 + sequence,
                episode_event(sequence),
                prev.as_deref(),
            ));
        }

        assert!(ledger_append_only(&old_root, old_count, &events).append_only);
        let new_root = ledger_merkle_root(&events).expect("new ledger root");
        let proof = ledger_consistency_proof(&events, old_count).expect("ledger consistency proof");
        assert!(verify_merkle_consistency_proof(
            &old_root, &new_root, &proof
        ));
    }

    fn episode_event(sequence: u64) -> MemoryEvent {
        MemoryEvent::EpisodeRecorded(EpisodeRecorded {
            id: format!("episode_{sequence}"),
            content: format!("event {sequence}"),
            tags: Vec::new(),
            mentions: Vec::new(),
            source_id: None,
            source_position: None,
            source_role: None,
            scope: None,
        })
    }
}
