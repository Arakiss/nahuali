use nahuali_core::{LedgerAudit, LedgerAuditEventKind, LedgerAuditOptions, MemoryEngine};

/// Audit what changed in the record ledger between two points without mutating
/// it. Exits non-zero when the history through the upper bound fails integrity
/// verification, so it can gate scripts and CI.
pub(crate) fn audit(
    memory: &MemoryEngine,
    options: LedgerAuditOptions,
    json: bool,
) -> anyhow::Result<()> {
    let report = memory.audit_ledger(&options);

    if json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        print_human(&report);
    }

    if !report.integrity.verified {
        anyhow::bail!("ledger audit failed integrity verification");
    }
    Ok(())
}

fn print_human(report: &LedgerAudit) {
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
