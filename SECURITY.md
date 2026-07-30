# Security

Nahuali stores its authoritative SurrealDB `memory_record` ledger in embedded
SurrealKV by default, or at an operator-configured SurrealDB endpoint. Qdrant is
an optional, derived semantic index. Treat every configured data directory and
remote endpoint as sensitive application infrastructure.

## Supported Versions

Nahuali is still in the `0.8` beta release train and does not offer an LTS
branch. Security fixes land on `main` and are included in the next supported
beta release; older beta builds should be upgraded rather than treated as
maintained release lines.

## Reporting

Open a private security advisory on GitHub for vulnerabilities. Do not include
real personal data, credentials, or customer data in public issues.

## Current Model

- The default quickstart uses embedded SurrealKV and requires no Docker service.
- Docker Compose is an optional development path for remote SurrealDB and
  Qdrant; Qdrant is not required for lexical recall.
- The default memory database owns the authoritative `memory_record` ledger;
  the semantic index is derived and can be rebuilt.
- Examples and benchmarks must use synthetic data.
- Snapshots are optional local artifacts and are not authoritative.
- Nahuali is not a secret manager. Do not store credentials, tokens, or customer
  secrets in memory databases.
- Future bindings and server modes must preserve the same evidence and
  inspection boundaries as the core crate.

## Release Checks

The release gate runs:

- formatting, clippy, tests, docs, and regression fixtures
- source-install smoke tests for CLI and MCP
- release dry-run artifact packaging
- license and crate metadata checks
- Cargo lockfile metadata and duplicate dependency inspection
- secret, identity, and large-file scans
- automation checks that reject direct publish, tag, and GitHub Release commands

Use:

```bash
bash scripts/security-supply-chain-check.sh
bash scripts/validate-clean-tree.sh
```
