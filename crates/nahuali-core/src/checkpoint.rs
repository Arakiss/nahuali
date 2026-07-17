//! Versioned signed checkpoints for the authoritative record ledger.
//!
//! A checkpoint binds the ledger lineage, tree algorithm, tree size, Merkle
//! root, chain tip, and creation time into fixed canonical bytes. Signatures are
//! authorized only through an operator-held policy; a public key carried by the
//! checkpoint never establishes trust by itself.

use std::collections::HashSet;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::attestation::{AttestationKeyStatus, decode_fixed, encode_hex};
use crate::audit::{LedgerAuditOptions, LedgerChainStatus, audit_events};
use crate::merkle::ledger_merkle_root;
use crate::store::MemoryEngine;
use crate::{EventEnvelope, NahualiError, Result};

/// Signed ledger checkpoint document version.
pub const LEDGER_CHECKPOINT_VERSION: u32 = 2;

/// Operator trust-policy document version for checkpoint v2.
pub const CHECKPOINT_TRUST_POLICY_VERSION: u32 = 1;

/// Maximum unique signatures or policy keys accepted by checkpoint v2.
pub const MAX_CHECKPOINT_SIGNATURES: usize = 32;

/// Default verifier tolerance for checkpoint times slightly ahead of its local
/// clock (five minutes).
pub const DEFAULT_CHECKPOINT_FUTURE_SKEW_MS: u64 = 5 * 60 * 1_000;

const CHECKPOINT_DOMAIN_V2: &[u8] = b"nahuali.ledger.checkpoint.v2\0";
const LEDGER_ID_DOMAIN_V1: &[u8] = b"nahuali.ledger.identity.v1\0";
const MAX_ORIGIN_BYTES: usize = 255;
const MAX_KEY_ID_BYTES: usize = 128;

/// Merkle tree algorithm committed by a checkpoint.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum CheckpointTreeAlgorithm {
    /// Historical Nahuali v1 tree over textual event chain hashes.
    #[serde(rename = "nahuali-merkle-v1")]
    NahualiMerkleV1,
}

impl CheckpointTreeAlgorithm {
    const fn tag(self) -> u8 {
        match self {
            Self::NahualiMerkleV1 => 1,
        }
    }
}

/// Hash algorithm used by the checkpoint tree and ledger identity.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointHashAlgorithm {
    /// SHA-256 with a 32-byte output.
    Sha256,
}

impl CheckpointHashAlgorithm {
    const fn tag(self) -> u8 {
        match self {
            Self::Sha256 => 1,
        }
    }
}

/// Signature algorithm used by a checkpoint signature.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointSignatureAlgorithm {
    /// Ed25519 detached signature.
    Ed25519,
}

/// Whether verification must match the ledger's current state or may anchor a
/// fully verified historical prefix.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointMatchMode {
    /// Require the checkpoint size to equal the live ledger size.
    Current,
    /// Verify the checkpointed prefix. Later events are reported separately and
    /// are not covered by this checkpoint verdict.
    Historical,
}

/// Explicit verification context for a signed checkpoint.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CheckpointVerificationOptionsV2 {
    /// Whether the checkpoint must match current history or an older prefix.
    pub match_mode: CheckpointMatchMode,
    /// Verifier-observed Unix epoch time in milliseconds.
    pub verification_time_ms: u64,
    /// Maximum signer-clock lead accepted over `verification_time_ms`.
    pub max_future_skew_ms: u64,
}

impl CheckpointVerificationOptionsV2 {
    /// Verify that a checkpoint represents the current ledger state.
    pub const fn current(verification_time_ms: u64) -> Self {
        Self {
            match_mode: CheckpointMatchMode::Current,
            verification_time_ms,
            max_future_skew_ms: DEFAULT_CHECKPOINT_FUTURE_SKEW_MS,
        }
    }

    /// Verify that a checkpoint authenticates a historical ledger prefix.
    pub const fn historical(verification_time_ms: u64) -> Self {
        Self {
            match_mode: CheckpointMatchMode::Historical,
            verification_time_ms,
            max_future_skew_ms: DEFAULT_CHECKPOINT_FUTURE_SKEW_MS,
        }
    }
}

/// Canonical checkpoint data whose fixed binary encoding is signed.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LedgerCheckpointV2 {
    /// Document format version; always [`LEDGER_CHECKPOINT_VERSION`].
    pub version: u32,
    /// Operator-selected stable name for this ledger origin.
    pub origin: String,
    /// SHA-256 lineage identifier derived from the genesis chain hash.
    pub ledger_id: String,
    /// Merkle construction used for `root_hash`.
    pub tree_algorithm: CheckpointTreeAlgorithm,
    /// Hash function used by the tree and identity.
    pub hash_algorithm: CheckpointHashAlgorithm,
    /// Number of ledger events committed by this checkpoint.
    pub tree_size: u64,
    /// Merkle root at `tree_size`, encoded as canonical lowercase hex.
    pub root_hash: String,
    /// Event chain hash at `tree_size`, encoded as canonical lowercase hex.
    pub chain_tip: String,
    /// Signer-asserted signing time in Unix epoch milliseconds. This is checked
    /// against a verifier-provided clock and is not an independent timestamping
    /// authority or a freshness proof.
    pub generated_at_ms: u64,
}

/// One detached signature over a [`LedgerCheckpointV2`].
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CheckpointSignatureV2 {
    /// Signature algorithm; currently Ed25519.
    pub algorithm: CheckpointSignatureAlgorithm,
    /// Stable operator key identifier matched against the external policy.
    pub key_id: String,
    /// Detached Ed25519 signature, 64 bytes as canonical lowercase hex.
    pub signature: String,
}

/// One strict verifier key in an operator-managed checkpoint policy.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CheckpointPolicyKey {
    /// Required stable key identifier referenced by checkpoint signatures.
    pub key_id: String,
    /// Ed25519 public key, 32 bytes as canonical lowercase hex.
    pub public_key: String,
    /// Whether signatures from this key may count toward the threshold.
    pub status: AttestationKeyStatus,
}

