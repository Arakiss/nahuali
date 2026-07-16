#!/usr/bin/env python3
"""Run the Agent Memory Trust Benchmark through Nahuali's public CLI."""

import argparse
import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import tempfile
import time
from typing import Optional


def command(binary: str, home: pathlib.Path, *args: str, json_output: bool = True):
    environment = os.environ.copy()
    environment["NAHUALI_HOME"] = str(home)
    environment["NO_COLOR"] = "1"
    environment.pop("NAHUALI_DB_URL", None)
    completed = subprocess.run(
        [binary, *args],
        check=True,
        capture_output=True,
        text=True,
        env=environment,
    )
    return json.loads(completed.stdout) if json_output else completed.stdout


def write_interchange(path: pathlib.Path, episodes, claims) -> pathlib.Path:
    path.mkdir(parents=True, exist_ok=True)
    document = path / "memory.json"
    document.write_text(
        json.dumps(
            {"version": 1, "episodes": episodes, "claims": claims},
            indent=2,
        ),
        encoding="utf-8",
    )
    return document


def resolve_binary(binary: str) -> pathlib.Path:
    candidate = pathlib.Path(binary).expanduser()
    if candidate.is_file():
        return candidate.resolve()
    resolved = shutil.which(binary)
    if resolved is None:
        raise SystemExit(f"benchmark binary not found: {binary}")
    return pathlib.Path(resolved).resolve()


