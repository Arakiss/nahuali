//! Portable, offline-verifiable receipts for evidence-backed claims.
//!
//! A claim receipt carries only the authoritative ledger envelopes needed to
//! establish one provenance path (claim -> episode -> optional source), one
//! inclusion proof per envelope, and a signed ledger checkpoint. Verification
//! is deterministic and requires an independently held checkpoint policy. It
//! proves ledger commitment and provenance linkage, never factual truth,
//! authorship, source authenticity, or an externally witnessed timestamp.

use std::{collections::HashSet, fmt};

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use serde_json::Value;

use crate::checkpoint::{verify_checkpoint_authorization_v2, verify_checkpoint_events_v2};
use crate::{
    CheckpointMatchMode, CheckpointTrustPolicyV2, CheckpointVerificationOptionsV2,
    DEFAULT_CHECKPOINT_FUTURE_SKEW_MS, EVENT_ENVELOPE_VERSION, EventEnvelope, MemoryEvent,
    MerkleProof, NahualiError, Result, SignedLedgerCheckpointV2, ledger_inclusion_proof,
    verify_merkle_proof,
};

/// Stable receipt format identifier.
pub const MEMORY_CLAIM_RECEIPT_FORMAT: &str = "nahuali.claim-receipt";
/// Current portable claim-receipt version.
pub const MEMORY_CLAIM_RECEIPT_VERSION: u32 = 1;
/// Maximum JSON receipt size accepted by the strict parser (four MiB).
pub const MAX_CLAIM_RECEIPT_BYTES: usize = 4 * 1024 * 1024;
/// Inclusion paths cannot exceed the depth of a tree indexed by `usize`.
pub const MAX_RECEIPT_MERKLE_SIBLINGS: usize = usize::BITS as usize;

/// Verification clock and skew supplied by the offline verifier.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReceiptVerificationOptionsV1 {
    /// Verifier-observed Unix epoch time in milliseconds.
    pub verification_time_ms: u64,
    /// Maximum signer-clock lead accepted over `verification_time_ms`.
    pub max_future_skew_ms: u64,
}

impl ReceiptVerificationOptionsV1 {
    /// Use the same five-minute signer-clock tolerance as checkpoint v2.
    pub const fn at(verification_time_ms: u64) -> Self {
        Self {
            verification_time_ms,
            max_future_skew_ms: DEFAULT_CHECKPOINT_FUTURE_SKEW_MS,
        }
    }
}

/// One authoritative event envelope and its inclusion path to the checkpoint.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct ReceiptEventProofV1 {
    /// Exact record-ledger envelope, not a derived projection object.
    pub event: EventEnvelope,
    /// Inclusion path for `event.chain_hash()` under the signed root.
    pub inclusion_proof: MerkleProof,
}

/// Minimal portable evidence bundle for one directly asserted claim.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct MemoryClaimReceiptV1 {
    /// Stable format discriminator.
    pub format: String,
    /// Receipt schema version.
    pub version: u32,
    /// One externally authorized checkpoint shared by every inclusion proof.
    pub signed_checkpoint: SignedLedgerCheckpointV2,
    /// Direct `FactAsserted` ledger event.
    pub claim_event: ReceiptEventProofV1,
    /// `EpisodeRecorded` event named by the claim's `source_episode_id`.
    pub evidence_episode_event: ReceiptEventProofV1,
    /// `SourceRecorded` event named by the episode, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_event: Option<ReceiptEventProofV1>,
}

/// Exact scope of the content-authority statement returned by receipt v1.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptContentAuthorityClassV1 {
    /// The selected content is committed in an authorized ledger checkpoint;
    /// its truth and external provenance are not independently established.
    LedgerCommittedEvidence,
}

/// Explicit negative guarantees preventing cryptographic integrity from being
/// presented as factual or source authority.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReceiptContentAuthorityV1 {
    /// Narrow authority classification for receipt v1.
    pub classification: ReceiptContentAuthorityClassV1,
    /// Always false in v1: ledger inclusion does not establish claim truth.
    pub claim_truth_verified: bool,
    /// Always false in v1: the episode author is not independently verified.
    pub evidence_authorship_verified: bool,
    /// Always false in v1: source URI and metadata remain assertions.
    pub external_source_authenticity_verified: bool,
    /// Always false in v1: external source bytes are not bundled or rehashed.
    pub external_source_content_verified: bool,
}

/// Cryptographic and structural gates evaluated for a receipt.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReceiptIntegrityV1 {
    /// Composite fail-closed verdict over every field below.
    pub verified: bool,
    /// Whether the checkpoint satisfies the independently supplied policy.
    pub checkpoint_authorized: bool,
    /// Whether every selected event checksum recomputes exactly.
    pub selected_event_checksums_valid: bool,
    /// Whether every event id matches its sequence and checksum.
    pub selected_event_ids_valid: bool,
    /// Whether sequence, chain-link shape, and timestamp constraints hold.
    pub selected_event_envelopes_valid: bool,
    /// Whether every selected chain hash is included under the signed root.
    pub selected_chain_hashes_included: bool,
    /// Whether claim, episode, source, and chronology form one exact path.
    pub provenance_links_valid: bool,
    /// Signatures accepted by the external policy.
    pub accepted_signature_count: u64,
    /// Minimum active signatures required by the policy.
    pub minimum_signature_count: u32,
    /// Stable reasons for each failed gate or ignored checkpoint signature.
    pub reasons: Vec<String>,
}

