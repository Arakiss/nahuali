pub(crate) fn graph_neighborhood(
    data: &MemoryData,
    seed: &str,
    options: GraphTraversalOptions,
) -> Result<MemoryGraphReport> {
    let seed = seed.trim();
    if seed.is_empty() {
        return Err(NahualiError::EmptyQuery);
    }

    let graph = build_graph(data);
    let seed_ids = seed_nodes(&graph.nodes, seed);
    let max_depth = options.max_depth;
    let limit = options.limit.max(1);
    let included = traverse(&graph.edges, &seed_ids, max_depth, limit);
    let mut nodes = graph
        .nodes
        .into_iter()
        .filter_map(|(id, mut node)| {
            included.get(&id).map(|depth| {
                node.depth = *depth;
                node
            })
        })
        .collect::<Vec<_>>();
    nodes.sort_by(|left, right| {
        left.depth
            .cmp(&right.depth)
            .then_with(|| node_kind_rank(&left.kind).cmp(&node_kind_rank(&right.kind)))
            .then_with(|| left.label.cmp(&right.label))
            .then_with(|| left.id.cmp(&right.id))
    });

    let included_ids = nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<BTreeSet<_>>();
    let mut edges = graph
        .edges
        .into_values()
        .filter(|edge| included_ids.contains(&edge.from) && included_ids.contains(&edge.to))
        .collect::<Vec<_>>();
    edges.sort_by(|left, right| {
        edge_kind_rank(&left.kind)
            .cmp(&edge_kind_rank(&right.kind))
            .then_with(|| left.from.cmp(&right.from))
            .then_with(|| left.to.cmp(&right.to))
            .then_with(|| left.id.cmp(&right.id))
    });

    let health = KnowledgeHealth::inspect(data);
    let store_authority = AuthorityDecision::evaluate(&health);
    let evidence_ids = nodes
        .iter()
        .flat_map(|node| {
            node.evidence_ids
                .iter()
                .chain(node.source_event_ids.iter())
                .cloned()
        })
        .collect::<BTreeSet<_>>();
    let authority = AuthorityDecision::evaluate_for_evidence(&health, &evidence_ids);
    let summary = summarize(&nodes, &edges);

    Ok(MemoryGraphReport {
        version: MEMORY_GRAPH_VERSION,
        seed: seed.to_string(),
        max_depth,
        limit,
        event_count: data.event_count,
        authority,
        store_authority,
        summary,
        nodes,
        edges,
    })
}

struct BuiltGraph {
    nodes: BTreeMap<String, MemoryGraphNode>,
    edges: BTreeMap<String, MemoryGraphEdge>,
}

