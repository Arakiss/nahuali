fn review_titles(store: &Path) -> Vec<String> {
    let output = run_ok(store, &["review", "--limit", "50", "--json"]);
    let review: Value = serde_json::from_str(&output).expect("review output is JSON");
    review["items"]
        .as_array()
        .expect("review items are an array")
        .iter()
        .map(|item| item["title"].as_str().unwrap_or_default().to_string())
        .collect()
}

fn entity_names(store: &Path) -> Vec<String> {
    let output = run_ok(store, &["data", "--json"]);
    let data: Value = serde_json::from_str(&output).expect("data output is JSON");
    data["entities"]
        .as_array()
        .expect("entities are an array")
        .iter()
        .map(|entity| entity["name"].as_str().unwrap_or_default().to_string())
        .collect()
}

/// Mirrors the logged friction: free-text claim values became entities, and
/// each one was reported as an isolated entity, flooding `review` and capping
/// authority with otherwise clean data.
#[test]
fn claim_literal_values_are_not_isolated_entities() {
    let store = temp_database("claim-literal-values");

    run_ok(
        &store,
        &[
            "remember",
            "Petru vive en Valencia con 2 podencos y 5 gatos; trabaja de enfermero",
            "--mention",
            "Petru",
        ],
    );
    let sourced = [
        ("mascotas", "2 podencos y 5 gatos"),
        ("ciudad", "Valencia"),
        ("profesion", "enfermero"),
        ("turno", "noches alternas"),
        ("coche", "furgoneta blanca"),
        ("idioma", "castellano y rumano"),
        ("deporte", "senderismo los domingos"),
        ("comida favorita", "sarmale"),
        ("alergia", "polen de olivo"),
        ("hora de despertar", "6:30"),
    ];
    for (predicate, value) in sourced {
        run_ok(
            &store,
            &[
                "claim",
                "petru",
                predicate,
                value,
                "--source-last",
                "--confidence",
                "0.95",
            ],
        );
    }
    run_ok(&store, &["claim", "petru", "edad", "41", "--confidence", "0.9"]);
    run_ok(&store, &["claim", "petru", "cumpleanos", "3 de marzo"]);

    let titles = review_titles(&store);
    assert!(
        !titles.iter().any(|title| title == "Isolated entity"),
        "literal claim values must not be isolated entities: {titles:?}"
    );
    assert_eq!(
        titles
            .iter()
            .filter(|title| title.as_str() == "Memory without source evidence")
            .count(),
        2,
        "the two unsourced claims are still reported once each: {titles:?}"
    );

    let inspect_output = run_ok(&store, &["inspect", "--json"]);
    let health: Value = serde_json::from_str(&inspect_output).expect("inspect output is JSON");
    assert_eq!(health["isolated_entity_count"], 0);
    assert_eq!(health["unsupported_fact_count"], 2);
    assert_eq!(health["fact_count"], 12);

    let names = entity_names(&store);
    assert!(names.iter().any(|name| name.eq_ignore_ascii_case("petru")));
    for literal in ["2 podencos y 5 gatos", "41", "3 de marzo", "6:30"] {
        assert!(
            !names.iter().any(|name| name == literal),
            "'{literal}' is a claim value, not an entity: {names:?}"
        );
    }

    let recall_output = run_ok(&store, &["recall", "podencos gatos", "--json"]);
    let recall: Value = serde_json::from_str(&recall_output).expect("recall output is JSON");
    assert!(
        recall
            .as_array()
            .unwrap()
            .iter()
            .all(|result| result["kind"] != "entity"),
        "the literal is recalled through its claim, not as an entity: {recall}"
    );
}

/// Real isolation must still be reported, and a claim whose value names a known
/// entity still connects it.
#[test]
fn real_isolated_entities_are_still_reported() {
    let store = temp_database("real-isolated-entities");

    run_ok(
        &store,
        &[
            "remember",
            "Petru walks the dogs with Dolores at the river park",
            "--mention",
            "Petru",
            "--mention",
            "Dolores",
        ],
    );
    run_ok(
        &store,
        &["claim", "Petru", "mascotas", "2 podencos", "--source-last"],
    );

    let inspect_output = run_ok(&store, &["inspect", "--json"]);
    let health: Value = serde_json::from_str(&inspect_output).expect("inspect output is JSON");
    assert_eq!(health["isolated_entity_count"], 1);
    assert!(
        health["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning
                .as_str()
                .unwrap_or_default()
                .starts_with("Entity 'dolores' is not connected")),
        "{health}"
    );
    assert!(review_titles(&store).iter().any(|title| title == "Isolated entity"));

    run_ok(
        &store,
        &["claim", "Petru", "walks with", "Dolores", "--source-last"],
    );
    let inspect_output = run_ok(&store, &["inspect", "--json"]);
    let health: Value = serde_json::from_str(&inspect_output).expect("inspect output is JSON");
    assert_eq!(
        health["isolated_entity_count"], 0,
        "a sourced claim whose value names a known entity connects it: {health}"
    );
}
