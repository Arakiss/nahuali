# nahuali-core

`nahuali-core` is the canonical self-inspecting memory engine for Nahuali.

It owns the append-only record ledger, deterministic projection, SurrealDB graph
projection, lexical recall, Qdrant-backed hybrid recall, and knowledge-health
inspection used by the first-party CLI, MCP server, and local HTTP API.
Persistence is backed by the authoritative SurrealDB `memory_record` ledger;
graph projection tables and semantic vectors are derived tiers. The public
storage boundary is summarized in the root README while design notes remain
private during pre-release development.

## Contract

- Databases are append-only SurrealDB record ledgers stored in the
  `memory_record` table.
- Opening a database validates event sequence order and checksums before
  projection.
- Record ledgers can be validated non-destructively before projection.
- State is rebuilt by deterministic projection, not by trusting a mutable
  snapshot.
- Optional snapshots can be written and validated against a fresh record-ledger
  replay, but they are never authoritative.
- Local backups preserve authoritative event envelopes exactly and restore only
  into an empty SurrealDB database.
- Backup drills validate a backup and dry-run restore against a target database
  without writing records.
- Source records preserve provenance for documents, transcripts, conversations,
  and adapter-produced source material.
- Ingestion documents validate source, episode, and explicit derived records
  before anything is appended; dry runs never mutate the ledger.
- Source-neutral interchange documents can be exported and imported
  append-only; they are not record ledgers or snapshots. Interchange and
  ingestion imports apply as a single batched ledger flush, so loading a large
  history stays fast.
- Default builds enable `attestation`, which implies the `tamper-evidence` hash
  chain. Each event binds the previous event's hash, so an in-place rewrite of
  any historical record breaks the chain at the next record even when the
  per-record checksum was recomputed. `--no-default-features` is the explicit
  legacy opt-out for an unchained build.
- The default attestation surface signs the chain tip with a detached Ed25519 signature.
  A full re-chain of history repairs every internal link but changes the tip, so
  a previously signed receipt no longer verifies and forging a new one needs the
  private key. Keys are operator-supplied seeds; the core never generates
  randomness or touches the network.
- Sources, entities, episodes, claims, links, procedures, intentions, health
  signals, and review/audit state are projected in Rust from the same validated
  ledger and materialized into rebuildable SurrealDB graph tables.
- The graph projection can be inspected, rebuilt, and validated without making
  it authoritative memory.
- Optional scopes label personal, project, organization, or custom context
  boundaries. Scoped projection keeps entities separate from unscoped entities
  with the same display name, and scoped recall is an exact filter.
- Facts and relations remain compatibility names for claims and links. New
  public code should prefer claims and links.
- Claims, links, procedures, and intentions can cite source episodes as
  evidence.
- Recall returns scored candidates with matched terms and evidence IDs when
  available.
- Graph traversal returns deterministic neighborhoods around entities or memory
  items with nodes, edges, depth, evidence IDs, authority, and health/review
  overlays.
- Semantic indexing rebuilds Qdrant from the current projection without making
  vectors authoritative.
- Hybrid recall preserves lexical, semantic, evidence, and authority score
  components for explainability.
- Briefing reports provide compact pre-work continuity without mutating memory.
- Inspection reports support gaps, low confidence, contradictions, stale facts,
  and isolated entities.
- Self-inspection reports are non-mutating consolidation passes with findings,
  review items, and an explicit no-automatic-write-back policy.
- Reflection reports group self-inspection findings into prioritized
  operator-approved cycles with source/evidence coverage.
- Sleep Mode reports replay recent episodes, inspect health, and propose
  evidence-backed consolidation candidates without writing memory.
- Consolidation-plan reports turn rest and review signals into replay,
  extraction, reconciliation, review-gate, and commit-eligibility operations
  without writing memory.
- Operator review reports prioritize self-inspection work for humans, scripts,
  and agents without writing memory automatically.
