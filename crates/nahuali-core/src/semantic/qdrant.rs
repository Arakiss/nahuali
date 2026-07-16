use std::time::Duration;

const DEFAULT_QDRANT_REQUEST_TIMEOUT_SECS: u64 = 120;

#[derive(Clone, Debug, Serialize)]
struct QdrantPoint {
    id: u64,
    vector: Vec<f32>,
    payload: SemanticPayload,
}

#[derive(Debug, Deserialize)]
struct QdrantEnvelope<T> {
    status: serde_json::Value,
    result: T,
}

#[derive(Debug, Deserialize)]
struct QueryResult {
    points: Vec<ScoredPoint>,
}

#[derive(Debug, Deserialize)]
struct ScoredPoint {
    score: f32,
    payload: Option<SemanticPayload>,
}

#[derive(Debug, Deserialize)]
struct ScrollResult {
    points: Vec<ScrolledPoint>,
    next_page_offset: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct ScrolledPoint {
    id: serde_json::Value,
    payload: Option<serde_json::Value>,
}

struct QdrantRestClient {
    http: Client,
    base_url: String,
    auth_header: Option<String>,
}

impl QdrantRestClient {
    fn new(config: &SemanticConfig) -> Self {
        let http = Client::builder()
            .timeout(qdrant_request_timeout())
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            http,
            base_url: normalize_qdrant_url(&config.qdrant_url),
            auth_header: non_empty_env("NAHUALI_QDRANT_API_KEY"),
        }
    }

    fn collection_exists(&self, collection_name: &str) -> Result<bool> {
        let endpoint = self.collection_url(collection_name);
        let request = self.with_api_key(self.http.get(&endpoint));
        let response = request
            .send()
            .map_err(|source| semantic_http_error(&endpoint, source))?;
        if response.status().as_u16() == 404 {
            return Ok(false);
        }
        let _: serde_json::Value = self.parse_response(&endpoint, response)?;
        Ok(true)
    }

    fn create_collection(
        &self,
        collection_name: &str,
        embedding: &EmbeddingProviderConfig,
    ) -> Result<()> {
        let endpoint = self.collection_url(collection_name);
        let body = serde_json::json!({
            "vectors": {
                "size": embedding.dimensions,
                "distance": "Cosine"
            },
            "metadata": {
                "nahuali_schema": format!("semantic_index_v{}", SEMANTIC_INDEX_SCHEMA_VERSION),
                "embedding_provider": embedding.model,
                "embedding_kind": embedding.kind,
                "embedding_dimensions": embedding.dimensions
            }
        });
        let request = self.with_api_key(self.http.put(&endpoint)).json(&body);
        let _: serde_json::Value = self.send_json(&endpoint, request)?;
        self.create_payload_indexes(collection_name)?;
        Ok(())
    }

    fn delete_collection_if_exists(&self, collection_name: &str) -> Result<bool> {
        let endpoint = self.collection_url(collection_name);
        let request = self.with_api_key(self.http.delete(&endpoint));
        let response = request
            .send()
            .map_err(|source| semantic_http_error(&endpoint, source))?;
        if response.status().as_u16() == 404 {
            return Ok(false);
        }
        let deleted: bool = self.parse_response(&endpoint, response)?;
        Ok(deleted)
    }

    fn upsert_points(&self, collection_name: &str, points: &[QdrantPoint]) -> Result<()> {
        let endpoint = format!(
            "{}/collections/{collection_name}/points?wait=true",
            self.base_url
        );
        let body = serde_json::json!({ "points": points });
        let request = self.with_api_key(self.http.put(&endpoint)).json(&body);
        let _: serde_json::Value = self.send_json(&endpoint, request)?;
        Ok(())
    }

    fn query_points(
        &self,
        collection_name: &str,
        query_vector: &[f32],
        limit: usize,
        filter: Option<SemanticQueryFilter>,
    ) -> Result<Vec<SemanticMatch>> {
        let endpoint = format!(
            "{}/collections/{collection_name}/points/query",
            self.base_url
        );
        let mut body = serde_json::json!({
            "query": query_vector,
            "limit": limit,
            "with_payload": true,
            "with_vector": false
        });
        if let Some(filter) = filter.and_then(|filter| filter.to_qdrant_filter()) {
            body["filter"] = filter;
        }
        let request = self.with_api_key(self.http.post(&endpoint)).json(&body);
        let result: QueryResult = self.send_json(&endpoint, request)?;
        Ok(result
            .points
            .into_iter()
            .filter_map(|point| {
                let payload = point.payload?;
                let kind = memory_kind_from_name(&payload.kind)?;
                Some(SemanticMatch {
                    kind,
                    id: payload.id,
                    event_id: payload.event_id,
                    score: point.score,
                    excerpt: payload.excerpt,
                    evidence_id: payload.evidence_id,
                    scope_key: payload.scope_key,
                })
            })
            .collect())
    }

