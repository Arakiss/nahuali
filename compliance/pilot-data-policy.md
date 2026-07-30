# Pilot Data Policy

This document states what data a Nahuali pilot deployment may store, the
isolation controls an operator must provide, and how data should be handled and
disposed of at the end of a pilot. It is an operational and data-handling policy, not a
commercial document and not legal advice. It exists so a technical evaluator can
run Nahuali against real workloads without putting sensitive data at risk.

It builds directly on the controlled-beta honesty note, which tells testers to
use Nahuali only with data they can recreate, to use a fresh `--database` value
per test project or person, and to start with synthetic or non-sensitive material
(`BETA.md:47-56`). Handling secrets and irreplaceable personal data is an
explicit non-goal of the current beta (`BETA.md:109-120`). A
pilot inherits those rules and makes them concrete.

## What a pilot deployment MAY store

- Synthetic or non-sensitive project notes, decisions, and activity logs.
- Agent memory whose loss is tolerable — data the pilot operator can recreate.
- Provenance-bearing source material (documents, transcripts, notes) that the
  operator is authorized to process and that contains no secrets or
  special-category personal data.

## What a pilot deployment MUST NOT store

- **No production secrets.** No credentials, API keys, tokens, or customer
  secrets. Nahuali is not a secret manager (`SECURITY.md:29-30`).
- **No special-category personal data**, and no irreplaceable personal data. If
  personal data is unavoidable for the evaluation, keep it minimal and scoped,
  and read `compliance/gdpr-position.md` first — Nahuali ships no per-subject
  erasure mechanism today.
- **No data the operator is not authorized to process** under its own contracts
  or applicable law.

## Isolation posture

- **Dedicated store per pilot.** Each pilot runs against its own database name
  (`--database`), never a shared or personal store
  (`BETA.md:49`; `crates/nahuali-api/src/main.rs:13-16`). This is an operator
  convention, not a tenant authorization control: external credentials and
  routing must prevent a caller from selecting another pilot's database.
- **Local or customer-hosted.** The stack runs against operator-controlled local
  storage by default, or operator-configured SurrealDB and optional Qdrant
  services (`SECURITY.md:20-32`). Keeping those endpoints inside a chosen region
  or network boundary is the operator's responsibility.
- **Operator-controlled runtime data paths.** The default store is embedded and
  the HTTP API binds to loopback. Memory can travel to operator-configured
  SurrealDB and Qdrant endpoints; inspect those URLs, credentials, logs,
  backups, and network routes before using pilot data
  (`crates/nahuali-core/src/database.rs:161-174`,
  `crates/nahuali-core/src/semantic/types.rs:16-77`,
  `crates/nahuali-api/src/main.rs:13-16`).
- **Scopes are labels, not walls.** A `MemoryScope` separates contexts for recall
  and inspection but is not an authorization boundary and does not enforce
  tenant isolation (`crates/nahuali-api/README.md:18-20`). Isolation between
  pilots comes from separate stores, not from scopes.

## Operational controls

The engine ships commands for backup, validation, restore rehearsal, and
integrity inspection. They reduce recovery risk but do not guarantee against
data loss. Use them as a routine, not only after something breaks:

- **Back up before changes.** `backup` writes (or dry-runs) a local
  record-ledger backup; `backup-validate` verifies a backup, and can require a
  tamper-evident chain link on every record
  (`crates/nahuali-cli/src/cli.rs:1022-1043`).
- **Rehearse the restore.** `backup-drill` validates a backup and dry-runs a
  restore into a target database before you ever need it for real; `restore`
  restores a backup into an empty SurrealDB database
  (`crates/nahuali-cli/src/cli.rs:1044-1067`). Run the drill on day one to verify
  that specific artifact and target configuration; it does not prove every
  future backup is complete or current.
- **Reconcile after an outage.** After any interruption or suspected drift,
  `reconcile` re-verifies the ledger and rebuilds the derived tiers (graph and
  semantic) from the authoritative record ledger
  (`crates/nahuali-cli/src/cli.rs:225-231`). Because the semantic index is derived, it
  is rebuildable and never the source of truth (`SECURITY.md:25-26`).
- **Check integrity on a schedule.** `validate`, `audit`, and `trust-report`
  expose ledger integrity and a composed trust verdict without mutating memory
  (`crates/nahuali-cli/src/cli.rs:829-889`).

## Incident handling

- **Detect.** Treat a failed `validate` or a broken chain in `audit` as an
  integrity signal. Treat a `Block` verdict in `trust-report` as a policy stop,
  inspect its reasons, and do not assume it means ledger corruption
  (`crates/nahuali-cli/src/cli.rs:829-889`).
- **Contain.** A dedicated store limits data co-residence only when external
  credentials, routing, and database selection are also isolated. Preserve the
  affected store and the relevant externally authorized checkpoint and backups
  for analysis before restoring.
- **Report.** Report suspected security vulnerabilities through a private GitHub
  security advisory, and never include real personal data, credentials, or
  customer data in public issues (`SECURITY.md:15-18`).
- **Recover.** Restore from the most recent validated backup (`backup-validate`
  then `restore`) and `reconcile` the derived tiers
  (`crates/nahuali-cli/src/cli.rs:1022-1067`, `crates/nahuali-cli/src/cli.rs:225-231`).

## End-of-pilot data disposition

- **Export what the operator keeps.** `export` writes a source-neutral memory
  interchange document the operator can retain or hand back
  (`crates/nahuali-cli/src/cli.rs:1069-1075`).
- **Delete every operator-controlled copy.** Dispose of the dedicated database,
  local and remote backups, exported files, logs that may contain payloads, and
  the derived Qdrant collection. Nahuali does not provide a report proving that
  every copy was found or erased.
- **Verify configured services separately.** This runtime-path inventory does
  not account for persistence, snapshots, or logs at operator-configured
  SurrealDB, Qdrant, storage, and monitoring services.

## Current limits

A pilot's risk is bounded only to the extent the operator honors the input
discipline: recreatable data, no secrets or special-category personal data,
externally enforced store isolation, and a rehearsed backup/restore/reconcile
routine. Nahuali provides integrity and recovery commands but does not enforce
data classification, tenant authorization, retention, or complete disposal. Do
not use the beta for irreplaceable data. Run the controlled-beta gate
(`bash scripts/verify-controlled-beta.sh`) before a pilot stores anything that
matters (`BETA.md:35-43`).

Last reviewed: 2026-07-17.
