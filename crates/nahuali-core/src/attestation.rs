//! Ed25519 detached attestation over the tamper-evident ledger chain tip.
//!
//! The hash chain ([`crate::verify_event_chain`]) makes an in-place rewrite of
//! any historical event detectable: the rewrite breaks the link at the *next*
//! event. The one rewrite a self-contained chain cannot catch is a full re-chain
//! of the ledger suffix — an attacker who controls the whole store can recompute
//! every chained hash after an edit and produce a self-consistent chain, just
//! with a different tip.
//!
//! Signing the tip with a key the attacker does not hold closes that gap. The
//! operator periodically signs the current [`MemoryEngine::chain_tip`] and keeps
//! the detached signature outside the store (the "receipt"). A later re-chain
//! changes the tip, so the receipt no longer verifies against the live ledger,
//! and forging a fresh receipt requires the Ed25519 private key. Signing is
//! detached: the signature lives next to the ledger, never inside it, so the
//! default records stay byte-for-byte identical.
//!
//! Keys are 32-byte Ed25519 seeds supplied as hex by the operator (generate one
//! with, e.g., `openssl rand -hex 32`). The core never generates randomness and
//! never touches the network.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

use serde::{Deserialize, Serialize};

use crate::error::{NahualiError, Result};
use crate::store::MemoryEngine;

/// Attestation document format version.
pub const LEDGER_ATTESTATION_VERSION: u32 = 1;

/// Algorithm identifier recorded in and required by every attestation.
pub const LEDGER_ATTESTATION_ALGORITHM: &str = "ed25519";

/// Domain separator bound into every signed tip message so a tip signature can
/// never be replayed as a signature over unrelated bytes.
const ATTESTATION_DOMAIN: &[u8] = b"nahuali-ledger-tip-attestation-v1";

/// A detached Ed25519 signature over a ledger chain tip.
///
/// This is the portable "receipt" an operator keeps outside the store. It binds
/// the tip's chained hash and its sequence number, signed under a domain
/// separator, together with the public key needed to verify it.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct LedgerAttestation {
    /// Attestation document format version.
    pub version: u32,
    /// Signature algorithm; always [`LEDGER_ATTESTATION_ALGORITHM`].
    pub algorithm: String,
    /// Sequence number of the attested tip event.
    pub sequence: u64,
    /// Chained hash of the attested tip event (hex).
    pub tip: String,
    /// Ed25519 public key, 32 bytes (hex).
    pub public_key: String,
    /// Ed25519 detached signature, 64 bytes (hex).
    pub signature: String,
}

/// Verdict produced by checking an attestation against a live ledger.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttestationVerdict {
    /// Whether the attestation's tip and sequence match the ledger's current tip.
    pub matches_tip: bool,
    /// Whether the signature verifies against the attested tip and public key.
    pub signature_valid: bool,
    /// The ledger's current chain tip at verification time, if any.
    pub current_tip: Option<String>,
    /// Sequence number of the ledger's current tip, if any.
    pub current_sequence: Option<u64>,
}

impl AttestationVerdict {
    /// Whether the receipt vouches for the ledger's current state: it matches the
    /// live tip and its signature verifies.
    pub fn is_valid(&self) -> bool {
        self.matches_tip && self.signature_valid
    }
}

/// Bytes signed for an attestation: the domain separator, then the length-
/// prefixed sequence and tip, so no two distinct `(sequence, tip)` pairs can
/// produce the same message.
fn attestation_message(sequence: u64, tip: &str) -> Vec<u8> {
    let tip_bytes = tip.as_bytes();
    let mut message = Vec::with_capacity(ATTESTATION_DOMAIN.len() + 1 + 8 + 8 + tip_bytes.len());
    message.extend_from_slice(ATTESTATION_DOMAIN);
    message.push(0x00);
    message.extend_from_slice(&sequence.to_be_bytes());
    message.extend_from_slice(&(tip_bytes.len() as u64).to_be_bytes());
    message.extend_from_slice(tip_bytes);
    message
}

