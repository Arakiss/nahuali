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
| Is every ordinary memory write individually recorded and traceable? | Every ordinary write accepted through the shipped Nahuali paths appends a typed `EventEnvelope` with a version, id, sequence, timestamp, checksum, optional chain link, and payload. This does not account for an administrator editing the database outside Nahuali. | `crates/nahuali-core/src/event.rs:25-52`; `crates/nahuali-core/src/event.rs:199-223`; `crates/nahuali-core/src/store/services.rs:833-883` |
| Can you reconstruct what changed between two points in time? | A non-mutating audit emits a diff between retained ledger bounds and restates checksum, sequence, chain, and Merkle-root status through the upper bound. It cannot recreate original bytes that were removed from the only store; that requires a retained original or backup. | `crates/nahuali-core/src/audit.rs:129-184`; `crates/nahuali-core/src/audit.rs:195-310` |
| Can you prove a specific record was committed, to a third party? | A Merkle inclusion proof establishes that a record is included under a supplied root. Third-party reliance additionally requires an independently retained, authorized checkpoint for that root; the proof alone does not authenticate it. | `crates/nahuali-core/src/merkle.rs:123-182`; `crates/nahuali-core/src/checkpoint.rs:125-220`; `crates/nahuali-cli/src/commands/audit.rs:66-120` |
| Is per-decision recall evidence available? | Recall can require a concrete evidence identifier on every returned result, and supports point-in-time replay (`as_of_ms`). A composed trust report folds health, authority, and ledger integrity into one non-mutating verdict (see threat model). | `crates/nahuali-core/src/recall.rs:8-27`; `compliance/threat-model.md` |
| Does the audit trail prove content is true or immutable? | No. Local validation detects specified checksum, sequence, and chain failures. Last-event rewrite, rollback, and a full re-chain require comparison with an authorized external checkpoint. None of these controls attest to content truth. | `crates/nahuali-core/src/audit.rs:129-150`; `crates/nahuali-core/src/audit.rs:267-310`; `crates/nahuali-core/src/checkpoint.rs:398-544` |

## Policy enforcement points

| Question | Answer | Evidence |
|---|---|---|
| Where are trust/policy decisions enforced? | At recall time (trust verdicts warn or block unsupported/contradictory memory rather than silently certifying it), via `require_evidence` on recall, and via governed repair, which validates and gates explicit proposals before any event is appended. | `crates/nahuali-core/src/recall.rs:8-27`; `compliance/owasp-asi06.md` |
| Is unsupported or fabricated memory rejected? | Ingestion validates that cited source records exist, and direct-write paths reject nonexistent evidence IDs. An existing ID does not prove that its content is true, relevant, independent, or sufficient. Recall surfaces unsupported and contradictory memory instead of silently certifying it. | `compliance/owasp-asi06.md` |
| Do scopes enforce authorization? | No. `MemoryScope` is a retrieval and projection boundary, not an authorization boundary. It is not an enforcement point. | `crates/nahuali-api/README.md:18-20`; `compliance/threat-model.md` |
| Is autonomous mutation of memory possible? | Self-inspection, reflection, sleep, and consolidation plan or recommend work without writing; writes require explicit, gated commands. | `compliance/owasp-asi06.md`; `BETA.md:18-19`; `BETA.md:54-55` |

## Data residency and isolation

