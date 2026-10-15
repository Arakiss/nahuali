# Laws

A law is an invariant the engine must keep for every input, stated in plain
language and backed by a machine check that CI runs on every pull request.
The checks live in [`crates/nahuali-core/src/laws.rs`](crates/nahuali-core/src/laws.rs).
Each law is written once, as a `check_*` function or a harness body, and run two
ways:

- **PROVEN** means a [Kani](https://github.com/model-checking/kani) harness
  (`#[kani::proof]`) checks the law for *every* input inside the stated bounds.
  Inputs come from `kani::any()`, which is fully symbolic and covers every bit
  pattern of every `f32` and `u64` and every choice among the listed strings.
  Kani passing means "no counterexample exists within these bounds". It does
  not mean "no counterexample exists at any size".
- **TESTED** means a [proptest](https://github.com/proptest-rs/proptest)
  property test, or an exhaustive enumeration, checks the law on many random or
  enumerated inputs larger than Kani can reach. This is evidence, not proof.

No external law or contract library is involved. The laws are plain Rust
assertions over the production functions: Kani and proptest only generate
inputs.

## Summary

| # | Law | Status | Kani bounds | Kani CBMC time |
| - | --- | ------ | ----------- | -------------- |
| 1 | Admitted confidence is a finite number in `0.0..=1.0` | PROVEN | all 2³² `f32` values | KANI_TIME_1 |
| 2 | Recall scores are bounded and monotone in confidence | PROVEN (arithmetic) + TESTED (recency decay) | all valid policies, all `f32` relevance/confidence/recency | KANI_TIME_2 |
| 3 | A superseded claim never ranks above current memory | PROVEN (3 results) + TESTED (≤ 64 results) | 3 candidates, every field symbolic | KANI_TIME_3 |
| 4 | Supersession always points to a current, newer, evidence-backed claim | PROVEN (3 claims) + TESTED (≤ 12 claims) | 3 claims, 2 values, 3 provenance options, 2 scopes, any `u64` timestamps | KANI_TIME_4 |
| 5 | Forgetting leaves no orphan provenance and never erases the audit trail | PROVEN (3 records, 2 targets) + TESTED (≤ 10 records, engine end-to-end) | 3 records × 7 record shapes × 3 ids × 7 citations, 2 targets | KANI_TIME_5 |
| 6 | The ledger is append-only, even under forget | TESTED | not model-checked (SHA-256, SurrealKV) | n/a |

Times are measured CBMC verification time per harness on a 4-vCPU, 16 GB Linux VM
with Kani 0.68.0 and CBMC 6.11.0, running one harness at a time. Building the
crate under Kani adds a few minutes on a cold cache. KANI_TOTAL

## Running the laws

```bash
cargo install --locked kani-verifier --version 0.68.0
cargo kani setup
bash scripts/verify-laws.sh            # CI gate: every harness and law test
cargo kani -p nahuali-core --lib --harness laws::proofs::law_confidence_admission
cargo test -p nahuali-core --lib laws::
```

`scripts/verify-laws.sh` fails closed. It fails when any harness or law test
fails, and also when the harnesses or tests in `laws.rs` differ from the
`kani:` and `test:` entries in this file, when a law test is `#[ignore]`d, or
when Kani's summary does not account for every harness. A law cannot silently
stop being checked. The CI job `laws` runs it, and `CI (all)` requires it.

## 1. Admitted confidence is a finite number in `0.0..=1.0`

**Statement.** Whatever confidence a caller passes when recording a claim,
link, procedure, imported record, or self-repair, the value written to the
ledger is a finite number between 0 and 1. A value already in range is stored
unchanged. NaN is rejected with `NahualiError::InvalidConfidence`, and nothing
is written.

**Counterexample found in the previous code: `f32::NAN`.** Every write path used
`confidence.clamp(0.0, 1.0)`, and `f32::clamp` returns NaN for NaN. The NaN was
committed to the ledger, where JSON encodes it as `null`. The ledger decoder then
rejects that record (`invalid type: null, expected f32`), so the write itself
reported a projection failure and **every later `MemoryEngine::open` of that
store failed**. One `add_claim(.., f32::NAN)` from the Rust API made the store
unopenable. The CLI was not affected, because it already rejects non-finite
`--confidence`, and JSON transports cannot carry NaN. The library API, the
interchange import, and self-repair proposals were affected.

**Fix.** `model::admitted_confidence` is the single admission rule, and all eight
write sites go through `admit_confidence`.

- kani: `law_confidence_admission`. Every `f32` bit pattern, including NaN
  payloads, infinities, subnormals, and `-0.0`. With the old `clamp` body, the
  same harness fails on NaN.
- test: `non_finite_confidence_is_rejected_before_it_reaches_the_ledger`. A
  regression test through the real engine: NaN is rejected on `add_claim`,
  `add_link`, and `add_procedure`, nothing is appended, infinity is clamped to
  `1.0`, and the store reopens.

## 2. Recall scores are bounded and monotone in confidence

**Statement.** A recall score is `relevance + confidence + evidence + recency`.
For every ranking policy that `RecallRankingPolicy::validate` accepts:

- the confidence part is between `0` and `confidence_weight`;
- the evidence part is exactly `0` or `evidence_bonus`;
- the recency part is between `0` and `recency_weight`, equals
  `recency_weight` for the newest result, and never grows as age grows;
- the score is never below relevance;
- raising confidence, with everything else equal, never lowers the score;
- a missing or NaN confidence contributes exactly `0`.

These hold under IEEE-754 rounding, not just in real arithmetic. Ranking
applies the same admission rule as the ledger (law 1), so caller-supplied
projections passed to `recall_projection_with_authority` with an out-of-range
or NaN confidence cannot outrank well-formed memory. The sort now uses
`f32::total_cmp`, so the comparator is a total order for any score.

- kani: `law_score_components_bounded`. Covers every valid policy, every finite
  non-negative relevance, every `Option<f32>` confidence, and every recency in
  `[0, recency_weight]`.
- kani: `law_score_confidence_component_monotone`. For any non-NaN
  `lower <= higher`, the confidence part does not decrease.
- kani: `law_score_total_monotone_and_at_least_relevance`. `total()` is monotone
  in the confidence part and is never below relevance. Together with the two
  harnesses above, and because `rank` sets `score = score_breakdown(..).total()`,
  this gives the full monotonicity statement. Splitting it this way keeps each
  SAT query small; the single combined query did not finish in 10 minutes.
- test: `law_recency_bounded_and_non_increasing_in_age`. The recency decay
  calls `f64::powf`.
- test: `law_score_bounded_and_monotone_in_confidence`. End to end through the
  real `recency`, with arbitrary `f32` confidences, including NaN and ±∞.
- test: `validate_agrees_with_the_kani_policy_domain`. Pins the inline policy
  domain the harnesses assume to `validate()`, over every combination of
  boundary weights.

**Why recency is TESTED, not PROVEN.** CBMC's built-in `pow` model is not
faithful to `libm`. On `recency_weight = 5.877475e-39` (subnormal),
`recency_half_life_ms = 29790816`, and `age = 2273136955814921472` it produced a
"violation" that native execution refutes, since the real result is exactly
`0.0`, inside the bound. Rust also documents `powf` precision as
platform-dependent. The `pow`-free arithmetic is proven, and the decay is
property-tested.

## 3. A superseded claim never ranks above current memory

**Statement.** Recall returns every matching result exactly once, and no
superseded claim appears before any non-superseded result: current or contested
claims, episodes, entities, links, procedures, or intentions. This holds
whatever the scores, confidences, timestamps, or policy are, including NaN and
infinite confidences in caller-supplied projections. Within each group,
results are ordered by non-increasing score. That last part is TESTED only.

- kani: `law_superseded_never_outranks_current`. Runs `recall::rank` on 3
  candidates, each with symbolic relevance, `Option<f32>` confidence, evidence
  flag, `u64` timestamps, and lifecycle (none, current, superseded, or
  contested), under any valid policy.
- test: `law_superseded_never_outranks_current`. Up to 64 candidates. This
  matters because Rust's `sort_by` switches from insertion sort to its general
  algorithm above 20 elements, which a 3-element proof does not exercise.

## 4. Supersession always points to a current, newer, evidence-backed claim

**Statement.** When a claim is marked superseded, its `superseded_by` names a
claim that:

- exists and is `Current`, so there are no chains or cycles of supersession;
- lists the retired claim in its `supersedes`;
- has the same scope, subject, and predicate, and a different value;
- was recorded strictly later (`superseded_at_ms` is that time);
- cites a source episode different from the retired claim's.

A claim that is not superseded carries no `superseded_by` and no
`superseded_at_ms`, and every projected claim gets a lifecycle. Recall reads
lifecycles from this same function (`inspection::claim_lifecycles`), so this
law is what keeps "returned as current" honest.

- kani: `law_supersession_points_to_current_claim`. 3 claims covering 2 values,
  no provenance or one of 2 source episodes, 2 scopes, and any `u64`
  timestamps. This includes timestamp ties, same-episode observations, and
  cross-scope pairs.
- test: `law_supersession_points_to_current_claim`. Up to 12 claims over 2
  subjects, 3 values, 4 provenance options, 3 scopes, and dense timestamp ties.

## 5. Forgetting leaves no orphan provenance and never erases the audit trail

**Statement.** For any ledger and any forget request with a reason:

- the request fails exactly when no non-blank target is given or a target
  names no live memory. A blank reason is always rejected; the existing unit
  tests in `forget.rs` cover that case;
- otherwise, every target is erased;
- no surviving record holds an erased memory id, or cites as its source or
  evidence an erased memory id or the event id of an erased record;
- only records holding live memory are redacted. A `memory_forgotten` audit
  record or an earlier redaction stub is never redacted;
- `forgotten_ids` names exactly the memory held by the redacted records,
  neither more nor less.

"Provenance" here means the references `forget::derived_from` follows: an
episode's source, a claim, link, procedure, or intention's source episode,
review evidence, and repair evidence. Intention `depends_on` and `goal_id` are
planning references, not provenance. Forgetting an intention leaves a dependent
intention in place, and inspection reports it as `MissingDependency`. That is
deliberate, not an orphan.

- kani: `law_forget_leaves_no_orphan_provenance`. Runs `forget::plan_forget` on
  3 records. Each record has one of 7 shapes: source, episode, claim, review,
  intention update (which shares the intention id), repair (which carries two
  ids), or an already-redacted stub. Each holds one of 3 memory ids and cites
  nothing, one of the 3 memory ids, or one of the 3 event ids, including
  forward and self references. There are 2 targets, covering whitespace,
  blank, and unknown cases.
- test: `law_forget_leaves_no_orphan_provenance`. Up to 10 records, 6 memory
  ids, and up to 2 targets.
- test: `law_ledger_is_append_only_even_under_forget`. Also asserts, through
  the real engine and store, that after `MemoryEngine::forget` the projection
  holds no erased episode or claim and no claim cites an erased episode (law 6).

## 6. The ledger is append-only, even under forget

**Statement.** After any engine operation, the earlier ledger is an unchanged
prefix of the new one. Each operation appends exactly one record. Every earlier
record keeps its id, sequence, timestamp, checksum, `prev_hash`, and chained
hash. A payload may change only during a forget, only from content to a
redaction stub, and only when a later `memory_forgotten` record authorizes it
(`verify_redactions`). The hash chain verifies. The earlier Merkle root has a
consistency proof to the new root (`verify_merkle_consistency_proof`), and
`ledger_append_only` accepts it. Separately, for every tree size up to 33 and
every prefix, inclusion and consistency proofs verify, and rewriting any leaf
of the prefix makes both checks fail.

- test: `law_ledger_is_append_only_even_under_forget`. Random sequences of up
  to 7 `remember`, `add_claim`, and `forget` operations against a real
  SurrealKV store with the default `attestation` features. The chain and Merkle
  parts run only with `tamper-evidence`.
- test: `law_merkle_commitments_prove_every_prefix_and_reject_rewrites`. An
  exhaustive enumeration over sizes 1 to 33, every prefix, every inclusion
  index, and every single-leaf rewrite.

**Why TESTED only.** The ledger lives in SurrealKV and its commitments are
SHA-256. Neither is within reach of a bounded model checker.

## Honest limits

- **Bounds are small.** Kani covers 3 results, 3 claims, and 3 records. Bugs
  that need more elements are reachable only by the property tests. Those
  cover larger sizes but are random samples, not proofs.
- **Proofs are over the functions, not the whole engine.** Kani verifies
  `admitted_confidence`, `score_breakdown`, `RecallScoreBreakdown::total`,
  `rank`, `claim_lifecycles`, and `plan_forget`. The glue that calls them, such
  as candidate collection, scope and time filters, and truncation, is covered
  by the existing unit tests and the end-to-end law tests, not by Kani.
- **Assumed preconditions.** The harnesses assume relevance is finite and
  non-negative. `recall_with_options` builds it from term counts and bonuses,
  so this holds, but it is not proven. Claim ids are distinct within a
  projection, as the engine's id generator guarantees.
- **Not covered.** The hybrid (Qdrant) recall path is not covered. It has its
  own superseded-last sort and was left unchanged. Concurrency is not covered:
  this PR does not claim "concurrent reads never observe partial writes". The
  existing `scripts/verify-projection-concurrency.sh` and store tests remain
  the evidence there. Backups, exports, receipts, and the semantic index after
  a forget are not covered either; `TRUST_MODEL.md` documents those forgetting
  limits.
- **Kani's models.** Results depend on Kani 0.68.0 and CBMC 6.11.0 modelling
  Rust semantics correctly. The `pow` episode above shows these models are not
  infallible, which is why the transcendental part is tested and the proven
  part avoids it.
