#[cfg(feature = "tamper-evidence")]
use serde::Serialize;

use nahuali_core::{LedgerAudit, LedgerAuditEventKind, LedgerAuditOptions, MemoryEngine};
#[cfg(feature = "tamper-evidence")]
use nahuali_core::{
    MerkleSibling, ledger_inclusion_proof, ledger_merkle_root, verify_merkle_proof,
};

/// Audit what changed in the record ledger between two points without mutating
/// it. Exits non-zero when the history through the upper bound fails integrity
/// verification, so it can gate scripts and CI.
pub(crate) fn audit(
    memory: &MemoryEngine,
    options: LedgerAuditOptions,
    #[cfg(feature = "tamper-evidence")] inclusion_sequence: Option<u64>,
    json: bool,
) -> anyhow::Result<()> {
    let report = memory.audit_ledger(&options);
    #[cfg(feature = "tamper-evidence")]
    let inclusion_proof = inclusion_sequence
        .map(|sequence| build_inclusion_proof(memory, &report, sequence))
        .transpose()?;

    if json {
        #[cfg(feature = "tamper-evidence")]
        if let Some(proof) = &inclusion_proof {
            let mut value = serde_json::to_value(&report)?;
            if let Some(object) = value.as_object_mut() {
                object.insert("inclusion_proof".to_string(), serde_json::to_value(proof)?);
            }
            println!("{}", serde_json::to_string(&value)?);
        } else {
            println!("{}", serde_json::to_string(&report)?);
        }
        #[cfg(not(feature = "tamper-evidence"))]
        println!("{}", serde_json::to_string(&report)?);
    } else {
        #[cfg(feature = "tamper-evidence")]
        print_human(&report, inclusion_proof.as_ref());
        #[cfg(not(feature = "tamper-evidence"))]
        print_human(&report);
    }

    if !report.integrity.verified {
        anyhow::bail!("ledger audit failed integrity verification");
    }
    Ok(())
}

#[cfg(feature = "tamper-evidence")]
#[derive(Debug, Serialize)]
struct LedgerInclusionProofReport {
    sequence: u64,
    index: usize,
    event_id: String,
    leaf_chain_hash: String,
    merkle_root: String,
    leaf_count: usize,
    siblings: Vec<MerkleSibling>,
    verified: bool,
}

#[cfg(feature = "tamper-evidence")]
fn build_inclusion_proof(
    memory: &MemoryEngine,
    report: &LedgerAudit,
    sequence: u64,
) -> anyhow::Result<LedgerInclusionProofReport> {
    use anyhow::{Context, bail};

    if sequence == 0 {
        bail!("Merkle inclusion proofs are event proofs; sequence 0 is the genesis anchor");
    }
    if sequence > report.to_sequence {
        bail!(
            "cannot prove sequence {sequence}: the audit upper bound is sequence {}",
            report.to_sequence
        );
    }

    let prefix_len = memory
        .events()
        .iter()
        .take_while(|event| event.sequence <= report.to_sequence)
        .count();
    let prefix = &memory.events()[..prefix_len];
    if prefix.is_empty() {
        bail!("cannot emit a Merkle inclusion proof for an empty ledger");
    }
    if !prefix.iter().all(|event| event.is_chained()) {
        bail!(
            "Merkle inclusion proofs require a fully chained ledger prefix; \
             this prefix contains legacy unchained records"
        );
    }

    let index = prefix
        .iter()
        .position(|event| event.sequence == sequence)
        .with_context(|| format!("sequence {sequence} is not present in the audited prefix"))?;
    let event = &prefix[index];
    let proof =
        ledger_inclusion_proof(prefix, index).context("failed to build Merkle inclusion proof")?;
    let root = ledger_merkle_root(prefix).context("audited prefix has no Merkle root")?;
    let leaf = event.chain_hash();
    let verified = verify_merkle_proof(&leaf, &proof, &root);

    Ok(LedgerInclusionProofReport {
        sequence,
        index,
        event_id: event.id.clone(),
        leaf_chain_hash: leaf,
        merkle_root: root,
        leaf_count: proof.leaf_count,
        siblings: proof.siblings,
        verified,
    })
}

#[cfg(feature = "tamper-evidence")]
fn print_human(report: &LedgerAudit, inclusion_proof: Option<&LedgerInclusionProofReport>) {
    print_human_report(report);
    if let Some(proof) = inclusion_proof {
        println!("Merkle inclusion proof: seq {}", proof.sequence);
        println!("  Event: {}", proof.event_id);
        println!("  Leaf index: {} of {}", proof.index, proof.leaf_count);
        println!("  Leaf hash: {}", proof.leaf_chain_hash);
        println!("  Root: {}", proof.merkle_root);
        println!("  Siblings: {}", proof.siblings.len());
        println!(
            "  Proof verifies: {}",
            if proof.verified { "yes" } else { "no" }
        );
    }
}

