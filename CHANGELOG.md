# Changelog

This is the release history of Nahuali as one product. It covers the CLI, MCP
server, local HTTP API, Rust core, and the user-facing terminal interface.
Internal crate changelogs are technical appendices, not separate product
release histories.

## Unreleased

No user-facing changes yet.

## [0.8.0-beta.1](https://github.com/Arakiss/nahuali/compare/v0.8.0-beta.0...v0.8.0-beta.1) (2026-07-15)


### ⚠ BREAKING CHANGES

* default builds now enable attestation and fail-closed validation; --require-chained becomes --allow-unchained (opt-out); new event envelopes are version 2 with SHA-256 checksums.
* the default validation verdict for an unchained ledger flips from accept to reject, the `--require-chained` flag is renamed to `--allow-unchained`, and new records carry a SHA-256 checksum at envelope version 2.
* --database now refuses names that normalization would rewrite (e.g. ./memory); the error suggests the normalized candidate. The documented idiom and all scripts/tests migrated to clean identifiers.
* **core:** KnowledgeHealth and SelfInspectionSummary gain public fields (signal_count, distinct_flagged_record_count), SelfInspectionFindingKind gains a LabelArtifact variant, and KnowledgeHealth.blind_spot_count changes meaning (coverage subset, not total signals). CLI JSON output reflects the new fields and semantics.

### merge

