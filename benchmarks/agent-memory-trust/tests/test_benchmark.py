import importlib.util
import json
import pathlib
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


ROOT = pathlib.Path(__file__).resolve().parents[3]
ADAPTER_PATH = ROOT / "benchmarks/agent-memory-trust/adapters/nahuali.py"
SCORE_PATH = ROOT / "benchmarks/agent-memory-trust/score.py"


def load_adapter():
    spec = importlib.util.spec_from_file_location("nahuali_trust_adapter", ADAPTER_PATH)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def valid_result():
    digest = "a" * 64
    return {
        "benchmarkVersion": "1.0.0",
        "system": {"name": "Nahuali", "version": "nahuali 0.8.0-beta.5"},
        "commit": f"sha256:{digest}",
        "artifact": {"sha256": digest, "sourceRevision": "f" * 40},
        "runner": {"relationship": "first-party", "adapter": str(ADAPTER_PATH)},
        "environment": {"services": [], "models": [], "operatorActions": []},
        "cases": [
            {"id": "evidence-traceability", "status": "pass", "evidenceIds": ["e1"], "normalizedVerdict": "trusted"},
            {"id": "unsupported-memory-abstention", "status": "pass", "evidenceIds": [], "normalizedVerdict": "refused"},
            {"id": "contradiction-detection", "status": "pass", "detected": True, "normalizedVerdict": "refused"},
            {"id": "staleness-signaling", "status": "pass", "detected": True, "normalizedVerdict": "qualified"},
            {"id": "non-mutating-inspection", "status": "pass", "detected": True, "mutated": False},
            {"id": "in-place-tamper-detection", "status": "pass", "detected": True},
            {"id": "full-rechain-detection", "status": "pass", "detected": True, "externalCheckpoint": True},
        ],
    }


class AdapterTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.adapter = load_adapter()

    def test_native_modes_are_normalized_without_expected_value_shortcuts(self):
        self.assertEqual(self.adapter.normalize_verdict("certify"), "trusted")
        self.assertEqual(self.adapter.normalize_verdict("block"), "refused")
        self.assertEqual(self.adapter.normalize_verdict("warn", False), "refused")
        self.assertEqual(self.adapter.normalize_verdict("warn"), "qualified")
        self.assertEqual(self.adapter.normalize_verdict("advisory"), "qualified")

    def test_observed_case_marks_failed_runtime_behavior(self):
        case = self.adapter.observed_case("contradiction-detection", False, detected=True)
        self.assertEqual(case["status"], "fail")

    def test_source_revision_requires_lowercase_hex(self):
        with self.assertRaisesRegex(SystemExit, "lowercase"):
            self.adapter.validate_source_revision("A" * 40)
        with self.assertRaisesRegex(SystemExit, "40-character"):
            self.adapter.validate_source_revision("a" * 39)
        self.assertEqual(self.adapter.validate_source_revision("a" * 40), "a" * 40)

    def test_release_identity_is_all_or_nothing(self):
        self.assertEqual(
            self.adapter.release_artifact_metadata(None, None, None, None),
            {"kind": "source-build"},
        )
        with self.assertRaisesRegex(SystemExit, "requires"):
            self.adapter.release_artifact_metadata("v1", None, None, None)
        published = self.adapter.release_artifact_metadata(
            "v1", "nahuali-v1-target.tar.gz", "target", "b" * 64
        )
        self.assertEqual(published["kind"], "published-release")

    def test_run_cleans_temporary_directory_after_failure(self):
        with tempfile.TemporaryDirectory() as parent:
            root = pathlib.Path(parent) / "benchmark"
            root.mkdir()
            with mock.patch.object(self.adapter.tempfile, "mkdtemp", return_value=str(root)):
                with mock.patch.object(
                    self.adapter, "run_at_root", side_effect=RuntimeError("fixture")
                ):
                    with self.assertRaisesRegex(RuntimeError, "fixture"):
                        self.adapter.run(sys.executable, "a" * 40)
            self.assertFalse(root.exists())

    def score(self, result):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / "result.json"
            path.write_text(json.dumps(result), encoding="utf-8")
            return subprocess.run(
                [sys.executable, str(SCORE_PATH), str(path)],
                text=True,
                capture_output=True,
            )

    def test_score_rejects_native_verdict_regression(self):
        mutations = {
            "missing evidence": lambda cases: cases[0].update(evidenceIds=[]),
            "unsupported accepted": lambda cases: cases[1].update(normalizedVerdict="qualified"),
            "contradiction missed": lambda cases: cases[2].update(detected=False),
            "staleness missed": lambda cases: cases[3].update(detected=False),
            "inspection mutated": lambda cases: cases[4].update(mutated=True),
            "tamper missed": lambda cases: cases[5].update(detected=False),
            "rechain missed": lambda cases: cases[6].update(externalCheckpoint=False),
        }
        for label, mutate in mutations.items():
            with self.subTest(label=label):
                result = valid_result()
                mutate(result["cases"])
                scored = self.score(result)
                self.assertNotEqual(scored.returncode, 0)

    def test_score_rejects_artifact_identity_mismatch(self):
        result = valid_result()
        result["commit"] = "sha256:" + "b" * 64
        scored = self.score(result)
        self.assertNotEqual(scored.returncode, 0)
        self.assertIn("must match", scored.stderr)

    def test_score_rejects_uppercase_identity(self):
        result = valid_result()
        result["artifact"]["sha256"] = "A" * 64
        result["commit"] = "sha256:" + "A" * 64
        scored = self.score(result)
        self.assertNotEqual(scored.returncode, 0)
        self.assertIn("lowercase", scored.stderr)


if __name__ == "__main__":
    unittest.main()
