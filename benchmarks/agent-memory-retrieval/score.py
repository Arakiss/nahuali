#!/usr/bin/env python3
"""Validate and score one Nahuali retrieval benchmark result."""

import argparse
import hashlib
import json
import pathlib
import re
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from adapters.nahuali import aggregate_metrics, ranking_metrics


ROOT = pathlib.Path(__file__).resolve().parents[2]
CASES = ROOT / "benchmarks/agent-memory-retrieval/cases.json"
LOWER_SHA256 = re.compile(r"^[0-9a-f]{64}$")
LOWER_SOURCE_REVISION = re.compile(r"^[0-9a-f]{40}$")


def sha256_file(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def close(left: float, right: float) -> bool:
    return abs(left - right) <= 1e-9


def score_document(result: dict, cases: dict, cases_sha256: str) -> dict:
    if result.get("benchmarkVersion") != cases["benchmarkVersion"]:
        raise ValueError("benchmark version does not match cases.json")
    if result.get("corpus", {}).get("sha256") != cases_sha256:
        raise ValueError("result corpus digest does not match cases.json")
    artifact = result.get("artifact", {})
    if not LOWER_SHA256.fullmatch(artifact.get("sha256", "")):
        raise ValueError("result must bind the tested binary SHA-256 as lowercase hexadecimal")
    if not LOWER_SOURCE_REVISION.fullmatch(artifact.get("sourceRevision", "")):
        raise ValueError("result must bind the exact source revision as lowercase hexadecimal")
    artifact_kind = artifact.get("kind")
    if artifact_kind not in {None, "source-build", "published-release"}:
        raise ValueError("result artifact kind must be source-build or published-release")
    if artifact_kind == "published-release":
        for field in ("releaseTag", "releaseAsset", "target"):
            if not artifact.get(field):
                raise ValueError(f"published release result must include artifact.{field}")
        if not LOWER_SHA256.fullmatch(artifact.get("archiveSha256", "")):
            raise ValueError("published release result must include artifact.archiveSha256")

    query_cases = {query["id"]: query for query in cases["queries"]}
    summaries = {}
    for mode_name, contract in cases["modes"].items():
        mode = result.get("modes", {}).get(mode_name)
        if not mode:
            raise ValueError(f"mode {mode_name} is absent")
        if mode.get("status") != "complete":
            if contract["required"]:
                raise ValueError(f"required mode {mode_name} is not complete")
            if not mode.get("reason"):
                raise ValueError(f"optional mode {mode_name} has no explicit unavailability reason")
            summaries[mode_name] = {"status": mode["status"], "passed": None}
            continue

        reports = []
        observed_ids = set()
        for report in mode.get("queries", []):
            query_id = report.get("id")
            if query_id not in query_cases or query_id in observed_ids:
                raise ValueError(f"mode {mode_name} has an unknown or duplicate query {query_id}")
            observed_ids.add(query_id)
            expected = ranking_metrics(
                report["resultIds"],
                query_cases[query_id]["relevant"],
                cases["kValues"],
                cases["maxK"],
            )
            for key, value in expected.items():
                if not close(report["metrics"].get(key, -1), value):
                    raise ValueError(f"mode {mode_name} query {query_id} has invalid {key}")
            if len(report.get("latencyMs", [])) != cases["measuredRuns"]:
                raise ValueError(f"mode {mode_name} query {query_id} has incomplete latency samples")
            reports.append(report)
        if observed_ids != set(query_cases):
            raise ValueError(f"mode {mode_name} did not evaluate every query")

        expected_aggregate = aggregate_metrics(reports, cases["kValues"])
        for key, value in expected_aggregate.items():
            if key == "latencyMs":
                continue
            if not close(mode["metrics"].get(key, -1), value):
                raise ValueError(f"mode {mode_name} has invalid aggregate {key}")
        failures = {
            metric: {"observed": mode["metrics"].get(metric), "minimum": minimum}
            for metric, minimum in contract["minimum"].items()
            if mode["metrics"].get(metric, -1) < minimum
        }
        summaries[mode_name] = {
            "status": "complete",
            "passed": not failures,
            "metrics": mode["metrics"],
            "failures": failures,
        }

    required_passed = all(
        summaries[name]["passed"] is True
        for name, contract in cases["modes"].items()
        if contract["required"]
    )
    return {"passed": required_passed, "modes": summaries}


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("result", type=pathlib.Path)
    parser.add_argument("--cases", type=pathlib.Path, default=CASES)
    arguments = parser.parse_args()
    cases_path = arguments.cases.resolve()
    cases = json.loads(cases_path.read_text(encoding="utf-8"))
    result = json.loads(arguments.result.read_text(encoding="utf-8"))
    try:
        summary = score_document(result, cases, sha256_file(cases_path))
    except ValueError as error:
        raise SystemExit(f"retrieval benchmark invalid: {error}") from error
    print(json.dumps(summary, indent=2))
    if not summary["passed"]:
        raise SystemExit("retrieval benchmark thresholds failed")


if __name__ == "__main__":
    main()
