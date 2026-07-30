#!/usr/bin/env python3
"""Exercise Nahuali's HTTP trust contract without third-party packages."""

from __future__ import annotations

import json
import os
import sys
import time
from typing import Any, Dict, List
from urllib.error import HTTPError, URLError
from urllib.parse import urlparse
from urllib.request import Request, urlopen


class ExampleFailure(RuntimeError):
    """Raised when the server response violates the documented trust contract."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ExampleFailure(message)


def require_object(value: Any, path: str) -> Dict[str, Any]:
    require(isinstance(value, dict), f"{path} must be an object")
    return value


def require_string_list(value: Any, path: str) -> List[str]:
    require(isinstance(value, list), f"{path} must be an array")
    require(all(isinstance(item, str) for item in value), f"{path} must contain strings")
    return value


def validate_loopback_url(base_url: str) -> str:
    parsed = urlparse(base_url)
    require(parsed.scheme in ("http", "https"), "NAHUALI_API_URL must use HTTP or HTTPS")
    require(
        parsed.hostname in ("127.0.0.1", "::1", "localhost"),
        "This example intentionally connects only to a loopback address",
    )
    require(parsed.path in ("", "/"), "NAHUALI_API_URL must not include a path")
    return base_url.rstrip("/")


def post_json(base_url: str, path: str, payload: Dict[str, Any]) -> Dict[str, Any]:
    request = Request(
        f"{base_url}{path}",
        data=json.dumps(payload).encode("utf-8"),
        headers={"content-type": "application/json"},
        method="POST",
    )
    try:
        with urlopen(request, timeout=15) as response:
            body = response.read().decode("utf-8")
    except HTTPError as error:
        body = error.read().decode("utf-8", errors="replace")
        raise ExampleFailure(f"{path} returned HTTP {error.code}: {body}") from error
    except URLError as error:
        raise ExampleFailure(f"{path} could not reach the local API: {error.reason}") from error

    try:
        return require_object(json.loads(body), path)
    except json.JSONDecodeError as error:
        raise ExampleFailure(f"{path} returned invalid JSON") from error


def validate_authority(value: Any, path: str) -> Dict[str, Any]:
    authority = require_object(value, path)
    require(isinstance(authority.get("mode"), str), f"{path}.mode must be a string")
    score = authority.get("score")
    require(
        isinstance(score, (int, float)) and not isinstance(score, bool) and 0 <= score <= 1,
        f"{path}.score must be a number in the 0..1 range",
    )
    require(isinstance(authority.get("can_trust"), bool), f"{path}.can_trust must be boolean")
    require_string_list(authority.get("reasons"), f"{path}.reasons")
    require_string_list(authority.get("signal_kinds"), f"{path}.signal_kinds")
    return authority


def validate_health(value: Any, path: str) -> Dict[str, Any]:
    health = require_object(value, path)
    signals = health.get("signals")
    require(isinstance(signals, list), f"{path}.signals must be an array")
    for index, raw_signal in enumerate(signals):
        signal_path = f"{path}.signals[{index}]"
        signal = require_object(raw_signal, signal_path)
        require(isinstance(signal.get("kind"), str), f"{signal_path}.kind must be a string")
        require_string_list(signal.get("dimensions"), f"{signal_path}.dimensions")
        require(isinstance(signal.get("severity"), str), f"{signal_path}.severity must be a string")
        require(isinstance(signal.get("message"), str), f"{signal_path}.message must be a string")
        require_string_list(signal.get("evidence_ids"), f"{signal_path}.evidence_ids")
    require_string_list(health.get("warnings"), f"{path}.warnings")
    return health


def validate_result_trust(value: Any, path: str) -> Dict[str, Any]:
    trust = require_object(value, path)
    require(isinstance(trust.get("mode"), str), f"{path}.mode must be a string")
    score = trust.get("score")
    require(
        isinstance(score, (int, float)) and not isinstance(score, bool) and 0 <= score <= 1,
        f"{path}.score must be a number in the 0..1 range",
    )
    require(isinstance(trust.get("can_trust"), bool), f"{path}.can_trust must be boolean")
    require_string_list(trust.get("reasons"), f"{path}.reasons")
    require_string_list(trust.get("signal_kinds"), f"{path}.signal_kinds")
    return trust


def validate_recall_results(recall: Dict[str, Any], path: str) -> None:
    results = recall.get("lexical_results")
    require(isinstance(results, list), f"{path}.lexical_results must be an array")
    for index, raw_result in enumerate(results):
        result_path = f"{path}.lexical_results[{index}]"
        result = require_object(raw_result, result_path)
        require(isinstance(result.get("id"), str), f"{result_path}.id must be a string")
        require(
            result.get("evidence_id") is None
            or isinstance(result.get("evidence_id"), str),
            f"{result_path}.evidence_id must be a string or null",
        )
        validate_result_trust(result.get("trust"), f"{result_path}.trust")


def find_result(recall: Dict[str, Any], result_id: str) -> Dict[str, Any]:
    results = recall.get("lexical_results")
    require(isinstance(results, list), "recall.lexical_results must be an array")
    for raw_result in results:
        result = require_object(raw_result, "recall.lexical_results[]")
        if result.get("id") == result_id:
            return result
    raise ExampleFailure(f"recall did not return expected result {result_id}")


def main() -> None:
    base_url = validate_loopback_url(
        os.environ.get("NAHUALI_API_URL", "http://127.0.0.1:7070")
    )
    run_id = os.environ.get(
        "NAHUALI_EXAMPLE_RUN_ID", f"python_{os.getpid()}_{time.time_ns()}"
    )
    subject = f"Python HTTP example {run_id}"
    predicate = "rollout mode"

    episode = post_json(
        base_url,
        "/v1/episode",
        {
            "content": f"Synthetic observation for {subject}: rollout mode is assisted.",
            "tags": ["http-example", "synthetic"],
        },
    )
    episode_id = episode.get("id")
    require(isinstance(episode_id, str) and episode_id, "episode.id must be a string")

    supported_claim = post_json(
        base_url,
        "/v1/claim",
        {
            "subject": subject,
            "predicate": predicate,
            "object": "assisted",
            "source_episode_id": episode_id,
            "confidence": 0.92,
        },
    )
    supported_claim_id = supported_claim.get("id")
    require(
        isinstance(supported_claim_id, str) and supported_claim_id,
        "supported claim.id must be a string",
    )

    supported_recall = post_json(
        base_url,
        "/v1/recall",
        {
            "query": f"{subject} rollout mode assisted",
            "limit": 10,
            "require_evidence": True,
        },
    )
    supported_authority = validate_authority(
        supported_recall.get("authority"), "supported_recall.authority"
    )
    validate_health(supported_recall.get("health"), "supported_recall.health")
    validate_recall_results(supported_recall, "supported_recall")
    require(
        supported_authority["mode"] == "certify"
        and supported_authority["can_trust"] is True,
        "the clean, evidence-backed store should certify",
    )
    supported_result = find_result(supported_recall, supported_claim_id)
    require(
        supported_result.get("evidence_id") == episode_id,
        "the supported result must cite its source episode",
    )
    supported_trust = validate_result_trust(
        supported_result.get("trust"), "supported_result.trust"
    )
    require(
        supported_trust["mode"] == "certify" and supported_trust["can_trust"] is True,
        "the evidence-backed result should certify",
    )

    unsupported_claim = post_json(
        base_url,
        "/v1/claim",
        {
            "subject": subject,
            "predicate": predicate,
            "object": "manual",
            "confidence": 0.91,
        },
    )
    unsupported_claim_id = unsupported_claim.get("id")
    require(
        isinstance(unsupported_claim_id, str) and unsupported_claim_id,
        "unsupported claim.id must be a string",
    )

    guarded_recall = post_json(
        base_url,
        "/v1/recall",
        {
            "query": f"{subject} rollout mode manual",
            "limit": 10,
        },
    )
    guarded_authority = validate_authority(
        guarded_recall.get("authority"), "guarded_recall.authority"
    )
    guarded_health = validate_health(guarded_recall.get("health"), "guarded_recall.health")
    validate_recall_results(guarded_recall, "guarded_recall")
    require(
        guarded_authority["can_trust"] is False
        and guarded_authority["mode"] != "certify",
        "an unsupported competing assertion must prevent store certification",
    )
    health_signal_kinds = {
        require_object(signal, "guarded_recall.health.signals[]").get("kind")
        for signal in guarded_health["signals"]
    }
    require(
        "unsupported_fact" in health_signal_kinds,
        "guarded health must identify the unsupported assertion",
    )
    require(
        "conflicting_fact" in health_signal_kinds,
        "guarded health must identify the competing values",
    )

    unsupported_result = find_result(guarded_recall, unsupported_claim_id)
    require(
        unsupported_result.get("evidence_id") is None,
        "the unsupported result must not invent an evidence identifier",
    )
    unsupported_trust = validate_result_trust(
        unsupported_result.get("trust"), "unsupported_result.trust"
    )
    require(
        unsupported_trust["can_trust"] is False
        and unsupported_trust["mode"] != "certify",
        "the unsupported competing result must carry a non-trust verdict",
    )

    print(
        json.dumps(
            {
                "client": "python",
                "evidence_backed_result": {
                    "store_mode": supported_authority["mode"],
                    "result_mode": supported_trust["mode"],
                    "evidence_id_present": True,
                },
                "synthetic_competing_assertion": {
                    "store_mode": guarded_authority["mode"],
                    "store_can_trust": guarded_authority["can_trust"],
                    "result_mode": unsupported_trust["mode"],
                    "result_can_trust": unsupported_trust["can_trust"],
                    "signal_kinds": unsupported_trust["signal_kinds"],
                },
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    try:
        main()
    except ExampleFailure as error:
        print(f"HTTP trust example failed: {error}", file=sys.stderr)
        raise SystemExit(1) from error
