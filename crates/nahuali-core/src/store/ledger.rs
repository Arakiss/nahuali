#[derive(Clone, Debug, Deserialize)]
struct MemoryRecord {
    sequence: u64,
    envelope: EventEnvelope,
}

async fn read_records(path: &Path) -> Result<Vec<EventEnvelope>> {
    let db = open_database(path).await?;
    let mut response = db
        .query("SELECT sequence, envelope FROM memory_record ORDER BY sequence ASC")
        .await
        .map_err(|source| database_error(path, source))?;
    let rows: Vec<serde_json::Value> = response
        .take(0)
        .map_err(|source| database_error(path, source))?;
    let mut events = Vec::with_capacity(rows.len());

    for (index, row) in rows.into_iter().enumerate() {
        let record_number = index + 1;
        let record = decode_record(path, record_number, row)?;
        if record.sequence != record.envelope.sequence {
            return Err(NahualiError::InvalidRecordLedger {
                path: path.to_path_buf(),
                record: record_number,
                message: format!(
                    "record sequence {} does not match envelope sequence {}",
                    record.sequence, record.envelope.sequence
                ),
            });
        }
        validate_event(path, record_number, &events, &record.envelope)?;
        events.push(record.envelope);
    }

    Ok(events)
}

async fn write_record(path: &Path, event: &EventEnvelope) -> Result<()> {
    let db = open_database(path).await?;
    let envelope = serde_json::to_value(event).map_err(NahualiError::EncodeRecord)?;
    let record = serde_json::json!({
        "sequence": event.sequence,
        "envelope": envelope,
    });
    db.query("CREATE memory_record CONTENT $record")
        .bind(("record", record))
        .await
        .map_err(|source| database_error(path, source))?;
    Ok(())
}

async fn write_records(path: &Path, events: &[EventEnvelope]) -> Result<()> {
    let db = open_database(path).await?;
    for event in events {
        let envelope = serde_json::to_value(event).map_err(NahualiError::EncodeRecord)?;
        let record = serde_json::json!({
            "sequence": event.sequence,
            "envelope": envelope,
        });
        db.query("CREATE memory_record CONTENT $record")
            .bind(("record", record))
            .await
            .map_err(|source| database_error(path, source))?;
    }
    Ok(())
}

fn decode_record(path: &Path, record: usize, row: serde_json::Value) -> Result<MemoryRecord> {
    serde_json::from_value(row).map_err(|source| NahualiError::DecodeRecord {
        path: path.to_path_buf(),
        record,
        source,
    })
}

async fn open_database(path: &Path) -> Result<Surreal<Client>> {
    let endpoint = normalized_endpoint();
    let namespace =
        std::env::var("NAHUALI_DB_NAMESPACE").unwrap_or_else(|_| SURREAL_NAMESPACE.to_string());
    let database = database_name(path);
    let username = std::env::var("NAHUALI_DB_USERNAME").unwrap_or_else(|_| "root".to_string());
    let db_pass = std::env::var("NAHUALI_DB_PASSWORD").unwrap_or_else(|_| "root".to_string());
    let db = Surreal::new::<Ws>(&endpoint)
        .await
        .map_err(|source| database_error(path, source))?;
    db.signin(Root {
        username,
        password: db_pass,
    })
    .await
    .map_err(|source| database_error(path, source))?;
    db.use_ns(namespace)
        .use_db(database)
        .await
        .map_err(|source| database_error(path, source))?;
    db.query(MEMORY_RECORD_SCHEMA)
        .await
        .map_err(|source| database_error(path, source))?;
    db.query(GRAPH_PROJECTION_SCHEMA)
        .await
        .map_err(|source| database_error(path, source))?;
    Ok(db)
}

fn database_error(path: &Path, source: surrealdb::Error) -> NahualiError {
    NahualiError::Database {
        path: path.to_path_buf(),
        source: Box::new(source),
    }
}

