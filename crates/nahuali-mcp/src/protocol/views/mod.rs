//! Typed `*View` mirrors of core report structs, grouped by family. Each view
//! derives `schemars::JsonSchema` so MCP tool outputs carry a precise output
//! schema, while staying a 1:1 mirror of the corresponding `nahuali-core` type.

#[cfg(feature = "tamper-evidence")]
use rmcp::schemars;
use serde::Serialize;

#[cfg(feature = "tamper-evidence")]
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LedgerChainStatusView {
    Empty,
    Verified,
    Legacy,
    Broken,
}

#[cfg(feature = "tamper-evidence")]
impl From<nahuali_core::LedgerChainStatus> for LedgerChainStatusView {
    fn from(status: nahuali_core::LedgerChainStatus) -> Self {
        match status {
            nahuali_core::LedgerChainStatus::Empty => Self::Empty,
            nahuali_core::LedgerChainStatus::Verified => Self::Verified,
            nahuali_core::LedgerChainStatus::Legacy => Self::Legacy,
            nahuali_core::LedgerChainStatus::Broken => Self::Broken,
        }
    }
}

mod audit;
mod briefing;
mod cognition;
mod graph;
mod health;
mod ingest;
mod intention;
mod operator;
mod proactive;
mod projection;
mod records;
mod review;
mod self_inspection;
mod semantic;
mod trust_report;

pub(crate) use audit::*;
pub(crate) use briefing::*;
pub(crate) use cognition::*;
pub(crate) use graph::*;
pub(crate) use health::*;
pub(crate) use ingest::*;
pub(crate) use intention::*;
pub(crate) use operator::*;
pub(crate) use proactive::*;
pub(crate) use projection::*;
pub(crate) use records::*;
pub(crate) use review::*;
pub(crate) use self_inspection::*;
pub(crate) use semantic::*;
pub(crate) use trust_report::*;

/// Serialize a value and extract its JSON string form, used to mirror core
/// enums as their snake_case wire representation. Shared by every view module.
pub(crate) fn json_string(value: &impl Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| "unknown".to_string())
}
