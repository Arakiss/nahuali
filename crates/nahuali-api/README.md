# nahuali-api

`nahuali-api` exposes a small HTTP v1 surface over `nahuali-core`.

The API does not own a separate memory model. Mutating endpoints append records
through the core engine, SurrealDB graph tables remain a derived projection, and
Qdrant remains a rebuildable semantic index.

Run locally:

```bash
cargo run -p nahuali-api -- --database memory --listen 127.0.0.1:7070
```

Health check:

```bash
curl http://127.0.0.1:7070/v1/status
```