    fn create_payload_indexes(&self, collection_name: &str) -> Result<()> {
        for (field_name, field_schema) in [
            ("scope_key", "keyword"),
            ("scope_kind", "keyword"),
            ("kind", "keyword"),
            ("surreal_table", "keyword"),
            ("surreal_id", "keyword"),
            ("event_ids", "keyword"),
            ("entity_names", "keyword"),
            ("source_ids", "keyword"),
            ("has_evidence", "bool"),
            ("created_at_ms", "integer"),
            ("projection_version", "integer"),
            ("schema_version", "integer"),
        ] {
            self.create_payload_index(collection_name, field_name, field_schema)?;
        }
        Ok(())
    }

    fn create_payload_index(
        &self,
        collection_name: &str,
        field_name: &str,
        field_schema: &str,
    ) -> Result<()> {
        let endpoint = format!("{}/collections/{collection_name}/index", self.base_url);
        let body = serde_json::json!({
            "field_name": field_name,
            "field_schema": field_schema,
        });
        let request = self.with_api_key(self.http.put(&endpoint)).json(&body);
        let _: serde_json::Value = self.send_json(&endpoint, request)?;
        Ok(())
    }

    fn scroll_point_payloads(
        &self,
        collection_name: &str,
    ) -> Result<BTreeMap<String, Option<SemanticPayload>>> {
        let endpoint = format!(
            "{}/collections/{collection_name}/points/scroll",
            self.base_url
        );
        let mut points = BTreeMap::new();
        let mut offset = None;

        loop {
            let mut body = serde_json::json!({
                "limit": 256,
                "with_payload": true,
                "with_vector": false
            });
            if let Some(value) = offset.take() {
                body["offset"] = value;
            }
            let request = self.with_api_key(self.http.post(&endpoint)).json(&body);
            let page: ScrollResult = self.send_json(&endpoint, request)?;
            for point in page.points {
                let point_id = qdrant_point_id_key(&point.id);
                let payload = point
                    .payload
                    .and_then(|payload| serde_json::from_value(payload).ok());
                points.insert(point_id, payload);
            }
            match page.next_page_offset {
                Some(next_offset) => offset = Some(next_offset),
                None => break,
            }
        }

        Ok(points)
    }

    #[cfg(test)]
    fn delete_points(&self, collection_name: &str, point_ids: &[u64]) -> Result<()> {
        let endpoint = format!(
            "{}/collections/{collection_name}/points/delete?wait=true",
            self.base_url
        );
        let body = serde_json::json!({ "points": point_ids });
        let request = self.with_api_key(self.http.post(&endpoint)).json(&body);
        let _: serde_json::Value = self.send_json(&endpoint, request)?;
        Ok(())
    }

    fn send_json<T: DeserializeOwned>(
        &self,
        endpoint: &str,
        request: reqwest::blocking::RequestBuilder,
    ) -> Result<T> {
        let response = request
            .send()
            .map_err(|source| semantic_http_error(endpoint, source))?;
        self.parse_response(endpoint, response)
    }

    fn parse_response<T: DeserializeOwned>(&self, endpoint: &str, response: Response) -> Result<T> {
        let status = response.status();
        let body = response
            .text()
            .map_err(|source| semantic_http_error(endpoint, source))?;
        if !status.is_success() {
            return Err(NahualiError::SemanticApi {
                endpoint: endpoint.to_string(),
                status: status.as_u16(),
                message: body,
            });
        }
        let envelope: QdrantEnvelope<T> =
            serde_json::from_str(&body).map_err(|source| NahualiError::SemanticDecode {
                endpoint: endpoint.to_string(),
                source,
            })?;
        if envelope.status != serde_json::Value::String("ok".to_string()) {
            return Err(NahualiError::SemanticApi {
                endpoint: endpoint.to_string(),
                status: status.as_u16(),
                message: envelope.status.to_string(),
            });
        }
        Ok(envelope.result)
    }

    fn with_api_key(
        &self,
        request: reqwest::blocking::RequestBuilder,
    ) -> reqwest::blocking::RequestBuilder {
        if let Some(api_key) = &self.auth_header {
            request.header("api-key", api_key)
        } else {
            request
        }
    }

    fn collection_url(&self, collection_name: &str) -> String {
        format!("{}/collections/{collection_name}", self.base_url)
    }
}

fn qdrant_point_id_key(value: &serde_json::Value) -> String {
    value
        .as_u64()
        .map(|id| id.to_string())
        .or_else(|| value.as_str().map(str::to_string))
        .unwrap_or_else(|| value.to_string())
}

fn qdrant_request_timeout() -> Duration {
    let seconds = non_empty_env("NAHUALI_QDRANT_REQUEST_TIMEOUT_SECS")
        .and_then(|raw| raw.parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .unwrap_or(DEFAULT_QDRANT_REQUEST_TIMEOUT_SECS);

    Duration::from_secs(seconds)
}
