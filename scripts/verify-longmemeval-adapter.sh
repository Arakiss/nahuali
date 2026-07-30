#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ -n "${NAHUALI_LONGMEMEVAL_BIN:-}" ]]; then
  binary="$NAHUALI_LONGMEMEVAL_BIN"
else
  cargo build --locked -p nahuali-cli
  binary="target/debug/nahuali"
fi

if [[ ! -x "$binary" ]]; then
  echo "Nahuali binary is not executable: $binary" >&2
  exit 1
fi

"$binary" import --help | grep -F "source-neutral memory interchange" >/dev/null
"$binary" recall --help | grep -F -- "--semantic" >/dev/null

python3 -m unittest discover -s benchmarks/longmemeval/tests -v

temporary="$(mktemp -d "${TMPDIR:-/tmp}/nahuali-longmemeval.XXXXXX")"
trap 'rm -r "$temporary"' EXIT

official_dataset=""
cache_dir="${NAHUALI_LONGMEMEVAL_CACHE_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/nahuali/longmemeval}"
pinned_dataset="$cache_dir/98d7416c24c778c2fee6e6f3006e7a073259d48f/longmemeval_s_cleaned.json"
if [[ -n "${NAHUALI_LONGMEMEVAL_DATASET:-}" ]]; then
  official_dataset="$NAHUALI_LONGMEMEVAL_DATASET"
elif [[ -f "$pinned_dataset" ]]; then
  official_dataset="$pinned_dataset"
elif [[ "${NAHUALI_LONGMEMEVAL_REQUIRE_OFFICIAL:-0}" == "1" ]]; then
  download_record="$temporary/download.json"
  downloaded=0
  for attempt in 1 2 3; do
    if python3 benchmarks/longmemeval/adapter.py download \
      --cache-dir "$cache_dir" >"$download_record"; then
      downloaded=1
      break
    fi
    echo "LongMemEval official dataset download attempt $attempt failed" >&2
  done
  if [[ "$downloaded" != "1" ]]; then
    echo "Unable to obtain the exact pinned LongMemEval dataset" >&2
    exit 1
  fi
  official_dataset="$(python3 - "$download_record" <<'PY'
import json
import pathlib
import sys

record = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
print(record["path"])
PY
)"
else
  echo "LongMemEval exact-corpus preflight skipped: pinned cache is absent." >&2
  echo "Set NAHUALI_LONGMEMEVAL_REQUIRE_OFFICIAL=1 for the manual/full gate." >&2
fi

if [[ -n "$official_dataset" ]]; then
  official_preflight="$temporary/official-preflight.json"
  python3 benchmarks/longmemeval/adapter.py preflight \
    --dataset "$official_dataset" >"$official_preflight"

  python3 - "$official_preflight" <<'PY'
import json
import pathlib
import sys

result = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
assert result["status"] == "compatible"
assert result["question_count"] == 500
assert result["answer_types"] == {"integer": 32, "string": 468}
assert result["raw_session_occurrence_count"] == 23867
assert result["canonical_session_id_count"] == 23854
assert result["duplicate_session_question_count"] == 13
assert result["duplicate_session_id_count"] == 13
assert result["duplicate_session_occurrence_count"] == 13
assert result["raw_turn_count"] == 246750
assert result["indexed_turn_count"] == 246738
assert result["skipped_empty_turn_count"] == 12
print("LongMemEval exact 500-question corpus preflight passed")
PY
fi

python3 - <<'PY'
import json
import os
import urllib.request

url = os.environ.get("NAHUALI_QDRANT_URL", "http://localhost:16333").rstrip("/")
request = urllib.request.Request(url + "/collections")
token = os.environ.get("NAHUALI_QDRANT_API_KEY", "").strip()
if token:
    request.add_header("api-key", token)
try:
    with urllib.request.urlopen(request, timeout=5) as response:
        payload = json.load(response)
except Exception as error:
    raise SystemExit(
        "local Qdrant is required for deterministic-hybrid smoke coverage; "
        "run scripts/ensure-dev-stack.sh first ({}).".format(error)
    )
if payload.get("status") != "ok":
    raise SystemExit("local Qdrant returned an unexpected health response")
PY

report="$temporary/report.json"
raw="$temporary/raw.ndjson"
hypotheses="$temporary/hypotheses.ndjson"
source_revision="$(git rev-parse HEAD)"
fixture_sha256="$(python3 - <<'PY'
import hashlib
import pathlib

print(hashlib.sha256(pathlib.Path("benchmarks/longmemeval/fixtures/smoke.json").read_bytes()).hexdigest())
PY
)"
edge_sha256="$(python3 - <<'PY'
import hashlib
import pathlib

print(hashlib.sha256(pathlib.Path("benchmarks/longmemeval/fixtures/corpus_edges.json").read_bytes()).hexdigest())
PY
)"

