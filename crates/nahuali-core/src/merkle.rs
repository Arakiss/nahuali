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
    /// Whether the earlier commitment is an unchanged prefix of the later one:
    /// the recorded history grew by appending only, with nothing rewritten.
    pub append_only: bool,
}

/// Verify that the commitment over `new_leaves` is an append-only extension of an
/// earlier commitment `old_root` taken over its first `old_leaf_count` leaves:
/// the earlier prefix is unchanged and only new leaves were appended.
///
/// This is the leaf-backed check -- the caller holds the later ledger's leaves
/// and the earlier root, which is the local audit case (you have both states).
/// It proves the earlier history was not rewritten. A *succinct* consistency
/// proof, which a remote witness could check without holding the full prefix, is
/// deliberately left as future work rather than shipped in a rushed, hard-to-
/// audit form.
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

#[cfg(test)]
mod tests {
    use super::{
        EventEnvelope, ledger_append_only, ledger_inclusion_proof, ledger_merkle_root,
        merkle_proof, merkle_root, verify_append_only, verify_merkle_proof,
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
