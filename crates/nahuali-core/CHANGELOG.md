# Changelog

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
