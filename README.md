<p align="center">
  <img src="assets/nahuali-cover.webp" alt="Nahuali, governed and verifiable memory for AI agents" width="100%">
</p>

# Nahuali

**The trust layer for agent memory.** Nahuali shows what a memory is based on,
whether it is safe to use, and when an agent should refuse it.

<p align="center">
  <a href="https://github.com/Arakiss/nahuali/actions/workflows/ci.yml"><img src="https://github.com/Arakiss/nahuali/actions/workflows/ci.yml/badge.svg?branch=main&event=push" alt="Tests"></a>
  <a href="https://codecov.io/gh/Arakiss/nahuali"><img src="https://codecov.io/gh/Arakiss/nahuali/branch/main/graph/badge.svg" alt="Coverage"></a>
  <a href="https://github.com/Arakiss/nahuali/releases"><img src="https://img.shields.io/badge/release-0.8_beta-blue.svg" alt="Latest release train: 0.8 beta"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-FSL--1.1--MIT-yellow.svg" alt="FSL-1.1-MIT license"></a>
  <a href="Cargo.toml"><img src="https://img.shields.io/badge/rust-2024_edition-orange.svg" alt="Rust 2024 edition"></a>
  <img src="https://img.shields.io/badge/platform-macOS_%7C_Linux-5d6d7e.svg" alt="macOS and Linux">
  <a href="https://registry.modelcontextprotocol.io/v0.1/servers?search=io.github.Arakiss%2Fnahuali"><img src="https://img.shields.io/badge/MCP_Registry-published-6f5bd3.svg" alt="Published in the official MCP Registry"></a>
  <img src="https://img.shields.io/badge/default-local--first-3f7f6c.svg" alt="Local-first by default">
  <a href="RELEASE_VERIFICATION.md"><img src="https://img.shields.io/badge/releases-Sigstore_signed-2f6f4e.svg" alt="Sigstore-signed release artifacts"></a>
  <a href=".github/workflows/sbom.yml"><img src="https://img.shields.io/badge/SBOM-CycloneDX-4c6ef5.svg" alt="CycloneDX software bill of materials"></a>
  <a href="TRUST_MODEL.md"><img src="https://img.shields.io/badge/ledger-tamper--evident_by_default-8b5cf6.svg" alt="Tamper-evident ledger by default"></a>
</p>

Most memory systems optimize for finding relevant context. Nahuali asks the
question that comes next: **should the agent trust what it found?**

It records observations, claims, relationships, procedures, and intentions in
an append-only ledger. Recall can return the supporting evidence and a
deterministic trust verdict. Self-inspection finds unsupported, stale, or
contradictory memory without silently rewriting it. The default ledger is hash
chained, and an operator can sign its tip to detect a fully rewritten history.

Nahuali is local-first, source-available, and in public beta.

<p align="center">
  <img src="assets/nahuali-tui.gif" alt="Nahuali explore showing memory, evidence, trust verdicts, ledger integrity, governance signals, and the nahual mascot" width="100%">
</p>

<p align="center"><sub><code>nahuali explore</code>: inspect what the agent remembers, why it trusts it, and whether the ledger is intact. The nahual in the bottom-right corner mirrors the current trust verdict.</sub></p>

## Quickstart

Install the signed macOS or Linux binaries:

```bash
curl -fsSL https://raw.githubusercontent.com/Arakiss/nahuali/main/scripts/install.sh | sh
export PATH="$HOME/.nahuali/bin:$PATH"
```

Record an observation, derive a claim from it, and inspect the result:

```bash
nahuali remember "Lena owns the release notes" --mention Lena --tag product
nahuali claim Lena owns "release notes" --source-last --confidence 0.92
nahuali recall "Who owns the release notes?" --authority --json
nahuali self-inspect --json
```

This is real persistent memory. The default store lives under
`~/.nahuali/data`, survives process restarts, and needs no Docker, database
server, model, account, or API key. Set `NAHUALI_DB_URL` when you want a shared
remote SurrealDB deployment instead.

After upgrading Nahuali, restart any application that keeps `nahuali-mcp`
running. This ensures the CLI and the long-lived MCP server open the embedded
store with the same engine version.

For a narrated integrity example with no writes, run:

```bash
nahuali demo
```

That command explains the hash chain and signed checkpoint without changing
your store. The end-to-end demo below is a separate, reproducible product flow.
From a source checkout, run `scripts/run-launch-demo.sh verify`. It creates a
disposable store, follows it through the CLI, TUI, and a real MCP tool call, then
asserts the expected results.

An evidence-backed claim can drive action. A competing unsourced claim is
retained but blocks action, and self-inspection explains the contradiction
without silently rewriting the ledger.

<p align="center">
  <img src="assets/nahuali-demo.gif" alt="Nahuali showing evidence-backed recall across CLI, TUI, and MCP, then blocking an unsupported contradiction without rewriting memory" width="100%">
</p>

## Why this is different

| Memory failure | Nahuali response |
|---|---|
| A relevant claim has no source | Return it as unsafe, not as a trusted fact |
| Good and weak memories are mixed together | Give each result its own evidence and verdict |
| Claims conflict or become stale | Surface the affected records for explicit review |
| A model proposes a repair | Validate the evidence and append an audited repair event |
| A historical record is edited | Break the ledger chain at the next record |
| The history is rewritten and re-chained | Refuse the old operator-signed checkpoint |

The four trust modes are deterministic:

