#[derive(Clone, Debug, Default)]
struct EpisodeSourceContext {
    source_id: Option<String>,
    source_position: Option<u32>,
    source_role: Option<String>,
    scope: Option<MemoryScope>,
}

#[derive(Clone, Debug)]
struct EvidenceContext {
    source_episode_id: Option<String>,
    confidence: f32,
    scope: Option<MemoryScope>,
}

impl EvidenceContext {
    fn new(source_episode_id: Option<String>, confidence: f32, scope: Option<MemoryScope>) -> Self {
        Self {
            source_episode_id,
            confidence,
            scope,
        }
    }
}

impl MemoryEngine {
    /// Open an existing SurrealDB store or initialize an empty one at `path`.
    ///
    /// Existing records are validated for monotonic sequence order and checksum
    /// integrity before projection.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let read_path = path.clone();
        let events = block_on_database(async move { read_records(&read_path).await })?;
        let next_sequence = events.last().map(|event| event.sequence + 1).unwrap_or(1);
        let data = projection::project(&events);
        let graph_path = path.clone();
        let graph_data = data.clone();
        let graph_events = events.clone();
        block_on_database(async move {
            rebuild_graph_projection(&graph_path, &graph_data, &graph_events).await
        })?;

        Ok(Self {
            path,
            events,
            data,
            next_sequence,
        })
    }

    /// Validate a SurrealDB database name without mutating or projecting invalid records.
    pub fn validate_store(path: impl AsRef<Path>) -> Result<RecordLedgerValidation> {
        validate_record_ledger(path)
    }

    /// Return the current deterministic projection.
    pub fn data(&self) -> &MemoryData {
        &self.data
    }

    /// Return the validated record envelopes backing the projection.
    pub fn events(&self) -> &[EventEnvelope] {
        &self.events
    }

    /// Record an observed episode as ground-truth memory.
    ///
    /// Empty content is rejected after trimming.
    pub fn remember(&mut self, content: impl Into<String>, tags: Vec<String>) -> Result<Episode> {
        self.remember_with_mentions(content, tags, Vec::new())
    }

    /// Record an observed episode in an explicit memory scope.
    pub fn remember_scoped(
        &mut self,
        content: impl Into<String>,
        tags: Vec<String>,
        scope: MemoryScope,
    ) -> Result<Episode> {
        self.remember_with_mentions_scoped(content, tags, Vec::new(), scope)
    }

    /// Record an observed episode with explicit entity mentions.
    ///
    /// Empty content is rejected after trimming. Empty mentions are ignored
    /// after trimming.
    pub fn remember_with_mentions(
        &mut self,
        content: impl Into<String>,
        tags: Vec<String>,
        mentions: Vec<String>,
    ) -> Result<Episode> {
        self.remember_episode_with_source(content, tags, mentions, EpisodeSourceContext::default())
    }

    /// Record an observed episode with explicit entity mentions and scope.
    pub fn remember_with_mentions_scoped(
        &mut self,
        content: impl Into<String>,
        tags: Vec<String>,
        mentions: Vec<String>,
        scope: MemoryScope,
    ) -> Result<Episode> {
        self.remember_episode_with_source(
            content,
            tags,
            mentions,
            EpisodeSourceContext {
                scope: Some(scope),
                ..EpisodeSourceContext::default()
            },
        )
    }

    /// Register source material for future provenance-aware episode ingestion.
    ///
    /// Empty checksums are rejected after trimming. Empty titles, URIs, metadata
    /// keys, and metadata values are omitted.
    pub fn record_source(
        &mut self,
        kind: SourceKind,
        title: Option<String>,
        uri: Option<String>,
        content_checksum: impl Into<String>,
        byte_len: u64,
        metadata: BTreeMap<String, String>,
    ) -> Result<SourceDocument> {
        self.record_source_with_options(SourceRecordOptions {
            kind,
            title,
            uri,
            content_checksum: content_checksum.into(),
            byte_len,
            metadata,
            scope: None,
        })
    }

    /// Register source material with structured options.
    pub fn record_source_with_options(
        &mut self,
        options: SourceRecordOptions,
    ) -> Result<SourceDocument> {
        let content_checksum = options.content_checksum.trim().to_string();
        if content_checksum.is_empty() {
            return Err(NahualiError::EmptyContent);
        }

        let source_id = make_id("source");
        let payload = MemoryEvent::SourceRecorded(SourceRecorded {
            id: source_id.clone(),
            kind: source_event_kind(options.kind),
            title: clean_optional_string(options.title),
            uri: clean_optional_string(options.uri),
            content_checksum,
            byte_len: options.byte_len,
            metadata: clean_metadata(options.metadata),
            scope: options.scope,
        });
        self.append(payload)?;

        Ok(self
            .data
            .sources
            .iter()
            .find(|source| source.id == source_id)
            .expect("appended source must project")
            .clone())
    }

    /// Record an observed episode from a registered source.
    ///
    /// Unknown source identifiers are rejected before writing. Empty source
    /// roles are omitted after trimming.
    pub fn remember_source_episode(
        &mut self,
        content: impl Into<String>,
        tags: Vec<String>,
        mentions: Vec<String>,
        source_id: impl Into<String>,
        source_position: Option<u32>,
        source_role: Option<String>,
    ) -> Result<Episode> {
        self.remember_source_episode_with_options(SourceEpisodeOptions {
            content: content.into(),
            tags,
            mentions,
            source_id: source_id.into(),
            source_position,
            source_role,
            scope: None,
        })
    }

    /// Record an observed source episode with structured options.
    pub fn remember_source_episode_with_options(
        &mut self,
        options: SourceEpisodeOptions,
    ) -> Result<Episode> {
        let source_id = options.source_id.trim().to_string();
        if source_id.is_empty() {
            return Err(NahualiError::EmptyContent);
        }
        let source_scope = self
            .data
            .sources
            .iter()
            .find(|source| source.id == source_id)
            .map(|source| source.scope.clone());
        let Some(source_scope) = source_scope else {
            return Err(NahualiError::UnknownSource { id: source_id });
        };

        self.remember_episode_with_source(
            options.content,
            options.tags,
            options.mentions,
            EpisodeSourceContext {
                source_id: Some(source_id),
                source_position: options.source_position,
                source_role: clean_optional_string(options.source_role),
                scope: options.scope.or(source_scope),
            },
        )
    }

    fn remember_episode_with_source(
        &mut self,
        content: impl Into<String>,
        tags: Vec<String>,
        mentions: Vec<String>,
        source: EpisodeSourceContext,
    ) -> Result<Episode> {
        let content = content.into().trim().to_string();
        if content.is_empty() {
            return Err(NahualiError::EmptyContent);
        }

        let episode_id = make_id("episode");
        let payload = MemoryEvent::EpisodeRecorded(EpisodeRecorded {
            id: episode_id.clone(),
            content,
            tags,
            mentions: clean_strings(mentions),
            source_id: source.source_id,
            source_position: source.source_position,
            source_role: source.source_role,
            scope: source.scope,
        });
        self.append(payload)?;

        Ok(self
            .data
            .episodes
            .iter()
            .find(|episode| episode.id == episode_id)
            .expect("appended episode must project")
            .clone())
    }

    /// Assert a canonical derived claim, optionally linked to a source episode.
    ///
    /// Empty fields are rejected after trimming. Confidence is clamped to the
    /// `0.0..=1.0` range.
    pub fn add_claim(
        &mut self,
        subject: impl Into<String>,
        predicate: impl Into<String>,
        object: impl Into<String>,
        source_episode_id: Option<String>,
        confidence: f32,
    ) -> Result<Claim> {
        self.assert_claim(
            "claim",
            subject,
            predicate,
            object,
            EvidenceContext::new(source_episode_id, confidence, None),
        )
    }

    /// Assert a canonical derived claim in an explicit memory scope.
    pub fn add_claim_scoped(
        &mut self,
        subject: impl Into<String>,
        predicate: impl Into<String>,
        object: impl Into<String>,
        source_episode_id: Option<String>,
        confidence: f32,
        scope: MemoryScope,
    ) -> Result<Claim> {
        self.assert_claim(
            "claim",
            subject,
            predicate,
            object,
            EvidenceContext::new(source_episode_id, confidence, Some(scope)),
        )
    }

    /// Assert a compatibility fact, optionally linked to a source episode.
    ///
    /// New public code should prefer [`Self::add_claim`]. Empty fields are
    /// rejected after trimming. Confidence is clamped to the `0.0..=1.0` range.
    pub fn add_fact(
        &mut self,
        subject: impl Into<String>,
        predicate: impl Into<String>,
        object: impl Into<String>,
        source_episode_id: Option<String>,
        confidence: f32,
    ) -> Result<Fact> {
        self.assert_claim(
            "fact",
            subject,
            predicate,
            object,
            EvidenceContext::new(source_episode_id, confidence, None),
        )
    }

    /// Assert a compatibility fact in an explicit memory scope.
    pub fn add_fact_scoped(
        &mut self,
        subject: impl Into<String>,
        predicate: impl Into<String>,
        object: impl Into<String>,
        source_episode_id: Option<String>,
        confidence: f32,
        scope: MemoryScope,
    ) -> Result<Fact> {
        self.assert_claim(
            "fact",
            subject,
            predicate,
            object,
            EvidenceContext::new(source_episode_id, confidence, Some(scope)),
        )
    }

    fn assert_claim(
        &mut self,
        id_prefix: &str,
        subject: impl Into<String>,
        predicate: impl Into<String>,
        object: impl Into<String>,
        context: EvidenceContext,
    ) -> Result<Claim> {
        let subject = subject.into().trim().to_string();
        let predicate = predicate.into().trim().to_string();
        let object = object.into().trim().to_string();
        if subject.is_empty() || predicate.is_empty() || object.is_empty() {
            return Err(NahualiError::EmptyContent);
        }

        let claim_id = make_id(id_prefix);
        let scope = context
            .scope
            .or_else(|| self.scope_for_episode(context.source_episode_id.as_deref()));
        let payload = MemoryEvent::FactAsserted(FactAsserted {
            id: claim_id.clone(),
            subject,
            predicate,
            object,
            source_episode_id: context.source_episode_id,
            confidence: context.confidence.clamp(0.0, 1.0),
            scope,
        });
        self.append(payload)?;

        Ok(self
            .data
            .facts
            .iter()
            .find(|claim| claim.id == claim_id)
            .expect("appended claim must project")
            .clone())
    }

    /// Record a canonical derived link, optionally linked to a source episode.
    ///
    /// Empty fields are rejected after trimming. Confidence is clamped to the
    /// `0.0..=1.0` range.
    pub fn add_link(
        &mut self,
        from: impl Into<String>,
        relation: impl Into<String>,
        to: impl Into<String>,
        source_episode_id: Option<String>,
        confidence: f32,
    ) -> Result<Link> {
        self.assert_link(
            "link",
            from,
            relation,
            to,
            EvidenceContext::new(source_episode_id, confidence, None),
        )
    }

    /// Record a canonical derived link in an explicit memory scope.
    pub fn add_link_scoped(
        &mut self,
        from: impl Into<String>,
        relation: impl Into<String>,
        to: impl Into<String>,
        source_episode_id: Option<String>,
        confidence: f32,
        scope: MemoryScope,
    ) -> Result<Link> {
        self.assert_link(
            "link",
            from,
            relation,
            to,
            EvidenceContext::new(source_episode_id, confidence, Some(scope)),
        )
    }

    /// Record a compatibility relation, optionally linked to a source episode.
    ///
    /// New public code should prefer [`Self::add_link`]. Empty fields are
    /// rejected after trimming. Confidence is clamped to the `0.0..=1.0` range.
    pub fn relate(
        &mut self,
        from: impl Into<String>,
        relation: impl Into<String>,
        to: impl Into<String>,
        source_episode_id: Option<String>,
        confidence: f32,
    ) -> Result<Relation> {
        self.assert_link(
            "relation",
            from,
            relation,
            to,
            EvidenceContext::new(source_episode_id, confidence, None),
        )
    }

    /// Record a compatibility relation in an explicit memory scope.
    pub fn relate_scoped(
        &mut self,
        from: impl Into<String>,
        relation: impl Into<String>,
        to: impl Into<String>,
        source_episode_id: Option<String>,
        confidence: f32,
        scope: MemoryScope,
    ) -> Result<Relation> {
        self.assert_link(
            "relation",
            from,
            relation,
            to,
            EvidenceContext::new(source_episode_id, confidence, Some(scope)),
        )
    }

    fn assert_link(
        &mut self,
        id_prefix: &str,
        from: impl Into<String>,
        relation: impl Into<String>,
        to: impl Into<String>,
        context: EvidenceContext,
    ) -> Result<Link> {
        let from = from.into().trim().to_string();
        let relation = relation.into().trim().to_string();
        let to = to.into().trim().to_string();
        if from.is_empty() || relation.is_empty() || to.is_empty() {
            return Err(NahualiError::EmptyContent);
        }

        let link_id = make_id(id_prefix);
        let scope = context
            .scope
            .or_else(|| self.scope_for_episode(context.source_episode_id.as_deref()));
        let payload = MemoryEvent::RelationRecorded(RelationRecorded {
            id: link_id.clone(),
            from,
            relation,
            to,
            source_episode_id: context.source_episode_id,
            confidence: context.confidence.clamp(0.0, 1.0),
            scope,
        });
        self.append(payload)?;

        Ok(self
            .data
            .relations
            .iter()
            .find(|link| link.id == link_id)
            .expect("appended link must project")
            .clone())
    }

    /// Record a reusable procedure.
    ///
    /// Empty names or bodies are rejected after trimming. Confidence is clamped
    /// to the `0.0..=1.0` range.
    pub fn add_procedure(
        &mut self,
        name: impl Into<String>,
        body: impl Into<String>,
        source_episode_id: Option<String>,
        confidence: f32,
    ) -> Result<Procedure> {
        self.record_procedure(
            "procedure",
            ProcedureRecordedKind::Procedure,
            name,
            body,
            EvidenceContext::new(source_episode_id, confidence, None),
        )
    }

    /// Record a reusable procedure in an explicit memory scope.
    pub fn add_procedure_scoped(
        &mut self,
        name: impl Into<String>,
        body: impl Into<String>,
        source_episode_id: Option<String>,
        confidence: f32,
        scope: MemoryScope,
    ) -> Result<Procedure> {
        self.record_procedure(
            "procedure",
            ProcedureRecordedKind::Procedure,
            name,
            body,
            EvidenceContext::new(source_episode_id, confidence, Some(scope)),
        )
    }

    /// Record a reusable behavioral preference.
    ///
    /// Empty names or bodies are rejected after trimming. Confidence is clamped
    /// to the `0.0..=1.0` range.
    pub fn add_preference(
        &mut self,
        name: impl Into<String>,
        body: impl Into<String>,
        source_episode_id: Option<String>,
        confidence: f32,
    ) -> Result<Procedure> {
        self.record_procedure(
            "preference",
            ProcedureRecordedKind::Preference,
            name,
            body,
            EvidenceContext::new(source_episode_id, confidence, None),
        )
    }

    /// Record a reusable behavioral preference in an explicit memory scope.
    pub fn add_preference_scoped(
        &mut self,
        name: impl Into<String>,
        body: impl Into<String>,
        source_episode_id: Option<String>,
        confidence: f32,
        scope: MemoryScope,
    ) -> Result<Procedure> {
        self.record_procedure(
            "preference",
            ProcedureRecordedKind::Preference,
            name,
            body,
            EvidenceContext::new(source_episode_id, confidence, Some(scope)),
        )
    }

    fn record_procedure(
        &mut self,
        id_prefix: &str,
        kind: ProcedureRecordedKind,
        name: impl Into<String>,
        body: impl Into<String>,
        context: EvidenceContext,
    ) -> Result<Procedure> {
        let name = name.into().trim().to_string();
        let body = body.into().trim().to_string();
        if name.is_empty() || body.is_empty() {
            return Err(NahualiError::EmptyContent);
        }

        let procedure_id = make_id(id_prefix);
        let scope = context
            .scope
            .or_else(|| self.scope_for_episode(context.source_episode_id.as_deref()));
        let payload = MemoryEvent::ProcedureRecorded(ProcedureRecorded {
            id: procedure_id.clone(),
            kind,
            name,
            body,
            source_episode_id: context.source_episode_id,
            confidence: context.confidence.clamp(0.0, 1.0),
            scope,
        });
        self.append(payload)?;

        Ok(self
            .data
            .procedures
            .iter()
            .find(|procedure| procedure.id == procedure_id)
            .expect("appended procedure must project")
            .clone())
    }

    /// Record future work, a goal, reminder, or commitment.
    ///
    /// Empty descriptions are rejected after trimming.
    pub fn add_intention(
        &mut self,
        description: impl Into<String>,
        kind: IntentionKind,
        priority: IntentionPriority,
        source_episode_id: Option<String>,
    ) -> Result<Intention> {
        self.add_intention_with_scope(description, kind, priority, source_episode_id, None)
    }

    /// Record future work, a goal, reminder, or commitment in a scope.
    pub fn add_intention_scoped(
        &mut self,
        description: impl Into<String>,
        kind: IntentionKind,
        priority: IntentionPriority,
        source_episode_id: Option<String>,
        scope: MemoryScope,
    ) -> Result<Intention> {
        self.add_intention_with_scope(description, kind, priority, source_episode_id, Some(scope))
    }

    fn add_intention_with_scope(
        &mut self,
        description: impl Into<String>,
        kind: IntentionKind,
        priority: IntentionPriority,
        source_episode_id: Option<String>,
        scope: Option<MemoryScope>,
    ) -> Result<Intention> {
        let description = description.into().trim().to_string();
        if description.is_empty() {
            return Err(NahualiError::EmptyContent);
        }

        let intention_id = make_id("intention");
        let scope = scope.or_else(|| self.scope_for_episode(source_episode_id.as_deref()));
        let payload = MemoryEvent::IntentionRecorded(IntentionRecorded {
            id: intention_id.clone(),
            kind: intention_event_kind(kind),
            priority: intention_event_priority(priority),
            description,
            source_episode_id,
            deadline_at_ms: None,
            depends_on: Vec::new(),
            goal_id: None,
            progress_percent: None,
            scope,
        });
        self.append(payload)?;

        Ok(self
            .data
            .intentions
            .iter()
            .find(|intention| intention.id == intention_id)
            .expect("appended intention must project")
            .clone())
    }

    /// Update intention metadata without changing its lifecycle status.
    ///
    /// Unknown intention identifiers are rejected before writing an update
    /// event. Optional metadata fields can be set or cleared through
    /// [`IntentionUpdateOptions`].
    pub fn update_intention(
        &mut self,
        id: impl Into<String>,
        options: IntentionUpdateOptions,
    ) -> Result<Intention> {
        let id = id.into().trim().to_string();
        if id.is_empty() {
            return Err(NahualiError::EmptyContent);
        }
        if !self
            .data
            .intentions
            .iter()
            .any(|intention| intention.id == id)
        {
            return Err(NahualiError::UnknownIntention { id });
        }

        let payload = prepare_intention_update(id.clone(), options)?;
        self.append(MemoryEvent::IntentionUpdated(payload))?;

        Ok(self
            .data
            .intentions
            .iter()
            .find(|intention| intention.id == id)
            .expect("updated intention must project")
            .clone())
    }

    /// Mark an intention as completed.
    pub fn complete_intention(
        &mut self,
        id: impl Into<String>,
        reason: Option<String>,
    ) -> Result<Intention> {
        self.set_intention_status(id, IntentionStatus::Completed, reason)
    }

    /// Mark an intention as blocked.
    pub fn block_intention(
        &mut self,
        id: impl Into<String>,
        reason: Option<String>,
    ) -> Result<Intention> {
        self.set_intention_status(id, IntentionStatus::Blocked, reason)
    }

    /// Mark an intention as deferred.
    pub fn defer_intention(
        &mut self,
        id: impl Into<String>,
        reason: Option<String>,
    ) -> Result<Intention> {
        self.set_intention_status(id, IntentionStatus::Deferred, reason)
    }

    /// Change an intention lifecycle state.
    ///
    /// Unknown intention identifiers are rejected before writing a lifecycle
    /// event.
    pub fn set_intention_status(
        &mut self,
        id: impl Into<String>,
        status: IntentionStatus,
        reason: Option<String>,
    ) -> Result<Intention> {
        let id = id.into().trim().to_string();
        if id.is_empty() {
            return Err(NahualiError::EmptyContent);
        }
        if !self
            .data
            .intentions
            .iter()
            .any(|intention| intention.id == id)
        {
            return Err(NahualiError::UnknownIntention { id });
        }

        let payload = MemoryEvent::IntentionStatusChanged(IntentionStatusChanged {
            id: id.clone(),
            status: intention_event_status(status),
            reason: reason.and_then(|reason| {
                let reason = reason.trim().to_string();
                if reason.is_empty() {
                    None
                } else {
                    Some(reason)
                }
            }),
        });
        self.append(payload)?;

        Ok(self
            .data
            .intentions
            .iter()
            .find(|intention| intention.id == id)
            .expect("updated intention must project")
            .clone())
    }

    fn scope_for_episode(&self, episode_id: Option<&str>) -> Option<MemoryScope> {
        episode_id.and_then(|episode_id| {
            self.data
                .episodes
                .iter()
                .find(|episode| episode.id == episode_id)
                .and_then(|episode| episode.scope.clone())
        })
    }
}

