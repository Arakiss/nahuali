#!/usr/bin/env python3
"""Evaluate Nahuali retrieval quality through the public CLI."""

import argparse
import hashlib
import json
import math
import os
import pathlib
import shutil
import statistics
import subprocess
import tempfile
import time
from typing import Any, Optional


ROOT = pathlib.Path(__file__).resolve().parents[3]
DEFAULT_CASES = ROOT / "benchmarks/agent-memory-retrieval/cases.json"


def sha256_file(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def resolve_binary(binary: str) -> pathlib.Path:
    candidate = pathlib.Path(binary).expanduser()
    if candidate.is_file():
        return candidate.resolve()
    resolved = shutil.which(binary)
    if resolved is None:
        raise SystemExit(f"benchmark binary not found: {binary}")
    return pathlib.Path(resolved).resolve()


def command(
    binary: pathlib.Path,
    home: pathlib.Path,
    *args: str,
    environment: Optional[dict[str, str]] = None,
) -> Any:
    env = os.environ.copy()
    env["NAHUALI_HOME"] = str(home)
    env["NO_COLOR"] = "1"
    env.pop("NAHUALI_DB_URL", None)
    if environment:
        env.update(environment)
    completed = subprocess.run(
        [str(binary), *args],
        check=True,
        capture_output=True,
        text=True,
        env=env,
    )
    return json.loads(completed.stdout)


def percentile(values: list[float], percentile_value: float) -> float:
    ordered = sorted(values)
    index = max(0, math.ceil(percentile_value * len(ordered)) - 1)
    return ordered[index]


def ranking_metrics(ranked: list[str], relevant: list[str], k_values: list[int], max_k: int) -> dict:
    relevant_set = set(relevant)
    if not relevant_set:
        raise ValueError("every query must declare at least one relevant memory")
    metrics = {
        f"recallAt{k}": len(relevant_set.intersection(ranked[:k])) / len(relevant_set)
        for k in k_values
    }
    reciprocal_rank = 0.0
    for index, memory_id in enumerate(ranked[:max_k], start=1):
        if memory_id in relevant_set:
            reciprocal_rank = 1.0 / index
            break
    dcg = sum(
        1.0 / math.log2(index + 1)
        for index, memory_id in enumerate(ranked[:max_k], start=1)
        if memory_id in relevant_set
    )
    ideal_hits = min(len(relevant_set), max_k)
    ideal_dcg = sum(1.0 / math.log2(index + 1) for index in range(1, ideal_hits + 1))
    metrics["reciprocalRank"] = reciprocal_rank
    metrics["ndcgAt10"] = dcg / ideal_dcg if ideal_dcg else 0.0
    return metrics


def aggregate_metrics(query_reports: list[dict], k_values: list[int]) -> dict:
    aggregate = {
        f"recallAt{k}": statistics.fmean(
            report["metrics"][f"recallAt{k}"] for report in query_reports
        )
        for k in k_values
    }
    aggregate["mrr"] = statistics.fmean(
        report["metrics"]["reciprocalRank"] for report in query_reports
    )
    aggregate["ndcgAt10"] = statistics.fmean(
        report["metrics"]["ndcgAt10"] for report in query_reports
    )
    latencies = [latency for report in query_reports for latency in report["latencyMs"]]
    aggregate["latencyMs"] = {
        "sampleCount": len(latencies),
        "median": statistics.median(latencies),
        "p95": percentile(latencies, 0.95),
        "maximum": max(latencies),
    }
    return aggregate


def import_corpus(binary: pathlib.Path, home: pathlib.Path, cases: dict, root: pathlib.Path) -> dict[str, str]:
    interchange = root / "corpus.interchange.json"
    interchange.write_text(
        json.dumps(
            {
                "version": 1,
                "episodes": [
                    {
                        "ref": memory["id"],
                        "content": memory["content"],
                        "tags": memory.get("tags", []),
                        "mentions": memory.get("mentions", []),
                    }
                    for memory in cases["memories"]
                ],
            },
            indent=2,
        ),
        encoding="utf-8",
    )
    command(binary, home, "import", str(interchange), "--json")
    projection = command(binary, home, "data", "--json")
    by_content = {episode["content"]: episode["id"] for episode in projection["episodes"]}
    return {memory["id"]: by_content[memory["content"]] for memory in cases["memories"]}


def evaluate_mode(
    name: str,
    binary: pathlib.Path,
    home: pathlib.Path,
    cases: dict,
    runtime_to_corpus: dict[str, str],
    environment: Optional[dict[str, str]] = None,
) -> dict:
    semantic = name != "lexical"
    if semantic:
        rebuild = command(binary, home, "semantic-rebuild", "--json", environment=environment)
        status = command(binary, home, "semantic-status", "--json", environment=environment)
        if not status["status"]["is_current"]:
            raise RuntimeError(f"{name} semantic index is not current after rebuild")
        embedding = rebuild["report"]["embedding"]
    else:
        embedding = None

    query_reports = []
    for query_case in cases["queries"]:
        args = [
            "recall",
            query_case["query"],
            "--kind",
            "episode",
            "--limit",
            str(cases["maxK"]),
            "--json",
        ]
        if semantic:
            args.insert(-1, "--semantic")
        for _ in range(cases["warmupRuns"]):
            command(binary, home, *args, environment=environment)

        rankings = []
        latencies = []
        for _ in range(cases["measuredRuns"]):
            started = time.perf_counter_ns()
            response = command(binary, home, *args, environment=environment)
            latencies.append((time.perf_counter_ns() - started) / 1_000_000)
            results = response["results"] if semantic else response
            rankings.append(
                [
                    runtime_to_corpus[result["id"]]
                    for result in results
                    if result["id"] in runtime_to_corpus
                ]
            )
        if any(ranking != rankings[0] for ranking in rankings[1:]):
            raise RuntimeError(f"non-deterministic ranking for query {query_case['id']} in {name}")
        query_reports.append(
            {
                "id": query_case["id"],
                "query": query_case["query"],
                "relevant": query_case["relevant"],
                "resultIds": rankings[0],
                "latencyMs": latencies,
                "metrics": ranking_metrics(
                    rankings[0], query_case["relevant"], cases["kValues"], cases["maxK"]
                ),
            }
        )

    return {
        "status": "complete",
        "embedding": embedding,
        "queryCount": len(query_reports),
        "metrics": aggregate_metrics(query_reports, cases["kValues"]),
        "queries": query_reports,
    }


def run(binary: str, cases_path: pathlib.Path, source_revision: str) -> dict:
    if not source_revision or len(source_revision) != 40:
        raise SystemExit("--source-revision must be the exact 40-character source commit")
    binary_path = resolve_binary(binary)
    cases = json.loads(cases_path.read_text(encoding="utf-8"))
    root = pathlib.Path(tempfile.mkdtemp(prefix="nahuali-retrieval-benchmark-"))
    home = root / "home"
    collection = f"nahuali_retrieval_eval_{os.getpid()}"
    deterministic_env = {
        "NAHUALI_QDRANT_COLLECTION": collection,
        "NAHUALI_EMBEDDING_PROVIDER": "deterministic",
    }
    try:
        corpus_to_runtime = import_corpus(binary_path, home, cases, root)
        runtime_to_corpus = {runtime: corpus for corpus, runtime in corpus_to_runtime.items()}
        modes = {
            "lexical": evaluate_mode(
                "lexical", binary_path, home, cases, runtime_to_corpus
            ),
            "deterministicHybrid": evaluate_mode(
                "deterministicHybrid",
                binary_path,
                home,
                cases,
                runtime_to_corpus,
                deterministic_env,
            ),
        }
        local_model_path = os.environ.get("NAHUALI_LOCAL_EMBEDDING_MODEL_PATH", "").strip()
        if local_model_path:
            modes["localModelHybrid"] = evaluate_mode(
                "localModelHybrid",
                binary_path,
                home,
                cases,
                runtime_to_corpus,
                {
                    "NAHUALI_QDRANT_COLLECTION": collection,
                    "NAHUALI_EMBEDDING_PROVIDER": "local-model",
                    "NAHUALI_LOCAL_EMBEDDING_MODEL_PATH": local_model_path,
                },
            )
        else:
            modes["localModelHybrid"] = {
                "status": "not_configured",
                "reason": "NAHUALI_LOCAL_EMBEDDING_MODEL_PATH is not set; no model result was measured.",
            }
        version = subprocess.run(
            [str(binary_path), "--version"],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
        return {
            "resultVersion": "1.0.0",
            "benchmarkVersion": cases["benchmarkVersion"],
            "corpus": {
                "path": str(cases_path.relative_to(ROOT)),
                "sha256": sha256_file(cases_path),
                "memoryCount": len(cases["memories"]),
                "queryCount": len(cases["queries"]),
            },
            "system": {"name": "Nahuali", "version": version},
            "artifact": {
                "name": binary_path.name,
                "sha256": sha256_file(binary_path),
                "sourceRevision": source_revision,
            },
            "runner": {
                "relationship": "first-party",
                "adapter": "benchmarks/agent-memory-retrieval/adapters/nahuali.py",
            },
            "configuration": {
                "maxK": cases["maxK"],
                "kValues": cases["kValues"],
                "warmupRuns": cases["warmupRuns"],
                "measuredRuns": cases["measuredRuns"],
            },
            "modes": modes,
        }
    finally:
        shutil.rmtree(root, ignore_errors=True)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", default="nahuali")
    parser.add_argument("--cases", type=pathlib.Path, default=DEFAULT_CASES)
    parser.add_argument("--source-revision", required=True)
    parser.add_argument("--output", type=pathlib.Path)
    arguments = parser.parse_args()
    report = run(arguments.binary, arguments.cases.resolve(), arguments.source_revision)
    rendered = json.dumps(report, indent=2) + "\n"
    if arguments.output:
        arguments.output.parent.mkdir(parents=True, exist_ok=True)
        arguments.output.write_text(rendered, encoding="utf-8")
    else:
        print(rendered, end="")


if __name__ == "__main__":
    main()
