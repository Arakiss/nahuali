# Changelog

## [0.4.1-beta.0](https://github.com/Arakiss/nahuali/compare/nahuali-cli-v0.4.0-beta.0...nahuali-cli-v0.4.1-beta.0) (2026-06-13)

## [0.4.0-beta.0](https://github.com/Arakiss/nahuali/compare/nahuali-cli-v0.3.0-beta.0...nahuali-cli-v0.4.0-beta.0) (2026-06-13)


### Features

* **cli:** accept 'episode' as a visible alias for 'remember' ([a9dbecd](https://github.com/Arakiss/nahuali/commit/a9dbecdb475fe1774972ff3cb41dbc2149faffaf))
* **cli:** add --verbose to surface connection and timing on stderr ([962c72d](https://github.com/Arakiss/nahuali/commit/962c72d3f2607e3f3ccd05e79d05743d1a90c487))
* **cli:** add `nahuali explore` — the interactive governance cockpit ([bf68553](https://github.com/Arakiss/nahuali/commit/bf685538d5f42fdd0baa5e5c4a37afd045d76b8c))
* **cli:** add `nahuali init` to wire the agent harness ([1669f56](https://github.com/Arakiss/nahuali/commit/1669f564e8d9a1456edaa75dacfe95f441640116))
* **cli:** add reconcile to rebuild derived tiers from the ledger ([e14cfc4](https://github.com/Arakiss/nahuali/commit/e14cfc4cbcd41445084656a5c162ecf2fa4f0345))
* **cli:** add the ledger audit command ([5f842e5](https://github.com/Arakiss/nahuali/commit/5f842e56fe6c7b7d6150393bf8046f3b628c7e7d))
* **cli:** add the semantic-sync command ([828e391](https://github.com/Arakiss/nahuali/commit/828e39113f9b362ac435f81a4b0a99e2ba9c447d))
* **cli:** add the trust-report command ([e5bf81b](https://github.com/Arakiss/nahuali/commit/e5bf81b3221cfc03cbf84f34594f5dd2e08f45bd))
* **cli:** add the zero-dependency `nahuali demo` first-look ([07c8ee0](https://github.com/Arakiss/nahuali/commit/07c8ee038319a5da674b708098d62838e5be115c))
* **cli:** anchor the ledger audit on a signed attestation ([cb5bd92](https://github.com/Arakiss/nahuali/commit/cb5bd9228024fbacd7c018d8c14ab601daad4ae4))
* **cli:** enable tamper-evidence by default ([39f0544](https://github.com/Arakiss/nahuali/commit/39f0544baf4ef7ee404a15162f6ab4e5847e8dfa))
* **cli:** expose the temporal recall filter via --as-of-ms and --max-age-days ([7f7c7c5](https://github.com/Arakiss/nahuali/commit/7f7c7c53e5751b9b0f977777451e5c35f0dd6fd1))
* **cli:** fail calmly when the store is unreachable and auto-start the stack ([0464fae](https://github.com/Arakiss/nahuali/commit/0464faeee419b9b1e568f00e461c5da0f9bd64ec))
* **cli:** federate recall over a read-only archive store with --archive ([ec18635](https://github.com/Arakiss/nahuali/commit/ec18635e6bc4f1b9c53d7c9db0a00c73bd33f09b))
* **cli:** forward local-embeddings so recall can use a local model ([1f2590c](https://github.com/Arakiss/nahuali/commit/1f2590c5313f2d037729dd9ecae53938413b3ac8))
* **cli:** forward the tamper-evidence feature so the audit chain is reachable ([8ebbe87](https://github.com/Arakiss/nahuali/commit/8ebbe8788586941a5ce71b670f6d31b4887660c8))
* **cli:** make the briefing render scannable instead of a wall of text ([d927de8](https://github.com/Arakiss/nahuali/commit/d927de80b9208307ab82521d2c67dfaa1299c57d))
* **cli:** render reports in the shared nahuali-ui clay-on-coffee palette ([cc93620](https://github.com/Arakiss/nahuali/commit/cc936207bb9da882f73250f66b49d5fc4660c7a7))
* **cli:** render the trust report as a self-contained HTML dossier ([9592048](https://github.com/Arakiss/nahuali/commit/9592048c1fe8173d8eac4ede2c19ef71f48fd9ae))
* **cli:** richer human write confirmations — say what happened, not an id ([3b88629](https://github.com/Arakiss/nahuali/commit/3b88629c453eae4648fdd7aecd45265aff375b81))
* **cli:** surface the configured archive in the briefing ([6770f5e](https://github.com/Arakiss/nahuali/commit/6770f5eb9f58e831ac90cb0212ee15116d88c501))
* **cli:** verify attestations against a trusted keyring via --keyring ([80278b4](https://github.com/Arakiss/nahuali/commit/80278b4d18d770eafdb426b8b213e4772918ba66))
* **core:** add a point-in-time and exclude-stale temporal recall filter ([88c5ab6](https://github.com/Arakiss/nahuali/commit/88c5ab606af9334540576f7ef7af6e5db52774f2))
* **core:** sign and verify the tamper-evident ledger tip with Ed25519 ([f04b6f3](https://github.com/Arakiss/nahuali/commit/f04b6f323d2d3a755260ec53430b9ab5533ba9fd))
* **core:** surface the ledger Merkle root in audit and trust-report integrity ([a4dd5ec](https://github.com/Arakiss/nahuali/commit/a4dd5ec42ae25fac81ddc29188908f34d5412dac))
* **ui:** add a governance signals bar and richer detail to explore ([92fd1f1](https://github.com/Arakiss/nahuali/commit/92fd1f1008628eaf04599f3242a0ec4abd4321af))
* **ui:** surface ledger integrity in the explore cockpit header ([fa8c14c](https://github.com/Arakiss/nahuali/commit/fa8c14ca21c5bb6bad7ec26ea8be6b95811154fd))


### Bug fixes

* **cli:** emit the archive section in recall --json and briefing --json ([c5a8de5](https://github.com/Arakiss/nahuali/commit/c5a8de566b3669207143273e7fd09136f51475ed))
* **cli:** list the attest commands in the grouped help ([55f1536](https://github.com/Arakiss/nahuali/commit/55f153601244bcb65a76c976b4e9340e2559c45f))
* **cli:** make the demo fallback truthful about its own build ([980d951](https://github.com/Arakiss/nahuali/commit/980d951c76495c8b78c3dfeba633931ea6991ed9))
* **cli:** render the trust verdict labels in English ([9844dcb](https://github.com/Arakiss/nahuali/commit/9844dcb3e1fa2c47edffc329918fa17344b6752e))
* **cli:** replace leftover Spanish trust labels in human output ([fa64689](https://github.com/Arakiss/nahuali/commit/fa6468976ff274fe605ebee6f30b9e750cb4c103))


### Refactor

* **cli:** share the scannable episode rendering across reports ([f3f8375](https://github.com/Arakiss/nahuali/commit/f3f83754e2fac3dba98310796174c162589ee3d8))

## [0.3.0-beta.0](https://github.com/Arakiss/nahuali/compare/nahuali-cli-v0.2.0-beta.0...nahuali-cli-v0.3.0-beta.0) (2026-06-02)


### Features

* **cli:** group commands in --help, document flags, add examples, and add shell completions ([a0928e0](https://github.com/Arakiss/nahuali/commit/a0928e0fddb7ea9e410bf044bef3d2391ff4a61c))
* **cli:** surface the trust verdict by default in plain language with color ([b1d2d97](https://github.com/Arakiss/nahuali/commit/b1d2d9749769f8c0f7a4a7ce10c899fa9922b2c6))
* **core:** add an opt-in tamper-evident hash-chained ledger ([09dfd0e](https://github.com/Arakiss/nahuali/commit/09dfd0e2b01cc27673eb52d2765173df85427c52))


### Bug fixes

* **cli:** reject --confidence values outside 0.0..=1.0 ([1adc5e8](https://github.com/Arakiss/nahuali/commit/1adc5e8faaf705659b18cc6d8a3e555cbe7eb2dd))

## [0.2.0-beta.0](https://github.com/Arakiss/nahuali/compare/nahuali-cli-v0.1.0-beta.0...nahuali-cli-v0.2.0-beta.0) (2026-06-01)


### Features

* add result-level trust to authority recall ([c8cd149](https://github.com/Arakiss/nahuali/commit/c8cd1499fe0117af3a920edb2ee95dcab0d52fd4))

## 0.1.0-beta.0

Initial public beta of the Nahuali self-inspecting memory engine CLI.
