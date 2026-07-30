# LongMemEval v1 retrieval adapter

This directory contains a first-party adapter that measures Nahuali's session
retrieval on the cleaned LongMemEval-S dataset. It is deliberately narrower
than the complete LongMemEval task: it does not run a reader model, grade an
answer, or produce a LongMemEval QA score.

The implementation follows the official LongMemEval v1 dataset shape and the
published retrieval metric definitions:

- [Official repository and dataset format](https://github.com/xiaowu0162/LongMemEval/tree/9e0b455f4ef0e2ab8f2e582289761153549043fc)
- [Official cleaned dataset](https://huggingface.co/datasets/xiaowu0162/longmemeval-cleaned)
- [Official retrieval evaluator](https://github.com/xiaowu0162/LongMemEval/blob/9e0b455f4ef0e2ab8f2e582289761153549043fc/src/retrieval/eval_utils.py)
- [Official aggregate reporting](https://github.com/xiaowu0162/LongMemEval/blob/9e0b455f4ef0e2ab8f2e582289761153549043fc/src/evaluation/print_retrieval_metrics.py)

The adapter uses only the Python standard library and a shipped `nahuali`
binary. Semantic modes use the normal local Qdrant service; no hosted model or
API credential is required.

## Reproducible dataset download

The official corpus is not vendored. The download command requires a cache
directory supplied by the operator and pins the current cleaned LongMemEval-S
artifact by repository revision, byte size, and SHA-256:

```bash
python3 benchmarks/longmemeval/adapter.py download \
  --cache-dir "$HOME/.cache/nahuali/longmemeval"
```

Pinned dataset identity:

- revision: `98d7416c24c778c2fee6e6f3006e7a073259d48f`
- file: `longmemeval_s_cleaned.json`
- size: `277383467` bytes
- SHA-256: `d6f21ea9d60a0d56f34a05b609c79c88a451d2ae03597821ea3d5a9678c3a442`

An alternate exact revision can be selected, but it must be accompanied by an
operator-supplied `--expected-sha256`. The adapter never downloads into the
repository unless that location is explicitly chosen as the cache.

## Run

Build Nahuali and make the local Qdrant service available:

```bash
cargo build --locked --release -p nahuali-cli
scripts/ensure-dev-stack.sh
```

Then run the complete cleaned dataset:

```bash
python3 benchmarks/longmemeval/adapter.py run \
  --dataset "$HOME/.cache/nahuali/longmemeval/98d7416c24c778c2fee6e6f3006e7a073259d48f/longmemeval_s_cleaned.json" \
  --dataset-version LongMemEval-v1-cleaned-S \
  --dataset-revision 98d7416c24c778c2fee6e6f3006e7a073259d48f \
  --binary target/release/nahuali \
  --source-revision "$(git rev-parse HEAD)" \
  --measured-runs 1 \
  --output /tmp/nahuali-longmemeval.json \
  --raw-output /tmp/nahuali-longmemeval.ndjson

python3 benchmarks/longmemeval/adapter.py validate \
  /tmp/nahuali-longmemeval.json
```

The default run evaluates two modes:

- `lexical`: Nahuali's local deterministic lexical recall.
- `deterministic-hybrid`: Nahuali's hybrid recall with its deterministic
  diagnostic embedding provider and local Qdrant.

If `NAHUALI_LOCAL_EMBEDDING_MODEL_PATH` is set, the adapter also evaluates
`local-model-hybrid`. It does not download a model or silently substitute a
hosted provider.

`--limit N` is useful for development, but a partial run must not be presented
as a result for the 500-question dataset.

## Isolation and provenance

Every question is imported into a unique database and a unique `custom` scope.
This prevents evidence from one LongMemEval question from leaking into another.
Each source session retains:

- the official session id and raw session date;
- a deterministic content checksum;
- every user and assistant turn;
- the source-local turn position and role; and
- a millisecond timestamp derived from the official date.

LongMemEval dates do not include a timezone. The adapter interprets them as UTC
for deterministic storage and retains the original date string in source
metadata and result artifacts.

The ground-truth `has_answer` marker is never imported into searchable memory.
It is evaluation data, not information the retriever should see.

## Metrics and raw output

For `k = 1, 3, 5, 10, 30, 50`, the adapter emits ranked session items and:

- `recall_any@k`
- `recall_all@k`
- `ndcg_any@k`

The NDCG implementation intentionally matches the official LongMemEval v1
evaluator rather than replacing it with a different library implementation.
Questions whose ids contain `_abs` remain in `raw_results` and optional NDJSON
output, but are excluded from retrieval aggregates because the official
evaluator gives them no target location. That exclusion is recorded per
question and counted in the report.

Each result also binds:

- the dataset SHA-256 and caller-provided version/revision;
- the Nahuali binary SHA-256, version, and source revision input;
- per-query retrieval latency and semantic indexing latency, both measured as
  wall-clock subprocess invocations including CLI startup;
- operating system, architecture, Python version, and logical CPU count without
  hostname or user identity;
- clean, dirty, or unavailable Git worktree state so an input revision is not
  mistaken for exact source identity when local changes exist;
- dataset and binary filenames plus SHA-256 digests without absolute local
  paths;
- the exact evaluated mode; and
- the runner relationship, disclosed as `first-party`.

## QA handoff

`--hypotheses-output PATH` writes the exact two-field NDJSON shape accepted by
the official LongMemEval QA evaluator:

```json
{"question_id": "...", "hypothesis": ""}
```

The hypotheses are intentionally empty. A reader must fill them before the
official evaluator is run. The adapter records `qa_status: not_evaluated` and
does not infer a QA score from retrieval metrics.

## Checked-in fixture

`fixtures/smoke.json` is synthetic and exists only to test ingestion,
isolation, ranking, abstention handling, metric validation, and output shape.
Its scores are not evidence about LongMemEval or comparative memory quality.

Run the local verification gate with:

```bash
scripts/verify-longmemeval-adapter.sh
```

## Boundaries

- This is session-level retrieval, obtained by ranking turns through Nahuali
  and keeping the first occurrence of each source session. Both user and
  assistant turns are indexed. The official flat session baseline concatenates
  user turns instead, so the two runs do not share an identical retrieval unit.
- The official baseline implementation and this adapter do not share an index
  implementation. Results must identify the adapter and mode, not be labelled
  as an official baseline run.
- No temporal query expansion, benchmark-specific answer extraction, reader
  model, judge model, or QA grading is performed.
- `deterministic-hybrid` is a reproducibility diagnostic. Model-quality claims
  require a disclosed local model and a separate `local-model-hybrid` result.
- Dataset text is treated as data. It is stored and retrieved, never executed
  as instructions by the adapter.
