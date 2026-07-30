# Threat Model

This document describes Nahuali's current shipped trust model. It is public
compliance collateral, not a certification or penetration test report.

## Scope

Nahuali is a local-first memory engine. The authoritative state is the
append-only SurrealDB `memory_record` ledger; graph projection tables, optional
snapshots, and the Qdrant semantic index are derived artifacts, not the source
of truth. The README states that the ledger is authoritative and derived tiers
must be rebuildable (`README.md:169-184`). The security model repeats that the
local database owns the memory ledger and the semantic index is derived
(`SECURITY.md:20-32`).

This threat model covers the Rust core, CLI, MCP stdio server, and local HTTP
API. It does not claim hosted multi-tenant security, account management, secret
management, or encryption at rest.

## Trust Boundaries

| Boundary | Trust role | Evidence |
|---|---|---|
| `nahuali-core` | Canonical typed event model, validated ledger replay, audit, recall, and health inspection. | `crates/nahuali-core/src/event.rs:25-53`; `crates/nahuali-core/src/store/ledger.rs:13-44`; `crates/nahuali-core/src/audit.rs:153-310`; `crates/nahuali-core/src/recall.rs:394-555`; `crates/nahuali-core/src/inspection.rs:181-380` |
| CLI | Operator-facing binary over the core. The default CLI feature set enables `tamper-evidence` and `attestation`; `--no-default-features` is the explicit legacy unchained build. | `crates/nahuali-cli/Cargo.toml:19-34`, `crates/nahuali-cli/src/commands/attestation.rs:6-41` |
| MCP stdio server | Local tool adapter over the same core. It opens a local database and exposes memory tools over stdio. | `crates/nahuali-mcp/src/main.rs:16-28`, `crates/nahuali-mcp/src/tools.rs:37-45` |
| Local HTTP API | Thin transport over the same core. Mutating endpoints append through core paths, and the beta API has no accounts, tenants, API keys, or role-based access. | `crates/nahuali-api/src/lib.rs:1-5`, `crates/nahuali-api/README.md:18-20` |
| Storage services | SurrealDB stores the authoritative ledger. Qdrant is an optional derived semantic index and can be rebuilt from ledger state. | `SECURITY.md:3-6`; `README.md:173-186` |
| Operator key custody | Ed25519 checkpoint keys, policies, and receipts are operator-held material outside the memory store. | `README.md:102-140`, `crates/nahuali-core/src/checkpoint.rs:125-220` |

`MemoryScope` is a retrieval and projection boundary, not an authorization
boundary. The model describes it as explicit context around the environment or
trust boundary that produced memory (`crates/nahuali-core/src/model.rs:66-79`);
the API README explicitly says scopes are labels, not permission boundaries
(`crates/nahuali-api/README.md:18-20`).

## Integrity Model

1. **Append-only event envelopes.** Each persisted record is an
   `EventEnvelope` with a version, id, sequence, timestamp, checksum, optional
   `prev_hash`, and typed payload (`crates/nahuali-core/src/event.rs:25-53`).
   Payload variants cover sources, episodes, facts, relations, procedures,
   intentions, reviews, and repairs (`crates/nahuali-core/src/event.rs:199-223`).

2. **Validated ledger replay.** Opening a database reads records in sequence and
   validates the envelope sequence and record checksum before projecting current
   memory (`crates/nahuali-core/src/store/ledger.rs:13-44`,
   `crates/nahuali-core/src/store/records.rs:49-68`). Writes go through the
   core append path before in-memory projection is updated
   (`crates/nahuali-core/src/store/services.rs:833-883`).

3. **Hash chain.** With `tamper-evidence` enabled, each new event binds the
   previous event's chain hash into `prev_hash`
   (`crates/nahuali-core/src/event.rs:106-159`,
   `crates/nahuali-core/src/store/services.rs:833-879`). The chain hash is
   SHA-256, domain-separated, and length-prefixed over the canonical event bytes
   and previous link (`crates/nahuali-core/src/event.rs:729-781`). Validation
   detects checksum mismatches, sequence gaps, missing chain links when strict
   mode is required, and broken chain links
   (`crates/nahuali-core/src/validation.rs:142-280`). A historical rewrite is
   exposed when a later stored `prev_hash` no longer matches. A last-event
   rewrite, truncation, rollback, or a fully recomputed suffix is not detectable
   from the live chain alone.

4. **Merkle commitments and audit.** Merkle roots and inclusion proofs can be
   derived from the event chain (`crates/nahuali-core/src/merkle.rs:54-142`).
   The audit report restates checksum, sequence, chain, Merkle root, and
   verification status (`crates/nahuali-core/src/audit.rs:129-184`) and verifies
   those properties during non-mutating audit (`crates/nahuali-core/src/audit.rs:195-310`).
   Inclusion and consistency are evaluated relative to supplied roots. A root is
   not a trust anchor until an authorized party retains or signs it outside the
   store.

5. **Authorized Ed25519 checkpoints.** Version 2 checkpoints bind origin,
   lineage, tree size, Merkle root, chain tip, and signer time. Verification
   requires a separately held operator policy; a key embedded in the signed
   document never authorizes itself (`crates/nahuali-core/src/checkpoint.rs:125-220`,
   `crates/nahuali-core/src/checkpoint.rs:375-391`). The older detached chain-tip
   attestation remains a compatibility path and needs an external keyring for
   trust-sensitive use (`crates/nahuali-core/src/attestation.rs:1-20`).

