use std::path::Path;

use anyhow::Context;
use nahuali_core::{AttestationKeyring, LedgerAttestation, MemoryEngine};

/// Sign the current chain tip with an Ed25519 seed read from `key_file`.
///
/// The seed file holds a 32-byte Ed25519 seed as hex (generate one with, e.g.,
/// `openssl rand -hex 32`). The resulting attestation is written to `output`
/// when given and printed to stdout.
pub(crate) fn sign(
    memory: &mut MemoryEngine,
    key_file: &Path,
    output: Option<&Path>,
    json: bool,
) -> anyhow::Result<()> {
    let seed = std::fs::read_to_string(key_file)
        .with_context(|| format!("failed to read signing key file {}", key_file.display()))?;
    let attestation = memory
        .attest_chain_tip(seed.trim())?
        .context("the ledger is empty; there is no chain tip to attest")?;
    let serialized = serde_json::to_string_pretty(&attestation)?;

    if let Some(output) = output {
        std::fs::write(output, format!("{serialized}\n"))
            .with_context(|| format!("failed to write attestation to {}", output.display()))?;
    }

    if json {
        println!("{serialized}");
    } else {
        println!("Signed the tamper-evident chain tip:");
        println!("  sequence:   {}", attestation.sequence);
        println!("  tip:        {}", attestation.tip);
        println!("  public key: {}", attestation.public_key);
        match output {
            Some(output) => println!("  written to: {}", output.display()),
            None => println!("  (keep this attestation outside the store as a receipt)"),
        }
    }
    Ok(())
}

/// Verify a previously signed attestation against the current ledger tip.
///
/// Exits non-zero when the attestation does not vouch for the current ledger
/// (a moved tip or an invalid signature), so it can gate scripts and CI.
pub(crate) fn verify(
    memory: &mut MemoryEngine,
    attestation_path: &Path,
    keyring_path: Option<&Path>,
    json: bool,
) -> anyhow::Result<()> {
    let raw = std::fs::read_to_string(attestation_path).with_context(|| {
        format!(
            "failed to read attestation document {}",
            attestation_path.display()
        )
    })?;
    let attestation: LedgerAttestation = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse attestation {}", attestation_path.display()))?;

    if let Some(keyring_path) = keyring_path {
        return verify_with_keyring(memory, &attestation, keyring_path, json);
    }

    let verdict = memory.verify_chain_tip_attestation(&attestation)?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "valid": verdict.is_valid(),
                "matches_tip": verdict.matches_tip,
                "signature_valid": verdict.signature_valid,
                "attested_sequence": attestation.sequence,
                "attested_tip": attestation.tip,
                "current_sequence": verdict.current_sequence,
                "current_tip": verdict.current_tip,
            }))?
        );
    } else if verdict.is_valid() {
        println!("VALID: the signed tip matches the current ledger and the signature verifies.");
    } else if !verdict.matches_tip {
        println!("STALE: the ledger tip has moved since this attestation was signed.");
        println!(
            "  attested tip: seq {} {}",
            attestation.sequence, attestation.tip
        );
        match (verdict.current_sequence, &verdict.current_tip) {
            (Some(sequence), Some(tip)) => println!("  current tip:  seq {sequence} {tip}"),
            _ => println!("  current tip:  (empty ledger)"),
        }
    } else {
        println!("INVALID: the signature does not verify against the attested tip.");
    }

    if !verdict.is_valid() {
        anyhow::bail!("attestation does not vouch for the current ledger");
    }
    Ok(())
}

/// Verify a receipt against the live ledger AND an operator keyring, so a
/// revoked or unknown signing key is rejected even when its signature verifies.
fn verify_with_keyring(
    memory: &MemoryEngine,
    attestation: &LedgerAttestation,
    keyring_path: &Path,
    json: bool,
) -> anyhow::Result<()> {
    let raw = std::fs::read_to_string(keyring_path)
        .with_context(|| format!("failed to read keyring {}", keyring_path.display()))?;
    let keyring: AttestationKeyring = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse keyring {}", keyring_path.display()))?;
    let verdict = memory.verify_chain_tip_attestation_with_keyring(attestation, &keyring)?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "trusted": verdict.is_trusted(),
                "matches_tip": verdict.matches_tip,
                "signature_valid": verdict.signature_valid,
                "key_trusted": verdict.key_trusted,
                "key_revoked": verdict.key_revoked,
                "attested_sequence": attestation.sequence,
                "attested_tip": attestation.tip,
            }))?
        );
    } else if verdict.is_trusted() {
        println!(
            "TRUSTED: the receipt matches the current ledger, the signature verifies, and the key is active in the keyring."
        );
    } else if verdict.key_revoked {
        println!("REJECTED: the signing key is revoked in the keyring.");
    } else if !verdict.key_trusted {
        println!("REJECTED: the signing key is not present in the keyring.");
    } else if !verdict.matches_tip {
        println!("STALE: the ledger tip has moved since this attestation was signed.");
    } else {
        println!("INVALID: the signature does not verify against the attested tip.");
    }

    if !verdict.is_trusted() {
        anyhow::bail!("attestation is not trusted under the supplied keyring");
    }
    Ok(())
}
