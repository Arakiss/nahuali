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