fn prepare_intention_update(
    id: String,
    options: IntentionUpdateOptions,
) -> Result<IntentionUpdated> {
    if options.description.is_none()
        && options.priority.is_none()
        && options.deadline_at_ms.is_none()
        && options.depends_on.is_none()
        && options.goal_id.is_none()
        && options.progress_percent.is_none()
    {
        return Err(NahualiError::InvalidIntentionUpdate {
            message: "at least one update field is required".to_string(),
        });
    }

    let description = match options.description {
        Some(description) => {
            let description = description.trim().to_string();
            if description.is_empty() {
                return Err(NahualiError::InvalidIntentionUpdate {
                    message: "description cannot be empty".to_string(),
                });
            }
            Some(description)
        }
        None => None,
    };

    let depends_on = match options.depends_on {
        Some(depends_on) => Some(clean_dependency_ids(&id, depends_on)?),
        None => None,
    };

    let goal_id = match options.goal_id {
        Some(Some(goal_id)) => {
            let goal_id = goal_id.trim().to_string();
            if goal_id.is_empty() {
                return Err(NahualiError::InvalidIntentionUpdate {
                    message: "goal_id cannot be empty".to_string(),
                });
            }
            if goal_id == id {
                return Err(NahualiError::InvalidIntentionUpdate {
                    message: "intention cannot be its own goal".to_string(),
                });
            }
            Some(Some(goal_id))
        }
        Some(None) => Some(None),
        None => None,
    };

    if let Some(Some(progress_percent)) = options.progress_percent
        && progress_percent > 100
    {
        return Err(NahualiError::InvalidIntentionUpdate {
            message: "progress_percent must be between 0 and 100".to_string(),
        });
    }

    Ok(IntentionUpdated {
        id,
        description,
        priority: options.priority.map(intention_event_priority),
        deadline_at_ms: options.deadline_at_ms,
        depends_on,
        goal_id,
        progress_percent: options.progress_percent,
    })
}

fn clean_dependency_ids(self_id: &str, values: Vec<String>) -> Result<Vec<String>> {
    let mut cleaned = Vec::new();
    for value in clean_strings(values) {
        if value == self_id {
            return Err(NahualiError::InvalidIntentionUpdate {
                message: "intention cannot depend on itself".to_string(),
            });
        }
        if !cleaned.iter().any(|existing| existing == &value) {
            cleaned.push(value);
        }
    }
    Ok(cleaned)
}
