# Regression Fixtures

These fixtures are release-gate checks for the Rust implementation. They are not
the public Knowledge Health Benchmark and should not be described as a benchmark
score or third-party certification.

Run the current fixture suite:

```bash
cargo run -p nahuali-regression -- --fixtures fixtures/knowledge-health-regression.json
cargo run -p nahuali-regression -- --fixtures fixtures/recall-regression.json
```

Write a report artifact:

```bash
mkdir -p regression-results
cargo run -p nahuali-regression -- \
  --fixtures fixtures/knowledge-health-regression.json \
  --output regression-results/knowledge-health.json
cargo run -p nahuali-regression -- \
  --fixtures fixtures/recall-regression.json \
  --output regression-results/recall.json
```

`knowledge-health-regression.json` tracks:

- empty-store inspection
- recall with evidence attribution
- unsupported fact surfacing
- low-confidence surfacing
- contradiction surfacing
- deterministic staleness surfacing
- relation-aware isolation checks
- calibrated signal severity and dimensions
- deterministic authority modes, scores, and trust flags
- SurrealDB database validation
- corrupted checksum rejection
- out-of-order sequence rejection
- focused recall ranking

`recall-regression.json` tracks:

- empty no-match recall results
- partial lexical matching
- canonical claim and link recall kinds
- procedure and intention recall
- authority mode coupling for weak recall context

The report is JSON and includes every fixture, check, pass/fail result, and
failure detail. CI runs this fixture suite through
`scripts/validate-clean-tree.sh`.

Public benchmark work belongs in the separate Knowledge Health Benchmark
repository. This workspace only keeps synthetic regression fixtures for
Nahuali's own core contract.