/// Offline receipt verdict. Content authority is intentionally separate from
/// integrity so callers cannot collapse "committed" into "true".
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MemoryClaimReceiptVerificationV1 {
    /// Receipt format encountered by the verifier.
    pub format: String,
    /// Receipt version encountered by the verifier.
    pub version: u32,
    /// Number of selected events (two or three).
    pub selected_event_count: u32,
    /// Claim identifier carried by the fact payload, when structurally present.
    pub claim_id: String,
    /// Episode identifier carried by the episode payload, when present.
    pub evidence_episode_id: String,
    /// Source identifier carried by the episode, when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    /// Cryptographic and structural result.
    pub receipt_integrity: ReceiptIntegrityV1,
    /// Explicit statement of what the receipt does not establish.
    pub content_authority: ReceiptContentAuthorityV1,
}

/// Create a minimal receipt from a verified ledger checkpoint.
///
/// Only direct `FactAsserted` claims are supported by v1. Claims materialized
/// inside `RepairApplied` carry plural repair provenance and are rejected rather
/// than silently reduced to one episode.
pub fn create_claim_receipt_v1(
    events: &[EventEnvelope],
    claim_id: &str,
    signed_checkpoint: SignedLedgerCheckpointV2,
    policy: &CheckpointTrustPolicyV2,
    options: ReceiptVerificationOptionsV1,
) -> Result<MemoryClaimReceiptV1> {
    if claim_id.is_empty() || claim_id.trim() != claim_id {
        return Err(receipt_error(
            "claim id must be non-empty without surrounding whitespace",
        ));
    }

    let checkpoint_verdict = verify_checkpoint_events_v2(
        events,
        &signed_checkpoint,
        policy,
        CheckpointVerificationOptionsV2 {
            match_mode: CheckpointMatchMode::Historical,
            verification_time_ms: options.verification_time_ms,
            max_future_skew_ms: options.max_future_skew_ms,
        },
    )?;
    if !checkpoint_verdict.trusted {
        return Err(receipt_error(format!(
            "cannot export from an untrusted checkpoint: {}",
            checkpoint_verdict.reasons.join("; ")
        )));
    }

    let prefix_len = usize::try_from(signed_checkpoint.checkpoint.tree_size)
        .map_err(|_| receipt_error("checkpoint tree size exceeds this platform"))?;
    let prefix = events
        .get(..prefix_len)
        .ok_or_else(|| receipt_error("checkpoint prefix is not present in the ledger"))?;

    if prefix.iter().any(|event| {
        matches!(
            &event.payload,
            MemoryEvent::RepairApplied(repair) if repair.materialized_id() == claim_id
        )
    }) {
        return Err(receipt_error(
            "receipt v1 does not export claims materialized by RepairApplied",
        ));
    }

    let claim_event = unique_event(
        prefix,
        "claim",
        |event| matches!(&event.payload, MemoryEvent::FactAsserted(claim) if claim.id == claim_id),
    )?;
    let evidence_id = match &claim_event.payload {
        MemoryEvent::FactAsserted(claim) => {
            claim.source_episode_id.as_deref().ok_or_else(|| {
                receipt_error(format!(
                    "claim '{}' has no source_episode_id and cannot produce an evidence receipt",
                    claim.id
                ))
            })?
        }
        _ => unreachable!("unique_event predicate fixes the payload type"),
    };
    let episode_event = unique_event(
        prefix,
        "evidence episode",
        |event| matches!(&event.payload, MemoryEvent::EpisodeRecorded(episode) if episode.id == evidence_id),
    )?;
    if episode_event.sequence >= claim_event.sequence {
        return Err(receipt_error(
            "evidence episode must precede the claim in ledger order",
        ));
    }

    let source_event = match &episode_event.payload {
        MemoryEvent::EpisodeRecorded(episode) => match episode.source_id.as_deref() {
            Some(source_id) => {
                let source = unique_event(
                    prefix,
                    "source",
                    |event| matches!(&event.payload, MemoryEvent::SourceRecorded(recorded) if recorded.id == source_id),
                )?;
                if source.sequence >= episode_event.sequence {
                    return Err(receipt_error(
                        "source record must precede the evidence episode in ledger order",
                    ));
                }
                Some(source)
            }
            None => None,
        },
        _ => unreachable!("unique_event predicate fixes the payload type"),
    };

    let receipt = MemoryClaimReceiptV1 {
        format: MEMORY_CLAIM_RECEIPT_FORMAT.to_string(),
        version: MEMORY_CLAIM_RECEIPT_VERSION,
        claim_event: event_proof(prefix, claim_event, &signed_checkpoint.checkpoint.root_hash)?,
        evidence_episode_event: event_proof(
            prefix,
            episode_event,
            &signed_checkpoint.checkpoint.root_hash,
        )?,
        source_event: source_event
            .map(|event| event_proof(prefix, event, &signed_checkpoint.checkpoint.root_hash))
            .transpose()?,
        signed_checkpoint,
    };

    let verification = verify_claim_receipt_v1(&receipt, policy, options)?;
    if !verification.receipt_integrity.verified {
        return Err(receipt_error(format!(
            "generated receipt failed self-verification: {}",
            verification.receipt_integrity.reasons.join("; ")
        )));
    }
    Ok(receipt)
}

