# Nahuali demo walkthrough

Two demos, both on synthetic data. The first needs nothing but Rust. The second
runs against the local stack and ends in a trust report you can open in a browser.

The point of both is the same: memory that can answer what do we know, why
trust it, what is missing, and was the history altered. The last one it can
prove.

Every database name below starts with `.local/`, which is gitignored, so nothing
you create here is tracked. Clean-up is at the end.

---

## Demo 1 — Tamper-evidence, zero dependencies (no Docker)

This one runs entirely in memory and offline. It builds a hash-chained ledger,
signs its tip, and then plays the attacker twice — first an in-place rewrite,
then a full re-chain — to show what each layer catches.

```bash
cargo run -p nahuali-core --example tamper_evidence --features attestation
```

Expected output (deterministic — the demo uses a fixed, non-secret seed):

```text
== Nahuali tamper-evidence demo ==

1. An append-only ledger of 4 chained events.
   Each event binds the previous event's chained hash.
   chain intact: true
   tip: seq 4 aff66e7daa32459e2be7e9feda4672405d408afdf7c40bc244de9ba5fb72f08b

2. The operator signs that tip with an Ed25519 key (the receipt).
   public key: 207a067892821e25d770f1fba0c47c11ff4b813e54162ece9eb839e076231ab6
   receipt verifies against the live tip: true

3. An attacker rewrites event 2 and recomputes its own checksum.
   per-event checksum still valid (checksum-only model fooled): true
   the chain catches it: broken link at record 3 (seq 3).

4. The attacker re-chains the entire suffix to repair every link.
   chain now reports intact: true
   but the tip changed: seq 4 24ccf21d0f1984cb82025018e5d099d88c37fc346ae27e2afb66532ce2566ff8
   the signed receipt no longer verifies: false
   forging a fresh receipt would require the operator's private key.

Checksum proves an event is internally consistent.
The chain proves the history was not rewritten in place.
The signed tip proves the history was not rewritten at all.
```

What each step shows:

1. An append-only ledger where every event binds the previous event's chained
   hash.
2. The operator signs the chain tip with an Ed25519 key. The receipt is
   portable and can live off the machine that holds the ledger.
3. The attacker rewrites a historical event and recomputes its self-contained
   checksum. A checksum-only model is fooled; the chain still catches the broken
   link.
4. The attacker re-chains the whole suffix so no link is broken. The chain now
   reports intact — but the tip moved, so the signed receipt no longer verifies.
   Forging a fresh receipt would require the operator's private key.

The source is annotated in [`../crates/nahuali-core/examples/tamper_evidence.rs`](../crates/nahuali-core/examples/tamper_evidence.rs).

---

## Demo 2 — The full trust report against the running stack

This one records a small memory, reads the composed trust report, signs a
checkpoint, and audits what changed since that checkpoint. It needs the local
SurrealDB + Qdrant stack and a CLI built with attestation.

### Prerequisites

```bash
# Start SurrealDB + Qdrant (needs Docker)
bash scripts/ensure-dev-stack.sh

# Build the CLI with the optional attestation surface
cargo build -p nahuali-cli --features attestation
```

The walkthrough below calls the built binary directly. Set it once:

```bash
BIN="target/debug/nahuali"
DB=".local/demo-sample"
```

### 1. Record a small, connected memory

One observed episode, one evidence-backed claim sourced to that episode, and one
typed link — enough that authority can certify it.

```bash
"$BIN" --database "$DB" remember "Lena owns the release notes for the 0.3 beta." \
  --tag product --mention Lena --mention "release notes"
"$BIN" --database "$DB" claim Lena owns "release notes" --confidence 0.92 --source-last
"$BIN" --database "$DB" link  Lena owns "release notes" --confidence 0.90 --source-last
```

```text
remembered episode_...
claimed claim_...
linked link_...
```

### 2. Read the trust report

One verdict over knowledge, authority, integrity, and health.

```bash
"$BIN" --database "$DB" trust-report
```

```text
Memory trust report
Trustworthy: yes
Knowledge: 3 events (1 episodes, 1 claims, 1 links, 0 procedures, 0 intentions, 0 sources, 2 entities)
Authority: Certify (score 1.00, can_trust yes)
Integrity: verified (checksums ok, sequence contiguous, chain intact)
Chain tip: <64-hex chain tip>
Health: 0 unsupported, 0 conflicting, 0 blind spots (avg confidence 0.92)
Reasons:
- ledger integrity verified
- authority certify (score 1.00)
```

The command exits non-zero if ledger integrity fails verification, so it can
gate CI. The broader `Trustworthy` verdict (which also folds in authority and
health) is reported, not gated.

### 3. Write the HTML receipt

