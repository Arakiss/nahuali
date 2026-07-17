# Agent Memory Trust Benchmark

The Agent Memory Trust Benchmark is a vendor-neutral contract for testing what
happens after a memory system retrieves something relevant. It does not measure
answer quality or retrieval accuracy. It measures whether a caller can inspect
the basis for trust and whether the system refuses unsafe memory.

The benchmark is intentionally small and adapter-based. Any memory product can
emit the result format without adopting Nahuali's data model or vocabulary.

## What it measures

| Capability | Question |
|---|---|
| Evidence traceability | Can the caller reach the observation behind a claim? |
| Unsupported-memory abstention | Does the system avoid presenting an unsupported claim as trusted? |
| Contradiction detection | Does conflicting memory stop or qualify recall? |
| Staleness signaling | Can old evidence reduce trust without deleting history? |
| Non-mutating inspection | Can the store identify defects without silently rewriting memory? |
| In-place tamper detection | Is a rewritten historical record detected after its local checksum is recomputed? |
| Full re-chain detection | Does an external checkpoint detect a rewritten and re-chained suffix? |

These capabilities are scored separately. A product may legitimately mark a
case `unsupported`; the report must not turn an absent control into a pass.

## Adapter contract

An adapter runs the cases in [`cases.json`](cases.json) and writes one result
per case:

```json
{
  "benchmarkVersion": "1.0.0",
  "system": { "name": "Example Memory", "version": "2.3.1" },
  "commit": "immutable source revision or image digest",
  "runner": {
    "relationship": "first-party",
    "adapter": "adapters/example.py"
  },
  "environment": {
    "services": [],
    "models": [],
    "operatorActions": []
  },
  "cases": [
    {
      "id": "unsupported-memory-abstention",
      "status": "pass",
      "verdict": "warn",
      "evidenceIds": [],
      "detected": true,
      "mutated": false,
      "notes": "Short factual explanation"
    }
  ]
}
```

Allowed statuses are `pass`, `fail`, and `unsupported`. Verdict names are not
standardized: adapters map their native output to `trusted`, `qualified`, or
`refused` in the optional `normalizedVerdict` field and preserve the native
value in `verdict`.

## Score a result

The scorer uses only the Python standard library:

```bash
python3 benchmarks/agent-memory-trust/score.py path/to/result.json
```

It prints machine-readable JSON with per-capability outcomes and the counts of
passes, failures, and unsupported cases. It deliberately does not collapse the
report into a single marketing score.

Run the Nahuali adapter against an installed or locally built CLI:

```bash
python3 benchmarks/agent-memory-trust/adapters/nahuali.py \
  --binary target/release/nahuali \
  --source-revision "$(git rev-parse HEAD)" \
  --output result.json
python3 benchmarks/agent-memory-trust/score.py result.json
```

The adapter uses only documented CLI commands and disposable embedded stores.

## Checked-in results

| System | Version | Runner | Artifact | Result |
|---|---|---|---|---|
| Nahuali | 0.8.0-beta.7 | First-party | Source build | [7 pass, 0 fail, 0 unsupported](results/nahuali-0.8.0-beta.7.json) |

The checked-in file records the tested binary SHA-256, exact source revision,
adapter path, environment, native verdicts, and complete per-case output. It is
a version-matched source build, not a claim about any published release archive.
Reproduce a source-build result with:

```bash
cargo build --release -p nahuali-cli
python3 benchmarks/agent-memory-trust/adapters/nahuali.py \
  --binary target/release/nahuali \
  --source-revision "$(git rev-parse HEAD)" \
  --output benchmarks/agent-memory-trust/results/nahuali-0.8.0-beta.7.json
python3 benchmarks/agent-memory-trust/score.py \
  benchmarks/agent-memory-trust/results/nahuali-0.8.0-beta.7.json
```

To label a result `published-release`, the adapter additionally requires the
release tag, archive name, target, and archive SHA-256. Validate that document
against the actual archive and extracted binary before publishing it:

```bash
python3 benchmarks/agent-memory-trust/adapters/nahuali.py \
  --binary /path/to/extracted/nahuali \
  --source-revision "$TAG_REVISION" \
  --release-tag vX.Y.Z-beta.N \
  --release-asset nahuali-vX.Y.Z-beta.N-TARGET.tar.gz \
  --target TARGET \
  --archive-sha256 "$ARCHIVE_SHA256" \
  --output result.json

python3 scripts/verify-benchmark-artifact-identity.py \
  --result result.json \
  --binary /path/to/extracted/nahuali \
  --tag vX.Y.Z-beta.N \
  --asset /path/to/nahuali-vX.Y.Z-beta.N-TARGET.tar.gz \
  --target TARGET
```

## Comparison rules

Every shared result must include:

1. the unmodified case file and benchmark version;
2. the product version plus an immutable commit or image digest;
3. the adapter source and exact command;
4. complete output, including failures and unsupported cases;
5. any external service, model, or operator action used;
6. a statement that the benchmark is first-party unless an independent party
   ran it.

A result may use `artifact.kind: source-build` or `published-release`. Only the
latter may be described as evidence for a release archive, and it must pass the
artifact identity verifier above.

Adapters must exercise public product behavior. They may not inspect private
database tables, patch product code, or infer a pass from documentation.

## Limits

The corpus is synthetic and covers known failure classes. A pass does not prove
that remembered content is true, prevent every poisoning attack, or replace
LOCOMO, LongMemEval, or other recall-quality benchmarks. The value of this suite
is reproducible comparison on a missing axis: whether retrieved memory exposes
enough evidence and control to be trusted.