6. **Policy rotation and revocation.** Version 2 checkpoint policies model
   active and revoked keys plus a signature threshold. Unknown and revoked keys
   do not count toward authorization (`crates/nahuali-core/src/checkpoint.rs:163-220`,
   `crates/nahuali-core/src/checkpoint.rs:575-709`).

7. **Composed trust report.** A trust report combines knowledge health,
   authority, ledger integrity, optional attestation status, and reasons into a
   non-mutating verdict (`crates/nahuali-core/src/trust_report.rs:119-222`).

## Attacker Assumptions

Nahuali assumes an attacker may be able to:

- edit, delete, truncate, roll back, or replace local ledger records;
- corrupt or rebuild derived projection tables, snapshots, or Qdrant vectors;
- submit poisoned source material or unsupported claims through normal write
  flows;
- recompute a per-event checksum after modifying one record;
- run a full suffix re-chain if the attacker controls the current ledger bytes;
- replay an older authorized checkpoint or compatibility receipt if freshness is not enforced
  outside the store;
- compromise an API or MCP caller when the deployment exposes those local
  transports beyond their intended trust boundary.

Nahuali assumes an attacker cannot produce an accepted Ed25519 checkpoint
signature without an authorized private signing key. If an active signing key
is compromised, an attacker can produce signatures accepted by policies that
still authorize that key. Rotation, revocation, and threshold policy are
operator responsibilities; the legacy chain-tip receipt has the same external
keyring dependency without the version 2 origin, lineage, or threshold fields.

## What The Controls Detect

| Attack class | Detection path | Evidence |
|---|---|---|
| Accidental record corruption | Envelope checksum and sequence validation on replay. | `crates/nahuali-core/src/store/ledger.rs:13-44`, `crates/nahuali-core/src/validation.rs:142-280` |
| Non-tip historical rewrite with recomputed checksum, without re-chaining | Hash-chain validation detects the next broken `prev_hash`. | `crates/nahuali-core/src/event.rs:106-197`, `crates/nahuali-core/src/validation.rs:240-273` |
| Missing chain links in a strict deployment | `require_chained` defaults to enabled and fails when strict validation sees unchained records. | `crates/nahuali-core/src/validation.rs:33-67`; `crates/nahuali-core/src/validation.rs:240-273` |
| Last-event rewrite, truncation, rollback, or full re-chain after an authorized checkpoint | Current-mode verification fails against the externally retained checkpoint and policy. Without that external reference, the live store can be internally consistent. | `crates/nahuali-core/src/checkpoint.rs:375-574`, `crates/nahuali-cli/src/commands/checkpoint.rs:187-235` |
| Revoked signing key | Checkpoint authorization ignores revoked keys even if their signatures are cryptographically valid. | `crates/nahuali-core/src/checkpoint.rs:575-709` |
| Derived-index corruption | Projection and semantic tiers are rebuildable from the validated ledger. | `README.md:173-186` |

## Known Limitations

- **No built-in encryption at rest.** Security guidance treats configured data
  directories and remote endpoints as sensitive, and the project is not a secret
  manager (`SECURITY.md:3-6`, `SECURITY.md:22-30`). Use platform storage
  controls where the data classification requires encryption.
- **No hosted authentication or tenant access control.** The beta API has no
  accounts, tenants, API keys, or role-based access
  (`crates/nahuali-api/README.md:18-20`). Scopes are labels for recall and
  inspection boundaries, not permission enforcement.
- **Concurrency protection has a bounded scope.** Ordinary concurrent appends to
  a remote store use a unique sequence index plus bounded refresh-and-retry on a
  collision. Batch imports use one transactional SurrealQL statement. Embedded
  SurrealKV still has one process owner, and operators must coordinate
  administrative restore and rebuild workflows; these controls are not a
  distributed-consensus protocol (`crates/nahuali-core/schema/memory_record.surql:1-2`,
  `crates/nahuali-core/src/store/services.rs:833-879`,
  `crates/nahuali-core/src/store/ledger.rs:95-118`,
  `crates/nahuali-core/src/database.rs:259-268`).
- **Checkpoint freshness is external.** An authorized old checkpoint proves a
  past committed state; it does not prove the live store is current unless
  automation verifies the latest accepted checkpoint or an operator-held freshness floor
  (`README.md:221-229`).
- **The unsigned tail is hash-chained but not externally anchored.** Events
  appended after the last signed tip remain covered by local chain validation,
  but detecting last-event rewrite, rollback, and full re-chain depends on a
  later authorized checkpoint
  (`README.md:221-229`).
- **Retention and deletion policy are not enforced.** Recall supports query-time
  `since_ms` and `as_of_ms` windows, and health inspection flags stale memory
  (`crates/nahuali-core/src/recall.rs:19-26`,
  `crates/nahuali-core/src/inspection.rs:289-328`), but automatic retention,
  expiry, and legal erasure workflows are not implemented.
- **Compliance mappings are not certification.** The governance benchmarks are
  first-party, synthetic, and release-gate oriented, not independent
  certification (`GOVERNANCE_BENCHMARKS.md:216-230`).

Last reviewed: 2026-07-17.
