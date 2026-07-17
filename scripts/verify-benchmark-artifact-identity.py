#!/usr/bin/env python3
"""Bind a published benchmark result to an exact release artifact and tag."""

import argparse
import hashlib
import json
import pathlib
import re
import subprocess
from typing import Any


LOWER_SHA256 = re.compile(r"^[0-9a-f]{64}$")
LOWER_SOURCE_REVISION = re.compile(r"^[0-9a-f]{40}$")


class IdentityError(ValueError):
    """Raised when benchmark and release identities disagree."""


def sha256_file(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def git_output(repo_root: pathlib.Path, *args: str) -> str:
    return subprocess.run(
        ["git", "-C", str(repo_root), *args],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()


def inspect_actual_identity(
    binary: pathlib.Path,
    tag: str,
    asset: pathlib.Path,
    target: str,
    repo_root: pathlib.Path,
    require_head: bool = False,
) -> dict[str, str]:
    binary = binary.resolve()
    asset = asset.resolve()
    repo_root = repo_root.resolve()
    if not binary.is_file():
        raise IdentityError(f"release binary does not exist: {binary}")
    if not asset.is_file():
        raise IdentityError(f"release asset does not exist: {asset}")
    if not tag.startswith("v") or len(tag) == 1:
        raise IdentityError(f"release tag must start with v: {tag}")
    expected_asset = f"nahuali-{tag}-{target}.tar.gz"
    if asset.name != expected_asset:
        raise IdentityError(
            f"release asset {asset.name} does not match tag and target; expected {expected_asset}"
        )

    tag_revision = git_output(repo_root, "rev-list", "-n", "1", tag)
    if not LOWER_SOURCE_REVISION.fullmatch(tag_revision):
        raise IdentityError(f"tag {tag} did not resolve to a lowercase commit SHA")
    head_revision = git_output(repo_root, "rev-parse", "HEAD")
    if require_head and head_revision != tag_revision:
        raise IdentityError(
            f"checkout HEAD {head_revision} does not match release tag {tag} at {tag_revision}"
        )

    version = subprocess.run(
        [str(binary), "--version"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    return {
        "artifactName": binary.name,
        "binarySha256": sha256_file(binary),
        "sourceRevision": tag_revision,
        "headRevision": head_revision,
        "systemVersion": version,
        "releaseTag": tag,
        "releaseAsset": asset.name,
        "target": target,
        "archiveSha256": sha256_file(asset),
    }


def validate_document(result: dict[str, Any], actual: dict[str, str]) -> None:
    artifact = result.get("artifact", {})
    observed_binary_sha = artifact.get("sha256", "")
    if not LOWER_SHA256.fullmatch(observed_binary_sha):
        raise IdentityError("benchmark artifact SHA-256 is not lowercase hexadecimal")
    if observed_binary_sha != actual["binarySha256"]:
        raise IdentityError("published benchmark artifact SHA does not match the release binary")

    observed_revision = artifact.get("sourceRevision", "")
    if not LOWER_SOURCE_REVISION.fullmatch(observed_revision):
        raise IdentityError("benchmark source revision is not lowercase hexadecimal")
    if observed_revision != actual["sourceRevision"]:
        raise IdentityError("published benchmark source revision does not match the release tag")

    expected_version = f"nahuali {actual['releaseTag'][1:]}"
    if actual["systemVersion"] != expected_version:
        raise IdentityError("release binary version does not match the release tag")
    if result.get("system", {}).get("version") != actual["systemVersion"]:
        raise IdentityError("benchmark system version does not match the release binary")
    if artifact.get("name") != actual["artifactName"]:
        raise IdentityError("benchmark artifact name does not match the release binary")
    if artifact.get("kind") != "published-release":
        raise IdentityError("published benchmark must declare artifact.kind as published-release")

    comparisons = {
        "releaseTag": "release tag",
        "releaseAsset": "release asset",
        "target": "release target",
        "archiveSha256": "release archive SHA-256",
    }
    for field, label in comparisons.items():
        if artifact.get(field) != actual[field]:
            raise IdentityError(f"published benchmark {label} does not match the release artifact")
    if not LOWER_SHA256.fullmatch(artifact.get("archiveSha256", "")):
        raise IdentityError("benchmark release archive SHA-256 is not lowercase hexadecimal")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--result", type=pathlib.Path, required=True)
    parser.add_argument("--binary", type=pathlib.Path, required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--asset", type=pathlib.Path, required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--repo-root", type=pathlib.Path, default=pathlib.Path.cwd())
    parser.add_argument("--require-head", action="store_true")
    arguments = parser.parse_args()

    try:
        result = json.loads(arguments.result.read_text(encoding="utf-8"))
        actual = inspect_actual_identity(
            arguments.binary,
            arguments.tag,
            arguments.asset,
            arguments.target,
            arguments.repo_root,
            arguments.require_head,
        )
        validate_document(result, actual)
    except (IdentityError, json.JSONDecodeError, OSError, subprocess.CalledProcessError) as error:
        raise SystemExit(f"benchmark artifact identity invalid: {error}") from error

    print(
        json.dumps(
            {
                "status": "pass",
                "releaseTag": actual["releaseTag"],
                "releaseAsset": actual["releaseAsset"],
                "target": actual["target"],
                "binarySha256": actual["binarySha256"],
                "sourceRevision": actual["sourceRevision"],
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
