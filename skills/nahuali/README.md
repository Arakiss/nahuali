# Give your agent evidence-aware memory

Nahuali records observations and derived memory with their available evidence.
Recall returns relevance and a separate deterministic trust decision. The
engine reports unsupported claims, contradictions, stale facts, and blind spots,
while its default append-only, hash-chained history supports local integrity
checks and comparison with an operator-held authorized checkpoint.

## Wire it up (one MCP server)

Nahuali ships an MCP stdio server, `nahuali-mcp`. Install it from source:

```bash
cargo install --path crates/nahuali-mcp --locked
```

Register it with your MCP client. The server name the agent references is
`nahuali`:

```json
{
  "mcpServers": {
    "nahuali": {
      "command": "nahuali-mcp",
      "args": ["--database", "my_project"]
    }
  }
}
```

The database value is a SurrealDB identifier, not a filesystem path. Use a
stable identifier such as `my_project` or `personal_memory`. Full server details
are in [`crates/nahuali-mcp/README.md`](../../crates/nahuali-mcp/README.md).

## Teach the agent the protocol

Wiring the server is not enough — the agent has to use it correctly. Two files
do that:

- **[`protocol.md`](protocol.md)** — the cross-harness protocol. Harness-agnostic
  rules for any agent system (Claude Code, Codex, Cursor, an MCP-aware harness,
  or your own loop): start by recalling, record observations as episodes, assert
  claims and links only with evidence, read the per-result trust decision (not
  just the score), check health before trusting at scale, never silently
  rewrite memory. If your harness reads a different convention (a system prompt,
  a skill, a hook), copy the protocol into that surface.

- **[`SKILL.md`](SKILL.md)** — the Claude Code
  skill. Same substance, packaged so Claude Code loads it automatically when a
  task needs persistent context, recall before answering, or a trust check
  before acting.

## What the agent should do, in one paragraph

Start every session with `briefing`, then `recall` specifics — never assume
context that can be queried. Capture decisions, facts, and events with
`remember` as episodes; those episodes are the evidence everything else cites.
Assert `claim`s and `link`s only when an episode supports them. On every recall
result, read the trust mode (`Certify` / `Advisory` / `Warn` / `Block`), not
just the relevance score: `Certify` is evidence-backed, but must be cited without
strengthening what its source actually says, `Advisory` is a
lead, `Warn` / `Block` should not be acted through. Before leaning on a lot of
recalled memory, check health with `inspect` or `trust_report`. And never edit
history silently — repairs go through `review_resolve` and stay on the ledger.

## Checking recorded history

`trust_report` gives one composed view of stored memory, authority, health, and
the integrity checks represented in the report. `audit` returns a diff between
retained ledger points and checks checksums, sequence contiguity, and the default
hash chain. A rewritten non-tip record is detected when a later stored link no
longer matches. Last-event replacement, truncation, rollback, or a fully
re-chained history require comparison with an authorized checkpoint retained
outside the store. Checkpoint signing is a CLI/operator action; the agent reads
and audits integrity, it does not sign.
