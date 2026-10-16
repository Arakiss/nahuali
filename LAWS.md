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
| 1 | Admitted confidence is a finite number in `0.0..=1.0` | PROVEN | all 2³² `f32` values | 0.3 s |
| 2 | Recall scores are bounded and monotone in confidence | PROVEN (bounds, score ≥ relevance, admission and addition monotone) + TESTED (multiplication step, recency decay) | all valid policies, all `f32` relevance/confidence/recency | 100 s (4 harnesses: 86.1 + 1.4 + 12.3 + 0.2) |
| 3 | A superseded claim never ranks above current memory | PROVEN (3 results) + TESTED (≤ 64 results) | 3 candidates, every field symbolic | 69 s |
| 4 | Supersession always points to a current, newer, evidence-backed claim | PROVEN (group rule, 4 facts) + TESTED (≤ 12 claims, grouping and lifecycles) | 4 facts, any `u8` value, any `Option<u8>` episode, any `u64` timestamp | 117 s |
| 5 | Forgetting leaves no orphan provenance and never erases the audit trail | PROVEN (cascade, 4 records) + TESTED (≤ 10 ledger records, engine end-to-end) | 4 records × 2 of 4 ids × 2 of 9 citations, 1–2 targets, all 16 rival subsets | 250 s |
| 6 | The ledger is append-only, even under forget | TESTED | not model-checked (SHA-256, SurrealKV) | n/a |

Times are measured CBMC verification time per harness on a 4-vCPU, 16 GB Linux VM
with Kani 0.68.0 and CBMC 6.11.0, running one harness at a time. Building the
crate under Kani adds a few minutes on a cold cache. The whole gate,
`scripts/verify-laws.sh` (law tests, then all 8 harnesses one after another),
took 9 min 41 s wall time on a warm build cache, of which 536 s was CBMC. Peak
memory was under 3 GB per harness.

## Running the laws

