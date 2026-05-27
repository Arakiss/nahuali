fn add_claim(
    graph: &mut BuiltGraph,
    entity_by_key: &mut BTreeMap<String, String>,
    event_to_nodes: &mut BTreeMap<String, Vec<String>>,
    claim: &Claim,
) {
    let subject_id = ensure_entity(graph, entity_by_key, &claim.subject, &claim.event_id);
    let object_id = ensure_entity(graph, entity_by_key, &claim.object, &claim.event_id);
    insert_node(
        &mut graph.nodes,
        MemoryGraphNode {
            id: claim.id.clone(),
            kind: MemoryGraphNodeKind::Claim,
            label: format!("{} {} {}", claim.subject, claim.predicate, claim.object),
            depth: usize::MAX,
            evidence_ids: claim.source_episode_id.iter().cloned().collect(),
            source_event_ids: vec![claim.event_id.clone()],
            health_signal_count: 0,
            review_decision_count: 0,
        },
    );
    event_to_nodes
        .entry(claim.event_id.clone())
        .or_default()
        .push(claim.id.clone());
    insert_edge(
        &mut graph.edges,
        subject_id,
        claim.id.clone(),
        MemoryGraphEdgeKind::ClaimSubject,
        claim.predicate.clone(),
        Some(claim.confidence),
        claim.source_episode_id.clone(),
    );
    insert_edge(
        &mut graph.edges,
        claim.id.clone(),
        object_id,
        MemoryGraphEdgeKind::ClaimObject,
        claim.predicate.clone(),
        Some(claim.confidence),
        claim.source_episode_id.clone(),
    );
    if let Some(episode_id) = &claim.source_episode_id {
        insert_edge(
            &mut graph.edges,
            episode_id.clone(),
            claim.id.clone(),
            MemoryGraphEdgeKind::Supports,
            "supports".to_string(),
            Some(claim.confidence),
            Some(episode_id.clone()),
        );
    }
}

fn add_link(
    graph: &mut BuiltGraph,
    entity_by_key: &mut BTreeMap<String, String>,
    event_to_nodes: &mut BTreeMap<String, Vec<String>>,
    link: &Link,
) {
    let from_id = ensure_entity(graph, entity_by_key, &link.from, &link.event_id);
    let to_id = ensure_entity(graph, entity_by_key, &link.to, &link.event_id);
    insert_node(
        &mut graph.nodes,
        MemoryGraphNode {
            id: link.id.clone(),
            kind: MemoryGraphNodeKind::Link,
            label: format!("{} {} {}", link.from, link.relation, link.to),
            depth: usize::MAX,
            evidence_ids: link.source_episode_id.iter().cloned().collect(),
            source_event_ids: vec![link.event_id.clone()],
            health_signal_count: 0,
            review_decision_count: 0,
        },
    );
    event_to_nodes
        .entry(link.event_id.clone())
        .or_default()
        .push(link.id.clone());
    insert_edge(
        &mut graph.edges,
        from_id.clone(),
        link.id.clone(),
        MemoryGraphEdgeKind::LinkSource,
        link.relation.clone(),
        Some(link.confidence),
        link.source_episode_id.clone(),
    );
    insert_edge(
        &mut graph.edges,
        link.id.clone(),
        to_id.clone(),
        MemoryGraphEdgeKind::LinkTarget,
        link.relation.clone(),
        Some(link.confidence),
        link.source_episode_id.clone(),
    );
    insert_edge(
        &mut graph.edges,
        from_id,
        to_id,
        MemoryGraphEdgeKind::Relation,
        link.relation.clone(),
        Some(link.confidence),
        link.source_episode_id.clone(),
    );
    if let Some(episode_id) = &link.source_episode_id {
        insert_edge(
            &mut graph.edges,
            episode_id.clone(),
            link.id.clone(),
            MemoryGraphEdgeKind::Supports,
            "supports".to_string(),
            Some(link.confidence),
            Some(episode_id.clone()),
        );
    }
}

fn attach_health_and_review_counts(
    data: &MemoryData,
    nodes: &mut BTreeMap<String, MemoryGraphNode>,
    event_to_nodes: &BTreeMap<String, Vec<String>>,
) {
    let health = KnowledgeHealth::inspect(data);
    for signal in health.signals {
        let mut touched = BTreeSet::new();
        for evidence_id in &signal.evidence_ids {
            for node_id in nodes_for_evidence(event_to_nodes, evidence_id) {
                touched.insert(node_id);
            }
        }
        for node_id in touched {
            if let Some(node) = nodes.get_mut(&node_id) {
                node.health_signal_count += 1;
            }
        }
    }

    for decision in data
        .review_decisions
        .iter()
        .filter(|decision| decision.outcome == ReviewDecisionOutcome::Resolved)
    {
        let mut touched = BTreeSet::new();
        for evidence_id in &decision.evidence_ids {
            for node_id in nodes_for_evidence(event_to_nodes, evidence_id) {
                touched.insert(node_id);
            }
        }
        for node_id in touched {
            if let Some(node) = nodes.get_mut(&node_id) {
                node.review_decision_count += 1;
            }
        }
    }
}

fn ensure_entity(
    graph: &mut BuiltGraph,
    entity_by_key: &mut BTreeMap<String, String>,
    name: &str,
    event_id: &str,
) -> String {
    let key = entity_key(name);
    if let Some(id) = entity_by_key.get(&key) {
        return id.clone();
    }

    let id = entity_id(name);
    entity_by_key.insert(key, id.clone());
    insert_node(
        &mut graph.nodes,
        MemoryGraphNode {
            id: id.clone(),
            kind: MemoryGraphNodeKind::Entity,
            label: clean_name(name),
            depth: usize::MAX,
            evidence_ids: vec![event_id.to_string()],
            source_event_ids: vec![event_id.to_string()],
            health_signal_count: 0,
            review_decision_count: 0,
        },
    );
    id
}

fn insert_node(nodes: &mut BTreeMap<String, MemoryGraphNode>, node: MemoryGraphNode) {
    nodes.entry(node.id.clone()).or_insert(node);
}

fn insert_edge(
    edges: &mut BTreeMap<String, MemoryGraphEdge>,
    from: String,
    to: String,
    kind: MemoryGraphEdgeKind,
    label: String,
    confidence: Option<f32>,
    evidence_id: Option<String>,
) {
    if from == to {
        return;
    }
    let id = edge_id(&from, &to, &kind, &label);
    edges.entry(id.clone()).or_insert(MemoryGraphEdge {
        id,
        from,
        to,
        kind,
        label,
        confidence,
        evidence_id,
    });
}
