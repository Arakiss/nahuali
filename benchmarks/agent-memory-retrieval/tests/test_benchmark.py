import copy
import hashlib
import importlib.util
import json
import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[3]
ADAPTER_PATH = ROOT / "benchmarks/agent-memory-retrieval/adapters/nahuali.py"
SCORE_PATH = ROOT / "benchmarks/agent-memory-retrieval/score.py"
CASES_PATH = ROOT / "benchmarks/agent-memory-retrieval/cases.json"


def load_module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


adapter = load_module("retrieval_adapter", ADAPTER_PATH)
score = load_module("retrieval_score", SCORE_PATH)


class RetrievalBenchmarkTests(unittest.TestCase):
    def test_ranking_metrics_match_known_order(self):
        metrics = adapter.ranking_metrics(
            ["other", "relevant", "third"], ["relevant"], [1, 3, 5], 10
        )
        self.assertEqual(metrics["recallAt1"], 0.0)
        self.assertEqual(metrics["recallAt3"], 1.0)
        self.assertEqual(metrics["reciprocalRank"], 0.5)
        self.assertAlmostEqual(metrics["ndcgAt10"], 1.0 / adapter.math.log2(3))

    def test_scorer_rejects_tampered_query_metric(self):
        cases = json.loads(CASES_PATH.read_text(encoding="utf-8"))
        reports = []
        for query in cases["queries"]:
            ranking = list(query["relevant"])
            reports.append(
                {
                    "id": query["id"],
                    "resultIds": ranking,
                    "latencyMs": [1.0, 1.0, 1.0],
                    "metrics": adapter.ranking_metrics(
                        ranking, query["relevant"], cases["kValues"], cases["maxK"]
                    ),
                }
            )
        mode = {
            "status": "complete",
            "queries": reports,
            "metrics": adapter.aggregate_metrics(reports, cases["kValues"]),
        }
        result = {
            "benchmarkVersion": cases["benchmarkVersion"],
            "corpus": {"sha256": hashlib.sha256(CASES_PATH.read_bytes()).hexdigest()},
            "artifact": {"sha256": "a" * 64, "sourceRevision": "b" * 40},
            "modes": {
                "lexical": copy.deepcopy(mode),
                "deterministicHybrid": copy.deepcopy(mode),
                "localModelHybrid": {"status": "not_configured", "reason": "fixture"},
            },
        }
        result["modes"]["lexical"]["queries"][0]["metrics"]["recallAt1"] = 0.0

        with self.assertRaisesRegex(ValueError, "invalid recallAt1"):
            score.score_document(
                result, cases, hashlib.sha256(CASES_PATH.read_bytes()).hexdigest()
            )

    def test_optional_mode_must_explain_unavailability(self):
        cases = json.loads(CASES_PATH.read_text(encoding="utf-8"))
        reports = []
        for query in cases["queries"]:
            ranking = list(query["relevant"])
            reports.append(
                {
                    "id": query["id"],
                    "resultIds": ranking,
                    "latencyMs": [1.0, 1.0, 1.0],
                    "metrics": adapter.ranking_metrics(
                        ranking, query["relevant"], cases["kValues"], cases["maxK"]
                    ),
                }
            )
        complete = {
            "status": "complete",
            "queries": reports,
            "metrics": adapter.aggregate_metrics(reports, cases["kValues"]),
        }
        with self.assertRaisesRegex(ValueError, "no explicit unavailability reason"):
            score.score_document(
                {
                    "benchmarkVersion": cases["benchmarkVersion"],
                    "corpus": {"sha256": hashlib.sha256(CASES_PATH.read_bytes()).hexdigest()},
                    "artifact": {"sha256": "a" * 64, "sourceRevision": "b" * 40},
                    "modes": {
                        "lexical": copy.deepcopy(complete),
                        "deterministicHybrid": copy.deepcopy(complete),
                        "localModelHybrid": {"status": "not_configured"},
                    },
                },
                cases,
                hashlib.sha256(CASES_PATH.read_bytes()).hexdigest(),
            )


if __name__ == "__main__":
    unittest.main()