The same report as a single self-contained HTML file — inline styles, system
fonts, zero network calls — so it renders offline and can be shared as evidence.

```bash
"$BIN" --database "$DB" trust-report --html examples/sample-trust-report.html
```

```text
Wrote trust report to examples/sample-trust-report.html
```

A pre-generated copy lives at
[`sample-trust-report.html`](./sample-trust-report.html) so you can open it
without running anything. Note: the public sample is written with `--attestation`
(step 4) so it also shows the signed-checkpoint row.

### 4. Sign a checkpoint and fold it into the report

Sign the current chain tip into a portable receipt, then verify it against the
report. The seed is a 32-byte Ed25519 key as hex; keep it off the machine that
holds the ledger.

```bash
openssl rand -hex 32 > .local/demo.key
"$BIN" --database "$DB" attest-sign --key-file .local/demo.key --output .local/checkpoint.json
"$BIN" --database "$DB" trust-report --attestation .local/checkpoint.json
```

```text
Signed the tamper-evident chain tip:
  sequence:   3
  tip:        <64-hex chain tip>
  public key: <64-hex public key>
  written to: .local/checkpoint.json

Memory trust report
Trustworthy: yes
Knowledge: 3 events (1 episodes, 1 claims, 1 links, 0 procedures, 0 intentions, 0 sources, 2 entities)
Authority: Certify (score 1.00, can_trust yes)
Integrity: verified (checksums ok, sequence contiguous, chain intact)
Chain tip: <64-hex chain tip>
Health: 0 unsupported, 0 conflicting, 0 blind spots (avg confidence 0.92)
Attestation: checkpoint at sequence 3 anchored
Reasons:
- ledger integrity verified
- authority certify (score 1.00)
- attested checkpoint at sequence 3 is anchored
```

### 5. Audit what changed since the signed checkpoint

Record one more memory, then diff the ledger from the signed checkpoint. The
lower bound is anchored on the verified receipt, so the diff starts from a point
you can prove was not altered.

```bash
"$BIN" --database "$DB" remember "Aaron reviews the changelog every Friday." \
  --tag process --mention Aaron
"$BIN" --database "$DB" claim Aaron reviews "changelog" --confidence 0.88 --source-last

"$BIN" --database "$DB" audit --from-attestation .local/checkpoint.json
```

```text
Anchor: signed checkpoint at seq 3 (verified)
Record ledger audit
Total events: 5
Range: seq 3 (exclusive) -> 5 (inclusive)
Changes in range: 2
Lower tip: <64-hex tip at the signed checkpoint>
Upper tip: <64-hex current tip>
Integrity: verified (checksums ok, sequence contiguous, chain intact)
By kind: 1 episodes, 1 facts
- seq 4 episode Aaron reviews the changelog every Friday.
- seq 5 fact Aaron reviews changelog
```

The audit anchors its lower bound on the verified receipt, lists exactly the two
records added since, and re-verifies integrity over the range. If the signed
checkpoint no longer matched the ledger, this command would refuse to anchor.

### Clean up

The `.local/` database and key files are gitignored, but you can drop them when
you are done. Database names are normalized for SurrealDB (`.local/demo-sample`
becomes `_local_demo_sample` in namespace `nahuali`).

```bash
rm -f .local/demo.key .local/checkpoint.json

# Drop the demo database (default endpoint localhost:18000, root:root)
curl -s -X POST "http://localhost:18000/sql" -u root:root \
  -H "Accept: application/json" \
  --data-binary "USE NS nahuali; REMOVE DATABASE IF EXISTS _local_demo_sample;"
```

---

## Regenerating the public sample

[`sample-trust-report.html`](./sample-trust-report.html) is checked in for the
instant-view aha. To regenerate it on the clean Certify state used above:

```bash
BIN="target/debug/nahuali"
DB=".local/demo-sample-clean"

"$BIN" --database "$DB" remember "Lena owns the release notes for the 0.3 beta." \
  --tag product --mention Lena --mention "release notes"
"$BIN" --database "$DB" claim Lena owns "release notes" --confidence 0.92 --source-last
"$BIN" --database "$DB" link  Lena owns "release notes" --confidence 0.90 --source-last

openssl rand -hex 32 > .local/demo-clean.key
"$BIN" --database "$DB" attest-sign --key-file .local/demo-clean.key --output .local/checkpoint-clean.json
"$BIN" --database "$DB" trust-report --attestation .local/checkpoint-clean.json \
  --html examples/sample-trust-report.html
```

Then prepend the header comment back to the file:

```text
<!-- Generated YYYY-MM-DD on synthetic data; regenerate with examples/DEMO.md -->
```

Clean up the same way as Demo 2 (`_local_demo_sample_clean`).
