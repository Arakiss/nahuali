//! The `reconcile` command: re-verify the ground-truth ledger and rebuild the
//! derived tiers from it in one gesture.
//!
//! The append-only `memory_record` ledger is authoritative and persists across
//! downtime. The derived tiers (the SurrealDB graph projection and the Qdrant
//! semantic index) can drift if a service was unavailable while writes landed,
//! so this restates the ledger's integrity and rebuilds the derived tiers from
//! it. Rebuilding the semantic index is best-effort: if Qdrant is unreachable,
//! the rest still reconciles and the gap is reported, not fatal.

use std::path::Path;

use nahuali_core::{LedgerAuditOptions, MemoryEngine};
use nahuali_ui::style;
use nahuali_ui::theme;

pub(crate) fn reconcile(
    memory: &mut MemoryEngine,
    database: &Path,
    json: bool,
) -> anyhow::Result<()> {
    // Ground truth first, then rebuild each derived tier from it.
    let audit = memory.audit_ledger(&LedgerAuditOptions::default());
    let projection = memory.projection_rebuild()?;
    let semantic = memory.sync_semantic_index();

    if json {
        let semantic_json = match &semantic {
            Ok(report) => serde_json::json!({
                "synced": true,
                "indexed_point_count": report.indexed_point_count,
                "collection": report.collection_name,
            }),
            Err(error) => serde_json::json!({ "synced": false, "error": error.to_string() }),
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "database": database.display().to_string(),
                "ledger": {
                    "verified": audit.integrity.verified,
                    "chain_intact": audit.integrity.chain_intact,
                    "merkle_root": audit.integrity.merkle_root,
                },
                "graph_projection": {
                    "node_rows_written": projection.node_rows_written,
                    "relation_rows_written": projection.relation_rows_written,
                },
                "semantic_index": semantic_json,
            }))?
        );
        return Ok(());
    }

    println!(
        "{}",
        style::heading(&format!("Reconcile · {}", database.display()))
    );

    // Ledger — the authoritative tier.
    let ledger_value = if audit.integrity.verified {
        let chain = match &audit.integrity.merkle_root {
            Some(root) => {
                let short: String = root.chars().take(10).collect();
                format!("chain intact · merkle {short}…")
            }
            None => "append-only · hash chain off".to_string(),
        };
        format!(
            "{} · {}",
            style::badge("verified", theme::GREEN),
            style::dim(&chain)
        )
    } else {
        style::badge("UNVERIFIED — investigate before trusting", theme::RED)
    };
    println!(
        "  {}{ledger_value}",
        style::dim(&format!("{:<10}", "ledger"))
    );

    // Graph projection — rebuilt from the ledger.
    println!(
        "  {}{} · {}",
        style::dim(&format!("{:<10}", "graph")),
        style::badge("rebuilt", theme::GREEN),
        style::dim(&format!(
            "{} nodes · {} relations",
            projection.node_rows_written, projection.relation_rows_written
        ))
    );

    // Semantic index — derived and best-effort.
    match &semantic {
        Ok(report) => println!(
            "  {}{} · {}",
            style::dim(&format!("{:<10}", "semantic")),
            style::badge("synced", theme::GREEN),
            style::dim(&format!("{} points", report.indexed_point_count))
        ),
        Err(_) => println!(
            "  {}{}",
            style::dim(&format!("{:<10}", "semantic")),
            style::badge(
                "skipped — Qdrant unreachable (derived tier; re-run when it is back)",
                theme::AMBER
            )
        ),
    }

    Ok(())
}
