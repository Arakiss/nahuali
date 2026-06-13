# Nahuali CLI

`nahuali-cli` ships the installed `nahuali` command for self-inspecting memory
workflows.

## Quickstart

```bash
curl -fsSL https://raw.githubusercontent.com/Arakiss/nahuali/main/scripts/install.sh | sh
nahuali demo          # the tamper-evidence trust story, in memory, no Docker
nahuali init          # wire your agent harness to use Nahuali
```

`demo` runs the trust story with zero dependencies; `init` installs the Claude
Code skill and prints the MCP config. (`demo`'s full narrative needs the
`attestation` build feature; a build made without it explains how to get one
that has it.)

## Install From Source

During pre-release development, avoid overwriting an existing global `nahuali`
command. Use `cargo run` for transition-safe checks:

```bash
cargo run -p nahuali-cli -- --database ./memory validate
```

After the Rust CLI is the intentional cutover, install from the repository root:

```bash
cargo install --path crates/nahuali-cli --locked
```

Verify the installed command:

```bash
nahuali --version
```

## Install From Release Archive

Pre-release archives are intended for isolated operator checks before replacing
an existing global command. Each archive contains the `nahuali`, `nahuali-mcp`,
and `nahuali-api` binaries under `bin/`.

Extract a release archive into a temporary or project-local tools directory,
run the binary through its full path, and only move it onto `PATH` after the
Rust CLI is the intentional cutover:

```bash
./bin/nahuali --version
./bin/nahuali validate
```

## Database

By default, `nahuali` selects the SurrealDB database:

```txt
memory
```

Override the database name with `--database` or `NAHUALI_DB_DATABASE`:

```bash
nahuali --database ./memory remember "Lena owns the release notes"
```

The value selects a SurrealDB database name. Path-like values such as
`./memory` are accepted for operator convenience and normalized into
SurrealDB-safe identifiers before use. The store is an append-only
`memory_record` ledger. Opening the store validates event ordering and checksums
before projecting memory state in Rust. The public storage boundary is
summarized in the root README while design notes remain private during
pre-release development.

## Primary Workflow

```bash
nahuali --database ./memory remember "Lena owns the release notes" --tag product --mention Lena
nahuali --database ./memory ingest-text examples/source-note.md --kind note --title "Release notes source" --chunking paragraphs --tag product --mention Lena --dry-run --json
nahuali --database ./memory ingest-text examples/source-note.md --kind note --title "Release notes source" --chunking paragraphs --tag product --mention Lena --json
nahuali --database ./memory ingest-dir examples --recursive --extension md --extension txt --chunking paragraphs --dry-run --json
nahuali --database ./memory ingest examples/ingest-conversation.json --dry-run --json
nahuali --database ./memory ingest examples/ingest-conversation.json --json
nahuali --database ./memory claim Lena owns "release notes" --confidence 0.92 --source-last
nahuali --database ./memory link Lena owns "release notes" --confidence 0.9 --source-last
nahuali --database ./memory remember "Release notes belong to the Nahuali project" --scope project:Nahuali
nahuali --database ./memory recall "release notes" --scope project:Nahuali --authority --json
nahuali --database ./memory preference "Release notes" "Keep release notes concise" --source-last
nahuali --database ./memory intention "Ship release notes" --priority high --source-last
nahuali --database ./memory intention-update <intention_id> --deadline-at-ms 1777939200000 --progress 25
nahuali --database ./memory intention-complete <intention_id> --reason "Release notes shipped"
nahuali --database ./memory reconcile-intentions --json
nahuali --database ./memory goal-progress --json
nahuali --database ./memory proactive --json
nahuali --database ./memory deadlines --json
nahuali --database ./memory anomalies --json
nahuali --database ./memory anomaly-acknowledge <anomaly_id> --note "Operator reviewed this alert" --dry-run --json
nahuali --database ./memory briefing --json
nahuali --database ./memory hook pre-prompt --input "Who owns release notes?" --json
nahuali --database ./memory sleep --json
nahuali --database ./memory consolidation-plan --json
nahuali --database ./memory recall "Lena release"
nahuali --database ./memory recall "Lena release" --authority --json
nahuali --database ./memory graph "Lena" --depth 2 --limit 20 --json
nahuali --database ./memory project "Lena" --json
nahuali --database ./memory semantic-rebuild
nahuali --database ./memory semantic-status --json
nahuali --database ./memory recall "Lena release" --semantic --json
nahuali --database ./memory inspect
nahuali --database ./memory self-inspect --json
nahuali --database ./memory reflect --json
nahuali --database ./memory review --limit 5 --min-priority high --json
nahuali --database ./memory review-resolve <review_id> --note "Operator reviewed this item" --json
nahuali --database ./memory validate
nahuali --database ./memory audit --json
nahuali --database ./memory audit --inclusion-proof 2 --json
nahuali --database ./memory maintenance
nahuali --database ./memory snapshot --output ./memory.snapshot.json --dry-run
nahuali --database ./memory snapshot --output ./memory.snapshot.json
nahuali --database ./memory snapshot-validate ./memory.snapshot.json --json
nahuali --database ./memory backup --output ./memory.backup.json --dry-run --json
nahuali --database ./memory backup --output ./memory.backup.json
nahuali backup-validate ./memory.backup.json --json
nahuali backup-drill ./memory.backup.json --target-database ./restored-memory --json
nahuali restore ./memory.backup.json --target-database ./restored-memory --dry-run --json
nahuali --database ./memory export --output ./memory.interchange.json
nahuali --database ./imported-memory import ./memory.interchange.json --dry-run --json
nahuali --database ./memory data --json
```

