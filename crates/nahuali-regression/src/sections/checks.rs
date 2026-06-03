fn append_authority_checks(
    checks: &mut Vec<CheckResult>,
    health: &KnowledgeHealth,
    expected: &Expected,
) {
    let authority = AuthorityDecision::evaluate(health);
    if let Some(expected_mode) = &expected.authority_mode {
        checks.push(CheckResult {
            name: "authority_mode".to_string(),
            passed: authority.mode == *expected_mode,
            detail: format!("expected={expected_mode:?} actual={:?}", authority.mode),
        });
    }
    if let Some(expected_score) = expected.authority_score {
        checks.push(CheckResult {
            name: "authority_score".to_string(),
            passed: (authority.score - expected_score).abs() < f32::EPSILON,
            detail: format!("expected={expected_score:.2} actual={:.2}", authority.score),
        });
    }
    if let Some(expected_can_trust) = expected.authority_can_trust {
        checks.push(CheckResult {
            name: "authority_can_trust".to_string(),
            passed: authority.can_trust == expected_can_trust,
            detail: format!(
                "expected={expected_can_trust} actual={}",
                authority.can_trust
            ),
        });
    }
}

fn append_projection_checks(
    checks: &mut Vec<CheckResult>,
    memory: &MemoryEngine,
    health: &KnowledgeHealth,
    expected: &Expected,
) {
    check_eq(
        checks,
        "event_count",
        memory.data().event_count,
        expected.event_count,
    );
    check_eq(
        checks,
        "episode_count",
        memory.data().episodes.len(),
        expected.episode_count,
    );
    check_eq(
        checks,
        "fact_count",
        memory.data().facts.len(),
        expected.fact_count,
    );
    check_eq(
        checks,
        "relation_count",
        memory.data().relations.len(),
        expected.relation_count,
    );
    check_eq(
        checks,
        "supported_fact_count",
        health.supported_fact_count,
        expected.supported_fact_count,
    );
    check_eq(
        checks,
        "unsupported_fact_count",
        health.unsupported_fact_count,
        expected.unsupported_fact_count,
    );
    check_eq(
        checks,
        "low_confidence_fact_count",
        health.low_confidence_fact_count,
        expected.low_confidence_fact_count,
    );
    check_eq(
        checks,
        "conflicting_fact_count",
        health.conflicting_fact_count,
        expected.conflicting_fact_count,
    );
    check_eq(
        checks,
        "stale_fact_count",
        health.stale_fact_count,
        expected.stale_fact_count,
    );
    check_eq(
        checks,
        "superseded_fact_count",
        health.superseded_fact_count,
        expected.superseded_fact_count,
    );
    check_eq(
        checks,
        "isolated_entity_count",
        health.isolated_entity_count,
        expected.isolated_entity_count,
    );
    check_min(
        checks,
        "blind_spot_count",
        health.blind_spot_count,
        expected.blind_spot_count_min,
    );
}

fn append_signal_checks(
    checks: &mut Vec<CheckResult>,
    health: &KnowledgeHealth,
    expected: &Expected,
) {
    if let Some(required_signal_kinds) = &expected.required_signal_kinds {
        for kind in required_signal_kinds {
            let present = health.signals.iter().any(|signal| signal.kind == *kind);
            checks.push(CheckResult {
                name: format!("signal::{kind:?}"),
                passed: present,
                detail: if present {
                    "present".to_string()
                } else {
                    "missing".to_string()
                },
            });
        }
    }

    if let Some(required_signals) = &expected.required_signals {
        for required in required_signals {
            let matching_signal = health
                .signals
                .iter()
                .find(|signal| signal.kind == required.kind);
            checks.push(CheckResult {
                name: format!("signal::{:?}::present", required.kind),
                passed: matching_signal.is_some(),
                detail: if matching_signal.is_some() {
                    "present".to_string()
                } else {
                    "missing".to_string()
                },
            });

            if let (Some(signal), Some(severity)) = (matching_signal, &required.severity) {
                checks.push(CheckResult {
                    name: format!("signal::{:?}::severity", required.kind),
                    passed: signal.severity == *severity,
                    detail: format!("expected={severity:?} actual={:?}", signal.severity),
                });
            }

            if let (Some(signal), Some(dimensions)) = (matching_signal, &required.dimensions) {
                for dimension in dimensions {
                    let present = signal.dimensions.iter().any(|actual| actual == dimension);
                    checks.push(CheckResult {
                        name: format!("signal::{:?}::{dimension:?}", required.kind),
                        passed: present,
                        detail: if present {
                            "present".to_string()
                        } else {
                            "missing".to_string()
                        },
                    });
                }
            }
        }
    }
}

fn append_recall_checks(
    checks: &mut Vec<CheckResult>,
    recall_observations: Vec<RecallObservation>,
) {
    for observation in recall_observations {
        for required_kind in observation.require_kinds {
            let present = observation
                .results
                .iter()
                .any(|result| result.kind == required_kind);
            checks.push(CheckResult {
                name: format!("recall::{}::{required_kind:?}", observation.query),
                passed: present,
                detail: if present {
                    "present".to_string()
                } else {
                    "missing".to_string()
                },
            });
        }

        if let Some(required_first_kind) = observation.require_first_kind {
            let actual = observation
                .results
                .first()
                .map(|result| result.kind.clone());
            let passed = actual.as_ref() == Some(&required_first_kind);
            let actual_detail = actual
                .map(|kind| format!("{kind:?}"))
                .unwrap_or_else(|| "none".to_string());
            checks.push(CheckResult {
                name: format!("recall::{}::first_kind", observation.query),
                passed,
                detail: format!("expected={required_first_kind:?} actual={actual_detail}"),
            });
        }

        if let Some(required_result_count) = observation.require_result_count {
            let actual = observation.results.len();
            checks.push(CheckResult {
                name: format!("recall::{}::result_count", observation.query),
                passed: actual == required_result_count,
                detail: format!("expected={required_result_count} actual={actual}"),
            });
        }

        if let Some(required_authority_mode) = observation.require_authority_mode {
            let actual_authority_mode = observation.authority_mode.as_ref();
            let passed = actual_authority_mode == Some(&required_authority_mode);
            let actual = actual_authority_mode
                .map(|mode| format!("{mode:?}"))
                .unwrap_or_else(|| "none".to_string());
            checks.push(CheckResult {
                name: format!("recall::{}::authority_mode", observation.query),
                passed,
                detail: format!("expected={required_authority_mode:?} actual={actual}"),
            });
        }
    }
}

