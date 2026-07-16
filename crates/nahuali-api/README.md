# nahuali-api

`nahuali-api` exposes a small HTTP v1 surface over `nahuali-core`.

The API does not own a separate memory model. Mutating endpoints append records
through the core engine, SurrealDB graph tables remain a derived projection, and
Qdrant remains a rebuildable semantic index.

Run locally:

```bash
cargo run -p nahuali-api -- --database memory --listen 127.0.0.1:7070
```

The default build writes hash-chained records. Use `--no-default-features` only
when you intentionally need the legacy unchained record format.

The API is a local beta surface. It has no authentication, accounts, tenants,
hosted operations, sync, or dashboard layer. Scopes in request bodies are memory
labels, not permission boundaries.

Health check and OpenAPI contract:

```bash
curl http://127.0.0.1:7070/v1/health
curl http://127.0.0.1:7070/v1/ready
curl http://127.0.0.1:7070/v1/status
curl http://127.0.0.1:7070/v1/openapi.json
```

`GET /health` and `GET /v1/health` return `{"status":"ok"}` without opening the
database, so they are safe as liveness probes. Transport-level failures (unknown
route, wrong `Content-Type`, malformed JSON, unknown or missing fields) return the
same structured envelope as core errors: `{"error":{"code":"...","message":"..."}}`.

`GET /ready` and `GET /v1/ready` open and refresh the ledger, validate the graph
projection, and return `503` unless those serving dependencies are current. Start
the server with `--require-semantic` to make Qdrant freshness part of readiness;
without that flag the response reports the semantic check as `not_required`.
Readiness responses contain only state and counts, never memory content.

The server handles Ctrl+C and SIGTERM with graceful shutdown. Request timing is
written to stderr with method, path, status, and duration only; query strings and
request or response bodies are never logged.

Core endpoint groups:

- writes: `POST /v1/episode`, `/v1/claim`, `/v1/link`, `/v1/procedure`,
  `/v1/intention`, `/v1/intention/update`, `/v1/intention/status`, and
  `/v1/anomaly/acknowledge`
- recall and context: `POST /v1/recall`, `/v1/session-resume`,
  `/v1/memory-health`, and `GET /v1/graph`, `/v1/audit`, `/v1/trust-report`
- operator reports: `POST /v1/intention/reconcile`, `/v1/proactive`,
  `/v1/deadlines`, `/v1/anomalies`, `/v1/review/resolve`, and
  `GET /v1/goal-progress`, `/v1/timeline`, `/v1/pending`
- derived-tier maintenance: `GET /v1/projection/status`,
  `POST /v1/projection/rebuild`, `POST /v1/projection/validate`,
  `GET /v1/semantic/status`, and `POST /v1/semantic/rebuild`

Recall example:

```bash
curl -X POST http://127.0.0.1:7070/v1/recall \
  -H 'content-type: application/json' \
  -d '{"query":"Lena release","limit":10,"semantic":true}'
```

Request bodies reject unknown fields (matching `additionalProperties: false` in the
OpenAPI contract), so a typo such as `"authority":true` returns a `400` with code
`validation_error` rather than being silently ignored.

## Optional build features

`tamper-evidence` is on by default to match the CLI and published binary
artifacts. Build with `--no-default-features` only when you intentionally need
the legacy unchained record format.

- `--features tamper-evidence`: recorded events are chained by hash, so ledger
  replay on open detects an in-place rewrite of any historical record.
- `--features local-embeddings`: `POST /v1/semantic/rebuild` and semantic recall
  use a static model2vec model instead of the deterministic embedder. Set
  `NAHUALI_EMBEDDING_PROVIDER=model2vec` and point
  `NAHUALI_LOCAL_EMBEDDING_MODEL_PATH` at a local model directory.

Chain-tip attestation (signing) is a CLI/operator action; the API exposes no
signing endpoint.
