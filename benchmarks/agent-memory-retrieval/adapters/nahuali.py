#!/usr/bin/env python3
"""Evaluate Nahuali retrieval quality through the public CLI."""

import argparse
import hashlib
import json
import math
import os
import pathlib
import re
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from typing import Any, Optional
from urllib import error as urlerror
from urllib import parse as urlparse
from urllib import request as urlrequest


ROOT = pathlib.Path(__file__).resolve().parents[3]
DEFAULT_CASES = ROOT / "benchmarks/agent-memory-retrieval/cases.json"
LOWER_SHA256 = re.compile(r"^[0-9a-f]{64}$")
LOWER_SOURCE_REVISION = re.compile(r"^[0-9a-f]{40}$")


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


def validate_source_revision(source_revision: str) -> str:
    if not LOWER_SOURCE_REVISION.fullmatch(source_revision or ""):
        raise SystemExit("--source-revision must be an exact lowercase 40-character commit SHA")
    return source_revision


def release_artifact_metadata(
    release_tag: Optional[str],
    release_asset: Optional[str],
    target: Optional[str],
    archive_sha256: Optional[str],
) -> dict:
    values = (release_tag, release_asset, target, archive_sha256)
    if not any(values):
        return {"kind": "source-build"}
    if not all(values):
        raise SystemExit(
            "published release identity requires --release-tag, --release-asset, "
            "--target, and --archive-sha256"
        )
    if not LOWER_SHA256.fullmatch(archive_sha256 or ""):
        raise SystemExit("--archive-sha256 must be an exact lowercase 64-character SHA-256")
    return {
        "kind": "published-release",
        "releaseTag": release_tag,
        "releaseAsset": release_asset,
        "target": target,
        "archiveSha256": archive_sha256,
    }


def delete_qdrant_collection_if_exists(collection_name: str) -> None:
    base_url = os.environ.get("NAHUALI_QDRANT_URL", "http://localhost:16333").rstrip("/")
    endpoint = f"{base_url}/collections/{urlparse.quote(collection_name, safe='')}"
    request = urlrequest.Request(endpoint, method="DELETE")
    qdrant_token = os.environ.get("NAHUALI_QDRANT_API_KEY", "").strip()
    if qdrant_token:
        request.add_header("api-key", qdrant_token)
    try:
        with urlrequest.urlopen(request, timeout=10) as response:
            payload = json.loads(response.read().decode("utf-8"))
    except urlerror.HTTPError as error:
        if error.code == 404:
            return
        raise RuntimeError(
            f"failed to delete benchmark Qdrant collection {collection_name}: HTTP {error.code}"
        ) from error
    except urlerror.URLError as error:
        raise RuntimeError(
            f"failed to delete benchmark Qdrant collection {collection_name}: {error.reason}"
        ) from error
    if payload.get("status") != "ok":
        raise RuntimeError(
            f"failed to delete benchmark Qdrant collection {collection_name}: unexpected response"
        )


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


def run(
    binary: str,
    cases_path: pathlib.Path,
    source_revision: str,
    artifact_metadata: Optional[dict] = None,
) -> dict:
    source_revision = validate_source_revision(source_revision)
    binary_path = resolve_binary(binary)
    cases = json.loads(cases_path.read_text(encoding="utf-8"))
    root = pathlib.Path(tempfile.mkdtemp(prefix="nahuali-retrieval-benchmark-"))
    home = root / "home"
    collection = f"nahuali_retrieval_eval_{os.getpid()}"
    cleanup_collections = {collection, f"{collection}__memory"}
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
                **(artifact_metadata or {"kind": "source-build"}),
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
        active_error = sys.exc_info()[0] is not None
        cleanup_errors = []
        for collection_name in cleanup_collections:
            try:
                delete_qdrant_collection_if_exists(collection_name)
            except RuntimeError as error:
                cleanup_errors.append(str(error))
        shutil.rmtree(root, ignore_errors=True)
        if cleanup_errors and not active_error:
            raise RuntimeError("; ".join(cleanup_errors))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", default="nahuali")
    parser.add_argument("--cases", type=pathlib.Path, default=DEFAULT_CASES)
    parser.add_argument("--source-revision", required=True)
    parser.add_argument("--release-tag")
    parser.add_argument("--release-asset")
    parser.add_argument("--target")
    parser.add_argument("--archive-sha256")
    parser.add_argument("--output", type=pathlib.Path)
    arguments = parser.parse_args()
    artifact_metadata = release_artifact_metadata(
        arguments.release_tag,
        arguments.release_asset,
        arguments.target,
        arguments.archive_sha256,
    )
    report = run(
        arguments.binary,
        arguments.cases.resolve(),
        arguments.source_revision,
        artifact_metadata,
    )
    rendered = json.dumps(report, indent=2) + "\n"
    if arguments.output:
        arguments.output.parent.mkdir(parents=True, exist_ok=True)
        arguments.output.write_text(rendered, encoding="utf-8")
    else:
        print(rendered, end="")


if __name__ == "__main__":
    main()
