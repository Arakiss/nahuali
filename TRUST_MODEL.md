# Nahuali trust model

Nahuali makes the basis for trusting agent memory inspectable. It does not
claim that a stored sentence is true merely because it was recorded.

The trust model has three separate questions:

1. **Can this result be used?** Authority-aware recall evaluates the result's
   evidence, confidence, freshness, contradictions, and review state.
2. **Does the store need attention?** Self-inspection reports unsupported,
   stale, contradictory, or isolated memory without changing it.
3. **Was recorded history rewritten?** Ledger checksums, a default hash chain,
   and operator-held signed checkpoints cover different tampering classes.

Keeping these questions separate matters. One supported result may still
receive `certify` while an unrelated contradiction makes the store's overall
authority `block`.

## Result authority

MCP and HTTP recall responses always include per-result trust. The CLI exposes
the same contract with:

```bash
nahuali --database memory recall "release owner" --authority --json
```

Each result includes its evidence identifier when one exists and one of four
deterministic verdicts:

| Verdict | Meaning |
|---|---|
| `certify` | The available checks support using the result with its evidence. |
| `advisory` | The result is useful as a lead but should be qualified. |
| `warn` | Evidence or health problems require independent verification. |
| `block` | The result should not drive action until its conflict is resolved. |

Authority is policy, not truth. An episode can faithfully record a mistaken
statement. Nahuali can prove which observation a claim cites and how that claim
was evaluated; it cannot prove the external world matched the observation.

## Store inspection

`inspect`, `self-inspect`, `review`, `reflect`, `sleep`, and
`consolidation-plan` are non-mutating. They report knowledge-health problems,
prioritize review, and describe proposed work. They do not silently merge,
delete, or rewrite memory.

```bash
nahuali --database memory inspect --json
nahuali --database memory self-inspect --json
nahuali --database memory review --json
```

Any accepted resolution is appended as an explicit audit event. Governed repair
validates evidence coverage and risk before a write becomes eligible. The full
state machine is documented in the [Self-Repair Contract](SELF_REPAIR.md).

## Ledger integrity

The SurrealDB `memory_record` table is the authoritative append-only ledger.
Default builds enable `attestation`, which includes the `tamper-evidence` hash
chain. The tiers deliberately make different guarantees:

| Integrity tier | Detects | Does not detect by itself |
|---|---|---|
| Event checksum | Accidental or direct modification of one event | An attacker who edits the event and recomputes its checksum |
| Hash chain | In-place history edits, even with recomputed event checksums | A full rewrite followed by re-chaining every later event |
| Signed chain-tip checkpoint | A full re-chain or rollback relative to the retained receipt | Compromise of the signing key or replacement of both store and receipt |

The chain is automatic in default binaries. Signing is an explicit operator
action because the private key and trusted receipt must remain under operator
control:

```bash
nahuali --database memory attest-sign \
  --key-file /secure/path/nahuali-signing-seed.hex \
  --output /separate/path/memory-checkpoint.json

nahuali --database memory attest-verify \
  /separate/path/memory-checkpoint.json
```

The checkpoint is useful only if the operator retains a trusted copy outside
the memory store. A current valid receipt does not by itself prove freshness;
the operator or deployment must know which checkpoint is the latest accepted
one.

## Recovery and derived data

Current projections, graph tables, optional snapshots, and Qdrant vectors are
derived state. They are not authoritative memory. `reconcile` verifies the
ledger and rebuilds the derived tiers. Backup and restore preserve the record
ledger, while semantic vectors should be rebuilt.

## Security boundaries

- Nahuali is not a secret manager. Do not store passwords, API keys, tokens, or
  customer secrets in a memory database.
- Scope labels separate contexts for recall; they are not authorization or
  tenant-isolation controls.
- The local HTTP API has no authentication. Do not expose it to an untrusted
  network.
- Persistent memory requires SurrealDB. Qdrant is optional and derived.
- `--no-default-features` produces a legacy compatibility build without the
  default attestation and hash-chain guarantees.
- Key compromise, malicious source observations, host compromise, and loss of
  the latest trusted checkpoint remain outside what a ledger can solve alone.

## Reproduce the contract

The zero-setup product demo exercises real public core functions:

```bash
nahuali demo
```

The governance fixtures and public contract tests cover authority calibration,
knowledge-health detection, non-mutating self-inspection, and integrity tiers:

```bash
cargo test -p nahuali-core --test public_contract
bash scripts/run-governance-benchmarks.sh --check
```

See [Governance Benchmark Methodology](GOVERNANCE_BENCHMARKS.md) for fixture
construction, formulas, current results, and limitations.
