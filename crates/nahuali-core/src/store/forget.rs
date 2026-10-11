impl MemoryEngine {
    /// Forget memory by identifier, with an auditable trace.
    ///
    /// Every ledger record holding the targets, or memory derived from them, is
    /// redacted in place and one `memory_forgotten` event records who asked,
    /// when, why, and which records were erased, all in a single transaction.
    /// The graph projection is then rebuilt, so recall, inspection, and graph
    /// reads stop surfacing the erased memory. Backups, snapshots, exports,
    /// receipts, and the Qdrant semantic index are outside this transaction;
    /// see the forgetting limits in the trust model documentation.
    pub fn forget(&mut self, options: &ForgetOptions) -> Result<ForgetReport> {
        const MAX_SEQUENCE_CONFLICT_RETRIES: u32 = 16;
        let mut attempt = 0;
        self.refresh()?;
        loop {
            let plan = forget::plan_forget(&self.events, options)?;
            let mut report = plan.preview(&self.events);
            if options.dry_run {
                return Ok(report);
            }

            let timestamp_ms = now_ms();
            let forget_id = make_id_at("forget", timestamp_ms);
            let payload = MemoryEvent::MemoryForgotten(plan.event(forget_id.clone(), &self.events));
            let stubs: Vec<EventEnvelope> = plan
                .redacted_indexes
                .iter()
                .map(|&index| self.events[index].redacted(&forget_id))
                .collect();
            #[cfg(not(feature = "tamper-evidence"))]
            let envelope = EventEnvelope::new(self.next_sequence, timestamp_ms, payload);
            #[cfg(feature = "tamper-evidence")]
            let envelope = {
                let previous_chain_hash = self.events.last().map(EventEnvelope::chain_hash);
                EventEnvelope::with_chain(
                    self.next_sequence,
                    timestamp_ms,
                    payload,
                    previous_chain_hash.as_deref(),
                )
            };

            let write_path = self.path.clone();
            let write_stubs = stubs.clone();
            let write_envelope = envelope.clone();
            match block_on_database(async move {
                write_forget(&write_path, &write_stubs, &write_envelope).await
            }) {
                Ok(()) => {}
                Err(error)
                    if is_record_sequence_conflict(&error)
                        && attempt < MAX_SEQUENCE_CONFLICT_RETRIES =>
                {
                    attempt += 1;
                    append_retry_backoff(attempt);
                    self.refresh()?;
                    continue;
                }
                Err(error) => return Err(error),
            }

            let read_path = self.path.clone();
            let events = block_on_database(async move { read_records(&read_path).await })?;
            self.next_sequence = events.last().map(|event| event.sequence + 1).unwrap_or(1);
            self.data = projection::project(&events);
            self.events = events;

            let graph_path = self.path.clone();
            if let Err(source) =
                block_on_database(async move { rebuild_graph_projection(&graph_path).await })
            {
                return Err(committed_projection_error(
                    std::slice::from_ref(&envelope),
                    source,
                ));
            }

            report.id = Some(forget_id);
            report.event_id = Some(envelope.id.clone());
            report.dry_run = false;
            report.forgotten_at_ms = Some(timestamp_ms);
            report.graph_projection_rebuilt = true;
            return Ok(report);
        }
    }
}

/// Redact `stubs` in place and append the forget `event` in one transaction.
///
/// Each update is guarded by the original envelope id, and the append claims the
/// next sequence under the unique index, so a concurrent writer aborts the whole
/// transaction instead of leaving a half-applied erasure.
async fn write_forget(path: &Path, stubs: &[EventEnvelope], event: &EventEnvelope) -> Result<()> {
    let db = open_database(path).await?;
    let mut statement = String::from("BEGIN TRANSACTION;\n");
    let mut bindings = Vec::with_capacity(stubs.len() * 3 + 1);
    for (index, stub) in stubs.iter().enumerate() {
        statement.push_str(&format!(
            "IF (SELECT VALUE envelope.id FROM memory_record WHERE sequence = $sequence_{index}) != [$event_id_{index}] {{ THROW \"record {} changed while it was being forgotten\"; }};\n\
             UPDATE memory_record SET envelope = $stub_{index} WHERE sequence = $sequence_{index};\n",
            stub.sequence
        ));
        bindings.push((format!("sequence_{index}"), serde_json::json!(stub.sequence)));
        bindings.push((format!("event_id_{index}"), serde_json::json!(stub.id)));
        bindings.push((
            format!("stub_{index}"),
            serde_json::to_value(stub).map_err(NahualiError::EncodeRecord)?,
        ));
    }
    statement.push_str("CREATE memory_record CONTENT $record;\nCOMMIT TRANSACTION;");
    bindings.push((
        "record".to_string(),
        serde_json::json!({
            "sequence": event.sequence,
            "envelope": serde_json::to_value(event).map_err(NahualiError::EncodeRecord)?,
        }),
    ));
    db.query_with_retry(path, statement, bindings).await?;
    Ok(())
}
