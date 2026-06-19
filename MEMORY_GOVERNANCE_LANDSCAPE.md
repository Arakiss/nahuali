# Agent-Memory Governance Landscape

Last external claim review: 2026-06-14.

Next scheduled review: 2026-09-14, or earlier before quoting the comparison
claim in a new public release, launch post, benchmark report, investor note, or
customer-facing document.

Nahuali's public claim is intentionally narrow:

> Nahuali combines a hash-chained, Merkle-proofed memory ledger, detached
> Ed25519 tip attestation, and a per-recall confidence-vs-provenance trust
> verdict over the memory it returns.

That is a composition claim, not a claim that Nahuali has the highest raw recall,
the largest ecosystem, or legal compliance certification. This document records
the adjacent prior art and external pressure behind that boundary so the README
can stay short without hiding the evidence.

## Claim Review Discipline

External comparison claims are time-bounded. Treat the README comparison as
stale if this document has not been reviewed within the last 90 days, or if a
credible new competitor appears to combine the same primitives before that date.

A claim review must update this document before the README is quoted
externally. The review should:

1. Re-check direct memory engines and adjacent prior art from primary sources:
   official repositories, documentation, release notes, papers, and standards or
   regulatory sources.
2. Inspect representative public code for the specific primitives Nahuali
   claims together: recall-path evidence/freshness verdicts, tamper-evident
   memory ledger, Merkle proofs, detached tip attestation, and reproducible
   governance benchmarks.
3. Separate open-source implementation evidence from vendor positioning,
   research prototypes, blog posts, and commercial claims without public code.
4. Weaken or remove the README claim if the evidence is ambiguous. A dated,
   narrow claim is better than an overconfident evergreen one.

This file is the canonical freshness marker for public comparison claims. The
README should summarize it, not duplicate the full competitor audit.

## Why This Matters

Persistent agent memory turns history into infrastructure. Once an agent writes
facts, summaries, preferences, task state, or decisions across sessions, callers
need more than a relevant retrieval result. They need to know:

1. what evidence supports the returned memory,
2. whether the memory is stale, contradictory, unsupported, or low-confidence,
3. whether the recorded history was rewritten after the fact, and
4. which reproducible checks prove those controls still work.

Recall-first memory benchmarks are still useful. They answer whether the right
context can be found. Governance benchmarks answer a different question: whether
the memory substrate exposes and verifies the basis for trust before a caller
acts on the retrieved context.

## External Pressure

### Memory Poisoning

