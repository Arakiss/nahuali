# Pilot Data Policy

This document states what data a Nahuali pilot deployment may store, how the
deployment is isolated, how it is operated safely, and how data is disposed of at
the end of a pilot. It is an operational and data-handling policy, not a
commercial document and not legal advice. It exists so a technical evaluator can
run Nahuali against real workloads without putting sensitive data at risk.

It builds directly on the controlled-beta honesty note, which tells testers to
use Nahuali only with data they can recreate, to use a fresh `--database` value
per test project or person, and to start with synthetic or non-sensitive material
(`BETA.md:38`, `BETA.md:40-42`). Safe handling of secrets and irreplaceable
personal data is an explicit non-goal of the current beta (`BETA.md:100`). A
pilot inherits those rules and makes them concrete.

## What a pilot deployment MAY store

- Synthetic or non-sensitive project notes, decisions, and activity logs.
- Agent memory whose loss is tolerable — data the pilot operator can recreate.
- Provenance-bearing source material (documents, transcripts, notes) that the
  operator is authorized to process and that contains no secrets or
  special-category personal data.

## What a pilot deployment MUST NOT store

- **No production secrets.** No credentials, API keys, tokens, or customer
  secrets. Nahuali is not a secret manager (`SECURITY.md:24-25`).
- **No special-category personal data**, and no irreplaceable personal data. If
  personal data is unavoidable for the evaluation, keep it minimal and scoped,
  and read `compliance/gdpr-position.md` first — Nahuali ships no per-subject
  erasure mechanism today.
- **No data the operator is not authorized to process** under its own contracts
  or applicable law.

## Isolation posture

- **Dedicated store per pilot.** Each pilot runs against its own database name
  (`--database`), never a shared or personal store
  (`BETA.md:40`; `crates/nahuali-api/src/main.rs:13-16`). One pilot's memory
  never lands in another's ledger.
- **Local or customer-hosted.** The stack runs against operator-controlled local
  services (SurrealDB ledger, derived Qdrant index) (`SECURITY.md:17-27`). A
  pilot can be hosted entirely inside the customer's own boundary.
- **No data leaves the deployment.** The crates ship no telemetry or phone-home
  code; the core never touches the network on its own
  (`crates/nahuali-core/src/attestation.rs:19-20`). The only network endpoints
  are the operator's own configured services. Nahuali does not sync pilot data
  anywhere.
- **Scopes are labels, not walls.** A `MemoryScope` separates contexts for recall
  and inspection but is not an authorization boundary and does not enforce
  tenant isolation (`crates/nahuali-api/README.md:18-20`). Isolation between
  pilots comes from separate stores, not from scopes.

## Operational safety

The engine ships the commands needed to run a pilot without losing data. Use
them as a routine, not only after something breaks:

- **Back up before changes.** `backup` writes (or dry-runs) a local
  record-ledger backup; `backup-validate` verifies a backup, and can require a
  tamper-evident chain link on every record
  (`crates/nahuali-cli/src/cli.rs:890-909`).
- **Rehearse the restore.** `backup-drill` validates a backup and dry-runs a
  restore into a target database before you ever need it for real; `restore`
  restores a backup into an empty SurrealDB database
  (`crates/nahuali-cli/src/cli.rs:910-929`). Run the drill on day one so the
  recovery path is known-good.
- **Reconcile after an outage.** After any interruption or suspected drift,
  `reconcile` re-verifies the ledger and rebuilds the derived tiers (graph and
  semantic) from the authoritative record ledger
  (`crates/nahuali-cli/src/cli.rs:73`). Because the semantic index is derived, it
  is rebuildable and never the source of truth (`SECURITY.md:20-21`).
- **Check integrity on a schedule.** `validate`, `audit`, and `trust-report`
  expose ledger integrity and a composed trust verdict without mutating memory
  (`crates/nahuali-cli/src/cli.rs:84-86`).

## Incident handling

- **Detect.** Treat a failed `validate`, a broken chain in `audit`, or a Block
  verdict in `trust-report` as an integrity incident and stop writing to the
  store until it is understood (`crates/nahuali-cli/src/cli.rs:84-86`).
- **Contain.** The dedicated-store posture means an incident in one pilot store
  does not reach another. Preserve the affected store and its latest validated
  backup for analysis before restoring.
- **Report.** Report suspected security vulnerabilities through a private GitHub
  security advisory, and never include real personal data, credentials, or
  customer data in public issues (`SECURITY.md:12-15`).
- **Recover.** Restore from the most recent validated backup (`backup-validate`
  then `restore`) and `reconcile` the derived tiers
  (`crates/nahuali-cli/src/cli.rs:899-929`, `crates/nahuali-cli/src/cli.rs:73`).

## End-of-pilot data disposition

- **Export what the operator keeps.** `export` writes a source-neutral memory
  interchange document the operator can retain or hand back
  (`crates/nahuali-cli/src/cli.rs:97`).
- **Delete the store and its copies.** Dispose of the pilot by deleting the
  dedicated database, its local backups, and the derived Qdrant index. Because a
  pilot is a dedicated store, store-level deletion is a clean disposition rather
  than a blunt instrument. Confirm no backup artifacts remain outside the
  intended retention window.
- **No residue elsewhere.** With no telemetry and no sync, deleting the store and
  its backups removes the pilot's data; there is no hosted copy to purge
  (`crates/nahuali-core/src/attestation.rs:19-20`).

## Honest Position

A pilot is safe to the extent the operator honors the input discipline: recreate-
able data only, no secrets, no special-category personal data, a dedicated store
per pilot, and a rehearsed backup/restore/reconcile routine. Nahuali gives the
operator the commands to isolate, verify, recover, and dispose of pilot data; it
does not enforce those rules for the operator, and it does not make the beta safe
for irreplaceable data. Run the controlled-beta gate
(`bash scripts/verify-controlled-beta.sh`) before a pilot stores anything that
matters (`BETA.md:26-34`).

Last reviewed: 2026-07-10.