- `certify`: the available checks support using the result.
- `advisory`: useful as a lead, but not safe to state without qualification.
- `warn`: evidence or health problems require verification.
- `block`: the result must not drive action until the conflict is resolved.

A verdict does not prove that remembered content is true. It makes the reason
for trust inspectable: evidence, provenance, confidence, freshness,
contradictions, and ledger integrity. The exact guarantees and limits live in
the [trust model](TRUST_MODEL.md).

## Use it from an agent

Nahuali ships a stdio MCP server with tools for capture, recall, inspection,
review, repair, intentions, backup, and ledger verification.

Run `nahuali init` to install the bundled skill where supported and print a
native-binary MCP configuration. The server is also published as
`io.github.Arakiss/nahuali` in the official MCP Registry and as an OCI image:

```json
{
  "mcpServers": {
    "nahuali": {
      "command": "docker",
      "args": [
        "run", "--rm", "-i",
        "-v", "nahuali-data:/data",
        "ghcr.io/arakiss/nahuali-mcp:latest"
      ]
    }
  }
}
```

The named volume keeps memory across container restarts. See
[MCP onboarding](crates/nahuali-mcp/ONBOARDING.md) for native and container
configurations.

## The trust loop

```mermaid
flowchart LR
    A[Observed episode] -->|evidence| B[Claim or link]
    B --> C[Authority-aware recall]
    C --> D{Trust verdict}
    D -->|certify| E[Use with evidence]
    D -->|advisory, warn, block| F[Inspect and review]
    F --> G[Explicit repair or resolution]
    G --> H[Append-only audit event]
    H --> C
```

The deterministic core never calls an LLM. A model may propose a repair, but
the engine classifies, gates, and records it. Contradictions are never silently
overwritten.

## Interfaces

| Interface | Purpose | Reference |
|---|---|---|
| `nahuali` | Local workflow for agents, operators, audits, backup, and migration | [CLI](crates/nahuali-cli/README.md) |
| `nahuali-mcp` | Structured tools and resources for MCP clients | [MCP](crates/nahuali-mcp/README.md) |
| `nahuali-api` | Local HTTP integrations with an OpenAPI contract | [HTTP API](crates/nahuali-api/README.md) |
| `nahuali-core` | Deterministic Rust engine and public data contracts | [Core](crates/nahuali-core/README.md) |

The local HTTP API is unauthenticated. Do not expose it to an untrusted
network. Nahuali does not currently provide accounts, hosted sync, billing, or
a managed control plane.

## Storage

SurrealDB's `memory_record` table is the source of truth. Current memory, graph
tables, snapshots, and semantic vectors are derived and rebuildable.

- Embedded SurrealKV is the zero-service default.
- A remote SurrealDB endpoint supports shared deployments.
- Qdrant is optional and used only for semantic recall.
- Deterministic lexical recall works without Qdrant or a model.
- `reconcile` verifies the ledger and rebuilds derived data.
- Backup and restore operate on the authoritative record ledger.

The embedded store has one process owner. A long-running MCP or HTTP server owns
the local store while it is active; a second process fails clearly instead of
waiting or risking concurrent writes. Use remote SurrealDB when several
independent processes need the same memory.

## Reproducible evidence

The [governance benchmark suite](GOVERNANCE_BENCHMARKS.md) tests provenance
coverage, contradiction and staleness detection, trust-verdict calibration,
ledger tampering, and attestation recovery against checked-in fixtures.

The vendor-neutral [Agent Memory Trust Benchmark](benchmarks/agent-memory-trust/README.md)
defines an adapter contract for comparing these capabilities across memory
systems. It reports each capability separately and keeps failures and
unsupported controls visible.

Run Nahuali's release gate:

```bash
bash scripts/verify-governance-benchmarks.sh
bash scripts/verify-controlled-beta.sh
NAHUALI_VERIFY_GITHUB_SETTINGS=1 bash scripts/security-supply-chain-check.sh
```

## Beta limits

- APIs and storage behavior may still change before 1.0.
- Self-inspection proposes work but never writes automatically.
- Evidence proves traceability, not factual truth.
- The operator must retain a trusted signed checkpoint to detect rollback or a
  fully re-chained history.
- Scope labels separate memory contexts but are not access-control boundaries.
- Semantic recall requires an optional Qdrant service; the default lexical path
  does not.

Read [BETA.md](BETA.md) before using irreplaceable data.

## Build from source

```bash
cargo build --workspace
cargo test --workspace
cargo install --path crates/nahuali-cli --locked
cargo install --path crates/nahuali-mcp --locked
cargo install --path crates/nahuali-api --locked
```

Docker is only needed for the optional remote development stack and Qdrant:

```bash
docker compose up -d
```

## More documentation

- [Trust model](TRUST_MODEL.md)
- [Release verification](RELEASE_VERIFICATION.md)
- [Self-repair contract](SELF_REPAIR.md)
- [Governance benchmarks](GOVERNANCE_BENCHMARKS.md)
- [Security policy](SECURITY.md)
- [Contributing](CONTRIBUTING.md)

Questions, use cases, and design feedback belong in
[GitHub Discussions](https://github.com/Arakiss/nahuali/discussions). Bugs and
benchmark contributions have structured [issue templates](https://github.com/Arakiss/nahuali/issues/new/choose).

## License

Nahuali uses the [Functional Source License 1.1 with an MIT future grant](LICENSE).
Each release converts to MIT two years after its release date.