| Question | Answer | Evidence |
|---|---|---|
| Where does data live? | The default ledger uses embedded SurrealKV. Operators can instead configure a SurrealDB endpoint, and Qdrant is an optional derived semantic tier. Nahuali provides no vendor-hosted store. | `SECURITY.md:3-6`; `README.md:169-184` |
| Can it be hosted entirely within our region/boundary? | It can be configured with storage and optional semantic endpoints inside a chosen boundary. The operator must also verify backups, logs, network routes, and any external adapters. | `SECURITY.md:3-7` |
| What runtime data paths can cross the host boundary? | Nahuali does not provide a vendor-hosted memory service. The default ledger is embedded and the HTTP API binds to loopback; data can cross the host boundary when an operator configures remote SurrealDB or Qdrant endpoints. | `crates/nahuali-core/src/database.rs:161-174`; `crates/nahuali-core/src/semantic/types.rs:16-77`; `crates/nahuali-api/src/main.rs:13-16` |
| How are tenants/pilots isolated? | Nahuali has no tenant authorization layer. A dedicated `--database` per pilot is an operator convention that reduces co-residence; access control, credentials, and prevention of a caller selecting another database must be enforced outside Nahuali. Scopes do not provide isolation. | `compliance/pilot-data-policy.md`; `crates/nahuali-api/src/main.rs:13-16` |

## Prompt injection and memory poisoning

| Question | Answer | Evidence |
|---|---|---|
| How is memory poisoning (OWASP ASI06) handled? | Provenance fields on supported record kinds, recall-side trust verdicts, scope-filtered retrieval, replayable history, hash-chain validation, Merkle inclusion evidence, and checkpoints verified against an external operator policy. Full control mapping in the ASI06 document. | `compliance/owasp-asi06.md` |
| Is poisoned context automatically re-ingested? | No. Reports and planning are non-mutating; governed repair gates proposals before append. | `compliance/owasp-asi06.md` |
| Is there a content-safety classifier for malicious input? | No. There is no complete malicious-content classifier. Nahuali controls memory writes and trust signals; it does not sanitize every upstream document. This is a stated gap. | `compliance/owasp-asi06.md` |

## Key custody (Ed25519 checkpoints)

| Question | Answer | Evidence |
|---|---|---|
| What key material exists and who holds it? | The CLI accepts operator-held files containing hex-encoded Ed25519 seeds. The core decodes each seed as 32 bytes and derives the public key stored in the external version 2 policy; key generation and custody remain operator responsibilities rather than a built-in key service. | `crates/nahuali-cli/src/cli.rs:924-946`; `crates/nahuali-core/src/checkpoint.rs:695-716` |
| How is signing invoked and verified? | `checkpoint-policy-init` creates the external policy; `checkpoint-sign` signs origin, lineage, tree size, Merkle root, chain tip, and signer time; `checkpoint-verify` checks the checkpoint against that policy and the selected ledger. The older `attest-*` chain-tip receipt remains a compatibility path and requires an external keyring for trust-sensitive use. | `crates/nahuali-cli/src/cli.rs:915-999`; `crates/nahuali-core/src/checkpoint.rs:122-220` |
| What happens if a signing key is compromised? | An attacker with an active private key can create signatures accepted by a policy until that key is revoked or the policy is replaced. Threshold policies can require multiple distinct active keys. Checkpoint freshness still depends on an external monotonic reference: an authorized old checkpoint proves a past state, not that the live store is current. | `crates/nahuali-core/src/checkpoint.rs:182-220`; `compliance/threat-model.md` |
| Is data encrypted at rest? | Nahuali does not add an application-level encryption layer around the configured store. Treat every data directory and remote endpoint as sensitive and apply storage or platform encryption where required. | `SECURITY.md:3-6`; `compliance/threat-model.md` |

## Authentication posture (be direct)

| Question | Answer | Evidence |
|---|---|---|
| Does the HTTP API authenticate callers? | **No.** The beta HTTP API has no authentication, accounts, tenants, API keys, or role-based access. The router applies no auth middleware. | `crates/nahuali-api/README.md:18-20`; `crates/nahuali-api/src/lib.rs:75-122` |
| What is the default network exposure? | The API server binds to loopback (`127.0.0.1:7070`) by default. | `crates/nahuali-api/src/main.rs:13-16` |
| What is the recommended deployment pattern today? | Keep the API on its default loopback address and do not expose it directly to an untrusted network. If remote access is required, authentication must be supplied outside Nahuali. | `README.md:168-169`; `crates/nahuali-api/src/main.rs:13-16`; `crates/nahuali-api/README.md:18-20` |
| Is the MCP server networked? | No. The MCP server is a local stdio adapter (a child process speaking over stdio), not a network listener. | `crates/nahuali-mcp/src/main.rs:16-27` |
| Is authenticated API access on the roadmap? | No committed authentication feature is published on the current roadmap. Assume no auth today and put any remote access behind the operator's own authenticating gateway or mTLS. | `ROADMAP.md`; `crates/nahuali-api/README.md:18-20` |

