# Changelog

This is the release history of Nahuali as one product. It covers the CLI, MCP
server, local HTTP API, Rust core, and the user-facing terminal interface.
Internal crate changelogs are technical appendices, not separate product
release histories.

## Unreleased

## [0.8.0-beta.12](https://github.com/Arakiss/nahuali/compare/v0.8.0-beta.11...v0.8.0-beta.12) (2026-10-17)


### New

* **intentions:** surface stale intentions in briefing and pending ([cc9d1ec](https://github.com/Arakiss/nahuali/commit/cc9d1ec2e56f5818c695590e3da6d2792d136e0f))


### Fixed

* **briefing:** honor zero limits instead of returning one item ([ac685aa](https://github.com/Arakiss/nahuali/commit/ac685aa23f31b93c66b99eefb217840b2741b399))
* **cli:** list every subcommand and visible alias in root help ([675eb84](https://github.com/Arakiss/nahuali/commit/675eb844e1f5c35a8b4a64534580d150c5c9ae51))
* **cli:** make the briefing explain its signals and respect zero limits ([b86b808](https://github.com/Arakiss/nahuali/commit/b86b8084fe8d4d4a31b756be9e79657fcb7ed040))
* **cli:** say why the store is not certified in the briefing ([ea99b60](https://github.com/Arakiss/nahuali/commit/ea99b60880ba89f7074f3cd5a959ce5fb0d6ded7))
* **core:** age graph projection rows at the last ledger event ([f189633](https://github.com/Arakiss/nahuali/commit/f189633f427ed26f6c5e777ddd7a14d764dfb268))
* **core:** name the rebuild command when projection validation fails ([54d1629](https://github.com/Arakiss/nahuali/commit/54d16298a430facf39433ba9b8e8d6732c847e51))
* **core:** reject NaN confidence before it reaches the ledger ([77daf5b](https://github.com/Arakiss/nahuali/commit/77daf5b2e71c092889f586882d2e5c10506560fa))
* **core:** say that out-of-range confidence is clamped, not rejected ([f0e9ec9](https://github.com/Arakiss/nahuali/commit/f0e9ec9372bb013db25871046d444b49a7a998b1))
* **core:** validate the graph projection against ledger time, not wall-clock time ([57a8503](https://github.com/Arakiss/nahuali/commit/57a850306655d3b015bc1192400ee176e69a101f))


### Changed

* **forget:** extract the redaction cascade as a generic least fixpoint ([0cb4985](https://github.com/Arakiss/nahuali/commit/0cb4985eb3f355168920907f273c7c15a8ab218a))
* **inspection:** extract the per-group supersession rule ([a691de3](https://github.com/Arakiss/nahuali/commit/a691de30afb01112051f5a671cb7e65e4118dbaf))
* **recall:** score candidates through one admitted, totally ordered path ([9e8cb5d](https://github.com/Arakiss/nahuali/commit/9e8cb5d88f228b73d3df195779db111852b6f240))

## [0.8.0-beta.11](https://github.com/Arakiss/nahuali/compare/v0.8.0-beta.10...v0.8.0-beta.11) (2026-10-11)


### New

* **cli:** add --store to choose the store location ([13ab68a](https://github.com/Arakiss/nahuali/commit/13ab68a33a210165ee2bb2d46bc31513f143c265))
* **core:** forget memory with an auditable redaction trail ([2f05b25](https://github.com/Arakiss/nahuali/commit/2f05b258e98c981d197f641b049d7b2e1fdbb626))


### Fixed

* **cli:** document every subcommand option ([8218c2d](https://github.com/Arakiss/nahuali/commit/8218c2dc741d982726c30acd58c09b400595ca1c))
* **cli:** show the review item id and resolve command in human review output ([c33415e](https://github.com/Arakiss/nahuali/commit/c33415e00813cd8b6e68ca88e174cc7e956cd989))
* **core:** keep claim supersession visible in recall and rank by recency ([7a3191f](https://github.com/Arakiss/nahuali/commit/7a3191fb3ba195e4ee0661a3520749b897f739ce))
* **core:** name the command that derives memory from an episode in review guidance ([2c2d14b](https://github.com/Arakiss/nahuali/commit/2c2d14bfa5498ac57f418debe2eaf2ffdae1886a))
* **core:** queue concurrent processes on the embedded store lock ([11c7715](https://github.com/Arakiss/nahuali/commit/11c771587ecdadee2a516824ced67f89b4ff28aa))
* **core:** stop flagging first-hand episodes as a source coverage gap ([1ec5695](https://github.com/Arakiss/nahuali/commit/1ec5695800951a76797285c4a97d0956e4916cc4))
* **core:** treat claim values as literals instead of entities ([63ae626](https://github.com/Arakiss/nahuali/commit/63ae62648c8741246f75c80913bdd750dd65f649))
* **install:** repeat the signature outcome in the install summary ([6aaed5d](https://github.com/Arakiss/nahuali/commit/6aaed5d9ac4e735ce8f9ff2065b59b7bc9f1b376))
* resolve logged daily-use friction (recall lifecycle, ranking, store lock, forget, literals, CLI polish) ([0e9cfa0](https://github.com/Arakiss/nahuali/commit/0e9cfa00f6ce561aea1314ee557180cedc07e055))

### Upgrade notes

#### Breaking changes and migration

- **Ledger.** Two new record types, `memory_forgotten` and `redacted`, appear
  only after `nahuali forget` runs. 0.8.0-beta.10 and older binaries refuse to
  open such a store (`unknown variant 'redacted'`) without modifying it.
  Upgrade every CLI, `nahuali-mcp`, and `nahuali-api` process before the first
  `forget`. Stores that never ran `forget` stay readable by older binaries.
- **Projection.** `MEMORY_DATA_VERSION` moves from 7 to 8 because claim objects
  no longer create entities. The ledger is unchanged and every read re-derives
  the projection, so existing stores need no migration; entity counts drop by
  the claim values that were only ever claim objects. Run `nahuali reconcile`
  to rebuild the graph projection and semantic index immediately (the next
  write also rebuilds the graph). Snapshots written by older versions fail
  `snapshot-validate` on the projection version; take a new snapshot.
- **JSON output (additive).** Recall results gain optional `created_at_ms`,
  `updated_at_ms`, `observed_at_ms`, `confidence`, `lifecycle`, and
  `score_breakdown`. Recall `score` values change because they now include the
  recency component; re-baseline any absolute score thresholds. `data` gains
  `forgotten`; `audit` gains `memories_forgotten` and `records_redacted`
  counts and the `memory_forgotten` and `redacted` event kinds; validation
  reports gain the `unauthorized_redaction` and
  `record_redaction_unauthorized` issue kinds. Self-inspection and review
  guidance strings changed. The OpenAPI contract documents the new request and
  response fields.
- **Review.** The "Source coverage gap" finding is raised only for derived
  memory without a source episode, always at medium severity; its detail text
  is unchanged.
- **Locking.** Commands that previously failed immediately on a locked
  embedded store now wait up to 30 seconds. Set
  `NAHUALI_STORE_LOCK_TIMEOUT_MS=0` to keep fail-fast behavior.
- **Rust API.** `MemoryEvent` gains `MemoryForgotten` and `Redacted`;
  `NahualiError` gains `InvalidRecallRanking`, `InvalidForgetRequest`,
  `UnknownForgetTarget`, and `StoreBusy`; `RecallOptions` gains
  `exclude_superseded` and `ranking` and no longer implements `Eq`;
  `RecallResult` and `MemoryData` gain fields. Exhaustive matches and struct
  literals need updating.

#### New in detail

- `nahuali forget <id>... --reason ... [--requested-by WHO] [--dry-run]`
  erases memory by id together with the memory derived from it, with an
  auditable `memory_forgotten` trace (who, when, why, which ids and records) that
  never contains the erased content. Redacted records keep their chained hash,
  so validation, Merkle proofs, and earlier checkpoints still verify; an
  unauthorized redaction fails closed. CLI-only by design. Backups, exports,
  receipts, the Qdrant index until rebuilt, and storage-engine compaction are
  outside its reach; see `compliance/gdpr-position.md`.
- Recall ranking is an explicit, tunable policy (relevance, confidence weight,
  evidence bonus, recency with a half-life) exposed as `--confidence-weight`,
  `--recency-weight`, and `--recency-half-life-days`, the HTTP `ranking`
  object, and `RecallRankingPolicy`. Each result reports its `score_breakdown`,
  timestamps, and confidence.
- `--store <PATH|URL>` chooses where the store lives, separately from
  `--database <NAME>`, which names a database inside it.

#### Fixed in detail

- A claim replaced by a newer evidence-backed value stays marked as
  `superseded` in recall after its review item is resolved. It ranks after
  current memory, carries `lifecycle.superseded_by`, and is hidden with
  `--exclude-superseded` (MCP `excludeSuperseded`, HTTP `exclude_superseded`).
- Parallel commands against the embedded store queue on its lock (bounded by
  `NAHUALI_STORE_LOCK_TIMEOUT_MS`, default 30000) instead of failing at once
  with `LOCK is already locked`.
- Claim values are literals and no longer create entities, so values such as
  `"2 podencos y 5 gatos"` stop raising "Isolated entity" review items.
- First-hand episodes no longer raise a "Source coverage gap" item; only
  derived memory without a source episode does.
- Human `review` output shows each item's id and the exact `review-resolve`
  command. Every subcommand option has help text.
- Review guidance for unsourced memory names the command that derives memory
  from an episode (`--source-last` / `--source-episode <id>`).
- The installer summary always ends with a `Signature:` line stating whether
  the Sigstore signature was verified or only the checksum was checked.

## [0.8.0-beta.10](https://github.com/Arakiss/nahuali/compare/v0.8.0-beta.9...v0.8.0-beta.10) (2026-07-30)


### New

* add LongMemEval retrieval adapter ([2df5b7d](https://github.com/Arakiss/nahuali/commit/2df5b7dabfd92970188cdd64955c9944bf169250))
* publish executable HTTP trust clients ([c09e0f4](https://github.com/Arakiss/nahuali/commit/c09e0f43b8e75a9322c39e17490178718cc81586))


### Fixed

* align beta release safeguards ([103a689](https://github.com/Arakiss/nahuali/commit/103a689e6768ee0c060e9f5842f71dfb0dc19b56))
* keep store setup failures explicit ([97f5cb9](https://github.com/Arakiss/nahuali/commit/97f5cb9227c0a086fee73cc88187ffe0fccac9e3))
* recognize supported claims as connected knowledge ([e1f1ebb](https://github.com/Arakiss/nahuali/commit/e1f1ebbfd2912e493a0ee833ff7480364a0bb795))
* tolerate bounded projection rebuild capacity ([017e35c](https://github.com/Arakiss/nahuali/commit/017e35c95b3b1e3b4caee7625d73c8cf86845eb6))

## [0.8.0-beta.9](https://github.com/Arakiss/nahuali/compare/v0.8.0-beta.8...v0.8.0-beta.9) (2026-07-30)


### New

* animate mascot in version output ([2494f0d](https://github.com/Arakiss/nahuali/commit/2494f0d3525b76daccae665484017d0dcd4f93b1))

## [0.8.0-beta.8](https://github.com/Arakiss/nahuali/compare/v0.8.0-beta.7...v0.8.0-beta.8) (2026-07-30)


### Performance

* stop rewriting unchanged graph rows ([af748a6](https://github.com/Arakiss/nahuali/commit/af748a6a42ae1f842007cc5a3793133a6d3dee33))

## [0.8.0-beta.7](https://github.com/Arakiss/nahuali/compare/v0.8.0-beta.6...v0.8.0-beta.7) (2026-07-17)


### New

* make ledger checkpoints independently verifiable ([97b6bd0](https://github.com/Arakiss/nahuali/commit/97b6bd0a04b23fb77210190dba802e7b4082007c))
* make trust evidence portable and understandable ([b9d28fe](https://github.com/Arakiss/nahuali/commit/b9d28fe66a1682b31781e408df805b8f8ff0656b))


### Fixed

* keep the TUI mascot visually canonical ([e130f6f](https://github.com/Arakiss/nahuali/commit/e130f6fa0723556b79cdc84db44b98adbac8f219))
* make graph projection rebuilds atomic and verifiable ([b2c225a](https://github.com/Arakiss/nahuali/commit/b2c225adc5f5b47b2b4f59a023f2c357efbda1da))
* make ledger verification fail closed ([8a4c281](https://github.com/Arakiss/nahuali/commit/8a4c281165c3d0520c70e9343d3f431e37a133fc))

## [0.8.0-beta.6](https://github.com/Arakiss/nahuali/compare/v0.8.0-beta.5...v0.8.0-beta.6) (2026-07-16)


### New

* close memory reliability gaps ([bde618d](https://github.com/Arakiss/nahuali/commit/bde618da9ddd285ba0df6227201744b48a268838))
* expose serving readiness ([3e83893](https://github.com/Arakiss/nahuali/commit/3e83893e262335b60a420afc59ed4cc2c0987784))


### Fixed

* certify derived projection freshness ([ada1e9a](https://github.com/Arakiss/nahuali/commit/ada1e9a8467edc1a92b1460900321ac5f1c2460a))
* coalesce completed graph rebuilds ([58e3c0b](https://github.com/Arakiss/nahuali/commit/58e3c0b99c5f8c0109b2f92ea9088bb4985c2dd0))
* keep query trust scoped to returned memory ([b736e04](https://github.com/Arakiss/nahuali/commit/b736e043dfd5eda484a2d7472efe32a928f46a9f))
* make ledger write outcomes unambiguous ([4e63e05](https://github.com/Arakiss/nahuali/commit/4e63e0533e047592d342a8bfa49b749bc1c12679))
* make restored stores operationally explicit ([c459c23](https://github.com/Arakiss/nahuali/commit/c459c2311f83fa29c3fdec61fb5b2171fec9e047))
* make trust benchmark results artifact-bound ([e47e8b7](https://github.com/Arakiss/nahuali/commit/e47e8b7001449e83ac957276eb690c70429ba89d))


### Performance

* skip unchanged ledger replay ([cd88d34](https://github.com/Arakiss/nahuali/commit/cd88d348305a6818651ab6af9c6dca6bb395e30b))

## [0.8.0-beta.5](https://github.com/Arakiss/nahuali/compare/v0.8.0-beta.4...v0.8.0-beta.5) (2026-07-16)


### Fixed

* require complete release provenance ([66afff7](https://github.com/Arakiss/nahuali/commit/66afff707cc5215f5299210b5419b277be18e08c))

## [0.8.0-beta.4](https://github.com/Arakiss/nahuali/compare/v0.8.0-beta.3...v0.8.0-beta.4) (2026-07-16)


### Fixed

* prevent partial graph projection rebuilds ([8d1c5d1](https://github.com/Arakiss/nahuali/commit/8d1c5d118831b1f93da8256e1c1fe1eda17362a5))

## [0.8.0-beta.3](https://github.com/Arakiss/nahuali/compare/v0.8.0-beta.2...v0.8.0-beta.3) (2026-07-15)


### Fixed

* prevent release verification from contending with active stores ([777d617](https://github.com/Arakiss/nahuali/commit/777d61740ea975329fd6fedc9f5a9c8abfcb750d))

## [0.8.0-beta.2](https://github.com/Arakiss/nahuali/compare/v0.8.0-beta.1...v0.8.0-beta.2) (2026-07-15)


### New

* make governed recall visible end to end ([56fdb14](https://github.com/Arakiss/nahuali/commit/56fdb14dd2c145860f9a7a93491ef9329d1866f1))
* make the TUI mascot crisp and reliably visible ([94caaef](https://github.com/Arakiss/nahuali/commit/94caaef7ed02943702322c1d5fe9e54b9b96f38d))


### Fixed

* keep the mascot asset within release limits ([ab16d31](https://github.com/Arakiss/nahuali/commit/ab16d3171cef05c981e5d82e62f0fb276aacaa7a))
* render curated notes from generated changelogs ([84d8d8f](https://github.com/Arakiss/nahuali/commit/84d8d8fbe9cadc4781cf61397612d35db1632e05))

## [0.8.0-beta.1](https://github.com/Arakiss/nahuali/compare/v0.8.0-beta.0...v0.8.0-beta.1) (2026-07-15)


### Fixed

* bind benchmark evidence to product releases ([0968b1b](https://github.com/Arakiss/nahuali/commit/0968b1be2da221274b77fef8946ae9d0889f0d80))
* make required CI independent of trigger type ([acc3836](https://github.com/Arakiss/nahuali/commit/acc38364d8d1052eebad59a105ff8b878a791bc2))
* prevent mixed-engine store upgrades ([d742e66](https://github.com/Arakiss/nahuali/commit/d742e6601756e2dedd27d754c64f61424d63def3))
* separate release proposals from public evidence ([8e38aa2](https://github.com/Arakiss/nahuali/commit/8e38aa2d8ed7a667b28397ece3843a35dec7783e))

## [0.8.0-beta.0] - 2026-07-15

### Why upgrade

This release makes the product's original promise usable without external
services: memory can be recorded, inspected, recalled with evidence, and given a
deterministic trust verdict from one local installation.

### New

- Embedded local storage for the default CLI, MCP, and HTTP workflows. Docker is
  no longer required to try or operate the core memory path.
- A trust-first `nahuali explore` terminal interface for browsing memory,
  evidence, store health, and ledger integrity.
- `nahuali demo`, `nahuali init`, evidence-required recall, self-inspection,
  review queues, trust reports, governed repair, snapshots, and migration tools.
- Official MCP Registry metadata and a multi-architecture MCP container image.

### Changed

- Tamper-evident SHA-256 chaining and Ed25519 tip attestation are enabled in
  normal builds. An unchained legacy build now requires an explicit opt-out.
- Strict validation rejects unchained or partially chained ledgers unless the
  caller deliberately selects the legacy-permissive path.
- Self-inspection distinguishes unsupported, stale, contradictory, and malformed
  memory more precisely and reports deduplicated affected-record counts.
- All shipped components now share one product version and one public release
  tag. The earlier `1.0.0-beta.0` and `1.1.0-beta.0` publications were premature
  automation errors and are superseded by this pre-1.0 release.

### Breaking changes and migration

- Strict validation now fails closed for unchained ledgers. Use
  `--allow-unchained` only while migrating a legacy store.
- New records use envelope version 2 and SHA-256 checksums. Existing version 1
  records remain readable and valid; they are not rewritten.
- The former `--require-chained` opt-in was replaced by the explicit
  `--allow-unchained` compatibility flag.

### Fixed

- Database names that would previously be normalized into a different name are
  rejected with a useful error.
- Configuration precedence is deterministic: command-line flag, environment,
  then built-in default.
- Same-observation multi-value facts are no longer misreported as contradictions.
- Embedded storage uses SurrealDB 3.1.5, which fixes the upstream cold-start
  session race that could intermittently abort a fresh local database.

### Security and integrity

- New ledger records use SHA-256 checksums and bind the preceding record hash.
- Install archives require SHA-256 verification and include Sigstore bundles and
  GitHub artifact attestations.
- Signed tip verification detects a fully recomputed ledger suffix that a hash
  chain alone cannot distinguish from a legitimate rewrite.

### Beta limits

- No stable 1.0 API guarantee yet.
- No hosted service, accounts, teams, billing, managed sync, or managed uptime.
- Nahuali evaluates evidence and memory health. It does not claim that recalled
  information is objectively true.

## [0.6.1-beta.0] - 2026-07-06

- Published verified macOS and Linux archives for x86_64 and arm64.
- Added mandatory checksums, Sigstore bundles, and release verification scripts.
- Curated the release page around evidence-backed recall, ledger integrity, and
  the supported local beta path.

## [0.6.0-beta.0] - 2026-06-20

- Added deterministic governed repair and the `nahuali repair` command.
- Added repair events to the append-only record model and surfaced repair needs
  without silently mutating memory.

## [0.5.0-beta.0] - 2026-06-14

- Added Merkle inclusion proofs to ledger audits.
- Made strict validation require chained records.
- Stabilized the CLI JSON output contract.

## [0.4.0-beta.0] - 2026-06-13

- Added the `explore` TUI, zero-service demo, harness initialization, ledger
  audit, trust report, temporal recall, archive recall, and reconciliation.
- Enabled tamper evidence by default for the CLI and added signed tip
  verification through a trusted keyring.
- Introduced the shared clay-on-coffee terminal presentation.

## [0.3.0-beta.0] - 2026-06-02

- Introduced the tamper-evident hash-chained ledger.
- Surfaced trust verdicts in normal CLI output.
- Added grouped help, shell completions, typed MCP results, and typed HTTP API
  response schemas.
- Changed the license to FSL-1.1-MIT. Each release converts to MIT two years
  after publication.

## [0.2.0-beta.0] - 2026-06-01

- Attached result-level trust to authority-ranked recall.
- Added Conventional Commit validation for release inputs.

## [0.1.0-beta.0] - 2026-06-01

- First public beta of the local Rust memory engine, CLI, MCP server, HTTP API,
  evidence-backed recall, knowledge-health inspection, and regression fixtures.

[0.8.0-beta.0]: https://github.com/Arakiss/nahuali/compare/nahuali-cli-v0.6.1-beta.0...v0.8.0-beta.0
[0.6.1-beta.0]: https://github.com/Arakiss/nahuali/compare/nahuali-cli-v0.6.0-beta.0...nahuali-cli-v0.6.1-beta.0
[0.6.0-beta.0]: https://github.com/Arakiss/nahuali/compare/nahuali-cli-v0.5.0-beta.0...nahuali-cli-v0.6.0-beta.0
[0.5.0-beta.0]: https://github.com/Arakiss/nahuali/compare/nahuali-cli-v0.4.0-beta.0...nahuali-cli-v0.5.0-beta.0
[0.4.0-beta.0]: https://github.com/Arakiss/nahuali/compare/nahuali-cli-v0.3.0-beta.0...nahuali-cli-v0.4.0-beta.0
[0.3.0-beta.0]: https://github.com/Arakiss/nahuali/compare/nahuali-cli-v0.2.0-beta.0...nahuali-cli-v0.3.0-beta.0
[0.2.0-beta.0]: https://github.com/Arakiss/nahuali/compare/nahuali-cli-v0.1.0-beta.0...nahuali-cli-v0.2.0-beta.0
[0.1.0-beta.0]: https://github.com/Arakiss/nahuali/releases/tag/nahuali-cli-v0.1.0-beta.0
