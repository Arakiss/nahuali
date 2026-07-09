# Governance Benchmark Methodology

Nahuali's governance benchmarks are first-party release gates for memory
trustworthiness. They are reproducible checks over fixed corpora, not third-party
certifications and not recall-accuracy leaderboards.

The benchmarks answer a narrower question:

> Given a labeled integrity, provenance, health, attestation, or trust-gate
> scenario, does the current engine produce the expected machine-verifiable
> verdict?

They deliberately publish the corpus shape, formula, command, and limits so the
numbers can be recomputed from a checkout instead of trusted as prose.

## Why This Is A Separate Axis

Established agent-memory benchmarks such as LOCOMO, LongMemEval, and BEAM are
valuable recall and answer-quality tests: they ask whether a memory system can
find or reason over past context. Nahuali's governance suite asks whether the
memory substrate exposes enough evidence to trust that context before a caller
acts on it.

That second axis matters because persistent memory is now both a product feature
and a security boundary. OWASP ASI06 treats memory poisoning as an agentic
application risk, while EU AI Act Article 12 points high-risk systems toward
automatic lifecycle logging. Nahuali's benchmarks do not claim compliance or
complete poisoning defense; they check the deterministic controls this repository
can prove: ledger integrity, provenance coverage, contradiction and staleness
signals, attestation lifecycle behavior, and trust-verdict calibration.

For the prior-art and market context behind this benchmark gap, see
[Agent-Memory Governance Landscape](MEMORY_GOVERNANCE_LANDSCAPE.md).

## Running The Suite

Run the full governance benchmark gate:

```bash
bash scripts/verify-governance-benchmarks.sh
```

Run individual reports:

```bash
cargo run -p nahuali-regression --features attestation -- --livr
cargo run -p nahuali-regression --features attestation -- --arp
cargo run -p nahuali-regression -- --fixtures fixtures/provenance-coverage-regression.json
cargo run -p nahuali-regression -- --fixtures fixtures/contradiction-staleness-regression.json
cargo run -p nahuali-regression -- --fixtures fixtures/trust-verdict-soundness-regression.json
```

Store-backed fixture runs require the local SurrealDB development stack used by
the normal validation scripts:

```bash
docker compose up -d
```

The CI release/install gate runs the same script through
`scripts/validate-clean-tree.sh`.

## Common Rules

Each benchmark follows the same discipline:

- **Fixed corpus.** Inputs are deterministic and checked into the repository or
  built directly in `nahuali-core`.
- **Real engine path.** The benchmark calls the same validators, projection,
  recall, attestation, or health logic used by the CLI and library.
- **Labeled expected outcome.** Each case states what should be detected,
  accepted, warned, blocked, or left clean.
- **Scriptable report.** The regression runner emits JSON or pass/fail fixture
  reports suitable for CI.
- **No hidden averaging.** Detector-tier misses and false positives are reported
  where they matter.

## LIVR: Ledger Integrity Verification Rate

**Source:** `crates/nahuali-core/src/livr.rs`

**Command:**

```bash
cargo run -p nahuali-regression --features attestation -- --livr
```

**Formula:** `TP / (TP + FN)` per detector tier, rounded to two decimals.

**Corpus:** one clean chained control plus nine tampering classes over a
synthetic six-record ledger.

| Attack class | Description | Weakest detector expected to catch it |
|---|---|---|
| `checksum_mutation` | Stored per-event checksum is corrupted. | checksum-only |
| `payload_edit_no_recompute` | Event payload is edited while checksum is left stale. | checksum-only |
| `in_place_rewrite` | Event payload is rewritten and its own checksum recomputed, preserving the recorded chain link. | replay-chain |
| `timestamp_skew` | Event timestamp is changed and checksum recomputed, preserving the recorded chain link. | replay-chain |
| `sequence_gap` | A middle event is removed, leaving non-contiguous sequence numbers. | replay-chain |
| `cross_ledger_graft` | An event from another ledger is inserted with a valid self-checksum but foreign chain link. | replay-chain |
| `chain_strip` | Hash-chain links are removed while per-event checksums remain valid. | replay-chain in strict mode |
| `suffix_rechain` | A middle event is rewritten and the suffix is fully re-chained. | attestation-tip |
| `payload_truncation_rechain` | A payload is redacted and the suffix is fully re-chained. | attestation-tip |

