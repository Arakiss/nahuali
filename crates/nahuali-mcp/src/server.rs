use std::{
    future::{Future, ready},
    path::PathBuf,
    sync::{Arc, Mutex},
};

use nahuali_core::MemoryEngine;
use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::router::tool::ToolRouter,
    model::{
        AnnotateAble, GetPromptRequestParams, GetPromptResult, Implementation, ListPromptsResult,
        ListResourceTemplatesResult, ListResourcesResult, PaginatedRequestParams, Prompt,
        PromptArgument, PromptMessage, PromptMessageRole, RawResource, ReadResourceRequestParams,
        ReadResourceResult, ResourceContents, ServerCapabilities, ServerInfo,
    },
    service::{MaybeSendFuture, RequestContext, RoleServer},
    tool_handler,
};
use serde_json::{Value, json};

use crate::{
    catalog::{
        PROMPT_RECALL_WITH_HEALTH, PROMPT_RECORD_EVIDENCE_BACKED_FACT, RESOURCE_CLAIMS,
        RESOURCE_ENTITIES, RESOURCE_EPISODES, RESOURCE_EVENTS, RESOURCE_FACTS, RESOURCE_HEALTH,
        RESOURCE_INTENTIONS, RESOURCE_LINKS, RESOURCE_PROCEDURES, RESOURCE_RELATIONS,
        RESOURCE_SOURCES, RESOURCE_SUMMARY, SERVER_NAME, prompt_catalog, resource_catalog,
    },
    protocol::{EntityView, json_string},
};

#[derive(Debug, Clone)]
pub struct NahualiMcpServer {
    pub(crate) tool_router: ToolRouter<Self>,
    pub(crate) memory: Arc<Mutex<MemoryEngine>>,
    pub(crate) database: PathBuf,
}

impl NahualiMcpServer {
    pub fn open(database: PathBuf) -> nahuali_core::Result<Self> {
        let memory = MemoryEngine::open(&database)?;
        Ok(Self {
            tool_router: crate::tools::tool_router(),
            memory: Arc::new(Mutex::new(memory)),
            database,
        })
    }

    pub(crate) fn with_memory<T>(
        &self,
        operation: impl FnOnce(&mut MemoryEngine) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut memory = self
            .memory
            .lock()
            .map_err(|_| "memory database lock is poisoned".to_string())?;
        memory.refresh().map_err(|error| error.to_string())?;
        operation(&mut memory)
    }

    pub(crate) fn resolve_source_episode_id(
        memory: &MemoryEngine,
        source_episode_id: Option<String>,
        source_last: bool,
    ) -> Result<Option<String>, String> {
        if source_episode_id.is_some() && source_last {
            return Err("sourceLast cannot be used with sourceEpisodeId".to_string());
        }

        if source_last {
            let episode = memory.data().episodes.last().ok_or_else(|| {
                "sourceLast requires at least one episode in the selected database".to_string()
            })?;
            return Ok(Some(episode.id.clone()));
        }

        Ok(source_episode_id)
    }

    fn resources() -> Vec<rmcp::model::Resource> {
        resource_catalog()
            .into_iter()
            .map(|resource| {
                RawResource::new(resource.uri, resource.name)
                    .with_title(resource.title)
                    .with_description(resource.description)
                    .with_mime_type("application/json")
                    .no_annotation()
            })
            .collect()
    }

    fn prompts() -> Vec<Prompt> {
        prompt_catalog()
            .into_iter()
            .map(|prompt| match prompt.name {
                PROMPT_RECALL_WITH_HEALTH => Prompt::new(
                    prompt.name,
                    Some(prompt.description),
                    Some(vec![
                        PromptArgument::new("query")
                            .with_title("Query")
                            .with_description("Memory question or topic to recall.")
                            .with_required(true),
                    ]),
                )
                .with_title(prompt.title),
                PROMPT_RECORD_EVIDENCE_BACKED_FACT => Prompt::new(
                    prompt.name,
                    Some(prompt.description),
                    Some(vec![
                        PromptArgument::new("observation")
                            .with_title("Observation")
                            .with_description("Observed episode content to preserve as evidence.")
                            .with_required(true),
                        PromptArgument::new("subject")
                            .with_title("Subject")
                            .with_description("Claim subject to derive from the observation.")
                            .with_required(false),
                        PromptArgument::new("predicate")
                            .with_title("Predicate")
                            .with_description("Claim predicate to derive from the observation.")
                            .with_required(false),
                        PromptArgument::new("object")
                            .with_title("Object")
                            .with_description("Claim object to derive from the observation.")
                            .with_required(false),
                    ]),
                )
                .with_title(prompt.title),
                _ => unreachable!("prompt catalog contains a known prompt"),
            })
            .collect()
    }

