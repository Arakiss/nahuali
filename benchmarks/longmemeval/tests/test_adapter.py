import hashlib
import importlib.util
import json
import os
import pathlib
import stat
import tempfile
import unittest
from unittest import mock


ROOT = pathlib.Path(__file__).resolve().parents[3]
ADAPTER_PATH = ROOT / "benchmarks/longmemeval/adapter.py"
FIXTURE_PATH = ROOT / "benchmarks/longmemeval/fixtures/smoke.json"
EDGE_FIXTURE_PATH = ROOT / "benchmarks/longmemeval/fixtures/corpus_edges.json"


def load_adapter():
    spec = importlib.util.spec_from_file_location("longmemeval_adapter", ADAPTER_PATH)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


adapter = load_adapter()


class LongMemEvalAdapterTests(unittest.TestCase):
    def test_streaming_loader_handles_small_chunks(self):
        expected = json.loads(FIXTURE_PATH.read_text(encoding="utf-8"))
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / "dataset.json"
            path.write_text(json.dumps(expected), encoding="utf-8")
            observed = list(adapter.iter_json_array(path, chunk_size=7))
        self.assertEqual(observed, expected)

    def test_official_retrieval_metrics_match_published_formula(self):
        metrics = adapter.retrieval_metrics(["wrong", "a", "b"], ["a", "b"])
        self.assertEqual(metrics["recall_any@1"], 0.0)
        self.assertEqual(metrics["recall_any@3"], 1.0)
        self.assertEqual(metrics["recall_all@3"], 1.0)
        expected_ndcg = (
            1.0 / adapter.math.log2(2) + 1.0 / adapter.math.log2(3)
        ) / 2.0
        self.assertAlmostEqual(metrics["ndcg_any@3"], expected_ndcg)

    def test_interchange_preserves_session_and_turn_provenance(self):
        entry = adapter.validate_question(
            json.loads(FIXTURE_PATH.read_text(encoding="utf-8"))[0]
        )
        document, manifest = adapter.build_interchange(entry)
        self.assertEqual(document["version"], 1)
        self.assertEqual(manifest["source_count"], 3)
        self.assertEqual(manifest["raw_turn_count"], 6)
        self.assertEqual(manifest["indexed_turn_count"], 6)
        self.assertEqual(manifest["skipped_empty_turn_count"], 0)
        self.assertEqual(document["sources"][1]["metadata"]["session_id"], "answer_city")
        self.assertEqual(
            document["sources"][1]["metadata"]["session_date"],
            "2024/01/02 (Tue) 10:00",
        )
        self.assertEqual(document["episodes"][2]["source_role"], "user")
        self.assertEqual(document["episodes"][2]["source_position"], 1)
        self.assertEqual(
            document["episodes"][2]["scope"]["key"], "custom:longmemeval_smoke_city"
        )

    def test_official_scalar_answer_shapes_are_valid_json_values(self):
        template = json.loads(FIXTURE_PATH.read_text(encoding="utf-8"))[0]
        for answer in (42, 4.2, True, None, "Kyoto", ["Kyoto"], {"city": "Kyoto"}):
            with self.subTest(answer=answer):
                entry = dict(template)
                entry["answer"] = answer
                self.assertIs(adapter.validate_question(entry), entry)
        self.assertFalse(adapter.is_json_value(float("nan")))

    def test_duplicate_sessions_use_positional_refs_and_canonical_credit(self):
        entry = adapter.validate_question(
            json.loads(EDGE_FIXTURE_PATH.read_text(encoding="utf-8"))[0]
        )
        document, manifest = adapter.build_interchange(entry)
        self.assertEqual(manifest["raw_session_occurrence_count"], 3)
        self.assertEqual(manifest["canonical_session_id_count"], 2)
        self.assertEqual(manifest["duplicate_session_id_count"], 1)
        self.assertEqual(manifest["duplicate_session_occurrence_count"], 1)
        self.assertEqual(manifest["raw_turn_count"], 6)
        self.assertEqual(manifest["indexed_turn_count"], 5)
        self.assertEqual(manifest["skipped_empty_turn_count"], 1)
        self.assertEqual(
            [source["ref"] for source in document["sources"]],
            ["session-1", "session-2", "session-3"],
        )
        self.assertEqual(len({source["uri"] for source in document["sources"]}), 3)
        repeated = document["sources"][:2]
        self.assertEqual(
            [source["metadata"]["canonical_session_id"] for source in repeated],
            ["repeated-session", "repeated-session"],
        )
        self.assertEqual(
            [source["metadata"]["session_occurrence"] for source in repeated],
            ["1", "2"],
        )
        self.assertTrue(
            all(
                isinstance(value, str)
                for source in document["sources"]
                for value in source["metadata"].values()
            )
        )
        self.assertNotIn("turn-1-1", {episode["ref"] for episode in document["episodes"]})
        self.assertEqual(document["episodes"][0]["source_position"], 2)

        ranked = adapter.ranked_sessions(
            [{"id": "second"}, {"id": "first"}],
            {
                "second": {
                    "session_id": "repeated-session",
                    "session_ref": "session-2",
                    "session_position": "2",
                    "session_occurrence": "2",
                },
                "first": {
                    "session_id": "repeated-session",
                    "session_ref": "session-1",
                    "session_position": "1",
                    "session_occurrence": "1",
                },
            },
        )
        self.assertEqual(len(ranked), 1)
        self.assertEqual(ranked[0]["session_id"], "repeated-session")
        self.assertEqual(ranked[0]["source_session_ref"], "session-2")

    def test_smoke_fixture_has_two_scorable_questions_and_one_abstention(self):
        entries = [adapter.validate_question(item) for item in adapter.iter_json_array(FIXTURE_PATH)]
        self.assertEqual(len(entries), 3)
        self.assertEqual(sum("_abs" in item["question_id"] for item in entries), 1)
        self.assertEqual(sum(bool(item["answer_session_ids"]) for item in entries), 2)

    def test_question_database_is_a_valid_stable_identifier(self):
        first = adapter.question_database("a" * 64, "question-1")
        second = adapter.question_database("a" * 64, "question-1")
        self.assertEqual(first, second)
        self.assertRegex(first, r"^[a-z][a-z0-9_]+$")

    def test_local_model_mode_is_added_only_when_configured(self):
        with mock.patch.dict("os.environ", {}, clear=True):
            self.assertEqual(
                adapter.selected_modes(None), ["lexical", "deterministic-hybrid"]
            )
        with mock.patch.dict(
            "os.environ", {"NAHUALI_LOCAL_EMBEDDING_MODEL_PATH": "/tmp/model"}, clear=True
        ):
            self.assertEqual(
                adapter.selected_modes(None),
                ["lexical", "deterministic-hybrid", "local-model-hybrid"],
            )

    def test_qdrant_endpoint_class_rejects_implicit_remote_transfer(self):
        self.assertEqual(
            adapter.qdrant_endpoint_class("http://localhost:16333"), "loopback"
        )
        self.assertEqual(
            adapter.qdrant_endpoint_class("https://127.0.0.1:6333"), "loopback"
        )
        self.assertEqual(
            adapter.qdrant_endpoint_class("http://[::1]:6333"), "loopback"
        )
        self.assertEqual(
            adapter.qdrant_endpoint_class("https://vectors.example.test"), "remote"
        )
        with self.assertRaises(adapter.AdapterError):
            adapter.qdrant_endpoint_class("vectors.example.test")

    def test_hypotheses_template_has_only_official_fields(self):
        results = [{"question_id": "q1"}, {"question_id": "q2_abs"}]
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / "hypotheses.ndjson"
            adapter.write_hypotheses_template(path, results)
            records = [json.loads(line) for line in path.read_text().splitlines()]
        self.assertEqual(records[0], {"question_id": "q1", "hypothesis": ""})
        self.assertEqual(records[1], {"question_id": "q2_abs", "hypothesis": ""})

    def test_download_reuses_only_digest_verified_cache_entry(self):
        revision = "b" * 40
        content = b"synthetic cached dataset"
        expected_sha256 = hashlib.sha256(content).hexdigest()
        with tempfile.TemporaryDirectory() as directory:
            cache = pathlib.Path(directory)
            target = cache / revision / adapter.OFFICIAL_DATASET_FILENAME
            target.parent.mkdir(parents=True)
            target.write_bytes(content)
            result = adapter.download_official_dataset(
                adapter.argparse.Namespace(
                    cache_dir=cache,
                    revision=revision,
                    expected_sha256=expected_sha256,
                )
            )
        self.assertEqual(result["status"], "cached")
        self.assertEqual(result["sha256"], expected_sha256)

    def test_preflight_walks_every_entry_and_reports_ingestion_counts(self):
        digest = hashlib.sha256(FIXTURE_PATH.read_bytes()).hexdigest()
        with mock.patch.multiple(
            adapter,
            OFFICIAL_DATASET_SIZE=FIXTURE_PATH.stat().st_size,
            OFFICIAL_DATASET_SHA256=digest,
            OFFICIAL_QUESTION_COUNT=3,
        ):
            result = adapter.preflight_official_dataset(FIXTURE_PATH)
        self.assertEqual(result["status"], "compatible")
        self.assertEqual(result["question_count"], 3)
        self.assertEqual(result["answer_types"], {"string": 3})
        self.assertEqual(result["raw_turn_count"], 16)
        self.assertEqual(result["indexed_turn_count"], 16)
        self.assertEqual(result["skipped_empty_turn_count"], 0)

    def test_non_pinned_dataset_identity_requires_matching_explicit_digest(self):
        digest = hashlib.sha256(FIXTURE_PATH.read_bytes()).hexdigest()
        with self.assertRaises(adapter.AdapterError):
            adapter.run_dataset_identity(FIXTURE_PATH, "fixture-v1", None)
        identity = adapter.run_dataset_identity(FIXTURE_PATH, "fixture-v1", digest)
        self.assertEqual(identity["sha256"], digest)
        self.assertEqual(
            identity["identity_policy"],
            "operator_supplied_sha256_for_non_pinned_revision",
        )

    def test_atomic_outputs_are_owner_only(self):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / "result.json"
            adapter.atomic_write_text(path, "{}\n")
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)

    def test_path_collision_detects_resolved_and_hard_link_aliases(self):
        with tempfile.TemporaryDirectory() as directory:
            original = pathlib.Path(directory) / "dataset.json"
            alias = pathlib.Path(directory) / "alias.json"
            original.write_text("[]", encoding="utf-8")
            os.link(original, alias)
            self.assertTrue(adapter.paths_refer_to_same_file(original, alias))

    def test_runtime_environment_excludes_host_identity(self):
        environment = adapter.runtime_environment()
        self.assertIn("operating_system", environment)
        self.assertIn("architecture", environment)
        self.assertIn("python_version", environment)
        self.assertIn("logical_cpu_count", environment)
        self.assertNotIn("hostname", environment)
        self.assertNotIn("username", environment)

    def test_source_worktree_state_discloses_clean_dirty_or_unavailable(self):
        self.assertIn(adapter.source_worktree_state(), {"clean", "dirty", "unavailable"})


if __name__ == "__main__":
    unittest.main()