/// Produce a detached Ed25519 attestation over `(sequence, tip)` using a 32-byte
/// seed supplied as hex.
pub fn sign_chain_tip(seed_hex: &str, sequence: u64, tip: &str) -> Result<LedgerAttestation> {
    let seed = decode_fixed::<32>(seed_hex, "signing key seed")?;
    let signing_key = SigningKey::from_bytes(&seed);
    let signature = signing_key.sign(&attestation_message(sequence, tip));

    Ok(LedgerAttestation {
        version: LEDGER_ATTESTATION_VERSION,
        algorithm: LEDGER_ATTESTATION_ALGORITHM.to_string(),
        sequence,
        tip: tip.to_string(),
        public_key: encode_hex(signing_key.verifying_key().as_bytes()),
        signature: encode_hex(&signature.to_bytes()),
    })
}

/// Verify a detached attestation against an expected `(sequence, tip)`.
///
/// Returns `true` only when the attestation's recorded tip and sequence match
/// the expected ones *and* the signature verifies (strictly) under the recorded
/// public key. A moved tip or a tampered field yields `false`; malformed key,
/// signature, or algorithm fields are reported as errors.
pub fn verify_chain_tip(attestation: &LedgerAttestation, sequence: u64, tip: &str) -> Result<bool> {
    if attestation.version != LEDGER_ATTESTATION_VERSION {
        return Err(NahualiError::Attestation {
            message: format!(
                "unsupported attestation version {}, expected {}",
                attestation.version, LEDGER_ATTESTATION_VERSION
            ),
        });
    }
    if attestation.algorithm != LEDGER_ATTESTATION_ALGORITHM {
        return Err(NahualiError::Attestation {
            message: format!(
                "unsupported attestation algorithm '{}', expected '{}'",
                attestation.algorithm, LEDGER_ATTESTATION_ALGORITHM
            ),
        });
    }

    let public_key_bytes = decode_fixed::<32>(&attestation.public_key, "public key")?;
    let signature_bytes = decode_fixed::<64>(&attestation.signature, "signature")?;
    let verifying_key = VerifyingKey::from_bytes(&public_key_bytes).map_err(|source| {
        NahualiError::Attestation {
            message: format!("invalid Ed25519 public key: {source}"),
        }
    })?;
    let signature = Signature::from_bytes(&signature_bytes);

    if attestation.sequence != sequence || attestation.tip != tip {
        return Ok(false);
    }

    Ok(verifying_key
        .verify_strict(&attestation_message(sequence, tip), &signature)
        .is_ok())
}

/// Verdict for whether a self-signed attestation matches a point in this
/// ledger's own history. This establishes internal consistency, not signer
/// authority; authorization boundaries must use an operator-held keyring.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttestedCheckpointVerdict {
    /// The signature over the attested `(sequence, tip)` pair verifies.
    pub signature_valid: bool,
    /// This ledger's chain hash at the attested sequence equals the attested tip,
    /// so the checkpoint and the history up to it are unchanged.
    pub matches_history: bool,
    /// The self-signed checkpoint is internally consistent: the signature
    /// verifies and it sits in this ledger's unaltered history. This does not
    /// authorize the signing key; callers that need an external trust anchor
    /// must also check an operator-held keyring.
    pub anchored: bool,
    /// The attested sequence the checkpoint anchors.
    pub sequence: u64,
}

/// Historical-checkpoint verdict that also applies an operator-held root of
/// trust to the receipt's signing key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustedAttestedCheckpointVerdict {
    /// The signature over the attested `(sequence, tip)` pair verifies.
    pub signature_valid: bool,
    /// The checkpoint matches this ledger's history at the attested sequence.
    pub matches_history: bool,
    /// The signing key is present and active in the operator keyring.
    pub key_trusted: bool,
    /// The signing key is explicitly revoked in the operator keyring.
    pub key_revoked: bool,
    /// Sequence number of the historical checkpoint.
    pub sequence: u64,
}

impl TrustedAttestedCheckpointVerdict {
    /// Whether history, signature, and external signer authorization all pass.
    pub fn is_trusted(&self) -> bool {
        self.signature_valid && self.matches_history && self.key_trusted && !self.key_revoked
    }
}

