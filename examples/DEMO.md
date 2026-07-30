# Nahuali demo walkthrough

These demos use synthetic data only. The first explains the trust model without
creating a store. The second exercises the real CLI, embedded database, TUI,
external trust policy, signed checkpoint, and portable claim receipt.

## Demo 1 — The trust model, offline

```bash
nahuali demo
```

The command is deterministic and non-mutating. It shows:

1. evidence-backed recall reaching `CERTIFY`;
2. an unsupported claim being refused;
3. contradiction detection without automatic rewriting;
4. an in-place ledger edit breaking the hash chain;
5. a fully re-chained suffix no longer matching the retained checkpoint.

The demo calls the same public `nahuali-core` functions as the product. It does
not carry a second trust implementation in the CLI.

## Demo 2 — A persistent, verifiable memory

The default embedded store needs no Docker. Use a disposable home so this run
cannot touch normal memory:

```bash
cargo build -p nahuali-cli

BIN="$PWD/target/debug/nahuali"
DEMO_DIR="$(mktemp -d "${TMPDIR:-/tmp}/nahuali-demo.XXXXXX")"
export NAHUALI_HOME="$DEMO_DIR/home"
DB="demo_sample"
```

### 1. Record evidence, then derive memory

```bash
"$BIN" --database "$DB" remember \
  "Lena owns the release notes." \
  --tag product --mention Lena --mention "release notes" \
  --scope project:Nahuali

CLAIM_ID="$(
  "$BIN" --database "$DB" claim Lena owns "release notes" \
    --confidence 0.92 --source-last --scope project:Nahuali --json \
    | jq -r '.id'
)"

"$BIN" --database "$DB" link Lena owns "release notes" \
  --confidence 0.90 --source-last --scope project:Nahuali
```

The claim and link point to the episode that supports them. They are not trusted
merely because their text looks plausible.

### 2. Inspect the three trust axes

```bash
"$BIN" --database "$DB" explore
```

The header keeps content authority, ledger integrity, and external anchoring
separate. Press `/` and search for `Lena`; use `j`/`k` to inspect the claim and
its evidence.

A machine-readable snapshot is available without opening the TUI:

```bash
"$BIN" --database "$DB" trust-report --json | jq '{
  trustworthy,
  authority,
  integrity,
  health,
  verdict_reasons
}'
```

### 3. Authorize a retained checkpoint

The compatibility `attest-sign` format signs a chain tip. A signature becomes
an external trust anchor only when its public key is active in a separately held
operator keyring:

```bash
openssl rand -hex 32 > "$DEMO_DIR/operator.seed"
chmod 600 "$DEMO_DIR/operator.seed"

"$BIN" --database "$DB" attest-sign \
  --key-file "$DEMO_DIR/operator.seed" \
  --output "$DEMO_DIR/tip-attestation.json"

PUBLIC_KEY="$(jq -r '.public_key' "$DEMO_DIR/tip-attestation.json")"
jq -n --arg public_key "$PUBLIC_KEY" '{
  keys: [{
    key_id: "demo-operator",
    public_key: $public_key,
    status: "active"
  }]
}' > "$DEMO_DIR/keyring.json"

"$BIN" --database "$DB" attest-verify \
  "$DEMO_DIR/tip-attestation.json" \
  --keyring "$DEMO_DIR/keyring.json"

"$BIN" --database "$DB" trust-report \
  --attestation "$DEMO_DIR/tip-attestation.json" \
  --keyring "$DEMO_DIR/keyring.json"
```

Omitting `--keyring` deliberately leaves the legacy v1 tip attestation
self-signed and therefore untrusted as an external anchor.

### 4. Create a versioned checkpoint and portable claim receipt

Checkpoint v2 binds the origin, ledger lineage, tree size, Merkle root, chain
tip, and signer-asserted time to an external threshold policy:

```bash
"$BIN" --database "$DB" checkpoint-policy-init \
  --origin demo-local \
  --key-id demo-operator \
  --key-file "$DEMO_DIR/operator.seed" \
  --output "$DEMO_DIR/checkpoint-policy.json"

"$BIN" --database "$DB" checkpoint-sign \
  --policy "$DEMO_DIR/checkpoint-policy.json" \
  --key-id demo-operator \
  --key-file "$DEMO_DIR/operator.seed" \
  --output "$DEMO_DIR/checkpoint.json"

"$BIN" --database "$DB" checkpoint-verify \
  "$DEMO_DIR/checkpoint.json" \
  --policy "$DEMO_DIR/checkpoint-policy.json"
```

Export only one claim and the evidence required to verify it:

```bash
"$BIN" --database "$DB" receipt-export \
  --claim-id "$CLAIM_ID" \
  --checkpoint "$DEMO_DIR/checkpoint.json" \
  --policy "$DEMO_DIR/checkpoint-policy.json" \
  --output "$DEMO_DIR/claim-receipt.json"
```

Verification is offline. This command succeeds even when the named database is
unreachable because it opens no store and performs no network I/O:

```bash
"$BIN" --database unreachable receipt-verify \
  "$DEMO_DIR/claim-receipt.json" \
  --policy "$DEMO_DIR/checkpoint-policy.json" \
  --json \
  | jq '{receipt_integrity, content_authority}'
```

`receipt_integrity.verified` can be true while all factual-authority flags stay
false. The receipt proves commitment and the selected provenance path, not
truth, authorship, source authenticity, or an independent timestamp.
It verifies only the selected envelopes under the authorized signers' root; it
does not replay the complete ledger prefix. The receipt also reveals those
selected envelopes verbatim, so handle it as sensitive memory data.

### 5. Append safely and verify the historical anchor

```bash
"$BIN" --database "$DB" remember \
  "Aaron reviews the changelog every Friday." \
  --tag process --mention Aaron
```

The old checkpoint no longer represents the current tip, so current-mode
verification refuses it. Historical mode verifies the unchanged prefix and
reports the appended event separately:

```bash
if "$BIN" --database "$DB" checkpoint-verify \
  "$DEMO_DIR/checkpoint.json" \
  --policy "$DEMO_DIR/checkpoint-policy.json"; then
  echo "unexpected: historical checkpoint accepted as current" >&2
  exit 1
fi

"$BIN" --database "$DB" checkpoint-verify \
  "$DEMO_DIR/checkpoint.json" \
  --policy "$DEMO_DIR/checkpoint-policy.json" \
  --mode historical

"$BIN" --database "$DB" explore \
  --checkpoint "$DEMO_DIR/checkpoint.json" \
  --policy "$DEMO_DIR/checkpoint-policy.json" \
  --checkpoint-mode historical
```

## Rebuild the public visual evidence

The README GIF is generated from fresh embedded stores and three real TUI
states. On macOS it captures complete Ghostty windows, including the TUI's
native Kitty raster layer; elsewhere it records the real half-block fallback
through VHS. Neither path overlays mascot art onto a terminal screenshot:

```bash
scripts/render-readme-tui-gif.sh
```

Both backends validate all three full frames and the protagonist region before
replacing `assets/nahuali-tui.gif` and `assets/nahuali-tui.png`. The Ghostty path
also verifies both raster corner-mascot regions; the VHS path cannot capture
terminal image layers and therefore validates its half-block protagonist.

The supplementary [`sample-trust-report.html`](sample-trust-report.html) is a
report-v2 snapshot generated from the synthetic episode, claim, and link above,
plus a legacy tip attestation authorized by an external keyring. Its
browser capture lives at `assets/nahuali-trust-report.png` (1440×1388). The
checked-in sample shows three events, `Certify`, passing internal checks, and a
trusted sequence-three checkpoint. Regenerate the HTML through
`trust-report --attestation ... --keyring ... --html`; do not edit verdicts into
the artifact by hand.

## Clean up

The demo directory is disposable and contains only this walkthrough's embedded
store, policy, portable claim receipt, and temporary signing seed:

```bash
find "$DEMO_DIR" -type f -delete
find "$DEMO_DIR" -depth -type d -empty -delete
unset NAHUALI_HOME
```
