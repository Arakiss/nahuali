# Add Nahuali to Your Agent (MCP Onboarding)

This is the fastest path to give an AI agent governed, tamper-evident memory.
If your client speaks MCP (Claude Desktop, Claude Code, Cursor, Windsurf, Cline,
and most others), you add one stdio server entry and you are done.

## Why governed, tamper-evident memory

Most agent "memory" is an undifferentiated blob: the agent cannot tell what is
actually known, why any of it should be trusted, what evidence backs it, or
whether the recorded history was quietly altered. Nahuali stores memory as an
append-only ledger where every recalled result carries its evidence and a trust
decision (`certify`, `advisory`, `warn`, or `block`), so an agent can refuse to
act on memory it should not trust. The ledger restates its own integrity:
`audit` and `trust_report` answer what is known, why to trust it, what is
missing, and whether any historical record was rewritten. With the optional
`tamper-evidence` build feature, records are hash-chained, so an in-place rewrite
of any past record is detected even if its checksum was recomputed. The result
is memory your agent can cite and an operator can verify, which is what
record-keeping and traceability obligations (such as EU AI Act Article 12)
actually require.

## Install the server binary

From the repository root:

```bash
cargo install --path crates/nahuali-mcp --locked
```

Verify it:

```bash
nahuali-mcp --version
```

The binary speaks MCP over stdin/stdout. Your client launches it; you do not run
it by hand. The only argument is `--database`, the path to the memory store
(default: `memory` in the launch directory).

## Copy-paste config

### Project-scoped (`.mcp.json` in the project root)

Use a relative database path so the memory store lives next to the project.

```json
{
  "mcpServers": {
    "nahuali": {
      "command": "nahuali-mcp",
      "args": ["--database", "./memory"]
    }
  }
}
```

### Global / user-level (absolute path)

Use an absolute database path so it resolves no matter where the client launches
the server from. Replace the path with one you own.

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

Where this file goes depends on the client:

- Claude Code: a project `.mcp.json`, or run `claude mcp add`.
- Claude Desktop: the `mcpServers` block of `claude_desktop_config.json`.
- Cursor / Windsurf / Cline: each has an "MCP servers" settings file with the
  same `command` / `args` shape.

If `nahuali-mcp` is not on your client's `PATH`, set `command` to the absolute
path of the installed binary (for example `~/.cargo/bin/nahuali-mcp`).

### Optional build features

Both are off by default; a default build writes byte-identical records.

```bash
# Hash-chained records: validate/audit detect an in-place rewrite of history.
cargo install --path crates/nahuali-mcp --locked --features tamper-evidence

# Local semantic recall via a static model2vec model instead of the
# deterministic embedder.
cargo install --path crates/nahuali-mcp --locked --features local-embeddings
```

For `local-embeddings`, set `NAHUALI_EMBEDDING_PROVIDER=model2vec` and point
`NAHUALI_LOCAL_EMBEDDING_MODEL_PATH` at a local model directory.

## First session walkthrough

Once the server is registered, the agent has the Nahuali tools. A good first
session looks like this.

1. **Briefing first.** Call `briefing` before anything else. It returns the
   read-only pre-work surface (authority, health, recent episodes, active
   intentions, high-priority review items, graph seeds) and changes nothing.

   ```json
   { "name": "briefing", "arguments": {} }
   ```

2. **Recall with trust.** Before acting on something memory might already know,
   call `recall`. Each result carries per-result trust and evidence IDs, plus a
   store-level authority decision and a health report in one call. Read the
   authority before relying on a result; if it is `warn` or `block`, state the
   gap instead of asserting. Narrow to evidence-backed records with
   `requireEvidence` and `kinds` when you are about to act.

   ```json
   {
     "name": "recall",
     "arguments": {
       "query": "deployment process",
       "requireEvidence": true,
       "kinds": ["claim", "procedure"]
     }
   }
   ```

