use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, bail};
use clap::Parser;
use nahuali_core::{
    AuthorityDecision, AuthorityMode, EpisodeRecorded, EventEnvelope, FactAsserted,
    HealthDimension, HealthSeverity, HealthSignalKind, IntentionKind, IntentionPriority,
    IntentionStatus, KnowledgeHealth, MEMORY_RECORD_SCHEMA, MemoryEngine, MemoryEvent, MemoryKind,
    RecallResult, RelationRecorded,
};
use serde::{Deserialize, Serialize};
use surrealdb::{
    Surreal,
    engine::remote::ws::{Client, Ws},
    opt::auth::Root,
};
use tokio::runtime::{Builder, Runtime};

static RUN_COUNTER: AtomicU64 = AtomicU64::new(1);
const SURREAL_NAMESPACE: &str = "nahuali";
const DEFAULT_SURREAL_ENDPOINT: &str = "localhost:18000";
const SURREAL_DATABASE: &str = "memory";

#[derive(Debug, Parser)]
#[command(name = "nahuali-regression")]
#[command(about = "Run synthetic Nahuali knowledge-health regression fixtures")]
struct Cli {
    #[arg(
        long,
        value_name = "PATH",
        default_value = "fixtures/knowledge-health-regression.json"
    )]
    fixtures: PathBuf,

    #[arg(long, value_name = "PATH")]
    output: Option<PathBuf>,

    /// Emit the computed LIVR (Ledger Integrity Verification Rate) report
    /// instead of running fixtures. Requires `--features attestation`; the gate
    /// fails if the attestation tier does not reach full detection with no
    /// false positives.
    #[arg(long)]
    livr: bool,
}

include!("sections/runner.rs");
include!("sections/checks.rs");
include!("sections/ledger.rs");
include!("sections/model.rs");
include!("sections/tests.rs");