edge_report="$temporary/edge-report.json"
edge_raw="$temporary/edge-raw.ndjson"
env -u NAHUALI_LOCAL_EMBEDDING_MODEL_PATH \
  python3 benchmarks/longmemeval/adapter.py run \
    --dataset benchmarks/longmemeval/fixtures/corpus_edges.json \
    --dataset-version synthetic-corpus-edges-v1 \
    --dataset-revision fixture-corpus-edges-v1 \
    --expected-dataset-sha256 "$edge_sha256" \
    --binary "$binary" \
    --source-revision "$source_revision" \
    --mode lexical \
    --measured-runs 1 \
    --output "$edge_report" \
    --raw-output "$edge_raw"

python3 benchmarks/longmemeval/adapter.py validate "$edge_report"

python3 - "$edge_report" "$edge_raw" <<'PY'
import json
import pathlib
import stat
import sys

report_path, raw_path = map(pathlib.Path, sys.argv[1:])
report = json.loads(report_path.read_text(encoding="utf-8"))
raw = [json.loads(line) for line in raw_path.read_text(encoding="utf-8").splitlines()]
assert len(raw) == 1
assert raw[0]["answer"] == 42
assert raw[0]["ingestion"]["raw_session_occurrence_count"] == 3
assert raw[0]["ingestion"]["canonical_session_id_count"] == 2
assert raw[0]["ingestion"]["duplicate_session_id_count"] == 1
assert raw[0]["ingestion"]["duplicate_session_occurrence_count"] == 1
assert raw[0]["ingestion"]["raw_turn_count"] == 6
assert raw[0]["ingestion"]["indexed_turn_count"] == 5
assert raw[0]["ingestion"]["skipped_empty_turn_count"] == 1
assert report["dataset"]["complete_dataset"] is True
assert all(stat.S_IMODE(path.stat().st_mode) == 0o600 for path in (report_path, raw_path))
print("LongMemEval real corpus-edge import passed")
PY

if [[ -n "$official_dataset" ]]; then
  official_sample_report="$temporary/official-first-four.json"
  env -u NAHUALI_LOCAL_EMBEDDING_MODEL_PATH \
    python3 benchmarks/longmemeval/adapter.py run \
      --dataset "$official_dataset" \
      --dataset-version LongMemEval-v1-cleaned-S \
      --dataset-revision 98d7416c24c778c2fee6e6f3006e7a073259d48f \
      --expected-dataset-sha256 d6f21ea9d60a0d56f34a05b609c79c88a451d2ae03597821ea3d5a9678c3a442 \
      --binary "$binary" \
      --source-revision "$source_revision" \
      --mode lexical \
      --measured-runs 1 \
      --limit 4 \
      --output "$official_sample_report"

  python3 benchmarks/longmemeval/adapter.py validate "$official_sample_report"

  python3 - "$official_sample_report" <<'PY'
import json
import pathlib
import sys

report = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
assert report["dataset"]["selected_question_count"] == 4
assert report["dataset"]["selection_limit"] == 4
assert report["dataset"]["selection_policy"] == "dataset_order_prefix"
assert report["dataset"]["complete_dataset"] is False
assert report["dataset"]["identity_policy"] == "pinned_official_revision_sha256_and_size"
print("LongMemEval first four official questions imported and retrieved")
PY
fi

env -u NAHUALI_LOCAL_EMBEDDING_MODEL_PATH \
  python3 benchmarks/longmemeval/adapter.py run \
    --dataset benchmarks/longmemeval/fixtures/smoke.json \
    --dataset-version synthetic-smoke-v1 \
    --dataset-revision fixture-v1 \
    --expected-dataset-sha256 "$fixture_sha256" \
    --binary "$binary" \
    --source-revision "$source_revision" \
    --mode lexical \
    --mode deterministic-hybrid \
    --measured-runs 2 \
    --output "$report" \
    --raw-output "$raw" \
    --hypotheses-output "$hypotheses"

python3 benchmarks/longmemeval/adapter.py validate "$report"

python3 - "$report" "$raw" "$hypotheses" "$binary" <<'PY'
import hashlib
import json
import os
import pathlib
import stat
import sys
import urllib.request


report_path, raw_path, hypotheses_path, binary_path = map(pathlib.Path, sys.argv[1:])
report = json.loads(report_path.read_text(encoding="utf-8"))
raw = [json.loads(line) for line in raw_path.read_text(encoding="utf-8").splitlines()]
hypotheses = [
    json.loads(line) for line in hypotheses_path.read_text(encoding="utf-8").splitlines()
]