    fn resource_json(&self, uri: &str) -> Result<String, McpError> {
        let value = self
            .with_memory(|memory| match uri {
                RESOURCE_SUMMARY => {
                    let data = memory.data();
                    let health = memory.inspect();
                    let authority = nahuali_core::AuthorityDecision::evaluate(&health);
                    Ok(json!({
                        "version": data.version,
                        "event_count": data.event_count,
                        "source_count": data.sources.len(),
                        "entity_count": data.entities.len(),
                        "episode_count": data.episodes.len(),
                        "claim_count": data.claims.len(),
                        "link_count": data.links.len(),
                        "fact_count": data.facts.len(),
                        "relation_count": data.relations.len(),
                        "procedure_count": data.procedures.len(),
                        "intention_count": data.intentions.len(),
                        "review_decision_count": data.review_decisions.len(),
                        "last_event_id": data.last_event_id,
                        "supported_fact_count": health.supported_fact_count,
                        "unsupported_fact_count": health.unsupported_fact_count,
                        "conflicting_fact_count": health.conflicting_fact_count,
                        "blind_spot_count": health.blind_spot_count,
                        "signal_count": health.signal_count,
                        "authority_mode": json_string(&authority.mode),
                        "authority_score": authority.score,
                        "authority_can_trust": authority.can_trust,
                    }))
                }
                RESOURCE_SOURCES => serde_json::to_value(&memory.data().sources)
                    .map_err(|error| format!("failed to encode sources resource: {error}")),
                RESOURCE_HEALTH => serde_json::to_value(memory.inspect()).map_err(|error| {
                    format!("failed to encode knowledge health resource: {error}")
                }),
                RESOURCE_ENTITIES => serde_json::to_value(
                    memory
                        .data()
                        .entities
                        .iter()
                        .cloned()
                        .map(EntityView::from)
                        .collect::<Vec<_>>(),
                )
                .map_err(|error| format!("failed to encode entities resource: {error}")),
                RESOURCE_EPISODES => serde_json::to_value(&memory.data().episodes)
                    .map_err(|error| format!("failed to encode episodes resource: {error}")),
                RESOURCE_CLAIMS => serde_json::to_value(&memory.data().claims)
                    .map_err(|error| format!("failed to encode claims resource: {error}")),
                RESOURCE_LINKS => serde_json::to_value(&memory.data().links)
                    .map_err(|error| format!("failed to encode links resource: {error}")),
                RESOURCE_FACTS => serde_json::to_value(&memory.data().facts)
                    .map_err(|error| format!("failed to encode facts resource: {error}")),
                RESOURCE_RELATIONS => serde_json::to_value(&memory.data().relations)
                    .map_err(|error| format!("failed to encode relations resource: {error}")),
                RESOURCE_PROCEDURES => serde_json::to_value(&memory.data().procedures)
                    .map_err(|error| format!("failed to encode procedures resource: {error}")),
                RESOURCE_INTENTIONS => serde_json::to_value(&memory.data().intentions)
                    .map_err(|error| format!("failed to encode intentions resource: {error}")),
                RESOURCE_EVENTS => serde_json::to_value(memory.events())
                    .map_err(|error| format!("failed to encode events resource: {error}")),
                _ => Err(format!("resource not found: {uri}")),
            })
            .map_err(|message| {
                if message.starts_with("resource not found:") {
                    McpError::resource_not_found(message, Some(json!({ "uri": uri })))
                } else {
                    McpError::internal_error(message, None)
                }
            })?;

        serde_json::to_string_pretty(&value).map_err(|error| {
            McpError::internal_error(format!("failed to encode resource: {error}"), None)
        })
    }

    fn required_string_arg(
        arguments: Option<&rmcp::model::JsonObject>,
        name: &'static str,
    ) -> Result<String, McpError> {
        arguments
            .and_then(|arguments| arguments.get(name))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .ok_or_else(|| {
                McpError::invalid_params(
                    format!("missing required prompt argument: {name}"),
                    Some(json!({ "argument": name })),
                )
            })
    }

    fn optional_string_arg(
        arguments: Option<&rmcp::model::JsonObject>,
        name: &'static str,
    ) -> Option<String> {
        arguments
            .and_then(|arguments| arguments.get(name))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    }