fn validate_event(
    path: &Path,
    record: usize,
    previous: &[EventEnvelope],
    event: &EventEnvelope,
) -> Result<()> {
    let expected_sequence = previous
        .last()
        .map(|previous| previous.sequence + 1)
        .unwrap_or(1);

    if event.sequence != expected_sequence {
        return Err(NahualiError::InvalidRecordLedger {
            path: path.to_path_buf(),
            record,
            message: format!(
                "expected sequence {expected_sequence}, found {}",
                event.sequence
            ),
        });
    }

    if event.version != EVENT_ENVELOPE_VERSION {
        return Err(NahualiError::InvalidRecordLedger {
            path: path.to_path_buf(),
            record,
            message: format!(
                "unsupported record envelope version {}, supported version is {}",
                event.version, EVENT_ENVELOPE_VERSION
            ),
        });
    }

    if !event.validate_checksum() {
        return Err(NahualiError::InvalidRecordLedger {
            path: path.to_path_buf(),
            record,
            message: "checksum mismatch".to_string(),
        });
    }

    Ok(())
}

fn runtime() -> Result<Runtime> {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|source| NahualiError::Runtime { source })
}

fn block_on_database<F, T>(future: F) -> Result<T>
where
    F: Future<Output = Result<T>> + Send + 'static,
    T: Send + 'static,
{
    if tokio::runtime::Handle::try_current().is_ok() {
        return match thread::spawn(move || runtime()?.block_on(future)).join() {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        };
    }

    runtime()?.block_on(future)
}

fn clean_strings(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .filter_map(|value| {
            let value = value.trim().to_string();
            if value.is_empty() { None } else { Some(value) }
        })
        .collect()
}

fn clean_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_string();
        if value.is_empty() { None } else { Some(value) }
    })
}

fn clean_metadata(values: BTreeMap<String, String>) -> BTreeMap<String, String> {
    values
        .into_iter()
        .filter_map(|(key, value)| {
            let key = key.trim().to_string();
            let value = value.trim().to_string();
            if key.is_empty() || value.is_empty() {
                None
            } else {
                Some((key, value))
            }
        })
        .collect()
}

fn source_event_kind(kind: SourceKind) -> SourceRecordedKind {
    match kind {
        SourceKind::Document => SourceRecordedKind::Document,
        SourceKind::Conversation => SourceRecordedKind::Conversation,
        SourceKind::Transcript => SourceRecordedKind::Transcript,
        SourceKind::WebPage => SourceRecordedKind::WebPage,
        SourceKind::Note => SourceRecordedKind::Note,
        SourceKind::Other => SourceRecordedKind::Other,
    }
}

fn intention_event_kind(kind: IntentionKind) -> IntentionRecordedKind {
    match kind {
        IntentionKind::Task => IntentionRecordedKind::Task,
        IntentionKind::Goal => IntentionRecordedKind::Goal,
        IntentionKind::Reminder => IntentionRecordedKind::Reminder,
    }
}

fn intention_event_priority(priority: IntentionPriority) -> IntentionRecordedPriority {
    match priority {
        IntentionPriority::Low => IntentionRecordedPriority::Low,
        IntentionPriority::Medium => IntentionRecordedPriority::Medium,
        IntentionPriority::High => IntentionRecordedPriority::High,
        IntentionPriority::Critical => IntentionRecordedPriority::Critical,
    }
}

fn intention_event_status(status: IntentionStatus) -> IntentionRecordedStatus {
    match status {
        IntentionStatus::Active => IntentionRecordedStatus::Active,
        IntentionStatus::Completed => IntentionRecordedStatus::Completed,
        IntentionStatus::Abandoned => IntentionRecordedStatus::Abandoned,
        IntentionStatus::Blocked => IntentionRecordedStatus::Blocked,
        IntentionStatus::Deferred => IntentionRecordedStatus::Deferred,
    }
}

fn make_id(prefix: &str) -> String {
    make_id_at(prefix, now_ms())
}

fn make_id_at(prefix: &str, timestamp_ms: u64) -> String {
    let counter = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}_{timestamp_ms}_{counter}")
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}
