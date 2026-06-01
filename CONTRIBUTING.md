# Contributing to Nahuali

Nahuali is early. The most valuable contributions are small, tested changes that
make self-inspecting agent memory more trustworthy.

## Ground Rules

1. **Evidence is the product boundary.** New memory behavior must preserve the
   distinction between observed episodes and derived facts or relations.
2. **Inspection must stay first-class.** Features that improve recall but make
   health signals weaker are regressions unless the tradeoff is explicit and
   tested.
3. **Fail closed on corrupt ledgers.** Record ordering and checksums are
   integrity gates. Do not silently repair or skip invalid records in the core.
4. **Keep the core self-inspecting.** Hosted sync, dashboards, and managed
   services can exist later as thin layers, but `nahuali-core` must keep
   knowledge-health inspection as a first-class contract.
5. **Prefer deterministic behavior.** Model-dependent scoring or extraction can
   be layered later; the core regression suite should stay reproducible.
6. **Document failure modes.** Public docs should explain unsupported memory,
   contradictions, stale facts, and blind spots, not only happy paths.

## Public API And Compatibility

`nahuali-core` is a public library boundary. It denies undocumented public API
and unsafe code. Breaking changes to exported types, record-ledger semantics, CLI
JSON output, or MCP structured content require:

- a clear migration note
- updated tests or fixtures
- versioning impact recorded in the release notes or release plan

The record ledger is the most important compatibility surface. Once public
releases start, compatibility fixtures must cover existing records before a
schema change lands.

## Commit Style

Use concise Conventional Commit style:

```txt
feat: add provenance-backed recall scoring
fix: reject unsupported fact evidence
docs: explain record-ledger storage format
test: cover stale fact inspection
ci: split release validation gates
```

Accepted types: `feat`, `fix`, `docs`, `test`, `refactor`, `perf`, `build`,
`ci`, `chore`, `security`, `revert`.

Use `!` for breaking changes and include a `BREAKING CHANGE:` footer.

Every pull request validates commit subjects in CI. To catch problems before you
push, enable the optional local hook:

```bash
git config core.hooksPath .githooks
```

## Development

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo doc -p nahuali-core --no-deps
bash scripts/verify-install.sh
bash scripts/validate-clean-tree.sh
```

## Release Discipline

The source-install path is the current supported path. Do not add package
registry, public release, installer, or hosted-service claims unless the
corresponding path is validated in CI.

Before a public release:

- the repository must pass the clean-tree validation script
- GitHub Actions must be green on the release commit
- privacy and secret scans must be clean
- the release notes must state the supported install path
- record-ledger compatibility expectations must be explicit

## Licensing Of Contributions

Nahuali is source-available under the Functional Source License (FSL-1.1-MIT).
By submitting a contribution you agree that it is licensed to the project under
those same terms, including the MIT future grant that applies to each released
version two years after publication.

## Style

- Use `rustfmt` defaults.
- Keep public core docs precise and example-driven.
- Keep CLI output scriptable; JSON mode must be valid JSON with no extra prose.
- Keep MCP stdout reserved for JSON-RPC protocol messages.
- Avoid adding abstractions that are not needed by the current public contract.