/// Portable v2 checkpoint plus one or more detached signatures.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SignedLedgerCheckpointV2 {
    /// Canonical checkpoint data shared by every signature.
    pub checkpoint: LedgerCheckpointV2,
    /// Unique signer entries. Threshold policies count unique public keys.
    pub signatures: Vec<CheckpointSignatureV2>,
}

/// Operator-held authorization policy for a v2 checkpoint.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CheckpointTrustPolicyV2 {
    /// Policy document version; always [`CHECKPOINT_TRUST_POLICY_VERSION`].
    pub version: u32,
    /// Exact origin this policy authorizes.
    pub expected_origin: String,
    /// Exact ledger lineage this policy authorizes.
    pub expected_ledger_id: String,
    /// Minimum number of distinct active signatures required.
    pub minimum_signatures: u32,
    /// Operator-managed active and revoked signing keys.
    pub keys: Vec<CheckpointPolicyKey>,
}

/// Per-signature evidence returned by checkpoint verification.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CheckpointSignatureEvaluationV2 {
    /// Key identifier carried by the signature.
    pub key_id: String,
    /// Whether the key identifier exists in the external policy.
    pub key_known: bool,
    /// Whether the detached signature verifies over the canonical checkpoint.
    pub signature_valid: bool,
    /// Whether the exact key id and public key are active in the policy.
    pub key_authorized: bool,
    /// Whether the public key is explicitly revoked in the policy.
    pub key_revoked: bool,
    /// Whether this signature can count toward the threshold.
    pub accepted: bool,
}

/// Fail-closed verification of a v2 checkpoint against ledger history and an
/// external operator policy.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CheckpointVerificationV2 {
    /// Match policy applied while comparing the checkpoint to live history.
    pub match_mode: CheckpointMatchMode,
    /// Whether the checkpoint origin matches the policy.
    pub origin_matches_policy: bool,
    /// Whether the checkpoint id matches both policy and live ledger lineage.
    pub ledger_id_matches: bool,
    /// Number of events committed by the checkpoint.
    pub checkpoint_tree_size: u64,
    /// Number of events currently present in the live ledger.
    pub current_tree_size: u64,
    /// Whether `tree_size` names a non-empty prefix present in the live ledger.
    pub tree_size_in_range: bool,
    /// Whether `tree_size` equals the live ledger length. Historical mode does
    /// not require this field to be true.
    pub current_size_matches: bool,
    /// Number of later events not covered by this checkpoint.
    pub appended_event_count: u64,
    /// Whether checksum, sequence, and hash-chain verification pass for that prefix.
    pub ledger_prefix_verified: bool,
    /// Whether the complete live ledger currently passes integrity verification.
    pub current_ledger_verified: bool,
    /// Whether the committed Merkle root matches the selected ledger prefix.
    pub root_hash_matches: bool,
    /// Whether the committed chain tip matches the selected ledger prefix.
    pub chain_tip_matches: bool,
    /// Whether the checkpoint time is no earlier than its latest event.
    pub timestamp_valid: bool,
    /// Whether the signer-asserted time is within the verifier's allowed future
    /// clock skew.
    pub timestamp_not_in_future: bool,
    /// Verifier time used for the timestamp check.
    pub verification_time_ms: u64,
    /// Allowed signer-clock lead over verifier time.
    pub max_future_skew_ms: u64,
    /// Number of distinct signatures accepted under the policy.
    pub accepted_signature_count: u64,
    /// Minimum distinct active signatures required by the policy.
    pub minimum_signature_count: u32,
    /// Evaluation of every supplied signature.
    pub signatures: Vec<CheckpointSignatureEvaluationV2>,
    /// Composite verdict under [`Self::match_mode`]. Unknown or revoked extra
    /// signatures never count toward the threshold; a malformed signature for
    /// an active policy key rejects the document.
    pub trusted: bool,
    /// Stable human-readable reasons for every failed gate.
    pub reasons: Vec<String>,
}

impl MemoryEngine {
    /// Build an operator trust policy for this ledger from explicitly supplied
    /// verifier keys. The returned policy must be protected independently of
    /// both the ledger and its checkpoint receipts.
    pub fn create_checkpoint_policy_v2(
        &self,
        origin: &str,
        minimum_signatures: u32,
        keys: Vec<CheckpointPolicyKey>,
    ) -> Result<CheckpointTrustPolicyV2> {
        let checkpoint = self.create_checkpoint_v2(origin, max_event_timestamp(self.events())?)?;
        CheckpointTrustPolicyV2::new(
            checkpoint.origin,
            checkpoint.ledger_id,
            minimum_signatures,
            keys,
        )
    }

    /// Build a v2 checkpoint over the current complete, fully verified ledger.
    ///
    /// Empty, legacy, corrupt, or discontinuous histories are rejected. The
    /// supplied creation time must not predate the latest committed event.
    pub fn create_checkpoint_v2(
        &self,
        origin: &str,
        generated_at_ms: u64,
    ) -> Result<LedgerCheckpointV2> {
        validate_origin(origin)?;
        let audit = self.audit_ledger(&LedgerAuditOptions::default());
        if !audit.integrity.verified || audit.integrity.chain_status != LedgerChainStatus::Verified
        {
            return Err(checkpoint_error(format!(
                "checkpoint v2 requires a non-empty fully verified ledger, found {}",
                audit.integrity.chain_status.as_str()
            )));
        }

        let events = self.events();
        let latest_timestamp = max_event_timestamp(events)?;
        if generated_at_ms < latest_timestamp {
            return Err(checkpoint_error(format!(
                "checkpoint timestamp {generated_at_ms} predates latest event timestamp {}",
                latest_timestamp
            )));
        }

        let tree_size = u64::try_from(events.len()).map_err(|_| {
            checkpoint_error("ledger contains more events than checkpoint v2 supports")
        })?;
        let root_hash = ledger_merkle_root(events)
            .ok_or_else(|| checkpoint_error("verified ledger did not produce a Merkle root"))?;
        let chain_tip = events
            .last()
            .map(EventEnvelope::chain_hash)
            .ok_or_else(|| checkpoint_error("verified ledger did not produce a chain tip"))?;
        let ledger_id = derive_ledger_id(events)?
            .ok_or_else(|| checkpoint_error("empty ledger has no lineage identifier"))?;

        Ok(LedgerCheckpointV2 {
            version: LEDGER_CHECKPOINT_VERSION,
            origin: origin.to_string(),
            ledger_id,
            tree_algorithm: CheckpointTreeAlgorithm::NahualiMerkleV1,
            hash_algorithm: CheckpointHashAlgorithm::Sha256,
            tree_size,
            root_hash,
            chain_tip,
            generated_at_ms,
        })
    }