/// Parse a receipt with duplicate-key rejection and canonical event envelopes.
///
/// The public receipt type intentionally does not implement `Deserialize`:
/// callers must cross this strict parser before verification so nested unknown
/// fields, duplicate JSON keys, and explicit non-canonical null/default fields
/// cannot be silently discarded by permissive event deserialization.
pub fn parse_claim_receipt_v1(bytes: &[u8]) -> Result<MemoryClaimReceiptV1> {
    if bytes.len() > MAX_CLAIM_RECEIPT_BYTES {
        return Err(receipt_error(format!(
            "receipt exceeds the {MAX_CLAIM_RECEIPT_BYTES}-byte input limit"
        )));
    }
    let mut duplicate_check = serde_json::Deserializer::from_slice(bytes);
    DuplicateChecked::deserialize(&mut duplicate_check)
        .map_err(|error| receipt_error(format!("invalid receipt JSON: {error}")))?;
    duplicate_check
        .end()
        .map_err(|error| receipt_error(format!("invalid trailing receipt JSON: {error}")))?;
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| receipt_error(format!("invalid receipt JSON: {error}")))?;
    let raw: RawMemoryClaimReceiptV1 = serde_json::from_value(value)
        .map_err(|error| receipt_error(format!("invalid receipt document: {error}")))?;

    Ok(MemoryClaimReceiptV1 {
        format: raw.format,
        version: raw.version,
        signed_checkpoint: raw.signed_checkpoint,
        claim_event: raw.claim_event.try_into()?,
        evidence_episode_event: raw.evidence_episode_event.try_into()?,
        source_event: raw.source_event.map(TryInto::try_into).transpose()?,
    })
}

/// Verify a parsed receipt without opening a store or performing network I/O.
pub fn verify_claim_receipt_v1(
    receipt: &MemoryClaimReceiptV1,
    policy: &CheckpointTrustPolicyV2,
    options: ReceiptVerificationOptionsV1,
) -> Result<MemoryClaimReceiptVerificationV1> {
    let authorization = verify_checkpoint_authorization_v2(
        &receipt.signed_checkpoint,
        policy,
        options.verification_time_ms,
        options.max_future_skew_ms,
    )?;
    let checkpoint = &receipt.signed_checkpoint.checkpoint;
    let selected = selected_events(receipt);

    let format_valid = receipt.format == MEMORY_CLAIM_RECEIPT_FORMAT
        && receipt.version == MEMORY_CLAIM_RECEIPT_VERSION;
    let checksums_valid = selected.iter().all(|proof| proof.event.validate_checksum());
    let ids_valid = selected.iter().all(|proof| event_id_valid(&proof.event));
    let envelopes_valid = selected
        .iter()
        .all(|proof| envelope_shape_valid(&proof.event, checkpoint.generated_at_ms))
        && selected_identity_is_distinct(&selected);
    let inclusions_valid = selected
        .iter()
        .all(|proof| inclusion_valid(proof, checkpoint.tree_size, checkpoint.root_hash.as_str()));
    let (provenance_links_valid, claim_id, evidence_episode_id, source_id) =
        provenance_path(receipt);

    let mut reasons = authorization
        .reasons
        .iter()
        .map(|reason| format!("checkpoint: {reason}"))
        .collect::<Vec<_>>();
    if !format_valid {
        reasons.push("receipt format or version is unsupported".to_string());
    }
    if !checksums_valid {
        reasons.push("one or more selected event checksums are invalid".to_string());
    }
    if !ids_valid {
        reasons
            .push("one or more selected event ids do not match sequence and checksum".to_string());
    }
    if !envelopes_valid {
        reasons.push(
            "one or more selected event envelopes have invalid sequence, chain-link, timestamp, or identity shape"
                .to_string(),
        );
    }
    if !inclusions_valid {
        reasons.push(
            "one or more selected event chain hashes are not included under the signed checkpoint root"
                .to_string(),
        );
    }
    if !provenance_links_valid {
        reasons.push(
            "claim, evidence episode, optional source, and ledger chronology do not form one exact provenance path"
                .to_string(),
        );
    }

    let verified = format_valid
        && authorization.authorized
        && checksums_valid
        && ids_valid
        && envelopes_valid
        && inclusions_valid
        && provenance_links_valid;

    Ok(MemoryClaimReceiptVerificationV1 {
        format: receipt.format.clone(),
        version: receipt.version,
        selected_event_count: u32::try_from(selected.len()).unwrap_or(u32::MAX),
        claim_id,
        evidence_episode_id,
        source_id,
        receipt_integrity: ReceiptIntegrityV1 {
            verified,
            checkpoint_authorized: authorization.authorized,
            selected_event_checksums_valid: checksums_valid,
            selected_event_ids_valid: ids_valid,
            selected_event_envelopes_valid: envelopes_valid,
            selected_chain_hashes_included: inclusions_valid,
            provenance_links_valid,
            accepted_signature_count: authorization.accepted_signature_count,
            minimum_signature_count: authorization.minimum_signature_count,
            reasons,
        },
        content_authority: ReceiptContentAuthorityV1 {
            classification: ReceiptContentAuthorityClassV1::LedgerCommittedEvidence,
            claim_truth_verified: false,
            evidence_authorship_verified: false,
            external_source_authenticity_verified: false,
            external_source_content_verified: false,
        },
    })
}

