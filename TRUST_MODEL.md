# Nahuali trust model

Nahuali makes the basis for trusting agent memory inspectable. It does not
claim that a stored sentence is true merely because it was recorded.

The trust model has four separate questions:

1. **Can this result be used?** Authority-aware recall evaluates the result's
   evidence, confidence, freshness, contradictions, and review state.
2. **Does the store need attention?** Self-inspection reports unsupported,
   stale, contradictory, or isolated memory without changing it.
3. **Do the store's internal history checks pass?** Event checksums, sequence
   validation, and the default hash chain detect different structural failures;
   Merkle paths establish membership under a supplied root.
4. **Is that exact ledger state independently authorized?** Version 2 signed
   checkpoints are accepted only under a separately held operator policy.

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
statement. Nahuali can show which recorded observation a claim cites and how
that claim was evaluated; it cannot prove the external world matched the
observation.

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
| Hash chain | A changed non-tip event when the later suffix is not also recomputed | A last-event rewrite, truncation, rollback, or a full rewrite followed by re-chaining the suffix |
| Merkle root and strict inclusion proof | Whether one selected chain hash is committed under one root | Who authorized the root, factual truth, or freshness |
| Version 2 signed checkpoint plus external policy | Full re-chaining, wrong lineage, unauthorized signers, and rollback relative to the checkpoint supplied | A newer valid checkpoint being withheld, key compromise, or external truth |
| Portable claim receipt | Commitment and provenance linkage for one claim, its episode, and optional source | Claim truth, authorship, source authenticity, source bytes, or an external timestamp |

The chain and Merkle root are automatic in default binaries. Signing is an
explicit operator action because the private key, policy, and accepted
checkpoint must remain under operator control:

```bash
nahuali --database memory checkpoint-policy-init \
  --origin workstation-1 \
  --key-id operator-1 \
  --key-file /secure/path/nahuali-signing-seed.hex \
  --minimum-signatures 1 \
  --output /separate/path/memory-policy.json

nahuali --database memory checkpoint-sign \
  --policy /separate/path/memory-policy.json \
  --key-id operator-1 \
  --key-file /secure/path/nahuali-signing-seed.hex \
  --output /separate/path/memory-checkpoint.json

nahuali --database memory checkpoint-verify \
  /separate/path/memory-checkpoint.json \
  --policy /separate/path/memory-policy.json \
  --mode current
```

The policy is the trust root; the checkpoint never authorizes its own key.
Current mode requires an exact live-tip match. Historical mode verifies the
checkpointed prefix and reports later, uncovered events instead of pretending
the old checkpoint covers them.

The earlier `attest-sign`/`attest-verify` chain-tip format remains available for
compatibility. A supplied version 1 attestation does not become trusted merely
because its embedded key verifies the signature. Trust reports and anchored
audits require an external keyring before treating that signer as authorized.

The checkpoint is useful only if the operator retains its policy and accepted
state outside the memory store. A cryptographically valid checkpoint does not
by itself prove freshness: the verifier must know which checkpoint or monotonic
tree-size floor is the latest one it accepted.

### Portable claim receipts

One claim and its provenance path can be detached from the database without
copying the rest of the memory store:

```bash
CLAIM_ID="$(nahuali --database memory data --json | jq -r '.claims[-1].id')"
nahuali --database memory receipt-export \
  --claim-id "$CLAIM_ID" \
  --checkpoint /separate/path/memory-checkpoint.json \
  --policy /separate/path/memory-policy.json \
  --output /separate/path/claim.receipt.json

nahuali receipt-verify /separate/path/claim.receipt.json \
  --policy /separate/path/memory-policy.json
```

The verifier checks strict JSON shape, supported event versions, event
checksums and identities, Merkle proof topology, checkpoint authorization, and
the exact claim-to-episode-to-source linkage. Its output deliberately separates
`receipt_integrity` from `content_authority`: integrity can be verified while
truth and external source authenticity remain unestablished.

Offline verification does not replay the complete ledger prefix behind the
signed root. It trusts the authorized signers' commitment to that root and
checks only the selected envelopes and inclusion paths. Use `checkpoint-verify`
with the ledger when complete-prefix integrity matters. Receipt v1 exports only
direct `FactAsserted` claims. Because a receipt contains the selected claim,
episode, and optional source metadata verbatim, protect it as memory data rather
than treating it as a public proof by default.

### Why this is not a blockchain

Nahuali uses transparency-log primitives inside a single-owner append-only
ledger: a hash chain, Merkle commitments, compact consistency proofs, and signed
checkpoints. It has no peer-to-peer consensus, mining, token, replicated public
state, or automatic global ordering. Calling it a blockchain would overstate
both the implementation and its guarantees.

## Recovery and derived data

Current projections, graph tables, optional snapshots, and Qdrant vectors are
derived state. They are not authoritative memory. Graph projection v2 uses a
permanent lock row with monotonically increasing fencing tokens: every mutation
batch conditionally updates that row in the same transaction as the projected
rows. A replaced owner therefore loses with a typed error and its whole batch is
rolled back. Successful rebuilds finish with a canonical SHA-256 content
manifest for every ledger-derived projected table plus the exact ledger tip and
schema version.

SurrealDB projection-backed entity, timeline, pending-work, and health reads
validate that checkpoint and refuse to serve while a rebuild is active or if
counts, content, schema version, or ledger tip no longer match.
`reconcile` verifies the ledger and rebuilds the derived tiers. Backup and
restore preserve the record ledger, while semantic vectors should be rebuilt.

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
- Independent checkpoint witnesses and gossip are not implemented in this
  beta. Without them, two verifiers do not automatically learn that they were
  shown inconsistent checkpoints.

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