    /// Verify a signed checkpoint against the matching ledger prefix and an
    /// operator-held threshold policy.
    pub fn verify_checkpoint_v2(
        &self,
        signed: &SignedLedgerCheckpointV2,
        policy: &CheckpointTrustPolicyV2,
        options: CheckpointVerificationOptionsV2,
    ) -> Result<CheckpointVerificationV2> {
        validate_signed_document(signed)?;
        validate_policy(policy)?;

        let checkpoint = &signed.checkpoint;
        let events = self.events();
        let current_tree_size = u64::try_from(events.len()).map_err(|_| {
            checkpoint_error("live ledger contains more events than checkpoint v2 supports")
        })?;
        let prefix_len = usize::try_from(checkpoint.tree_size).ok();
        let prefix = prefix_len
            .filter(|length| *length > 0 && *length <= events.len())
            .map(|length| &events[..length]);
        let tree_size_in_range = prefix.is_some();
        let current_size_matches = prefix_len == Some(events.len());
        let appended_event_count = current_tree_size.saturating_sub(checkpoint.tree_size);
        let ledger_prefix_verified = prefix.is_some_and(|prefix| {
            let audit = audit_events(prefix, &LedgerAuditOptions::default());
            audit.integrity.verified && audit.integrity.chain_status == LedgerChainStatus::Verified
        });
        let current_audit = audit_events(events, &LedgerAuditOptions::default());
        let current_ledger_verified = current_audit.integrity.verified
            && current_audit.integrity.chain_status == LedgerChainStatus::Verified;

        let current_ledger_id = derive_ledger_id(events)?;
        let origin_matches_policy = checkpoint.origin == policy.expected_origin;
        let ledger_id_matches = current_ledger_id.as_deref() == Some(&checkpoint.ledger_id)
            && checkpoint.ledger_id == policy.expected_ledger_id;
        let root_hash_matches = prefix
            .and_then(ledger_merkle_root)
            .is_some_and(|root| root == checkpoint.root_hash);
        let chain_tip_matches = prefix
            .and_then(|prefix| prefix.last())
            .map(EventEnvelope::chain_hash)
            .is_some_and(|tip| tip == checkpoint.chain_tip);
        let timestamp_valid = prefix
            .and_then(|prefix| prefix.iter().map(|event| event.timestamp_ms).max())
            .is_some_and(|timestamp| checkpoint.generated_at_ms >= timestamp);
        let timestamp_not_in_future = checkpoint.generated_at_ms
            <= options
                .verification_time_ms
                .saturating_add(options.max_future_skew_ms);

        let message = checkpoint_signing_message_v2(checkpoint)?;
        let signatures = signed
            .signatures
            .iter()
            .map(|signature| evaluate_signature(signature, &message, policy))
            .collect::<Vec<_>>();
        let accepted_signature_count = signatures
            .iter()
            .filter(|evaluation| evaluation.accepted)
            .count() as u64;
        let active_signature_invalid = signatures.iter().any(|evaluation| {
            evaluation.key_authorized && !evaluation.key_revoked && !evaluation.signature_valid
        });
        let threshold_met = accepted_signature_count >= u64::from(policy.minimum_signatures);
        let requested_size_matches =
            options.match_mode == CheckpointMatchMode::Historical || current_size_matches;
        let requested_ledger_verified =
            options.match_mode == CheckpointMatchMode::Historical || current_ledger_verified;

        let mut reasons = Vec::new();
        if !origin_matches_policy {
            reasons.push("checkpoint origin does not match the operator policy".to_string());
        }
        if !ledger_id_matches {
            reasons.push(
                "checkpoint ledger id does not match both policy and live ledger lineage"
                    .to_string(),
            );
        }
        if !tree_size_in_range {
            reasons
                .push("checkpoint tree size is not a present non-empty ledger prefix".to_string());
        }
        if tree_size_in_range && !requested_size_matches {
            reasons.push(
                "checkpoint is historical but verification requires the current ledger size"
                    .to_string(),
            );
        }
        if !requested_ledger_verified {
            reasons.push(
                "current ledger failed integrity verification required by current mode".to_string(),
            );
        }
        if tree_size_in_range && !ledger_prefix_verified {
            reasons.push("checkpoint ledger prefix failed integrity verification".to_string());
        }
        if !root_hash_matches {
            reasons.push("checkpoint Merkle root does not match ledger history".to_string());
        }
        if !chain_tip_matches {
            reasons.push("checkpoint chain tip does not match ledger history".to_string());
        }
        if !timestamp_valid {
            reasons.push("checkpoint timestamp predates its latest ledger event".to_string());
        }
        if !timestamp_not_in_future {
            reasons.push(format!(
                "checkpoint timestamp exceeds verifier time by more than {} ms",
                options.max_future_skew_ms
            ));
        }
        for evaluation in &signatures {
            if evaluation.key_authorized && !evaluation.signature_valid {
                reasons.push(format!(
                    "checkpoint signature '{}' for an active policy key is cryptographically invalid",
                    evaluation.key_id
                ));
            } else if evaluation.key_revoked {
                reasons.push(format!(
                    "checkpoint signature '{}' uses a revoked key and was ignored",
                    evaluation.key_id
                ));
            } else if !evaluation.key_known {
                reasons.push(format!(
                    "checkpoint signature '{}' is unknown to the policy and was ignored",
                    evaluation.key_id
                ));
            }
        }
        if !threshold_met {
            reasons.push(format!(
                "checkpoint has {accepted_signature_count} accepted signature(s), policy requires {}",
                policy.minimum_signatures
            ));
        }

        let trusted = origin_matches_policy
            && ledger_id_matches
            && tree_size_in_range
            && requested_size_matches
            && requested_ledger_verified
            && ledger_prefix_verified
            && root_hash_matches
            && chain_tip_matches
            && timestamp_valid
            && timestamp_not_in_future
            && !active_signature_invalid
            && threshold_met;

        Ok(CheckpointVerificationV2 {
            match_mode: options.match_mode,
            origin_matches_policy,
            ledger_id_matches,
            checkpoint_tree_size: checkpoint.tree_size,
            current_tree_size,
            tree_size_in_range,
            current_size_matches,
            appended_event_count,
            ledger_prefix_verified,
            current_ledger_verified,
            root_hash_matches,
            chain_tip_matches,
            timestamp_valid,
            timestamp_not_in_future,
            verification_time_ms: options.verification_time_ms,
            max_future_skew_ms: options.max_future_skew_ms,
            accepted_signature_count,
            minimum_signature_count: policy.minimum_signatures,
            signatures,
            trusted,
            reasons,
        })
    }
}