## SDLC and supply chain

| Question | Answer | Evidence |
|---|---|---|
| Are release archives signed? | Yes, with Sigstore. The release job runs `cosign sign-blob` over each packaged `.tar.gz` archive and verifies it against the release workflow identity and GitHub Actions OIDC issuer. | `.github/workflows/release.yml:335-346` |
| What exactly is signed? | Each release archive has a detached `.sigstore.json` bundle and a SHA-256 checksum. The MCP OCI image is a separate distribution artifact; this archive-signing evidence does not claim that the image is cosign-signed. | `.github/workflows/release.yml:306-346`; `.github/workflows/publish-mcp.yml` |
| Is a bill of materials produced? | Yes. A CycloneDX SBOM is generated with `anchore/sbom-action` and attached to the published release. | `.github/workflows/sbom.yml:32-42` |
| Is the project scored for supply-chain posture? | Yes. OSSF Scorecard runs via `ossf/scorecard-action` and uploads SARIF results. | `.github/workflows/scorecard.yml:25-35` |
| What does the release gate enforce? | Formatting, clippy, tests, docs, and regression fixtures; source-install smoke tests for CLI and MCP; release dry-run packaging; license and crate-metadata checks; lockfile and duplicate-dependency inspection; secret/identity/large-file scans; and automation checks that reject direct publish/tag/GitHub-Release commands. | `SECURITY.md:34-44` |
| Do release binaries carry the trust posture by default? | Yes. Release binaries build the CLI with `attestation` and the MCP/API with `tamper-evidence`, so shipped binaries chain their writes. | `.github/workflows/release.yml:300-305`; `crates/nahuali-cli/Cargo.toml:19-34` |

## Incident response

| Question | Answer | Evidence |
|---|---|---|
| How do we detect an integrity incident? | A failed `validate` or broken chain in `audit` is an integrity signal. A `Block` verdict is a policy stop whose reasons must be inspected; it can result from knowledge-health issues rather than ledger corruption. These reports are non-mutating. | `crates/nahuali-cli/src/cli.rs:829-889`; `crates/nahuali-core/src/recall.rs:522-555`; `crates/nahuali-cli/src/commands/trust_report.rs:10-49` |
| How do we report a vulnerability? | Through a private GitHub security advisory. Do not include real personal data, credentials, or customer data in public issues. | `SECURITY.md:15-18` |
| How do we recover? | Select a backup whose integrity and freshness are acceptable under an external checkpoint or recovery policy, run `backup-validate`, restore into an empty database, and reconcile the derived tiers. Validation alone does not prove the backup is the newest or semantically correct copy. | `crates/nahuali-cli/src/cli.rs:1022-1067`; `crates/nahuali-cli/src/cli.rs:225-231` |

## Current security limits

Implemented controls include append-oriented audit records, provenance fields,
operator-held checkpoint keys, configurable local deployment, and signed release
archives with an SBOM. Important gaps remain: the beta HTTP API has no
authentication, there is no encryption at rest or complete content-safety
classifier, scopes are not an authorization boundary, dedicated stores are an
operator convention rather than tenant enforcement, and checkpoint freshness is
external. A security team should deploy the API on loopback or behind its own
authenticating gateway, apply storage-layer encryption where required, and read
`compliance/threat-model.md` and `compliance/owasp-asi06.md` alongside these
answers.

Last reviewed: 2026-07-17.