OWASP's agentic security work makes memory poisoning a first-class concern:
[OWASP Agent Memory Guard](https://owasp.org/www-project-agent-memory-guard/)
directly targets ASI06, Memory Poisoning, and its roadmap targets a stable v1.0
in Q4 2026. Published memory-poisoning work reports that MINJA-style attacks can
poison long-term memory through normal query interactions, with reported
injection success above 95% and attack success above 70% in idealized settings
([Memory Poisoning Attack and Defense on Memory Based LLM-Agents](https://arxiv.org/html/2601.05504v2)).

Nahuali is not a complete poisoning-defense product. Its contribution is the
part a memory substrate can enforce deterministically: provenance-aware recall,
freshness-aware trust decisions, explicit health signals, non-mutating review
paths, and a ledger that makes rewritten history detectable.

### AI Act Record Keeping

Article 12 of the EU AI Act requires high-risk AI systems to support automatic
event logging over the lifetime of the system, with logs intended to support
traceability, post-market monitoring, risk identification, and operational
tracking
([AI Act Service Desk: Article 12](https://ai-act-service-desk.ec.europa.eu/en/ai-act/article-12)).
The Council and Parliament's 7 May 2026 provisional agreement on the Digital
Omnibus introduced fixed delayed application dates for high-risk rules: 2
December 2027 for stand-alone high-risk systems and 2 August 2028 for high-risk
AI embedded in products
([Council press release](https://www.consilium.europa.eu/en/press/press-releases/2026/05/07/artificial-intelligence-council-and-parliament-agree-to-simplify-and-streamline-rules/)).

Nahuali does not claim AI Act compliance. The relevant engineering point is
smaller: memory systems aimed at regulated or high-stakes use should be designed
around traceable, replayable, tamper-evident records rather than opaque mutable
state.

## Adjacent Prior Art

### SuperLocalMemory

[SuperLocalMemory](https://arxiv.org/abs/2603.02240) is the closest
memory-specific prior art found in this review. It describes a local-first
multi-agent memory system with SQLite/FTS5, graph clustering, per-agent
provenance, adaptive ranking, and Bayesian trust scoring against OWASP ASI06
memory poisoning.

Boundary relative to Nahuali: it scores writer/agent trust and poisoning risk,
but it does not expose Nahuali's per-recall evidence/freshness/health verdict
over a tamper-evident memory ledger.

### MentisDB

[MentisDB](https://docs.rs/mentisdb/latest/mentisdb/) is a Rust memory crate
with append-only, hash-chained semantic thoughts and hybrid retrieval. Its
project material also describes cryptographically signable versioned skill
uploads.

Boundary relative to Nahuali: it overlaps on durable hash-chained memory
records, but it is not positioned as a governance benchmark suite and does not
define a recall-path trust verdict contract.

### OpenFang

[OpenFang](https://github.com/RightNow-AI/openfang) is an agent operating system
with a Merkle hash-chain audit trail over agent actions, plus broader runtime
isolation and security controls.

Boundary relative to Nahuali: it is useful evidence that cryptographic
agent-action audit trails are becoming expected, but it audits actions rather
than a memory store and does not provide memory recall verdicts.

### Right To History

[Right to History](https://arxiv.org/abs/2602.20214) is a research prototype for
verifiable agent execution using RFC 6962-style Merkle audit logs and a Rust
sovereignty kernel.

Boundary relative to Nahuali: it is directly relevant to verifiable agent
history, but it is execution-history research, not an agent-memory engine with
provenance-aware recall and governance benchmarks.

### Trace Continuity

[Trace Continuity](https://dev.to/heath_99ab1667dfecd3da406/trace-continuity-vs-mem0-vs-zep-ai-memory-governance-compared-1mhp)
is vendor-published positioning around PII redaction, retention, audit logging,
and tenant isolation for regulated memory use cases.

Boundary relative to Nahuali: it is useful as market signal, not as independent
technical proof. Its public positioning validates demand for governance-first
memory, while Nahuali's current differentiator is cryptographic history plus
recall trust verdicts in a local OSS engine.

Other commercial pages now use overlapping language around signed or
tamper-evident agent-memory retrieval logs, for example
[CyborgDB's agent-memory audit-trail positioning](https://www.cyborg.co/solutions-use-cases/).
Treat those as market signals unless they provide public implementation
evidence for the full open-source memory-engine composition being claimed here.

The conclusion is that the building blocks are not unique. Hash chains, Merkle
proofs, signatures, provenance, and trust scoring are established ideas.
Nahuali's differentiator is the composition: recall trust verdicts grounded in
evidence, freshness, and health signals, over a tamper-evident memory ledger
with attestation, measured by reproducible governance benchmarks.

## Benchmark Gap

Existing agent-memory benchmark work is dominated by recall and long-context
quality:

- [LOCOMO](https://snap-research.github.io/locomo/) evaluates long-term
  conversational memory through question answering, event summarization, and
  multimodal dialogue generation.
- [LongMemEval and BEAM](https://github.com/mem0ai/memory-benchmarks) are used
  in memory-augmented LLM benchmark suites to measure long-term recall,
  extraction, temporal reasoning, multi-session reasoning, contradiction
  resolution, and related answer quality.

Those are important tests, but they do not measure whether a memory engine:

- detects rewritten ledger history,
- distinguishes sourced from unsupported recall,
- reports stale or contradictory knowledge,
- verifies signature-key lifecycle behavior,
- calibrates an authority verdict over a memory store, or
- publishes the fixed corpus and formula needed to recompute those governance
  numbers from a checkout.

Nahuali's [Governance Benchmark Methodology](GOVERNANCE_BENCHMARKS.md) covers
that second axis. It is first-party and synthetic by design, so it should be
quoted with the command, commit or release, and report JSON. It should not be
quoted as independent certification.

## How To Compare Nahuali Responsibly

Use Nahuali when the evaluation question is:

> Can this memory substrate explain why a returned memory should be trusted, and
> can it prove its recorded history was not silently rewritten?

Use a recall-first engine when the evaluation question is:

> Can this system maximize answer quality, ecosystem reach, or benchmark scores
> on LOCOMO, LongMemEval, BEAM, or application-specific retrieval tasks?

The two axes are complementary. A production system can pair high-recall memory
retrieval with governance checks. Nahuali's current OSS repository focuses on
the local deterministic governance foundation, not on replacing every
recall-first product surface.

When citing the landscape, keep these limits attached:

- Nahuali is pre-release.
- The governance benchmarks are first-party release gates.
- The project does not claim legal compliance.
- The project does not claim memory contents are true.
- The public API can still change before 1.0.
- Public claims should be tied to a commit, release, fixture, or validation
  command.
