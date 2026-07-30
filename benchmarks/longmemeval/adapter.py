#!/usr/bin/env python3
"""Run a first-party, retrieval-only LongMemEval v1 evaluation through Nahuali.

The adapter intentionally uses only the Python standard library and the public
``nahuali`` CLI. It does not run a reader model and never reports a QA score.
"""

from __future__ import annotations

import argparse
from collections import Counter
import datetime as dt
import hashlib
import ipaddress
import json
import math
import os
import pathlib
import platform
import re
import shutil
import stat
import statistics
import subprocess
import sys
import tempfile
import time
from typing import Any, Dict, Iterator, List, Optional, Sequence, Tuple
from urllib import error as urlerror
from urllib import parse as urlparse
from urllib import request as urlrequest


K_VALUES = (1, 3, 5, 10, 30, 50)
RESULT_VERSION = "1.0.0"
OFFICIAL_DATASET_REVISION = "98d7416c24c778c2fee6e6f3006e7a073259d48f"
OFFICIAL_EVALUATOR_REVISION = "9e0b455f4ef0e2ab8f2e582289761153549043fc"
OFFICIAL_DATASET_FILENAME = "longmemeval_s_cleaned.json"
OFFICIAL_DATASET_SHA256 = "d6f21ea9d60a0d56f34a05b609c79c88a451d2ae03597821ea3d5a9678c3a442"
OFFICIAL_DATASET_SIZE = 277_383_467
OFFICIAL_QUESTION_COUNT = 500
OFFICIAL_DATASET_URL = (
    "https://huggingface.co/datasets/xiaowu0162/longmemeval-cleaned/resolve/"
    "{revision}/longmemeval_s_cleaned.json"
)
LOWER_SHA256 = re.compile(r"^[0-9a-f]{64}$")
LOWER_SOURCE_REVISION = re.compile(r"^[0-9a-f]{40}$")
MODE_NAMES = ("lexical", "deterministic-hybrid", "local-model-hybrid")
REPOSITORY_ROOT = pathlib.Path(__file__).resolve().parents[2]


class AdapterError(RuntimeError):
    """A bounded, user-facing adapter failure."""


def runtime_environment() -> Dict[str, Any]:
    """Return useful benchmark context without recording host or user identity."""

    environment: Dict[str, Any] = {
        "operating_system": platform.system(),
        "operating_system_release": platform.release(),
        "architecture": platform.machine(),
        "python_version": platform.python_version(),
        "logical_cpu_count": os.cpu_count() or 1,
    }
    processor = platform.processor().strip()
    if processor:
        environment["processor"] = processor
    return environment


def source_snapshot() -> Dict[str, Any]:
    """Capture revision and a non-disclosing fingerprint of the current worktree."""

    adapter_sha256 = sha256_file(pathlib.Path(__file__).resolve())
    try:
        revision = subprocess.run(
            ["git", "-C", str(REPOSITORY_ROOT), "rev-parse", "HEAD"],
            check=True,
            capture_output=True,
            text=True,
            timeout=10,
        ).stdout.strip()
        status = subprocess.run(
            ["git", "-C", str(REPOSITORY_ROOT), "status", "--porcelain=v1", "-uall"],
            check=True,
            capture_output=True,
            text=True,
            timeout=10,
        ).stdout
        tracked_diff = subprocess.run(
            ["git", "-C", str(REPOSITORY_ROOT), "diff", "--binary", "HEAD", "--", "."],
            check=True,
            capture_output=True,
            timeout=30,
        ).stdout
        untracked_output = subprocess.run(
            [
                "git",
                "-C",
                str(REPOSITORY_ROOT),
                "ls-files",
                "--others",
                "--exclude-standard",
                "-z",
            ],
            check=True,
            capture_output=True,
            timeout=10,
        ).stdout
    except (FileNotFoundError, subprocess.SubprocessError):
        return {
            "head_revision": None,
            "worktree_state": "unavailable",
            "worktree_fingerprint": None,
            "adapter_sha256": adapter_sha256,
        }
    fingerprint = hashlib.sha256()
    fingerprint.update(tracked_diff)
    for raw_relative in sorted(item for item in untracked_output.split(b"\0") if item):
        relative = os.fsdecode(raw_relative)
        path = REPOSITORY_ROOT / relative
        fingerprint.update(len(raw_relative).to_bytes(8, "big"))
        fingerprint.update(raw_relative)
        if path.is_symlink():
            target = os.readlink(path).encode("utf-8")
            fingerprint.update(target)
        elif path.is_file():
            with path.open("rb") as handle:
                for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                    fingerprint.update(chunk)
    return {
        "head_revision": revision,
        "worktree_state": "dirty" if status.strip() else "clean",
        "worktree_fingerprint": fingerprint.hexdigest(),
        "adapter_sha256": adapter_sha256,
    }


def source_worktree_state() -> str:
    """Return the current high-level worktree state."""

    return str(source_snapshot()["worktree_state"])