impl MemoryEngine {
    /// Sign the current chain tip, producing a portable attestation receipt.
    ///
    /// Returns `None` for an empty ledger (no tip to attest). The `seed_hex` is a
    /// 32-byte Ed25519 seed in hex held only by the operator.
    pub fn attest_chain_tip(&self, seed_hex: &str) -> Result<Option<LedgerAttestation>> {
        let Some(tip) = self.chain_tip() else {
            return Ok(None);
        };
        let sequence = self
            .events()
            .last()
            .map(|event| event.sequence)
            .expect("a non-empty chain tip implies at least one event");
        sign_chain_tip(seed_hex, sequence, &tip).map(Some)
    }

    /// Check a previously issued attestation against this ledger's current tip.
    pub fn verify_chain_tip_attestation(
        &self,
        attestation: &LedgerAttestation,
    ) -> Result<AttestationVerdict> {
        let current_tip = self.chain_tip();
        let current_sequence = self.events().last().map(|event| event.sequence);

        let (matches_tip, signature_valid) = match (&current_tip, current_sequence) {
            (Some(tip), Some(sequence)) => {
                let matches = attestation.sequence == sequence && &attestation.tip == tip;
                let valid = verify_chain_tip(attestation, sequence, tip)?;
                (matches, valid)
            }
            _ => (false, false),
        };

        Ok(AttestationVerdict {
            matches_tip,
            signature_valid,
            current_tip,
            current_sequence,
        })
    }

    /// Check whether a self-signed attestation matches a checkpoint in this
    /// ledger's own history, rather than only the current tip. Unlike
    /// [`Self::verify_chain_tip_attestation`], the attested sequence need not be
    /// the latest event. This does not authorize the embedded key; callers that
    /// make trust decisions must use
    /// [`Self::verify_attested_checkpoint_with_keyring`].
    pub fn verify_attested_checkpoint(
        &self,
        attestation: &LedgerAttestation,
    ) -> Result<AttestedCheckpointVerdict> {
        let signature_valid =
            verify_chain_tip(attestation, attestation.sequence, &attestation.tip)?;
        let matches_history = self
            .events()
            .iter()
            .find(|event| event.sequence == attestation.sequence)
            .map(|event| event.chain_hash() == attestation.tip)
            .unwrap_or(false);

        Ok(AttestedCheckpointVerdict {
            signature_valid,
            matches_history,
            anchored: signature_valid && matches_history,
            sequence: attestation.sequence,
        })
    }

    /// Verify a historical receipt against this ledger and an operator-held
    /// keyring. Unlike [`Self::verify_attested_checkpoint`], this result can be
    /// used as an authorization boundary because the receipt's embedded key is
    /// not allowed to establish trust by itself.
    pub fn verify_attested_checkpoint_with_keyring(
        &self,
        attestation: &LedgerAttestation,
        keyring: &AttestationKeyring,
    ) -> Result<TrustedAttestedCheckpointVerdict> {
        let checkpoint = self.verify_attested_checkpoint(attestation)?;
        Ok(TrustedAttestedCheckpointVerdict {
            signature_valid: checkpoint.signature_valid,
            matches_history: checkpoint.matches_history,
            key_trusted: keyring.authorizes(&attestation.public_key),
            key_revoked: keyring.is_revoked(&attestation.public_key),
            sequence: checkpoint.sequence,
        })
    }

    /// Like [`Self::verify_chain_tip_attestation`], but also requires the
    /// receipt's signing key to be trusted and active in `keyring`. A receipt
    /// under a revoked or unknown key is not honored even when its signature
    /// verifies, which is what lets a leaked key be retired without rewriting
    /// history.
    pub fn verify_chain_tip_attestation_with_keyring(
        &self,
        attestation: &LedgerAttestation,
        keyring: &AttestationKeyring,
    ) -> Result<TrustedAttestationVerdict> {
        match (self.chain_tip(), self.events().last().map(|e| e.sequence)) {
            (Some(tip), Some(sequence)) => {
                verify_attestation_with_keyring(attestation, sequence, &tip, keyring)
            }
            _ => Ok(TrustedAttestationVerdict {
                signature_valid: false,
                matches_tip: false,
                key_trusted: keyring.authorizes(&attestation.public_key),
                key_revoked: keyring.is_revoked(&attestation.public_key),
            }),
        }
    }
}

