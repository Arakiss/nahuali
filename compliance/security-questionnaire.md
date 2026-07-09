# Security Review Answer Pack

This is a pre-filled answer pack for an AI-vendor security review — the kind an
enterprise security team or a DeepInspect-style agent-security assessment runs
before approving a memory component. Answers are grounded in shipped code with
`file:line` citations and are deliberately honest about what is not yet built. It
is not a certification or a legal attestation.

Nahuali is a local-first memory engine for AI agents: an append-only, hash-chained
SurrealDB `memory_record` ledger with a derived, rebuildable semantic index. Two
companion documents carry the detail behind several answers and are cross-
referenced below: `compliance/threat-model.md` and `compliance/owasp-asi06.md`.

Primary sources:

- Repo security policy: `SECURITY.md`
- Threat model: `compliance/threat-model.md`
- OWASP ASI06 (memory and context poisoning) mapping: `compliance/owasp-asi06.md`
- OWASP Top 10 for Agentic Applications (2026):
  https://genai.owasp.org/resource/owasp-top-10-for-agentic-applications-for-2026/
- Sigstore / cosign keyless signing: https://docs.sigstore.dev/

## Auditability and traceability

| Question | Answer | Evidence |
|---|---|---|
| Is every memory write individually recorded and traceable? | Yes. Each write is an append-only typed `EventEnvelope` with a version, id, monotonic sequence, timestamp, checksum, optional chain link, and payload. Payload variants cover sources, episodes, facts, relations, procedures, intentions, reviews, and repairs. | `crates/nahuali-core/src/event.rs:13-39`; `crates/nahuali-core/src/event.rs:169-193` |
| Can you reconstruct what changed between two points in time? | Yes. A non-mutating audit emits a diff of what changed between two ledger points and restates checksum, sequence, chain, and Merkle-root integrity through the upper bound. | `crates/nahuali-core/src/audit.rs:122-160`; `crates/nahuali-core/src/audit.rs:101-120` |
| Can you prove a specific record was committed, to a third party? | Yes. Merkle roots and portable inclusion proofs are derivable over the chained ledger; the CLI `audit --inclusion-proof <SEQUENCE>` emits one under the audited root. | `crates/nahuali-core/src/merkle.rs:131-142`; `crates/nahuali-cli/src/cli.rs:824-828` |
| Is per-decision recall evidence available? | Recall can require a concrete evidence identifier on every returned result, and supports point-in-time replay (`as_of_ms`). A composed trust report folds health, authority, and ledger integrity into one non-mutating verdict (see threat model). | `crates/nahuali-core/src/recall.rs:8-27`; `compliance/threat-model.md` |
| Does the audit trail prove content is true? | No. The chain proves records were not altered after the fact and were committed in a given order. It does not attest to the truthfulness of the content. | `crates/nahuali-core/src/audit.rs:101-120` |

## Policy enforcement points

| Question | Answer | Evidence |
|---|---|---|
| Where are trust/policy decisions enforced? | At recall time (trust verdicts warn or block unsupported/contradictory memory rather than silently certifying it), via `require_evidence` on recall, and via governed repair, which validates and gates explicit proposals before any event is appended. | `crates/nahuali-core/src/recall.rs:8-27`; `compliance/owasp-asi06.md` |
| Is unsupported or fabricated memory rejected? | Ingestion validates source references and direct-write paths reject fabricated evidence references; recall surfaces unsupported/contradictory memory instead of certifying it. Detail and citations are in the ASI06 mapping. | `compliance/owasp-asi06.md` |
| Do scopes enforce authorization? | No. `MemoryScope` is a retrieval and projection boundary, not an authorization boundary. It is not an enforcement point. | `crates/nahuali-api/README.md:18-20`; `compliance/threat-model.md` |
| Is autonomous mutation of memory possible? | Self-inspection, reflection, sleep, and consolidation plan or recommend work without writing; writes require explicit, gated commands. | `compliance/owasp-asi06.md`; `BETA.md:45-46` |

## Data residency and isolation

| Question | Answer | Evidence |
|---|---|---|
| Where does data live? | In operator-controlled local services: an authoritative local SurrealDB `memory_record` ledger and a derived Qdrant semantic index. There is no vendor-hosted store. | `SECURITY.md:17-27`; `README.md:18` |
| Can it be hosted entirely within our region/boundary? | Yes. The whole stack runs against services the deployer controls, so residency is a deployment choice. | `SECURITY.md:17-27` |
| Does Nahuali send data anywhere (telemetry/analytics)? | No. The crates ship no telemetry, analytics, or phone-home code; the core never touches the network on its own. The only network endpoints are the operator's own configured services. | `crates/nahuali-core/src/attestation.rs:19-20` |
| How are tenants/pilots isolated? | By dedicated store per deployment (`--database`), not by scopes. See the pilot data policy. | `compliance/pilot-data-policy.md`; `crates/nahuali-api/src/main.rs:13-16` |

## Prompt injection and memory poisoning

| Question | Answer | Evidence |
|---|---|---|
| How is memory poisoning (OWASP ASI06) handled? | Provenance metadata on every write, recall-side trust verdicts, scope-filtered retrieval, replayable history, hash-chain validation, Merkle inclusion evidence, and detached attestation. Full control mapping in the ASI06 document. | `compliance/owasp-asi06.md` |
| Is poisoned context automatically re-ingested? | No. Reports and planning are non-mutating; governed repair gates proposals before append. | `compliance/owasp-asi06.md` |
| Is there a content-safety classifier for malicious input? | No. There is no complete malicious-content classifier. Nahuali controls memory writes and trust signals; it does not sanitize every upstream document. This is a stated gap. | `compliance/owasp-asi06.md` |

