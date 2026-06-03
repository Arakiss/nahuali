# Give your agent governed memory

Most agent memory stores more text and hopes recall surfaces the right thing.
The failure is silent: the agent recalls something, treats it as true, and acts
on it — with no way to tell an observed fact from a guess, a fresh fact from a
stale one, or an intact history from one that was edited after the fact.

Nahuali is a governance layer over memory. Every recall comes back with the
evidence behind it and a trust decision. The engine inspects its own health
(unsupported claims, contradictions, stale facts, blind spots) before the agent
acts, and its history is an append-only ledger you can audit — and, with the
optional build, prove was not tampered with.

The shift is from **recall-more** to **trust-what-you-recall**. An agent that
recalls a confident wrong answer is worse than one with no memory. Nahuali
exists so the agent can tell the difference, and say so.

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
      "args": ["--database", "/absolute/path/to/memory"]
    }
  }
}
```

Use an absolute database path for a user- or global-level config. A
project-scoped `.mcp.json` may use a relative `./memory`. Full server details
are in [`crates/nahuali-mcp/README.md`](../crates/nahuali-mcp/README.md).

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

- **[`skills/nahuali/SKILL.md`](../skills/nahuali/SKILL.md)** — the Claude Code
  skill. Same substance, packaged so Claude Code loads it automatically when a
  task needs persistent context, recall before answering, or a trust check
  before acting.

## What the agent should do, in one paragraph

Start every session with `briefing`, then `recall` specifics — never assume
context that can be queried. Capture decisions, facts, and events with
`remember` as episodes; those episodes are the evidence everything else cites.
Assert `claim`s and `link`s only when an episode supports them. On every recall
result, read the trust mode (`Certify` / `Advisory` / `Warn` / `Block`), not
just the relevance score: `Certify` is safe to state and cite, `Advisory` is a
lead, `Warn` / `Block` should not be acted through. Before leaning on a lot of
recalled memory, check health with `inspect` or `trust_report`. And never edit
history silently — repairs go through `review_resolve` and stay on the ledger.

## Proving the history was not altered

`trust_report` gives one composed verdict — what we know, why to trust it,
what's missing, and whether the recorded history was altered. `audit` returns a
verifiable diff of what the ledger recorded between two points. With the
optional `tamper-evidence` build feature, events are chained by hash so an
in-place rewrite is detectable even if the checksum was recomputed. Signing a
checkpoint (chain-tip attestation) is a CLI/operator action; the agent reads and
audits integrity, it does not sign.
