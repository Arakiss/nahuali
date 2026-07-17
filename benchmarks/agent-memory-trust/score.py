#!/usr/bin/env python3
"""Validate and summarize an Agent Memory Trust Benchmark result."""

import json
import pathlib
import re
import sys


def fail(message: str) -> None:
    raise SystemExit(message)


LOWER_SHA256 = re.compile(r"^[0-9a-f]{64}$")
LOWER_SOURCE_REVISION = re.compile(r"^[0-9a-f]{40}$")


if len(sys.argv) != 2:
    fail("usage: score.py RESULT.json")

root = pathlib.Path(__file__).resolve().parent
cases_document = json.loads((root / "cases.json").read_text(encoding="utf-8"))
result = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))

if result.get("benchmarkVersion") != cases_document["benchmarkVersion"]:
    fail("result benchmarkVersion does not match cases.json")
if not result.get("system", {}).get("name") or not result.get("system", {}).get("version"):
    fail("result must identify the system name and version")
artifact_digest = result.get("artifact", {}).get("sha256")
if not LOWER_SHA256.fullmatch(artifact_digest or ""):
    fail("result must include the tested artifact SHA-256 as lowercase hexadecimal")
if result.get("commit") != f"sha256:{artifact_digest}":
    fail("result commit identity must match the tested artifact SHA-256")
source_revision = result.get("artifact", {}).get("sourceRevision")
if not LOWER_SOURCE_REVISION.fullmatch(source_revision or ""):
    fail("result must include the exact source revision as lowercase hexadecimal")
artifact_kind = result.get("artifact", {}).get("kind")
if artifact_kind not in {None, "source-build", "published-release"}:
    fail("result artifact kind must be source-build or published-release")
if artifact_kind == "published-release":
    for field in ("releaseTag", "releaseAsset", "target"):
        if not result["artifact"].get(field):
            fail(f"published release result must include artifact.{field}")
    if not LOWER_SHA256.fullmatch(result["artifact"].get("archiveSha256", "")):
        fail("published release result must include artifact.archiveSha256")
if result.get("runner", {}).get("relationship") not in {"first-party", "independent"}:
    fail("result must identify the runner as first-party or independent")

expected = {case["id"]: case for case in cases_document["cases"]}
reported = {case.get("id"): case for case in result.get("cases", [])}
if len(reported) != len(result.get("cases", [])):
    fail("result contains a missing or duplicate case id")
if set(reported) != set(expected):
    fail("result case ids do not exactly match cases.json")

allowed = {"pass", "fail", "unsupported"}
summary = {"pass": 0, "fail": 0, "unsupported": 0}
capabilities = []
for case_id, definition in expected.items():
    observation = reported[case_id]
    status = observation.get("status")
    if status not in allowed:
        fail(f"{case_id}: status must be pass, fail, or unsupported")
    if status == "pass":
        required = definition["required"]
        evidence = required.get("evidence")
        evidence_ids = observation.get("evidenceIds", [])
        if evidence == "present" and not evidence_ids:
            status = "fail"
        if evidence == "absent" and evidence_ids:
            status = "fail"
        expected_verdict = required.get("normalizedVerdict")
        actual_verdict = observation.get("normalizedVerdict")
        if expected_verdict == "qualified_or_refused":
            if actual_verdict not in {"qualified", "refused"}:
                status = "fail"
        elif expected_verdict and actual_verdict != expected_verdict:
            status = "fail"
        for field in ("detected", "mutated", "externalCheckpoint"):
            if field in required and observation.get(field) != required[field]:
                status = "fail"
    summary[status] += 1
    capabilities.append(
        {
            "id": case_id,
            "capability": definition["capability"],
            "status": status,
        }
    )

output = {
    "benchmarkVersion": result["benchmarkVersion"],
    "system": result.get("system"),
    "commit": result.get("commit"),
    "sourceRevision": result.get("artifact", {}).get("sourceRevision"),
    "summary": summary,
    "capabilities": capabilities,
}
print(json.dumps(output, indent=2, sort_keys=True))
if summary["fail"]:
    raise SystemExit(1)
