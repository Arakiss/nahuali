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
