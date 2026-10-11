use anyhow::Context;
use nahuali_core::{ForgetOptions, ForgetReport, MemoryEngine};

/// Forget memory by id: redact it and everything derived from it, and record
/// an audit trace that names who, when, and why without the erased content.
pub(crate) fn forget(
    memory: &mut MemoryEngine,
    options: ForgetOptions,
    rebuild_semantic: bool,
    json: bool,
) -> anyhow::Result<()> {
    let mut report = memory.forget(&options)?;
    if rebuild_semantic && !report.dry_run {
        memory
            .rebuild_semantic_index()
            .context("memory was forgotten, but the semantic index rebuild failed; run `nahuali semantic-rebuild`")?;
        report.semantic_index_rebuilt = true;
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_report(&report);
    }
    Ok(())
}

fn print_report(report: &ForgetReport) {
    if report.dry_run {
        println!("Dry run: nothing was erased.");
    } else if let Some(id) = &report.id {
        println!("Forgotten: {id}");
    }
    println!("Reason: {}", report.reason);
    if let Some(requested_by) = &report.requested_by {
        println!("Requested by: {requested_by}");
    }
    println!("Targets: {}", report.targets.join(", "));
    println!(
        "Memory ids {}: {}",
        if report.dry_run { "to erase" } else { "erased" },
        report.forgotten_ids.join(", ")
    );
    println!("Ledger records redacted: {}", report.redacted_event_count);
    for record in &report.redacted_events {
        println!(
            "  #{} {} ({})",
            record.sequence, record.kind, record.event_id
        );
    }
    if report.dry_run {
        return;
    }
    println!(
        "Graph projection rebuilt: {}",
        report.graph_projection_rebuilt
    );
    if report.semantic_index_rebuilt {
        println!("Semantic index rebuilt: true");
    } else {
        println!(
            "Semantic index: not rebuilt. Recall already hides the erased memory; if you use Qdrant, run `nahuali semantic-rebuild` so the index no longer stores its text."
        );
    }
    println!(
        "Not erased: backups, snapshots, exports, and receipts written before now. Replace or delete them to complete the erasure."
    );
}
