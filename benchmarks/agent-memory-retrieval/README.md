# Agent Memory Retrieval Benchmark

This first-party benchmark measures retrieval, not answer generation. It runs
the public `nahuali` CLI against a versioned 24-memory, 12-query corpus and
reports macro Recall@1/3/5, MRR, nDCG@10, and end-to-end CLI latency.

The required modes are lexical recall and hybrid recall with Nahuali's built-in
deterministic embedder. A local model result is measured only when
`NAHUALI_LOCAL_EMBEDDING_MODEL_PATH` is configured; otherwise the result records
`not_configured` with a reason. The scorer rejects missing required modes,
unexplained optional modes, incomplete queries, altered metrics, corpus drift,
and results that do not identify the tested binary and source revision.

## Checked-in baseline

The current version-matched source-build result is checked in as
[`nahuali-0.8.0-beta.6.json`](results/nahuali-0.8.0-beta.6.json).

| Mode | Recall@1 | Recall@3 | MRR | nDCG@10 | Median latency | p95 latency |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Lexical | 1.000 | 1.000 | 1.000 | 1.000 | 33.8 ms | 55.0 ms |
| Deterministic hybrid | 1.000 | 1.000 | 1.000 | 1.000 | 39.0 ms | 40.9 ms |
| Optional local model | not configured | — | — | — | — | — |

This is a deliberately small regression corpus, so a perfect score is a gate,
not a state-of-the-art claim. The result is useful because every ranked item,
latency sample, corpus digest, binary digest, and source revision is inspectable.
It is not described as a published release artifact: that stronger identity
requires a release tag, archive name, target, archive digest, and exact-binary
verification.

Run the complete gate:

```bash
bash scripts/verify-retrieval-benchmark.sh
```

Or run the adapter and scorer directly:

```bash
python3 benchmarks/agent-memory-retrieval/adapters/nahuali.py \
  --binary target/release/nahuali \
  --source-revision "$(git rev-parse HEAD)" \
  --output /tmp/nahuali-retrieval.json
python3 benchmarks/agent-memory-retrieval/score.py /tmp/nahuali-retrieval.json
```

For a result produced from an extracted release archive, pass all four release
identity fields to the adapter and then verify the result against the archive:

```bash
python3 benchmarks/agent-memory-retrieval/adapters/nahuali.py \
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

## Interpretation boundary

These numbers are not LoCoMo or LongMemEval scores. Those suites evaluate an
end-to-end conversational system, including answer generation, temporal or
multi-session reasoning, abstention, and model-based grading. This benchmark is
the smaller reproducible layer Nahuali can certify today: whether the expected
memory reaches the ranked retrieval set through the shipped CLI.

Official references:

- [LoCoMo data and code](https://github.com/snap-research/locomo)
- [LongMemEval](https://github.com/xiaowu0162/LongMemEval)
- [LongMemEval-V2](https://github.com/xiaowu0162/LongMemEval-V2)
