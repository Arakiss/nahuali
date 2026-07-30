# Add Nahuali to Your Agent (MCP Onboarding)

This is a short path to give an agent evidence-aware, integrity-checked memory.
If your client speaks MCP (Claude Desktop, Claude Code, Cursor, Windsurf, Cline,
and most others), add one stdio server entry. The default persistent store is
embedded, so there is no database service to start.

## Why governed, tamper-evident memory

Some memory integrations retrieve context without preserving a usable evidence
path or reporting conflicts. Nahuali stores append-oriented records and returns
a trust decision (`certify`, `advisory`, `warn`, or `block`) with evidence when
available, so the caller can apply its own action policy. `audit` and
`trust_report` report what is recorded, which support is missing, and whether
specified integrity checks pass. Default builds hash-chain records: a rewritten
non-tip record is detected when the following link was not recomputed. A
last-event rewrite, truncation, rollback, or full re-chain requires comparison
with an externally retained, authorized checkpoint. These controls can
contribute to recordkeeping and traceability programs, but they do not establish
factual truth or regulatory compliance; deployment-specific obligations still
need their own review.

## Install the server

Install the release binaries:

```bash
curl -fsSLo /tmp/nahuali-install.sh \
  https://raw.githubusercontent.com/Arakiss/nahuali/main/scripts/install.sh
sh /tmp/nahuali-install.sh
export PATH="$HOME/.nahuali/bin:$PATH"
```

Or build only the MCP server from source:

```bash
cargo install --path crates/nahuali-mcp --locked
```

Verify it:

```bash
nahuali-mcp --version
```

The binary speaks MCP over stdin/stdout. Your client launches it; you do not run
it by hand. The main argument is `--database`, a SurrealDB database identifier
(default: `memory`). It is not a filesystem path. The embedded store lives under
`~/.nahuali/data`; use `NAHUALI_DB_URL` for a remote SurrealDB deployment.

Nahuali is also published as `io.github.Arakiss/nahuali` in the official MCP
Registry. Its OCI package can be launched with a persistent named volume:

```json
{
  "mcpServers": {
    "nahuali": {
      "command": "docker",
      "args": ["run", "--rm", "-i", "-v", "nahuali-data:/data", "ghcr.io/arakiss/nahuali-mcp:v0.8.0-beta.7"]
    }
  }
}
```

Pin the image to the release you reviewed. The official registry metadata also
uses a versioned image reference; `latest` is convenient for local experiments
but is not a reproducible deployment input.

## Copy-paste config

### Project-scoped (`.mcp.json` in the project root)

Choose a stable database identifier for the project.

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

### Global / user-level

Use the same config shape and choose a different identifier when you want a
separate personal memory context.

```json
{
  "mcpServers": {
    "nahuali": {
      "command": "nahuali-mcp",
      "args": ["--database", "personal_memory"]
    }
  }
}
```

Where this file goes depends on the client:

- Claude Code: a project `.mcp.json`, or run `claude mcp add`.
- Claude Desktop: the `mcpServers` block of `claude_desktop_config.json`.
- Cursor / Windsurf / Cline: each has an "MCP servers" settings file with the
  same `command` / `args` shape.

If `nahuali-mcp` is not on your client's `PATH`, set `command` to its fully
expanded absolute path. MCP clients usually launch the process without a shell,
so `~` may not be expanded.

### Build features

The default build includes hash chaining and the core checkpoint/attestation
types; checkpoint signing remains a CLI/operator action rather than an MCP tool.
The optional feature in this example is the local model-backed semantic provider.

```bash
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

   (`fact` and `relate` are deprecated aliases of `claim` and `link`.
   `preference` is a distinct type for stated rules and defaults, while
   `procedure` records a repeatable how-to.)

5. **Trust report.** Before relying on the memory as a whole, call
   `trust_report`. It returns one composed, non-mutating verdict over knowledge
   counts, authority, restated ledger integrity, knowledge health, and an
   overall `trustworthy` flag with the reasons behind it. That flag reports the
   represented checks; it does not establish factual truth.

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
| Official MCP Registry | `registry.modelcontextprotocol.io` | `server.json` points to the public OCI image and release automation publishes it as `io.github.Arakiss/nahuali` through GitHub OIDC. |
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
