# Nahuali demo walkthrough

Two demos, both on synthetic data. The first is the product tour and needs only
the released CLI. The second runs against the local stack and ends in a trust
report you can open in a browser.

Both answer the same questions: what does the memory contain, what supports it,
what needs review, and has the recorded history changed? The integrity layers
make the last answer verifiable.

The persistent walkthrough uses a dedicated SurrealDB database identifier, so
it stays separate from normal memory. Clean-up is at the end.

---

## Demo 1 — Governed memory, zero dependencies (no Docker)

This one runs entirely in memory and offline. It uses the production recall and
self-inspection policy to show a supported result, an unsupported result, and a
contradictory store. It then builds a hash-chained ledger and plays the attacker
twice to show what each integrity layer catches.

```bash
cargo run -p nahuali-cli -- demo
```

The output is deterministic and uses a fixed, non-secret signing seed. Its
opening sections demonstrate the product contract:

```text
1 · Recall returns evidence and a verdict.
    CERTIFY  Lena owns release notes
             evidence: episode_release_notes   can trust: yes
    WARN     Mateo owns deployment keys
             evidence: none   can trust: no

2 · The store inspects itself before anything is repaired.
    unsupported claims: 1   contradictions: 1   review required: yes
    overall authority: BLOCK   automatic write-back: no
```

The remaining sections show:

1. An append-only ledger where every event binds the previous event's hash.
2. An operator-held Ed25519 receipt for the current chain tip.
3. An in-place rewrite whose recomputed event checksum cannot repair the chain.
4. A full re-chain whose changed tip no longer matches the signed receipt.

The demo calls public functions from `nahuali-core`; it does not carry a second
trust implementation inside the CLI.

---

## Demo 2 — The full trust report against the running stack

This one records a small memory, reads the composed trust report, signs a
checkpoint, and audits what changed since that checkpoint. It needs the local
SurrealDB + Qdrant stack and a CLI built with attestation.

### Prerequisites

```bash
# Start SurrealDB + Qdrant (needs Docker)
bash scripts/ensure-dev-stack.sh

# The default CLI build includes tamper evidence and attestation
cargo build -p nahuali-cli
```

The walkthrough below calls the built binary directly. Set it once:

```bash
BIN="target/debug/nahuali"
DB="demo_sample"
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

The key files under `.local/` are gitignored, but you can drop them when you are
done, along with the demo database. A database name must be a valid SurrealDB
identifier (`[A-Za-z0-9_]`); this demo uses `demo_sample` in namespace `nahuali`.

```bash
rm -f .local/demo.key .local/checkpoint.json

# Drop the demo database (default endpoint localhost:18000, root:root)
curl -s -X POST "http://localhost:18000/sql" -u root:root \
  -H "Accept: application/json" \
  --data-binary "USE NS nahuali; REMOVE DATABASE IF EXISTS demo_sample;"
```

---

## Regenerating the public sample and README capture

[`sample-trust-report.html`](./sample-trust-report.html) is checked in for the
instant-view aha. To regenerate it on the clean Certify state used above:

```bash
BIN="target/debug/nahuali"
DB="readme_evidence"

"$BIN" --database "$DB" ingest-text examples/source-note.md \
  --kind note --title "Release notes source" --chunking paragraphs \
  --tag product --mention Lena --role release-review --scope project:Nahuali
"$BIN" --database "$DB" claim Lena owns "release notes" \
  --confidence 0.96 --source-last --scope project:Nahuali
"$BIN" --database "$DB" link Lena owns "release notes" \
  --confidence 0.94 --source-last --scope project:Nahuali
"$BIN" --database "$DB" procedure "Evidence-backed release notes" \
  "Keep release notes concise and cite the source episode." \
  --confidence 0.95 --source-last --scope project:Nahuali
"$BIN" --database "$DB" intention "Publish verified release notes" \
  --kind task --priority high --source-last --scope project:Nahuali

openssl rand -hex 32 > .local/demo-clean.key
"$BIN" --database "$DB" attest-sign --key-file .local/demo-clean.key --output .local/checkpoint-clean.json
"$BIN" --database "$DB" trust-report --attestation .local/checkpoint-clean.json \
  --html examples/sample-trust-report.html
```

The README image at `assets/nahuali-trust-report.png` is a browser capture of
this exact self-contained HTML output at a 1440-pixel viewport. It shows eight
events, one source, a `Certify` authority verdict, no health defects, and the
anchored checkpoint at sequence eight. Do not edit values into either artifact;
regenerate the report from the synthetic ledger and recapture it.

Clean up the same way as Demo 2, using the `readme_evidence` database name.