Use `--json` on primary commands when scripts or agents need structured output
without human prose.

`recall --authority --json` returns scored results, `authority.mode`,
`authority.score`, `authority.can_trust`, deduplicated
`authority.signal_kinds`, and the health report used to make the decision.
Use `--kind <kind>` to narrow lexical or authority recall to specific memory
families, and `--require-evidence` when every returned result must carry an
evidence ID.

Use `--scope kind:name` on recording and recall commands when memory belongs to
an explicit context boundary. Supported kinds are `personal`, `project`,
`organization`, and `custom`. Scoped recall is an exact filter and does not
merge unscoped records into the result set. Unscoped recall remains the broad
compatibility path. Scope-aware semantic recall is intentionally deferred; use
lexical recall or `recall --authority --json` with `--scope` for v1.

`graph <seed> --json` returns a deterministic neighborhood around a matching
entity or memory item, including nodes, edges, depth, evidence IDs, authority,
and health/review overlays.

`project <entity> --json` returns a focused entity or project dashboard. It
combines the matched entity, graph neighborhood, recall results,
evidence-backed episodes, claims, links, procedures, intentions, authority,
health, and review context so agents do not have to stitch together several
reports manually.

`ingest <path> --json` reads a provenance-aware source document. The document
registers source metadata, appends source episodes, and writes only the explicit
claims, links, procedures, and intentions included in that document. `--dry-run`
validates the whole document without mutating the ledger. JSON reports include
`report.preflight` with scope, source-size, derived-record, evidence-gap, and
episode-reference counts for batch inspection.

`ingest-text <path> --json` reads a UTF-8 local text file, converts it into
source episodes, and then uses the same validated ingestion path. It supports
document, paragraph, or line chunking plus tags, mentions, metadata, and
`--dry-run`; it does not infer claims or links from the text. The ingestion
report still includes the same preflight object, so scripts can confirm a text
source produced only source episodes before applying it.

`ingest-dir <path> --json` applies the same text intake path to a directory.
By default it considers `md`, `markdown`, and `txt` files; pass `--recursive`
and repeated `--extension` flags for explicit batch control. The command
preflights every discovered file before writing any records, and each file
report includes its own preflight summary.

`briefing --json` is the default pre-work agent surface. It returns authority,
knowledge health, recent source episodes, active intentions, high-priority
review items, and graph seeds without mutating the ledger.

`status --json`, `session-resume --json`, `timeline --json`, and
`pending --json` preserve the core daily workflow names from earlier Nahuali
CLI work while using the current Rust engine, SurrealDB graph projection, and
knowledge-health contracts.

