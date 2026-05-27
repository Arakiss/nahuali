fn open_error_check(path: &Path, expected: &str) -> CheckResult {
    match MemoryEngine::open(path) {
        Ok(_) => CheckResult {
            name: "expected_open_error".to_string(),
            passed: false,
            detail: "database opened successfully".to_string(),
        },
        Err(error) => {
            let detail = error.to_string();
            CheckResult {
                name: "expected_open_error".to_string(),
                passed: detail.contains(expected),
                detail,
            }
        }
    }
}

fn write_raw_record_ledger(path: &Path, events: &[EventEnvelope]) -> anyhow::Result<()> {
    runtime()?
        .block_on(write_raw_records(path, events))
        .with_context(|| format!("failed to write raw records to {}", path.display()))
}

async fn write_raw_records(path: &Path, events: &[EventEnvelope]) -> anyhow::Result<()> {
    let db = open_database(path).await?;
    db.query("DELETE memory_record")
        .await
        .with_context(|| format!("failed to clear raw records in {}", path.display()))?;

    for event in events {
        let envelope = serde_json::to_value(event)?;
        let record = serde_json::json!({
            "sequence": event.sequence,
            "envelope": envelope,
        });
        db.query("CREATE memory_record CONTENT $record")
            .bind(("record", record))
            .await
            .with_context(|| format!("failed to write raw record {}", event.sequence))?;
    }

    Ok(())
}

async fn open_database(path: &Path) -> anyhow::Result<Surreal<Client>> {
    let endpoint = normalized_endpoint();
    let namespace =
        std::env::var("NAHUALI_DB_NAMESPACE").unwrap_or_else(|_| SURREAL_NAMESPACE.to_string());
    let database = database_name(path);
    let username = std::env::var("NAHUALI_DB_USERNAME").unwrap_or_else(|_| "root".to_string());
    let db_pass = std::env::var("NAHUALI_DB_PASSWORD").unwrap_or_else(|_| "root".to_string());
    let db = Surreal::new::<Ws>(&endpoint)
        .await
        .with_context(|| format!("failed to connect to SurrealDB at {endpoint}"))?;
    db.signin(Root {
        username,
        password: db_pass,
    })
    .await
    .context("failed to authenticate to SurrealDB")?;
    db.use_ns(namespace)
        .use_db(database)
        .await
        .context("failed to select SurrealDB namespace/database")?;
    db.query(MEMORY_RECORD_SCHEMA)
        .await
        .context("failed to initialize raw record schema")?;
    Ok(db)
}

fn normalized_endpoint() -> String {
    let endpoint =
        std::env::var("NAHUALI_DB_URL").unwrap_or_else(|_| DEFAULT_SURREAL_ENDPOINT.to_string());
    endpoint
        .trim()
        .trim_start_matches("ws://")
        .trim_start_matches("wss://")
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .to_string()
}

fn database_name(path: &Path) -> String {
    let explicit = std::env::var("NAHUALI_DB_DATABASE").ok();
    let raw = explicit
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .or_else(|| path.to_str().map(str::to_string))
        .unwrap_or_else(|| SURREAL_DATABASE.to_string());
    let mut name = raw
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    if name.is_empty() {
        name = SURREAL_DATABASE.to_string();
    }
    if name
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_digit())
    {
        name.insert_str(0, "tenant_");
    }
    name
}

fn runtime() -> anyhow::Result<Runtime> {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to create regression runtime")
}