def sha256_file(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def normalize_verdict(mode: str, can_trust: Optional[bool] = None) -> str:
    if can_trust is False:
        return "refused"
    if mode == "certify":
        return "trusted"
    if mode == "block":
        return "refused"
    return "qualified"


def observed_case(case_id: str, passed: bool, **observation) -> dict:
    return {"id": case_id, "status": "pass" if passed else "fail", **observation}


def run(binary: str, source_revision: Optional[str] = None) -> dict:
    binary_path = resolve_binary(binary)
    binary = str(binary_path)
    binary_sha256 = sha256_file(binary_path)
    root = pathlib.Path(tempfile.mkdtemp(prefix="nahuali-trust-benchmark-"))
    supported_home = root / "supported"
    command(binary, supported_home, "remember", "Lena owns release notes", "--json")
    command(
        binary,
        supported_home,
        "claim",
        "Lena",
        "owns",
        "release notes",
        "--source-last",
        "--json",
    )
    supported = command(
        binary,
        supported_home,
        "recall",
        "Lena release notes",
        "--authority",
        "--json",
    )
    supported_claim = next(item for item in supported["results"] if item["kind"] == "claim")

    unsupported_home = root / "unsupported"
    command(
        binary,
        unsupported_home,
        "claim",
        "Mateo",
        "owns",
        "deployment keys",
        "--json",
    )
    unsupported = command(
        binary,
        unsupported_home,
        "recall",
        "Mateo deployment keys",
        "--authority",
        "--json",
    )
    unsupported_claim = next(item for item in unsupported["results"] if item["kind"] == "claim")

    now_ms = int(time.time() * 1000)
    contradiction_home = root / "contradiction"
    contradiction_document = write_interchange(
        root / "contradiction-input",
        [
            {"ref": "review", "content": "Launch is Tuesday", "timestamp_ms": now_ms - 1_000},
            {"ref": "incident", "content": "Launch is Friday", "timestamp_ms": now_ms - 1_000},
        ],
        [
            {
                "subject": "Launch",
                "predicate": "day",
                "object": "Tuesday",
                "source_episode_ref": "review",
                "timestamp_ms": now_ms - 1_000,
            },
            {
                "subject": "Launch",
                "predicate": "day",
                "object": "Friday",
                "source_episode_ref": "incident",
                "timestamp_ms": now_ms - 1_000,
            },
        ],
    )
    command(binary, contradiction_home, "import", str(contradiction_document), "--json")
    before = command(binary, contradiction_home, "data", "--json")
    contradiction = command(binary, contradiction_home, "self-inspect", "--json")
    after = command(binary, contradiction_home, "data", "--json")

    stale_home = root / "stale"
    old_ms = now_ms - (100 * 24 * 60 * 60 * 1000)
    stale_document = write_interchange(
        root / "stale-input",
        [{"ref": "old", "content": "The deploy region is eu-west", "timestamp_ms": old_ms}],
        [
            {
                "subject": "Deploy",
                "predicate": "region",
                "object": "eu-west",
                "source_episode_ref": "old",
                "timestamp_ms": old_ms + 1,
            }
        ],
    )
    command(binary, stale_home, "import", str(stale_document), "--json")
    stale = command(binary, stale_home, "self-inspect", "--json")

    demo = command(binary, root / "demo", "demo", "--json")
    version = command(binary, root / "version", "--version", json_output=False).strip()
    supported_mode = supported_claim["trust"]["mode"]
    supported_verdict = normalize_verdict(supported_mode)
    supported_evidence = [supported_claim["evidence_id"]] if supported_claim.get("evidence_id") else []
    unsupported_mode = unsupported_claim["trust"]["mode"]
    unsupported_detected = not unsupported_claim["trust"]["can_trust"]
    unsupported_verdict = normalize_verdict(
        unsupported_mode, unsupported_claim["trust"]["can_trust"]
    )
    contradiction_mode = contradiction["authority"]["mode"]
    contradiction_verdict = normalize_verdict(contradiction_mode)
    contradiction_detected = contradiction["summary"]["contradiction_count"] > 0
    stale_mode = stale["authority"]["mode"]
    stale_verdict = normalize_verdict(stale_mode)
    stale_detected = stale["summary"]["stale_memory_count"] > 0
    inspection_detected = len(contradiction["findings"]) > 0
    inspection_mutated = before != after
    rewrite_detected = demo["history_integrity"]["in_place_rewrite_detected"]
    rechain_detected = demo["history_integrity"]["checkpoint_rejects_rechain"]
    external_checkpoint = demo["history_integrity"]["external_checkpoint"]

    return {
        "benchmarkVersion": "1.0.0",
        "system": {"name": "Nahuali", "version": version},
        "commit": f"sha256:{binary_sha256}",
        "artifact": {
            "name": binary_path.name,
            "sha256": binary_sha256,
            "sourceRevision": source_revision,
        },
        "runner": {
            "relationship": "first-party",
            "adapter": "benchmarks/agent-memory-trust/adapters/nahuali.py",
        },
        "environment": {
            "services": ["embedded SurrealKV"],
            "models": [],
            "operatorActions": [],
        },
        "cases": [
            observed_case(
                "evidence-traceability",
                bool(supported_evidence) and supported_verdict == "trusted",
                verdict=supported_mode,
                normalizedVerdict=supported_verdict,
                evidenceIds=supported_evidence,
                detected=True,
                mutated=False,
            ),
            observed_case(
                "unsupported-memory-abstention",
                unsupported_detected and unsupported_verdict == "refused",
                verdict=unsupported_mode,
                normalizedVerdict=unsupported_verdict,
                evidenceIds=[],
                detected=unsupported_detected,
                mutated=False,
            ),
            observed_case(
                "contradiction-detection",
                contradiction_detected and contradiction_verdict == "refused",
                verdict=contradiction_mode,
                normalizedVerdict=contradiction_verdict,
                evidenceIds=[],
                detected=contradiction_detected,
                mutated=False,
            ),
            observed_case(
                "staleness-signaling",
                stale_detected and stale_verdict in {"qualified", "refused"},
                verdict=stale_mode,
                normalizedVerdict=stale_verdict,
                evidenceIds=[],
                detected=stale_detected,
                mutated=False,
            ),
            observed_case(
                "non-mutating-inspection",
                inspection_detected and not inspection_mutated,
                evidenceIds=[],
                detected=inspection_detected,
                mutated=inspection_mutated,
            ),
            observed_case(
                "in-place-tamper-detection",
                rewrite_detected,
                evidenceIds=[],
                detected=rewrite_detected,
                mutated=False,
            ),
            observed_case(
                "full-rechain-detection",
                rechain_detected and external_checkpoint,
                evidenceIds=[],
                detected=rechain_detected,
                externalCheckpoint=external_checkpoint,
                mutated=False,
            ),
        ],
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", default="nahuali")
    parser.add_argument("--source-revision")
    parser.add_argument("--output", type=pathlib.Path)
    arguments = parser.parse_args()
    report = run(arguments.binary, arguments.source_revision)
    encoded = json.dumps(report, indent=2) + "\n"
    if arguments.output:
        arguments.output.parent.mkdir(parents=True, exist_ok=True)
        arguments.output.write_text(encoded, encoding="utf-8")
    else:
        print(encoded, end="")


if __name__ == "__main__":
    main()
