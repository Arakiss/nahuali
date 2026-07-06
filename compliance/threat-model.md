# Threat Model

This document describes Nahuali's current shipped trust model. It is public
compliance collateral, not a certification or penetration test report.

## Scope

Nahuali is a local-first memory engine. The authoritative state is the
append-only SurrealDB `memory_record` ledger; graph projection tables, optional
snapshots, and the Qdrant semantic index are derived artifacts, not the source
of truth. The README states that the ledger is authoritative and derived tiers
must be rebuildable (`README.md:420-433`). The security model repeats that the
local database owns the memory ledger and the semantic index is derived
(`SECURITY.md:17-27`).

This threat model covers the Rust core, CLI, MCP stdio server, and local HTTP
API. It does not claim hosted multi-tenant security, account management, secret
management, or encryption at rest.

## Trust Boundaries

| Boundary | Trust role | Evidence |
|---|---|---|
| `nahuali-core` | Canonical event model, ledger replay, validation, audit, attestation, recall, and inspection. | `crates/nahuali-core/src/event.rs:8-39`, `crates/nahuali-core/src/store/records.rs:26-63` |
| CLI | Operator-facing binary over the core. The default CLI feature set enables `tamper-evidence`; Ed25519 attestation is an opt-in feature. | `crates/nahuali-cli/Cargo.toml:19-33`, `crates/nahuali-cli/src/commands/attestation.rs:6-41` |
| MCP stdio server | Local tool adapter over the same core. It opens a local database and exposes memory tools over stdio. | `crates/nahuali-mcp/src/main.rs:16-29`, `crates/nahuali-mcp/src/tools.rs:37-45` |
| Local HTTP API | Thin transport over the same core. Mutating endpoints append through core paths, and the beta API has no accounts, tenants, API keys, or role-based access. | `crates/nahuali-api/src/lib.rs:1-5`, `crates/nahuali-api/README.md:18-20` |
| Storage services | SurrealDB stores the authoritative ledger. Qdrant is a derived semantic index and can be rebuilt from ledger state. | `crates/nahuali-core/src/store/ledger.rs:38-66`, `README.md:431-433` |
| Operator key custody | Ed25519 signing keys, attestation receipts, and keyrings are operator-held material outside the memory store. | `README.md:307-334`, `crates/nahuali-core/src/attestation.rs:1-20` |

`MemoryScope` is a retrieval and projection boundary, not an authorization
boundary. The model describes it as explicit context around the environment or
trust boundary that produced memory (`crates/nahuali-core/src/model.rs:66-79`);
the API README explicitly says scopes are labels, not permission boundaries
(`crates/nahuali-api/README.md:18-20`).

## Integrity Model

1. **Append-only event envelopes.** Each persisted record is an
   `EventEnvelope` with a version, id, sequence, timestamp, checksum, optional
   `prev_hash`, and typed payload (`crates/nahuali-core/src/event.rs:8-39`).
   Payload variants cover sources, episodes, facts, relations, procedures,
   intentions, reviews, and repairs (`crates/nahuali-core/src/event.rs:169-193`).

2. **Validated ledger replay.** Opening a database reads records in sequence and
   validates the envelope sequence and record checksum before projecting current
   memory (`crates/nahuali-core/src/store/ledger.rs:7-35`,
   `crates/nahuali-core/src/store/records.rs:26-63`). Writes go through the
   core append path before in-memory projection is updated
   (`crates/nahuali-core/src/store/services.rs:688-755`).

3. **Hash chain.** With `tamper-evidence` enabled, each new event binds the
   previous event's chain hash into `prev_hash`
   (`crates/nahuali-core/src/event.rs:76-89`,
   `crates/nahuali-core/src/store/services.rs:688-729`). The chain hash is
   SHA-256, domain-separated, and length-prefixed over the canonical event bytes
   and previous link (`crates/nahuali-core/src/event.rs:634-678`). Validation
   detects checksum mismatches, sequence gaps, missing chain links when strict
   mode is required, and broken chain links
   (`crates/nahuali-core/src/validation.rs:120-239`).

4. **Merkle commitments and audit.** Merkle roots and inclusion proofs can be
   derived from the event chain (`crates/nahuali-core/src/merkle.rs:54-142`).
   The audit report restates checksum, sequence, chain, Merkle root, and
   verification status (`crates/nahuali-core/src/audit.rs:101-120`) and verifies
   those properties during non-mutating audit (`crates/nahuali-core/src/audit.rs:223-249`).

5. **Detached Ed25519 tip attestation.** Attestation signs the current chain tip
   with an operator-supplied Ed25519 key and stores the receipt outside the
   ledger (`crates/nahuali-core/src/attestation.rs:1-20`,
   `crates/nahuali-core/src/attestation.rs:39-58`). The core signs and verifies
   tips through deterministic methods (`crates/nahuali-core/src/attestation.rs:95-144`).
   The CLI reads a seed file for signing and returns a non-zero verification
   result when the receipt does not match the live ledger
   (`crates/nahuali-cli/src/commands/attestation.rs:6-101`).