fn build_graph(data: &MemoryData) -> BuiltGraph {
    let mut graph = BuiltGraph {
        nodes: BTreeMap::new(),
        edges: BTreeMap::new(),
    };
    let mut entity_by_key = BTreeMap::<String, String>::new();
    let mut event_to_nodes = BTreeMap::<String, Vec<String>>::new();

    for entity in &data.entities {
        entity_by_key.insert(entity_key(&entity.name), entity.id.clone());
        insert_node(
            &mut graph.nodes,
            MemoryGraphNode {
                id: entity.id.clone(),
                kind: MemoryGraphNodeKind::Entity,
                label: entity.name.clone(),
                depth: usize::MAX,
                evidence_ids: entity.source_event_ids.clone(),
                source_event_ids: entity.source_event_ids.clone(),
                health_signal_count: 0,
                review_decision_count: 0,
            },
        );
        for event_id in &entity.source_event_ids {
            event_to_nodes
                .entry(event_id.clone())
                .or_default()
                .push(entity.id.clone());
        }
    }

    for episode in &data.episodes {
        insert_node(
            &mut graph.nodes,
            MemoryGraphNode {
                id: episode.id.clone(),
                kind: MemoryGraphNodeKind::Episode,
                label: truncate_label(&episode.content),
                depth: usize::MAX,
                evidence_ids: vec![episode.id.clone()],
                source_event_ids: vec![episode.event_id.clone()],
                health_signal_count: 0,
                review_decision_count: 0,
            },
        );
        event_to_nodes
            .entry(episode.event_id.clone())
            .or_default()
            .push(episode.id.clone());
        for mention in &episode.mentions {
            if let Some(entity_id) = entity_by_key.get(&entity_key(mention)) {
                insert_edge(
                    &mut graph.edges,
                    episode.id.clone(),
                    entity_id.clone(),
                    MemoryGraphEdgeKind::Mentions,
                    "mentions".to_string(),
                    None,
                    Some(episode.id.clone()),
                );
            }
        }
    }

    for claim in projected_claims(data) {
        add_claim(&mut graph, &mut entity_by_key, &mut event_to_nodes, claim);
    }

    for link in projected_links(data) {
        add_link(&mut graph, &mut entity_by_key, &mut event_to_nodes, link);
    }

    for procedure in &data.procedures {
        insert_node(
            &mut graph.nodes,
            MemoryGraphNode {
                id: procedure.id.clone(),
                kind: MemoryGraphNodeKind::Procedure,
                label: procedure.name.clone(),
                depth: usize::MAX,
                evidence_ids: procedure.source_episode_id.iter().cloned().collect(),
                source_event_ids: vec![procedure.event_id.clone()],
                health_signal_count: 0,
                review_decision_count: 0,
            },
        );
        event_to_nodes
            .entry(procedure.event_id.clone())
            .or_default()
            .push(procedure.id.clone());
        if let Some(episode_id) = &procedure.source_episode_id {
            insert_edge(
                &mut graph.edges,
                episode_id.clone(),
                procedure.id.clone(),
                MemoryGraphEdgeKind::Supports,
                "supports".to_string(),
                Some(procedure.confidence),
                Some(episode_id.clone()),
            );
        }
    }

    for intention in &data.intentions {
        insert_node(
            &mut graph.nodes,
            MemoryGraphNode {
                id: intention.id.clone(),
                kind: MemoryGraphNodeKind::Intention,
                label: intention.description.clone(),
                depth: usize::MAX,
                evidence_ids: intention.source_episode_id.iter().cloned().collect(),
                source_event_ids: vec![
                    intention.event_id.clone(),
                    intention.updated_event_id.clone(),
                ],
                health_signal_count: 0,
                review_decision_count: 0,
            },
        );
        event_to_nodes
            .entry(intention.event_id.clone())
            .or_default()
            .push(intention.id.clone());
        event_to_nodes
            .entry(intention.updated_event_id.clone())
            .or_default()
            .push(intention.id.clone());
        if let Some(episode_id) = &intention.source_episode_id {
            insert_edge(
                &mut graph.edges,
                episode_id.clone(),
                intention.id.clone(),
                MemoryGraphEdgeKind::Supports,
                "supports".to_string(),
                None,
                Some(episode_id.clone()),
            );
        }
    }

    for decision in &data.review_decisions {
        insert_node(
            &mut graph.nodes,
            MemoryGraphNode {
                id: decision.id.clone(),
                kind: MemoryGraphNodeKind::ReviewDecision,
                label: format!("{:?}: {}", decision.outcome, truncate_label(&decision.note)),
                depth: usize::MAX,
                evidence_ids: decision.evidence_ids.clone(),
                source_event_ids: vec![decision.event_id.clone()],
                health_signal_count: 0,
                review_decision_count: 1,
            },
        );
        event_to_nodes
            .entry(decision.event_id.clone())
            .or_default()
            .push(decision.id.clone());
        for evidence_id in &decision.evidence_ids {
            for node_id in nodes_for_evidence(&event_to_nodes, evidence_id) {
                insert_edge(
                    &mut graph.edges,
                    decision.id.clone(),
                    node_id,
                    MemoryGraphEdgeKind::Reviews,
                    "reviews".to_string(),
                    None,
                    Some(evidence_id.clone()),
                );
            }
        }
    }

    attach_health_and_review_counts(data, &mut graph.nodes, &event_to_nodes);
    graph
}
