fn open_error_check(path: &Path, expected: &str) -> CheckResult {
    match MemoryEngine::open(path) {
        Ok(_) => CheckResult {
            name: "expected_open_error".to_string(),
            passed: false,
            detail: "database opened successfully".to_string(),
        },
        Err(error) => {
            let detail = error.to_string();
            CheckResult {
                name: "expected_open_error".to_string(),
                passed: detail.contains(expected),
                detail,
            }
        }
    }
}

fn write_raw_record_ledger(path: &Path, events: &[EventEnvelope]) -> anyhow::Result<()> {
    MemoryEngine::replace_record_ledger_for_regression(path, events)
        .with_context(|| format!("failed to write raw records to {}", path.display()))
}