assert report["benchmark"]["task"] == "retrieval-only"
assert report["benchmark"]["relationship"] == "first-party"
assert report["benchmark"]["official_evaluator_revision"] == (
    "9e0b455f4ef0e2ab8f2e582289761153549043fc"
)
assert report["benchmark"]["qa_score"] is None
assert report["benchmark"]["qa_status"] == "not_evaluated"
assert report["runner"]["relationship"] == "first-party"
assert report["runner"]["environment"]["operating_system"]
assert report["runner"]["environment"]["architecture"]
assert report["runner"]["environment"]["python_version"]
assert report["runner"]["environment"]["logical_cpu_count"] >= 1
assert "hostname" not in report["runner"]["environment"]
assert report["system"]["source_worktree_state"] in {"clean", "dirty", "unavailable"}
assert report["system"]["source_revision_matches_start_head"] is True
assert report["system"]["source_head_stable"] is True
assert report["system"]["source_worktree_stable"] is True
assert "binary_path" not in report["system"]
assert "path" not in report["dataset"]
assert report["dataset"]["version_input"] == "synthetic-smoke-v1"
assert report["dataset"]["revision_input"] == "fixture-v1"
assert report["dataset"]["selected_question_count"] == 3
assert report["dataset"]["abstention_question_count"] == 1
assert report["dataset"]["selection_limit"] is None
assert report["dataset"]["selection_policy"] == "complete_dataset"
assert report["dataset"]["complete_dataset"] is True
assert report["dataset"]["identity_policy"] == (
    "operator_supplied_sha256_for_non_pinned_revision"
)
assert report["configuration"]["modes"] == ["lexical", "deterministic-hybrid"]
assert report["configuration"]["latency_population"] == (
    "all_selected_questions_including_abstentions"
)
assert report["configuration"]["local_model_artifacts"] is None
assert report["configuration"]["retrieval_latency_boundary"] == (
    "wall_clock_subprocess_invocation_including_cli_startup"
)
assert report["configuration"]["semantic_index_latency_boundary"] == (
    "wall_clock_subprocess_invocation_including_cli_startup"
)
assert len(report["raw_results"]) == len(raw) == len(hypotheses) == 3

abstentions = [result for result in raw if result["abstention"]]
assert len(abstentions) == 1
assert abstentions[0]["excluded_from_retrieval_metrics"] is True
assert abstentions[0]["exclusion_reason"] == "abstention"
assert all(mode["metrics"] is None for mode in abstentions[0]["modes"].values())
assert all(set(item) == {"question_id", "hypothesis"} for item in hypotheses)
assert all(item["hypothesis"] == "" for item in hypotheses)
assert report["qa_handoff"]["hypotheses_template_filename"] == hypotheses_path.name
assert report["artifact_handling"] == {
    "contains_dataset_content": True,
    "absolute_local_paths_recorded": False,
    "output_file_mode": "0600",
}
assert all(
    stat.S_IMODE(path.stat().st_mode) == 0o600
    for path in (report_path, raw_path, hypotheses_path)
)

databases = {result["ingestion"]["database"] for result in raw}
scopes = {result["ingestion"]["scope"] for result in raw}
assert len(databases) == len(scopes) == 3
assert all(result["ingestion"]["raw_dates_preserved"] for result in raw)
assert all(result["ingestion"]["turn_roles_preserved"] for result in raw)
assert all(result["ingestion"]["turn_positions_preserved"] for result in raw)

for mode in ("lexical", "deterministic-hybrid"):
    aggregate = report["aggregates"][mode]
    assert aggregate["evaluated_question_count"] == 2
    assert aggregate["excluded_question_count"] == 1
    assert aggregate["metrics"]["recall_any@1"] == 1.0
    assert aggregate["metrics"]["recall_all@3"] == 1.0
    assert aggregate["metrics"]["ndcg_any@3"] == 1.0
    assert aggregate["latency"]["retrieval_ms"]["sample_count"] == 6
assert report["aggregates"]["deterministic-hybrid"]["latency"]["index_ms"][
    "sample_count"
] == 3

fixture = pathlib.Path("benchmarks/longmemeval/fixtures/smoke.json")
assert report["dataset"]["sha256"] == hashlib.sha256(fixture.read_bytes()).hexdigest()
assert report["system"]["binary_sha256"] == hashlib.sha256(
    binary_path.read_bytes()
).hexdigest()

qdrant_url = os.environ.get("NAHUALI_QDRANT_URL", "http://localhost:16333").rstrip("/")
request = urllib.request.Request(qdrant_url + "/collections")
token = os.environ.get("NAHUALI_QDRANT_API_KEY", "").strip()
if token:
    request.add_header("api-key", token)
with urllib.request.urlopen(request, timeout=5) as response:
    payload = json.load(response)
collection_names = {
    collection["name"] for collection in payload["result"]["collections"]
}
assert not any(
    collection.endswith("__" + database)
    for collection in collection_names
    for database in databases
)

print("LongMemEval adapter smoke checks passed")
PY