/// Status of a key in the operator's attestation keyring.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttestationKeyStatus {
    /// Receipts signed by this key are honored.
    Active,
    /// Receipts signed by this key are rejected even when the signature itself
    /// verifies — the revocation path for a leaked or retired key.
    Revoked,
}

/// One operator-trusted attestation key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttestationKey {
    /// Optional human-readable key identifier or label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_id: Option<String>,
    /// Ed25519 public key, 32 bytes (hex).
    pub public_key: String,
    /// Whether receipts under this key are honored or rejected.
    pub status: AttestationKeyStatus,
}

/// An operator-held set of trusted attestation keys, kept outside the store like
/// the receipts themselves.
///
/// This is the recovery layer the self-attesting receipt cannot provide on its
/// own: [`verify_chain_tip`] trusts whatever public key a receipt carries, so a
/// leaked seed's receipts keep verifying forever. The keyring moves the trust
/// decision to the operator. **Rotation** is adding a new [`AttestationKeyStatus::Active`]
/// key (and re-attesting the current tip with it); **revocation** is flipping a
/// key to [`AttestationKeyStatus::Revoked`] so its receipts stop being honored.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttestationKeyring {
    /// The trusted keys, in operator-defined order.
    pub keys: Vec<AttestationKey>,
}

impl AttestationKeyring {
    /// Whether `public_key` is present and [`AttestationKeyStatus::Active`].
    pub fn authorizes(&self, public_key: &str) -> bool {
        self.keys
            .iter()
            .any(|key| key.public_key == public_key && key.status == AttestationKeyStatus::Active)
    }

    /// Whether `public_key` is present and [`AttestationKeyStatus::Revoked`].
    pub fn is_revoked(&self, public_key: &str) -> bool {
        self.keys
            .iter()
            .any(|key| key.public_key == public_key && key.status == AttestationKeyStatus::Revoked)
    }
}

/// Verdict combining a receipt's cryptographic validity with the operator
/// keyring's authorization decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrustedAttestationVerdict {
    /// The signature verifies against the recorded public key and `(sequence, tip)`.
    pub signature_valid: bool,
    /// The receipt's `(sequence, tip)` match the expected checkpoint.
    pub matches_tip: bool,
    /// The signing key is present and active in the keyring.
    pub key_trusted: bool,
    /// The signing key is present and revoked in the keyring.
    pub key_revoked: bool,
}

impl TrustedAttestationVerdict {
    /// Honored only when the receipt matches the checkpoint, the signature
    /// verifies, and the signing key is trusted and not revoked.
    pub fn is_trusted(&self) -> bool {
        self.matches_tip && self.signature_valid && self.key_trusted && !self.key_revoked
    }
}

/// Verify an attestation against an expected `(sequence, tip)` *and* an operator
/// keyring.
///
/// Unlike [`verify_chain_tip`], which trusts whatever public key the receipt
/// carries, this defers the trust decision to the operator: a receipt signed by
/// a revoked or unknown key is not honored even when its signature is
/// cryptographically valid. This is what lets a leaked key be retired without
/// rewriting history — rotate to a new active key and revoke the old one.
pub fn verify_attestation_with_keyring(
    attestation: &LedgerAttestation,
    sequence: u64,
    tip: &str,
    keyring: &AttestationKeyring,
) -> Result<TrustedAttestationVerdict> {
    let signature_valid = verify_chain_tip(attestation, sequence, tip)?;
    let matches_tip = attestation.sequence == sequence && attestation.tip == tip;
    Ok(TrustedAttestationVerdict {
        signature_valid,
        matches_tip,
        key_trusted: keyring.authorizes(&attestation.public_key),
        key_revoked: keyring.is_revoked(&attestation.public_key),
    })
}

