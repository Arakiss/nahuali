#[derive(Debug)]
struct RecallObservation {
    query: String,
    require_kinds: Vec<MemoryKind>,
    require_result_count: Option<usize>,
    require_first_kind: Option<MemoryKind>,
    require_authority_mode: Option<AuthorityMode>,
    authority_mode: Option<AuthorityMode>,
    results: Vec<RecallResult>,
}

impl From<RawPayload> for MemoryEvent {
    fn from(payload: RawPayload) -> Self {
        match payload {
            RawPayload::EpisodeRecorded {
                id,
                content,
                tags,
                mentions,
            } => MemoryEvent::EpisodeRecorded(EpisodeRecorded {
                id,
                content,
                tags,
                mentions: mentions.unwrap_or_default(),
                source_id: None,
                source_position: None,
                source_role: None,
                scope: None,
            }),
            RawPayload::FactAsserted {
                id,
                subject,
                predicate,
                object,
                source_episode_id,
                confidence,
            } => MemoryEvent::FactAsserted(FactAsserted {
                id,
                subject,
                predicate,
                object,
                source_episode_id,
                confidence,
                scope: None,
            }),
            RawPayload::RelationRecorded {
                id,
                from,
                relation,
                to,
                source_episode_id,
                confidence,
            } => MemoryEvent::RelationRecorded(RelationRecorded {
                id,
                from,
                relation,
                to,
                source_episode_id,
                confidence,
                scope: None,
            }),
        }
    }
}

fn resolve_binding(
    bindings: &BTreeMap<String, String>,
    source_episode_ref: Option<String>,
) -> anyhow::Result<Option<String>> {
    source_episode_ref
        .map(|name| {
            bindings
                .get(&name)
                .cloned()
                .with_context(|| format!("unknown source episode binding: {name}"))
        })
        .transpose()
}

fn check_eq(checks: &mut Vec<CheckResult>, name: &str, actual: usize, expected: Option<usize>) {
    if let Some(expected) = expected {
        checks.push(CheckResult {
            name: name.to_string(),
            passed: actual == expected,
            detail: format!("expected={expected} actual={actual}"),
        });
    }
}

fn check_min(
    checks: &mut Vec<CheckResult>,
    name: &str,
    actual: usize,
    expected_min: Option<usize>,
) {
    if let Some(expected_min) = expected_min {
        checks.push(CheckResult {
            name: name.to_string(),
            passed: actual >= expected_min,
            detail: format!("expected_min={expected_min} actual={actual}"),
        });
    }
}

fn temp_database(fixture_id: &str) -> PathBuf {
    let count = RUN_COUNTER.fetch_add(1, Ordering::Relaxed);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    std::env::temp_dir().join(format!("nahuali_regression_{fixture_id}_{now}_{count}"))
}

#[derive(Clone, Debug, Deserialize)]
struct FixtureFile {
    version: u32,
    fixtures: Vec<Fixture>,
}

#[derive(Clone, Debug, Deserialize)]
struct Fixture {
    id: String,
    goal: String,
    inspect_at_ms: Option<u64>,
    steps: Vec<Step>,
    expected: Expected,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Step {
    Remember {
        bind: Option<String>,
        content: String,
        tags: Option<Vec<String>>,
    },
    Fact {
        subject: String,
        predicate: String,
        object: String,
        source_episode_ref: Option<String>,
        confidence: Option<f32>,
    },
    Relate {
        from: String,
        relation: String,
        to: String,
        source_episode_ref: Option<String>,
        confidence: Option<f32>,
    },
    Procedure {
        name: String,
        body: String,
        source_episode_ref: Option<String>,
        confidence: Option<f32>,
    },
    Preference {
        name: String,
        body: String,
        source_episode_ref: Option<String>,
        confidence: Option<f32>,
    },
    Intention {
        bind: Option<String>,
        description: String,
        kind: IntentionKind,
        priority: IntentionPriority,
        source_episode_ref: Option<String>,
    },
    IntentionStatus {
        id_ref: String,
        status: IntentionStatus,
        reason: Option<String>,
    },
    Recall {
        query: String,
        limit: Option<usize>,
        require_kinds: Option<Vec<MemoryKind>>,
        require_result_count: Option<usize>,
        require_authority_mode: Option<AuthorityMode>,
    },
    RankedRecall {
        query: String,
        limit: Option<usize>,
        require_first_kind: MemoryKind,
    },
    RawEvent {
        version: Option<u32>,
        sequence: u64,
        timestamp_ms: u64,
        payload: RawPayload,
        corrupt_checksum: Option<bool>,
    },
    WriteRawRecords,
    ExpectOpenError {
        contains: String,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RawPayload {
    EpisodeRecorded {
        id: String,
        content: String,
        tags: Vec<String>,
        mentions: Option<Vec<String>>,
    },
    FactAsserted {
        id: String,
        subject: String,
        predicate: String,
        object: String,
        source_episode_id: Option<String>,
        confidence: f32,
    },
    RelationRecorded {
        id: String,
        from: String,
        relation: String,
        to: String,
        source_episode_id: Option<String>,
        confidence: f32,
    },
}

#[derive(Clone, Debug, Deserialize)]
struct Expected {
    event_count: Option<usize>,
    episode_count: Option<usize>,
    fact_count: Option<usize>,
    relation_count: Option<usize>,
    supported_fact_count: Option<usize>,
    unsupported_fact_count: Option<usize>,
    low_confidence_fact_count: Option<usize>,
    conflicting_fact_count: Option<usize>,
    stale_fact_count: Option<usize>,
    superseded_fact_count: Option<usize>,
    isolated_entity_count: Option<usize>,
    blind_spot_count_min: Option<usize>,
    required_signal_kinds: Option<Vec<HealthSignalKind>>,
    required_signals: Option<Vec<RequiredSignal>>,
    authority_mode: Option<AuthorityMode>,
    authority_score: Option<f32>,
    authority_can_trust: Option<bool>,
    open_error_contains: Option<String>,
    check_count_min: Option<usize>,
}

#[derive(Clone, Debug, Deserialize)]
struct RequiredSignal {
    kind: HealthSignalKind,
    severity: Option<HealthSeverity>,
    dimensions: Option<Vec<HealthDimension>>,
}

#[derive(Debug, Serialize)]
struct RegressionReport {
    version: u32,
    fixture_count: usize,
    passed: usize,
    failed: usize,
    results: Vec<FixtureResult>,
}

#[derive(Debug, Serialize)]
struct FixtureResult {
    id: String,
    goal: String,
    passed: bool,
    checks: Vec<CheckResult>,
}

#[derive(Debug, Serialize)]
struct CheckResult {
    name: String,
    passed: bool,
    detail: String,
}