`intention-update`, `intention-complete`, `intention-block`, and
`intention-defer` provide explicit append-only lifecycle writes. Updates can
set or clear deadline timestamps, dependencies, parent goals, and progress
metadata. `reconcile-intentions --json` and `goal-progress --json` are
non-mutating reports over the same projected state.

`proactive --json` composes deadline signals, anomaly alerts, evidence capture
opportunities, and high-risk review work into one non-mutating operator report.
Use `deadlines --json` and `anomalies --json` for narrower scriptable views.
`anomaly-acknowledge <anomaly_id> --note ...` is the explicit audit path for
acknowledging an alert; `--dry-run --json` previews the append-only review
decision without mutating the record ledger.

`hook <kind> --json` packages memory context for host execution points.
Supported kinds are `session-start`, `pre-prompt`, `post-action`,
`session-close`, and `sleep-cycle`. `pre-prompt` and `post-action` require
`--input`; hook output includes authority, directives, recall where relevant,
and the same no-automatic-write-back policy used by self-inspection.

`sleep --json` runs the standalone Sleep Mode report. It replays recent
episodes, includes the same reflection and self-inspection context used by the
sleep hook, proposes evidence-backed consolidation candidates, and never writes
memory automatically. Use `--episode-limit`, `--candidate-limit`,
`--cycle-limit`, and `--evidence-limit` to tune report size for operators or
scripts.

`consolidation-plan --json` turns rest, reflection, and review signals into
explicit replay, extraction-candidate, reconciliation, review-gate, and
commit-eligibility operations. It is non-mutating, carries evidence IDs where
available, and keeps automatic write-back disabled. Use `--episode-limit`,
`--candidate-limit`, `--cycle-limit`, `--evidence-limit`, and `--review-limit`
to tune report size.

`semantic-rebuild` recreates the configured Qdrant collection from the current
Rust projection. `semantic-status --json` reports whether that collection
exists and how many points it contains. `recall --semantic --json` returns
hybrid recall with separate lexical and semantic score components, authority
context, evidence IDs, and explanations.

`projection-status --json`, `projection-rebuild --json`, and
`projection-validate --json` inspect, repair, and verify the SurrealDB graph
projection derived from the authoritative `memory_record` ledger.

`self-inspect --json` returns a non-mutating consolidation report with
knowledge health, authority, findings, proposed review items, and an explicit
no-automatic-write-back policy. It includes source-coverage findings when
episodes lack source records or derived memory lacks source episode evidence.

`reflect --json` groups self-inspection findings into non-mutating reflection
cycles with priority, evidence IDs, source/evidence coverage, and the same
operator-review write-back policy.

`review --json` turns the same self-inspection findings into a prioritized
operator queue with evidence IDs, proposed action, status, authority context,
and concrete guidance. It is still non-mutating; follow-up memory writes must be
explicit commands. Use `--action <action>` to focus the queue before resolving
or recording follow-up evidence.

`review-resolve <review_id> --note ...` is the explicit write-back path for
review work. It appends an audit decision only after an operator supplies a note;
`--dry-run --json` previews the decision without mutating the record ledger.

`validate --json` is non-destructive. It reports record-ledger compatibility issues
as structured JSON before exiting non-zero, includes `database`, and lets
automation inspect invalid logs without projecting them. Default validation stays
compatible with pre-chain records; add `--require-chained` when automation must
fail closed if any record lacks a hash-chain link.

`audit` is a non-mutating diff of what the ledger recorded between two points. It
bounds the range with `--from`/`--to` (exclusive then inclusive sequence) and
`--since`/`--until` (timestamp), reports per-kind counts and per-event entries,
restates the integrity of the history through the upper bound (checksums,
sequence contiguity, and the hash chain and anchoring tips under `tamper-evidence`),
and exits non-zero when that history fails verification.

