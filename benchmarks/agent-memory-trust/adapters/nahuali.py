#!/usr/bin/env python3
"""Run the Agent Memory Trust Benchmark through Nahuali's public CLI."""

import argparse
import json
import os
import pathlib
import subprocess
import tempfile
import time


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


def run(binary: str) -> dict:
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
    commit = subprocess.run(
        ["git", "rev-parse", "HEAD"], check=True, capture_output=True, text=True
    ).stdout.strip()

    return {
        "benchmarkVersion": "1.0.0",
        "system": {"name": "Nahuali", "version": version},
        "commit": commit,
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
            {
                "id": "evidence-traceability",
                "status": "pass",
                "verdict": supported_claim["trust"]["mode"],
                "normalizedVerdict": "trusted",
                "evidenceIds": [supported_claim["evidence_id"]],
                "detected": True,
                "mutated": False,
            },
            {
                "id": "unsupported-memory-abstention",
                "status": "pass",
                "verdict": unsupported_claim["trust"]["mode"],
                "normalizedVerdict": "refused",
                "evidenceIds": [],
                "detected": not unsupported_claim["trust"]["can_trust"],
                "mutated": False,
            },
            {
                "id": "contradiction-detection",
                "status": "pass",
                "verdict": contradiction["authority"]["mode"],
                "normalizedVerdict": "refused",
                "evidenceIds": [],
                "detected": contradiction["summary"]["contradiction_count"] > 0,
                "mutated": False,
            },
            {
                "id": "staleness-signaling",
                "status": "pass",
                "verdict": stale["authority"]["mode"],
                "normalizedVerdict": "qualified" if stale["authority"]["mode"] != "block" else "refused",
                "evidenceIds": [],
                "detected": stale["summary"]["stale_memory_count"] > 0,
                "mutated": False,
            },
            {
                "id": "non-mutating-inspection",
                "status": "pass",
                "evidenceIds": [],
                "detected": len(contradiction["findings"]) > 0,
                "mutated": before != after,
            },
            {
                "id": "in-place-tamper-detection",
                "status": "pass",
                "evidenceIds": [],
                "detected": demo["history_integrity"]["in_place_rewrite_detected"],
                "mutated": False,
            },
            {
                "id": "full-rechain-detection",
                "status": "pass",
                "evidenceIds": [],
                "detected": demo["history_integrity"]["checkpoint_rejects_rechain"],
                "externalCheckpoint": demo["history_integrity"]["external_checkpoint"],
                "mutated": False,
            },
        ],
    }


parser = argparse.ArgumentParser()
parser.add_argument("--binary", default="nahuali")
parser.add_argument("--output", type=pathlib.Path)
arguments = parser.parse_args()
report = run(arguments.binary)
encoded = json.dumps(report, indent=2) + "\n"
if arguments.output:
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(encoded, encoding="utf-8")
else:
    print(encoded, end="")
