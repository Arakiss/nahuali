pub(crate) struct ImportEvidenceContext {
    pub(crate) source_episode_id: Option<String>,
    pub(crate) confidence: f32,
    pub(crate) scope: Option<MemoryScope>,
    pub(crate) timestamp_ms: u64,
}

impl MemoryEngine {
    pub(crate) fn import_source_at(
        &mut self,
        options: SourceRecordOptions,
        timestamp_ms: u64,
    ) -> Result<SourceDocument> {
        let content_checksum = options.content_checksum.trim().to_string();
        if content_checksum.is_empty() {
            return Err(NahualiError::EmptyContent);
        }

        let source_id = make_id_at("source", timestamp_ms);
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
        self.append_at(timestamp_ms, payload)?;

        Ok(self
            .data
            .sources
            .iter()
            .find(|source| source.id == source_id)
            .expect("appended source must project")
            .clone())
    }

    pub(crate) fn import_episode_at(
        &mut self,
        content: impl Into<String>,
        tags: Vec<String>,
        mentions: Vec<String>,
        scope: Option<MemoryScope>,
        source_role: Option<String>,
        timestamp_ms: u64,
    ) -> Result<Episode> {
        let content = content.into().trim().to_string();
        if content.is_empty() {
            return Err(NahualiError::EmptyContent);
        }

        let episode_id = make_id_at("episode", timestamp_ms);
        let payload = MemoryEvent::EpisodeRecorded(EpisodeRecorded {
            id: episode_id.clone(),
            content,
            tags,
            mentions: clean_strings(mentions),
            source_id: None,
            source_position: None,
            source_role: clean_optional_string(source_role),
            scope,
        });
        self.append_at(timestamp_ms, payload)?;

        Ok(self
            .data
            .episodes
            .iter()
            .find(|episode| episode.id == episode_id)
            .expect("appended episode must project")
            .clone())
    }

    pub(crate) fn import_source_episode_at(
        &mut self,
        options: SourceEpisodeOptions,
        timestamp_ms: u64,
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

        let content = options.content.trim().to_string();
        if content.is_empty() {
            return Err(NahualiError::EmptyContent);
        }

        let episode_id = make_id_at("episode", timestamp_ms);
        let payload = MemoryEvent::EpisodeRecorded(EpisodeRecorded {
            id: episode_id.clone(),
            content,
            tags: options.tags,
            mentions: clean_strings(options.mentions),
            source_id: Some(source_id),
            source_position: options.source_position,
            source_role: clean_optional_string(options.source_role),
            scope: options.scope.or(source_scope),
        });
        self.append_at(timestamp_ms, payload)?;

        Ok(self
            .data
            .episodes
            .iter()
            .find(|episode| episode.id == episode_id)
            .expect("appended source episode must project")
            .clone())
    }

    pub(crate) fn import_claim_at(
        &mut self,
        subject: impl Into<String>,
        predicate: impl Into<String>,
        object: impl Into<String>,
        context: ImportEvidenceContext,
    ) -> Result<Claim> {
        let subject = subject.into().trim().to_string();
        let predicate = predicate.into().trim().to_string();
        let object = object.into().trim().to_string();
        if subject.is_empty() || predicate.is_empty() || object.is_empty() {
            return Err(NahualiError::EmptyContent);
        }

        let claim_id = make_id_at("claim", context.timestamp_ms);
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
        self.append_at(context.timestamp_ms, payload)?;

        Ok(self
            .data
            .facts
            .iter()
            .find(|claim| claim.id == claim_id)
            .expect("appended claim must project")
            .clone())
    }

    pub(crate) fn import_link_at(
        &mut self,
        from: impl Into<String>,
        relation: impl Into<String>,
        to: impl Into<String>,
        context: ImportEvidenceContext,
    ) -> Result<Link> {
        let from = from.into().trim().to_string();
        let relation = relation.into().trim().to_string();
        let to = to.into().trim().to_string();
        if from.is_empty() || relation.is_empty() || to.is_empty() {
            return Err(NahualiError::EmptyContent);
        }

        let link_id = make_id_at("link", context.timestamp_ms);
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
        self.append_at(context.timestamp_ms, payload)?;

        Ok(self
            .data
            .relations
            .iter()
            .find(|link| link.id == link_id)
            .expect("appended link must project")
            .clone())
    }

    pub(crate) fn import_procedure_at(
        &mut self,
        kind: ProcedureKind,
        name: impl Into<String>,
        body: impl Into<String>,
        context: ImportEvidenceContext,
    ) -> Result<Procedure> {
        let name = name.into().trim().to_string();
        let body = body.into().trim().to_string();
        if name.is_empty() || body.is_empty() {
            return Err(NahualiError::EmptyContent);
        }

        let procedure_id = make_id_at("procedure", context.timestamp_ms);
        let kind = match kind {
            ProcedureKind::Procedure => ProcedureRecordedKind::Procedure,
            ProcedureKind::Preference => ProcedureRecordedKind::Preference,
        };
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
        self.append_at(context.timestamp_ms, payload)?;

        Ok(self
            .data
            .procedures
            .iter()
            .find(|procedure| procedure.id == procedure_id)
            .expect("appended procedure must project")
            .clone())
    }

    pub(crate) fn import_intention_at(
        &mut self,
        description: impl Into<String>,
        kind: IntentionKind,
        priority: IntentionPriority,
        source_episode_id: Option<String>,
        scope: Option<MemoryScope>,
        timestamp_ms: u64,
    ) -> Result<Intention> {
        let description = description.into().trim().to_string();
        if description.is_empty() {
            return Err(NahualiError::EmptyContent);
        }

        let intention_id = make_id_at("intention", timestamp_ms);
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
        self.append_at(timestamp_ms, payload)?;

        Ok(self
            .data
            .intentions
            .iter()
            .find(|intention| intention.id == intention_id)
            .expect("appended intention must project")
            .clone())
    }

    pub(crate) fn import_intention_status_at(
        &mut self,
        id: impl Into<String>,
        status: IntentionStatus,
        reason: Option<String>,
        timestamp_ms: u64,
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
            reason: clean_optional_string(reason),
        });
        self.append_at(timestamp_ms, payload)?;

        Ok(self
            .data
            .intentions
            .iter()
            .find(|intention| intention.id == id)
            .expect("updated intention must project")
            .clone())
    }
}
