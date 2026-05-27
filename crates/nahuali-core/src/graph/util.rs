fn seed_nodes(nodes: &BTreeMap<String, MemoryGraphNode>, seed: &str) -> Vec<String> {
    let normalized = normalize(seed);
    let exact = nodes
        .values()
        .filter(|node| node.id == seed || normalize(&node.label) == normalized)
        .map(|node| node.id.clone())
        .collect::<Vec<_>>();
    if !exact.is_empty() {
        return exact;
    }

    nodes
        .values()
        .filter(|node| normalize(&node.label).contains(&normalized))
        .map(|node| node.id.clone())
        .collect()
}

fn traverse(
    edges: &BTreeMap<String, MemoryGraphEdge>,
    seed_ids: &[String],
    max_depth: usize,
    limit: usize,
) -> BTreeMap<String, usize> {
    let mut adjacency = BTreeMap::<String, BTreeSet<String>>::new();
    for edge in edges.values() {
        adjacency
            .entry(edge.from.clone())
            .or_default()
            .insert(edge.to.clone());
        adjacency
            .entry(edge.to.clone())
            .or_default()
            .insert(edge.from.clone());
    }

    let mut depths = BTreeMap::<String, usize>::new();
    let mut queue = VecDeque::new();
    for seed_id in seed_ids {
        if depths.len() >= limit {
            break;
        }
        if depths.insert(seed_id.clone(), 0).is_none() {
            queue.push_back(seed_id.clone());
        }
    }

    while let Some(node_id) = queue.pop_front() {
        if depths.len() >= limit {
            break;
        }
        let depth = depths.get(&node_id).copied().unwrap_or_default();
        if depth >= max_depth {
            continue;
        }
        for neighbor in adjacency.get(&node_id).into_iter().flatten() {
            if depths.contains_key(neighbor) {
                continue;
            }
            depths.insert(neighbor.clone(), depth + 1);
            queue.push_back(neighbor.clone());
            if depths.len() >= limit {
                break;
            }
        }
    }

    depths
}

fn summarize(nodes: &[MemoryGraphNode], edges: &[MemoryGraphEdge]) -> MemoryGraphSummary {
    MemoryGraphSummary {
        node_count: nodes.len(),
        edge_count: edges.len(),
        entity_count: nodes
            .iter()
            .filter(|node| node.kind == MemoryGraphNodeKind::Entity)
            .count(),
        memory_count: nodes
            .iter()
            .filter(|node| node.kind != MemoryGraphNodeKind::Entity)
            .count(),
        support_edge_count: edges
            .iter()
            .filter(|edge| edge.kind == MemoryGraphEdgeKind::Supports)
            .count(),
        relation_edge_count: edges
            .iter()
            .filter(|edge| edge.kind == MemoryGraphEdgeKind::Relation)
            .count(),
        health_signal_count: nodes.iter().map(|node| node.health_signal_count).sum(),
        review_decision_count: nodes.iter().map(|node| node.review_decision_count).sum(),
    }
}

fn nodes_for_evidence(
    event_to_nodes: &BTreeMap<String, Vec<String>>,
    evidence_id: &str,
) -> Vec<String> {
    let mut nodes = event_to_nodes.get(evidence_id).cloned().unwrap_or_default();
    nodes.push(evidence_id.to_string());
    nodes
}

fn projected_claims(data: &MemoryData) -> &[Claim] {
    if data.claims.is_empty() {
        &data.facts
    } else {
        &data.claims
    }
}

fn projected_links(data: &MemoryData) -> &[Link] {
    if data.links.is_empty() {
        &data.relations
    } else {
        &data.links
    }
}

fn node_kind_rank(kind: &MemoryGraphNodeKind) -> u8 {
    match kind {
        MemoryGraphNodeKind::Entity => 0,
        MemoryGraphNodeKind::Episode => 1,
        MemoryGraphNodeKind::Claim => 2,
        MemoryGraphNodeKind::Link => 3,
        MemoryGraphNodeKind::Procedure => 4,
        MemoryGraphNodeKind::Intention => 5,
        MemoryGraphNodeKind::ReviewDecision => 6,
    }
}

fn edge_kind_rank(kind: &MemoryGraphEdgeKind) -> u8 {
    match kind {
        MemoryGraphEdgeKind::Relation => 0,
        MemoryGraphEdgeKind::Mentions => 1,
        MemoryGraphEdgeKind::Supports => 2,
        MemoryGraphEdgeKind::ClaimSubject => 3,
        MemoryGraphEdgeKind::ClaimObject => 4,
        MemoryGraphEdgeKind::LinkSource => 5,
        MemoryGraphEdgeKind::LinkTarget => 6,
        MemoryGraphEdgeKind::Reviews => 7,
    }
}

fn edge_id(from: &str, to: &str, kind: &MemoryGraphEdgeKind, label: &str) -> String {
    format!(
        "edge_{:08x}",
        fnv1a32(format!("{from}\0{to}\0{kind:?}\0{label}").as_bytes())
    )
}

fn entity_id(name: &str) -> String {
    let key = entity_key(name);
    let slug = key
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    let readable = if slug.is_empty() { "entity" } else { &slug };
    format!("entity_{readable}_{:08x}", fnv1a32(key.as_bytes()))
}

fn entity_key(name: &str) -> String {
    clean_name(name).to_ascii_lowercase()
}

fn clean_name(name: &str) -> String {
    name.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize(input: &str) -> String {
    clean_name(input).to_ascii_lowercase()
}

fn truncate_label(input: &str) -> String {
    const MAX_LABEL_CHARS: usize = 96;
    let cleaned = clean_name(input);
    if cleaned.chars().count() <= MAX_LABEL_CHARS {
        return cleaned;
    }
    cleaned
        .chars()
        .take(MAX_LABEL_CHARS.saturating_sub(1))
        .collect::<String>()
}

fn fnv1a32(bytes: &[u8]) -> u32 {
    const FNV_OFFSET: u32 = 0x811c9dc5;
    const FNV_PRIME: u32 = 0x01000193;

    bytes.iter().fold(FNV_OFFSET, |hash, byte| {
        (hash ^ u32::from(*byte)).wrapping_mul(FNV_PRIME)
    })
}
