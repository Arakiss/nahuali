#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::{
        AuthorityMode, IntentionKind, IntentionPriority, IntentionStatus, IntentionUpdateOptions,
        NahualiError, SourceKind, SourceRecordOptions,
        model::{MemoryScope, MemoryScopeKind, ProcedureKind},
    };

    use super::MemoryEngine;

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

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("nahuali_{name}_{nanos}"))
    }
}
