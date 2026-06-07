fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    if cli.livr {
        return run_livr_report(cli.output);
    }

    let report = run_fixture_file(&cli.fixtures)?;
    let encoded = serde_json::to_string_pretty(&report)?;

    if let Some(output) = cli.output {
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&output, format!("{encoded}\n"))
            .with_context(|| format!("failed to write {}", output.display()))?;
    } else {
        println!("{encoded}");
    }

    if report.failed > 0 {
        bail!("{} regression fixture(s) failed", report.failed);
    }

    Ok(())
}

/// Compute the LIVR integrity report, emit it as versioned JSON, and gate on
/// the attestation tier reaching full detection with no false positives.
#[cfg(feature = "attestation")]
fn run_livr_report(output: Option<PathBuf>) -> anyhow::Result<()> {
    use nahuali_core::{LivrDetectorTier, run_livr};

    let report = run_livr();
    let encoded = serde_json::to_string_pretty(&report)?;

    if let Some(output) = output {
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&output, format!("{encoded}\n"))
            .with_context(|| format!("failed to write {}", output.display()))?;
    } else {
        println!("{encoded}");
    }

    let attestation = report
        .tiers
        .iter()
        .find(|tier| tier.tier == LivrDetectorTier::AttestationTip)
        .context("LIVR report is missing the attestation tier")?;

    if attestation.detection_rate < 1.0 || attestation.false_positives > 0 {
        bail!(
            "LIVR attestation tier missed the clean target: detection_rate={:.2}, false_positives={}",
            attestation.detection_rate,
            attestation.false_positives
        );
    }

    Ok(())
}

/// On a default build the attestation tier is absent, so the LIVR report cannot
/// be computed. Fail with an actionable message instead of a missing symbol.
#[cfg(not(feature = "attestation"))]
fn run_livr_report(_output: Option<PathBuf>) -> anyhow::Result<()> {
    bail!("--livr requires building nahuali-regression with --features attestation");
}

fn run_fixture_file(path: &Path) -> anyhow::Result<RegressionReport> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read fixture file {}", path.display()))?;
    let fixtures: FixtureFile = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse fixture file {}", path.display()))?;
    run_fixtures(fixtures)
}

fn run_fixtures(fixtures: FixtureFile) -> anyhow::Result<RegressionReport> {
    let mut results = Vec::new();

    for fixture in fixtures.fixtures {
        results.push(run_fixture(fixture)?);
    }

    let passed = results.iter().filter(|result| result.passed).count();
    let failed = results.len().saturating_sub(passed);

    Ok(RegressionReport {
        version: fixtures.version,
        fixture_count: results.len(),
        passed,
        failed,
        results,
    })
}