def sha256_file(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def paths_refer_to_same_file(left: pathlib.Path, right: pathlib.Path) -> bool:
    left_resolved = left.expanduser().resolve()
    right_resolved = right.expanduser().resolve()
    if left_resolved == right_resolved:
        return True
    try:
        return left_resolved.samefile(right_resolved)
    except (FileNotFoundError, OSError):
        return False


def run_dataset_identity(
    path: pathlib.Path, revision: str, expected_sha256: Optional[str]
) -> Dict[str, Any]:
    observed_sha256 = sha256_file(path)
    observed_size = path.stat().st_size
    if revision == OFFICIAL_DATASET_REVISION:
        if observed_sha256 != OFFICIAL_DATASET_SHA256 or observed_size != OFFICIAL_DATASET_SIZE:
            raise AdapterError(
                "the pinned official dataset revision requires its published SHA-256 and size"
            )
        if expected_sha256 is not None and expected_sha256 != OFFICIAL_DATASET_SHA256:
            raise AdapterError(
                "--expected-dataset-sha256 conflicts with the pinned official dataset digest"
            )
        return {
            "sha256": observed_sha256,
            "size": observed_size,
            "identity_policy": "pinned_official_revision_sha256_and_size",
            "expected_sha256": OFFICIAL_DATASET_SHA256,
        }

    if expected_sha256 is None or not LOWER_SHA256.fullmatch(expected_sha256):
        raise AdapterError(
            "non-pinned dataset revisions require an exact --expected-dataset-sha256"
        )
    if observed_sha256 != expected_sha256:
        raise AdapterError(
            f"dataset SHA-256 mismatch: expected {expected_sha256}, got {observed_sha256}"
        )
    return {
        "sha256": observed_sha256,
        "size": observed_size,
        "identity_policy": "operator_supplied_sha256_for_non_pinned_revision",
        "expected_sha256": expected_sha256,
    }


def local_model_artifact_manifest(directory: pathlib.Path) -> Dict[str, Any]:
    root = directory.expanduser().resolve()
    if not root.is_dir():
        raise AdapterError("NAHUALI_LOCAL_EMBEDDING_MODEL_PATH must name a model directory")
    required = ("config.json", "model.safetensors", "tokenizer.json")
    missing = [name for name in required if not (root / name).is_file()]
    if missing:
        raise AdapterError("local model directory is missing: " + ", ".join(missing))

    files = sorted(path for path in root.rglob("*") if path.is_file())
    if not files:
        raise AdapterError("local model directory contains no files")
    digest = hashlib.sha256()
    total_bytes = 0
    for path in files:
        if path.is_symlink():
            raise AdapterError("local model artifact sets must not contain symbolic links")
        relative = path.relative_to(root).as_posix().encode("utf-8")
        digest.update(len(relative).to_bytes(8, "big"))
        digest.update(relative)
        size = path.stat().st_size
        digest.update(size.to_bytes(8, "big"))
        total_bytes += size
        with path.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
    return {
        "artifact_set_sha256": digest.hexdigest(),
        "file_count": len(files),
        "total_bytes": total_bytes,
        "required_files": list(required),
        "path_recorded": False,
        "required_cli_feature": "local-embeddings",
    }


def qdrant_endpoint_class(value: str) -> str:
    parsed = urlparse.urlparse(value)
    if parsed.scheme not in {"http", "https"} or parsed.hostname is None:
        raise AdapterError("NAHUALI_QDRANT_URL must be an HTTP(S) URL with a host")
    host = parsed.hostname.rstrip(".").lower()
    if host == "localhost":
        return "loopback"
    try:
        if ipaddress.ip_address(host).is_loopback:
            return "loopback"
    except ValueError:
        pass
    return "remote"


def canonical_json_bytes(value: Any) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode(
        "utf-8"
    )


def atomic_write_text(path: pathlib.Path, content: str) -> None:
    path = path.expanduser().resolve()
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    temporary.write_text(content, encoding="utf-8")
    os.chmod(temporary, 0o600)
    os.replace(temporary, path)


def iter_json_array(path: pathlib.Path, chunk_size: int = 1024 * 1024) -> Iterator[Any]:
    """Stream a top-level JSON array without loading the 277 MB corpus at once."""

    decoder = json.JSONDecoder()
    buffer = ""
    position = 0
    started = False
    finished = False

    with path.open("r", encoding="utf-8") as handle:
        while not finished:
            if position >= len(buffer):
                chunk = handle.read(chunk_size)
                if not chunk:
                    raise AdapterError(f"{path} ended before the top-level JSON array closed")
                buffer = chunk
                position = 0

            while True:
                while position < len(buffer) and buffer[position].isspace():
                    position += 1
                if position >= len(buffer):
                    break

                if not started:
                    if buffer[position] != "[":
                        raise AdapterError(f"{path} must contain a top-level JSON array")
                    started = True
                    position += 1
                    continue

                if buffer[position] == ",":
                    position += 1
                    continue
                if buffer[position] == "]":
                    finished = True
                    position += 1
                    break

                try:
                    value, end = decoder.raw_decode(buffer, position)
                except json.JSONDecodeError:
                    remainder = buffer[position:]
                    chunk = handle.read(chunk_size)
                    if not chunk:
                        raise AdapterError(f"{path} contains invalid or truncated JSON")
                    buffer = remainder + chunk
                    position = 0
                    continue
                position = end
                yield value

            if not finished and position >= len(buffer):
                buffer = ""
                position = 0

        trailing = buffer[position:] + handle.read()
        if trailing.strip():
            raise AdapterError(f"{path} contains data after the top-level JSON array")


def parse_longmemeval_date(value: str) -> int:
    """Parse the timezone-free official date as UTC while retaining the raw date elsewhere."""

    matched = re.fullmatch(
        r"(?P<date>\d{4}/\d{2}/\d{2}) \([A-Za-z]{3}\) (?P<time>\d{2}:\d{2})",
        value,
    )
    if matched is None:
        raise AdapterError(f"unsupported LongMemEval date {value!r}")
    try:
        parsed = dt.datetime.strptime(
            f"{matched.group('date')} {matched.group('time')}", "%Y/%m/%d %H:%M"
        )
    except ValueError as error:
        raise AdapterError(f"unsupported LongMemEval date {value!r}") from error
    return int(parsed.replace(tzinfo=dt.timezone.utc).timestamp() * 1000)


def normalize_scope_key(name: str) -> str:
    normalized = "".join(
        character.lower() if character.isascii() and character.isalnum() else "_"
        for character in name
    )
    normalized = re.sub(r"_+", "_", normalized).strip("_")
    return f"custom:{normalized}"


def question_scope(question_id: str) -> Tuple[str, Dict[str, str]]:
    name = f"LongMemEval {question_id}"
    return f"custom:{name}", {"kind": "custom", "name": name, "key": normalize_scope_key(name)}


def question_database(dataset_sha256: str, question_id: str, isolation_nonce: str = "") -> str:
    suffix = hashlib.sha256(
        f"{dataset_sha256}:{question_id}:{isolation_nonce}".encode("utf-8")
    ).hexdigest()[:20]
    return f"lme_{suffix}"


def is_json_value(value: Any) -> bool:
    """Return whether ``value`` can be represented as strict JSON."""

    if value is None or isinstance(value, (str, bool, int)):
        return True
    if isinstance(value, float):
        return math.isfinite(value)
    if isinstance(value, list):
        return all(is_json_value(item) for item in value)
    if isinstance(value, dict):
        return all(isinstance(key, str) and is_json_value(item) for key, item in value.items())
    return False


def json_type_name(value: Any) -> str:
    if value is None:
        return "null"
    if isinstance(value, bool):
        return "boolean"
    if isinstance(value, int):
        return "integer"
    if isinstance(value, float):
        return "number"
    if isinstance(value, str):
        return "string"
    if isinstance(value, list):
        return "array"
    if isinstance(value, dict):
        return "object"
    return type(value).__name__


def validate_question(entry: Any) -> Dict[str, Any]:
    if not isinstance(entry, dict):
        raise AdapterError("every LongMemEval instance must be a JSON object")
    required = (
        "question_id",
        "question_type",
        "question",
        "answer",
        "question_date",
        "answer_session_ids",
        "haystack_dates",
        "haystack_session_ids",
        "haystack_sessions",
    )
    missing = [field for field in required if field not in entry]
    if missing:
        raise AdapterError(f"LongMemEval instance is missing fields: {', '.join(missing)}")

    for field in ("question_id", "question_type", "question", "question_date"):
        if not isinstance(entry[field], str) or not entry[field].strip():
            raise AdapterError(f"LongMemEval field {field} must be a non-empty string")
    if not is_json_value(entry["answer"]):
        raise AdapterError("LongMemEval field answer must be a valid JSON value")
    if not isinstance(entry["answer_session_ids"], list) or not all(
        isinstance(value, str) and value for value in entry["answer_session_ids"]
    ):
        raise AdapterError("answer_session_ids must be a list of strings")

    session_ids = entry["haystack_session_ids"]
    dates = entry["haystack_dates"]
    sessions = entry["haystack_sessions"]
    if not all(isinstance(value, list) for value in (session_ids, dates, sessions)):
        raise AdapterError("haystack fields must be lists")
    if not (len(session_ids) == len(dates) == len(sessions)):
        raise AdapterError("haystack session ids, dates, and sessions must have equal lengths")
    if not session_ids:
        raise AdapterError("haystack_session_ids must be non-empty")

    for session_id, date, turns in zip(session_ids, dates, sessions):
        if not isinstance(session_id, str) or not session_id:
            raise AdapterError("every haystack session id must be a non-empty string")
        if not isinstance(date, str):
            raise AdapterError(f"session {session_id} has a non-string date")
        parse_longmemeval_date(date)
        if not isinstance(turns, list) or not turns:
            raise AdapterError(f"session {session_id} must contain at least one turn")
        for turn in turns:
            if not isinstance(turn, dict):
                raise AdapterError(f"session {session_id} contains a non-object turn")
            if turn.get("role") not in {"user", "assistant"}:
                raise AdapterError(f"session {session_id} contains an unsupported role")
            if not isinstance(turn.get("content"), str):
                raise AdapterError(f"session {session_id} contains a non-string turn")

    missing_targets = sorted(set(entry["answer_session_ids"]) - set(session_ids))
    if missing_targets and "_abs" not in entry["question_id"]:
        raise AdapterError(
            f"question {entry['question_id']} has answer sessions absent from its haystack: "
            + ", ".join(missing_targets)
        )
    return entry


def build_interchange(entry: Dict[str, Any]) -> Tuple[Dict[str, Any], Dict[str, Any]]:
    scope_text, scope = question_scope(entry["question_id"])
    sources: List[Dict[str, Any]] = []
    episodes: List[Dict[str, Any]] = []
    session_totals = Counter(entry["haystack_session_ids"])
    session_occurrences: Counter[str] = Counter()
    raw_turn_count = 0
    skipped_empty_turn_count = 0

    for session_index, (session_id, date, turns) in enumerate(
        zip(
            entry["haystack_session_ids"],
            entry["haystack_dates"],
            entry["haystack_sessions"],
        )
    ):
        source_ref = f"session-{session_index + 1}"
        session_occurrences[session_id] += 1
        session_occurrence = session_occurrences[session_id]
        session_bytes = canonical_json_bytes(turns)
        timestamp_ms = parse_longmemeval_date(date)
        sources.append(
            {
                "ref": source_ref,
                "kind": "conversation",
                "title": f"LongMemEval session {session_id} occurrence {session_occurrence}",
                "uri": f"longmemeval://{entry['question_id']}/session/{session_index + 1}",
                "content_checksum": hashlib.sha256(session_bytes).hexdigest(),
                "byte_len": len(session_bytes),
                "metadata": {
                    "adapter": "nahuali-longmemeval-v1",
                    "question_id": entry["question_id"],
                    "session_date": date,
                    "session_id": session_id,
                    "canonical_session_id": session_id,
                    "session_ref": source_ref,
                    "session_position": str(session_index + 1),
                    "session_occurrence": str(session_occurrence),
                    "session_occurrence_count": str(session_totals[session_id]),
                },
                "scope": scope,
                "timestamp_ms": timestamp_ms,
            }
        )
        for turn_index, turn in enumerate(turns):
            raw_turn_count += 1
            if not turn["content"].strip():
                skipped_empty_turn_count += 1
                continue
            episodes.append(
                {
                    "ref": f"turn-{session_index + 1}-{turn_index + 1}",
                    "content": turn["content"],
                    "source_role": turn["role"],
                    "source_ref": source_ref,
                    "source_position": turn_index + 1,
                    "scope": scope,
                    "timestamp_ms": timestamp_ms + turn_index,
                }
            )

    return {"version": 1, "sources": sources, "episodes": episodes}, {
        "scope": scope_text,
        "source_count": len(sources),
        "raw_session_occurrence_count": len(sources),
        "canonical_session_id_count": len(session_totals),
        "duplicate_session_id_count": sum(1 for count in session_totals.values() if count > 1),
        "duplicate_session_occurrence_count": len(sources) - len(session_totals),
        "raw_turn_count": raw_turn_count,
        "indexed_turn_count": len(episodes),
        "skipped_empty_turn_count": skipped_empty_turn_count,
    }


def resolve_binary(binary: str) -> pathlib.Path:
    candidate = pathlib.Path(binary).expanduser()
    if candidate.is_file():
        return candidate.resolve()
    resolved = shutil.which(binary)
    if resolved is None:
        raise AdapterError(f"Nahuali binary not found: {binary}")
    return pathlib.Path(resolved).resolve()


def run_cli(
    binary: pathlib.Path,
    home: pathlib.Path,
    arguments: Sequence[str],
    environment: Optional[Dict[str, str]] = None,
) -> Any:
    env = os.environ.copy()
    env["NAHUALI_HOME"] = str(home)
    env["NO_COLOR"] = "1"
    env.pop("NAHUALI_DB_URL", None)
    if environment:
        env.update(environment)
    completed = subprocess.run(
        [str(binary), *arguments],
        check=False,
        capture_output=True,
        text=True,
        env=env,
    )
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip() or "no diagnostic output"
        raise AdapterError(
            f"nahuali {' '.join(arguments[:2])} failed with exit {completed.returncode}: {detail}"
        )
    try:
        return json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise AdapterError("nahuali command did not emit valid JSON") from error


def import_question(
    binary: pathlib.Path,
    home: pathlib.Path,
    database: str,
    entry: Dict[str, Any],
    workdir: pathlib.Path,
) -> Tuple[Dict[str, Dict[str, Any]], Dict[str, Any]]:
    interchange, manifest = build_interchange(entry)
    interchange_path = workdir / f"{database}.interchange.json"
    interchange_path.write_text(
        json.dumps(interchange, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )

    started = time.perf_counter_ns()
    dry_run = run_cli(
        binary,
        home,
        ["import", str(interchange_path), "--database", database, "--dry-run", "--json"],
    )
    if not dry_run.get("report", {}).get("valid"):
        raise AdapterError(f"Nahuali rejected the import preflight for {entry['question_id']}")
    imported = run_cli(
        binary,
        home,
        ["import", str(interchange_path), "--database", database, "--json"],
    )
    if imported.get("report", {}).get("imported_event_count", 0) <= 0:
        raise AdapterError(f"Nahuali imported no events for {entry['question_id']}")
    import_latency_ms = (time.perf_counter_ns() - started) / 1_000_000

    projection = run_cli(binary, home, ["data", "--database", database, "--json"])
    sources = projection.get("sources", [])
    episodes = projection.get("episodes", [])
    source_to_session: Dict[str, Dict[str, Any]] = {}
    for source in sources:
        metadata = source.get("metadata", {})
        session_id = metadata.get("canonical_session_id") or metadata.get("session_id")
        if session_id:
            source_to_session[source["id"]] = {
                "session_id": session_id,
                "date": metadata.get("session_date"),
                "session_ref": metadata.get("session_ref"),
                "session_position": metadata.get("session_position"),
                "session_occurrence": metadata.get("session_occurrence"),
            }

    episode_map: Dict[str, Dict[str, Any]] = {}
    for episode in episodes:
        source = source_to_session.get(episode.get("source_id"))
        if source:
            episode_map[episode["id"]] = {**episode, **source}

    if len(episode_map) != manifest["indexed_turn_count"]:
        raise AdapterError(
            f"Nahuali projected {len(episode_map)} of "
            f"{manifest['indexed_turn_count']} indexed turns"
        )
    manifest.update(
        {
            "database": database,
            "import_latency_ms": import_latency_ms,
            "raw_dates_preserved": True,
            "turn_roles_preserved": True,
            "turn_positions_preserved": True,
        }
    )
    return episode_map, manifest


def official_dcg(relevances: Sequence[int], k: int) -> float:
    """Match LongMemEval v1's published ``eval_utils.py`` implementation exactly."""

    values = list(relevances[:k])
    if not values:
        return 0.0
    return float(values[0]) + sum(
        float(relevance) / math.log2(index + 1)
        for index, relevance in enumerate(values[1:], start=1)
    )


def retrieval_metrics(
    ranked_session_ids: Sequence[str], relevant_session_ids: Sequence[str]
) -> Dict[str, float]:
    relevant = set(relevant_session_ids)
    if not relevant:
        raise AdapterError("retrieval metrics require at least one answer session")
    metrics: Dict[str, float] = {}
    ideal_relevances = [1] * len(relevant)
    for k in K_VALUES:
        retrieved = set(ranked_session_ids[:k])
        relevance = [1 if session_id in relevant else 0 for session_id in ranked_session_ids]
        ideal_dcg = official_dcg(ideal_relevances, k)
        metrics[f"recall_any@{k}"] = float(bool(relevant.intersection(retrieved)))
        metrics[f"recall_all@{k}"] = float(relevant.issubset(retrieved))
        metrics[f"ndcg_any@{k}"] = official_dcg(relevance, k) / ideal_dcg if ideal_dcg else 0.0
    return metrics


def ranked_sessions(
    results: Sequence[Dict[str, Any]], episode_map: Dict[str, Dict[str, Any]]
) -> List[Dict[str, Any]]:
    ranked: List[Dict[str, Any]] = []
    seen = set()
    for result in results:
        episode = episode_map.get(result.get("id"))
        if episode is None or episode["session_id"] in seen:
            continue
        seen.add(episode["session_id"])
        item = {
            "rank": len(ranked) + 1,
            "session_id": episode["session_id"],
            "session_date": episode.get("date"),
            "source_session_ref": episode.get("session_ref"),
            "source_session_position": episode.get("session_position"),
            "source_session_occurrence": episode.get("session_occurrence"),
            "episode_id": result["id"],
            "source_position": episode.get("source_position"),
            "source_role": episode.get("source_role"),
            "score": result.get("score"),
            "excerpt": result.get("excerpt", episode.get("content", "")),
        }
        for key in ("lexical_score", "semantic_score"):
            if key in result:
                item[key] = result[key]
        ranked.append(item)
        if len(ranked) == max(K_VALUES):
            break
    return ranked


def delete_qdrant_collection(collection_name: str) -> None:
    base_url = os.environ.get("NAHUALI_QDRANT_URL", "http://localhost:16333").rstrip("/")
    endpoint = f"{base_url}/collections/{urlparse.quote(collection_name, safe='')}"
    request = urlrequest.Request(endpoint, method="DELETE")
    token = os.environ.get("NAHUALI_QDRANT_API_KEY", "").strip()
    if token:
        request.add_header("api-key", token)
    try:
        with urlrequest.urlopen(request, timeout=15) as response:
            payload = json.loads(response.read().decode("utf-8"))
    except urlerror.HTTPError as error:
        if error.code == 404:
            return
        raise AdapterError(
            f"failed to remove temporary Qdrant collection {collection_name}: HTTP {error.code}"
        ) from error
    except urlerror.URLError as error:
        raise AdapterError(
            f"failed to remove temporary Qdrant collection {collection_name}: {error.reason}"
        ) from error
    if payload.get("status") != "ok":
        raise AdapterError(f"Qdrant returned an unexpected cleanup result for {collection_name}")


def evaluate_mode(
    mode: str,
    binary: pathlib.Path,
    home: pathlib.Path,
    database: str,
    scope: str,
    question: str,
    answer_session_ids: Sequence[str],
    episode_map: Dict[str, Dict[str, Any]],
    measured_runs: int,
    collection_base: str,
) -> Tuple[Dict[str, Any], List[str]]:
    semantic = mode != "lexical"
    environment: Dict[str, str] = {}
    cleanup_collections: List[str] = []
    embedding = None
    index_latency_ms = None

    if semantic:
        environment = {
            "NAHUALI_QDRANT_COLLECTION": collection_base,
            "NAHUALI_EMBEDDING_PROVIDER": (
                "deterministic" if mode == "deterministic-hybrid" else "local-model"
            ),
        }
        if mode == "local-model-hybrid":
            model_path = os.environ.get("NAHUALI_LOCAL_EMBEDDING_MODEL_PATH", "").strip()
            if not model_path:
                raise AdapterError(
                    "local-model-hybrid requires NAHUALI_LOCAL_EMBEDDING_MODEL_PATH"
                )
            environment["NAHUALI_LOCAL_EMBEDDING_MODEL_PATH"] = model_path
        started = time.perf_counter_ns()
        rebuilt = run_cli(
            binary,
            home,
            ["semantic-rebuild", "--database", database, "--json"],
            environment,
        )
        index_latency_ms = (time.perf_counter_ns() - started) / 1_000_000
        status = run_cli(
            binary,
            home,
            ["semantic-status", "--database", database, "--json"],
            environment,
        )
        semantic_status = status.get("status", {})
        if not semantic_status.get("is_current"):
            raise AdapterError(f"semantic index is not current for {database} in mode {mode}")
        collection_name = semantic_status.get("collection_name")
        if collection_name:
            cleanup_collections.append(collection_name)
        embedding = rebuilt.get("report", {}).get("embedding")

    arguments = [
        "recall",
        question,
        "--database",
        database,
        "--scope",
        scope,
        "--kind",
        "episode",
        "--limit",
        str(max(len(episode_map), max(K_VALUES))),
        "--json",
    ]
    if semantic:
        arguments.insert(-1, "--semantic")

    latencies: List[float] = []
    rankings: List[List[Dict[str, Any]]] = []
    result_ids: List[List[str]] = []
    for _ in range(measured_runs):
        started = time.perf_counter_ns()
        response = run_cli(binary, home, arguments, environment)
        latencies.append((time.perf_counter_ns() - started) / 1_000_000)
        results = response.get("results", []) if semantic else response
        if not isinstance(results, list):
            raise AdapterError(f"Nahuali returned an invalid recall payload in mode {mode}")
        rankings.append(ranked_sessions(results, episode_map))
        result_ids.append([result.get("id", "") for result in results])

    if any(ids != result_ids[0] for ids in result_ids[1:]):
        raise AdapterError(f"Nahuali returned a non-deterministic turn ranking in mode {mode}")
    if any(ranking != rankings[0] for ranking in rankings[1:]):
        raise AdapterError(f"Nahuali returned a non-deterministic session ranking in mode {mode}")

    ranking = rankings[0]
    return {
        "mode": mode,
        "status": "complete",
        "ranked_items": ranking,
        "metrics": retrieval_metrics(
            [item["session_id"] for item in ranking], answer_session_ids
        ),
        "retrieval_latency_ms": latencies,
        "index_latency_ms": index_latency_ms,
        "embedding": embedding,
    }, cleanup_collections


def percentile(values: Sequence[float], fraction: float) -> float:
    ordered = sorted(values)
    index = max(0, math.ceil(fraction * len(ordered)) - 1)
    return ordered[index]


def aggregate_mode(raw_results: Sequence[Dict[str, Any]], mode: str) -> Dict[str, Any]:
    evaluated = [
        result
        for result in raw_results
        if not result["excluded_from_retrieval_metrics"]
    ]
    metrics = {
        name: statistics.fmean(result["modes"][mode]["metrics"][name] for result in evaluated)
        for k in K_VALUES
        for name in (f"recall_any@{k}", f"recall_all@{k}", f"ndcg_any@{k}")
    }
    retrieval_latencies = [
        sample
        for result in raw_results
        for sample in result["modes"][mode]["retrieval_latency_ms"]
    ]
    index_latencies = [
        result["modes"][mode]["index_latency_ms"]
        for result in raw_results
        if result["modes"][mode]["index_latency_ms"] is not None
    ]
    latency = {
        "retrieval_ms": {
            "sample_count": len(retrieval_latencies),
            "median": statistics.median(retrieval_latencies),
            "p95": percentile(retrieval_latencies, 0.95),
            "maximum": max(retrieval_latencies),
        }
    }
    if index_latencies:
        latency["index_ms"] = {
            "sample_count": len(index_latencies),
            "median": statistics.median(index_latencies),
            "p95": percentile(index_latencies, 0.95),
            "maximum": max(index_latencies),
        }
    return {
        "status": "complete",
        "evaluated_question_count": len(evaluated),
        "excluded_question_count": len(raw_results) - len(evaluated),
        "metrics": metrics,
        "latency": latency,
    }


def selected_modes(requested: Optional[Sequence[str]]) -> List[str]:
    modes = list(requested or ("lexical", "deterministic-hybrid"))
    if os.environ.get("NAHUALI_LOCAL_EMBEDDING_MODEL_PATH", "").strip():
        if "local-model-hybrid" not in modes:
            modes.append("local-model-hybrid")
    if len(set(modes)) != len(modes):
        raise AdapterError("each mode may be selected only once")
    if "local-model-hybrid" in modes and not os.environ.get(
        "NAHUALI_LOCAL_EMBEDDING_MODEL_PATH", ""
    ).strip():
        raise AdapterError(
            "local-model-hybrid was selected without NAHUALI_LOCAL_EMBEDDING_MODEL_PATH"
        )
    return modes


def write_raw_ndjson(path: pathlib.Path, results: Sequence[Dict[str, Any]]) -> None:
    atomic_write_text(
        path,
        "".join(json.dumps(result, ensure_ascii=False) + "\n" for result in results),
    )


def write_hypotheses_template(path: pathlib.Path, results: Sequence[Dict[str, Any]]) -> None:
    """Write the exact two-field shape accepted by LongMemEval's QA evaluator."""

    atomic_write_text(
        path,
        "".join(
            json.dumps({"question_id": result["question_id"], "hypothesis": ""}) + "\n"
            for result in results
        ),
    )


def preflight_official_dataset(dataset: pathlib.Path) -> Dict[str, Any]:
    """Validate every instance in the exact pinned LongMemEval-S corpus."""

    dataset_path = dataset.expanduser().resolve()
    if not dataset_path.is_file():
        raise AdapterError(f"LongMemEval dataset not found: {dataset_path}")
    observed_size = dataset_path.stat().st_size
    if observed_size != OFFICIAL_DATASET_SIZE:
        raise AdapterError(
            f"official dataset size mismatch: expected {OFFICIAL_DATASET_SIZE}, "
            f"got {observed_size}"
        )
    observed_sha256 = sha256_file(dataset_path)
    if observed_sha256 != OFFICIAL_DATASET_SHA256:
        raise AdapterError(
            f"official dataset SHA-256 mismatch: expected {OFFICIAL_DATASET_SHA256}, "
            f"got {observed_sha256}"
        )

    question_ids = set()
    answer_types: Counter[str] = Counter()
    totals: Counter[str] = Counter()
    for candidate in iter_json_array(dataset_path):
        entry = validate_question(candidate)
        question_id = entry["question_id"]
        if question_id in question_ids:
            raise AdapterError(f"duplicate question_id in dataset: {question_id}")
        question_ids.add(question_id)
        document, manifest = build_interchange(entry)

        source_refs = [source["ref"] for source in document["sources"]]
        source_uris = [source["uri"] for source in document["sources"]]
        episode_refs = [episode["ref"] for episode in document["episodes"]]
        if len(source_refs) != len(set(source_refs)):
            raise AdapterError(f"question {question_id} produced duplicate positional source refs")
        if len(source_uris) != len(set(source_uris)):
            raise AdapterError(f"question {question_id} produced duplicate positional source URIs")
        if len(episode_refs) != len(set(episode_refs)):
            raise AdapterError(f"question {question_id} produced duplicate positional turn refs")
        if any(not episode["content"].strip() for episode in document["episodes"]):
            raise AdapterError(f"question {question_id} indexed an empty turn")
        if len(document["sources"]) != manifest["raw_session_occurrence_count"]:
            raise AdapterError(f"question {question_id} has inconsistent session counts")
        if len(document["episodes"]) != manifest["indexed_turn_count"]:
            raise AdapterError(f"question {question_id} has inconsistent indexed-turn counts")
        if (
            manifest["raw_turn_count"]
            != manifest["indexed_turn_count"] + manifest["skipped_empty_turn_count"]
        ):
            raise AdapterError(f"question {question_id} has inconsistent raw-turn counts")

        answer_types[json_type_name(entry["answer"])] += 1
        totals["raw_session_occurrence_count"] += manifest[
            "raw_session_occurrence_count"
        ]
        totals["canonical_session_id_count"] += manifest["canonical_session_id_count"]
        totals["duplicate_session_id_count"] += manifest["duplicate_session_id_count"]
        totals["duplicate_session_occurrence_count"] += manifest[
            "duplicate_session_occurrence_count"
        ]
        totals["raw_turn_count"] += manifest["raw_turn_count"]
        totals["indexed_turn_count"] += manifest["indexed_turn_count"]
        totals["skipped_empty_turn_count"] += manifest["skipped_empty_turn_count"]
        totals["duplicate_session_question_count"] += bool(
            manifest["duplicate_session_occurrence_count"]
        )

    if len(question_ids) != OFFICIAL_QUESTION_COUNT:
        raise AdapterError(
            f"official dataset question count mismatch: expected {OFFICIAL_QUESTION_COUNT}, "
            f"got {len(question_ids)}"
        )
    return {
        "status": "compatible",
        "dataset_revision": OFFICIAL_DATASET_REVISION,
        "dataset_sha256": observed_sha256,
        "dataset_size": observed_size,
        "question_count": len(question_ids),
        "answer_types": dict(sorted(answer_types.items())),
        **dict(totals),
    }


def run_benchmark(arguments: argparse.Namespace) -> Dict[str, Any]:
    dataset_path = arguments.dataset.expanduser().resolve()
    if not dataset_path.is_file():
        raise AdapterError(f"LongMemEval dataset not found: {dataset_path}")
    if not arguments.dataset_version.strip() or not arguments.dataset_revision.strip():
        raise AdapterError("dataset version and revision inputs must be non-empty")
    if not LOWER_SOURCE_REVISION.fullmatch(arguments.source_revision):
        raise AdapterError("--source-revision must be an exact lowercase 40-character commit SHA")
    if arguments.measured_runs < 1:
        raise AdapterError("--measured-runs must be at least 1")
    if arguments.limit is not None and arguments.limit < 1:
        raise AdapterError("--limit must be at least 1")
    binary = resolve_binary(arguments.binary)
    output_paths = [
        path.expanduser().resolve()
        for path in (arguments.output, arguments.raw_output, arguments.hypotheses_output)
        if path is not None
    ]
    if len(set(output_paths)) != len(output_paths):
        raise AdapterError("output, raw output, and hypotheses output must use distinct paths")
    for output_path in output_paths:
        if paths_refer_to_same_file(output_path, dataset_path):
            raise AdapterError("benchmark outputs must not overwrite the input dataset")
        if paths_refer_to_same_file(output_path, binary):
            raise AdapterError("benchmark outputs must not overwrite the Nahuali binary")

    binary_sha256 = sha256_file(binary)
    dataset_identity = run_dataset_identity(
        dataset_path,
        arguments.dataset_revision.strip(),
        arguments.expected_dataset_sha256,
    )
    dataset_sha256 = dataset_identity["sha256"]
    source_start = source_snapshot()
    if (
        source_start["head_revision"] is not None
        and source_start["head_revision"] != arguments.source_revision
    ):
        raise AdapterError(
            "--source-revision does not match the repository HEAD captured at run start"
        )
    modes = selected_modes(arguments.mode)
    semantic_selected = any(mode != "lexical" for mode in modes)
    qdrant_class = "not_used"
    if semantic_selected:
        qdrant_class = qdrant_endpoint_class(
            os.environ.get("NAHUALI_QDRANT_URL", "http://localhost:16333")
        )
        if qdrant_class == "remote" and not arguments.allow_remote_qdrant:
            raise AdapterError(
                "remote Qdrant is blocked for dataset safety; pass --allow-remote-qdrant "
                "only after authorizing that data transfer"
            )
    local_model_start = None
    if "local-model-hybrid" in modes:
        local_model_start = local_model_artifact_manifest(
            pathlib.Path(os.environ["NAHUALI_LOCAL_EMBEDDING_MODEL_PATH"])
        )
    version = subprocess.run(
        [str(binary), "--version"], check=True, capture_output=True, text=True
    ).stdout.strip()
    root = pathlib.Path(tempfile.mkdtemp(prefix="nahuali-longmemeval-"))
    home = root / "home"
    workdir = root / "interchange"
    workdir.mkdir(parents=True)
    run_nonce = hashlib.sha256(
        f"{dataset_sha256}:{binary_sha256}:{os.getpid()}:{time.time_ns()}".encode("utf-8")
    ).hexdigest()[:12]

    raw_results: List[Dict[str, Any]] = []
    observed_question_ids = set()
    abstention_count = 0
    cleanup_errors: List[str] = []
    active_error = False
    selection_truncated = False
    progress_total: Any = arguments.limit
    if progress_total is None and arguments.dataset_revision == OFFICIAL_DATASET_REVISION:
        progress_total = OFFICIAL_QUESTION_COUNT
    if progress_total is None:
        progress_total = "?"
    try:
        for index, candidate in enumerate(iter_json_array(dataset_path)):
            if arguments.limit is not None and len(raw_results) >= arguments.limit:
                selection_truncated = True
                break
            entry = validate_question(candidate)
            question_id = entry["question_id"]
            if question_id in observed_question_ids:
                raise AdapterError(f"duplicate question_id in dataset: {question_id}")
            observed_question_ids.add(question_id)
            print(
                f"LongMemEval [{len(raw_results) + 1}/{progress_total}] "
                f"evaluating {question_id}",
                file=sys.stderr,
                flush=True,
            )
            database = question_database(dataset_sha256, question_id, run_nonce)
            scope, _ = question_scope(question_id)
            episode_map, ingestion = import_question(
                binary, home, database, entry, workdir
            )
            abstention = "_abs" in question_id
            exclusion_reason = "abstention" if abstention else None
            if abstention:
                abstention_count += 1
            elif not entry["answer_session_ids"]:
                exclusion_reason = "no_retrieval_target"

            question_modes: Dict[str, Any] = {}
            for mode in modes:
                collection_base = f"nahuali_lme_{run_nonce}_{index}_{mode.replace('-', '_')}"
                collections: List[str] = []
                try:
                    if exclusion_reason:
                        # Raw rankings are still produced for abstentions; only metrics are omitted.
                        evaluation, collections = evaluate_mode(
                            mode,
                            binary,
                            home,
                            database,
                            scope,
                            entry["question"],
                            entry["answer_session_ids"] or ["__no_target__"],
                            episode_map,
                            arguments.measured_runs,
                            collection_base,
                        )
                        evaluation["metrics"] = None
                    else:
                        evaluation, collections = evaluate_mode(
                            mode,
                            binary,
                            home,
                            database,
                            scope,
                            entry["question"],
                            entry["answer_session_ids"],
                            episode_map,
                            arguments.measured_runs,
                            collection_base,
                        )
                    question_modes[mode] = evaluation
                finally:
                    expected = f"{collection_base}__{database}"
                    names = list(collections)
                    if mode != "lexical" and expected not in names:
                        names.append(expected)
                    for collection_name in names:
                        try:
                            delete_qdrant_collection(collection_name)
                        except AdapterError as error:
                            cleanup_errors.append(str(error))
                    collections = []

            raw_results.append(
                {
                    "question_id": question_id,
                    "question_type": entry["question_type"],
                    "question": entry["question"],
                    "question_date": entry["question_date"],
                    "answer": entry["answer"],
                    "answer_session_ids": entry["answer_session_ids"],
                    "haystack_session_ids": entry["haystack_session_ids"],
                    "haystack_dates": entry["haystack_dates"],
                    "abstention": abstention,
                    "excluded_from_retrieval_metrics": exclusion_reason is not None,
                    "exclusion_reason": exclusion_reason,
                    "ingestion": ingestion,
                    "modes": question_modes,
                }
            )
            print(
                f"LongMemEval [{len(raw_results)}/{progress_total}] completed {question_id}",
                file=sys.stderr,
                flush=True,
            )
    except BaseException:
        active_error = True
        raise
    finally:
        shutil.rmtree(root, ignore_errors=True)
        if cleanup_errors and not active_error:
            raise AdapterError("; ".join(cleanup_errors))

    if not raw_results:
        raise AdapterError("the selected dataset range contains no questions")
    if all(result["excluded_from_retrieval_metrics"] for result in raw_results):
        raise AdapterError("the selected dataset range contains no scorable retrieval questions")

    dataset_final = run_dataset_identity(
        dataset_path,
        arguments.dataset_revision.strip(),
        arguments.expected_dataset_sha256,
    )
    if dataset_final != dataset_identity:
        raise AdapterError("dataset identity changed while the benchmark was running")
    if sha256_file(binary) != binary_sha256:
        raise AdapterError("Nahuali binary changed while the benchmark was running")
    source_final = source_snapshot()
    source_head_stable = source_start["head_revision"] == source_final["head_revision"]
    if source_start["head_revision"] is not None and not source_head_stable:
        raise AdapterError("repository HEAD changed while the benchmark was running")
    source_worktree_stable = (
        source_start["worktree_fingerprint"] == source_final["worktree_fingerprint"]
    )
    if source_start["worktree_fingerprint"] is not None and not source_worktree_stable:
        raise AdapterError("repository worktree changed while the benchmark was running")
    if source_start["adapter_sha256"] != source_final["adapter_sha256"]:
        raise AdapterError("LongMemEval adapter changed while the benchmark was running")
    local_model_stable = None
    if local_model_start is not None:
        local_model_final = local_model_artifact_manifest(
            pathlib.Path(os.environ["NAHUALI_LOCAL_EMBEDDING_MODEL_PATH"])
        )
        local_model_stable = local_model_start == local_model_final
        if not local_model_stable:
            raise AdapterError("local model artifacts changed while the benchmark was running")

    ingestion_counts = {
        name: sum(result["ingestion"][name] for result in raw_results)
        for name in (
            "raw_session_occurrence_count",
            "canonical_session_id_count",
            "duplicate_session_id_count",
            "duplicate_session_occurrence_count",
            "raw_turn_count",
            "indexed_turn_count",
            "skipped_empty_turn_count",
        )
    }

    report = {
        "result_version": RESULT_VERSION,
        "benchmark": {
            "name": "LongMemEval v1",
            "task": "retrieval-only",
            "granularity": "session",
            "relationship": "first-party",
            "official_evaluator_revision": OFFICIAL_EVALUATOR_REVISION,
            "qa_score": None,
            "qa_status": "not_evaluated",
        },
        "dataset": {
            "filename": dataset_path.name,
            "sha256": dataset_sha256,
            "size": dataset_identity["size"],
            "expected_sha256": dataset_identity["expected_sha256"],
            "identity_policy": dataset_identity["identity_policy"],
            "version_input": arguments.dataset_version,
            "revision_input": arguments.dataset_revision,
            "selection_limit": arguments.limit,
            "selection_policy": (
                "dataset_order_prefix" if selection_truncated else "complete_dataset"
            ),
            "complete_dataset": not selection_truncated,
            "selected_question_count": len(raw_results),
            "abstention_question_count": abstention_count,
            "ingestion_counts": ingestion_counts,
        },
        "system": {
            "name": "Nahuali",
            "version": version,
            "binary_name": binary.name,
            "binary_sha256": binary_sha256,
            "source_revision_input": arguments.source_revision,
            "source_revision_matches_start_head": (
                None
                if source_start["head_revision"] is None
                else source_start["head_revision"] == arguments.source_revision
            ),
            "source_start": source_start,
            "source_final": source_final,
            "source_head_stable": source_head_stable,
            "source_worktree_stable": source_worktree_stable,
            "source_worktree_state": source_start["worktree_state"],
        },
        "runner": {
            "relationship": "first-party",
            "adapter": "benchmarks/longmemeval/adapter.py",
            "dependencies": ["python-stdlib", "nahuali-cli"],
            "environment": runtime_environment(),
        },
        "configuration": {
            "k_values": list(K_VALUES),
            "modes": modes,
            "measured_runs": arguments.measured_runs,
            "abstention_policy": "preserved_in_raw_results_excluded_from_retrieval_metrics",
            "question_isolation": "unique_database_and_scope",
            "ranking_unit": "turn_then_first_canonical_session_id_occurrence",
            "duplicate_session_policy": (
                "preserve_positional_source_occurrences_and_assign_retrieval_credit_"
                "once_per_canonical_session_id"
            ),
            "empty_turn_policy": "count_raw_and_skip_blank_content_from_indexing",
            "indexed_roles": ["user", "assistant"],
            "latency_population": "all_selected_questions_including_abstentions",
            "local_model_artifacts": local_model_start,
            "local_model_artifacts_stable": local_model_stable,
            "qdrant_endpoint_class": qdrant_class,
            "remote_qdrant_explicitly_allowed": bool(arguments.allow_remote_qdrant),
            "retrieval_latency_boundary": "wall_clock_subprocess_invocation_including_cli_startup",
            "semantic_index_latency_boundary": "wall_clock_subprocess_invocation_including_cli_startup",
        },
        "aggregates": {mode: aggregate_mode(raw_results, mode) for mode in modes},
        "raw_results": raw_results,
        "qa_handoff": {
            "status": "reader_required",
            "hypotheses_template_filename": arguments.hypotheses_output.name
            if arguments.hypotheses_output
            else None,
            "claim": "No reader or official LongMemEval QA evaluation was run.",
        },
        "artifact_handling": {
            "contains_dataset_content": True,
            "absolute_local_paths_recorded": False,
            "output_file_mode": "0600",
        },
    }
    atomic_write_text(arguments.output, json.dumps(report, indent=2, ensure_ascii=False) + "\n")
    if arguments.raw_output:
        write_raw_ndjson(arguments.raw_output, raw_results)
    if arguments.hypotheses_output:
        write_hypotheses_template(arguments.hypotheses_output, raw_results)
    return report


def close(left: float, right: float) -> bool:
    return abs(left - right) <= 1e-9


def finite_nonnegative(value: Any) -> bool:
    return (
        isinstance(value, (int, float))
        and not isinstance(value, bool)
        and math.isfinite(float(value))
        and value >= 0
    )


def validate_report(path: pathlib.Path) -> Dict[str, Any]:
    permissions_safe = stat.S_IMODE(path.stat().st_mode) == 0o600
    report = json.loads(path.read_text(encoding="utf-8"))
    if report.get("result_version") != RESULT_VERSION:
        raise AdapterError("unsupported LongMemEval adapter result version")
    if report.get("benchmark", {}).get("task") != "retrieval-only":
        raise AdapterError("report does not identify itself as retrieval-only")
    if report.get("benchmark", {}).get("relationship") != "first-party":
        raise AdapterError("report must disclose the first-party relationship")
    if (
        report.get("benchmark", {}).get("official_evaluator_revision")
        != OFFICIAL_EVALUATOR_REVISION
    ):
        raise AdapterError("report has an invalid official evaluator revision")
    if report.get("benchmark", {}).get("qa_score") is not None:
        raise AdapterError("retrieval-only reports must not contain a QA score")
    if report.get("benchmark", {}).get("qa_status") != "not_evaluated":
        raise AdapterError("retrieval-only reports must mark QA as not evaluated")
    environment = report.get("runner", {}).get("environment", {})
    for field in (
        "operating_system",
        "operating_system_release",
        "architecture",
        "python_version",
        "logical_cpu_count",
    ):
        if environment.get(field) in (None, ""):
            raise AdapterError(f"report lacks runner environment field {field}")
    configuration = report.get("configuration", {})
    modes = configuration.get("modes", [])
    if (
        not isinstance(modes, list)
        or not modes
        or len(modes) != len(set(modes))
        or any(mode not in MODE_NAMES for mode in modes)
    ):
        raise AdapterError("report has invalid or duplicate retrieval modes")
    if configuration.get("k_values") != list(K_VALUES):
        raise AdapterError("report has unsupported retrieval cutoffs")
    measured_runs = configuration.get("measured_runs")
    if not isinstance(measured_runs, int) or isinstance(measured_runs, bool) or measured_runs < 1:
        raise AdapterError("report has an invalid measured-run count")
    expected_latency_boundary = "wall_clock_subprocess_invocation_including_cli_startup"
    if configuration.get("retrieval_latency_boundary") != expected_latency_boundary:
        raise AdapterError("report has an unsupported retrieval latency boundary")
    if configuration.get("semantic_index_latency_boundary") != expected_latency_boundary:
        raise AdapterError("report has an unsupported semantic index latency boundary")
    if configuration.get("ranking_unit") != (
        "turn_then_first_canonical_session_id_occurrence"
    ):
        raise AdapterError("report has an unsupported ranking unit")
    if configuration.get("duplicate_session_policy") != (
        "preserve_positional_source_occurrences_and_assign_retrieval_credit_"
        "once_per_canonical_session_id"
    ):
        raise AdapterError("report has an unsupported duplicate-session policy")
    if configuration.get("empty_turn_policy") != (
        "count_raw_and_skip_blank_content_from_indexing"
    ):
        raise AdapterError("report has an unsupported empty-turn policy")
    if configuration.get("latency_population") != (
        "all_selected_questions_including_abstentions"
    ):
        raise AdapterError("report has an unsupported latency population")
    semantic_selected = any(mode != "lexical" for mode in modes)
    qdrant_class = configuration.get("qdrant_endpoint_class")
    remote_allowed = configuration.get("remote_qdrant_explicitly_allowed")
    if semantic_selected:
        if qdrant_class not in {"loopback", "remote"}:
            raise AdapterError("semantic result has an invalid Qdrant endpoint class")
        if qdrant_class == "remote" and remote_allowed is not True:
            raise AdapterError("remote Qdrant result lacks an explicit transfer authorization")
    elif qdrant_class != "not_used":
        raise AdapterError("lexical-only result unexpectedly records a Qdrant endpoint")
    if not isinstance(remote_allowed, bool):
        raise AdapterError("report has an invalid remote-Qdrant authorization marker")
    if not LOWER_SHA256.fullmatch(report.get("dataset", {}).get("sha256", "")):
        raise AdapterError("report has an invalid dataset SHA-256")
    dataset = report.get("dataset", {})
    if dataset.get("expected_sha256") != dataset.get("sha256"):
        raise AdapterError("report does not bind the observed dataset to its expected SHA-256")
    if dataset.get("revision_input") == OFFICIAL_DATASET_REVISION:
        if (
            dataset.get("sha256") != OFFICIAL_DATASET_SHA256
            or dataset.get("size") != OFFICIAL_DATASET_SIZE
            or dataset.get("identity_policy")
            != "pinned_official_revision_sha256_and_size"
        ):
            raise AdapterError("report has an invalid pinned official dataset identity")
    elif dataset.get("identity_policy") != (
        "operator_supplied_sha256_for_non_pinned_revision"
    ):
        raise AdapterError("report has an invalid non-pinned dataset identity policy")
    if dataset.get("complete_dataset"):
        if dataset.get("selection_policy") != "complete_dataset":
            raise AdapterError("complete dataset report has an inconsistent selection policy")
    elif dataset.get("selection_policy") != "dataset_order_prefix":
        raise AdapterError("partial dataset report has an inconsistent selection policy")
    selection_limit = dataset.get("selection_limit")
    if selection_limit is not None and (
        not isinstance(selection_limit, int)
        or isinstance(selection_limit, bool)
        or selection_limit < 1
    ):
        raise AdapterError("report has an invalid dataset selection limit")
    if not LOWER_SHA256.fullmatch(report.get("system", {}).get("binary_sha256", "")):
        raise AdapterError("report has an invalid binary SHA-256")
    if not LOWER_SOURCE_REVISION.fullmatch(
        report.get("system", {}).get("source_revision_input", "")
    ):
        raise AdapterError("report has an invalid source revision input")
    if report.get("system", {}).get("source_worktree_state") not in {
        "clean",
        "dirty",
        "unavailable",
    }:
        raise AdapterError("report has an invalid source worktree state")
    system = report.get("system", {})
    source_start = system.get("source_start", {})
    source_final = system.get("source_final", {})
    for snapshot in (source_start, source_final):
        head_revision = snapshot.get("head_revision")
        fingerprint = snapshot.get("worktree_fingerprint")
        adapter_sha256 = snapshot.get("adapter_sha256")
        if head_revision is not None and not LOWER_SOURCE_REVISION.fullmatch(head_revision):
            raise AdapterError("report has an invalid captured source HEAD")
        if fingerprint is not None and not LOWER_SHA256.fullmatch(fingerprint):
            raise AdapterError("report has an invalid worktree fingerprint")
        if not LOWER_SHA256.fullmatch(adapter_sha256 or ""):
            raise AdapterError("report has an invalid adapter SHA-256")
        if snapshot.get("worktree_state") not in {"clean", "dirty", "unavailable"}:
            raise AdapterError("report has an invalid captured worktree state")
    if source_start.get("head_revision") is not None:
        if source_start.get("head_revision") != system.get("source_revision_input"):
            raise AdapterError("report source revision does not match its captured start HEAD")
        if system.get("source_revision_matches_start_head") is not True:
            raise AdapterError("report does not confirm its source revision match")
    if source_start.get("head_revision") != source_final.get("head_revision"):
        raise AdapterError("report source HEAD changed during the run")
    if system.get("source_head_stable") is not True:
        raise AdapterError("report does not confirm stable source HEAD")
    expected_worktree_stability = (
        source_start.get("worktree_fingerprint")
        == source_final.get("worktree_fingerprint")
    )
    if system.get("source_worktree_stable") != expected_worktree_stability:
        raise AdapterError("report has an inconsistent worktree stability marker")
    if source_start.get("adapter_sha256") != source_final.get("adapter_sha256"):
        raise AdapterError("report adapter digest changed during the run")
    artifact_handling = report.get("artifact_handling", {})
    if artifact_handling != {
        "contains_dataset_content": True,
        "absolute_local_paths_recorded": False,
        "output_file_mode": "0600",
    }:
        raise AdapterError("report lacks the required dataset-content handling disclosure")
    for section_name in ("dataset", "system", "runner", "configuration", "qa_handoff"):
        section = report.get(section_name, {})
        if isinstance(section, dict) and any(
            key in section for key in ("path", "binary_path", "dataset_path", "output_path")
        ):
            raise AdapterError(f"report section {section_name} contains a local path field")
    hypotheses_filename = report.get("qa_handoff", {}).get(
        "hypotheses_template_filename"
    )
    if hypotheses_filename is not None and pathlib.Path(hypotheses_filename).name != (
        hypotheses_filename
    ):
        raise AdapterError("QA handoff must not contain an absolute or relative local path")

    local_model_artifacts = configuration.get("local_model_artifacts")
    if "local-model-hybrid" in configuration.get("modes", []):
        if (
            not isinstance(local_model_artifacts, dict)
            or not LOWER_SHA256.fullmatch(
                local_model_artifacts.get("artifact_set_sha256", "")
            )
            or local_model_artifacts.get("required_cli_feature") != "local-embeddings"
            or configuration.get("local_model_artifacts_stable") is not True
        ):
            raise AdapterError("local-model result lacks stable artifact provenance")
    elif local_model_artifacts is not None:
        raise AdapterError("non-model result unexpectedly contains local-model provenance")

    raw_results = report.get("raw_results", [])
    if not raw_results or not modes:
        raise AdapterError("report has no raw results or evaluated modes")
    question_ids = [result.get("question_id") for result in raw_results]
    if any(not isinstance(question_id, str) or not question_id for question_id in question_ids):
        raise AdapterError("report has an invalid question id")
    if len(question_ids) != len(set(question_ids)):
        raise AdapterError("report contains duplicate question ids")
    if dataset.get("selected_question_count") != len(raw_results):
        raise AdapterError("report has an inconsistent selected-question count")
    observed_abstentions = sum(bool(result.get("abstention")) for result in raw_results)
    if dataset.get("abstention_question_count") != observed_abstentions:
        raise AdapterError("report has an inconsistent abstention-question count")
    if selection_limit is not None and len(raw_results) > selection_limit:
        raise AdapterError("report selected more questions than its stated limit")
    if not dataset.get("complete_dataset") and selection_limit != len(raw_results):
        raise AdapterError("partial report does not match its dataset selection limit")
    if (
        dataset.get("revision_input") == OFFICIAL_DATASET_REVISION
        and dataset.get("complete_dataset")
        and len(raw_results) != OFFICIAL_QUESTION_COUNT
    ):
        raise AdapterError("complete official report does not contain all 500 questions")
    if set(report.get("aggregates", {})) != set(modes):
        raise AdapterError("report aggregate modes do not match its configured modes")
    for result in raw_results:
        if not is_json_value(result.get("answer")):
            raise AdapterError(f"question {result.get('question_id')} has an invalid JSON answer")
        answer_session_ids = result.get("answer_session_ids")
        haystack_session_ids = result.get("haystack_session_ids")
        if not isinstance(answer_session_ids, list) or not all(
            isinstance(value, str) and value for value in answer_session_ids
        ):
            raise AdapterError("report has invalid answer session ids")
        if not isinstance(haystack_session_ids, list) or not all(
            isinstance(value, str) and value for value in haystack_session_ids
        ):
            raise AdapterError("report has invalid haystack session ids")
        expected_abstention = "_abs" in result["question_id"]
        if result.get("abstention") is not expected_abstention:
            raise AdapterError("question has an invalid abstention marker")
        if not expected_abstention and not set(answer_session_ids).issubset(
            set(haystack_session_ids)
        ):
            raise AdapterError("scorable question has targets outside its haystack")
        expected_exclusion_reason = (
            "abstention"
            if expected_abstention
            else "no_retrieval_target"
            if not answer_session_ids
            else None
        )
        expected_excluded = expected_exclusion_reason is not None
        if result.get("exclusion_reason") != expected_exclusion_reason:
            raise AdapterError("question has an invalid retrieval exclusion reason")
        if result.get("excluded_from_retrieval_metrics") is not expected_excluded:
            raise AdapterError("question has an invalid retrieval exclusion marker")
        ingestion = result.get("ingestion", {})
        for name in (
            "source_count",
            "raw_session_occurrence_count",
            "canonical_session_id_count",
            "duplicate_session_id_count",
            "duplicate_session_occurrence_count",
            "raw_turn_count",
            "indexed_turn_count",
            "skipped_empty_turn_count",
        ):
            value = ingestion.get(name)
            if not isinstance(value, int) or isinstance(value, bool) or value < 0:
                raise AdapterError(
                    f"question {result.get('question_id')} has an invalid ingestion counter {name}"
                )
        if ingestion["source_count"] != ingestion["raw_session_occurrence_count"]:
            raise AdapterError("source count does not match raw session occurrences")
        if ingestion["duplicate_session_occurrence_count"] != (
            ingestion["raw_session_occurrence_count"]
            - ingestion["canonical_session_id_count"]
        ):
            raise AdapterError("duplicate session occurrence count is inconsistent")
        if ingestion.get("raw_turn_count") != (
            ingestion.get("indexed_turn_count", -1)
            + ingestion.get("skipped_empty_turn_count", -1)
        ):
            raise AdapterError(
                f"question {result.get('question_id')} has inconsistent ingestion counts"
            )
        excluded = result.get("excluded_from_retrieval_metrics")
        if result.get("abstention") and not excluded:
            raise AdapterError("abstention question was included in retrieval metrics")
        if set(result.get("modes", {})) != set(modes):
            raise AdapterError(
                f"question {result.get('question_id')} mode set does not match configuration"
            )
        for mode in modes:
            mode_result = result.get("modes", {}).get(mode)
            if not mode_result or mode_result.get("status") != "complete":
                raise AdapterError(f"question {result.get('question_id')} lacks mode {mode}")
            ranked_items = mode_result.get("ranked_items", [])
            if not isinstance(ranked_items, list):
                raise AdapterError("ranked items must be a list")
            if len(ranked_items) > max(K_VALUES):
                raise AdapterError("ranked item count exceeds the configured retrieval cutoff")
            if [item.get("rank") for item in ranked_items] != list(
                range(1, len(ranked_items) + 1)
            ):
                raise AdapterError("ranked item positions are not contiguous")
            ranked_ids = [item.get("session_id") for item in ranked_items]
            if any(not isinstance(session_id, str) or not session_id for session_id in ranked_ids):
                raise AdapterError("ranked item has an invalid canonical session id")
            if not set(ranked_ids).issubset(set(haystack_session_ids)):
                raise AdapterError("ranked item contains a session outside the question haystack")
            if len(ranked_ids) != len(set(ranked_ids)):
                raise AdapterError(
                    f"question {result.get('question_id')} mode {mode} contains duplicate "
                    "canonical session credit"
                )
            latencies = mode_result.get("retrieval_latency_ms")
            if (
                not isinstance(latencies, list)
                or len(latencies) != measured_runs
                or any(not finite_nonnegative(value) for value in latencies)
            ):
                raise AdapterError("mode result has invalid retrieval latency samples")
            index_latency = mode_result.get("index_latency_ms")
            embedding = mode_result.get("embedding")
            if mode == "lexical":
                if index_latency is not None:
                    raise AdapterError("lexical mode unexpectedly contains index latency")
                if embedding is not None:
                    raise AdapterError("lexical mode unexpectedly contains embedding metadata")
            else:
                if not finite_nonnegative(index_latency):
                    raise AdapterError("semantic mode lacks a valid index latency")
                if not isinstance(embedding, dict):
                    raise AdapterError("semantic mode lacks embedding metadata")
                dimensions = embedding.get("dimensions")
                if (
                    not isinstance(dimensions, int)
                    or isinstance(dimensions, bool)
                    or dimensions < 1
                ):
                    raise AdapterError("semantic mode has invalid embedding dimensions")
                if mode == "deterministic-hybrid" and (
                    embedding.get("kind") != "deterministic_local"
                    or embedding.get("model") != "nahuali/deterministic-ngram-v1"
                ):
                    raise AdapterError("deterministic mode has invalid embedding identity")
                if mode == "local-model-hybrid" and embedding.get("kind") != "local_model":
                    raise AdapterError("local-model mode has invalid embedding identity")
            if excluded:
                if mode_result.get("metrics") is not None:
                    raise AdapterError("excluded question contains retrieval metrics")
            else:
                expected = retrieval_metrics(ranked_ids, result["answer_session_ids"])
                for name, value in expected.items():
                    if not close(mode_result.get("metrics", {}).get(name, -1), value):
                        raise AdapterError(
                            f"question {result['question_id']} mode {mode} has invalid {name}"
                        )

    expected_ingestion_counts = {
        name: sum(result["ingestion"][name] for result in raw_results)
        for name in (
            "raw_session_occurrence_count",
            "canonical_session_id_count",
            "duplicate_session_id_count",
            "duplicate_session_occurrence_count",
            "raw_turn_count",
            "indexed_turn_count",
            "skipped_empty_turn_count",
        )
    }
    if report.get("dataset", {}).get("ingestion_counts") != expected_ingestion_counts:
        raise AdapterError("report has inconsistent aggregate ingestion counts")

    for mode in modes:
        expected = aggregate_mode(raw_results, mode)
        observed = report.get("aggregates", {}).get(mode, {})
        for field in ("status", "evaluated_question_count", "excluded_question_count"):
            if observed.get(field) != expected[field]:
                raise AdapterError(f"aggregate mode {mode} has invalid {field}")
        for name, value in expected["metrics"].items():
            if not close(observed.get("metrics", {}).get(name, -1), value):
                raise AdapterError(f"aggregate mode {mode} has invalid {name}")
        if set(observed.get("latency", {})) != set(expected["latency"]):
            raise AdapterError(f"aggregate mode {mode} has invalid latency boundaries")
        for boundary, expected_latency in expected["latency"].items():
            observed_latency = observed["latency"].get(boundary, {})
            if observed_latency.get("sample_count") != expected_latency["sample_count"]:
                raise AdapterError(
                    f"aggregate mode {mode} has invalid {boundary} sample count"
                )
            for field in ("median", "p95", "maximum"):
                if not close(observed_latency.get(field, -1), expected_latency[field]):
                    raise AdapterError(
                        f"aggregate mode {mode} has invalid {boundary} {field}"
                    )
    return {
        "valid": True,
        "question_count": len(raw_results),
        "modes": modes,
        "qa_status": "not_evaluated",
        "permissions_safe": permissions_safe,
    }


def download_official_dataset(arguments: argparse.Namespace) -> Dict[str, Any]:
    revision = arguments.revision.strip()
    if not LOWER_SOURCE_REVISION.fullmatch(revision):
        raise AdapterError("--revision must be an exact lowercase 40-character dataset commit SHA")
    expected_sha256 = arguments.expected_sha256
    if expected_sha256 is None and revision == OFFICIAL_DATASET_REVISION:
        expected_sha256 = OFFICIAL_DATASET_SHA256
    if not expected_sha256 or not LOWER_SHA256.fullmatch(expected_sha256):
        raise AdapterError(
            "an exact --expected-sha256 is required for revisions other than the pinned default"
        )

    cache_dir = arguments.cache_dir.expanduser().resolve()
    target = cache_dir / revision / OFFICIAL_DATASET_FILENAME
    target.parent.mkdir(parents=True, exist_ok=True)
    cached_size = target.stat().st_size if target.is_file() else None
    cached_size_matches = (
        revision != OFFICIAL_DATASET_REVISION or cached_size == OFFICIAL_DATASET_SIZE
    )
    if (
        target.is_file()
        and cached_size_matches
        and sha256_file(target) == expected_sha256
    ):
        return {
            "status": "cached",
            "path": str(target),
            "revision": revision,
            "sha256": expected_sha256,
            "size": cached_size,
        }

    partial = target.with_suffix(target.suffix + ".part")
    url = OFFICIAL_DATASET_URL.format(revision=urlparse.quote(revision, safe=""))
    request = urlrequest.Request(url, headers={"User-Agent": "nahuali-longmemeval-adapter/1"})
    digest = hashlib.sha256()
    size = 0
    try:
        with urlrequest.urlopen(request, timeout=120) as response, partial.open("wb") as output:
            while True:
                chunk = response.read(1024 * 1024)
                if not chunk:
                    break
                output.write(chunk)
                digest.update(chunk)
                size += len(chunk)
    except BaseException:
        partial.unlink(missing_ok=True)
        raise
    observed_sha256 = digest.hexdigest()
    if observed_sha256 != expected_sha256:
        partial.unlink(missing_ok=True)
        raise AdapterError(
            f"downloaded dataset SHA-256 mismatch: expected {expected_sha256}, got {observed_sha256}"
        )
    if revision == OFFICIAL_DATASET_REVISION and size != OFFICIAL_DATASET_SIZE:
        partial.unlink(missing_ok=True)
        raise AdapterError(
            f"downloaded dataset size mismatch: expected {OFFICIAL_DATASET_SIZE}, got {size}"
        )
    os.replace(partial, target)
    return {
        "status": "downloaded",
        "path": str(target),
        "revision": revision,
        "sha256": observed_sha256,
        "size": size,
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    run_parser = subparsers.add_parser("run", help="run retrieval-only evaluation")
    run_parser.add_argument("--dataset", type=pathlib.Path, required=True)
    run_parser.add_argument("--dataset-version", required=True)
    run_parser.add_argument("--dataset-revision", required=True)
    run_parser.add_argument("--expected-dataset-sha256")
    run_parser.add_argument("--binary", default="nahuali")
    run_parser.add_argument("--source-revision", required=True)
    run_parser.add_argument("--mode", action="append", choices=MODE_NAMES)
    run_parser.add_argument("--measured-runs", type=int, default=1)
    run_parser.add_argument("--limit", type=int)
    run_parser.add_argument("--allow-remote-qdrant", action="store_true")
    run_parser.add_argument("--output", type=pathlib.Path, required=True)
    run_parser.add_argument("--raw-output", type=pathlib.Path)
    run_parser.add_argument("--hypotheses-output", type=pathlib.Path)

    validate_parser = subparsers.add_parser("validate", help="validate a result artifact")
    validate_parser.add_argument("report", type=pathlib.Path)

    preflight_parser = subparsers.add_parser(
        "preflight", help="validate all 500 questions in the exact pinned official corpus"
    )
    preflight_parser.add_argument("--dataset", type=pathlib.Path, required=True)

    download_parser = subparsers.add_parser(
        "download", help="download the pinned official cleaned LongMemEval-S dataset"
    )
    download_parser.add_argument("--cache-dir", type=pathlib.Path, required=True)
    download_parser.add_argument("--revision", default=OFFICIAL_DATASET_REVISION)
    download_parser.add_argument("--expected-sha256")
    return parser


def main() -> None:
    parser = build_parser()
    arguments = parser.parse_args()
    try:
        if arguments.command == "run":
            report = run_benchmark(arguments)
            output = {
                "status": "complete",
                "output": str(arguments.output.expanduser().resolve()),
                "question_count": len(report["raw_results"]),
                "modes": report["configuration"]["modes"],
                "qa_status": "not_evaluated",
            }
        elif arguments.command == "validate":
            output = validate_report(arguments.report.expanduser().resolve())
        elif arguments.command == "preflight":
            output = preflight_official_dataset(arguments.dataset)
        elif arguments.command == "download":
            output = download_official_dataset(arguments)
        else:
            parser.error(f"unsupported command {arguments.command}")
            return
    except (AdapterError, OSError, subprocess.SubprocessError) as error:
        raise SystemExit(f"longmemeval adapter failed: {error}") from error
    print(json.dumps(output, indent=2))


if __name__ == "__main__":
    main()