impl CheckpointTrustPolicyV2 {
    /// Create and validate an independently managed checkpoint trust policy.
    pub fn new(
        expected_origin: String,
        expected_ledger_id: String,
        minimum_signatures: u32,
        keys: Vec<CheckpointPolicyKey>,
    ) -> Result<Self> {
        let policy = Self {
            version: CHECKPOINT_TRUST_POLICY_VERSION,
            expected_origin,
            expected_ledger_id,
            minimum_signatures,
            keys,
        };
        validate_policy(&policy)?;
        Ok(policy)
    }

    /// Re-validate a deserialized policy before using it as a trust root.
    pub fn validate(&self) -> Result<()> {
        validate_policy(self)
    }
}

/// Fixed canonical bytes signed by every v2 checkpoint signer.
///
/// The encoding is the domain separator, big-endian version, a u32-length
/// prefixed UTF-8 origin, raw 32-byte ledger id, one-byte tree and hash tags,
/// big-endian u64 tree size, raw 32-byte root, raw 32-byte chain tip, and
/// big-endian u64 timestamp. JSON serialization is never signed.
pub fn checkpoint_signing_message_v2(checkpoint: &LedgerCheckpointV2) -> Result<Vec<u8>> {
    validate_checkpoint(checkpoint)?;
    let ledger_id = canonical_hex::<32>(&checkpoint.ledger_id, "checkpoint ledger id")?;
    let root = canonical_hex::<32>(&checkpoint.root_hash, "checkpoint Merkle root")?;
    let tip = canonical_hex::<32>(&checkpoint.chain_tip, "checkpoint chain tip")?;
    let origin_len = u32::try_from(checkpoint.origin.len())
        .map_err(|_| checkpoint_error("checkpoint origin is too long"))?;

    let mut message = Vec::with_capacity(
        CHECKPOINT_DOMAIN_V2.len() + 4 + 4 + checkpoint.origin.len() + 32 + 1 + 1 + 8 + 32 + 32 + 8,
    );
    message.extend_from_slice(CHECKPOINT_DOMAIN_V2);
    message.extend_from_slice(&checkpoint.version.to_be_bytes());
    message.extend_from_slice(&origin_len.to_be_bytes());
    message.extend_from_slice(checkpoint.origin.as_bytes());
    message.extend_from_slice(&ledger_id);
    message.push(checkpoint.tree_algorithm.tag());
    message.push(checkpoint.hash_algorithm.tag());
    message.extend_from_slice(&checkpoint.tree_size.to_be_bytes());
    message.extend_from_slice(&root);
    message.extend_from_slice(&tip);
    message.extend_from_slice(&checkpoint.generated_at_ms.to_be_bytes());
    Ok(message)
}

/// Derive one active verifier entry for an operator-managed checkpoint policy.
///
/// Only the public key is returned; the seed is never included in the policy.
pub fn checkpoint_policy_key_v2(key_id: &str, seed_hex: &str) -> Result<CheckpointPolicyKey> {
    validate_key_id(key_id)?;
    let seed = decode_fixed::<32>(seed_hex, "checkpoint signing key seed")?;
    let signing_key = SigningKey::from_bytes(&seed);
    Ok(CheckpointPolicyKey {
        key_id: key_id.to_string(),
        public_key: encode_hex(signing_key.verifying_key().as_bytes()),
        status: AttestationKeyStatus::Active,
    })
}

/// Sign a checkpoint with one policy-authorized Ed25519 seed and create a v2
/// signed document.
pub fn sign_checkpoint_v2(
    checkpoint: LedgerCheckpointV2,
    policy: &CheckpointTrustPolicyV2,
    key_id: &str,
    seed_hex: &str,
) -> Result<SignedLedgerCheckpointV2> {
    let signature = sign_one(&checkpoint, policy, key_id, seed_hex)?;
    Ok(SignedLedgerCheckpointV2 {
        checkpoint,
        signatures: vec![signature],
    })
}

/// Add a distinct Ed25519 signature to an existing v2 checkpoint document.
pub fn add_checkpoint_signature_v2(
    signed: &mut SignedLedgerCheckpointV2,
    policy: &CheckpointTrustPolicyV2,
    key_id: &str,
    seed_hex: &str,
) -> Result<()> {
    validate_signed_document(signed)?;
    let signature = sign_one(&signed.checkpoint, policy, key_id, seed_hex)?;
    if signed
        .signatures
        .iter()
        .any(|existing| existing.key_id == signature.key_id)
    {
        return Err(checkpoint_error(format!(
            "duplicate checkpoint key id '{}'",
            signature.key_id
        )));
    }
    signed.signatures.push(signature);
    Ok(())
}

