//! Typed `*View` mirrors of core report structs, grouped by family. Each view
//! derives `schemars::JsonSchema` so MCP tool outputs carry a precise output
//! schema, while staying a 1:1 mirror of the corresponding `nahuali-core` type.

use serde::Serialize;

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

/// Serialize a value and extract its JSON string form, used to mirror core
/// enums as their snake_case wire representation. Shared by every view module.
pub(crate) fn json_string(value: &impl Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| "unknown".to_string())
}