    fn recall_with_health_prompt(
        request: GetPromptRequestParams,
    ) -> Result<GetPromptResult, McpError> {
        let query = Self::required_string_arg(request.arguments.as_ref(), "query")?;
        let text = format!(
            "Recall Nahuali memory for this query: \"{query}\".\n\nWorkflow:\n1. Call the `recall` tool with the query and a focused limit.\n2. Read each result's `trust` object before using it, then read the store-level `authority` and `health` context; call `inspect` only if broader database context is needed.\n3. Cite `evidence_id` values for any claim, link, fact, relation, procedure, or intention used.\n4. If result trust is `warn` or `block`, authority is `warn` or `block`, or health reports unsupported facts, contradictions, stale facts, or blind spots, state that uncertainty directly.\n5. Prefer saying that memory is insufficient over filling gaps from inference."
        );

        Ok(
            GetPromptResult::new(vec![PromptMessage::new_text(PromptMessageRole::User, text)])
                .with_description("Recall memory with explicit health and evidence checks."),
        )
    }

    fn record_evidence_backed_fact_prompt(
        request: GetPromptRequestParams,
    ) -> Result<GetPromptResult, McpError> {
        let observation = Self::required_string_arg(request.arguments.as_ref(), "observation")?;
        let subject = Self::optional_string_arg(request.arguments.as_ref(), "subject");
        let predicate = Self::optional_string_arg(request.arguments.as_ref(), "predicate");
        let object = Self::optional_string_arg(request.arguments.as_ref(), "object");

        let mut text = format!(
            "Record this observation as Nahuali evidence before deriving assertions:\n\n{observation}\n\nWorkflow:\n1. Call `remember` with the observation.\n2. Only if the observation explicitly supports a claim, call `claim` with `sourceLast: true` so the claim cites the just-recorded episode.\n3. If a link is explicit, call `link` with `sourceLast: true`.\n4. Call `inspect` and confirm the derived memory is supported.\n5. Do not assert claims that are not directly supported by the observation."
        );

        if let (Some(subject), Some(predicate), Some(object)) = (subject, predicate, object) {
            text.push_str(&format!(
                "\n\nCandidate fact to validate against the observation: {subject} {predicate} {object}."
            ));
        }

        Ok(
            GetPromptResult::new(vec![PromptMessage::new_text(PromptMessageRole::User, text)])
                .with_description("Record a supported claim without detaching it from evidence."),
        )
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for NahualiMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .build(),
        )
        .with_server_info(
            Implementation::new(SERVER_NAME, env!("CARGO_PKG_VERSION"))
                .with_title("Nahuali")
                .with_description("Self-inspecting memory for AI agents."),
        )
        .with_instructions(
            "Use tools to record synthetic or user-approved memory, recall it with evidence, and inspect health signals before trusting results.",
        )
    }

    fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListResourcesResult, McpError>> + MaybeSendFuture + '_ {
        ready(Ok(ListResourcesResult {
            resources: Self::resources(),
            next_cursor: None,
            meta: None,
        }))
    }

    fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListResourceTemplatesResult, McpError>> + MaybeSendFuture + '_
    {
        ready(Ok(ListResourceTemplatesResult {
            resource_templates: Vec::new(),
            next_cursor: None,
            meta: None,
        }))
    }

    fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ReadResourceResult, McpError>> + MaybeSendFuture + '_ {
        let result = self.resource_json(&request.uri).map(|text| {
            ReadResourceResult::new(vec![
                ResourceContents::text(text, request.uri).with_mime_type("application/json"),
            ])
        });

        ready(result)
    }

    fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListPromptsResult, McpError>> + MaybeSendFuture + '_ {
        ready(Ok(ListPromptsResult {
            prompts: Self::prompts(),
            next_cursor: None,
            meta: None,
        }))
    }

    fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<GetPromptResult, McpError>> + MaybeSendFuture + '_ {
        let result = match request.name.as_str() {
            PROMPT_RECALL_WITH_HEALTH => Self::recall_with_health_prompt(request),
            PROMPT_RECORD_EVIDENCE_BACKED_FACT => Self::record_evidence_backed_fact_prompt(request),
            _ => Err(McpError::invalid_params(
                "unknown prompt",
                Some(json!({ "name": request.name })),
            )),
        };

        ready(result)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rmcp::ServerHandler;

    use super::NahualiMcpServer;
    use crate::{
        SERVER_NAME,
        catalog::{
            PROMPT_RECALL_WITH_HEALTH, RESOURCE_ENTITIES, RESOURCE_HEALTH, RESOURCE_SOURCES,
            RESOURCE_SUMMARY,
        },
        prompt_catalog, resource_catalog, tool_catalog,
    };

    #[test]
    fn exposes_initial_tool_boundary() {
        let tools = tool_catalog();
        let names = tools.iter().map(|tool| tool.name).collect::<Vec<_>>();

        assert_eq!(SERVER_NAME, "nahuali");
        assert_eq!(
            names,
            vec![
                "remember",
                "ingest",
                "ingest_text",
                "claim",
                "fact",
                "link",
                "relate",
                "procedure",
                "preference",
                "intention",
                "intention_status",
                "intention_update",
                "reconcile_intentions",
                "goal_progress",
                "proactive",
                "deadlines",
                "anomalies",
                "anomaly_acknowledge",
                "briefing",
                "memory_hook",
                "recall",
                "graph",
                "inspect",
                "self_inspect",
                "reflect",
                "consolidation_plan",
                "review",
                "review_resolve",
                "projection_status",
                "projection_rebuild",
                "projection_validate",
                "semantic_status",
                "semantic_rebuild",
                "semantic_sync",
                "validate",
                "audit",
                "trust_report"
            ]
        );
        assert!(tools.iter().any(
            |tool| tool.name == "validate" && tool.description.contains("memory_record ledger")
        ));
    }

    #[test]
    fn exposes_resource_and_prompt_boundaries() {
        let resources = resource_catalog();
        let resource_uris = resources
            .iter()
            .map(|resource| resource.uri)
            .collect::<Vec<_>>();
        let prompt_names = prompt_catalog()
            .into_iter()
            .map(|prompt| prompt.name)
            .collect::<Vec<_>>();

        assert_eq!(
            resource_uris,
            vec![
                RESOURCE_SUMMARY,
                RESOURCE_SOURCES,
                RESOURCE_HEALTH,
                RESOURCE_ENTITIES,
                "nahuali://database/episodes",
                "nahuali://database/claims",
                "nahuali://database/links",
                "nahuali://database/facts",
                "nahuali://database/relations",
                "nahuali://database/procedures",
                "nahuali://database/intentions",
                "nahuali://database/records",
            ]
        );
        assert_eq!(
            prompt_names,
            vec![PROMPT_RECALL_WITH_HEALTH, "record_evidence_backed_fact"]
        );
        assert!(
            resources
                .iter()
                .any(|resource| resource.uri == RESOURCE_SUMMARY
                    && resource.description.contains("SurrealDB record ledger"))
        );
        assert!(
            resources
                .iter()
                .any(|resource| resource.uri == "nahuali://database/records"
                    && resource.description.contains("memory_record envelopes"))
        );
    }

    #[test]
    fn exposes_rmcp_tool_router() {
        let path = std::env::temp_dir().join(format!(
            "nahuali-mcp-router-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        let _ = fs::remove_dir_all(&path);

        let server = NahualiMcpServer::open(path.clone()).expect("server opens");
        let info = server.get_info();
        let mut names = server
            .tool_router
            .list_all()
            .into_iter()
            .map(|tool| tool.name.to_string())
            .collect::<Vec<_>>();
        names.sort();

        assert_eq!(
            names,
            vec![
                "anomalies",
                "anomaly_acknowledge",
                "audit",
                "briefing",
                "claim",
                "consolidation_plan",
                "deadlines",
                "fact",
                "goal_progress",
                "graph",
                "ingest",
                "ingest_text",
                "inspect",
                "intention",
                "intention_status",
                "intention_update",
                "link",
                "memory_hook",
                "preference",
                "proactive",
                "procedure",
                "projection_rebuild",
                "projection_status",
                "projection_validate",
                "recall",
                "reconcile_intentions",
                "reflect",
                "relate",
                "remember",
                "review",
                "review_resolve",
                "self_inspect",
                "semantic_rebuild",
                "semantic_status",
                "semantic_sync",
                "trust_report",
                "validate"
            ]
        );
        assert!(info.capabilities.tools.is_some());
        assert!(info.capabilities.resources.is_some());
        assert!(info.capabilities.prompts.is_some());

        let _ = fs::remove_file(path);
    }

    fn unique_suffix() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock is after epoch")
            .as_nanos()
    }
}
