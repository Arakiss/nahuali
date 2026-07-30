# Examples

Examples must use synthetic data.

Current validated workflow:

```bash
cargo run -p nahuali-cli -- --database nahuali_demo remember "Lena owns the release notes" --tag product --mention Lena
cargo run -p nahuali-cli -- --database nahuali_demo ingest-text examples/source-note.md --kind note --title "Release notes source" --chunking paragraphs --tag product --mention Lena --dry-run --json
cargo run -p nahuali-cli -- --database nahuali_demo ingest-text examples/source-note.md --kind note --title "Release notes source" --chunking paragraphs --tag product --mention Lena --json
cargo run -p nahuali-cli -- --database nahuali_demo ingest-dir examples --recursive --extension md --extension txt --chunking paragraphs --dry-run --json
cargo run -p nahuali-cli -- --database nahuali_demo ingest examples/ingest-conversation.json --dry-run --json
cargo run -p nahuali-cli -- --database nahuali_demo ingest examples/ingest-conversation.json --json
cargo run -p nahuali-cli -- --database nahuali_demo claim Lena owns "release notes" --confidence 0.92 --source-last
cargo run -p nahuali-cli -- --database nahuali_demo link Lena owns "release notes" --confidence 0.9 --source-last
cargo run -p nahuali-cli -- --database nahuali_demo preference "Release notes" "Keep release notes concise" --source-last
cargo run -p nahuali-cli -- --database nahuali_demo intention "Ship release notes" --priority high --source-last
cargo run -p nahuali-cli -- --database nahuali_demo briefing --json
cargo run -p nahuali-cli -- --database nahuali_demo recall "Lena release"
cargo run -p nahuali-cli -- --database nahuali_demo recall "Lena release" --authority --json
cargo run -p nahuali-cli -- --database nahuali_demo inspect --json
cargo run -p nahuali-cli -- --database nahuali_demo reflect --json
cargo run -p nahuali-cli -- --database nahuali_demo validate --json
cargo run -p nahuali-cli -- --database nahuali_demo audit --json
```

This path is covered by CLI integration tests. It should produce an episode,
source provenance, entities, supported claims, links, preferences, intentions,
and a valid record ledger.

`ingest-conversation.json` is a structured source-neutral intake document. It is
not a record ledger or a snapshot. The CLI validates the whole document before
appending source, episode, claim, link, procedure, or intention records.

`source-note.md` exercises the text adapter path. It becomes source provenance
and source episodes only; derived claims, links, procedures, and intentions
remain explicit operator or adapter-supplied records.

`ingest-dir` uses the same text adapter in batch mode. The CLI validates every
discovered file before appending records, so one invalid file blocks the batch
without partial mutation.

Scripted workflows can request JSON from the same commands:

```bash
cargo run -p nahuali-cli -- --database nahuali_demo recall "Lena release" --json
cargo run -p nahuali-cli -- --database nahuali_demo briefing --json
cargo run -p nahuali-cli -- --database nahuali_demo reflect --json
```

MCP clients can run the local stdio server against the same record ledger:

```bash
cargo run -p nahuali-mcp -- --database nahuali_demo
```

The MCP stdio workflow is covered by integration tests that perform
`initialize`, `tools/list`, `tools/call`, `resources/list`, `resources/read`,
`prompts/list`, and `prompts/get` against the real binary.

Validated MCP tools include `remember`, `ingest`, `ingest_text`, `claim`,
`link`, `briefing`, `recall`, `graph`, `inspect`, `reflect`, `review`,
`review_resolve`, and `validate`.

Validated MCP resources:

```txt
nahuali://database/summary
nahuali://database/sources
nahuali://database/health
nahuali://database/entities
nahuali://database/episodes
nahuali://database/claims
nahuali://database/links
nahuali://database/facts
nahuali://database/relations
nahuali://database/procedures
nahuali://database/intentions
nahuali://database/records
```

Validated MCP prompts:

```txt
recall_with_health_check
record_evidence_backed_fact
```

## HTTP trust clients

The dependency-free Python 3 and TypeScript examples exercise the local HTTP
API as a trust contract, not as a source of factual truth. Each client records a
synthetic episode, creates an evidence-backed claim, validates store-level and
per-result trust fields, then adds an unsupported competing assertion and
checks that Nahuali returns a non-trust verdict.

Build the existing API binary and run both clients against separate disposable
databases bound only to loopback:

```bash
cargo build -p nahuali-api
bash scripts/verify-http-client-examples.sh
```

The verifier accepts `NAHUALI_API_BIN=/path/to/nahuali-api` when the binary is
outside `target/debug`. To run one client manually against a local disposable
API, set `NAHUALI_API_URL` and use either:

```bash
python3 examples/http/python_client.py
bun examples/http/typescript_client.ts
```