fn sign_one(
    checkpoint: &LedgerCheckpointV2,
    policy: &CheckpointTrustPolicyV2,
    key_id: &str,
    seed_hex: &str,
) -> Result<CheckpointSignatureV2> {
    validate_policy(policy)?;
    validate_key_id(key_id)?;
    let message = checkpoint_signing_message_v2(checkpoint)?;
    if checkpoint.origin != policy.expected_origin
        || checkpoint.ledger_id != policy.expected_ledger_id
    {
        return Err(checkpoint_error(
            "checkpoint origin and ledger id must match the signing policy",
        ));
    }
    let seed = decode_fixed::<32>(seed_hex, "checkpoint signing key seed")?;
    let signing_key = SigningKey::from_bytes(&seed);
    let public_key = encode_hex(signing_key.verifying_key().as_bytes());
    let policy_key = policy
        .keys
        .iter()
        .find(|key| key.key_id == key_id)
        .ok_or_else(|| {
            checkpoint_error(format!("checkpoint key id '{key_id}' is not in the policy"))
        })?;
    if policy_key.status != AttestationKeyStatus::Active {
        return Err(checkpoint_error(format!(
            "checkpoint key id '{key_id}' is revoked in the policy"
        )));
    }
    if policy_key.public_key != public_key {
        return Err(checkpoint_error(format!(
            "checkpoint seed does not match policy key id '{key_id}'"
        )));
    }
    let signature = signing_key.sign(&message);
    Ok(CheckpointSignatureV2 {
        algorithm: CheckpointSignatureAlgorithm::Ed25519,
        key_id: key_id.to_string(),
        signature: encode_hex(&signature.to_bytes()),
    })
}

fn evaluate_signature(
    signature: &CheckpointSignatureV2,
    message: &[u8],
    policy: &CheckpointTrustPolicyV2,
) -> CheckpointSignatureEvaluationV2 {
    let policy_key = policy
        .keys
        .iter()
        .find(|key| key.key_id == signature.key_id);
    let key_known = policy_key.is_some();
    let key_authorized = policy_key.is_some_and(|key| key.status == AttestationKeyStatus::Active);
    let key_revoked = policy_key.is_some_and(|key| key.status == AttestationKeyStatus::Revoked);
    let signature_valid =
        policy_key.is_some_and(|key| verify_signature(signature, message, &key.public_key));
    CheckpointSignatureEvaluationV2 {
        key_id: signature.key_id.clone(),
        key_known,
        signature_valid,
        key_authorized,
        key_revoked,
        accepted: signature_valid && key_authorized && !key_revoked,
    }
}

fn verify_signature(signature: &CheckpointSignatureV2, message: &[u8], public_key: &str) -> bool {
    let Ok(public_key) = canonical_hex::<32>(public_key, "checkpoint public key") else {
        return false;
    };
    let Ok(signature_bytes) = canonical_hex::<64>(&signature.signature, "checkpoint signature")
    else {
        return false;
    };
    let Ok(verifying_key) = VerifyingKey::from_bytes(&public_key) else {
        return false;
    };
    verifying_key
        .verify_strict(message, &Signature::from_bytes(&signature_bytes))
        .is_ok()
}

fn validate_signed_document(signed: &SignedLedgerCheckpointV2) -> Result<()> {
    checkpoint_signing_message_v2(&signed.checkpoint)?;
    if signed.signatures.is_empty() {
        return Err(checkpoint_error(
            "signed checkpoint must contain at least one signature",
        ));
    }
    if signed.signatures.len() > MAX_CHECKPOINT_SIGNATURES {
        return Err(checkpoint_error(format!(
            "signed checkpoint exceeds the {MAX_CHECKPOINT_SIGNATURES}-signature limit"
        )));
    }
    let mut key_ids = HashSet::new();
    for signature in &signed.signatures {
        validate_key_id(&signature.key_id)?;
        canonical_hex::<64>(&signature.signature, "checkpoint signature")?;
        if !key_ids.insert(signature.key_id.as_str()) {
            return Err(checkpoint_error(format!(
                "duplicate checkpoint key id '{}'",
                signature.key_id
            )));
        }
    }
    Ok(())
}

fn validate_policy(policy: &CheckpointTrustPolicyV2) -> Result<()> {
    if policy.version != CHECKPOINT_TRUST_POLICY_VERSION {
        return Err(checkpoint_error(format!(
            "unsupported checkpoint policy version {}, expected {}",
            policy.version, CHECKPOINT_TRUST_POLICY_VERSION
        )));
    }
    validate_origin(&policy.expected_origin)?;
    canonical_hex::<32>(&policy.expected_ledger_id, "policy ledger id")?;
    if policy.minimum_signatures == 0 {
        return Err(checkpoint_error(
            "checkpoint policy minimum_signatures must be at least one",
        ));
    }
    if policy.keys.len() > MAX_CHECKPOINT_SIGNATURES {
        return Err(checkpoint_error(format!(
            "checkpoint policy exceeds the {MAX_CHECKPOINT_SIGNATURES}-key limit"
        )));
    }

    let mut key_ids = HashSet::new();
    let mut public_keys = HashSet::new();
    let mut active_count = 0u64;
    for key in &policy.keys {
        let key_id = key.key_id.as_str();
        validate_key_id(key_id)?;
        canonical_hex::<32>(&key.public_key, "checkpoint policy public key")?;
        if !key_ids.insert(key_id) {
            return Err(checkpoint_error(format!(
                "duplicate checkpoint policy key id '{key_id}'"
            )));
        }
        if !public_keys.insert(key.public_key.as_str()) {
            return Err(checkpoint_error("duplicate checkpoint policy public key"));
        }
        if key.status == AttestationKeyStatus::Active {
            active_count += 1;
        }
    }
    if active_count < u64::from(policy.minimum_signatures) {
        return Err(checkpoint_error(format!(
            "checkpoint policy requires {} signature(s) but has only {active_count} active unique key(s)",
            policy.minimum_signatures
        )));
    }
    Ok(())
}

