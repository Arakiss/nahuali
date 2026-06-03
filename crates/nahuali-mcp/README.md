# Nahuali MCP Server

`nahuali-mcp` is the local MCP stdio server for Nahuali. It uses the same
append-only record ledger and core memory engine as the CLI.

## Install From Source

From the repository root:

```bash
cargo install --path crates/nahuali-mcp --locked
```

Verify the installed command:

```bash
nahuali-mcp --version
```

## Run

```bash
nahuali-mcp --database ./memory
```

MCP clients launch this process and communicate over stdin/stdout. stdout is
reserved for MCP JSON-RPC messages; diagnostics must not print extra data to
stdout.

## Client Configuration

Register `nahuali-mcp` as a stdio server in your MCP client. The command is the
installed binary; pass the memory database with `--database`.

For a project-scoped MCP config file (for example `.mcp.json` in the project root):

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

For a global or user-level MCP client config, use an absolute database
path so it resolves regardless of the launch directory:

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

At the start of every session, call the `briefing` tool first. It returns the
read-only pre-work surface (authority, health, recent episodes, active
intentions, high-priority review items, and graph seeds) and changes nothing, so
it is the recommended session-start entry point before recalling specifics or
acting.

The server exposes tools for `remember`, `claim`, `fact`, `link`, `relate`,
`procedure`, `preference`, `intention`, `intention_update`,
`intention_status`, `reconcile_intentions`, `goal_progress`, `proactive`,
`deadlines`, `anomalies`, `anomaly_acknowledge`, `ingest`, `ingest_text`,
`briefing`, `memory_hook`, `recall`, `inspect`, `graph`, `self_inspect`,
`review`, `reflect`, `consolidation_plan`, `review_resolve`,
`projection_status`, `projection_rebuild`, `projection_validate`,
`semantic_status`, `semantic_rebuild`, and `validate`. It also exposes
read-only JSON resources for database summary, sources, health, entities,
episodes, claims, links, facts, relations, procedures, intentions, and records,
plus prompts for health-checked recall and evidence-backed claim recording.

Every tool advertises a typed JSON Schema for its output in the `tools/list`
response and returns matching `structuredContent`, so a client can validate
results against the advertised schema instead of parsing prose. An stdio
integration test freezes this surface: the tool-name set, the typed output view
each tool maps to, and the nested fields hosts rely on, such as the trust view
on recall results.

Recall tool responses include an authority decision and the health report used
to produce it, so clients can distinguish `certify`, `advisory`, `warn`, and
`block` outcomes without issuing separate calls. Authority JSON includes stable
`mode`, `score`, `can_trust`, and deduplicated `signal_kinds` fields.
The tool also accepts `kinds` and `requireEvidence` when a host needs a narrow,
evidence-backed recall surface before acting.
The `graph` tool returns a deterministic graph neighborhood around a seed,
including nodes, edges, depth, evidence IDs, authority, and health/review
overlays.

Recording and recall tools accept an optional structured `scope` argument:

```json
{
  "kind": "project",
  "name": "Nahuali"
}
```

Scopes are explicit memory context boundaries, not permissions. Supported kinds
are `personal`, `project`, `organization`, and `custom`. Scoped recall is an
exact filter; it returns records in the requested scope and does not silently
merge unscoped memory into the result set.

The `ingest` tool accepts the same provenance-aware source document as the CLI.
Set `dryRun` to validate the document without appending records. The returned
ingestion report includes `preflight` with scope, source-size, derived-record,
evidence-gap, and episode-reference counts.

Interchange import reports expose the same migration-readiness idea through a
compact self-inspection forecast before writes are applied by the CLI import
path.

The `ingest_text` tool accepts direct UTF-8 text content and converts it into
source episodes through the same adapter used by the CLI text path. It returns
both the adapter report and the ingestion report; `dryRun` validates without
appending records. It does not infer claims or links from the text, and its
ingestion report exposes the same preflight summary.

The `briefing` tool returns the compact pre-work surface: authority, health,
recent episodes, active intentions, high-priority review items, and graph seeds.

The operator-loop tools match the CLI/API beta subset for non-mutating planning
and explicit lifecycle writes. Use `intention_update` to set goal links,
deadlines, dependencies, and progress; `reconcile_intentions` and
`goal_progress` to inspect commitments; `proactive`, `deadlines`, and
`anomalies` to read operator reports; and `anomaly_acknowledge` to append or
preview an explicit anomaly review decision.

The `memory_hook` tool packages the same governed context for host execution
points. Hosts pass `kind` as `session_start`, `pre_prompt`, `post_action`,
`session_close`, or `sleep_cycle`; prompt and action hooks also pass `input`.
The result includes authority, directives, recall when relevant, and
non-mutating reflection or self-inspection reports for close and sleep hooks.
Sleep hooks also include the Sleep Mode report with replay stages,
consolidation candidates, review items, and
`automatic_write_back=false`.

The `validate` tool includes the record-ledger compatibility report fields
`supported_event_version`, `observed_event_versions`, `legacy_event_count`,
`migration_required`, and `issues`.

The `self_inspect` tool returns a non-mutating consolidation report with health,
authority, findings, proposed review items, and an explicit
`automatic_write_back=false` policy. It includes source-coverage findings when
episodes lack source records or derived memory lacks source episode evidence.

The `reflect` tool groups self-inspection findings into non-mutating reflection
cycles with priority, evidence IDs, source/evidence coverage, and the same
operator-review write-back policy.

The `consolidation_plan` tool turns rest, reflection, and review signals into
explicit replay, extraction, reconciliation, review-gate, and
commit-eligibility operations. It is non-mutating and keeps
`automatic_write_back=false`.

The `review` tool returns a prioritized non-mutating operator queue derived from
self-inspection. Agents should treat it as guidance for explicit follow-up tool
calls, not as automatic memory write-back. The optional `action` argument
narrows the queue to one proposed operator action.

The `review_resolve` tool is the explicit review write-back path. It requires a
review item ID and an operator note, then appends an audit decision or previews
that decision when `dryRun` is true.

The projection and semantic tools expose derived-tier operations without
changing the ledger contract. `projection_status`, `projection_rebuild`, and
`projection_validate` inspect or rebuild the SurrealDB graph projection derived
from `memory_record`; `semantic_status` and `semantic_rebuild` inspect or
rebuild the Qdrant semantic index from projected memory.

## Optional Build Features

These are off by default; a default build is unchanged and writes byte-identical
records.

- `--features tamper-evidence`: recorded events are chained by hash, so the
  `validate` tool detects an in-place rewrite of any historical record even when
  its checksum was recomputed.
- `--features local-embeddings`: `semantic_rebuild` and semantic recall use a
  static model2vec model instead of the deterministic embedder. Set
  `NAHUALI_EMBEDDING_PROVIDER=model2vec` and point
  `NAHUALI_LOCAL_EMBEDDING_MODEL_PATH` at a local model directory.

Chain-tip attestation (signing) is a CLI/operator action; the MCP server exposes
no signing tool.

`fact`, `relate`, and `preference` are deprecated compatibility aliases of
`claim`, `link`, and `procedure`. Prefer the canonical tools; the aliases stay
only until clients finish migrating.
