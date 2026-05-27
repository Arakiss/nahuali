# Examples

Examples must use synthetic data.

Current validated workflow:

```bash
cargo run -p nahuali-cli -- --database .nahuali-demo remember "Lena owns the release notes" --tag product --mention Lena
cargo run -p nahuali-cli -- --database .nahuali-demo ingest-text examples/source-note.md --kind note --title "Release notes source" --chunking paragraphs --tag product --mention Lena --dry-run --json
cargo run -p nahuali-cli -- --database .nahuali-demo ingest-text examples/source-note.md --kind note --title "Release notes source" --chunking paragraphs --tag product --mention Lena --json
cargo run -p nahuali-cli -- --database .nahuali-demo ingest-dir examples --recursive --extension md --extension txt --chunking paragraphs --dry-run --json
cargo run -p nahuali-cli -- --database .nahuali-demo ingest examples/ingest-conversation.json --dry-run --json
cargo run -p nahuali-cli -- --database .nahuali-demo ingest examples/ingest-conversation.json --json
cargo run -p nahuali-cli -- --database .nahuali-demo claim Lena owns "release notes" --confidence 0.92 --source-last
cargo run -p nahuali-cli -- --database .nahuali-demo link Lena owns "release notes" --confidence 0.9 --source-last
cargo run -p nahuali-cli -- --database .nahuali-demo preference "Release notes" "Keep release notes concise" --source-last
cargo run -p nahuali-cli -- --database .nahuali-demo intention "Ship release notes" --priority high --source-last
cargo run -p nahuali-cli -- --database .nahuali-demo briefing --json
cargo run -p nahuali-cli -- --database .nahuali-demo recall "Lena release"
cargo run -p nahuali-cli -- --database .nahuali-demo recall "Lena release" --authority --json
cargo run -p nahuali-cli -- --database .nahuali-demo inspect --json
cargo run -p nahuali-cli -- --database .nahuali-demo reflect --json
cargo run -p nahuali-cli -- --database .nahuali-demo validate --json
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
cargo run -p nahuali-cli -- --database .nahuali-demo recall "Lena release" --json
cargo run -p nahuali-cli -- --database .nahuali-demo briefing --json
cargo run -p nahuali-cli -- --database .nahuali-demo reflect --json
```

MCP clients can run the local stdio server against the same record ledger:

```bash
cargo run -p nahuali-mcp -- --database .nahuali-demo
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

Planned examples:

- inspection-gated recall