fn run_fixture(fixture: Fixture) -> anyhow::Result<FixtureResult> {
    let database = temp_database(&fixture.id);
    let _ = fs::remove_file(&database);

    let mut bindings = BTreeMap::new();
    let mut raw_events = Vec::new();
    let mut recall_observations = Vec::new();
    let mut checks = Vec::new();

    for step in fixture.steps {
        match step {
            Step::Remember {
                bind,
                content,
                tags,
            } => {
                let mut memory = MemoryEngine::open(&database)?;
                let episode = memory.remember(content, tags.unwrap_or_default())?;
                if let Some(bind) = bind {
                    bindings.insert(bind, episode.id);
                }
            }
            Step::Fact {
                subject,
                predicate,
                object,
                source_episode_ref,
                confidence,
            } => {
                let source_episode_id = resolve_binding(&bindings, source_episode_ref)?;
                let mut memory = MemoryEngine::open(&database)?;
                memory.add_fact(
                    subject,
                    predicate,
                    object,
                    source_episode_id,
                    confidence.unwrap_or(0.8),
                )?;
            }
            Step::Relate {
                from,
                relation,
                to,
                source_episode_ref,
                confidence,
            } => {
                let source_episode_id = resolve_binding(&bindings, source_episode_ref)?;
                let mut memory = MemoryEngine::open(&database)?;
                memory.relate(
                    from,
                    relation,
                    to,
                    source_episode_id,
                    confidence.unwrap_or(0.8),
                )?;
            }
            Step::Procedure {
                name,
                body,
                source_episode_ref,
                confidence,
            } => {
                let source_episode_id = resolve_binding(&bindings, source_episode_ref)?;
                let mut memory = MemoryEngine::open(&database)?;
                memory.add_procedure(name, body, source_episode_id, confidence.unwrap_or(0.8))?;
            }
            Step::Preference {
                name,
                body,
                source_episode_ref,
                confidence,
            } => {
                let source_episode_id = resolve_binding(&bindings, source_episode_ref)?;
                let mut memory = MemoryEngine::open(&database)?;
                memory.add_preference(name, body, source_episode_id, confidence.unwrap_or(0.8))?;
            }
            Step::Intention {
                bind,
                description,
                kind,
                priority,
                source_episode_ref,
            } => {
                let source_episode_id = resolve_binding(&bindings, source_episode_ref)?;
                let mut memory = MemoryEngine::open(&database)?;
                let intention =
                    memory.add_intention(description, kind, priority, source_episode_id)?;
                if let Some(bind) = bind {
                    bindings.insert(bind, intention.id);
                }
            }
            Step::IntentionStatus {
                id_ref,
                status,
                reason,
            } => {
                let id = bindings
                    .get(&id_ref)
                    .cloned()
                    .with_context(|| format!("unknown intention binding: {id_ref}"))?;
                let mut memory = MemoryEngine::open(&database)?;
                memory.set_intention_status(id, status, reason)?;
            }
            Step::Recall {
                query,
                limit,
                require_kinds,
                require_result_count,
                require_authority_mode,
            } => {
                let memory = MemoryEngine::open(&database)?;
                let recall = memory.recall_with_authority(&query, limit.unwrap_or(10))?;
                recall_observations.push(RecallObservation {
                    query,
                    require_kinds: require_kinds.unwrap_or_default(),
                    require_result_count,
                    require_first_kind: None,
                    require_authority_mode,
                    authority_mode: Some(recall.authority.mode),
                    results: recall.results,
                });
            }
            Step::RankedRecall {
                query,
                limit,
                require_first_kind,
            } => {
                let memory = MemoryEngine::open(&database)?;
                let results = memory.recall(&query, limit.unwrap_or(10))?;
                recall_observations.push(RecallObservation {
                    query,
                    require_kinds: Vec::new(),
                    require_result_count: None,
                    require_first_kind: Some(require_first_kind),
                    require_authority_mode: None,
                    authority_mode: None,
                    results,
                });
            }
            Step::RawEvent {
                version,
                sequence,
                timestamp_ms,
                payload,
                corrupt_checksum,
            } => {
                let mut event = EventEnvelope::new(sequence, timestamp_ms, payload.into());
                if let Some(version) = version {
                    event.version = version;
                }
                if corrupt_checksum.unwrap_or(false) {
                    event.checksum = "corrupted".to_string();
                }
                raw_events.push(event);
            }
            Step::WriteRawRecords => {
                write_raw_record_ledger(&database, &raw_events)?;
            }
            Step::ExpectOpenError { contains } => {
                checks.push(open_error_check(&database, &contains));
            }
        }
    }

    if let Some(expected_open_error) = &fixture.expected.open_error_contains {
        if !checks
            .iter()
            .any(|check| check.name == "expected_open_error")
        {
            checks.push(open_error_check(&database, expected_open_error));
        }
    } else {
        let reopened = MemoryEngine::open(&database)?;
        let health = fixture
            .inspect_at_ms
            .map(|at| KnowledgeHealth::inspect_at(reopened.data(), at))
            .unwrap_or_else(|| reopened.inspect());

        append_projection_checks(&mut checks, &reopened, &health, &fixture.expected);
        append_signal_checks(&mut checks, &health, &fixture.expected);
        append_authority_checks(&mut checks, &health, &fixture.expected);
        append_recall_checks(&mut checks, recall_observations);

        checks.push(CheckResult {
            name: "store_validation".to_string(),
            passed: reopened.data().event_count == reopened.events().len(),
            detail: format!(
                "events={} projected={}",
                reopened.events().len(),
                reopened.data().event_count
            ),
        });
    }

    if let Some(expected_check_count_min) = fixture.expected.check_count_min {
        let actual = checks.len();
        checks.push(CheckResult {
            name: "check_count".to_string(),
            passed: actual >= expected_check_count_min,
            detail: format!("expected_min={expected_check_count_min} actual={actual}"),
        });
    }

    let _ = fs::remove_file(&database);
    let passed = checks.iter().all(|check| check.passed);

    Ok(FixtureResult {
        id: fixture.id,
        goal: fixture.goal,
        passed,
        checks,
    })
}