fn validate_checkpoint(checkpoint: &LedgerCheckpointV2) -> Result<()> {
    if checkpoint.version != LEDGER_CHECKPOINT_VERSION {
        return Err(checkpoint_error(format!(
            "unsupported ledger checkpoint version {}, expected {}",
            checkpoint.version, LEDGER_CHECKPOINT_VERSION
        )));
    }
    validate_origin(&checkpoint.origin)?;
    if checkpoint.tree_size == 0 {
        return Err(checkpoint_error(
            "ledger checkpoint tree_size must be at least one",
        ));
    }
    if checkpoint.generated_at_ms == 0 {
        return Err(checkpoint_error(
            "ledger checkpoint generated_at_ms must be non-zero",
        ));
    }
    canonical_hex::<32>(&checkpoint.ledger_id, "checkpoint ledger id")?;
    canonical_hex::<32>(&checkpoint.root_hash, "checkpoint Merkle root")?;
    canonical_hex::<32>(&checkpoint.chain_tip, "checkpoint chain tip")?;
    Ok(())
}

fn validate_origin(origin: &str) -> Result<()> {
    if origin.is_empty() || origin.trim() != origin {
        return Err(checkpoint_error(
            "checkpoint origin must be non-empty without surrounding whitespace",
        ));
    }
    if origin.len() > MAX_ORIGIN_BYTES {
        return Err(checkpoint_error(format!(
            "checkpoint origin exceeds {MAX_ORIGIN_BYTES} UTF-8 bytes"
        )));
    }
    if !origin.bytes().all(|byte| (0x21..=0x7e).contains(&byte)) {
        return Err(checkpoint_error(
            "checkpoint origin must contain only visible ASCII without spaces",
        ));
    }
    Ok(())
}

fn validate_key_id(key_id: &str) -> Result<()> {
    if key_id.is_empty() || key_id.trim() != key_id {
        return Err(checkpoint_error(
            "checkpoint key id must be non-empty without surrounding whitespace",
        ));
    }
    if key_id.len() > MAX_KEY_ID_BYTES {
        return Err(checkpoint_error(format!(
            "checkpoint key id exceeds {MAX_KEY_ID_BYTES} UTF-8 bytes"
        )));
    }
    if !key_id.bytes().all(|byte| (0x21..=0x7e).contains(&byte)) {
        return Err(checkpoint_error(
            "checkpoint key id must contain only visible ASCII without spaces",
        ));
    }
    Ok(())
}

fn canonical_hex<const N: usize>(value: &str, label: &str) -> Result<[u8; N]> {
    if value.trim() != value || value.bytes().any(|byte| byte.is_ascii_uppercase()) {
        return Err(checkpoint_error(format!(
            "{label} must use canonical lowercase hex"
        )));
    }
    decode_fixed::<N>(value, label)
}

fn max_event_timestamp(events: &[EventEnvelope]) -> Result<u64> {
    events
        .iter()
        .map(|event| event.timestamp_ms)
        .max()
        .ok_or_else(|| checkpoint_error("empty ledger cannot define a checkpoint policy"))
}

fn derive_ledger_id(events: &[EventEnvelope]) -> Result<Option<String>> {
    let Some(genesis) = events.first() else {
        return Ok(None);
    };
    let genesis_hash = canonical_hex::<32>(&genesis.chain_hash(), "genesis chain hash")?;
    let mut hasher = Sha256::new();
    hasher.update(LEDGER_ID_DOMAIN_V1);
    hasher.update(genesis_hash);
    Ok(Some(encode_hex(&hasher.finalize())))
}