fn selected_events(receipt: &MemoryClaimReceiptV1) -> Vec<&ReceiptEventProofV1> {
    let mut selected = vec![&receipt.claim_event, &receipt.evidence_episode_event];
    if let Some(source) = receipt.source_event.as_ref() {
        selected.push(source);
    }
    selected
}

fn unique_event<'a>(
    events: &'a [EventEnvelope],
    role: &str,
    matches: impl Fn(&EventEnvelope) -> bool,
) -> Result<&'a EventEnvelope> {
    let mut matching = events.iter().filter(|event| matches(event));
    let event = matching
        .next()
        .ok_or_else(|| receipt_error(format!("checkpoint prefix has no matching {role} event")))?;
    if matching.next().is_some() {
        return Err(receipt_error(format!(
            "checkpoint prefix has more than one matching {role} event"
        )));
    }
    Ok(event)
}

fn event_proof(
    prefix: &[EventEnvelope],
    event: &EventEnvelope,
    root_hash: &str,
) -> Result<ReceiptEventProofV1> {
    if !event_id_valid(event) {
        return Err(receipt_error(format!(
            "selected event at sequence {} has a non-canonical id",
            event.sequence
        )));
    }
    let index = event
        .sequence
        .checked_sub(1)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| receipt_error("selected event sequence cannot index this platform"))?;
    let inclusion_proof = ledger_inclusion_proof(prefix, index).ok_or_else(|| {
        receipt_error(format!(
            "could not build inclusion proof for selected sequence {}",
            event.sequence
        ))
    })?;
    if !verify_merkle_proof(&event.chain_hash(), &inclusion_proof, root_hash) {
        return Err(receipt_error(format!(
            "generated inclusion proof failed for selected sequence {}",
            event.sequence
        )));
    }
    Ok(ReceiptEventProofV1 {
        event: event.clone(),
        inclusion_proof,
    })
}

fn event_id_valid(event: &EventEnvelope) -> bool {
    event.id == format!("event_{}_{}", event.sequence, event.checksum)
}

fn envelope_shape_valid(event: &EventEnvelope, checkpoint_time_ms: u64) -> bool {
    if event.version > EVENT_ENVELOPE_VERSION
        || event.sequence == 0
        || event.timestamp_ms > checkpoint_time_ms
    {
        return false;
    }
    match (event.sequence, event.prev_hash.as_deref()) {
        (1, Some("")) => true,
        (1, _) => false,
        (_, Some(previous)) => canonical_sha256(previous),
        (_, None) => false,
    }
}

fn selected_identity_is_distinct(selected: &[&ReceiptEventProofV1]) -> bool {
    let mut ids = HashSet::new();
    let mut sequences = HashSet::new();
    selected
        .iter()
        .all(|proof| ids.insert(proof.event.id.as_str()) && sequences.insert(proof.event.sequence))
}

fn inclusion_valid(
    proof: &ReceiptEventProofV1,
    checkpoint_tree_size: u64,
    checkpoint_root: &str,
) -> bool {
    let Ok(tree_size) = usize::try_from(checkpoint_tree_size) else {
        return false;
    };
    let Some(expected_index) = proof
        .event
        .sequence
        .checked_sub(1)
        .and_then(|sequence| usize::try_from(sequence).ok())
    else {
        return false;
    };
    proof.inclusion_proof.leaf_count == tree_size
        && proof.inclusion_proof.index == expected_index
        && proof.inclusion_proof.siblings.len() <= MAX_RECEIPT_MERKLE_SIBLINGS
        && proof
            .inclusion_proof
            .siblings
            .iter()
            .all(|sibling| canonical_sha256(&sibling.hash))
        && verify_merkle_proof(
            &proof.event.chain_hash(),
            &proof.inclusion_proof,
            checkpoint_root,
        )
}

