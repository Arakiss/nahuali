# Changelog

> Technical appendix only. The authoritative product history is the root
> [`CHANGELOG.md`](../../CHANGELOG.md). The generated 1.x entries below record
> withdrawn automation output and are not current Nahuali releases.

## [1.1.0-beta.0](https://github.com/Arakiss/nahuali/compare/nahuali-core-v1.0.0-beta.0...nahuali-core-v1.1.0-beta.0) (2026-07-14)


### Features

* make trusted memory usable without services ([3e1d70b](https://github.com/Arakiss/nahuali/commit/3e1d70bd5ed09f18ec3a0028299ad16c6a1aa2f4))


### Bug fixes

* coordinate embedded conflict recovery ([cb12252](https://github.com/Arakiss/nahuali/commit/cb12252b157da69b10096c67ad4939a8339f5dfd))
* prevent concurrent store initialization conflicts ([4c13988](https://github.com/Arakiss/nahuali/commit/4c1398858b5f926b545c99cfa87856dd715aaedd))
* retry embedded database selection conflicts ([1ff89ec](https://github.com/Arakiss/nahuali/commit/1ff89ec8b5dabd7a956576316540200bceaf513b))
* retry embedded transaction conflicts consistently ([baa2330](https://github.com/Arakiss/nahuali/commit/baa23301a76471c37aad2c06edd9d5406a0ef889))
* retry transient embedded schema conflicts ([1cc4096](https://github.com/Arakiss/nahuali/commit/1cc4096c44cc4c832ffb5073f36b30590a0a3d6d))

## [1.0.0-beta.0](https://github.com/Arakiss/nahuali/compare/nahuali-core-v0.7.0-beta.0...nahuali-core-v1.0.0-beta.0) (2026-07-14)


### ⚠ BREAKING CHANGES

* default builds now enable attestation and fail-closed validation; --require-chained becomes --allow-unchained (opt-out); new event envelopes are version 2 with SHA-256 checksums.
* the default validation verdict for an unchained ledger flips from accept to reject, the `--require-chained` flag is renamed to `--allow-unchained`, and new records carry a SHA-256 checksum at envelope version 2.
* --database now refuses names that normalization would rewrite (e.g. ./memory); the error suggests the normalized candidate. The documented idiom and all scripts/tests migrated to clean identifiers.
* **core:** KnowledgeHealth and SelfInspectionSummary gain public fields (signal_count, distinct_flagged_record_count), SelfInspectionFindingKind gains a LabelArtifact variant, and KnowledgeHealth.blind_spot_count changes meaning (coverage subset, not total signals). CLI JSON output reflects the new fields and semantics.

### merge

* config integrity + signed installer (C2+C6) ([94d9c22](https://github.com/Arakiss/nahuali/commit/94d9c22228c2d43ef2791fadb5af1154f628c50f))
* flagship defaults true by default (D1+D2+D3+C3) ([3cb436a](https://github.com/Arakiss/nahuali/commit/3cb436a62b33f6094aa6e69d106535cad2f24784))


### Features

* **config:** resolve database name once with flag&gt;env&gt;default precedence ([2afe8c1](https://github.com/Arakiss/nahuali/commit/2afe8c1b7e93fa5f28941b5fc8835a506e36ad6f))
* **core:** attach a per-result trust verdict to hybrid recall ([1d2d5e7](https://github.com/Arakiss/nahuali/commit/1d2d5e77c090ce7a6d43f2a21668befaac58bd6f))
* **core:** fix self-inspect signal quality (R1-R5) ([1e384ab](https://github.com/Arakiss/nahuali/commit/1e384ab14e811a031576bc57093adc142e7c5270))
* default to a fail-closed, SHA-256-checksummed, attested ledger ([118488f](https://github.com/Arakiss/nahuali/commit/118488fa936e9d1a83bc6d66c1816a77140f97f5))
* make memory trust visible on first run ([15fc455](https://github.com/Arakiss/nahuali/commit/15fc455cb3eeceeecdb6a3b6e9e17c9eb31495fa))

## [0.7.0-beta.0](https://github.com/Arakiss/nahuali/compare/nahuali-core-v0.6.0-beta.0...nahuali-core-v0.7.0-beta.0) (2026-07-06)


### Features

* add result-level trust to authority recall ([c8cd149](https://github.com/Arakiss/nahuali/commit/c8cd1499fe0117af3a920edb2ee95dcab0d52fd4))
* **cli:** anchor the ledger audit on a signed attestation ([cb5bd92](https://github.com/Arakiss/nahuali/commit/cb5bd9228024fbacd7c018d8c14ab601daad4ae4))
* **core:** add a non-destructive semantic index sync ([ec6df22](https://github.com/Arakiss/nahuali/commit/ec6df2232c72db0b1587e720ee0399f8e74b456a))
* **core:** add a point-in-time and exclude-stale temporal recall filter ([88c5ab6](https://github.com/Arakiss/nahuali/commit/88c5ab606af9334540576f7ef7af6e5db52774f2))
* **core:** add an append-only consistency verdict over the ledger ([b070c83](https://github.com/Arakiss/nahuali/commit/b070c833020009d8252b97bd0e3d6bd39bd63ac2))
* **core:** add an attestation keyring for key rotation and revocation ([b26219e](https://github.com/Arakiss/nahuali/commit/b26219ef0d79bf4fa178b3e56274b08a3a4655b4))
* **core:** add an opt-in tamper-evident hash-chained ledger ([09dfd0e](https://github.com/Arakiss/nahuali/commit/09dfd0e2b01cc27673eb52d2765173df85427c52))
* **core:** add keyring-aware chain-tip attestation verification ([d18c775](https://github.com/Arakiss/nahuali/commit/d18c775b7a7f922855a2b191430a55adfeb01ec6))
* **core:** add Merkle inclusion proofs over the ledger ([3b99760](https://github.com/Arakiss/nahuali/commit/3b997601e376540e8b7296c517fbff4cc02c0791))
* **core:** add non-mutating ledger audit/diff ([44b2b07](https://github.com/Arakiss/nahuali/commit/44b2b079b3bc326ec0eefa2a4b07e2488a492944))
* **core:** add RepairApplied event and projection ([837798b](https://github.com/Arakiss/nahuali/commit/837798bccfe15573829b0245979e8e081c27bd54))
* **core:** add self-repair contract types and autonomy classifier ([de465f9](https://github.com/Arakiss/nahuali/commit/de465f986c96b28e25e8f295815fa4c4b6016feb))
* **core:** add the Attestation Recovery Profile (ARP) benchmark ([5b91ed1](https://github.com/Arakiss/nahuali/commit/5b91ed1b53955a0cbedcbad10e93a4943e47d3a5))
* **core:** add the composed memory trust report ([f1d566b](https://github.com/Arakiss/nahuali/commit/f1d566b6b0860af3b1533328b6add774221deac7))
* **core:** audit confidence-vs-provenance and flag overconfident unsourced memory ([2e2357c](https://github.com/Arakiss/nahuali/commit/2e2357c5a9c6f0e721392a34c8c1f08d7e49a851))
* **core:** derive provenance-coverage and overconfidence rates per kind ([23415a9](https://github.com/Arakiss/nahuali/commit/23415a90620123fc1781f8ad613ba9c3d2e1f900))
* **core:** expose the LIVR integrity harness as a reproducible library function ([a3517e7](https://github.com/Arakiss/nahuali/commit/a3517e7c3039065130aadcafb365dfd627ee786b))
* **core:** flag a dormant store as stale so it doesn't silently certify ([610b17d](https://github.com/Arakiss/nahuali/commit/610b17dda1853ec3859448fd9305af78ee8a44bc))
* **core:** improve the default embedder with character n-grams ([12742e1](https://github.com/Arakiss/nahuali/commit/12742e1925fcbaba4e9269694df9d6af624d4a3c))
* **core:** sign and verify the tamper-evident ledger tip with Ed25519 ([f04b6f3](https://github.com/Arakiss/nahuali/commit/f04b6f323d2d3a755260ec53430b9ab5533ba9fd))
* **core:** surface deterministic repair-need nudge ([5f2f24d](https://github.com/Arakiss/nahuali/commit/5f2f24df953712c2bc8747f2fb9260f5ff835d44))
* **core:** surface recency-resolved supersession as a warn-level signal ([95d00e0](https://github.com/Arakiss/nahuali/commit/95d00e0ff8bf06fb66e089a82328c0bdbeabef2c))
* **core:** surface the ledger Merkle root in audit and trust-report integrity ([a4dd5ec](https://github.com/Arakiss/nahuali/commit/a4dd5ec42ae25fac81ddc29188908f34d5412dac))
* **core:** validate and apply LLM repair proposals ([4d664f9](https://github.com/Arakiss/nahuali/commit/4d664f90cdd2a46edbb4983bf2dedbf1705b6bb4))
* initial public beta ([4157a62](https://github.com/Arakiss/nahuali/commit/4157a62b1f4b3c6ff97f6dda61cada69990652c6))
* require chained records in strict validation ([42dc62b](https://github.com/Arakiss/nahuali/commit/42dc62bf414b6c694763df32cac581a95ea2e99c))
* **semantic:** add optional local model2vec embedder behind a feature flag ([d224799](https://github.com/Arakiss/nahuali/commit/d2247997eb31b89a3aee91eb2b59a7a71adf2232))


### Bug fixes

* **core:** compute report-layer health at the requested timestamp ([de98e4a](https://github.com/Arakiss/nahuali/commit/de98e4adf124e621aaf71e556f21a43283ee950b))
* **core:** cover StaleEpisode in the remaining signal-kind matches ([b7db42f](https://github.com/Arakiss/nahuali/commit/b7db42ff16d466276e1fe45276abd5ca8edb2c3d))
* **core:** don't flag isolated entities in a knowledge-free episode log ([fdb4107](https://github.com/Arakiss/nahuali/commit/fdb410738f96b4a52537a488964cc634753409d0))
* **core:** reject fabricated evidence citations on the direct write path ([b467c53](https://github.com/Arakiss/nahuali/commit/b467c53bf09eb2808fae179d4e7db550cb6cf07c))
* **semantic:** bump the deterministic embedding identity after the n-gram change ([d04378b](https://github.com/Arakiss/nahuali/commit/d04378b2dd775fdf3c84eb25c4c24807964b41f7))


### Performance

* apply projection updates incrementally ([a74a25e](https://github.com/Arakiss/nahuali/commit/a74a25ef04aff2414b303aff586f3f112ef67707))
* **core:** batch interchange and ingestion imports into one ledger flush ([4808121](https://github.com/Arakiss/nahuali/commit/4808121aeeee95ae68b83fcae6c1115e8cfe2c48))

## [0.6.0-beta.0](https://github.com/Arakiss/nahuali/compare/nahuali-core-v0.5.0-beta.0...nahuali-core-v0.6.0-beta.0) (2026-06-19)


### Features

* add result-level trust to authority recall ([c8cd149](https://github.com/Arakiss/nahuali/commit/c8cd1499fe0117af3a920edb2ee95dcab0d52fd4))
* **cli:** anchor the ledger audit on a signed attestation ([cb5bd92](https://github.com/Arakiss/nahuali/commit/cb5bd9228024fbacd7c018d8c14ab601daad4ae4))
* **core:** add a non-destructive semantic index sync ([ec6df22](https://github.com/Arakiss/nahuali/commit/ec6df2232c72db0b1587e720ee0399f8e74b456a))
* **core:** add a point-in-time and exclude-stale temporal recall filter ([88c5ab6](https://github.com/Arakiss/nahuali/commit/88c5ab606af9334540576f7ef7af6e5db52774f2))
* **core:** add an append-only consistency verdict over the ledger ([b070c83](https://github.com/Arakiss/nahuali/commit/b070c833020009d8252b97bd0e3d6bd39bd63ac2))
* **core:** add an attestation keyring for key rotation and revocation ([b26219e](https://github.com/Arakiss/nahuali/commit/b26219ef0d79bf4fa178b3e56274b08a3a4655b4))
* **core:** add an opt-in tamper-evident hash-chained ledger ([09dfd0e](https://github.com/Arakiss/nahuali/commit/09dfd0e2b01cc27673eb52d2765173df85427c52))
* **core:** add keyring-aware chain-tip attestation verification ([d18c775](https://github.com/Arakiss/nahuali/commit/d18c775b7a7f922855a2b191430a55adfeb01ec6))
* **core:** add Merkle inclusion proofs over the ledger ([3b99760](https://github.com/Arakiss/nahuali/commit/3b997601e376540e8b7296c517fbff4cc02c0791))
* **core:** add non-mutating ledger audit/diff ([44b2b07](https://github.com/Arakiss/nahuali/commit/44b2b079b3bc326ec0eefa2a4b07e2488a492944))
* **core:** add RepairApplied event and projection ([837798b](https://github.com/Arakiss/nahuali/commit/837798bccfe15573829b0245979e8e081c27bd54))
* **core:** add self-repair contract types and autonomy classifier ([de465f9](https://github.com/Arakiss/nahuali/commit/de465f986c96b28e25e8f295815fa4c4b6016feb))
* **core:** add the Attestation Recovery Profile (ARP) benchmark ([5b91ed1](https://github.com/Arakiss/nahuali/commit/5b91ed1b53955a0cbedcbad10e93a4943e47d3a5))
* **core:** add the composed memory trust report ([f1d566b](https://github.com/Arakiss/nahuali/commit/f1d566b6b0860af3b1533328b6add774221deac7))
* **core:** audit confidence-vs-provenance and flag overconfident unsourced memory ([2e2357c](https://github.com/Arakiss/nahuali/commit/2e2357c5a9c6f0e721392a34c8c1f08d7e49a851))
* **core:** derive provenance-coverage and overconfidence rates per kind ([23415a9](https://github.com/Arakiss/nahuali/commit/23415a90620123fc1781f8ad613ba9c3d2e1f900))
* **core:** expose the LIVR integrity harness as a reproducible library function ([a3517e7](https://github.com/Arakiss/nahuali/commit/a3517e7c3039065130aadcafb365dfd627ee786b))
* **core:** flag a dormant store as stale so it doesn't silently certify ([610b17d](https://github.com/Arakiss/nahuali/commit/610b17dda1853ec3859448fd9305af78ee8a44bc))
* **core:** improve the default embedder with character n-grams ([12742e1](https://github.com/Arakiss/nahuali/commit/12742e1925fcbaba4e9269694df9d6af624d4a3c))
* **core:** sign and verify the tamper-evident ledger tip with Ed25519 ([f04b6f3](https://github.com/Arakiss/nahuali/commit/f04b6f323d2d3a755260ec53430b9ab5533ba9fd))
* **core:** surface deterministic repair-need nudge ([5f2f24d](https://github.com/Arakiss/nahuali/commit/5f2f24df953712c2bc8747f2fb9260f5ff835d44))
* **core:** surface recency-resolved supersession as a warn-level signal ([95d00e0](https://github.com/Arakiss/nahuali/commit/95d00e0ff8bf06fb66e089a82328c0bdbeabef2c))
* **core:** surface the ledger Merkle root in audit and trust-report integrity ([a4dd5ec](https://github.com/Arakiss/nahuali/commit/a4dd5ec42ae25fac81ddc29188908f34d5412dac))
* **core:** validate and apply LLM repair proposals ([4d664f9](https://github.com/Arakiss/nahuali/commit/4d664f90cdd2a46edbb4983bf2dedbf1705b6bb4))
* initial public beta ([4157a62](https://github.com/Arakiss/nahuali/commit/4157a62b1f4b3c6ff97f6dda61cada69990652c6))
* require chained records in strict validation ([42dc62b](https://github.com/Arakiss/nahuali/commit/42dc62bf414b6c694763df32cac581a95ea2e99c))
* **semantic:** add optional local model2vec embedder behind a feature flag ([d224799](https://github.com/Arakiss/nahuali/commit/d2247997eb31b89a3aee91eb2b59a7a71adf2232))


### Bug fixes

* **core:** compute report-layer health at the requested timestamp ([de98e4a](https://github.com/Arakiss/nahuali/commit/de98e4adf124e621aaf71e556f21a43283ee950b))
* **core:** cover StaleEpisode in the remaining signal-kind matches ([b7db42f](https://github.com/Arakiss/nahuali/commit/b7db42ff16d466276e1fe45276abd5ca8edb2c3d))
* **core:** don't flag isolated entities in a knowledge-free episode log ([fdb4107](https://github.com/Arakiss/nahuali/commit/fdb410738f96b4a52537a488964cc634753409d0))
* **core:** reject fabricated evidence citations on the direct write path ([b467c53](https://github.com/Arakiss/nahuali/commit/b467c53bf09eb2808fae179d4e7db550cb6cf07c))
* **semantic:** bump the deterministic embedding identity after the n-gram change ([d04378b](https://github.com/Arakiss/nahuali/commit/d04378b2dd775fdf3c84eb25c4c24807964b41f7))


### Performance

* apply projection updates incrementally ([a74a25e](https://github.com/Arakiss/nahuali/commit/a74a25ef04aff2414b303aff586f3f112ef67707))
* **core:** batch interchange and ingestion imports into one ledger flush ([4808121](https://github.com/Arakiss/nahuali/commit/4808121aeeee95ae68b83fcae6c1115e8cfe2c48))

## [0.5.0-beta.0](https://github.com/Arakiss/nahuali/compare/nahuali-core-v0.4.0-beta.0...nahuali-core-v0.5.0-beta.0) (2026-06-14)


### Features

* require chained records in strict validation ([42dc62b](https://github.com/Arakiss/nahuali/commit/42dc62bf414b6c694763df32cac581a95ea2e99c))


### Performance

* apply projection updates incrementally ([a74a25e](https://github.com/Arakiss/nahuali/commit/a74a25ef04aff2414b303aff586f3f112ef67707))

## [0.4.0-beta.0](https://github.com/Arakiss/nahuali/compare/nahuali-core-v0.3.0-beta.0...nahuali-core-v0.4.0-beta.0) (2026-06-13)


### Features

* add result-level trust to authority recall ([c8cd149](https://github.com/Arakiss/nahuali/commit/c8cd1499fe0117af3a920edb2ee95dcab0d52fd4))
* **cli:** anchor the ledger audit on a signed attestation ([cb5bd92](https://github.com/Arakiss/nahuali/commit/cb5bd9228024fbacd7c018d8c14ab601daad4ae4))
* **core:** add a non-destructive semantic index sync ([ec6df22](https://github.com/Arakiss/nahuali/commit/ec6df2232c72db0b1587e720ee0399f8e74b456a))
* **core:** add a point-in-time and exclude-stale temporal recall filter ([88c5ab6](https://github.com/Arakiss/nahuali/commit/88c5ab606af9334540576f7ef7af6e5db52774f2))
* **core:** add an append-only consistency verdict over the ledger ([b070c83](https://github.com/Arakiss/nahuali/commit/b070c833020009d8252b97bd0e3d6bd39bd63ac2))
* **core:** add an attestation keyring for key rotation and revocation ([b26219e](https://github.com/Arakiss/nahuali/commit/b26219ef0d79bf4fa178b3e56274b08a3a4655b4))
* **core:** add an opt-in tamper-evident hash-chained ledger ([09dfd0e](https://github.com/Arakiss/nahuali/commit/09dfd0e2b01cc27673eb52d2765173df85427c52))
* **core:** add keyring-aware chain-tip attestation verification ([d18c775](https://github.com/Arakiss/nahuali/commit/d18c775b7a7f922855a2b191430a55adfeb01ec6))
* **core:** add Merkle inclusion proofs over the ledger ([3b99760](https://github.com/Arakiss/nahuali/commit/3b997601e376540e8b7296c517fbff4cc02c0791))
* **core:** add non-mutating ledger audit/diff ([44b2b07](https://github.com/Arakiss/nahuali/commit/44b2b079b3bc326ec0eefa2a4b07e2488a492944))
* **core:** add the Attestation Recovery Profile (ARP) benchmark ([5b91ed1](https://github.com/Arakiss/nahuali/commit/5b91ed1b53955a0cbedcbad10e93a4943e47d3a5))
* **core:** add the composed memory trust report ([f1d566b](https://github.com/Arakiss/nahuali/commit/f1d566b6b0860af3b1533328b6add774221deac7))
* **core:** audit confidence-vs-provenance and flag overconfident unsourced memory ([2e2357c](https://github.com/Arakiss/nahuali/commit/2e2357c5a9c6f0e721392a34c8c1f08d7e49a851))
* **core:** derive provenance-coverage and overconfidence rates per kind ([23415a9](https://github.com/Arakiss/nahuali/commit/23415a90620123fc1781f8ad613ba9c3d2e1f900))
* **core:** expose the LIVR integrity harness as a reproducible library function ([a3517e7](https://github.com/Arakiss/nahuali/commit/a3517e7c3039065130aadcafb365dfd627ee786b))
* **core:** flag a dormant store as stale so it doesn't silently certify ([610b17d](https://github.com/Arakiss/nahuali/commit/610b17dda1853ec3859448fd9305af78ee8a44bc))
* **core:** improve the default embedder with character n-grams ([12742e1](https://github.com/Arakiss/nahuali/commit/12742e1925fcbaba4e9269694df9d6af624d4a3c))
* **core:** sign and verify the tamper-evident ledger tip with Ed25519 ([f04b6f3](https://github.com/Arakiss/nahuali/commit/f04b6f323d2d3a755260ec53430b9ab5533ba9fd))
* **core:** surface recency-resolved supersession as a warn-level signal ([95d00e0](https://github.com/Arakiss/nahuali/commit/95d00e0ff8bf06fb66e089a82328c0bdbeabef2c))
* **core:** surface the ledger Merkle root in audit and trust-report integrity ([a4dd5ec](https://github.com/Arakiss/nahuali/commit/a4dd5ec42ae25fac81ddc29188908f34d5412dac))
* initial public beta ([4157a62](https://github.com/Arakiss/nahuali/commit/4157a62b1f4b3c6ff97f6dda61cada69990652c6))
* **semantic:** add optional local model2vec embedder behind a feature flag ([d224799](https://github.com/Arakiss/nahuali/commit/d2247997eb31b89a3aee91eb2b59a7a71adf2232))


### Bug fixes

* **core:** compute report-layer health at the requested timestamp ([de98e4a](https://github.com/Arakiss/nahuali/commit/de98e4adf124e621aaf71e556f21a43283ee950b))
* **core:** cover StaleEpisode in the remaining signal-kind matches ([b7db42f](https://github.com/Arakiss/nahuali/commit/b7db42ff16d466276e1fe45276abd5ca8edb2c3d))
* **core:** don't flag isolated entities in a knowledge-free episode log ([fdb4107](https://github.com/Arakiss/nahuali/commit/fdb410738f96b4a52537a488964cc634753409d0))
* **core:** reject fabricated evidence citations on the direct write path ([b467c53](https://github.com/Arakiss/nahuali/commit/b467c53bf09eb2808fae179d4e7db550cb6cf07c))
* **semantic:** bump the deterministic embedding identity after the n-gram change ([d04378b](https://github.com/Arakiss/nahuali/commit/d04378b2dd775fdf3c84eb25c4c24807964b41f7))


### Performance

* **core:** batch interchange and ingestion imports into one ledger flush ([4808121](https://github.com/Arakiss/nahuali/commit/4808121aeeee95ae68b83fcae6c1115e8cfe2c48))

## [0.3.0-beta.0](https://github.com/Arakiss/nahuali/compare/nahuali-core-v0.2.0-beta.0...nahuali-core-v0.3.0-beta.0) (2026-06-02)


### Features

* add result-level trust to authority recall ([c8cd149](https://github.com/Arakiss/nahuali/commit/c8cd1499fe0117af3a920edb2ee95dcab0d52fd4))
* **core:** add an opt-in tamper-evident hash-chained ledger ([09dfd0e](https://github.com/Arakiss/nahuali/commit/09dfd0e2b01cc27673eb52d2765173df85427c52))
* initial public beta ([4157a62](https://github.com/Arakiss/nahuali/commit/4157a62b1f4b3c6ff97f6dda61cada69990652c6))
* **semantic:** add optional local model2vec embedder behind a feature flag ([d224799](https://github.com/Arakiss/nahuali/commit/d2247997eb31b89a3aee91eb2b59a7a71adf2232))

## [0.2.0-beta.0](https://github.com/Arakiss/nahuali/compare/nahuali-core-v0.1.0-beta.0...nahuali-core-v0.2.0-beta.0) (2026-06-01)


### Features

* add result-level trust to authority recall ([c8cd149](https://github.com/Arakiss/nahuali/commit/c8cd1499fe0117af3a920edb2ee95dcab0d52fd4))
* initial public beta ([4157a62](https://github.com/Arakiss/nahuali/commit/4157a62b1f4b3c6ff97f6dda61cada69990652c6))
* **semantic:** add optional local model2vec embedder behind a feature flag ([d224799](https://github.com/Arakiss/nahuali/commit/d2247997eb31b89a3aee91eb2b59a7a71adf2232))