* config integrity + signed installer (C2+C6) ([94d9c22](https://github.com/Arakiss/nahuali/commit/94d9c22228c2d43ef2791fadb5af1154f628c50f))
* flagship defaults true by default (D1+D2+D3+C3) ([3cb436a](https://github.com/Arakiss/nahuali/commit/3cb436a62b33f6094aa6e69d106535cad2f24784))


### New

* add result-level trust to authority recall ([c8cd149](https://github.com/Arakiss/nahuali/commit/c8cd1499fe0117af3a920edb2ee95dcab0d52fd4))
* **api:** cache the memory engine and type the OpenAPI response schemas ([5d0c742](https://github.com/Arakiss/nahuali/commit/5d0c742d3e13dc044844e5f110027a30d658de2d))
* **api:** expose the ledger audit endpoint ([6a2d88a](https://github.com/Arakiss/nahuali/commit/6a2d88a8d12adcafe0c1e2e7159f6f21e5627fd0))
* **api:** expose the temporal recall filter and a semantic-sync endpoint ([e8152bc](https://github.com/Arakiss/nahuali/commit/e8152bc4fc0368764763089a94ad1ee122d6c166))
* **api:** expose the trust-report endpoint ([e2366e8](https://github.com/Arakiss/nahuali/commit/e2366e87732ffd12f110efc2bcf0e56245bb9153))
* **api:** forward the tamper-evidence and local-embeddings features ([8b8e905](https://github.com/Arakiss/nahuali/commit/8b8e9054a66d15d8e75dbbcd4736fd8ea80a42fb))
* **api:** wrap transport errors in the structured envelope, add /health, and reject unknown fields ([f5e1b4c](https://github.com/Arakiss/nahuali/commit/f5e1b4c548654c7db860aa77a0d174ab84584a89))
* **cli,mcp:** surface signal_count and distinct_flagged_record_count ([68c28df](https://github.com/Arakiss/nahuali/commit/68c28df23d265d741d311d9b25f28e65ddcd4d98))
* **cli:** accept 'episode' as a visible alias for 'remember' ([a9dbecd](https://github.com/Arakiss/nahuali/commit/a9dbecdb475fe1774972ff3cb41dbc2149faffaf))
* **cli:** add --verbose to surface connection and timing on stderr ([962c72d](https://github.com/Arakiss/nahuali/commit/962c72d3f2607e3f3ccd05e79d05743d1a90c487))
* **cli:** add `nahuali explore` — the interactive governance cockpit ([bf68553](https://github.com/Arakiss/nahuali/commit/bf685538d5f42fdd0baa5e5c4a37afd045d76b8c))
* **cli:** add `nahuali init` to wire the agent harness ([1669f56](https://github.com/Arakiss/nahuali/commit/1669f564e8d9a1456edaa75dacfe95f441640116))
* **cli:** add nahuali repair command ([2226e35](https://github.com/Arakiss/nahuali/commit/2226e35d676fff8fc7bcc47be19cf4e5319d131e))
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
* **cli:** group commands in --help, document flags, add examples, and add shell completions ([a0928e0](https://github.com/Arakiss/nahuali/commit/a0928e0fddb7ea9e410bf044bef3d2391ff4a61c))
* **cli:** make the briefing render scannable instead of a wall of text ([d927de8](https://github.com/Arakiss/nahuali/commit/d927de80b9208307ab82521d2c67dfaa1299c57d))
* **cli:** render reports in the shared nahuali-ui clay-on-coffee palette ([cc93620](https://github.com/Arakiss/nahuali/commit/cc936207bb9da882f73250f66b49d5fc4660c7a7))
* **cli:** render the trust report as a self-contained HTML dossier ([9592048](https://github.com/Arakiss/nahuali/commit/9592048c1fe8173d8eac4ede2c19ef71f48fd9ae))
* **cli:** richer human write confirmations — say what happened, not an id ([3b88629](https://github.com/Arakiss/nahuali/commit/3b88629c453eae4648fdd7aecd45265aff375b81))
* **cli:** run the full demo on a default build ([f76691f](https://github.com/Arakiss/nahuali/commit/f76691f39af2958890ba55bc81263d9245b58339))
* **cli:** surface the configured archive in the briefing ([6770f5e](https://github.com/Arakiss/nahuali/commit/6770f5eb9f58e831ac90cb0212ee15116d88c501))
* **cli:** surface the trust verdict by default in plain language with color ([b1d2d97](https://github.com/Arakiss/nahuali/commit/b1d2d9749769f8c0f7a4a7ce10c899fa9922b2c6))
* **cli:** verify attestations against a trusted keyring via --keyring ([80278b4](https://github.com/Arakiss/nahuali/commit/80278b4d18d770eafdb426b8b213e4772918ba66))
* **config:** resolve database name once with flag&gt;env&gt;default precedence ([2afe8c1](https://github.com/Arakiss/nahuali/commit/2afe8c1b7e93fa5f28941b5fc8835a506e36ad6f))
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
* **core:** attach a per-result trust verdict to hybrid recall ([1d2d5e7](https://github.com/Arakiss/nahuali/commit/1d2d5e77c090ce7a6d43f2a21668befaac58bd6f))
* **core:** audit confidence-vs-provenance and flag overconfident unsourced memory ([2e2357c](https://github.com/Arakiss/nahuali/commit/2e2357c5a9c6f0e721392a34c8c1f08d7e49a851))
* **core:** derive provenance-coverage and overconfidence rates per kind ([23415a9](https://github.com/Arakiss/nahuali/commit/23415a90620123fc1781f8ad613ba9c3d2e1f900))
* **core:** expose the LIVR integrity harness as a reproducible library function ([a3517e7](https://github.com/Arakiss/nahuali/commit/a3517e7c3039065130aadcafb365dfd627ee786b))
* **core:** fix self-inspect signal quality (R1-R5) ([1e384ab](https://github.com/Arakiss/nahuali/commit/1e384ab14e811a031576bc57093adc142e7c5270))
* **core:** flag a dormant store as stale so it doesn't silently certify ([610b17d](https://github.com/Arakiss/nahuali/commit/610b17dda1853ec3859448fd9305af78ee8a44bc))
* **core:** improve the default embedder with character n-grams ([12742e1](https://github.com/Arakiss/nahuali/commit/12742e1925fcbaba4e9269694df9d6af624d4a3c))
* **core:** sign and verify the tamper-evident ledger tip with Ed25519 ([f04b6f3](https://github.com/Arakiss/nahuali/commit/f04b6f323d2d3a755260ec53430b9ab5533ba9fd))
* **core:** surface deterministic repair-need nudge ([5f2f24d](https://github.com/Arakiss/nahuali/commit/5f2f24df953712c2bc8747f2fb9260f5ff835d44))
* **core:** surface recency-resolved supersession as a warn-level signal ([95d00e0](https://github.com/Arakiss/nahuali/commit/95d00e0ff8bf06fb66e089a82328c0bdbeabef2c))
* **core:** surface the ledger Merkle root in audit and trust-report integrity ([a4dd5ec](https://github.com/Arakiss/nahuali/commit/a4dd5ec42ae25fac81ddc29188908f34d5412dac))
* **core:** validate and apply LLM repair proposals ([4d664f9](https://github.com/Arakiss/nahuali/commit/4d664f90cdd2a46edbb4983bf2dedbf1705b6bb4))
* default to a fail-closed, SHA-256-checksummed, attested ledger ([118488f](https://github.com/Arakiss/nahuali/commit/118488fa936e9d1a83bc6d66c1816a77140f97f5))
* expose Merkle inclusion proofs from audit ([e79c390](https://github.com/Arakiss/nahuali/commit/e79c390816f6fa678029130c210bea7114a48450))
* initial public beta ([4157a62](https://github.com/Arakiss/nahuali/commit/4157a62b1f4b3c6ff97f6dda61cada69990652c6))
* **install:** make checksum mandatory and verify the Sigstore signature ([afcbfdb](https://github.com/Arakiss/nahuali/commit/afcbfdbd7d9fe502a3cfdb798467a83c663a8a68))
* make memory trust visible on first run ([15fc455](https://github.com/Arakiss/nahuali/commit/15fc455cb3eeceeecdb6a3b6e9e17c9eb31495fa))
* make trusted memory usable without services ([3e1d70b](https://github.com/Arakiss/nahuali/commit/3e1d70bd5ed09f18ec3a0028299ad16c6a1aa2f4))
* **mcp:** expose the ledger audit tool ([2307a53](https://github.com/Arakiss/nahuali/commit/2307a53e062818ee99253f9d82154b68f0e8d857))
* **mcp:** expose the temporal recall filter and a semantic_sync tool ([fb2441a](https://github.com/Arakiss/nahuali/commit/fb2441a1dbba34e0fee8dbdf2c2faf9c03a3394f))
* **mcp:** expose the trust_report tool ([041d342](https://github.com/Arakiss/nahuali/commit/041d342557f1650066833453a4a173c776417611))
* **mcp:** forward the tamper-evidence and local-embeddings features ([d71e4c6](https://github.com/Arakiss/nahuali/commit/d71e4c649500e77c60383a3004e1c79c214ce40c))
* **mcp:** guide agents with when/next tool descriptions, annotations, and argument docs ([27ce050](https://github.com/Arakiss/nahuali/commit/27ce0506abde38417f690701de397ec8e7f6b2ac))
* **mcp:** return a typed output view for memory_hook ([145fa00](https://github.com/Arakiss/nahuali/commit/145fa00509599a4da86d23c711e352b8e2833f49))
* **mcp:** return typed output views for briefing, self-inspect, review, and graph ([3305cf1](https://github.com/Arakiss/nahuali/commit/3305cf1c720f782a159b3fa5a00a869e3e7e7d84))
* **mcp:** type the consolidation_plan tool output ([81427b7](https://github.com/Arakiss/nahuali/commit/81427b741b8fb7c4878d1bbced87d0ecf343cdbd))
* **mcp:** type the graph projection tool outputs ([167f816](https://github.com/Arakiss/nahuali/commit/167f81657078c16fde8604cc23d56b0699ff428a))
* **mcp:** type the ingest tool outputs ([f18cf58](https://github.com/Arakiss/nahuali/commit/f18cf58c9c74a822c61dbe8a36547ffd97503e43))
* **mcp:** type the intention reconciliation and goal-progress outputs ([19b5927](https://github.com/Arakiss/nahuali/commit/19b5927d9483dd24dd4bf3e7fed2ada8c587819e))
* **mcp:** type the proactive/deadline/anomaly tool outputs ([4da61c0](https://github.com/Arakiss/nahuali/commit/4da61c05499d9138d75b57d7216d13efec246cac))
* **mcp:** type the reflect tool output ([64d3b60](https://github.com/Arakiss/nahuali/commit/64d3b603a301088ad87f90b23b187c0a4c074fad))
* **mcp:** type the review_resolve tool output ([0c91736](https://github.com/Arakiss/nahuali/commit/0c91736089ef7ed90c97d6ddd1ad6122aaece93d))
* **mcp:** type the semantic index tool outputs ([e765d02](https://github.com/Arakiss/nahuali/commit/e765d0254e6eec15fc0be49b87e4c5ff5384dd1d))
* **mcp:** type the sleep report view ([eea26ae](https://github.com/Arakiss/nahuali/commit/eea26ae1923f1ea8fa7f08639afd64832439802c))
* **regression:** add the Contradiction & Staleness Detection Rate fixture ([cb32858](https://github.com/Arakiss/nahuali/commit/cb328583f293daf01ee9b071602e083e7903cf74))
* **regression:** add the Provenance Coverage Rate benchmark fixture ([750e631](https://github.com/Arakiss/nahuali/commit/750e63145a699e7e58ca2a8c6307183ed6d834b1))
* **regression:** add the Trust Verdict Soundness (TVS) benchmark fixture ([e65feb7](https://github.com/Arakiss/nahuali/commit/e65feb7054089316f47dd532ed435960b5eaf2a0))
* **regression:** emit a versioned ARP report under --arp ([6be8295](https://github.com/Arakiss/nahuali/commit/6be829579e56271d42a700cac7b067285ca8f12a))
* **regression:** emit a versioned LIVR integrity report under --livr ([178ef18](https://github.com/Arakiss/nahuali/commit/178ef1816ffaa97d4dda6d6ef79615ff1a52ac74))
* require chained records in strict validation ([42dc62b](https://github.com/Arakiss/nahuali/commit/42dc62bf414b6c694763df32cac581a95ea2e99c))
* **scripts:** add a one-line curl installer ([aca969e](https://github.com/Arakiss/nahuali/commit/aca969ecc8326f5ff56fe91d385bf83bd86aae53))
* **semantic:** add optional local model2vec embedder behind a feature flag ([d224799](https://github.com/Arakiss/nahuali/commit/d2247997eb31b89a3aee91eb2b59a7a71adf2232))
* **ui:** add a governance signals bar and richer detail to explore ([92fd1f1](https://github.com/Arakiss/nahuali/commit/92fd1f1008628eaf04599f3242a0ec4abd4321af))
* **ui:** add comfy-table-backed table rendering to nahuali-ui ([3f97526](https://github.com/Arakiss/nahuali/commit/3f97526043ceb88b4ff37c24ef3713d1421ed985))
* **ui:** add the explore governance cockpit (ratatui), feature-gated ([c7cc8fb](https://github.com/Arakiss/nahuali/commit/c7cc8fb3761d27ad6c4fbfc973bf7f7e27e85c46))
* **ui:** axolotl nahual mascot bound to the trust verdict in explore ([4e72d49](https://github.com/Arakiss/nahuali/commit/4e72d4998d53441f5364d7b2ab02d12207997ba0))
* **ui:** filter the explore cockpit by memory kind ([f8acc8b](https://github.com/Arakiss/nahuali/commit/f8acc8b57ee18d7e8aa934475b3a0b7ac70a91b4))
* **ui:** polish the explore cockpit — clipped titles, selection bar, item count ([9865d2c](https://github.com/Arakiss/nahuali/commit/9865d2cd8e989c707d1a604a0128e000f20a1d12))
* **ui:** relocate the nahual to subtle homes ([af68a6a](https://github.com/Arakiss/nahuali/commit/af68a6ab4f0c83fd2dd78d11eeed6b6f89ce773b))
* **ui:** scaffold nahuali-ui, the clay-on-coffee terminal presentation crate ([7ec8354](https://github.com/Arakiss/nahuali/commit/7ec8354cbeb07777872314bc316e81deabbf61ec))
* **ui:** surface ledger integrity in the explore cockpit header ([fa8c14c](https://github.com/Arakiss/nahuali/commit/fa8c14ca21c5bb6bad7ec26ea8be6b95811154fd))


### Fixed

* align transport tamper-evidence defaults ([50a0872](https://github.com/Arakiss/nahuali/commit/50a087250d2d970bfaf2a31639200d65ff9b3121))
* attach SBOM on release reruns ([e56504b](https://github.com/Arakiss/nahuali/commit/e56504b4b00750efff519b4c6a36b2c8639333cb))
* bind MCP publication to the canonical namespace ([0c1f39d](https://github.com/Arakiss/nahuali/commit/0c1f39da34de32b957ba7b76c0440683237da86f))
* bind MCP publication to the canonical namespace ([324d62b](https://github.com/Arakiss/nahuali/commit/324d62b78f894d06142ddcfe9fd0ea964366b212))
* **cli:** emit the archive section in recall --json and briefing --json ([c5a8de5](https://github.com/Arakiss/nahuali/commit/c5a8de566b3669207143273e7fd09136f51475ed))
* **cli:** list the attest commands in the grouped help ([55f1536](https://github.com/Arakiss/nahuali/commit/55f153601244bcb65a76c976b4e9340e2559c45f))
* **cli:** make the demo fallback truthful about its own build ([980d951](https://github.com/Arakiss/nahuali/commit/980d951c76495c8b78c3dfeba633931ea6991ed9))
* **cli:** reject --confidence values outside 0.0..=1.0 ([1adc5e8](https://github.com/Arakiss/nahuali/commit/1adc5e8faaf705659b18cc6d8a3e555cbe7eb2dd))
* **cli:** render the trust verdict labels in English ([9844dcb](https://github.com/Arakiss/nahuali/commit/9844dcb3e1fa2c47edffc329918fa17344b6752e))
* **cli:** replace leftover Spanish trust labels in human output ([fa64689](https://github.com/Arakiss/nahuali/commit/fa6468976ff274fe605ebee6f30b9e750cb4c103))
* coordinate embedded conflict recovery ([cb12252](https://github.com/Arakiss/nahuali/commit/cb12252b157da69b10096c67ad4939a8339f5dfd))
* **core:** compute report-layer health at the requested timestamp ([de98e4a](https://github.com/Arakiss/nahuali/commit/de98e4adf124e621aaf71e556f21a43283ee950b))
* **core:** cover StaleEpisode in the remaining signal-kind matches ([b7db42f](https://github.com/Arakiss/nahuali/commit/b7db42ff16d466276e1fe45276abd5ca8edb2c3d))
* **core:** don't flag isolated entities in a knowledge-free episode log ([fdb4107](https://github.com/Arakiss/nahuali/commit/fdb410738f96b4a52537a488964cc634753409d0))
* **core:** reject fabricated evidence citations on the direct write path ([b467c53](https://github.com/Arakiss/nahuali/commit/b467c53bf09eb2808fae179d4e7db550cb6cf07c))
* eliminate embedded session startup races ([3079f55](https://github.com/Arakiss/nahuali/commit/3079f552145aff82513bb832b03afaa5e690aeb3))
* give large test graphs independent runners ([f2a99a4](https://github.com/Arakiss/nahuali/commit/f2a99a474ea470f04c89d16ba3eed91e22505ffe))
* govern superseded MCP registry versions ([974a44b](https://github.com/Arakiss/nahuali/commit/974a44b38f3b6be5a40bd425607248b1b2a5db23))
* keep internal crates publishable ([e002bc1](https://github.com/Arakiss/nahuali/commit/e002bc1a341dd07bf3ebc9366059d7fc40a65aa7))
* keep MCP package metadata aligned with releases ([6039be0](https://github.com/Arakiss/nahuali/commit/6039be0515d6d3f7f93b4b6d1a8ca1b3700033a0))
* keep release PR checks enforceable ([9182f1f](https://github.com/Arakiss/nahuali/commit/9182f1f93ac91a00d009f666898468dd452dbf49))
* make coverage publication enforceable ([7e3c09e](https://github.com/Arakiss/nahuali/commit/7e3c09edaf549922acccaa46c39debd02e201b34))
* make supply-chain checks runner-portable ([6540c1b](https://github.com/Arakiss/nahuali/commit/6540c1b726edd209ec0f46a9246cc58fb55d7877))
* preserve component release boundaries ([56e54ef](https://github.com/Arakiss/nahuali/commit/56e54ef860d374807d45a8ebfa98ee60fa00d4e5))
* prevent concurrent store initialization conflicts ([4c13988](https://github.com/Arakiss/nahuali/commit/4c1398858b5f926b545c99cfa87856dd715aaedd))
* restore coherent pre-1.0 release governance ([66af649](https://github.com/Arakiss/nahuali/commit/66af6495aea857b30706fb8be7c1d3d7ef7285a7))
* restore coherent pre-1.0 release governance ([19f34da](https://github.com/Arakiss/nahuali/commit/19f34dae5c70eda372df8392f013cb2195c230bd))
* retry embedded database selection conflicts ([1ff89ec](https://github.com/Arakiss/nahuali/commit/1ff89ec8b5dabd7a956576316540200bceaf513b))
* retry embedded transaction conflicts consistently ([baa2330](https://github.com/Arakiss/nahuali/commit/baa23301a76471c37aad2c06edd9d5406a0ef889))
* retry transient embedded schema conflicts ([1cc4096](https://github.com/Arakiss/nahuali/commit/1cc4096c44cc4c832ffb5073f36b30590a0a3d6d))
* **semantic:** bump the deterministic embedding identity after the n-gram change ([d04378b](https://github.com/Arakiss/nahuali/commit/d04378b2dd775fdf3c84eb25c4c24807964b41f7))
* stabilize CLI JSON output contract ([b9f5c64](https://github.com/Arakiss/nahuali/commit/b9f5c64a12d0791ef6f05d22c38a8ea8cbce47d8))


### Performance

* apply projection updates incrementally ([a74a25e](https://github.com/Arakiss/nahuali/commit/a74a25ef04aff2414b303aff586f3f112ef67707))
* **core:** batch interchange and ingestion imports into one ledger flush ([4808121](https://github.com/Arakiss/nahuali/commit/4808121aeeee95ae68b83fcae6c1115e8cfe2c48))


### Changed

* **cli:** share the scannable episode rendering across reports ([f3f8375](https://github.com/Arakiss/nahuali/commit/f3f83754e2fac3dba98310796174c162589ee3d8))
* **mcp:** split protocol::views into per-family modules ([7f69412](https://github.com/Arakiss/nahuali/commit/7f694123273a54db351aa1b43b7808bd54aecc79))

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
- No hosted accounts, teams, billing, managed sync, or managed uptime.
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