fn provenance_path(receipt: &MemoryClaimReceiptV1) -> (bool, String, String, Option<String>) {
    let claim = match &receipt.claim_event.event.payload {
        MemoryEvent::FactAsserted(claim) => claim,
        _ => return (false, String::new(), String::new(), None),
    };
    let episode = match &receipt.evidence_episode_event.event.payload {
        MemoryEvent::EpisodeRecorded(episode) => episode,
        _ => return (false, claim.id.clone(), String::new(), None),
    };
    let evidence_matches = claim.source_episode_id.as_deref() == Some(episode.id.as_str());
    let chronology_valid =
        receipt.evidence_episode_event.event.sequence < receipt.claim_event.event.sequence;
    let source_valid = match (episode.source_id.as_deref(), receipt.source_event.as_ref()) {
        (None, None) => true,
        (Some(expected), Some(proof)) => match &proof.event.payload {
            MemoryEvent::SourceRecorded(source) => {
                source.id == expected
                    && proof.event.sequence < receipt.evidence_episode_event.event.sequence
            }
            _ => false,
        },
        _ => false,
    };
    (
        evidence_matches && chronology_valid && source_valid,
        claim.id.clone(),
        episode.id.clone(),
        episode.source_id.clone(),
    )
}

fn canonical_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMemoryClaimReceiptV1 {
    format: String,
    version: u32,
    signed_checkpoint: SignedLedgerCheckpointV2,
    claim_event: RawReceiptEventProofV1,
    evidence_episode_event: RawReceiptEventProofV1,
    #[serde(default)]
    source_event: Option<RawReceiptEventProofV1>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawReceiptEventProofV1 {
    event: Value,
    inclusion_proof: MerkleProof,
}

impl TryFrom<RawReceiptEventProofV1> for ReceiptEventProofV1 {
    type Error = NahualiError;

    fn try_from(raw: RawReceiptEventProofV1) -> Result<Self> {
        let event: EventEnvelope = serde_json::from_value(raw.event.clone())
            .map_err(|error| receipt_error(format!("invalid selected event envelope: {error}")))?;
        let canonical_bytes = serde_json::to_vec(&event).map_err(|error| {
            receipt_error(format!(
                "could not canonicalize selected event envelope: {error}"
            ))
        })?;
        let canonical: Value = serde_json::from_slice(&canonical_bytes).map_err(|error| {
            receipt_error(format!(
                "could not parse canonical selected event envelope: {error}"
            ))
        })?;
        if let Some(path) = first_json_difference_path(&raw.event, &canonical, "$") {
            return Err(receipt_error(format!(
                "selected event envelope contains unknown or non-canonical fields at {path}"
            )));
        }
        Ok(Self {
            event,
            inclusion_proof: raw.inclusion_proof,
        })
    }
}

fn first_json_difference_path(left: &Value, right: &Value, path: &str) -> Option<String> {
    if left == right {
        return None;
    }
    match (left, right) {
        (Value::Object(left), Value::Object(right)) => {
            for key in left.keys().chain(right.keys()) {
                match (left.get(key), right.get(key)) {
                    (Some(left), Some(right)) => {
                        if let Some(found) =
                            first_json_difference_path(left, right, &format!("{path}.{key}"))
                        {
                            return Some(found);
                        }
                    }
                    _ => return Some(format!("{path}.{key}")),
                }
            }
            None
        }
        (Value::Array(left), Value::Array(right)) => {
            for index in 0..left.len().max(right.len()) {
                match (left.get(index), right.get(index)) {
                    (Some(left), Some(right)) => {
                        if let Some(found) =
                            first_json_difference_path(left, right, &format!("{path}[{index}]"))
                        {
                            return Some(found);
                        }
                    }
                    _ => return Some(format!("{path}[{index}]")),
                }
            }
            None
        }
        _ => Some(path.to_string()),
    }
}

struct DuplicateChecked;

impl<'de> Deserialize<'de> for DuplicateChecked {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(DuplicateCheckVisitor)
    }
}

struct DuplicateCheckVisitor;