6. **Keyring rotation and revocation.** Attestation keyrings model active and
   revoked keys (`crates/nahuali-core/src/attestation.rs:254-285`). Verification
   with a keyring accepts only trusted active keys and rejects revoked or unknown
   keys (`crates/nahuali-core/src/attestation.rs:292-352`).

7. **Composed trust report.** A trust report combines knowledge health,
   authority, ledger integrity, optional attestation status, and reasons into a
   non-mutating verdict (`crates/nahuali-core/src/trust_report.rs:53-101`,
   `crates/nahuali-core/src/trust_report.rs:103-156`).

## Attacker Assumptions

Nahuali assumes an attacker may be able to:

- edit or delete local ledger records;
- corrupt or rebuild derived projection tables, snapshots, or Qdrant vectors;
- submit poisoned source material or unsupported claims through normal write
  flows;
- recompute a per-event checksum after modifying one record;
- run a full suffix re-chain if the attacker controls the current ledger bytes;
- replay an older valid attestation receipt if freshness is not enforced
  outside the store;
- compromise an API or MCP caller when the deployment exposes those local
  transports beyond their intended trust boundary.

Nahuali assumes an attacker cannot forge an Ed25519 attestation for a chain tip
without access to an authorized private signing key. If the seed file or active
signing key is compromised, a malicious receipt may verify until the operator
rotates or revokes the keyring entry.

## What The Controls Detect

| Attack class | Detection path | Evidence |
|---|---|---|
| Accidental record corruption | Envelope checksum and sequence validation on replay. | `crates/nahuali-core/src/store/ledger.rs:7-35`, `crates/nahuali-core/src/validation.rs:120-239` |
| Historical in-place rewrite with recomputed checksum | Hash-chain validation detects the next broken `prev_hash`. | `crates/nahuali-core/src/event.rs:76-89`, `crates/nahuali-core/src/validation.rs:165-206` |
| Missing chain links in a strict deployment | `require_chained` fails when strict validation sees unchained records. | `crates/nahuali-core/src/validation.rs:18-46`, `crates/nahuali-core/src/validation.rs:187-196` |
| Full suffix re-chain after a signed checkpoint | Old attestation no longer verifies against the new tip. | `crates/nahuali-core/src/attestation.rs:162-228`, `README.md:316-319` |
| Revoked signing key | Keyring verification rejects revoked keys even if the signature is cryptographically valid. | `crates/nahuali-core/src/attestation.rs:292-352`, `crates/nahuali-cli/src/commands/attestation.rs:104-149` |
| Derived-index corruption | Projection and semantic tiers are rebuildable from the validated ledger. | `README.md:424-433`, `crates/nahuali-core/src/maintenance.rs:13-17` |

## Known Limitations

- **No built-in encryption at rest.** Security guidance treats local database
  directories and backup artifacts as sensitive, and the project is not a secret
  manager (`SECURITY.md:3-5`, `SECURITY.md:21-26`). Use platform storage
  controls where the data classification requires encryption.
- **No hosted authentication or tenant access control.** The beta API has no
  accounts, tenants, API keys, or role-based access
  (`crates/nahuali-api/README.md:18-20`). Scopes are labels for recall and
  inspection boundaries, not permission enforcement.
- **Multi-writer concurrency is not yet a cryptographic or transactional
  guarantee.** The current append path computes the next sequence from the
  in-memory event list and writes records one by one
  (`crates/nahuali-core/src/store/services.rs:688-729`,
  `crates/nahuali-core/src/store/ledger.rs:38-66`). Deploy one writer per store
  unless a future version adds DB-side sequencing or transactional append locks.
- **Attestation freshness is external.** A valid old receipt proves that a past
  checkpoint existed; it does not prove the live store is current unless
  automation verifies the latest receipt or an operator-held freshness floor
  (`README.md:336-355`).
- **The unsigned tail is hash-chained but not full-rechain protected.** Events
  appended after the last signed tip remain covered by local chain validation,
  but full-rechain detection depends on a later signed checkpoint
  (`README.md:343-355`).
- **Retention and deletion policy are not enforced.** Recall supports query-time
  `since_ms` and `as_of_ms` windows, and health inspection flags stale memory
  (`crates/nahuali-core/src/recall.rs:19-26`,
  `crates/nahuali-core/src/inspection.rs:289-328`), but automatic retention,
  expiry, and legal erasure workflows are not implemented.
- **Compliance mappings are not certification.** The governance benchmarks are
  first-party, synthetic, and release-gate oriented, not independent
  certification (`GOVERNANCE_BENCHMARKS.md:216-230`).
