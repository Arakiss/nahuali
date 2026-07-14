# Changelog

## [2.0.0-beta.0](https://github.com/Arakiss/nahuali/compare/nahuali-mcp-v1.1.0-beta.0...nahuali-mcp-v2.0.0-beta.0) (2026-07-14)


### ⚠ BREAKING CHANGES

* default builds now enable attestation and fail-closed validation; --require-chained becomes --allow-unchained (opt-out); new event envelopes are version 2 with SHA-256 checksums.
* the default validation verdict for an unchained ledger flips from accept to reject, the `--require-chained` flag is renamed to `--allow-unchained`, and new records carry a SHA-256 checksum at envelope version 2.
* --database now refuses names that normalization would rewrite (e.g. ./memory); the error suggests the normalized candidate. The documented idiom and all scripts/tests migrated to clean identifiers.

### merge

* config integrity + signed installer (C2+C6) ([94d9c22](https://github.com/Arakiss/nahuali/commit/94d9c22228c2d43ef2791fadb5af1154f628c50f))
* flagship defaults true by default (D1+D2+D3+C3) ([3cb436a](https://github.com/Arakiss/nahuali/commit/3cb436a62b33f6094aa6e69d106535cad2f24784))


### Features

* add result-level trust to authority recall ([c8cd149](https://github.com/Arakiss/nahuali/commit/c8cd1499fe0117af3a920edb2ee95dcab0d52fd4))
* **cli,mcp:** surface signal_count and distinct_flagged_record_count ([68c28df](https://github.com/Arakiss/nahuali/commit/68c28df23d265d741d311d9b25f28e65ddcd4d98))
* **config:** resolve database name once with flag&gt;env&gt;default precedence ([2afe8c1](https://github.com/Arakiss/nahuali/commit/2afe8c1b7e93fa5f28941b5fc8835a506e36ad6f))
* **core:** add a point-in-time and exclude-stale temporal recall filter ([88c5ab6](https://github.com/Arakiss/nahuali/commit/88c5ab606af9334540576f7ef7af6e5db52774f2))
* **core:** surface the ledger Merkle root in audit and trust-report integrity ([a4dd5ec](https://github.com/Arakiss/nahuali/commit/a4dd5ec42ae25fac81ddc29188908f34d5412dac))
* default to a fail-closed, SHA-256-checksummed, attested ledger ([118488f](https://github.com/Arakiss/nahuali/commit/118488fa936e9d1a83bc6d66c1816a77140f97f5))
* initial public beta ([4157a62](https://github.com/Arakiss/nahuali/commit/4157a62b1f4b3c6ff97f6dda61cada69990652c6))
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


### Bug fixes

* align transport tamper-evidence defaults ([50a0872](https://github.com/Arakiss/nahuali/commit/50a087250d2d970bfaf2a31639200d65ff9b3121))


### Refactor

* **mcp:** split protocol::views into per-family modules ([7f69412](https://github.com/Arakiss/nahuali/commit/7f694123273a54db351aa1b43b7808bd54aecc79))

## [1.1.0-beta.0](https://github.com/Arakiss/nahuali/compare/nahuali-mcp-v1.0.0-beta.0...nahuali-mcp-v1.1.0-beta.0) (2026-07-14)


### Features

* make trusted memory usable without services ([3e1d70b](https://github.com/Arakiss/nahuali/commit/3e1d70bd5ed09f18ec3a0028299ad16c6a1aa2f4))

## [1.0.0-beta.0](https://github.com/Arakiss/nahuali/compare/nahuali-mcp-v0.6.0-beta.0...nahuali-mcp-v1.0.0-beta.0) (2026-07-14)


### ⚠ BREAKING CHANGES

* default builds now enable attestation and fail-closed validation; --require-chained becomes --allow-unchained (opt-out); new event envelopes are version 2 with SHA-256 checksums.
* the default validation verdict for an unchained ledger flips from accept to reject, the `--require-chained` flag is renamed to `--allow-unchained`, and new records carry a SHA-256 checksum at envelope version 2.
* --database now refuses names that normalization would rewrite (e.g. ./memory); the error suggests the normalized candidate. The documented idiom and all scripts/tests migrated to clean identifiers.

### merge

* config integrity + signed installer (C2+C6) ([94d9c22](https://github.com/Arakiss/nahuali/commit/94d9c22228c2d43ef2791fadb5af1154f628c50f))
* flagship defaults true by default (D1+D2+D3+C3) ([3cb436a](https://github.com/Arakiss/nahuali/commit/3cb436a62b33f6094aa6e69d106535cad2f24784))


### Features

* **cli,mcp:** surface signal_count and distinct_flagged_record_count ([68c28df](https://github.com/Arakiss/nahuali/commit/68c28df23d265d741d311d9b25f28e65ddcd4d98))
* **config:** resolve database name once with flag&gt;env&gt;default precedence ([2afe8c1](https://github.com/Arakiss/nahuali/commit/2afe8c1b7e93fa5f28941b5fc8835a506e36ad6f))
* default to a fail-closed, SHA-256-checksummed, attested ledger ([118488f](https://github.com/Arakiss/nahuali/commit/118488fa936e9d1a83bc6d66c1816a77140f97f5))
* make memory trust visible on first run ([15fc455](https://github.com/Arakiss/nahuali/commit/15fc455cb3eeceeecdb6a3b6e9e17c9eb31495fa))

## [0.6.0-beta.0](https://github.com/Arakiss/nahuali/compare/nahuali-mcp-v0.5.0-beta.0...nahuali-mcp-v0.6.0-beta.0) (2026-07-06)


### Features

* add result-level trust to authority recall ([c8cd149](https://github.com/Arakiss/nahuali/commit/c8cd1499fe0117af3a920edb2ee95dcab0d52fd4))
* **core:** add a point-in-time and exclude-stale temporal recall filter ([88c5ab6](https://github.com/Arakiss/nahuali/commit/88c5ab606af9334540576f7ef7af6e5db52774f2))
* **core:** surface the ledger Merkle root in audit and trust-report integrity ([a4dd5ec](https://github.com/Arakiss/nahuali/commit/a4dd5ec42ae25fac81ddc29188908f34d5412dac))
* initial public beta ([4157a62](https://github.com/Arakiss/nahuali/commit/4157a62b1f4b3c6ff97f6dda61cada69990652c6))
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


### Bug fixes

* align transport tamper-evidence defaults ([50a0872](https://github.com/Arakiss/nahuali/commit/50a087250d2d970bfaf2a31639200d65ff9b3121))


### Refactor

* **mcp:** split protocol::views into per-family modules ([7f69412](https://github.com/Arakiss/nahuali/commit/7f694123273a54db351aa1b43b7808bd54aecc79))

## [0.5.0-beta.0](https://github.com/Arakiss/nahuali/compare/nahuali-mcp-v0.4.1-beta.0...nahuali-mcp-v0.5.0-beta.0) (2026-06-19)


### Features

* add result-level trust to authority recall ([c8cd149](https://github.com/Arakiss/nahuali/commit/c8cd1499fe0117af3a920edb2ee95dcab0d52fd4))
* **core:** add a point-in-time and exclude-stale temporal recall filter ([88c5ab6](https://github.com/Arakiss/nahuali/commit/88c5ab606af9334540576f7ef7af6e5db52774f2))
* **core:** surface the ledger Merkle root in audit and trust-report integrity ([a4dd5ec](https://github.com/Arakiss/nahuali/commit/a4dd5ec42ae25fac81ddc29188908f34d5412dac))
* initial public beta ([4157a62](https://github.com/Arakiss/nahuali/commit/4157a62b1f4b3c6ff97f6dda61cada69990652c6))
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


### Bug fixes

* align transport tamper-evidence defaults ([50a0872](https://github.com/Arakiss/nahuali/commit/50a087250d2d970bfaf2a31639200d65ff9b3121))


### Refactor

* **mcp:** split protocol::views into per-family modules ([7f69412](https://github.com/Arakiss/nahuali/commit/7f694123273a54db351aa1b43b7808bd54aecc79))

## [0.4.1-beta.0](https://github.com/Arakiss/nahuali/compare/nahuali-mcp-v0.4.0-beta.0...nahuali-mcp-v0.4.1-beta.0) (2026-06-14)


### Bug fixes

* align transport tamper-evidence defaults ([50a0872](https://github.com/Arakiss/nahuali/commit/50a087250d2d970bfaf2a31639200d65ff9b3121))

## [0.4.0-beta.0](https://github.com/Arakiss/nahuali/compare/nahuali-mcp-v0.3.0-beta.0...nahuali-mcp-v0.4.0-beta.0) (2026-06-13)


### Features

* add result-level trust to authority recall ([c8cd149](https://github.com/Arakiss/nahuali/commit/c8cd1499fe0117af3a920edb2ee95dcab0d52fd4))
* **core:** add a point-in-time and exclude-stale temporal recall filter ([88c5ab6](https://github.com/Arakiss/nahuali/commit/88c5ab606af9334540576f7ef7af6e5db52774f2))
* **core:** surface the ledger Merkle root in audit and trust-report integrity ([a4dd5ec](https://github.com/Arakiss/nahuali/commit/a4dd5ec42ae25fac81ddc29188908f34d5412dac))
* initial public beta ([4157a62](https://github.com/Arakiss/nahuali/commit/4157a62b1f4b3c6ff97f6dda61cada69990652c6))
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


### Refactor

* **mcp:** split protocol::views into per-family modules ([7f69412](https://github.com/Arakiss/nahuali/commit/7f694123273a54db351aa1b43b7808bd54aecc79))

## [0.3.0-beta.0](https://github.com/Arakiss/nahuali/compare/nahuali-mcp-v0.2.0-beta.0...nahuali-mcp-v0.3.0-beta.0) (2026-06-02)


### Features

* add result-level trust to authority recall ([c8cd149](https://github.com/Arakiss/nahuali/commit/c8cd1499fe0117af3a920edb2ee95dcab0d52fd4))
* initial public beta ([4157a62](https://github.com/Arakiss/nahuali/commit/4157a62b1f4b3c6ff97f6dda61cada69990652c6))
* **mcp:** guide agents with when/next tool descriptions, annotations, and argument docs ([27ce050](https://github.com/Arakiss/nahuali/commit/27ce0506abde38417f690701de397ec8e7f6b2ac))
* **mcp:** return typed output views for briefing, self-inspect, review, and graph ([3305cf1](https://github.com/Arakiss/nahuali/commit/3305cf1c720f782a159b3fa5a00a869e3e7e7d84))

## [0.2.0-beta.0](https://github.com/Arakiss/nahuali/compare/nahuali-mcp-v0.1.0-beta.0...nahuali-mcp-v0.2.0-beta.0) (2026-06-01)


### Features

* add result-level trust to authority recall ([c8cd149](https://github.com/Arakiss/nahuali/commit/c8cd1499fe0117af3a920edb2ee95dcab0d52fd4))
* initial public beta ([4157a62](https://github.com/Arakiss/nahuali/commit/4157a62b1f4b3c6ff97f6dda61cada69990652c6))
