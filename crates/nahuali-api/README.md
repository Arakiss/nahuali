# nahuali-api

`nahuali-api` exposes a small HTTP v1 surface over `nahuali-core`.

The API does not own a separate memory model. Mutating endpoints append records
through the core engine, SurrealDB graph tables remain a derived projection, and
Qdrant remains a rebuildable semantic index.

Run locally:

```bash
cargo run -p nahuali-api -- --database memory --listen 127.0.0.1:7070
```

The API is a local beta surface. It has no authentication, accounts, tenants,
hosted operations, sync, or dashboard layer. Scopes in request bodies are memory
labels, not permission boundaries.

Health check and OpenAPI contract:

```bash
curl http://127.0.0.1:7070/v1/status
curl http://127.0.0.1:7070/v1/openapi.json
```

Core endpoint groups:

- writes: `POST /v1/episode`, `/v1/claim`, `/v1/link`, `/v1/procedure`,
  `/v1/intention`, `/v1/intention/update`, `/v1/intention/status`, and
  `/v1/anomaly/acknowledge`
- recall and context: `POST /v1/recall`, `/v1/session-resume`,
  `/v1/memory-health`, and `GET /v1/graph`
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
  -d '{"query":"Lena release","limit":10,"authority":true}'
```
