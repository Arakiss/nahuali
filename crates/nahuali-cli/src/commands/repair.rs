use std::fs;
use std::io::Read;
use std::path::Path;

use anyhow::Context;
use nahuali_core::{MemoryEngine, RepairProposal};

use crate::output;

/// Read a repair proposal (from a file or stdin), then validate, classify, and
/// apply it. The LLM proposed the JSON; the deterministic engine decides.
pub(crate) fn repair(
    memory: &mut MemoryEngine,
    proposal_path: Option<&Path>,
    approve: bool,
    dry_run: bool,
    json: bool,
) -> anyhow::Result<()> {
    let raw = read_proposal(proposal_path)?;
    let proposal: RepairProposal =
        serde_json::from_str(&raw).context("failed to parse the repair proposal as JSON")?;

    let report = memory.apply_repair(proposal, approve, dry_run)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        output::print_repair_report(&report);
    }
    Ok(())
}

/// Read the proposal JSON from a path, or from stdin when the path is omitted or
/// given as "-".
fn read_proposal(proposal_path: Option<&Path>) -> anyhow::Result<String> {
    match proposal_path {
        Some(path) if path.as_os_str() != "-" => {
            fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))
        }
        _ => {
            let mut buffer = String::new();
            std::io::stdin()
                .read_to_string(&mut buffer)
                .context("failed to read the repair proposal from stdin")?;
            Ok(buffer)
        }
    }
}