- Review resolutions are explicit operator-approved audit events. They can mark
  reviewed evidence as resolved while preserving the underlying record history.
- `MemoryEngine` is the canonical Rust entry point. `LocalMemory`, `Fact`, and
  `Relation` remain compatibility aliases for the pre-release foundation.

## Minimal Usage

```rust
use nahuali_core::{MemoryEngine, MemoryScope, MemoryScopeKind};

fn main() -> nahuali_core::Result<()> {
    let mut memory = MemoryEngine::open("memory")?;
    let source = memory.record_source(
        nahuali_core::SourceKind::Conversation,
        Some("Release review".to_string()),
        Some("fixture://release-review".to_string()),
        "fnv1a64:example",
        33,
        Default::default(),
    )?;
    let episode = memory.remember(
        "Lena owns the release notes.",
        vec!["product".to_string()],
    )?;
    let project_scope = MemoryScope::new(MemoryScopeKind::Project, "Nahuali")?;
    let scoped_episode = memory.remember_with_mentions_scoped(
        "Release notes belong to the Nahuali project.",
        vec!["product".to_string()],
        vec!["Release Notes".to_string()],
        project_scope.clone(),
    )?;
    let sourced_episode = memory.remember_source_episode(
        "Release notes should stay concise.",
        vec!["product".to_string()],
        vec!["Release Notes".to_string()],
        source.id,
        Some(1),
        Some("user".to_string()),
    )?;

    memory.add_claim(
        "Lena",
        "owns",
        "release notes",
        Some(episode.id.clone()),
        0.92,
    )?;
    memory.add_preference(
        "Release notes",
        "Keep release notes concise.",
        Some(sourced_episode.id),
        0.9,
    )?;

    let results = memory.recall("Lena release", 10)?;
    let scoped_results = memory.recall_scoped("release notes", 10, &project_scope)?;
    let briefing = memory.briefing();
    let graph = memory.graph_neighborhood("Lena", 2, 20)?;
    let health = memory.inspect();
    let self_inspection = memory.self_inspect();
    let reflection = memory.reflect();
    let sleep = memory.sleep();
    let plan = memory.consolidation_plan();
    let review = memory.operator_review(5);
    if let Some(item) = review.items.first() {
        let _plan = memory.review_resolution_plan(
            item.id.clone(),
            "Operator reviewed this item.",
        )?;
    }

    assert!(!results.is_empty());
    assert_eq!(
        scoped_results[0].evidence_id.as_deref(),
        Some(scoped_episode.id.as_str())
    );
    assert_eq!(briefing.summary.active_intention_count, 0);
    assert!(!graph.nodes.is_empty());
    assert_eq!(memory.data().sources.len(), 1);
    assert_eq!(health.unsupported_fact_count, 0);
    assert!(!self_inspection.write_back_policy.automatic_write_back);
    assert!(!reflection.write_back_policy.automatic_write_back);
    assert!(!sleep.write_back_policy.automatic_write_back);
    assert!(!plan.write_back_policy.automatic_write_back);
    assert!(!review.write_back_policy.automatic_write_back);

    Ok(())
}
```

## Current Limits

- The built-in embedding provider is deterministic and local. An optional
  `local-embeddings` build feature adds a static model2vec model, loaded from a
  local directory, for stronger semantic recall; both providers stay fully local
  and offline. Hosted embedding providers require an external adapter above this
  crate.
- The SurrealDB record ledger is append-only. Destructive compaction is not
  supported until a future versioned format can prove replay equivalence.
- Qdrant vectors are derived state. Rebuild them after restore or migration
  instead of treating vector snapshots as authoritative memory.
- Default semantic operations scope derived collections to the selected
  database. Explicit semantic configs can still target an exact collection for
  controlled tests or advanced operators.
- Shared server deployments belong outside this crate and should remain thin
  layers over the same record-ledger contract.
- The crate is pre-1.0. Public APIs are documented and tested, but semver
  stability is still intentionally conservative until the OSS release candidate
  hardens further.