Current expected report:

| Detector tier | True positives | False negatives | False positives | Detection rate |
|---|---:|---:|---:|---:|
| checksum-only | 2 | 7 | 0 | 0.22 |
| replay-chain | 7 | 2 | 0 | 0.78 |
| attestation-tip | 9 | 0 | 0 | 1.00 |

**Important interpretation:** replay-chain in LIVR is the strict detector. The
default validator remains compatible with pre-chain legacy records; operators
use `validate --require-chained` or `backup-validate --require-chained` when a
deployment must fail closed on stripped links.

## PCR: Provenance Coverage Rate

**Fixture:** `fixtures/provenance-coverage-regression.json`

**Command:**

```bash
cargo run -p nahuali-regression -- --fixtures fixtures/provenance-coverage-regression.json
```

**Formula:** `evidence_backed / total_assertional_memory`.

The fixture seeds a known mix of evidence-backed and unsupported claims. The
runner verifies:

- provenance coverage is `0.75`;
- overconfidence rate is `0.25`;
- unsupported high-confidence claims are surfaced;
- insufficient sample guards prevent misleading scores on tiny stores.

PCR checks whether assertional memory cites an observation. It does not prove the
observation is true, that the claim is semantically correct, or that the model
extracted the claim well.

## CDR: Contradiction And Staleness Detection Rate

**Fixture:** `fixtures/contradiction-staleness-regression.json`

**Command:**

```bash
cargo run -p nahuali-regression -- --fixtures fixtures/contradiction-staleness-regression.json
```

**Formula:** labeled health defects detected over labeled health defects.

The fixture covers:

- cross-observation contradictions (values disagreeing across distinct episodes
  that cannot be cleanly ordered — two values from one episode are a deliberate
  multi-valued observation, not a defect);
- recency-resolved supersessions;
- deterministic stale facts;
- a clean control with zero expected false positives.

CDR measures only the defect classes implemented by the health pipeline. It is
not a complete inconsistency detector for every possible memory graph.

## ARP: Attestation Recovery Profile

**Source:** `crates/nahuali-core/src/arp.rs`

**Command:**

```bash
cargo run -p nahuali-regression --features attestation -- --arp
```

ARP is a pass/fail profile rather than a scalar score. The matrix verifies that:

- a live receipt is honored;
- a re-chained suffix invalidates the old receipt;
- a rotated key can sign a valid new receipt;
- a revoked key's cryptographically valid receipt is rejected;
- a receipt for another ledger's tip is rejected.

ARP tests keyring and ledger-tip behavior. It does not claim anything about
operational key custody or the general cryptographic strength of Ed25519.

## TVS: Trust Verdict Soundness

**Fixture:** `fixtures/trust-verdict-soundness-regression.json`

**Command:**

```bash
cargo run -p nahuali-regression -- --fixtures fixtures/trust-verdict-soundness-regression.json
```

TVS verifies that labeled stores land in the expected recall authority mode:

- clean connected memory certifies;
- isolated or weakly connected context degrades to advisory;
- unsupported assertions warn;
- cross-observation contradictions block (two values recorded against one
  episode are a deliberate multi-valued observation and do not block).

TVS checks the gate calibration over known stores. It does not prove that every
future store will be classified perfectly.

## Release-Gate Regressions

The governance gate also runs:

- `fixtures/knowledge-health-regression.json`
- `fixtures/recall-regression.json`

These are contract regressions for health and recall behavior. They are included
in the release gate because a trust product can regress even if the five named
governance benchmarks still pass.

## Limits

These benchmarks are intentionally conservative:

- They are synthetic. They verify known classes, not all possible attacks or
  knowledge failures.
- They are first-party. They are useful for reproducibility and release gating,
  not independent certification.
- They measure governance behavior. They do not replace recall-quality
  benchmarks such as LOCOMO, LongMemEval, or BEAM.
- They are code-coupled. A published number is meaningful only when tied to a
  concrete commit or release.

For that reason, quote a benchmark result with the command, commit or release,
and report JSON when possible.
