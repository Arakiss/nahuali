use std::{path::PathBuf, time::Instant};

use nahuali_core::{EpisodeRecorded, EventEnvelope, MemoryEngine, MemoryEvent};

const SAMPLE_COUNT: usize = 9;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let median_budget_ms = env_budget("NAHUALI_REFRESH_MEDIAN_BUDGET_MS", 250.0);
    let p95_budget_ms = env_budget("NAHUALI_REFRESH_P95_BUDGET_MS", 1_000.0);
    let mut reports = Vec::new();
    let mut passed = true;

    for event_count in [1_000, 10_000] {
        let database = PathBuf::from(format!(
            "refresh_perf_{}_{}",
            std::process::id(),
            event_count
        ));
        let events = fixture_events(event_count);
        MemoryEngine::replace_record_ledger_for_regression(&database, &events)?;
        let mut memory = MemoryEngine::open(&database)?;

        for _ in 0..3 {
            let outcome = memory.refresh_if_changed()?;
            assert!(!outcome.changed);
            assert_eq!(outcome.replayed_event_count, 0);
        }

        let mut samples_ms = Vec::with_capacity(SAMPLE_COUNT);
        for _ in 0..SAMPLE_COUNT {
            let started = Instant::now();
            let outcome = memory.refresh_if_changed()?;
            samples_ms.push(started.elapsed().as_secs_f64() * 1_000.0);
            assert!(!outcome.changed);
            assert_eq!(outcome.replayed_event_count, 0);
        }
        samples_ms.sort_by(f64::total_cmp);
        let median_ms = samples_ms[SAMPLE_COUNT / 2];
        let p95_ms = samples_ms[SAMPLE_COUNT - 1];
        let case_passed = median_ms <= median_budget_ms && p95_ms <= p95_budget_ms;
        passed &= case_passed;
        reports.push(serde_json::json!({
            "event_count": event_count,
            "samples": SAMPLE_COUNT,
            "median_ms": median_ms,
            "p95_ms": p95_ms,
            "median_budget_ms": median_budget_ms,
            "p95_budget_ms": p95_budget_ms,
            "full_replays": 0,
            "passed": case_passed,
        }));

        MemoryEngine::replace_record_ledger_for_regression(&database, &[])?;
    }

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "benchmark": "unchanged-ledger-refresh",
            "schema_version": 1,
            "cases": reports,
            "passed": passed,
        }))?
    );
    if !passed {
        std::process::exit(1);
    }
    Ok(())
}

fn fixture_events(count: usize) -> Vec<EventEnvelope> {
    (1..=count)
        .map(|sequence| {
            EventEnvelope::new(
                sequence as u64,
                sequence as u64,
                MemoryEvent::EpisodeRecorded(EpisodeRecorded {
                    id: format!("episode_refresh_{sequence}"),
                    content: format!("Deterministic refresh fixture event {sequence}"),
                    tags: vec!["refresh-performance".to_string()],
                    mentions: Vec::new(),
                    source_id: None,
                    source_position: None,
                    source_role: None,
                    scope: None,
                }),
            )
        })
        .collect()
}

fn env_budget(name: &str, default: f64) -> f64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(default)
}