## Key custody (Ed25519 attestation)

| Question | Answer | Evidence |
|---|---|---|
| What key material exists and who holds it? | Ed25519 signing keys are operator-held, supplied as 32-byte hex seeds. The core never generates randomness and never touches the network. Signing keys, receipts, and keyrings live outside the memory store. | `crates/nahuali-core/src/attestation.rs:1-20`; `crates/nahuali-core/src/attestation.rs:29-58` |
| How is signing invoked and verified? | `attest-sign` signs the current chain tip from an operator `--key-file`; `attest-verify` checks a receipt against the live ledger and can authorize it against a trusted keyring that rejects revoked or unknown keys. | `crates/nahuali-cli/src/cli.rs:845-868` |
| What happens if a signing key is compromised? | A malicious receipt may verify until the operator rotates or revokes the keyring entry; keyrings model active and revoked keys. Attestation freshness is enforced externally (a valid old receipt proves a past checkpoint, not that the live store is current). | `compliance/threat-model.md` |
| Is data encrypted at rest? | No built-in encryption at rest. Treat local database and backup directories as sensitive and apply platform storage controls where required. | `SECURITY.md:3-5`; `compliance/threat-model.md` |

## Authentication posture (be direct)

| Question | Answer | Evidence |
|---|---|---|
| Does the HTTP API authenticate callers? | **No.** The beta HTTP API has no authentication, accounts, tenants, API keys, or role-based access. The router applies no auth middleware. | `crates/nahuali-api/README.md:18-20`; `crates/nahuali-api/src/lib.rs:66-109` |
| What is the default network exposure? | The API server binds to loopback (`127.0.0.1:7070`) by default. | `crates/nahuali-api/src/main.rs:13-16` |
| What is the safe deployment pattern today? | Keep the API on loopback or inside a trusted network segment, and place any remote access behind the operator's own authenticating gateway or mTLS. Do not expose the beta API directly to untrusted networks. | `crates/nahuali-api/README.md:18-20` |
| Is the MCP server networked? | No. The MCP server is a local stdio adapter (a child process speaking over stdio), not a network listener. | `crates/nahuali-mcp/src/main.rs:16-23` |
| Is authenticated API access on the roadmap? | Bearer-token auth and TLS/mTLS guidance are planned but not shipped. Assume no auth today. | `crates/nahuali-api/README.md:18-20` |

## SDLC and supply chain

| Question | Answer | Evidence |
|---|---|---|
| Are release artifacts signed? | Yes, with Sigstore. The release job runs `cosign sign-blob` over the packaged `.tar.gz` release archive and immediately verifies it with `cosign verify-blob`, pinning the certificate identity to the release workflow ref and the OIDC issuer to GitHub Actions (keyless signing). | `.github/workflows/release.yml:293-304`; `.github/workflows/release.yml:226` |
| What exactly is signed? | The release archive (a detached blob signature, emitted as a `.sigstore.json` bundle) plus a SHA-256 checksum of the archive. It signs the archive, not container images. | `.github/workflows/release.yml:288-292`; `.github/workflows/release.yml:305-312` |
| Is a bill of materials produced? | Yes. A CycloneDX SBOM is generated with `anchore/sbom-action` and attached to the published release. | `.github/workflows/sbom.yml:32-37` |
| Is the project scored for supply-chain posture? | Yes. OSSF Scorecard runs via `ossf/scorecard-action` and uploads SARIF results. | `.github/workflows/scorecard.yml:25-35` |
| What does the release gate enforce? | Formatting, clippy, tests, docs, and regression fixtures; source-install smoke tests for CLI and MCP; release dry-run packaging; license and crate-metadata checks; lockfile and duplicate-dependency inspection; secret/identity/large-file scans; and automation checks that reject direct publish/tag/GitHub-Release commands. | `SECURITY.md:29-46` |
| Do release binaries carry the trust posture by default? | Yes. Release binaries build the CLI with `attestation` and the MCP/API with `tamper-evidence`, so shipped binaries chain their writes. | `.github/workflows/release.yml:258-263` |

## Incident response

| Question | Answer | Evidence |
|---|---|---|
| How do we detect an integrity incident? | A failed `validate`, a broken chain in `audit`, or a Block verdict in `trust-report` signals an integrity incident; all three are non-mutating. | `crates/nahuali-cli/src/cli.rs:84-86` |
| How do we report a vulnerability? | Through a private GitHub security advisory. Do not include real personal data, credentials, or customer data in public issues. | `SECURITY.md:12-15` |
| How do we recover? | Restore from the most recent validated backup (`backup-validate` then `restore`) and `reconcile` the derived tiers from the authoritative ledger. | `crates/nahuali-cli/src/cli.rs:899-929`; `crates/nahuali-cli/src/cli.rs:73` |

## Honest Position

Nahuali's strongest answers are on auditability, tamper-evidence, provenance,
operator-held key custody, local data residency, and a signed, SBOM-backed
release pipeline. Its weakest, stated plainly, are: the beta HTTP API has no
authentication, there is no encryption at rest, there is no content-safety
classifier, scopes are not an authorization boundary, and attestation freshness
is enforced outside the store. A security team should deploy the API on loopback
or behind its own authenticating gateway, apply storage-layer encryption where
data classification requires it, and read `compliance/threat-model.md` and
`compliance/owasp-asi06.md` alongside these answers.

Last reviewed: 2026-07-10.
