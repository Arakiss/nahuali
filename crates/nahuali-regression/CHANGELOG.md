# Changelog

## [2.0.0-beta.0](https://github.com/Arakiss/nahuali/compare/nahuali-regression-v1.0.0-beta.0...nahuali-regression-v2.0.0-beta.0) (2026-07-14)


### ⚠ BREAKING CHANGES

* default builds now enable attestation and fail-closed validation; --require-chained becomes --allow-unchained (opt-out); new event envelopes are version 2 with SHA-256 checksums.
* the default validation verdict for an unchained ledger flips from accept to reject, the `--require-chained` flag is renamed to `--allow-unchained`, and new records carry a SHA-256 checksum at envelope version 2.

### merge

* flagship defaults true by default (D1+D2+D3+C3) ([3cb436a](https://github.com/Arakiss/nahuali/commit/3cb436a62b33f6094aa6e69d106535cad2f24784))


### Features

* **core:** surface recency-resolved supersession as a warn-level signal ([95d00e0](https://github.com/Arakiss/nahuali/commit/95d00e0ff8bf06fb66e089a82328c0bdbeabef2c))
* default to a fail-closed, SHA-256-checksummed, attested ledger ([118488f](https://github.com/Arakiss/nahuali/commit/118488fa936e9d1a83bc6d66c1816a77140f97f5))
* initial public beta ([4157a62](https://github.com/Arakiss/nahuali/commit/4157a62b1f4b3c6ff97f6dda61cada69990652c6))
* **regression:** add the Provenance Coverage Rate benchmark fixture ([750e631](https://github.com/Arakiss/nahuali/commit/750e63145a699e7e58ca2a8c6307183ed6d834b1))
* **regression:** emit a versioned ARP report under --arp ([6be8295](https://github.com/Arakiss/nahuali/commit/6be829579e56271d42a700cac7b067285ca8f12a))
* **regression:** emit a versioned LIVR integrity report under --livr ([178ef18](https://github.com/Arakiss/nahuali/commit/178ef1816ffaa97d4dda6d6ef79615ff1a52ac74))

## [1.0.0-beta.0](https://github.com/Arakiss/nahuali/compare/nahuali-regression-v0.3.0-beta.0...nahuali-regression-v1.0.0-beta.0) (2026-07-14)


### ⚠ BREAKING CHANGES

* default builds now enable attestation and fail-closed validation; --require-chained becomes --allow-unchained (opt-out); new event envelopes are version 2 with SHA-256 checksums.
* the default validation verdict for an unchained ledger flips from accept to reject, the `--require-chained` flag is renamed to `--allow-unchained`, and new records carry a SHA-256 checksum at envelope version 2.

### merge

* flagship defaults true by default (D1+D2+D3+C3) ([3cb436a](https://github.com/Arakiss/nahuali/commit/3cb436a62b33f6094aa6e69d106535cad2f24784))


### Features

* default to a fail-closed, SHA-256-checksummed, attested ledger ([118488f](https://github.com/Arakiss/nahuali/commit/118488fa936e9d1a83bc6d66c1816a77140f97f5))

## [0.3.0-beta.0](https://github.com/Arakiss/nahuali/compare/nahuali-regression-v0.2.0-beta.0...nahuali-regression-v0.3.0-beta.0) (2026-07-06)


### Features

* **core:** surface recency-resolved supersession as a warn-level signal ([95d00e0](https://github.com/Arakiss/nahuali/commit/95d00e0ff8bf06fb66e089a82328c0bdbeabef2c))
* initial public beta ([4157a62](https://github.com/Arakiss/nahuali/commit/4157a62b1f4b3c6ff97f6dda61cada69990652c6))
* **regression:** add the Provenance Coverage Rate benchmark fixture ([750e631](https://github.com/Arakiss/nahuali/commit/750e63145a699e7e58ca2a8c6307183ed6d834b1))
* **regression:** emit a versioned ARP report under --arp ([6be8295](https://github.com/Arakiss/nahuali/commit/6be829579e56271d42a700cac7b067285ca8f12a))
* **regression:** emit a versioned LIVR integrity report under --livr ([178ef18](https://github.com/Arakiss/nahuali/commit/178ef1816ffaa97d4dda6d6ef79615ff1a52ac74))

## [0.2.0-beta.0](https://github.com/Arakiss/nahuali/compare/nahuali-regression-v0.1.4-beta.0...nahuali-regression-v0.2.0-beta.0) (2026-06-19)


### Features

* **core:** surface recency-resolved supersession as a warn-level signal ([95d00e0](https://github.com/Arakiss/nahuali/commit/95d00e0ff8bf06fb66e089a82328c0bdbeabef2c))
* initial public beta ([4157a62](https://github.com/Arakiss/nahuali/commit/4157a62b1f4b3c6ff97f6dda61cada69990652c6))
* **regression:** add the Provenance Coverage Rate benchmark fixture ([750e631](https://github.com/Arakiss/nahuali/commit/750e63145a699e7e58ca2a8c6307183ed6d834b1))
* **regression:** emit a versioned ARP report under --arp ([6be8295](https://github.com/Arakiss/nahuali/commit/6be829579e56271d42a700cac7b067285ca8f12a))
* **regression:** emit a versioned LIVR integrity report under --livr ([178ef18](https://github.com/Arakiss/nahuali/commit/178ef1816ffaa97d4dda6d6ef79615ff1a52ac74))

## [0.1.4-beta.0](https://github.com/Arakiss/nahuali/compare/nahuali-regression-v0.1.3-beta.0...nahuali-regression-v0.1.4-beta.0) (2026-06-14)