impl<'de> Visitor<'de> for DuplicateCheckVisitor {
    type Value = DuplicateChecked;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("JSON without duplicate object keys")
    }

    fn visit_bool<E>(self, value: bool) -> std::result::Result<Self::Value, E> {
        let _ = value;
        Ok(DuplicateChecked)
    }

    fn visit_i64<E>(self, value: i64) -> std::result::Result<Self::Value, E> {
        let _ = value;
        Ok(DuplicateChecked)
    }

    fn visit_u64<E>(self, value: u64) -> std::result::Result<Self::Value, E> {
        let _ = value;
        Ok(DuplicateChecked)
    }

    fn visit_f64<E>(self, value: f64) -> std::result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        if value.is_finite() {
            Ok(DuplicateChecked)
        } else {
            Err(E::custom("JSON number must be finite"))
        }
    }

    fn visit_str<E>(self, value: &str) -> std::result::Result<Self::Value, E> {
        let _ = value;
        Ok(DuplicateChecked)
    }

    fn visit_string<E>(self, value: String) -> std::result::Result<Self::Value, E> {
        let _ = value;
        Ok(DuplicateChecked)
    }

    fn visit_none<E>(self) -> std::result::Result<Self::Value, E> {
        Ok(DuplicateChecked)
    }

    fn visit_unit<E>(self) -> std::result::Result<Self::Value, E> {
        Ok(DuplicateChecked)
    }

    fn visit_some<D>(self, deserializer: D) -> std::result::Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        DuplicateChecked::deserialize(deserializer)
    }

    fn visit_seq<A>(self, mut sequence: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while sequence.next_element::<DuplicateChecked>()?.is_some() {}
        Ok(DuplicateChecked)
    }

    fn visit_map<A>(self, mut object: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut keys = HashSet::new();
        while let Some(key) = object.next_key::<String>()? {
            if !keys.insert(key.clone()) {
                return Err(de::Error::custom(format!(
                    "duplicate JSON object key '{key}'"
                )));
            }
            object.next_value::<DuplicateChecked>()?;
        }
        Ok(DuplicateChecked)
    }
}