pub(crate) fn encode_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from_digit((byte >> 4) as u32, 16).expect("nibble is a hex digit"));
        out.push(char::from_digit((byte & 0x0f) as u32, 16).expect("nibble is a hex digit"));
    }
    out
}

pub(crate) fn decode_fixed<const N: usize>(hex: &str, label: &str) -> Result<[u8; N]> {
    let hex = hex.trim();
    if hex.len() != N * 2 {
        return Err(NahualiError::Attestation {
            message: format!(
                "{label} must be {N} bytes ({} hex characters), got {}",
                N * 2,
                hex.len()
            ),
        });
    }
    let mut bytes = [0u8; N];
    for (index, byte) in bytes.iter_mut().enumerate() {
        let high = hex_value(hex.as_bytes()[index * 2], label)?;
        let low = hex_value(hex.as_bytes()[index * 2 + 1], label)?;
        *byte = (high << 4) | low;
    }
    Ok(bytes)
}

fn hex_value(byte: u8, label: &str) -> Result<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        other => Err(NahualiError::Attestation {
            message: format!("{label} contains a non-hex character '{}'", other as char),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A fixed, non-secret test seed (32 bytes of 0x01..0x20).
    fn test_seed() -> String {
        encode_hex(&core_test_seed_bytes())
    }

    fn core_test_seed_bytes() -> [u8; 32] {
        let mut seed = [0u8; 32];
        for (index, byte) in seed.iter_mut().enumerate() {
            *byte = (index as u8) + 1;
        }
        seed
    }

    #[test]
    fn sign_then_verify_roundtrips() {
        let attestation = sign_chain_tip(&test_seed(), 7, "deadbeef").unwrap();
        assert_eq!(attestation.version, LEDGER_ATTESTATION_VERSION);
        assert_eq!(attestation.algorithm, LEDGER_ATTESTATION_ALGORITHM);
        assert_eq!(attestation.public_key.len(), 64);
        assert_eq!(attestation.signature.len(), 128);
        assert!(verify_chain_tip(&attestation, 7, "deadbeef").unwrap());
    }

    #[test]
    fn verification_fails_when_the_tip_moves() {
        let attestation = sign_chain_tip(&test_seed(), 7, "deadbeef").unwrap();
        // A re-chain that changes the tip (and/or its sequence) no longer
        // matches the signed receipt.
        assert!(!verify_chain_tip(&attestation, 7, "feedface").unwrap());
        assert!(!verify_chain_tip(&attestation, 8, "deadbeef").unwrap());
    }

    #[test]
    fn verification_fails_when_the_signature_is_forged() {
        let mut attestation = sign_chain_tip(&test_seed(), 7, "deadbeef").unwrap();
        // Flip the first signature byte; the strict check must reject it.
        let mut bytes = decode_fixed::<64>(&attestation.signature, "signature").unwrap();
        bytes[0] ^= 0x01;
        attestation.signature = encode_hex(&bytes);
        assert!(!verify_chain_tip(&attestation, 7, "deadbeef").unwrap());
    }

    #[test]
    fn malformed_key_material_is_rejected() {
        let attestation = sign_chain_tip(&test_seed(), 1, "abcd").unwrap();
        assert!(sign_chain_tip("not-hex", 1, "abcd").is_err());
        assert!(sign_chain_tip("00", 1, "abcd").is_err());

        let mut wrong_algorithm = attestation.clone();
        wrong_algorithm.algorithm = "rsa".to_string();
        assert!(verify_chain_tip(&wrong_algorithm, 1, "abcd").is_err());

        let mut wrong_version = attestation;
        wrong_version.version += 1;
        assert!(verify_chain_tip(&wrong_version, 1, "abcd").is_err());
    }

    fn rotated_seed() -> String {
        let mut seed = [0u8; 32];
        for (index, byte) in seed.iter_mut().enumerate() {
            *byte = (index as u8) + 100;
        }
        encode_hex(&seed)
    }

    #[test]
    fn keyring_honors_active_and_rejects_revoked_or_unknown_keys() {
        let attestation = sign_chain_tip(&test_seed(), 7, "deadbeef").unwrap();
        let public_key = attestation.public_key.clone();

        let active = AttestationKeyring {
            keys: vec![AttestationKey {
                key_id: Some("primary".to_string()),
                public_key: public_key.clone(),
                status: AttestationKeyStatus::Active,
            }],
        };
        let verdict =
            verify_attestation_with_keyring(&attestation, 7, "deadbeef", &active).unwrap();
        assert!(verdict.is_trusted());
        assert!(verdict.key_trusted && !verdict.key_revoked);

        // A revoked key: the signature still verifies, but the receipt is not honored.
        let revoked = AttestationKeyring {
            keys: vec![AttestationKey {
                key_id: Some("primary".to_string()),
                public_key,
                status: AttestationKeyStatus::Revoked,
            }],
        };
        let verdict =
            verify_attestation_with_keyring(&attestation, 7, "deadbeef", &revoked).unwrap();
        assert!(verdict.signature_valid && verdict.matches_tip);
        assert!(verdict.key_revoked && !verdict.is_trusted());

        // An unknown key (empty keyring): valid signature, but untrusted.
        let verdict = verify_attestation_with_keyring(
            &attestation,
            7,
            "deadbeef",
            &AttestationKeyring::default(),
        )
        .unwrap();
        assert!(verdict.signature_valid && !verdict.key_trusted && !verdict.is_trusted());
    }

    #[test]
    fn keyring_supports_rotation_to_a_new_key() {
        let old = sign_chain_tip(&test_seed(), 9, "cafe").unwrap();
        let new = sign_chain_tip(&rotated_seed(), 9, "cafe").unwrap();
        assert_ne!(old.public_key, new.public_key);

        // After rotation the new key is active and the old key is revoked.
        let keyring = AttestationKeyring {
            keys: vec![
                AttestationKey {
                    key_id: Some("retired".to_string()),
                    public_key: old.public_key.clone(),
                    status: AttestationKeyStatus::Revoked,
                },
                AttestationKey {
                    key_id: Some("current".to_string()),
                    public_key: new.public_key.clone(),
                    status: AttestationKeyStatus::Active,
                },
            ],
        };
        assert!(
            verify_attestation_with_keyring(&new, 9, "cafe", &keyring)
                .unwrap()
                .is_trusted()
        );
        assert!(
            !verify_attestation_with_keyring(&old, 9, "cafe", &keyring)
                .unwrap()
                .is_trusted()
        );
    }

    #[test]
    fn historical_checkpoint_requires_an_active_operator_key() {
        let path = temp_store("historical_checkpoint_keyring");
        let _ = std::fs::remove_file(&path);
        let mut memory = MemoryEngine::open(&path).expect("open test store");
        memory
            .remember("Hrafn retains the release decision.", vec![])
            .expect("record episode");
        let receipt = memory
            .attest_chain_tip(&"01".repeat(32))
            .expect("sign checkpoint")
            .expect("non-empty ledger");

        let unknown = memory
            .verify_attested_checkpoint_with_keyring(&receipt, &AttestationKeyring::default())
            .expect("evaluate unknown key");
        assert!(unknown.signature_valid);
        assert!(unknown.matches_history);
        assert!(!unknown.is_trusted());

        let active = AttestationKeyring {
            keys: vec![AttestationKey {
                key_id: Some("primary".to_string()),
                public_key: receipt.public_key.clone(),
                status: AttestationKeyStatus::Active,
            }],
        };
        assert!(
            memory
                .verify_attested_checkpoint_with_keyring(&receipt, &active)
                .expect("evaluate active key")
                .is_trusted()
        );

        let revoked = AttestationKeyring {
            keys: vec![AttestationKey {
                key_id: Some("primary".to_string()),
                public_key: receipt.public_key.clone(),
                status: AttestationKeyStatus::Revoked,
            }],
        };
        assert!(
            !memory
                .verify_attested_checkpoint_with_keyring(&receipt, &revoked)
                .expect("evaluate revoked key")
                .is_trusted()
        );

        drop(memory);
        let _ = std::fs::remove_file(path);
    }

    fn temp_store(label: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "nahuali_attestation_{label}_{}_{}",
            std::process::id(),
            nanos
        ))
    }
}