```bash
cargo install --locked kani-verifier --version 0.68.0
cargo kani setup
bash scripts/verify-laws.sh            # CI gate: every harness and law test
cargo kani -p nahuali-core --lib -Z stubbing --harness laws::proofs::law_confidence_admission
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

The proven parts hold under IEEE-754 rounding, not just in real arithmetic. Ranking
applies the same admission rule as the ledger (law 1), so caller-supplied
projections passed to `recall_projection_with_authority` with an out-of-range
or NaN confidence cannot outrank well-formed memory. The sort now uses
`f32::total_cmp`, so the comparator is a total order for any score.

- kani: `law_score_components_bounded`. Covers every valid policy, every finite
  non-negative relevance, every `Option<f32>` confidence, and every recency in
  `[0, recency_weight]`.
- kani: `law_score_total_at_least_relevance`. `total()` is never below
  relevance, for any components inside the bounds above.
- kani: `law_confidence_admission_is_monotone`. For any non-NaN
  `lower <= higher`, `admitted_confidence(lower) <= admitted_confidence(higher)`.
- kani: `law_score_addition_is_monotone`. For finite `0 <= lower <= higher` and
  a finite non-negative term, `lower + term <= higher + term`, and addition
  commutes. `total()` is `((relevance + confidence) + evidence) + recency`, so
  three applications of this step (with commutativity for the first) show that
  a larger confidence part never gives a smaller total. That composition is
  done by hand, not by Kani.
- test: `law_recency_bounded_and_non_increasing_in_age`. The recency decay
  calls `f64::powf`.
- test: `law_score_bounded_and_monotone_in_confidence`. End to end through the
  real `recency`, with arbitrary `f32` confidences, including NaN and ±∞. This
  is the only check of monotonicity of the confidence part itself.

**What is not proven.** The confidence part is `admitted(c) * confidence_weight`,
and "multiplying by the same non-negative weight preserves order" is the one
step Kani could not finish: that query alone, and the direct
`total()`-is-monotone query, ran 17 to 30 minutes with CaDiCaL and Kissat
without an answer. IEEE-754 multiplication by a non-negative number is
monotone (rounding is monotone), and the property test checks it, but within
this PR it is TESTED, so law 2's end-to-end monotonicity is TESTED while the
bounds and the "never below relevance" part are PROVEN.
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
  contested), under any valid policy. `recency` is stubbed
  (`-Z stubbing`) with "any value in `[0, recency_weight]`", the bound law 2
  tests: run through CBMC's `pow` model, the unstubbed harness reports a NaN
  score that native execution cannot produce (see law 2). The order this law
  states does not depend on the decay curve, so the stub only widens the
  inputs.
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

The rule lives in `inspection::resolve_group`, which decides one
scope/subject/predicate group: settled (the values agree, or every fact comes
from one source episode), superseded (a unique newest fact with provenance
retires every fact that disagrees with it and comes from another episode), or
contested. `classify_fact_groups` only groups facts and turns each decision into
a report, and `claim_lifecycles` turns the reports into lifecycles.
`resolve_group` is generic over `Observation`, meaning the value, episode, and
time it reads, so Kani can check the same code on integers; production
instantiates it with `Claim`.

- kani: `law_supersession_points_to_current_claim`. `resolve_group` on a group
  of 4 facts, each with any `u8` value, any `Option<u8>` episode, and any `u64`
  timestamp. That covers every equality pattern of values and episodes over 4
  facts, timestamp ties anywhere, and missing provenance. The check:
  supersession has a unique, strictly newest winner with provenance and
  retires exactly the facts that differ from it in both value and episode;
  settled groups agree or are one observation; contested groups have no clean
  winner.
- test: `law_supersession_points_to_current_claim`. The same group check on
  every group, plus the full statement above through `claim_lifecycles`, on up
  to 12 real `Claim`s over 2 subjects, 3 values, 4 provenance options, 3
  scopes, and dense timestamp ties. Grouping, scope separation, and the
  mapping to lifecycles are covered here, not by Kani.

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
- nothing is redacted that the references do not force: the redacted records
  are the least set closed under the cascade, so unrelated memory survives
  even when it sits in a citation cycle;
- `forgotten_ids` names exactly the targets and the memory held by the
  redacted records, neither more nor less.

"Provenance" here means the references `forget::derived_from` follows: an
episode's source, a claim, link, procedure, or intention's source episode,
review evidence, and repair evidence. Intention `depends_on` and `goal_id` are
planning references, not provenance. Forgetting an intention leaves a dependent
intention in place, and inspection reports it as `MissingDependency`. That is
deliberate, not an orphan.

The cascade lives in `forget::redaction_closure`, which takes each live record
reduced to its index, event id, held ids, and citations. `plan_forget` only
validates the request, extracts those references with `memory_ids` and
`derived_from`, and converts the result back to ledger indexes. The cascade is
generic over the id type, the id list, and the id set (`IdSet`), so Kani checks
the same code over small integers, fixed arrays, and a bitset; production
instantiates it with `&str`, `Vec`, and `BTreeSet`. The check also proves the
redaction is the *least* one: any set of records that the cascade cannot
escape contains every redacted record, so nothing is erased that the
references do not force.

- kani: `law_forget_leaves_no_orphan_provenance`. `redaction_closure` on 4
  records. Each holds 2 ids out of 4 memory ids, and cites 2 references out
  of the 4 memory ids, the 4 event ids, and one id nobody holds. Repeats and
  the unheld id stand for "one id" and "no citation", so this covers every
  record with one or two ids and zero to two citations, including forward,
  self, and cyclic references, and every request for one or two targets. The
  rival set is any of the 16 subsets of records.
- test: `law_forget_cascade_is_closed_and_least`. The same check with
  `BTreeSet`, up to 8 records with up to 3 citations each, against every
  rival subset.
- test: `law_forget_leaves_no_orphan_provenance`. End to end through
  `plan_forget` on up to 10 real ledger records over all 7 record shapes
  (source, episode, claim, review, intention update, repair, and redaction
  stub), with up to 2 targets including padded, blank, unknown, and repair ids.
  It checks request validation, that only live records are redacted, and the
  cascade check against every rival subset.
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

## How the proofs were made tractable

The first harnesses ran the engine functions on real `Claim`s and ledger
records and never finished: supersession was still in symbolic execution after
17 minutes, and forget ran out of memory at 6.5 GB with a single target. The
causes were in the data structures, not the laws. Symbolic keys in a
`BTreeMap` or `BTreeSet` make every node shift a symbolic-offset copy, and
`collect` bulk-sorts first. `String` comparisons unroll `memcmp`, because the
length stays symbolic. Iterator adapters that hold a reference in their state
merge pointers across iterations, so CBMC loses every slice bound.

So the decision logic was separated from the collections it runs on. This is
refactoring, not a behaviour change, and every existing test still passes:

- `inspection::resolve_group` decides one group and is generic over
  `Observation`. `classify_fact_groups` keeps the grouping.
- `forget::redaction_closure` computes the cascade and is generic over the id,
  the id list, and the `IdSet`. Its fixpoint loop is bounded by
  `live.len() + 1` passes, which it provably needs, instead of `loop`.
  `plan_forget` keeps validation and extraction.

Kani checks the generic code on integers, arrays, and a bitset. The property
tests run the production instance (`Claim`, `&str`, `Vec`, `BTreeSet`) through
the same checks. What Kani does not see is std's `BTreeSet`, `Vec`, and `str`
equality, and the extraction of references from ledger payloads, which the
tests cover.

Each checker was also validated against deliberate bugs:

| Mutant | Caught by |
| ------ | --------- |
| `confidence.clamp(0.0, 1.0)` restored (NaN admitted) | Kani `law_confidence_admission`, regression test |
| ranking sorts by score before lifecycle | property test `law_superseded_never_outranks_current` |
| supersession ignores the different-episode rule | Kani and property test `law_supersession_points_to_current_claim` |
| supersession accepts a tie at the newest timestamp | Kani `law_supersession_points_to_current_claim` |
| forget cascade skips event-id citations | property test `law_forget_leaves_no_orphan_provenance` |
| forget cascade redacts two records citing each other with no target reaching them | Kani `law_forget_leaves_no_orphan_provenance`, property test `law_forget_cascade_is_closed_and_least` (the 256-case end-to-end test alone missed it) |

## Honest limits

- **Bounds are small.** Kani covers 3 ranked results, groups of 4 facts, and
  4 records with 2 ids and 2 citations each. Bugs that need more elements are
  reachable only by the property tests. Those cover larger sizes but are
  random samples, not proofs.
- **Proofs are over the functions, not the whole engine.** Kani verifies
  `admitted_confidence`, `score_breakdown`, `RecallScoreBreakdown::total`,
  `rank`, `resolve_group`, and `redaction_closure`. It does not verify the
  grouping in `classify_fact_groups`, the mapping to lifecycles in
  `claim_lifecycles`, reference extraction and validation in `plan_forget`,
  candidate collection, scope and time filters, or truncation. Those are
  covered by the property tests, the existing unit tests, and the end-to-end
  law tests.
- **Kani sees model instances.** The supersession and forget proofs run the
  production generic code on integers, arrays, and a bitset instead of
  `String`, `Vec`, and `BTreeSet`. They rely on those std types behaving as
  sets and lists, which the property tests exercise but Kani does not prove.
- **Assumed preconditions.** The harnesses assume relevance is finite and
  non-negative. `recall_with_options` builds it from term counts and bonuses,
  so this holds, but it is not proven. Within a group, facts are told apart by
  identity, and the property tests use distinct claim and event ids, as the
  engine's id generator guarantees.
- **Assume-guarantee step in law 3.** The ranking proof replaces `recency` with
  its tested contract (a value in `[0, recency_weight]`). If the real decay
  ever left that range, law 3's proof would not notice; the law 2 property
  test would.
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
