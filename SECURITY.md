# Security

Nahuali stores memory in the configured local data stack, including an
authoritative SurrealDB `memory_record` ledger and a derived Qdrant semantic
index. Treat those data directories as sensitive application data.

## Supported Versions

Nahuali is still in a pre-release phase. Security fixes land on `main` until a
public versioning policy exists.

## Reporting

Open a private security advisory on GitHub for vulnerabilities. Do not include
real personal data, credentials, or customer data in public issues.

## Current Model

- The default quickstart requires the local Docker Compose database stack.
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
