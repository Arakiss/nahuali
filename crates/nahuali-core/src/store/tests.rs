#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::{Arc, Barrier, mpsc},
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    use crate::{
        AuthorityMode, AutonomyLevel, IntentionKind, IntentionPriority, IntentionStatus,
        IntentionUpdateOptions, NahualiError, RepairClaim, RepairKind, RepairLink, RepairPayload,
        RepairProposal, SourceKind, SourceRecordOptions,
        event::{EpisodeRecorded, EventEnvelope, MemoryEvent},
        model::{MemoryScope, MemoryScopeKind, ProcedureKind},
        projection,
    };

    use super::{
        DatabaseSession, MemoryEngine, acquire_graph_projection_rebuild_lock, block_on_database,
        clear_graph_projection, completed_concurrent_rebuild, create_single_projected_record,
        ensure_graph_projection_rebuild_postcondition, inject_graph_projection_failure_once,
        open_database, query_graph_projection_mutation, read_records, rebuild_graph_projection,
        rebuild_graph_projection_locked, release_graph_projection_rebuild_lock,
        verify_graph_projection_lease, write_record, write_records,
    };

    #[test]
    fn concurrent_store_opens_serialize_schema_initialization() {
        const SESSION_COUNT: usize = 12;
        let barrier = Arc::new(Barrier::new(SESSION_COUNT));
        let handles = (0..SESSION_COUNT)
            .map(|index| {
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    let path = temp_path(&format!("concurrent_schema_open_{index}"));
                    barrier.wait();
                    let result = MemoryEngine::open(&path).and_then(|mut memory| {
                        memory.remember(format!("Concurrent memory {index}"), Vec::new())?;
                        Ok(memory.events().len())
                    });
                    (path, result)
                })
            })
            .collect::<Vec<_>>();

        for handle in handles {
            let (path, result) = handle.join().expect("store opener thread completes");
            assert_eq!(result.expect("concurrent store opens and writes cleanly"), 1);
            let _ = fs::remove_file(path);
        }
    }

    fn consolidate(
        subject: &str,
        predicate: &str,
        object: &str,
        evidence: &[&str],
    ) -> RepairProposal {
        RepairProposal {
            payload: RepairPayload::ConsolidateClaim(RepairClaim {
                subject: subject.to_string(),
                predicate: predicate.to_string(),
                object: object.to_string(),
                confidence: 0.9,
                scope: None,
            }),
            evidence_episode_ids: evidence.iter().map(|id| id.to_string()).collect(),
            proposed_by: "claude-opus-4-8".to_string(),
            rationale: "repeated observations".to_string(),
        }
    }

    #[test]
    fn stores_and_recalls_episode_from_record_ledger() {
        let path = temp_path("stores_and_recalls_episode_from_record_ledger");
        let _ = fs::remove_file(&path);

        let mut memory = MemoryEngine::open(&path).unwrap();
        memory
            .remember(
                "Lena prefers concise release notes.",
                vec!["example".to_string()],
            )
            .unwrap();

        let reopened = MemoryEngine::open(&path).unwrap();
        let results = reopened.recall("release notes", 10).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(reopened.events().len(), 1);
        assert_eq!(reopened.inspect().episode_count, 1);
        assert_eq!(reopened.data().event_count, 1);
        assert_eq!(reopened.authority().mode, AuthorityMode::Certify);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn refresh_skips_unchanged_ledger_and_replays_a_changed_tip() {
        let path = temp_path("refresh_changed_tip");
        let _ = fs::remove_file(&path);
        let mut cached = MemoryEngine::open(&path).unwrap();

        let empty = cached.refresh_if_changed().unwrap();
        assert!(!empty.changed);
        assert_eq!(empty.replayed_event_count, 0);
        assert_eq!(empty.observed_sequence, None);

        cached.remember("First episode", Vec::new()).unwrap();
        let unchanged = cached.refresh_if_changed().unwrap();
        assert!(!unchanged.changed);
        assert_eq!(unchanged.replayed_event_count, 0);
        assert_eq!(unchanged.previous_sequence, Some(1));
        assert_eq!(unchanged.observed_sequence, Some(1));

        let mut writer = MemoryEngine::open(&path).unwrap();
        writer.remember("Second episode", Vec::new()).unwrap();
        let changed = cached.refresh_if_changed().unwrap();
        assert!(changed.changed);
        assert_eq!(changed.previous_sequence, Some(1));
        assert_eq!(changed.observed_sequence, Some(2));
        assert_eq!(changed.replayed_event_count, 2);
        assert_eq!(cached.events().len(), 2);

        let stable_again = cached.refresh_if_changed().unwrap();
        assert!(!stable_again.changed);
        assert_eq!(stable_again.replayed_event_count, 0);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_fabricated_evidence_citations_on_the_direct_write_path() {
        let path = temp_path("rejects_fabricated_evidence_citations_on_the_direct_write_path");
        let _ = fs::remove_file(&path);

        let mut memory = MemoryEngine::open(&path).unwrap();
        let episode = memory
            .remember("Lena owns release notes.", vec!["product".to_string()])
            .unwrap();

        let ghost = Some("episode_never_recorded".to_string());
        let rejected = [
            memory
                .add_claim("Lena", "owns", "release notes", ghost.clone(), 0.9)
                .err()
                .map(|error| error.to_string()),
            memory
                .add_link("Lena", "owns", "Release Notes", ghost.clone(), 0.9)
                .err()
                .map(|error| error.to_string()),
            memory
                .add_procedure("release", "Write the notes.", ghost.clone(), 0.9)
                .err()
                .map(|error| error.to_string()),
            memory
                .add_intention(
                    "Ship the notes",
                    IntentionKind::Task,
                    IntentionPriority::Medium,
                    ghost,
                )
                .err()
                .map(|error| error.to_string()),
        ];
        for error in rejected {
            assert_eq!(
                error.as_deref(),
                Some("unknown source episode: episode_never_recorded")
            );
        }

        // A real citation still writes, and none of the rejects landed.
        memory
            .add_claim("Lena", "owns", "release notes", Some(episode.id), 0.9)
            .unwrap();
        assert_eq!(memory.data().claims.len(), 1);
        assert!(memory.data().links.is_empty());
        assert!(memory.data().procedures.is_empty());
        assert!(memory.data().intentions.is_empty());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn projects_facts_and_relations_from_events() {
        let path = temp_path("projects_facts_and_relations_from_events");
        let _ = fs::remove_file(&path);

        let mut memory = MemoryEngine::open(&path).unwrap();
        let episode = memory
            .remember("Lena owns release notes.", vec!["product".to_string()])
            .unwrap();
        memory
            .add_fact(
                "Lena",
                "owns",
                "release notes",
                Some(episode.id.clone()),
                0.9,
            )
            .unwrap();
        memory
            .relate("Lena", "owns", "release notes", Some(episode.id), 0.9)
            .unwrap();

        let reopened = MemoryEngine::open(&path).unwrap();

        assert_eq!(reopened.events().len(), 3);
        assert_eq!(reopened.data().claims.len(), 1);
        assert_eq!(reopened.data().links.len(), 1);
        assert_eq!(reopened.data().facts.len(), 1);
        assert_eq!(reopened.data().relations.len(), 1);
        assert_eq!(reopened.data().claims, reopened.data().facts);
        assert_eq!(reopened.data().links, reopened.data().relations);
        assert_eq!(reopened.data().entities.len(), 2);
        assert_eq!(
            reopened.data().last_event_id,
            Some(reopened.events()[2].id.clone())
        );

        let results = reopened.recall("owns release", 10).unwrap();
        assert!(results.iter().any(|result| result.id.starts_with("fact_")));
        assert!(
            results
                .iter()
                .any(|result| result.kind == crate::MemoryKind::Claim)
        );
        assert!(
            results
                .iter()
                .any(|result| result.id.starts_with("relation_"))
        );
        assert!(
            results
                .iter()
                .any(|result| result.kind == crate::MemoryKind::Link)
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn projects_episode_mentions_claims_links_procedures_and_intentions() {
        let path = temp_path("projects_expanded_memory_families");
        let _ = fs::remove_file(&path);

        let mut memory = MemoryEngine::open(&path).unwrap();
        let episode = memory
            .remember_with_mentions(
                "Lena wants the release notes kept concise.",
                vec!["product".to_string()],
                vec![" Lena ".to_string(), "Release Notes".to_string()],
            )
            .unwrap();
        let claim = memory
            .add_claim(
                "Lena",
                "owns",
                "release notes",
                Some(episode.id.clone()),
                2.0,
            )
            .unwrap();
        let link = memory
            .add_link(
                "Lena",
                "owns",
                "Release Notes",
                Some(episode.id.clone()),
                0.9,
            )
            .unwrap();
        let preference = memory
            .add_preference(
                "Release notes",
                "Keep release notes concise.",
                Some(episode.id.clone()),
                0.95,
            )
            .unwrap();
        let intention = memory
            .add_intention(
                "Ship the release notes",
                IntentionKind::Task,
                IntentionPriority::High,
                Some(episode.id),
            )
            .unwrap();
        let completed = memory
            .set_intention_status(
                intention.id.clone(),
                IntentionStatus::Completed,
                Some("Released".to_string()),
            )
            .unwrap();

        let reopened = MemoryEngine::open(&path).unwrap();

        assert_eq!(reopened.events().len(), 6);
        assert_eq!(reopened.data().entities.len(), 2);
        assert_eq!(reopened.data().claims[0].id, claim.id);
        assert_eq!(reopened.data().claims[0].confidence, 1.0);
        assert_eq!(reopened.data().links[0].id, link.id);
        assert_eq!(reopened.data().procedures[0].id, preference.id);
        assert_eq!(
            reopened.data().procedures[0].kind,
            ProcedureKind::Preference
        );
        assert_eq!(reopened.data().intentions[0].id, intention.id);
        assert_eq!(
            reopened.data().intentions[0].status,
            IntentionStatus::Completed
        );
        assert_eq!(
            reopened.data().intentions[0].status_reason.as_deref(),
            Some("Released")
        );
        assert_eq!(completed.status, IntentionStatus::Completed);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn append_path_keeps_incremental_projection_equivalent_to_full_replay() {
        let path = temp_path("append_path_keeps_incremental_projection_equivalent_to_full_replay");
        let _ = fs::remove_file(&path);

        let mut memory = MemoryEngine::open(&path).unwrap();

        let episode = memory
            .remember_with_mentions(
                "Lena wants the release notes kept concise.",
                vec!["product".to_string()],
                vec!["Lena".to_string(), "Release Notes".to_string()],
            )
            .unwrap();
        assert_eq!(memory.data(), &projection::project(memory.events()));

        memory
            .add_claim(
                "Lena",
                "owns",
                "release notes",
                Some(episode.id.clone()),
                0.9,
            )
            .unwrap();
        assert_eq!(memory.data(), &projection::project(memory.events()));

        memory
            .add_link(
                "Lena",
                "owns",
                "Release Notes",
                Some(episode.id.clone()),
                0.9,
            )
            .unwrap();
        assert_eq!(memory.data(), &projection::project(memory.events()));

        let intention = memory
            .add_intention(
                "Ship the release notes",
                IntentionKind::Task,
                IntentionPriority::High,
                Some(episode.id),
            )
            .unwrap();
        assert_eq!(memory.data(), &projection::project(memory.events()));

        memory
            .update_intention(
                intention.id.clone(),
                IntentionUpdateOptions {
                    description: Some("Ship the public release notes".to_string()),
                    progress_percent: Some(Some(50)),
                    ..IntentionUpdateOptions::default()
                },
            )
            .unwrap();
        assert_eq!(memory.data(), &projection::project(memory.events()));

        memory
            .set_intention_status(
                intention.id,
                IntentionStatus::Completed,
                Some("Released".to_string()),
            )
            .unwrap();
        assert_eq!(memory.data(), &projection::project(memory.events()));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn materializes_surrealdb_graph_projection_from_record_ledger() {
        let path = temp_path("materializes_surrealdb_graph_projection");
        let _ = fs::remove_file(&path);

        let mut memory = MemoryEngine::open(&path).unwrap();
        let episode = memory
            .remember_with_mentions(
                "Lena owns release notes.",
                vec!["product".to_string()],
                vec!["Lena".to_string(), "Release Notes".to_string()],
            )
            .unwrap();
        memory
            .add_claim(
                "Lena",
                "owns",
                "release notes",
                Some(episode.id.clone()),
                0.9,
            )
            .unwrap();
        memory
            .add_link(
                "Lena",
                "owns",
                "Release Notes",
                Some(episode.id.clone()),
                0.9,
            )
            .unwrap();
        let dependency = memory
            .add_intention(
                "Prepare launch checklist",
                IntentionKind::Task,
                IntentionPriority::Medium,
                None,
            )
            .unwrap();
        let intention = memory
            .add_intention(
                "Ship release notes",
                IntentionKind::Task,
                IntentionPriority::High,
                Some(episode.id),
            )
            .unwrap();
        memory
            .update_intention(
                intention.id.clone(),
                IntentionUpdateOptions {
                    deadline_at_ms: Some(Some(1234)),
                    depends_on: Some(vec![dependency.id.clone()]),
                    progress_percent: Some(Some(10)),
                    ..IntentionUpdateOptions::default()
                },
            )
            .unwrap();

        let validation = memory.projection_validate().unwrap();
        assert!(validation.valid, "{:?}", validation.issues);
        assert!(validation.status.in_sync);
        assert_eq!(
            validation.status.checkpoint_sequence,
            validation.status.latest_sequence
        );
        assert_eq!(validation.status.table_counts["episode"], 1);
        assert_eq!(validation.status.table_counts["entity"], 2);
        assert_eq!(validation.status.table_counts["claim"], 1);
        assert_eq!(validation.status.table_counts["intention"], 2);
        assert_eq!(validation.status.table_counts["mentions"], 2);
        assert_eq!(validation.status.table_counts["supports"], 2);
        assert_eq!(validation.status.table_counts["relates_to"], 1);
        assert_eq!(validation.status.table_counts["intention_depends_on"], 1);
        assert!(validation.status.table_counts["anomaly_alert"] >= 1);

        let reopened = MemoryEngine::open(&path).unwrap();
        let reopened_validation = reopened.projection_validate().unwrap();
        assert!(reopened_validation.valid, "{:?}", reopened_validation.issues);
        assert_eq!(reopened_validation.status.table_counts["episode"], 1);
        assert_eq!(reopened_validation.status.table_counts["relates_to"], 1);
        assert!(reopened_validation.status.table_counts["anomaly_alert"] >= 1);

        let entities = reopened.projection_entities(Some("lena"), 10).unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, "Lena");

        let timeline = reopened.projection_timeline(10).unwrap();
        assert_eq!(timeline.len(), 1);
        assert_eq!(timeline[0].content, "Lena owns release notes.");

        let pending = reopened.projection_pending(10).unwrap();
        assert_eq!(pending.len(), 2);
        let projected_intention = pending
            .iter()
            .find(|pending| pending.memory_id == intention.id)
            .expect("updated intention is pending");
        assert_eq!(projected_intention.deadline_at_ms, Some(1234));
        assert_eq!(projected_intention.depends_on, vec![dependency.id]);
        assert_eq!(projected_intention.progress_percent, Some(10));

        let health = reopened.projection_health_signals(10).unwrap();
        assert!(health.is_empty());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn rebuilds_1500_event_projection_in_fenced_batches() {
        const EVENT_COUNT: u64 = 1_500;
        const ENTITY_COUNT: u64 = 357;

        let path = temp_path("projection_scale_1500_events");
        let _ = fs::remove_file(&path);
        drop(MemoryEngine::open(&path).unwrap());

        let events = (1..=EVENT_COUNT)
            .map(|sequence| {
                let entity = format!("Entity {}", sequence % ENTITY_COUNT);
                EventEnvelope::new(
                    sequence,
                    sequence,
                    MemoryEvent::EpisodeRecorded(EpisodeRecorded {
                        id: format!("episode_scale_{sequence}"),
                        content: format!("Representative memory {sequence} mentions {entity}."),
                        tags: vec!["scale".to_string()],
                        mentions: vec![entity],
                        source_id: None,
                        source_position: None,
                        source_role: None,
                        scope: None,
                    }),
                )
            })
            .collect::<Vec<_>>();
        let write_path = path.clone();
        block_on_database(async move { write_records(&write_path, &events).await }).unwrap();

        let mut memory = MemoryEngine::open(&path).unwrap();
        let started_at = Instant::now();
        let report = memory.projection_rebuild().unwrap();
        let elapsed = started_at.elapsed();

        assert!(report.status.in_sync);
        assert_eq!(report.status.ledger_event_count, EVENT_COUNT as usize);
        assert_eq!(report.status.table_counts["episode"], EVENT_COUNT as usize);
        assert_eq!(report.status.table_counts["entity"], ENTITY_COUNT as usize);
        assert_eq!(report.status.table_counts["mentions"], EVENT_COUNT as usize);
        assert!(
            report.node_rows_written > super::GRAPH_PROJECTION_MUTATION_BATCH_SIZE,
            "scale fixture must cross at least one mutation batch"
        );
        assert!(
            report.relation_rows_written > super::GRAPH_PROJECTION_MUTATION_BATCH_SIZE,
            "scale fixture must cross at least one relation batch"
        );
        eprintln!(
            "projection-scale: events={EVENT_COUNT} nodes={} relations={} elapsed_ms={}",
            report.node_rows_written,
            report.relation_rows_written,
            elapsed.as_millis()
        );

        let validation = memory.projection_validate().unwrap();
        assert!(validation.valid, "{:?}", validation.issues);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn synchronizes_only_changed_rows_after_a_large_projection() {
        const BASE_EVENT_COUNT: u64 = 300;

        let path = temp_path("projection_incremental_sync");
        let _ = fs::remove_file(&path);
        drop(MemoryEngine::open(&path).unwrap());

        let events = (1..=BASE_EVENT_COUNT)
            .map(|sequence| {
                EventEnvelope::new(
                    sequence,
                    sequence,
                    MemoryEvent::EpisodeRecorded(EpisodeRecorded {
                        id: format!("episode_incremental_{sequence}"),
                        content: format!("Incremental projection memory {sequence}."),
                        tags: vec!["incremental".to_string()],
                        mentions: vec![format!("Entity {sequence}")],
                        source_id: None,
                        source_position: None,
                        source_role: None,
                        scope: None,
                    }),
                )
            })
            .collect::<Vec<_>>();
        let write_path = path.clone();
        block_on_database(async move { write_records(&write_path, &events).await }).unwrap();

        let mut memory = MemoryEngine::open(&path).unwrap();
        let initial = memory.projection_rebuild().unwrap();
        assert!(initial.status.in_sync);
        assert!(initial.node_rows_written > BASE_EVENT_COUNT as usize);
        assert_eq!(
            initial.relation_rows_written,
            BASE_EVENT_COUNT as usize
        );

        let next_sequence = BASE_EVENT_COUNT + 1;
        let event = EventEnvelope::new(
            next_sequence,
            next_sequence,
            MemoryEvent::EpisodeRecorded(EpisodeRecorded {
                id: format!("episode_incremental_{next_sequence}"),
                content: "Only this memory should be materialized.".to_string(),
                tags: vec!["incremental".to_string()],
                mentions: vec![format!("Entity {next_sequence}")],
                source_id: None,
                source_position: None,
                source_role: None,
                scope: None,
            }),
        );
        let write_path = path.clone();
        block_on_database(async move { write_record(&write_path, &event).await }).unwrap();

        let rebuild_path = path.clone();
        let report =
            block_on_database(async move { super::rebuild_graph_projection(&rebuild_path).await })
                .unwrap();
        assert!(report.status.in_sync);
        assert!(
            report.node_rows_written < 16,
            "incremental synchronization rewrote {} node rows",
            report.node_rows_written
        );
        assert_eq!(report.relation_rows_written, 1);

        memory.refresh().unwrap();
        let validation = memory.projection_validate().unwrap();
        assert!(validation.valid, "{:?}", validation.issues);
        assert_eq!(
            validation.status.table_counts["episode"],
            next_sequence as usize
        );
        assert_eq!(
            validation.status.table_counts["mentions"],
            next_sequence as usize
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn incremental_sync_removes_relations_that_are_no_longer_projected() {
        let path = temp_path("projection_incremental_stale_relation");
        let _ = fs::remove_file(&path);

        let mut memory = MemoryEngine::open(&path).unwrap();
        let dependency = memory
            .add_intention(
                "Prepare release",
                IntentionKind::Task,
                IntentionPriority::Medium,
                None,
            )
            .unwrap();
        let intention = memory
            .add_intention(
                "Publish release",
                IntentionKind::Task,
                IntentionPriority::High,
                None,
            )
            .unwrap();
        memory
            .update_intention(
                intention.id.clone(),
                IntentionUpdateOptions {
                    depends_on: Some(vec![dependency.id]),
                    ..IntentionUpdateOptions::default()
                },
            )
            .unwrap();
        assert_eq!(
            memory.projection_validate().unwrap().status.table_counts
                ["intention_depends_on"],
            1
        );

        memory
            .update_intention(
                intention.id,
                IntentionUpdateOptions {
                    depends_on: Some(Vec::new()),
                    ..IntentionUpdateOptions::default()
                },
            )
            .unwrap();
        let validation = memory.projection_validate().unwrap();
        assert!(validation.valid, "{:?}", validation.issues);
        assert_eq!(
            validation.status.table_counts["intention_depends_on"],
            0
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn concurrent_graph_rebuilds_leave_one_complete_projection() {
        const REBUILDER_COUNT: usize = 8;
        let path = temp_path("concurrent_graph_rebuilds");
        let _ = fs::remove_file(&path);

        let mut memory = MemoryEngine::open(&path).unwrap();
        let episode = memory
            .remember_with_mentions(
                "Lena owns release notes.",
                vec!["product".to_string()],
                vec!["Lena".to_string(), "Release Notes".to_string()],
            )
            .unwrap();
        memory
            .add_claim(
                "Lena",
                "owns",
                "release notes",
                Some(episode.id.clone()),
                0.9,
            )
            .unwrap();
        memory
            .add_link(
                "Lena",
                "owns",
                "Release Notes",
                Some(episode.id),
                0.9,
            )
            .unwrap();
        drop(memory);

        let barrier = Arc::new(Barrier::new(REBUILDER_COUNT));
        let handles = (0..REBUILDER_COUNT)
            .map(|_| {
                let path = path.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    let mut memory = MemoryEngine::open(&path)?;
                    barrier.wait();
                    memory.projection_rebuild()
                })
            })
            .collect::<Vec<_>>();

        for handle in handles {
            let report = handle
                .join()
                .expect("projection rebuilder thread completes")
                .expect("concurrent projection rebuild succeeds");
            assert!(report.status.in_sync);
        }

        let memory = MemoryEngine::open(&path).unwrap();
        let validation = memory.projection_validate().unwrap();
        assert!(validation.valid, "{:?}", validation.issues);
        assert_eq!(validation.status.table_counts["claim"], 1);
        assert_eq!(validation.status.table_counts["mentions"], 2);
        assert_eq!(validation.status.table_counts["relates_to"], 1);
        assert_eq!(validation.status.table_counts["supports"], 1);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn rebuild_never_returns_success_for_a_failed_projection_postcondition() {
        let path = temp_path("projection_postcondition_failure");
        let _ = fs::remove_file(&path);
        let mut memory = MemoryEngine::open(&path).unwrap();
        let mut report = memory.projection_rebuild().unwrap();
        report.status.in_sync = false;

        let error = ensure_graph_projection_rebuild_postcondition(report)
            .expect_err("an out-of-sync rebuild report must be rejected");
        assert!(matches!(
            error,
            NahualiError::GraphProjectionPostconditionFailed { .. }
        ));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn projection_manifest_detects_mutated_content_with_unchanged_row_identity_and_count() {
        let path = temp_path("projection_manifest_content_mutation");
        let _ = fs::remove_file(&path);
        let mut memory = MemoryEngine::open(&path).unwrap();
        let episode = memory
            .remember("Lena owns release notes.", Vec::new())
            .unwrap();
        let claim = memory
            .add_claim(
                "Lena",
                "owns",
                "release notes",
                Some(episode.id),
                0.9,
            )
            .unwrap();
        assert!(memory.projection_validate().unwrap().valid);

        let query_path = path.clone();
        let claim_id = claim.id.clone();
        block_on_database(async move {
            let db = open_database(&query_path).await?;
            db.query_with_retry(
                &query_path,
                "UPDATE claim SET object = $object WHERE memory_id = $memory_id",
                vec![
                    (
                        "object".to_string(),
                        serde_json::json!("tampered release notes"),
                    ),
                    ("memory_id".to_string(), serde_json::json!(claim_id)),
                ],
            )
            .await?;
            Ok(())
        })
        .unwrap();

        let validation = memory.projection_validate().unwrap();
        assert!(!validation.valid);
        assert_eq!(validation.status.table_counts["claim"], 1);
        assert!(validation.issues.iter().any(|issue| {
            issue.contains("manifest table digests") && issue.contains("claim")
        }));
        let read_error = memory
            .projection_entities(None, 10)
            .expect_err("graph navigation must fail closed on manifest drift");
        assert!(matches!(
            read_error,
            NahualiError::GraphProjectionInvalid { .. }
        ));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn projection_validation_rejects_a_stored_projection_version_mismatch() {
        let path = temp_path("projection_checkpoint_version_mismatch");
        let _ = fs::remove_file(&path);
        let mut memory = MemoryEngine::open(&path).unwrap();
        memory.remember("Versioned projection", Vec::new()).unwrap();
        assert!(memory.projection_validate().unwrap().valid);

        let query_path = path.clone();
        block_on_database(async move {
            let db = open_database(&query_path).await?;
            db.query_with_retry(
                &query_path,
                "UPDATE projection_checkpoint SET projection_version = 999 \
                 WHERE checkpoint_id = $checkpoint_id",
                vec![(
                    "checkpoint_id".to_string(),
                    serde_json::json!(super::GRAPH_PROJECTION_CHECKPOINT_ID),
                )],
            )
            .await?;
            Ok(())
        })
        .unwrap();

        let validation = memory.projection_validate().unwrap();
        assert!(!validation.valid);
        assert!(validation
            .issues
            .iter()
            .any(|issue| issue.contains("checkpoint projection version Some(999)")));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn graph_projection_lease_fences_a_replaced_owner() {
        let path = temp_path("projection_lease_fencing");
        let _ = fs::remove_file(&path);
        let async_path = path.clone();
        let (first_fence, second_fence, stale_error) = block_on_database(async move {
            let db = open_database(&async_path).await?;
            let first =
                acquire_graph_projection_rebuild_lock(&async_path, &db, "first-owner").await?;
            release_graph_projection_rebuild_lock(&async_path, &db, &first).await?;
            let second =
                acquire_graph_projection_rebuild_lock(&async_path, &db, "second-owner").await?;
            let stale_error = verify_graph_projection_lease(&async_path, &db, &first)
                .await
                .expect_err("the replaced owner must be fenced");
            let result = (first.fencing_token, second.fencing_token, stale_error);
            release_graph_projection_rebuild_lock(&async_path, &db, &second).await?;
            Ok(result)
        })
        .unwrap();

        assert!(second_fence > first_fence);
        assert!(matches!(
            stale_error,
            NahualiError::GraphProjectionLeaseLost { fencing_token }
                if fencing_token == first_fence
        ));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn stale_projection_owner_cannot_mutate_after_replacement_completes() {
        let path = temp_path("stale_projection_owner_mutation");
        let _ = fs::remove_file(&path);
        let mut memory = MemoryEngine::open(&path).unwrap();
        let episode = memory
            .remember("Lena owns release notes.", Vec::new())
            .unwrap();
        memory
            .add_claim(
                "Lena",
                "owns",
                "release notes",
                Some(episode.id),
                0.9,
            )
            .unwrap();
        assert!(memory.projection_validate().unwrap().valid);
        drop(memory);

        let async_path = path.clone();
        let (first_fence, second_fence, stale_delete, stale_create) =
            block_on_database(async move {
                let db = open_database(&async_path).await?;
                let first = acquire_graph_projection_rebuild_lock(
                    &async_path,
                    &db,
                    "paused-first-owner",
                )
                .await?;
                verify_graph_projection_lease(&async_path, &db, &first).await?;

                // Deterministically model A pausing after verification while
                // its database lease expires and a replacement rebuilds.
                db.query_with_retry(
                    &async_path,
                    format!(
                        "UPDATE ONLY projection_rebuild_lock:{} \
                         SET expires_at_ms = 0 \
                         WHERE owner_token = $lease_token AND fencing_token = $fencing_token",
                        super::GRAPH_PROJECTION_REBUILD_LOCK_ID
                    ),
                    vec![
                        (
                            "lease_token".to_string(),
                            serde_json::json!(first.owner_token.as_str()),
                        ),
                        (
                            "fencing_token".to_string(),
                            serde_json::json!(first.fencing_token),
                        ),
                    ],
                )
                .await?;

                let second = acquire_graph_projection_rebuild_lock(
                    &async_path,
                    &db,
                    "replacement-owner",
                )
                .await?;
                let events = read_records(&async_path).await?;
                let data = projection::project(&events);
                let report = rebuild_graph_projection_locked(
                    &async_path,
                    &data,
                    &events,
                    db.clone(),
                    &second,
                )
                .await?;
                assert!(report.status.in_sync);
                release_graph_projection_rebuild_lock(&async_path, &db, &second).await?;

                let stale_delete = query_graph_projection_mutation(
                    &async_path,
                    &db,
                    &first,
                    "DELETE claim",
                    Vec::new(),
                )
                .await;
                let stale_create = create_single_projected_record(
                    &async_path,
                    &db,
                    &first,
                    "claim",
                    "claim_stale_owner",
                    serde_json::json!({
                        "memory_id": "claim_stale_owner",
                        "object": "stale mutation",
                    }),
                )
                .await;

                Ok((
                    first.fencing_token,
                    second.fencing_token,
                    stale_delete,
                    stale_create,
                ))
            })
            .unwrap();

        assert!(second_fence > first_fence);
        for error in [stale_delete, stale_create] {
            assert!(matches!(
                &error,
                Err(NahualiError::GraphProjectionLeaseLost { fencing_token })
                    if *fencing_token == first_fence
            ), "unexpected stale mutation result: {error:?}");
        }

        let validation = MemoryEngine::open(&path)
            .unwrap()
            .projection_validate()
            .unwrap();
        assert!(validation.valid, "{:?}", validation.issues);
        assert_eq!(validation.status.table_counts["claim"], 1);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn overlapping_multi_row_projection_batches_serialize_on_the_lock_row() {
        async fn next_mutation_guard_value(
            path: &Path,
            db: &DatabaseSession,
        ) -> crate::Result<u64> {
            let mut response = db
                .query_with_retry(
                    path,
                    format!(
                        "RETURN sequence::nextval('{}')",
                        super::GRAPH_PROJECTION_MUTATION_GUARD_SEQUENCE
                    ),
                    Vec::new(),
                )
                .await?;
            let value: Option<serde_json::Value> = response
                .take(0)
                .map_err(|source| super::database_error(path, source))?;
            Ok(value
                .and_then(|value| value.as_u64())
                .expect("the projection mutation guard sequence returns an integer"))
        }

        let path = temp_path("overlapping_projection_batches");
        let _ = fs::remove_file(&path);
        let mut memory = MemoryEngine::open(&path).unwrap();
        let episode = memory
            .remember("Lena owns release notes.", Vec::new())
            .unwrap();
        memory
            .add_claim(
                "Lena",
                "owns",
                "release notes",
                Some(episode.id),
                0.9,
            )
            .unwrap();
        assert!(memory.projection_validate().unwrap().valid);
        drop(memory);

        let first_path = path.clone();
        let (first_db, replacement_db, first, baseline_guard_value) =
            block_on_database(async move {
            let first_db = open_database(&first_path).await?;
            let replacement_db = open_database(&first_path).await?;
            let lease = acquire_graph_projection_rebuild_lock(
                &first_path,
                &first_db,
                "overlapping-first-owner",
            )
            .await?;
            first_db.query_with_retry(
                &first_path,
                format!(
                    "UPDATE ONLY projection_rebuild_lock:{} \
                     SET expires_at_ms = 0 \
                     WHERE owner_token = $lease_token AND fencing_token = $fencing_token",
                    super::GRAPH_PROJECTION_REBUILD_LOCK_ID
                ),
                vec![
                    (
                        "lease_token".to_string(),
                        serde_json::json!(lease.owner_token.as_str()),
                    ),
                    (
                        "fencing_token".to_string(),
                        serde_json::json!(lease.fencing_token),
                    ),
                ],
            )
            .await?;
            let guard_value = next_mutation_guard_value(&first_path, &first_db).await?;
            Ok((first_db, replacement_db, lease, guard_value))
        })
        .unwrap();
        let first_fence = first.fencing_token;

        let (started_sender, started_receiver) = mpsc::channel();
        let stale_path = path.clone();
        let stale_writer = std::thread::spawn(move || {
            block_on_database(async move {
                started_sender
                    .send(())
                    .expect("overlap test starter still receives");
                let mutation = query_graph_projection_mutation(
                    &stale_path,
                    &first_db,
                    &first,
                    "SLEEP 5s; \
                     DELETE claim; \
                     CREATE claim:claim_overlap_stale_1 CONTENT { \
                         memory_id: 'claim_overlap_stale_1', \
                         projection_version: 2 \
                     }; \
                     CREATE claim:claim_overlap_stale_2 CONTENT { \
                         memory_id: 'claim_overlap_stale_2', \
                         projection_version: 2 \
                     }",
                    Vec::new(),
                )
                .await;
                if mutation.is_ok() {
                    release_graph_projection_rebuild_lock(&stale_path, &first_db, &first).await?;
                }
                Ok(mutation)
            })
        });

        started_receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("stale writer starts its guarded transaction");

        // SurrealDB sequences are never rolled back. Polling the same BATCH 1
        // sequence until this thread observes a gap proves that A consumed a
        // value inside its open transaction and has reached the guarded sleep.
        // This is an ordering handshake, not an elapsed-time race.
        let guard_deadline = Instant::now() + Duration::from_secs(10);
        let mut previous_guard_value = baseline_guard_value;
        loop {
            let guard_path = path.clone();
            let guard_db = replacement_db.clone();
            let guard_value = block_on_database(async move {
                next_mutation_guard_value(&guard_path, &guard_db).await
            })
            .unwrap();
            if guard_value > previous_guard_value + 1 {
                break;
            }
            previous_guard_value = guard_value;
            assert!(
                Instant::now() < guard_deadline,
                "stale writer never entered its fenced transaction"
            );
            std::thread::yield_now();
        }
        assert!(
            !stale_writer.is_finished(),
            "stale writer must still be sleeping after the sequence handshake"
        );

        let replacement_path = path.clone();
        let acquire_db = replacement_db.clone();
        let replacement = block_on_database(async move {
            let lease = acquire_graph_projection_rebuild_lock(
                &replacement_path,
                &acquire_db,
                "overlapping-replacement-owner",
            )
            .await?;
            Ok(lease)
        })
        .unwrap();
        let stale_finished_when_replacement_acquired = stale_writer.is_finished();
        let second_fence = replacement.fencing_token;

        let stale_mutation = stale_writer
            .join()
            .expect("stale writer thread completes")
            .unwrap();
        let error = stale_mutation.expect_err(
            "the replacement must win the lock-row conflict while the stale batch sleeps",
        );
        assert!(
            !stale_finished_when_replacement_acquired,
            "the stale transaction must still be running when its commit loses the lock-row conflict"
        );
        assert!(matches!(
            error,
            NahualiError::GraphProjectionLeaseLost { fencing_token }
                if fencing_token == first_fence
        ), "unexpected stale batch result: {error:?}");
        let stale_path = path.clone();
        let (db, stale_row_count) = block_on_database(async move {
            let mut response = replacement_db
                .query_with_retry(
                    &stale_path,
                    "SELECT memory_id FROM claim \
                     WHERE memory_id IN ['claim_overlap_stale_1', 'claim_overlap_stale_2']",
                    Vec::new(),
                )
                .await?;
            let rows: Vec<serde_json::Value> = response
                .take(0)
                .map_err(|source| super::database_error(&stale_path, source))?;
            Ok((replacement_db, rows.len()))
        })
        .unwrap();
        assert_eq!(
            stale_row_count, 0,
            "a conflicted stale batch must roll back every row"
        );

        let rebuild_path = path.clone();
        block_on_database(async move {
            let events = read_records(&rebuild_path).await?;
            let data = projection::project(&events);
            let report = rebuild_graph_projection_locked(
                &rebuild_path,
                &data,
                &events,
                db.clone(),
                &replacement,
            )
            .await?;
            release_graph_projection_rebuild_lock(&rebuild_path, &db, &replacement).await?;
            Ok(report)
        })
        .unwrap();

        assert!(second_fence > first_fence);
        let validation = MemoryEngine::open(&path)
            .unwrap()
            .projection_validate()
            .unwrap();
        assert!(validation.valid, "{:?}", validation.issues);
        assert_eq!(validation.status.table_counts["claim"], 1);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn rebuild_waiter_cannot_coalesce_while_an_active_owner_can_clear_projection() {
        let path = temp_path("projection_waiter_active_owner");
        let _ = fs::remove_file(&path);
        let mut memory = MemoryEngine::open(&path).unwrap();
        memory.remember("Projected episode", Vec::new()).unwrap();
        assert!(memory.projection_validate().unwrap().valid);

        let owner_path = path.clone();
        let (owner_db, owner_lease) = block_on_database(async move {
            let db = open_database(&owner_path).await?;
            let lease =
                acquire_graph_projection_rebuild_lock(&owner_path, &db, "active-owner").await?;
            Ok((db, lease))
        })
        .unwrap();

        let active_read_error = memory
            .projection_entities(None, 10)
            .expect_err("graph navigation must refuse an active rebuild");
        assert!(matches!(
            active_read_error,
            NahualiError::GraphProjectionInvalid { .. }
        ));

        let waiter_path = path.clone();
        let (sender, receiver) = mpsc::channel();
        let waiter = std::thread::spawn(move || {
            let result = block_on_database(async move {
                rebuild_graph_projection(&waiter_path).await
            });
            sender.send(result).expect("waiter result receiver exists");
        });

        assert!(matches!(
            receiver.recv_timeout(Duration::from_millis(250)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));

        let clear_path = path.clone();
        block_on_database(async move {
            clear_graph_projection(&clear_path, &owner_db, &owner_lease).await?;
            release_graph_projection_rebuild_lock(&clear_path, &owner_db, &owner_lease).await
        })
        .unwrap();

        let report = receiver
            .recv_timeout(Duration::from_secs(45))
            .expect("waiter completes after the owner releases")
            .expect("waiter rebuild succeeds");
        assert!(report.status.in_sync);
        assert!(report.node_rows_written > 0);
        waiter.join().expect("waiter thread completes");

        let validation = MemoryEngine::open(&path)
            .unwrap()
            .projection_validate()
            .unwrap();
        assert!(validation.valid, "{:?}", validation.issues);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn coalesces_only_a_completed_rebuild_at_a_stable_ledger_tip() {
        let path = temp_path("coalesced_graph_rebuild");
        let _ = fs::remove_file(&path);
        let mut memory = MemoryEngine::open(&path).unwrap();
        memory.remember("Projected episode", Vec::new()).unwrap();

        let check_path = path.clone();
        let report = block_on_database(async move {
            completed_concurrent_rebuild(&check_path).await
        })
        .unwrap()
        .expect("an in-sync stable projection satisfies the rebuild request");
        assert!(report.status.in_sync);
        assert_eq!(report.node_rows_written, 0);
        assert_eq!(report.relation_rows_written, 0);

        let pending = EventEnvelope::new(
            2,
            2,
            MemoryEvent::EpisodeRecorded(EpisodeRecorded {
                id: "episode_unprojected".to_string(),
                content: "Ledger-only episode".to_string(),
                tags: Vec::new(),
                mentions: Vec::new(),
                source_id: None,
                source_position: None,
                source_role: None,
                scope: None,
            }),
        );
        let write_path = path.clone();
        block_on_database(async move { write_record(&write_path, &pending).await }).unwrap();

        let stale_path = path.clone();
        let stale = block_on_database(async move {
            completed_concurrent_rebuild(&stale_path).await
        })
        .unwrap();
        assert!(stale.is_none(), "an unprojected ledger tip must not coalesce");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn scoped_records_project_and_recall_after_reopen() {
        let path = temp_path("scoped_records_project_and_recall_after_reopen");
        let _ = fs::remove_file(&path);
        let scope = MemoryScope::new(MemoryScopeKind::Project, "Nahuali").unwrap();
        let other_scope = MemoryScope::new(MemoryScopeKind::Project, "Other").unwrap();

        let mut memory = MemoryEngine::open(&path).unwrap();
        let source = memory
            .record_source_with_options(SourceRecordOptions {
                kind: SourceKind::Conversation,
                title: Some("Release review".to_string()),
                uri: Some("fixture://release-review".to_string()),
                content_checksum: "fnv1a64:scoped".to_string(),
                byte_len: 64,
                metadata: Default::default(),
                scope: Some(scope.clone()),
            })
            .unwrap();
        let episode = memory
            .remember_source_episode(
                "Lena owns the release notes.",
                vec!["product".to_string()],
                vec!["Lena".to_string()],
                source.id,
                Some(1),
                Some("user".to_string()),
            )
            .unwrap();
        memory
            .add_claim("Lena", "owns", "release notes", Some(episode.id), 0.9)
            .unwrap();
        let other_episode = memory
            .remember_with_mentions_scoped(
                "Lena owns other release notes.",
                vec!["product".to_string()],
                vec!["Lena".to_string()],
                other_scope,
            )
            .unwrap();

        let reopened = MemoryEngine::open(&path).unwrap();
        let scoped = reopened.recall_scoped("release notes", 10, &scope).unwrap();
        let global = reopened.recall("release notes", 10).unwrap();

        assert_eq!(reopened.data().sources[0].scope, Some(scope.clone()));
        assert_eq!(reopened.data().episodes[0].scope, Some(scope.clone()));
        assert_eq!(reopened.data().claims[0].scope, Some(scope.clone()));
        assert!(
            scoped
                .iter()
                .all(|result| result.scope.as_ref().map(|scope| &scope.key) == Some(&scope.key))
        );
        assert!(!scoped.iter().any(|result| result.id == other_episode.id));
        assert!(global.iter().any(|result| result.id == other_episode.id));
        assert_eq!(
            reopened
                .data()
                .entities
                .iter()
                .filter(|entity| entity.name == "Lena")
                .count(),
            2
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_unknown_intention_status_updates() {
        let path = temp_path("rejects_unknown_intention_status_updates");
        let _ = fs::remove_file(&path);

        let mut memory = MemoryEngine::open(&path).unwrap();
        let error = memory
            .set_intention_status("intention_missing", IntentionStatus::Completed, None)
            .unwrap_err();

        assert!(matches!(error, NahualiError::UnknownIntention { .. }));
        assert!(memory.events().is_empty());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_unknown_noop_and_self_dependent_intention_updates() {
        let path = temp_path("rejects_unknown_noop_and_self_dependent_intention_updates");
        let _ = fs::remove_file(&path);

        let mut memory = MemoryEngine::open(&path).unwrap();
        let unknown = memory
            .update_intention(
                "intention_missing",
                IntentionUpdateOptions {
                    progress_percent: Some(Some(10)),
                    ..IntentionUpdateOptions::default()
                },
            )
            .unwrap_err();
        assert!(matches!(unknown, NahualiError::UnknownIntention { .. }));

        let intention = memory
            .add_intention(
                "Ship release notes",
                IntentionKind::Task,
                IntentionPriority::High,
                None,
            )
            .unwrap();
        let events_before_invalid = memory.events().len();
        let no_op = memory
            .update_intention(intention.id.clone(), IntentionUpdateOptions::default())
            .unwrap_err();
        assert!(matches!(
            no_op,
            NahualiError::InvalidIntentionUpdate { .. }
        ));
        let self_dependency = memory
            .update_intention(
                intention.id.clone(),
                IntentionUpdateOptions {
                    depends_on: Some(vec![intention.id]),
                    ..IntentionUpdateOptions::default()
                },
            )
            .unwrap_err();
        assert!(matches!(
            self_dependency,
            NahualiError::InvalidIntentionUpdate { .. }
        ));
        assert_eq!(memory.events().len(), events_before_invalid);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn inspection_reports_unsupported_facts() {
        let path = temp_path("inspection_reports_unsupported_facts");
        let _ = fs::remove_file(&path);

        let mut memory = MemoryEngine::open(&path).unwrap();
        memory
            .add_fact("Lena", "prefers", "release notes", None, 0.4)
            .unwrap();

        let health = memory.inspect();
        let authority = memory.authority();

        assert_eq!(health.unsupported_fact_count, 1);
        assert_eq!(health.low_confidence_fact_count, 1);
        assert_eq!(authority.mode, AuthorityMode::Block);

        let _ = fs::remove_file(path);
    }

    /// Seed a throwaway source store with a representative spread of records and
    /// return its source-neutral interchange document. Used by the batched-import
    /// tests below so they exercise sources, episodes, claims, links, procedures,
    /// and intention lifecycle events in a single import.
    fn seed_interchange(name: &str) -> crate::MemoryInterchange {
        let path = temp_path(name);
        let _ = fs::remove_file(&path);
        let mut source = MemoryEngine::open(&path).unwrap();
        let document = source
            .record_source(
                SourceKind::Conversation,
                Some("Release review".to_string()),
                Some("fixture://release-review".to_string()),
                "checksum-release-review",
                82,
                std::collections::BTreeMap::new(),
            )
            .unwrap();
        let episode = source
            .remember_source_episode(
                "Lena owns the release notes.",
                vec!["product".to_string()],
                vec!["Lena".to_string(), "Release Notes".to_string()],
                document.id,
                Some(1),
                Some("operator".to_string()),
            )
            .unwrap();
        source
            .add_claim("Lena", "owns", "release notes", Some(episode.id.clone()), 0.9)
            .unwrap();
        source
            .add_link("Lena", "owns", "Release Notes", Some(episode.id.clone()), 0.9)
            .unwrap();
        source
            .add_preference(
                "Release notes",
                "Keep release notes concise.",
                Some(episode.id.clone()),
                0.88,
            )
            .unwrap();
        let intention = source
            .add_intention(
                "Ship release notes",
                IntentionKind::Task,
                IntentionPriority::High,
                Some(episode.id),
            )
            .unwrap();
        source
            .set_intention_status(
                intention.id,
                IntentionStatus::Blocked,
                Some("Waiting for review".to_string()),
            )
            .unwrap();

        let interchange = source.export_interchange();
        let _ = fs::remove_file(path);
        interchange
    }

    #[test]
    fn batched_import_writes_a_well_formed_ledger_in_one_flush() {
        // Importing buffers every event and flushes the records with a single
        // database write plus one graph rebuild. Reopening replays and validates
        // the ledger, so this proves the deferred flush wrote the same ordered,
        // checksum-valid records the per-event path would have, and that the
        // single graph rebuild matches the projection.
        let interchange = seed_interchange("batched_import_source");

        let target_path = temp_path("batched_import_target");
        let _ = fs::remove_file(&target_path);
        let mut target = MemoryEngine::open(&target_path).unwrap();
        let report = target.import_interchange(&interchange, false).unwrap();
        assert!(report.valid);
        let expected_events = report.imported_event_count;
        assert!(expected_events >= 6, "fixture should be multi-record");
        assert_eq!(target.events().len(), expected_events);

        let validation = MemoryEngine::validate_store(&target_path).unwrap();
        assert!(validation.valid, "{:?}", validation.issues);

        let reopened = MemoryEngine::open(&target_path).unwrap();
        assert_eq!(reopened.events().len(), expected_events);
        assert_eq!(reopened.data().event_count, expected_events);
        assert_eq!(reopened.data().sources.len(), 1);
        assert_eq!(reopened.data().episodes.len(), 1);
        assert_eq!(reopened.data().claims.len(), 1);
        assert_eq!(reopened.data().links.len(), 1);
        assert_eq!(reopened.data().procedures.len(), 1);
        assert_eq!(reopened.data().intentions.len(), 1);
        let projection = reopened.projection_validate().unwrap();
        assert!(projection.valid, "{:?}", projection.issues);

        let _ = fs::remove_file(target_path);
    }

    #[test]
    fn bulk_ledger_insert_rolls_back_every_record_when_one_conflicts() {
        let path = temp_path("bulk_ledger_insert_is_atomic");
        let _ = fs::remove_file(&path);
        drop(MemoryEngine::open(&path).unwrap());

        let payload = |id: &str| {
            MemoryEvent::EpisodeRecorded(EpisodeRecorded {
                id: id.to_string(),
                content: format!("Atomic fixture {id}"),
                tags: Vec::new(),
                mentions: Vec::new(),
                source_id: None,
                source_position: None,
                source_role: None,
                scope: None,
            })
        };
        let events = vec![
            EventEnvelope::new(1, 1, payload("episode_first")),
            EventEnvelope::new(2, 2, payload("episode_second")),
            EventEnvelope::new(2, 3, payload("episode_conflict")),
        ];
        let write_path = path.clone();
        let error = block_on_database(async move { write_records(&write_path, &events).await })
            .expect_err("duplicate sequence aborts the bulk insert");
        assert!(
            super::is_record_sequence_conflict(&error),
            "unexpected error: {error}"
        );

        let read_path = path.clone();
        let persisted =
            block_on_database(async move { read_records(&read_path).await }).unwrap();
        assert!(persisted.is_empty(), "a failed batch persisted a prefix");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn concurrent_direct_writers_recover_sequence_collisions() {
        const WRITER_COUNT: usize = 4;
        let path = temp_path("concurrent_direct_writers");
        let _ = fs::remove_file(&path);
        drop(MemoryEngine::open(&path).unwrap());

        let barrier = Arc::new(Barrier::new(WRITER_COUNT));
        let handles = (0..WRITER_COUNT)
            .map(|index| {
                let path = path.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    let mut memory = MemoryEngine::open(&path)?;
                    barrier.wait();
                    memory.remember(format!("Concurrent episode {index}"), Vec::new())
                })
            })
            .collect::<Vec<_>>();

        for handle in handles {
            handle
                .join()
                .expect("concurrent writer thread completes")
                .expect("sequence collision is recovered without operator retry");
        }

        let reopened = MemoryEngine::open(&path).unwrap();
        assert_eq!(reopened.events().len(), WRITER_COUNT);
        assert_eq!(
            reopened
                .events()
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 4]
        );
        assert!(reopened.projection_validate().unwrap().valid);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn post_commit_projection_failure_reports_committed_event_and_keeps_state() {
        let path = temp_path("post_commit_projection_failure");
        let _ = fs::remove_file(&path);
        let mut memory = MemoryEngine::open(&path).unwrap();
        inject_graph_projection_failure_once(&path);

        let error = memory
            .remember("Committed before projection failure", Vec::new())
            .expect_err("injected projection failure must be surfaced");
        match &error {
            NahualiError::LedgerCommittedProjectionFailed {
                first_event_id,
                last_event_id,
                first_sequence,
                last_sequence,
                event_count,
                ..
            } => {
                assert_eq!(first_event_id, last_event_id);
                assert_eq!((*first_sequence, *last_sequence, *event_count), (1, 1, 1));
            }
            other => panic!("unexpected error: {other}"),
        }
        assert!(error.ledger_commit_confirmed());
        assert_eq!(memory.events().len(), 1, "committed state was rolled back");

        let mut reopened = MemoryEngine::open(&path).unwrap();
        assert_eq!(reopened.events().len(), 1);
        assert!(!reopened.projection_validate().unwrap().valid);
        reopened.projection_rebuild().unwrap();
        assert!(reopened.projection_validate().unwrap().valid);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn batch_projection_failure_reports_the_full_committed_range() {
        let interchange = seed_interchange("batch_projection_failure_source");
        let path = temp_path("batch_projection_failure");
        let _ = fs::remove_file(&path);
        let mut memory = MemoryEngine::open(&path).unwrap();
        inject_graph_projection_failure_once(&path);

        let error = memory
            .import_interchange(&interchange, false)
            .expect_err("injected batch projection failure must be surfaced");
        match &error {
            NahualiError::LedgerCommittedProjectionFailed {
                first_event_id,
                last_event_id,
                first_sequence,
                last_sequence,
                event_count,
                ..
            } => {
                assert_ne!(first_event_id, last_event_id);
                assert_eq!(*first_sequence, 1);
                assert_eq!(*last_sequence as usize, *event_count);
                assert_eq!(*event_count, memory.events().len());
                assert!(*event_count >= 6, "fixture should be multi-record");
            }
            other => panic!("unexpected error: {other}"),
        }
        assert!(error.ledger_commit_confirmed());

        let mut reopened = MemoryEngine::open(&path).unwrap();
        assert_eq!(reopened.events().len(), memory.events().len());
        assert!(!reopened.projection_validate().unwrap().valid);
        reopened.projection_rebuild().unwrap();
        assert!(reopened.projection_validate().unwrap().valid);

        let _ = fs::remove_file(path);
    }

    #[cfg(feature = "tamper-evidence")]
    #[test]
    fn batched_import_preserves_the_hash_chain() {
        // The hash chain links each event to the previous one's chained hash.
        // A buffered batch must produce exactly the chain the per-event path
        // would: reopening runs the chain-linkage check on every record, so a
        // mismatch would fail the reopen. Every imported event must be chained
        // and the tip must be exposed for anchoring.
        let interchange = seed_interchange("batched_chain_source");

        let target_path = temp_path("batched_chain_target");
        let _ = fs::remove_file(&target_path);
        let mut target = MemoryEngine::open(&target_path).unwrap();
        let expected_events = target
            .import_interchange(&interchange, false)
            .unwrap()
            .imported_event_count;
        assert!(target.chain_tip().is_some());
        assert!(target.events().iter().all(|event| event.is_chained()));

        let reopened = MemoryEngine::open(&target_path).unwrap();
        assert_eq!(reopened.events().len(), expected_events);
        assert!(reopened.events().iter().all(|event| event.is_chained()));
        assert_eq!(reopened.chain_tip(), target.chain_tip());

        let _ = fs::remove_file(target_path);
    }

    #[test]
    fn repair_rejects_fabricated_and_empty_evidence() {
        let path = temp_path("repair_rejects_fabricated_and_empty_evidence");
        let _ = fs::remove_file(&path);

        let mut memory = MemoryEngine::open(&path).unwrap();
        memory
            .remember("Lena shipped the release.", vec!["release".to_string()])
            .unwrap();

        // A fabricated citation is never minted into evidence-backed memory.
        let fabricated = consolidate("Lena", "owns", "release notes", &["episode_ghost"]);
        let error = memory.apply_repair(fabricated, false, false).unwrap_err();
        assert_eq!(error.to_string(), "unknown source episode: episode_ghost");

        // No evidence at all is rejected too.
        let empty = consolidate("Lena", "owns", "release notes", &[]);
        assert!(matches!(
            memory.apply_repair(empty, false, false).unwrap_err(),
            NahualiError::InvalidRepairProposal { .. }
        ));

        assert!(memory.data().claims.is_empty());
        assert!(memory.data().repairs.is_empty());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn repair_auto_applies_homogeneous_consolidation() {
        let path = temp_path("repair_auto_applies_homogeneous_consolidation");
        let _ = fs::remove_file(&path);

        let mut memory = MemoryEngine::open(&path).unwrap();
        let first = memory
            .remember("Lena shipped the release.", vec!["release".to_string()])
            .unwrap();
        let second = memory
            .remember("Lena owns the release notes.", vec!["release".to_string()])
            .unwrap();

        let proposal = consolidate("Lena", "owns", "release notes", &[&first.id, &second.id]);

        // A dry-run plan previews Auto without writing.
        let plan = memory.repair_plan(proposal.clone()).unwrap();
        assert_eq!(plan.autonomy_level, AutonomyLevel::Auto);
        assert!(plan.dry_run);
        assert!(!plan.applied);
        assert!(memory.data().claims.is_empty());

        // Applying it writes one claim anchored to the first cited episode.
        let report = memory.apply_repair(proposal, false, false).unwrap();
        assert_eq!(report.autonomy_level, AutonomyLevel::Auto);
        assert!(report.applied);
        assert!(!report.operator_override);
        assert_eq!(report.kind, RepairKind::ConsolidateClaim);

        assert_eq!(memory.data().claims.len(), 1);
        let claim = &memory.data().claims[0];
        assert_eq!(claim.subject, "Lena");
        assert_eq!(claim.source_episode_id.as_deref(), Some(first.id.as_str()));
        assert_eq!(memory.data().repairs.len(), 1);
        assert_eq!(
            memory.data().repairs[0].materialized_id,
            report.materialized_id.unwrap()
        );

        // The repaired claim survives a reopen and is recallable.
        let reopened = MemoryEngine::open(&path).unwrap();
        assert_eq!(reopened.data().claims.len(), 1);
        assert_eq!(reopened.data().repairs.len(), 1);
        assert!(!reopened.recall("release notes", 10).unwrap().is_empty());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn repair_queues_ambiguous_consolidation_until_approved() {
        let path = temp_path("repair_queues_ambiguous_consolidation_until_approved");
        let _ = fs::remove_file(&path);

        let mut memory = MemoryEngine::open(&path).unwrap();
        // Distinct tags: not homogeneous, so the pattern needs operator judgment.
        let first = memory
            .remember("Lena shipped the release.", vec!["release".to_string()])
            .unwrap();
        let second = memory
            .remember("Lena reviewed the billing run.", vec!["billing".to_string()])
            .unwrap();

        let proposal = consolidate("Lena", "owns", "release notes", &[&first.id, &second.id]);

        // Without approval, a queued repair is reported but not written.
        let report = memory.apply_repair(proposal.clone(), false, false).unwrap();
        assert_eq!(report.autonomy_level, AutonomyLevel::Queue);
        assert!(!report.applied);
        assert!(memory.data().claims.is_empty());

        // With explicit operator approval it is written, recorded as an override.
        let approved = memory.apply_repair(proposal, true, false).unwrap();
        assert_eq!(approved.autonomy_level, AutonomyLevel::Queue);
        assert!(approved.applied);
        assert!(approved.operator_override);
        assert_eq!(memory.data().claims.len(), 1);
        assert!(memory.data().repairs[0].operator_override);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn repair_refuses_contradicting_claim_even_with_approval() {
        let path = temp_path("repair_refuses_contradicting_claim_even_with_approval");
        let _ = fs::remove_file(&path);

        let mut memory = MemoryEngine::open(&path).unwrap();
        let origin = memory
            .remember("Lena leads the roadmap.", vec!["roadmap".to_string()])
            .unwrap();
        memory
            .add_claim("Lena", "owns", "the roadmap", Some(origin.id), 0.9)
            .unwrap();
        let first = memory
            .remember("Lena shipped the release.", vec!["release".to_string()])
            .unwrap();
        let second = memory
            .remember("Lena owns the release notes.", vec!["release".to_string()])
            .unwrap();

        // Same subject+predicate, different value: a contradiction.
        let proposal = consolidate("Lena", "owns", "release notes", &[&first.id, &second.id]);

        let report = memory.apply_repair(proposal, true, false).unwrap();
        assert_eq!(report.autonomy_level, AutonomyLevel::NeverAuto);
        assert!(!report.applied);
        assert!(report.verdict.blocked_by.is_some());

        // The contradiction was surfaced, not masked: only the original claim
        // exists and no repair was recorded.
        assert_eq!(memory.data().claims.len(), 1);
        assert_eq!(memory.data().claims[0].object, "the roadmap");
        assert!(memory.data().repairs.is_empty());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn repair_links_two_present_entities() {
        let path = temp_path("repair_links_two_present_entities");
        let _ = fs::remove_file(&path);

        let mut memory = MemoryEngine::open(&path).unwrap();
        let episode = memory
            .remember_with_mentions(
                "Lena owns the release notes.",
                vec!["release".to_string()],
                vec!["Lena".to_string(), "Release Notes".to_string()],
            )
            .unwrap();

        let proposal = RepairProposal {
            payload: RepairPayload::LinkEntities(RepairLink {
                from: "Lena".to_string(),
                relation: "owns".to_string(),
                to: "Release Notes".to_string(),
                confidence: 0.9,
                scope: None,
            }),
            evidence_episode_ids: vec![episode.id.clone()],
            proposed_by: "claude-opus-4-8".to_string(),
            rationale: "the entities co-occur".to_string(),
        };

        let report = memory.apply_repair(proposal, false, false).unwrap();
        assert_eq!(report.autonomy_level, AutonomyLevel::Auto);
        assert!(report.applied);
        assert_eq!(report.kind, RepairKind::LinkEntities);
        assert_eq!(memory.data().links.len(), 1);
        assert_eq!(
            memory.data().links[0].source_episode_id.as_deref(),
            Some(episode.id.as_str())
        );

        // A link to an entity that is not present is rejected.
        let absent = RepairProposal {
            payload: RepairPayload::LinkEntities(RepairLink {
                from: "Lena".to_string(),
                relation: "mentors".to_string(),
                to: "Nobody".to_string(),
                confidence: 0.9,
                scope: None,
            }),
            evidence_episode_ids: vec![episode.id],
            proposed_by: "claude-opus-4-8".to_string(),
            rationale: "invented".to_string(),
        };
        assert!(matches!(
            memory.apply_repair(absent, false, false).unwrap_err(),
            NahualiError::InvalidRepairProposal { .. }
        ));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn repair_is_additive_and_a_newer_observation_supersedes_it() {
        let path = temp_path("repair_is_additive_and_a_newer_observation_supersedes_it");
        let _ = fs::remove_file(&path);

        let mut memory = MemoryEngine::open(&path).unwrap();
        let first = memory
            .remember("Bruno opened the migration.", vec!["status".to_string()])
            .unwrap();
        let second = memory
            .remember("Bruno is mid-migration.", vec!["status".to_string()])
            .unwrap();

        // An Auto repair consolidates the current status.
        let report = memory
            .apply_repair(
                consolidate("Bruno", "status", "active", &[&first.id, &second.id]),
                false,
                false,
            )
            .unwrap();
        assert!(report.applied);

        // A newer real observation disagrees. The repair is not mutated or
        // deleted; a superseding claim is appended (rule 3: additive, reversible).
        let third = memory
            .remember("Bruno finished the migration.", vec!["status".to_string()])
            .unwrap();
        memory
            .add_claim("Bruno", "status", "done", Some(third.id), 0.95)
            .unwrap();

        // Both claims remain in the append-only ledger; the engine surfaces the
        // disagreement rather than silently overwriting the repair.
        assert_eq!(memory.data().claims.len(), 2);
        let health = memory.inspect();
        assert!(health.superseded_fact_count + health.conflicting_fact_count >= 1);
        assert_eq!(memory.data().repairs.len(), 1);

        let _ = fs::remove_file(path);
    }

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("nahuali_{name}_{nanos}"))
    }
}
