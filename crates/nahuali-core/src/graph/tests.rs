#[cfg(test)]
mod tests {
    use crate::{
        Claim, Episode, Link, MemoryData, ReviewDecision, ReviewDecisionAction,
        ReviewDecisionOutcome,
    };

    use super::{
        GraphTraversalOptions, MEMORY_GRAPH_VERSION, MemoryGraphEdgeKind, MemoryGraphNodeKind,
        graph_neighborhood,
    };

    #[test]
    fn traverses_entity_neighborhood_with_health_and_review_overlays() {
        let data = MemoryData {
            event_count: 4,
            sources: Vec::new(),
            entities: vec![crate::Entity {
                id: "entity_lena".to_string(),
                name: "Lena".to_string(),
                mention_count: 2,
                first_seen_at_ms: 1,
                last_seen_at_ms: 2,
                source_event_ids: vec!["event_1".to_string(), "event_2".to_string()],
                scope: None,
            }],
            episodes: vec![Episode {
                id: "episode_1".to_string(),
                event_id: "event_1".to_string(),
                content: "Lena owns the release notes.".to_string(),
                tags: vec!["product".to_string()],
                mentions: vec!["Lena".to_string()],
                source_id: None,
                source_position: None,
                source_role: None,
                scope: None,
                created_at_ms: 1,
            }],
            claims: vec![Claim {
                id: "claim_1".to_string(),
                event_id: "event_2".to_string(),
                subject: "Lena".to_string(),
                predicate: "owns".to_string(),
                object: "release notes".to_string(),
                source_episode_id: None,
                confidence: 0.9,
                scope: None,
                created_at_ms: 2,
            }],
            links: vec![Link {
                id: "link_1".to_string(),
                event_id: "event_3".to_string(),
                from: "Lena".to_string(),
                relation: "owns".to_string(),
                to: "Release Notes".to_string(),
                source_episode_id: Some("episode_1".to_string()),
                confidence: 0.9,
                scope: None,
                created_at_ms: 3,
            }],
            review_decisions: vec![ReviewDecision {
                id: "review_decision_1".to_string(),
                event_id: "event_4".to_string(),
                review_id: "review_weak_evidence".to_string(),
                finding_id: "finding_weak_evidence".to_string(),
                action: ReviewDecisionAction::CaptureEvidence,
                outcome: ReviewDecisionOutcome::Resolved,
                note: "Operator confirmed ownership evidence.".to_string(),
                evidence_ids: vec!["event_2".to_string()],
                scope: None,
                created_at_ms: 4,
            }],
            facts: Vec::new(),
            relations: Vec::new(),
            ..MemoryData::default()
        };

        let report = graph_neighborhood(
            &data,
            "Lena",
            GraphTraversalOptions {
                max_depth: 2,
                limit: 20,
            },
        )
        .expect("graph traversal succeeds");

        assert_eq!(report.version, MEMORY_GRAPH_VERSION);
        assert!(report.summary.node_count >= 4);
        assert!(report.summary.edge_count >= 3);
        assert!(report.summary.relation_edge_count >= 1);
        assert!(report.summary.review_decision_count >= 1);
        assert!(
            report
                .nodes
                .iter()
                .any(|node| node.kind == MemoryGraphNodeKind::ReviewDecision)
        );
        assert!(
            report
                .edges
                .iter()
                .any(|edge| edge.kind == MemoryGraphEdgeKind::Relation)
        );
    }
}
