#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{FixtureFile, run_fixtures};

    #[test]
    fn bundled_fixtures_pass() {
        let fixtures = bundled_knowledge_health_fixtures();

        let report = run_fixtures(fixtures).unwrap();

        assert_eq!(report.failed, 0);
        assert!(report.passed >= 3);
    }

    #[test]
    fn bundled_recall_fixtures_pass() {
        let fixtures = bundled_recall_fixtures();

        let report = run_fixtures(fixtures).unwrap();

        assert_eq!(report.failed, 0);
        assert!(report.passed >= 4);
    }

    #[test]
    fn bundled_fixture_ids_are_unique() {
        let mut seen = BTreeSet::new();
        for fixtures in [
            bundled_knowledge_health_fixtures(),
            bundled_recall_fixtures(),
        ] {
            for fixture in &fixtures.fixtures {
                assert!(
                    seen.insert(fixture.id.clone()),
                    "duplicate fixture id: {}",
                    fixture.id
                );
            }
        }
    }

    #[test]
    fn bundled_fixtures_are_order_independent() {
        let fixtures = bundled_knowledge_health_fixtures();
        let mut reversed = fixtures.clone();
        reversed.fixtures.reverse();

        let forward = run_fixtures(fixtures).unwrap();
        let reversed = run_fixtures(reversed).unwrap();

        assert_eq!(forward.failed, 0);
        assert_eq!(reversed.failed, 0);

        let forward_ids = forward
            .results
            .into_iter()
            .map(|result| result.id)
            .collect::<BTreeSet<_>>();
        let reversed_ids = reversed
            .results
            .into_iter()
            .map(|result| result.id)
            .collect::<BTreeSet<_>>();
        assert_eq!(forward_ids, reversed_ids);
    }

    #[test]
    fn invalid_record_ledger_expectations_pass() {
        let fixtures: FixtureFile = serde_json::from_str(
            r#"
            {
              "version": 1,
              "fixtures": [
                {
                  "id": "inline_corrupt_checksum",
                  "goal": "Corrupt checksums fail closed.",
                  "steps": [
                    {
                      "type": "raw_event",
                      "sequence": 1,
                      "timestamp_ms": 1000,
                      "corrupt_checksum": true,
                      "payload": {
                        "type": "episode_recorded",
                        "id": "episode_1",
                        "content": "Synthetic memory",
                        "tags": []
                      }
                    },
                    { "type": "write_raw_records" }
                  ],
                  "expected": {
                    "open_error_contains": "checksum mismatch"
                  }
                }
              ]
            }
            "#,
        )
        .unwrap();

        let report = run_fixtures(fixtures).unwrap();

        assert_eq!(report.failed, 0);
    }

    fn bundled_knowledge_health_fixtures() -> FixtureFile {
        serde_json::from_str(include_str!(
            "../../../../fixtures/knowledge-health-regression.json"
        ))
        .unwrap()
    }

    fn bundled_recall_fixtures() -> FixtureFile {
        serde_json::from_str(include_str!("../../../../fixtures/recall-regression.json")).unwrap()
    }
}
