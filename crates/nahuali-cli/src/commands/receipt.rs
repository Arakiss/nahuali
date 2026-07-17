use std::{fs::File, io::Read, path::Path};

use anyhow::{Context, ensure};
use nahuali_core::{
    CheckpointTrustPolicyV2, MAX_CLAIM_RECEIPT_BYTES, MemoryEngine, ReceiptVerificationOptionsV1,
    SignedLedgerCheckpointV2, create_claim_receipt_v1, parse_claim_receipt_v1,
    verify_claim_receipt_v1,
};

use super::checkpoint::{now_ms, read_json_bounded, write_json_new};

pub(crate) fn export(
    memory: &mut MemoryEngine,
    claim_id: &str,
    checkpoint_path: &Path,
    policy_path: &Path,
    output: &Path,
) -> anyhow::Result<()> {
    let signed_checkpoint: SignedLedgerCheckpointV2 =
        read_json_bounded(checkpoint_path, "signed ledger checkpoint")?;
    let policy: CheckpointTrustPolicyV2 =
        read_json_bounded(policy_path, "checkpoint trust policy")?;
    policy
        .validate()
        .context("validate external checkpoint policy")?;
    let receipt = create_claim_receipt_v1(
        memory.events(),
        claim_id,
        signed_checkpoint,
        &policy,
        ReceiptVerificationOptionsV1::at(now_ms()?),
    )
    .context("create offline claim receipt")?;
    let selected_event_count = 2 + usize::from(receipt.source_event.is_some());
    write_json_new(output, &receipt, "claim receipt")?;

    println!(
        "Created claim receipt '{}' with {selected_event_count} selected event(s) at {}",
        claim_id,
        output.display()
    );
    println!("Content authority: ledger-committed evidence only");
    Ok(())
}

pub(crate) fn verify(
    receipt_path: &Path,
    policy_path: &Path,
    verification_time_ms: Option<u64>,
    max_future_skew_ms: u64,
    json: bool,
) -> anyhow::Result<()> {
    let receipt_bytes = read_receipt_bounded(receipt_path)?;
    let receipt = parse_claim_receipt_v1(&receipt_bytes).context("parse strict claim receipt")?;
    let policy: CheckpointTrustPolicyV2 =
        read_json_bounded(policy_path, "checkpoint trust policy")?;
    policy
        .validate()
        .context("validate external checkpoint policy")?;
    let verdict = verify_claim_receipt_v1(
        &receipt,
        &policy,
        ReceiptVerificationOptionsV1 {
            verification_time_ms: verification_time_ms.unwrap_or(now_ms()?),
            max_future_skew_ms,
        },
    )
    .context("verify claim receipt")?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&verdict).context("serialize claim receipt verdict")?
        );
    } else if verdict.receipt_integrity.verified {
        println!("VERIFIED CLAIM RECEIPT");
        println!("Selected events: {}", verdict.selected_event_count);
        println!(
            "Checkpoint signatures: {}/{}",
            verdict.receipt_integrity.accepted_signature_count,
            verdict.receipt_integrity.minimum_signature_count
        );
        println!("Ledger commitment: verified");
        println!("Content authority: ledger-committed evidence only");
        println!("Claim truth: not established");
        println!("External source authenticity: not established");
    } else {
        println!("UNVERIFIED CLAIM RECEIPT");
        println!("Selected events: {}", verdict.selected_event_count);
        for reason in &verdict.receipt_integrity.reasons {
            println!("- {reason}");
        }
        println!("Claim truth: not established");
        println!("External source authenticity: not established");
    }

    ensure!(
        verdict.receipt_integrity.verified,
        "claim receipt integrity is not verified under the supplied external policy"
    );
    Ok(())
}

fn read_receipt_bounded(path: &Path) -> anyhow::Result<Vec<u8>> {
    let file =
        File::open(path).with_context(|| format!("open claim receipt at '{}'", path.display()))?;
    let limit = u64::try_from(MAX_CLAIM_RECEIPT_BYTES)
        .context("receipt size limit exceeds this platform")?;
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("read claim receipt from '{}'", path.display()))?;
    ensure!(
        bytes.len() <= MAX_CLAIM_RECEIPT_BYTES,
        "claim receipt at '{}' exceeds the {} MiB input limit",
        path.display(),
        MAX_CLAIM_RECEIPT_BYTES / (1024 * 1024)
    );
    Ok(bytes)
}
