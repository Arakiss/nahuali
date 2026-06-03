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

/// Verdict for whether a signed attestation anchors a point in this ledger's
/// own history, so an audit can diff changes since a trusted checkpoint.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttestedCheckpointVerdict {
    /// The signature over the attested `(sequence, tip)` pair verifies.
    pub signature_valid: bool,
    /// This ledger's chain hash at the attested sequence equals the attested tip,
    /// so the checkpoint and the history up to it are unchanged.
    pub matches_history: bool,
    /// The checkpoint is genuine: the signature verifies and it sits in this
    /// ledger's unaltered history.
    pub anchored: bool,
    /// The attested sequence the checkpoint anchors.
    pub sequence: u64,
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

    /// Check whether an attestation anchors a verified checkpoint in this
    /// ledger's own history, rather than only the current tip. Unlike
    /// [`Self::verify_chain_tip_attestation`], the attested sequence need not be
    /// the latest event: the audit path uses this to diff changes appended since
    /// a past signed checkpoint, only when that checkpoint is genuine.
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
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from_digit((byte >> 4) as u32, 16).expect("nibble is a hex digit"));
        out.push(char::from_digit((byte & 0x0f) as u32, 16).expect("nibble is a hex digit"));
    }
    out
}

fn decode_fixed<const N: usize>(hex: &str, label: &str) -> Result<[u8; N]> {
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
    }
}
