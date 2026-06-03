use nahuali_core::{MemoryEngine, MemoryTrustReport, TrustReportOptions};

/// Print a composed, non-mutating memory trust report. Exits non-zero when the
/// recorded history fails integrity verification, so it can gate scripts and CI;
/// the broader `trustworthy` verdict (which folds in conservative authority and
/// health signals) is reported, not gated.
pub(crate) fn trust_report(
    memory: &MemoryEngine,
    options: TrustReportOptions,
    json: bool,
) -> anyhow::Result<()> {
    let report = memory.trust_report_with_options(options);

    if json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        print_human(&report);
    }

    if !report.integrity.ledger_verified {
        anyhow::bail!("memory trust report failed ledger integrity verification");
    }
    Ok(())
}

fn print_human(report: &MemoryTrustReport) {
    let knowledge = &report.knowledge;
    let integrity = &report.integrity;

    println!("Memory trust report");
    println!("Trustworthy: {}", yes_no(report.trustworthy));
    println!(
        "Knowledge: {} events ({} episodes, {} claims, {} links, {} procedures, {} intentions, {} sources, {} entities)",
        knowledge.event_count,
        knowledge.episode_count,
        knowledge.claim_count,
        knowledge.link_count,
        knowledge.procedure_count,
        knowledge.intention_count,
        knowledge.source_count,
        knowledge.entity_count
    );
    println!(
        "Authority: {:?} (score {:.2}, can_trust {})",
        report.authority.mode,
        report.authority.score,
        yes_no(report.authority.can_trust)
    );

    print!(
        "Integrity: {} (checksums {}, sequence {}",
        if integrity.ledger_verified {
            "verified"
        } else {
            "FAILED"
        },
        if integrity.checksums_valid {
            "ok"
        } else {
            "bad"
        },
        if integrity.sequence_contiguous {
            "contiguous"
        } else {
            "broken"
        }
    );
    #[cfg(feature = "tamper-evidence")]
    print!(
        ", chain {}",
        if integrity.chain_intact {
            "intact"
        } else {
            "broken"
        }
    );
    println!(")");
    #[cfg(feature = "tamper-evidence")]
    if let Some(tip) = &integrity.chain_tip {
        println!("Chain tip: {tip}");
    }

    println!(
        "Health: {} unsupported, {} conflicting, {} blind spots (avg confidence {:.2})",
        report.health.unsupported_fact_count,
        report.health.conflicting_fact_count,
        report.health.blind_spot_count,
        report.health.average_fact_confidence
    );

    #[cfg(feature = "attestation")]
    if let Some(verdict) = &report.attestation {
        println!(
            "Attestation: checkpoint at sequence {} {}",
            verdict.sequence,
            if verdict.anchored {
                "anchored"
            } else {
                "NOT anchored"
            }
        );
    }

    println!("Reasons:");
    for reason in &report.verdict_reasons {
        println!("- {reason}");
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

/// Read and parse a signed attestation receipt to fold into the report.
#[cfg(feature = "attestation")]
pub(crate) fn read_attestation(
    path: &std::path::Path,
) -> anyhow::Result<nahuali_core::LedgerAttestation> {
    use anyhow::Context;

    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read attestation {}", path.display()))?;
    serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse attestation {}", path.display()))
}