fn checkpoint_error(message: impl Into<String>) -> NahualiError {
    NahualiError::Attestation {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, time::SystemTime};

    use super::*;

    const ORIGIN: &str = "nahuali.local/tests/checkpoint";

    fn seed(byte: u8) -> String {
        encode_hex(&[byte; 32])
    }

    fn temp_store(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "nahuali_checkpoint_{label}_{}_{}",
            std::process::id(),
            nanos
        ))
    }

    fn memory_with_episode(label: &str) -> (PathBuf, MemoryEngine) {
        let path = temp_store(label);
        let _ = fs::remove_file(&path);
        let mut memory = MemoryEngine::open(&path).expect("open checkpoint test store");
        memory
            .remember("Hrafn retains evidence for the signed checkpoint.", vec![])
            .expect("record checkpoint evidence");
        (path, memory)
    }

    fn current_checkpoint(memory: &MemoryEngine) -> LedgerCheckpointV2 {
        let generated_at_ms = memory
            .events()
            .last()
            .expect("test ledger has an event")
            .timestamp_ms;
        memory
            .create_checkpoint_v2(ORIGIN, generated_at_ms)
            .expect("create checkpoint")
    }

    fn signed_and_policy(
        memory: &MemoryEngine,
    ) -> (SignedLedgerCheckpointV2, CheckpointTrustPolicyV2) {
        let policy = memory
            .create_checkpoint_policy_v2(
                ORIGIN,
                1,
                vec![
                    checkpoint_policy_key_v2("operator-primary", &seed(1))
                        .expect("derive verifier key"),
                ],
            )
            .expect("create independent checkpoint policy");
        let signed = sign_checkpoint_v2(
            current_checkpoint(memory),
            &policy,
            "operator-primary",
            &seed(1),
        )
        .expect("sign checkpoint");
        (signed, policy)
    }

    fn verification_options(
        signed: &SignedLedgerCheckpointV2,
        match_mode: CheckpointMatchMode,
    ) -> CheckpointVerificationOptionsV2 {
        CheckpointVerificationOptionsV2 {
            match_mode,
            verification_time_ms: signed.checkpoint.generated_at_ms,
            max_future_skew_ms: 0,
        }
    }

    #[test]
    fn current_and_historical_checkpoint_modes_are_distinct() {
        let (path, mut memory) = memory_with_episode("current_and_historical");
        let (signed, policy) = signed_and_policy(&memory);

        let current = memory
            .verify_checkpoint_v2(
                &signed,
                &policy,
                verification_options(&signed, CheckpointMatchMode::Current),
            )
            .expect("verify current checkpoint");
        assert!(current.trusted);
        assert!(current.current_size_matches);
        assert!(current.reasons.is_empty());

        memory
            .remember("A later append preserves the checkpointed prefix.", vec![])
            .expect("append later event");
        let stale = memory
            .verify_checkpoint_v2(
                &signed,
                &policy,
                verification_options(&signed, CheckpointMatchMode::Current),
            )
            .expect("evaluate stale current checkpoint");
        assert!(!stale.trusted);
        assert!(!stale.current_size_matches);
        assert!(stale.ledger_prefix_verified);

        let historical = memory
            .verify_checkpoint_v2(
                &signed,
                &policy,
                verification_options(&signed, CheckpointMatchMode::Historical),
            )
            .expect("verify historical checkpoint");
        assert!(historical.trusted);
        assert!(!historical.current_size_matches);
        assert!(historical.ledger_prefix_verified);

        drop(memory);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn signer_asserted_time_cannot_exceed_verifier_clock_tolerance() {
        let (path, memory) = memory_with_episode("future_timestamp");
        let policy = memory
            .create_checkpoint_policy_v2(
                ORIGIN,
                1,
                vec![
                    checkpoint_policy_key_v2("operator-primary", &seed(1))
                        .expect("derive verifier key"),
                ],
            )
            .expect("create policy");
        let event_time = max_event_timestamp(memory.events()).expect("event time");
        let checkpoint = memory
            .create_checkpoint_v2(ORIGIN, event_time + 1_000)
            .expect("signer may assert a later local time");
        let signed = sign_checkpoint_v2(checkpoint, &policy, "operator-primary", &seed(1))
            .expect("sign future checkpoint");

        let verdict = memory
            .verify_checkpoint_v2(
                &signed,
                &policy,
                CheckpointVerificationOptionsV2 {
                    match_mode: CheckpointMatchMode::Current,
                    verification_time_ms: event_time,
                    max_future_skew_ms: 100,
                },
            )
            .expect("evaluate verifier clock tolerance");
        assert!(!verdict.timestamp_not_in_future);
        assert!(!verdict.trusted);

        drop(memory);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn every_signed_scalar_field_is_cryptographically_bound() {
        let (path, memory) = memory_with_episode("field_mutation");
        let (signed, policy) = signed_and_policy(&memory);

        let mut mutations = Vec::new();

        let mut changed = signed.clone();
        changed.checkpoint.origin = "nahuali.local/tests/other".to_string();
        mutations.push(changed);

        let mut changed = signed.clone();
        changed.checkpoint.ledger_id = "11".repeat(32);
        mutations.push(changed);

        let mut changed = signed.clone();
        changed.checkpoint.tree_size += 1;
        mutations.push(changed);

        let mut changed = signed.clone();
        changed.checkpoint.root_hash = "22".repeat(32);
        mutations.push(changed);

        let mut changed = signed.clone();
        changed.checkpoint.chain_tip = "33".repeat(32);
        mutations.push(changed);

        let mut changed = signed.clone();
        changed.checkpoint.generated_at_ms += 1;
        mutations.push(changed);

        for mutation in mutations {
            let verdict = memory
                .verify_checkpoint_v2(
                    &mutation,
                    &policy,
                    verification_options(&mutation, CheckpointMatchMode::Historical),
                )
                .expect("well-formed mutation receives a rejection verdict");
            assert!(!verdict.trusted);
            assert!(!verdict.signatures[0].signature_valid);
        }

        let mut unsupported_version = signed;
        unsupported_version.checkpoint.version += 1;
        assert!(
            memory
                .verify_checkpoint_v2(
                    &unsupported_version,
                    &policy,
                    verification_options(&unsupported_version, CheckpointMatchMode::Historical,),
                )
                .is_err()
        );

        drop(memory);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn threshold_requires_distinct_active_operator_keys() {
        let (path, memory) = memory_with_episode("threshold");
        let policy = memory
            .create_checkpoint_policy_v2(
                ORIGIN,
                2,
                vec![
                    checkpoint_policy_key_v2("operator-a", &seed(1)).expect("derive first key"),
                    checkpoint_policy_key_v2("operator-b", &seed(2)).expect("derive second key"),
                ],
            )
            .expect("create two-of-two policy");
        let mut signed =
            sign_checkpoint_v2(current_checkpoint(&memory), &policy, "operator-a", &seed(1))
                .expect("sign first key");
        add_checkpoint_signature_v2(&mut signed, &policy, "operator-b", &seed(2))
            .expect("sign second key");

        assert!(
            memory
                .verify_checkpoint_v2(
                    &signed,
                    &policy,
                    verification_options(&signed, CheckpointMatchMode::Current),
                )
                .expect("verify threshold")
                .trusted
        );

        let mut missing = signed.clone();
        missing.signatures.pop();
        let verdict = memory
            .verify_checkpoint_v2(
                &missing,
                &policy,
                verification_options(&missing, CheckpointMatchMode::Current),
            )
            .expect("evaluate missing signer");
        assert!(!verdict.trusted);
        assert_eq!(verdict.accepted_signature_count, 1);

        assert!(add_checkpoint_signature_v2(&mut signed, &policy, "operator-a", &seed(3)).is_err());
        assert!(add_checkpoint_signature_v2(&mut signed, &policy, "operator-c", &seed(2)).is_err());

        drop(memory);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn unknown_and_revoked_extras_are_ignored_but_active_bad_signatures_block() {
        let (path, memory) = memory_with_episode("unknown_and_revoked");
        let policy = memory
            .create_checkpoint_policy_v2(
                ORIGIN,
                1,
                vec![
                    checkpoint_policy_key_v2("operator-a", &seed(1)).expect("derive first key"),
                    checkpoint_policy_key_v2("operator-b", &seed(2)).expect("derive second key"),
                ],
            )
            .expect("create one-of-two policy");
        let mut signed =
            sign_checkpoint_v2(current_checkpoint(&memory), &policy, "operator-a", &seed(1))
                .expect("sign first key");
        add_checkpoint_signature_v2(&mut signed, &policy, "operator-b", &seed(2))
            .expect("sign second key");

        let mut unknown = signed.clone();
        let mut unknown_signature = unknown.signatures[1].clone();
        unknown_signature.key_id = "untrusted-operator".to_string();
        unknown.signatures.push(unknown_signature);
        let verdict = memory
            .verify_checkpoint_v2(
                &unknown,
                &policy,
                verification_options(&unknown, CheckpointMatchMode::Current),
            )
            .expect("evaluate unknown signer");
        assert!(verdict.trusted);
        assert!(!verdict.signatures[2].key_known);
        assert!(!verdict.signatures[2].accepted);

        let mut revoked_policy = policy.clone();
        revoked_policy.keys[1].status = AttestationKeyStatus::Revoked;
        let verdict = memory
            .verify_checkpoint_v2(
                &signed,
                &revoked_policy,
                verification_options(&signed, CheckpointMatchMode::Current),
            )
            .expect("evaluate revoked extra signer");
        assert!(verdict.trusted);
        assert!(verdict.signatures[1].signature_valid);
        assert!(verdict.signatures[1].key_revoked);
        assert!(!verdict.signatures[1].accepted);

        let mut active_bad = signed;
        let mut bytes = canonical_hex::<64>(
            &active_bad.signatures[1].signature,
            "test checkpoint signature",
        )
        .expect("decode test signature");
        bytes[0] ^= 1;
        active_bad.signatures[1].signature = encode_hex(&bytes);
        let verdict = memory
            .verify_checkpoint_v2(
                &active_bad,
                &policy,
                verification_options(&active_bad, CheckpointMatchMode::Current),
            )
            .expect("evaluate malformed active signature");
        assert!(!verdict.trusted);
        assert!(!verdict.signatures[1].signature_valid);

        drop(memory);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn canonical_signing_message_has_a_fixed_golden_encoding() {
        let checkpoint = LedgerCheckpointV2 {
            version: LEDGER_CHECKPOINT_VERSION,
            origin: "nahuali.test".to_string(),
            ledger_id: "00".repeat(32),
            tree_algorithm: CheckpointTreeAlgorithm::NahualiMerkleV1,
            hash_algorithm: CheckpointHashAlgorithm::Sha256,
            tree_size: 7,
            root_hash: "11".repeat(32),
            chain_tip: "22".repeat(32),
            generated_at_ms: 9,
        };

        let message = checkpoint_signing_message_v2(&checkpoint).expect("encode checkpoint");
        assert_eq!(
            encode_hex(&message),
            concat!(
                "6e616875616c692e6c65646765722e636865636b706f696e742e763200",
                "00000002",
                "0000000c",
                "6e616875616c692e74657374",
                "0000000000000000000000000000000000000000000000000000000000000000",
                "01",
                "01",
                "0000000000000007",
                "1111111111111111111111111111111111111111111111111111111111111111",
                "2222222222222222222222222222222222222222222222222222222222222222",
                "0000000000000009",
            )
        );
    }

    #[test]
    fn canonical_fields_and_strict_json_are_enforced() {
        let checkpoint = LedgerCheckpointV2 {
            version: LEDGER_CHECKPOINT_VERSION,
            origin: "nahuali.test".to_string(),
            ledger_id: "00".repeat(32),
            tree_algorithm: CheckpointTreeAlgorithm::NahualiMerkleV1,
            hash_algorithm: CheckpointHashAlgorithm::Sha256,
            tree_size: 1,
            root_hash: "11".repeat(32),
            chain_tip: "22".repeat(32),
            generated_at_ms: 1,
        };
        let mut uppercase = checkpoint.clone();
        uppercase.root_hash = "AA".repeat(32);
        assert!(checkpoint_signing_message_v2(&uppercase).is_err());

        let mut value = serde_json::to_value(checkpoint).expect("serialize checkpoint");
        value
            .as_object_mut()
            .expect("checkpoint is an object")
            .insert("unexpected".to_string(), serde_json::json!(true));
        assert!(serde_json::from_value::<LedgerCheckpointV2>(value).is_err());
    }

    #[test]
    fn malformed_or_ambiguous_policy_is_rejected() {
        let (path, memory) = memory_with_episode("malformed_policy");
        let (signed, policy) = signed_and_policy(&memory);

        let mut policy_json = serde_json::to_value(&policy).expect("serialize policy");
        policy_json["keys"][0]
            .as_object_mut()
            .expect("policy key is an object")
            .insert("unexpected".to_string(), serde_json::json!(true));
        assert!(serde_json::from_value::<CheckpointTrustPolicyV2>(policy_json).is_err());

        let mut duplicate_id = policy.clone();
        duplicate_id.keys.push(duplicate_id.keys[0].clone());
        assert!(
            memory
                .verify_checkpoint_v2(
                    &signed,
                    &duplicate_id,
                    verification_options(&signed, CheckpointMatchMode::Current),
                )
                .is_err()
        );

        let mut wrong_origin = policy.clone();
        wrong_origin.expected_origin = "nahuali.local/tests/other".to_string();
        let verdict = memory
            .verify_checkpoint_v2(
                &signed,
                &wrong_origin,
                verification_options(&signed, CheckpointMatchMode::Current),
            )
            .expect("evaluate mismatched origin");
        assert!(!verdict.trusted);
        assert!(!verdict.origin_matches_policy);

        let mut wrong_ledger = policy;
        wrong_ledger.expected_ledger_id = "44".repeat(32);
        let verdict = memory
            .verify_checkpoint_v2(
                &signed,
                &wrong_ledger,
                verification_options(&signed, CheckpointMatchMode::Current),
            )
            .expect("evaluate mismatched lineage");
        assert!(!verdict.trusted);
        assert!(!verdict.ledger_id_matches);

        drop(memory);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn empty_ledger_cannot_create_a_checkpoint() {
        let path = temp_store("empty");
        let _ = fs::remove_file(&path);
        let memory = MemoryEngine::open(&path).expect("open empty checkpoint test store");
        assert!(memory.create_checkpoint_v2(ORIGIN, 1).is_err());
        drop(memory);
        let _ = fs::remove_file(path);
    }
}