`trust-report` composes the trust primitives into one non-mutating verdict:
knowledge counts, authority, restated ledger integrity (with the chain tip under
`tamper-evidence`), knowledge health, an overall `trustworthy` flag, and the
reasons behind it. Under the `attestation` feature, `--attestation <PATH>` folds
in a verified signed checkpoint. `--html <PATH>` also writes a self-contained
HTML dossier (inline styles, no network calls) that renders offline. It exits
non-zero only when the recorded history fails ledger integrity verification.

`maintenance` reports the non-destructive local maintenance state. `snapshot`
writes an optional projection artifact or previews it with `--dry-run`.
`snapshot-validate` checks that artifact against a fresh replay of the current
record ledger.

`backup` writes an authoritative record-ledger manifest or previews it with
`--dry-run`. `backup-validate` checks the manifest and all included records;
with `--require-chained`, it also rejects backups where a valid checksum hides
stripped hash-chain links.
`backup-drill` validates the backup and dry-runs restore into a target database
without writing records. `restore` writes backup records only into an empty
target database and reports that Qdrant vectors must be rebuilt from the
restored records. Default semantic commands scope derived collections to the
selected database, so temporary migration or restore targets do not overwrite
another database's derived index.

`export` writes a source-neutral interchange document. `import` validates the
whole document first and appends new events only when the document is valid;
`--dry-run --json` reports the append plan without mutating the target store.
Import reports include `report.preflight` with source coverage, scope coverage,
evidence-gap, and episode-coverage counts for migration rehearsals. They also
include `report.readiness`, a self-inspection forecast for projected findings,
review item count, and write-back policy before any import write.

`convert-legacy-export` is the bridge for historical exports. It accepts the
structured export shape plus deterministic SurrealQL export bundles, then emits
the source-neutral interchange format so `import --dry-run --json` can inspect
the write plan before any record is appended. This bridge imports into the OSS
record-ledger/interchange contract and is not legacy schema parity.

`convert-projection-export` is a dry-run-friendly bridge for projected memory
exports. It accepts canonical top-level arrays plus conservative envelope and
table aliases for episodes, entities, relations, procedures, and intentions,
then emits the source-neutral interchange format so `import --dry-run --json`
can inspect the write plan before any record is appended. When projected
records include source labels, conversation identifiers, source positions,
roles, epoch-millisecond timestamps, or UTC ISO timestamps, the bridge carries
them into interchange so import can preserve historical provenance and event
time instead of collapsing migrated memory into the import time.

## Optional Build Features

The CLI default build includes `tui` and `tamper-evidence`; build with
`--no-default-features` only when you intentionally want the minimal,
unchained compatibility surface. Extra features remain opt-in.

- `--features tamper-evidence` (default in `nahuali-cli`): recorded events are chained by hash, so
  `validate` detects an in-place rewrite of any historical record even when its
  checksum was recomputed. `validate --require-chained` and
  `backup-validate --require-chained` reject ledgers or backups that are missing
  chain links. `audit --inclusion-proof <sequence> --json` emits a Merkle
  inclusion proof for one event under the audited root.
- `--features attestation` (implies `tamper-evidence`): adds `attest-sign` and
  `attest-verify`. `attest-sign --key-file <seed> -o tip.json` signs the current
  chain tip into a portable receipt; `attest-verify tip.json` checks it against
  the live ledger and exits non-zero when the tip has moved or the signature is
  invalid. Supply a 32-byte Ed25519 seed as hex (`openssl rand -hex 32`). It also
  adds `audit --from-attestation tip.json`, which anchors the audit's lower bound
  on a verified checkpoint and diffs only what was appended since it.
- `--features local-embeddings`: lets `semantic-rebuild` and `recall --semantic`
  use a static model2vec model instead of the deterministic embedder. Set
  `NAHUALI_EMBEDDING_PROVIDER=model2vec` and point
  `NAHUALI_LOCAL_EMBEDDING_MODEL_PATH` at a local model directory.

```bash
cargo run -p nahuali-cli --features attestation -- \
  --database ./memory attest-sign --key-file ledger.key -o tip.json
cargo run -p nahuali-cli --features attestation -- \
  --database ./memory attest-verify tip.json
```

`fact` and `relate` remain compatibility commands while the canonical public
language moves toward `claim` and `link`.