3. **Remember the observation.** When the user states something worth keeping,
   record it verbatim as an episode. Episodes are the evidence every other piece
   of memory cites, so record them before asserting any claim or link.

   ```json
   {
     "name": "remember",
     "arguments": {
       "content": "We deploy from the main branch only after CI is green.",
       "mentions": ["deployment"]
     }
   }
   ```

4. **Claim with evidence.** If the episode explicitly supports an assertion,
   derive a `claim` and cite the episode you just recorded with
   `sourceLast: true`. This is what makes the assertion evidence-backed instead
   of an orphaned statement.

   ```json
   {
     "name": "claim",
     "arguments": {
       "subject": "deployment",
       "predicate": "requires",
       "object": "green CI on main",
       "sourceLast": true,
       "confidence": 0.9
     }
   }
   ```

   (`fact`, `relate`, and `preference` are deprecated aliases of `claim`,
   `link`, and `procedure`; prefer the canonical tools.)

5. **Trust report.** Before relying on the memory as a whole, call
   `trust_report`. It returns one composed, non-mutating verdict over knowledge
   counts, authority, restated ledger integrity, knowledge health, and an
   overall `trustworthy` flag with the reasons behind it.

   ```json
   { "name": "trust_report", "arguments": {} }
   ```

   When you need the underlying diff (what the ledger recorded between two
   points and whether that history verifies, including the hash chain and
   anchoring tips under `tamper-evidence`), call `audit`.

Every tool advertises a typed JSON Schema for its output in `tools/list` and
returns matching `structuredContent`, so your client can validate results
against the schema instead of parsing prose. For the full tool, resource, and
prompt surface, see the [README](./README.md).

---

## Distribution checklist

For maintainers publishing the server so agent builders can discover it. The
order below is the recommended sequence: the official registry is the source
other tools sync from; the rest add reach. Nahuali ships as a compiled Rust
binary, so npm is not required for any of these.

| Target | URL | What a submission needs |
| --- | --- | --- |
| Official MCP Registry | `registry.modelcontextprotocol.io` | A `server.json` published with `mcp-publisher`. Namespace `io.github.<owner>/nahuali`, verified by GitHub login. The registry hosts metadata only, not artifacts, so point it at a published package. The Rust binary qualifies via an **MCPB GitHub release** or an **OCI image** (GitHub Container Registry) — both are supported, so npm/PyPI are not needed. Wire it into CI so each release self-registers via GitHub OIDC (no stored secrets). |
| Glama | `glama.ai/mcp/servers` | Auto-indexer. It crawls GitHub and refreshes daily, so a public repo with a clear README and the `mcp` topic is largely picked up on its own; an "Add Server" form exists to submit the repo URL directly. **Zero-maintenance / GitHub-indexed.** No npm package required. |
| mcp.so | `mcp.so` | Community directory. Submit via the "Submit" button or by opening a GitHub issue on their repo with the server name, description, features, and connection details (the `nahuali-mcp` command and `--database` arg). **Low-maintenance**, one-time submission. No npm package required. |
| awesome-mcp-servers | `github.com/punkpeye/awesome-mcp-servers` | A pull request against `main`. Add one line in the appropriate category (Rust language tag; a knowledge/memory category), in alphabetical order, matching the existing format, with an accurate link and description. The canonical GitHub list that other tools and LLMs read. **Zero-maintenance / GitHub-indexed** after merge. No npm package required. |
| Smithery | `smithery.ai` | App-store-style directory oriented toward installable and hosted (remote) servers; its tooling assumes an npm/Node package or a hosted deployment. **Lower fit for a local Rust stdio binary** and the most maintenance to satisfy. Defer until there is a packaged/hosted distribution; the targets above cover discovery without it. |

Practical sequence for a solo maintainer:

1. Publish a GitHub release with the built binary (MCPB) or an OCI image.
2. Add `server.json`, run `mcp-publisher`, and let CI re-register on each release
   (official registry).
3. Open the PR to `awesome-mcp-servers` and the submission to mcp.so.
4. Confirm Glama has indexed the repo (or submit the URL once).
5. Revisit Smithery only if a packaged or hosted distribution is added later.