#[cfg(not(feature = "tamper-evidence"))]
fn print_human(report: &LedgerAudit) {
    print_human_report(report);
}

fn print_human_report(report: &LedgerAudit) {
    println!("Record ledger audit");
    println!("Total events: {}", report.total_event_count);
    println!(
        "Range: seq {} (exclusive) -> {} (inclusive)",
        report.from_sequence, report.to_sequence
    );
    println!("Changes in range: {}", report.range_event_count);

    #[cfg(feature = "tamper-evidence")]
    {
        if let Some(tip) = &report.from_tip {
            println!("Lower tip: {tip}");
        }
        if let Some(tip) = &report.to_tip {
            println!("Upper tip: {tip}");
        }
    }

    print!(
        "Integrity: {} (checksums {}, sequence {}",
        if report.integrity.verified {
            "verified"
        } else {
            "FAILED"
        },
        ok_or_bad(report.integrity.checksums_valid),
        if report.integrity.sequence_contiguous {
            "contiguous"
        } else {
            "broken"
        }
    );
    #[cfg(feature = "tamper-evidence")]
    print!(
        ", chain {}",
        if report.integrity.chain_intact {
            "intact"
        } else {
            "broken"
        }
    );
    println!(")");
    #[cfg(feature = "tamper-evidence")]
    if let Some(root) = &report.integrity.merkle_root {
        println!("Merkle root: {root}");
    }

    print_counts(report);

    for entry in &report.entries {
        let scope = entry
            .scope
            .as_ref()
            .map(|scope| format!(" [{}]", scope.key))
            .unwrap_or_default();
        println!(
            "- seq {} {} {}{scope}",
            entry.sequence,
            kind_label(entry.kind),
            entry.summary
        );
    }
}

fn print_counts(report: &LedgerAudit) {
    let counts = &report.counts;
    let rows = [
        ("sources", counts.sources_recorded),
        ("episodes", counts.episodes_recorded),
        ("facts", counts.facts_asserted),
        ("relations", counts.relations_recorded),
        ("procedures", counts.procedures_recorded),
        ("intentions", counts.intentions_recorded),
        ("intention updates", counts.intentions_updated),
        ("intention status changes", counts.intention_status_changes),
        ("reviews", counts.reviews_recorded),
    ];
    let summary = rows
        .iter()
        .filter(|(_, count)| *count > 0)
        .map(|(label, count)| format!("{count} {label}"))
        .collect::<Vec<_>>();
    if !summary.is_empty() {
        println!("By kind: {}", summary.join(", "));
    }
}

fn kind_label(kind: LedgerAuditEventKind) -> &'static str {
    match kind {
        LedgerAuditEventKind::SourceRecorded => "source",
        LedgerAuditEventKind::EpisodeRecorded => "episode",
        LedgerAuditEventKind::FactAsserted => "fact",
        LedgerAuditEventKind::RelationRecorded => "relation",
        LedgerAuditEventKind::ProcedureRecorded => "procedure",
        LedgerAuditEventKind::IntentionRecorded => "intention",
        LedgerAuditEventKind::IntentionUpdated => "intention-update",
        LedgerAuditEventKind::IntentionStatusChanged => "intention-status",
        LedgerAuditEventKind::ReviewRecorded => "review",
    }
}

fn ok_or_bad(value: bool) -> &'static str {
    if value { "ok" } else { "bad" }
}

/// Resolve the exclusive lower bound from a signed attestation receipt: verify it
/// anchors a genuine checkpoint in this ledger's history, then return its
/// sequence. Exits non-zero when the receipt does not anchor a verified
/// checkpoint, so an audit can never claim to diff from an unverified point.
#[cfg(feature = "attestation")]
pub(crate) fn resolve_attestation_anchor(
    memory: &MemoryEngine,
    path: &std::path::Path,
    json: bool,
) -> anyhow::Result<u64> {
    use anyhow::Context;
    use nahuali_core::LedgerAttestation;

    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read attestation {}", path.display()))?;
    let attestation: LedgerAttestation = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse attestation {}", path.display()))?;
    let verdict = memory.verify_attested_checkpoint(&attestation)?;

    if !json {
        println!(
            "Anchor: signed checkpoint at seq {} ({})",
            verdict.sequence,
            if verdict.anchored {
                "verified"
            } else {
                "NOT verified"
            }
        );
    }

    if !verdict.anchored {
        anyhow::bail!(
            "the attestation does not anchor a verified checkpoint in this ledger \
             (signature_valid={}, matches_history={})",
            verdict.signature_valid,
            verdict.matches_history
        );
    }
    Ok(verdict.sequence)
}
