import importlib.util
import json
import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[3]
VERIFIER_PATH = ROOT / "scripts/verify-benchmark-artifact-identity.py"
PUBLISHED_BETA6 = (
    ROOT / "benchmarks/agent-memory-retrieval/results/nahuali-0.8.0-beta.6.json"
)


def load_verifier():
    spec = importlib.util.spec_from_file_location("benchmark_artifact_identity", VERIFIER_PATH)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


class ArtifactIdentityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.verifier = load_verifier()

    def actual(self):
        return {
            "artifactName": "nahuali",
            "binarySha256": "a" * 64,
            "sourceRevision": "b" * 40,
            "headRevision": "b" * 40,
            "systemVersion": "nahuali 0.8.0-beta.6",
            "releaseTag": "v0.8.0-beta.6",
            "releaseAsset": "nahuali-v0.8.0-beta.6-x86_64-unknown-linux-gnu.tar.gz",
            "target": "x86_64-unknown-linux-gnu",
            "archiveSha256": "c" * 64,
        }

    def result(self):
        actual = self.actual()
        return {
            "system": {"name": "Nahuali", "version": actual["systemVersion"]},
            "artifact": {
                "name": actual["artifactName"],
                "sha256": actual["binarySha256"],
                "sourceRevision": actual["sourceRevision"],
                "kind": "published-release",
                "releaseTag": actual["releaseTag"],
                "releaseAsset": actual["releaseAsset"],
                "target": actual["target"],
                "archiveSha256": actual["archiveSha256"],
            },
        }

    def test_accepts_exact_published_release_identity(self):
        self.verifier.validate_document(self.result(), self.actual())

    def test_rejects_source_build_as_published_release(self):
        result = self.result()
        result["artifact"]["kind"] = "source-build"
        with self.assertRaisesRegex(self.verifier.IdentityError, "published-release"):
            self.verifier.validate_document(result, self.actual())

    def test_rejects_wrong_tag_revision(self):
        result = self.result()
        result["artifact"]["sourceRevision"] = "d" * 40
        with self.assertRaisesRegex(self.verifier.IdentityError, "release tag"):
            self.verifier.validate_document(result, self.actual())

    def test_current_beta6_result_fails_against_exact_release_binary(self):
        result = json.loads(PUBLISHED_BETA6.read_text(encoding="utf-8"))
        actual = {
            "artifactName": "nahuali",
            "binarySha256": "46134789d82f90a56663526800f3b45a309a65c68d9d175b209146d5b3524147",
            "sourceRevision": "4ef05ddc3f0d524fa3ed2185e129eabf34e5a0e2",
            "headRevision": "4ef05ddc3f0d524fa3ed2185e129eabf34e5a0e2",
            "systemVersion": "nahuali 0.8.0-beta.6",
            "releaseTag": "v0.8.0-beta.6",
            "releaseAsset": "nahuali-v0.8.0-beta.6-aarch64-apple-darwin.tar.gz",
            "target": "aarch64-apple-darwin",
            "archiveSha256": "0f7781259ad02e75f4c2fbf86548f9b2f7550e8fe11ce68835b9d92e65fe9805",
        }
        with self.assertRaisesRegex(
            self.verifier.IdentityError,
            "published benchmark artifact SHA does not match the release binary",
        ):
            self.verifier.validate_document(result, actual)


if __name__ == "__main__":
    unittest.main()