fn receipt_error(message: impl Into<String>) -> NahualiError {
    NahualiError::ClaimReceipt {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::checkpoint::derive_ledger_id;
    use crate::{
        CheckpointHashAlgorithm, CheckpointTreeAlgorithm, EpisodeRecorded, FactAsserted,
        LEDGER_CHECKPOINT_VERSION, LedgerCheckpointV2, SourceRecorded, SourceRecordedKind,
        add_checkpoint_signature_v2, checkpoint_policy_key_v2, ledger_merkle_root,
        sign_checkpoint_v2,
    };

    const ORIGIN: &str = "nahuali.local/tests/claim-receipt";

    struct Fixture {
        events: Vec<EventEnvelope>,
        claim_id: String,
        signed: SignedLedgerCheckpointV2,
        policy: CheckpointTrustPolicyV2,
        verification_time_ms: u64,
    }

    fn append_event(events: &mut Vec<EventEnvelope>, timestamp_ms: u64, payload: MemoryEvent) {
        let sequence = u64::try_from(events.len()).expect("fixture event count fits u64") + 1;
        let previous = events.last().map(EventEnvelope::chain_hash);
        events.push(EventEnvelope::with_chain(
            sequence,
            timestamp_ms,
            payload,
            previous.as_deref(),
        ));
    }

    fn fixture(label: &str, with_source: bool, threshold: u32) -> Fixture {
        let source_id = format!("source_{label}");
        let episode_id = format!("episode_{label}");
        let claim_id = format!("claim_{label}");
        let mut events = Vec::new();
        let mut timestamp_ms = 1_000;

        if with_source {
            append_event(
                &mut events,
                timestamp_ms,
                MemoryEvent::SourceRecorded(SourceRecorded {
                    id: source_id.clone(),
                    kind: SourceRecordedKind::Document,
                    title: Some("Hrafn evidence".to_string()),
                    uri: Some("file:///private/evidence.txt".to_string()),
                    content_checksum: "fixture-source-checksum".to_string(),
                    byte_len: 21,
                    metadata: BTreeMap::from([("author".to_string(), "operator".to_string())]),
                    scope: None,
                }),
            );
            timestamp_ms += 1_000;
        }

        append_event(
            &mut events,
            timestamp_ms,
            MemoryEvent::EpisodeRecorded(EpisodeRecorded {
                id: episode_id.clone(),
                content: "Hrafn observed the checkpoint decision.".to_string(),
                tags: vec!["evidence".to_string()],
                mentions: vec!["Hrafn".to_string()],
                source_id: with_source.then_some(source_id),
                source_position: with_source.then_some(1),
                source_role: with_source.then_some("operator".to_string()),
                scope: None,
            }),
        );
        timestamp_ms += 1_000;
        append_event(
            &mut events,
            timestamp_ms,
            MemoryEvent::FactAsserted(FactAsserted {
                id: claim_id.clone(),
                subject: "Hrafn".to_string(),
                predicate: "retains".to_string(),
                object: "the checkpoint decision".to_string(),
                source_episode_id: Some(episode_id),
                confidence: 0.95,
                scope: None,
            }),
        );
        let verification_time_ms = timestamp_ms;
        let keys = [1u8, 2]
            .into_iter()
            .take(threshold as usize)
            .enumerate()
            .map(|(index, byte)| {
                checkpoint_policy_key_v2(&format!("operator-{index}"), &seed(byte))
                    .expect("derive checkpoint policy key")
            })
            .collect::<Vec<_>>();
        let ledger_id = derive_ledger_id(&events)
            .expect("derive fixture ledger id")
            .expect("fixture ledger is not empty");
        let policy =
            CheckpointTrustPolicyV2::new(ORIGIN.to_string(), ledger_id.clone(), threshold, keys)
                .expect("create checkpoint policy");
        let checkpoint = LedgerCheckpointV2 {
            version: LEDGER_CHECKPOINT_VERSION,
            origin: ORIGIN.to_string(),
            ledger_id,
            tree_algorithm: CheckpointTreeAlgorithm::NahualiMerkleV1,
            hash_algorithm: CheckpointHashAlgorithm::Sha256,
            tree_size: u64::try_from(events.len()).expect("fixture event count fits u64"),
            root_hash: ledger_merkle_root(&events).expect("fixture Merkle root"),
            chain_tip: events.last().expect("fixture event exists").chain_hash(),
            generated_at_ms: verification_time_ms,
        };
        let mut signed = sign_checkpoint_v2(checkpoint, &policy, "operator-0", &seed(1))
            .expect("sign checkpoint");
        if threshold == 2 {
            add_checkpoint_signature_v2(&mut signed, &policy, "operator-1", &seed(2))
                .expect("add second checkpoint signature");
        }
        Fixture {
            events,
            claim_id,
            signed,
            policy,
            verification_time_ms,
        }
    }

    fn seed(byte: u8) -> String {
        format!("{byte:02x}").repeat(32)
    }

    fn options(fixture: &Fixture) -> ReceiptVerificationOptionsV1 {
        ReceiptVerificationOptionsV1 {
            verification_time_ms: fixture.verification_time_ms,
            max_future_skew_ms: 0,
        }
    }

    fn receipt(fixture: &Fixture) -> MemoryClaimReceiptV1 {
        create_claim_receipt_v1(
            &fixture.events,
            &fixture.claim_id,
            fixture.signed.clone(),
            &fixture.policy,
            options(fixture),
        )
        .expect("create claim receipt")
    }

    #[test]
    fn local_and_sourced_claims_verify_with_explicitly_limited_authority() {
        for (with_source, threshold) in [(false, 1), (true, 2)] {
            let fixture = fixture(
                if with_source { "sourced" } else { "local" },
                with_source,
                threshold,
            );
            let receipt = receipt(&fixture);
            let encoded = serde_json::to_vec_pretty(&receipt).expect("serialize receipt");
            let parsed = parse_claim_receipt_v1(&encoded).expect("strictly parse receipt");
            let verdict = verify_claim_receipt_v1(&parsed, &fixture.policy, options(&fixture))
                .expect("verify receipt");

            assert!(verdict.receipt_integrity.verified);
            assert!(verdict.receipt_integrity.checkpoint_authorized);
            assert_eq!(
                verdict.selected_event_count,
                if with_source { 3 } else { 2 }
            );
            assert_eq!(verdict.source_id.is_some(), with_source);
            assert_eq!(
                verdict.receipt_integrity.accepted_signature_count,
                u64::from(threshold)
            );
            assert_eq!(
                verdict.content_authority.classification,
                ReceiptContentAuthorityClassV1::LedgerCommittedEvidence
            );
            assert!(!verdict.content_authority.claim_truth_verified);
            assert!(!verdict.content_authority.evidence_authorship_verified);
            assert!(
                !verdict
                    .content_authority
                    .external_source_authenticity_verified
            );
            assert!(!verdict.content_authority.external_source_content_verified);
        }
    }

    #[test]
    fn historical_checkpoint_exports_after_later_unrelated_appends() {
        let mut fixture = fixture("historical", true, 1);
        let timestamp_ms = fixture.verification_time_ms + 1_000;
        append_event(
            &mut fixture.events,
            timestamp_ms,
            MemoryEvent::EpisodeRecorded(EpisodeRecorded {
                id: "episode_later".to_string(),
                content: "Later unrelated private memory.".to_string(),
                tags: Vec::new(),
                mentions: Vec::new(),
                source_id: None,
                source_position: None,
                source_role: None,
                scope: None,
            }),
        );
        let receipt = receipt(&fixture);
        let encoded = serde_json::to_string(&receipt).expect("serialize receipt");

        assert!(!encoded.contains("Later unrelated private memory"));
        assert!(
            verify_claim_receipt_v1(&receipt, &fixture.policy, options(&fixture))
                .expect("verify historical receipt")
                .receipt_integrity
                .verified
        );
    }

    #[test]
    fn mutations_of_content_proofs_links_and_checkpoint_fail_closed() {
        let fixture = fixture("mutations", true, 1);
        let original = receipt(&fixture);

        let mut mutations = Vec::new();

        let mut claim_content = original.clone();
        if let MemoryEvent::FactAsserted(claim) = &mut claim_content.claim_event.event.payload {
            claim.object.push_str(" rewritten");
        }
        mutations.push(claim_content);

        let mut episode_content = original.clone();
        if let MemoryEvent::EpisodeRecorded(episode) =
            &mut episode_content.evidence_episode_event.event.payload
        {
            episode.content.push_str(" rewritten");
        }
        mutations.push(episode_content);

        let mut source_content = original.clone();
        if let Some(source_proof) = source_content.source_event.as_mut()
            && let MemoryEvent::SourceRecorded(source) = &mut source_proof.event.payload
        {
            source.content_checksum.push('0');
        }
        mutations.push(source_content);

        let mut envelope_id = original.clone();
        envelope_id.claim_event.event.id.push_str("_forged");
        mutations.push(envelope_id);

        let mut envelope_link = original.clone();
        envelope_link.claim_event.event.prev_hash = Some("00".repeat(32));
        mutations.push(envelope_link);

        let mut future_envelope = original.clone();
        future_envelope.claim_event.event.version = EVENT_ENVELOPE_VERSION + 1;
        assert!(!envelope_shape_valid(
            &future_envelope.claim_event.event,
            fixture.signed.checkpoint.generated_at_ms
        ));
        mutations.push(future_envelope);

        let mut proof_index = original.clone();
        proof_index.claim_event.inclusion_proof.index = 0;
        mutations.push(proof_index);

        let mut proof_size = original.clone();
        proof_size.claim_event.inclusion_proof.leaf_count += 1;
        mutations.push(proof_size);

        let mut proof_hash = original.clone();
        proof_hash.claim_event.inclusion_proof.siblings[0]
            .hash
            .replace_range(0..2, "ff");
        mutations.push(proof_hash);

        let mut checkpoint_root = original.clone();
        checkpoint_root
            .signed_checkpoint
            .checkpoint
            .root_hash
            .replace_range(0..2, "ff");
        mutations.push(checkpoint_root);

        let mut provenance = original.clone();
        if let MemoryEvent::FactAsserted(claim) = &mut provenance.claim_event.event.payload {
            claim.source_episode_id = Some("episode_other".to_string());
        }
        mutations.push(provenance);

        let mut missing_source = original.clone();
        missing_source.source_event = None;
        mutations.push(missing_source);

        for (index, mutation) in mutations.iter().enumerate() {
            let rejected = verify_claim_receipt_v1(mutation, &fixture.policy, options(&fixture))
                .map(|verdict| !verdict.receipt_integrity.verified)
                .unwrap_or(true);
            assert!(rejected, "mutation {index} must fail closed");
        }
    }

    #[test]
    fn strict_parser_rejects_unknown_duplicate_noncanonical_and_oversized_json() {
        let fixture = fixture("strict-json", false, 1);
        let receipt = receipt(&fixture);
        let encoded = serde_json::to_string_pretty(&receipt).expect("serialize receipt");

        let duplicate =
            encoded.replacen("\"version\": 1,", "\"version\": 1,\n  \"version\": 1,", 1);
        assert!(parse_claim_receipt_v1(duplicate.as_bytes()).is_err());

        let mut unknown: Value = serde_json::from_str(&encoded).expect("receipt value");
        unknown["claim_event"]["event"]["hidden"] = Value::Bool(true);
        assert!(
            parse_claim_receipt_v1(&serde_json::to_vec(&unknown).expect("encode unknown field"))
                .is_err()
        );

        let mut noncanonical_null: Value =
            serde_json::from_str(&encoded).expect("receipt value for null");
        noncanonical_null["claim_event"]["event"]["payload"]["scope"] = Value::Null;
        assert!(
            parse_claim_receipt_v1(
                &serde_json::to_vec(&noncanonical_null).expect("encode noncanonical null")
            )
            .is_err()
        );

        let mut noncanonical_precision: Value =
            serde_json::from_str(&encoded).expect("receipt value for precision");
        noncanonical_precision["claim_event"]["event"]["payload"]["confidence"] =
            serde_json::from_str("0.9500000001").expect("high-precision JSON number");
        assert!(
            parse_claim_receipt_v1(
                &serde_json::to_vec(&noncanonical_precision)
                    .expect("encode noncanonical precision")
            )
            .is_err()
        );

        assert!(parse_claim_receipt_v1(&vec![b' '; MAX_CLAIM_RECEIPT_BYTES + 1]).is_err());
    }

    #[test]
    fn receipt_excludes_policy_keys_seeds_and_unrelated_events() {
        let mut fixture = fixture("privacy", true, 1);
        let timestamp_ms = fixture.verification_time_ms + 1_000;
        append_event(
            &mut fixture.events,
            timestamp_ms,
            MemoryEvent::EpisodeRecorded(EpisodeRecorded {
                id: "episode_private".to_string(),
                content: "Unrelated secret-shaped text must stay out.".to_string(),
                tags: Vec::new(),
                mentions: Vec::new(),
                source_id: None,
                source_position: None,
                source_role: None,
                scope: None,
            }),
        );
        let receipt = receipt(&fixture);
        let encoded = serde_json::to_string(&receipt).expect("serialize receipt");

        assert!(!encoded.contains("Unrelated secret-shaped text"));
        assert!(!encoded.contains(&seed(1)));
        assert!(!encoded.contains("public_key"));
        assert!(receipt.source_event.is_some());
    }
}
