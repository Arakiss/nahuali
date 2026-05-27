export type JsonPrimitive = string | number | boolean | null;
export type JsonValue = JsonPrimitive | JsonObject | JsonValue[];
export type JsonObject = { [key: string]: JsonValue };

export type MemoryScopeKind = "personal" | "project" | "organization" | "custom";
export type MemoryKind =
  | "entity"
  | "episode"
  | "claim"
  | "link"
  | "procedure"
  | "intention"
  | "fact"
  | "relation";
export type IntentionKind = "task" | "goal" | "reminder";
export type IntentionPriority = "low" | "medium" | "high" | "critical";
export type IntentionStatus = "active" | "completed" | "abandoned" | "blocked" | "deferred";

export interface NahualiClientOptions {
  baseUrl: string;
  fetch?: typeof fetch;
}

export interface MemoryScope {
  kind: MemoryScopeKind;
  name: string;
}

export interface EpisodeRequest {
  content: string;
  tags?: string[];
  mentions?: string[];
  scope?: MemoryScope;
}

export interface ClaimRequest {
  subject: string;
  predicate: string;
  object: string;
  source_episode_id?: string;
  confidence?: number;
  scope?: MemoryScope;
}

export interface LinkRequest {
  from: string;
  relation: string;
  to: string;
  source_episode_id?: string;
  confidence?: number;
  scope?: MemoryScope;
}

export interface ProcedureRequest {
  name: string;
  body: string;
  source_episode_id?: string;
  confidence?: number;
  scope?: MemoryScope;
}

export interface RecallRequest {
  query: string;
  limit?: number;
  scope?: MemoryScope;
  kinds?: MemoryKind[];
  require_evidence?: boolean;
  semantic?: boolean;
}

export interface SessionResumeRequest {
  episode_limit?: number;
  intention_limit?: number;
  review_limit?: number;
  graph_seed_limit?: number;
}

export interface IntentionRequest {
  description: string;
  kind: IntentionKind;
  priority: IntentionPriority;
  source_episode_id?: string;
  scope?: MemoryScope;
}

export interface IntentionUpdateRequest {
  id: string;
  description?: string;
  priority?: IntentionPriority;
  deadline_at_ms?: number | null;
  depends_on?: string[];
  goal_id?: string | null;
  progress_percent?: number | null;
}

export interface IntentionStatusRequest {
  id: string;
  status: IntentionStatus;
  reason?: string;
}

export interface IntentionReconcileRequest {
  now_ms?: number;
  stale_after_ms?: number;
}

export interface ProactiveRequest {
  now_ms?: number;
  deadline_horizon_ms?: number;
  stale_after_ms?: number;
  review_limit?: number;
}

export interface AnomalyAcknowledgeRequest {
  anomaly_id: string;
  note: string;
  dry_run?: boolean;
}

export interface ReviewResolveRequest {
  review_id: string;
  note: string;
  dry_run?: boolean;
}

export interface GraphQuery {
  seed: string;
  depth?: number;
  limit?: number;
}

export interface LimitQuery {
  limit?: number;
}

export interface NahualiApiErrorBody {
  code?: string;
  message?: string;
}

export class NahualiApiError extends Error {
  readonly status: number;
  readonly code?: string;
  readonly body: unknown;

  constructor(status: number, body: unknown) {
    const error = errorBody(body);
    super(error.message ?? `Nahuali API request failed with status ${status}`);
    this.name = "NahualiApiError";
    this.status = status;
    this.code = error.code;
    this.body = body;
  }
}

export class NahualiClient {
  private readonly baseUrl: string;
  private readonly fetchImpl: typeof fetch;

  constructor(options: NahualiClientOptions) {
    if (!options.baseUrl.trim()) {
      throw new Error("baseUrl is required");
    }
    this.baseUrl = options.baseUrl.replace(/\/+$/, "");
    this.fetchImpl = options.fetch ?? fetch;
  }

  status<T = unknown>(): Promise<T> {
    return this.get("/v1/status");
  }

  openapi<T = unknown>(): Promise<T> {
    return this.get("/v1/openapi.json");
  }

  episode<T = unknown>(request: EpisodeRequest): Promise<T> {
    return this.post("/v1/episode", request);
  }

  claim<T = unknown>(request: ClaimRequest): Promise<T> {
    return this.post("/v1/claim", request);
  }

  link<T = unknown>(request: LinkRequest): Promise<T> {
    return this.post("/v1/link", request);
  }

  procedure<T = unknown>(request: ProcedureRequest): Promise<T> {
    return this.post("/v1/procedure", request);
  }

  intention<T = unknown>(request: IntentionRequest): Promise<T> {
    return this.post("/v1/intention", request);
  }

  updateIntention<T = unknown>(request: IntentionUpdateRequest): Promise<T> {
    return this.post("/v1/intention/update", request);
  }

  setIntentionStatus<T = unknown>(request: IntentionStatusRequest): Promise<T> {
    return this.post("/v1/intention/status", request);
  }

  reconcileIntentions<T = unknown>(request: IntentionReconcileRequest = {}): Promise<T> {
    return this.post("/v1/intention/reconcile", request);
  }

  goalProgress<T = unknown>(): Promise<T> {
    return this.get("/v1/goal-progress");
  }

  recall<T = unknown>(request: RecallRequest): Promise<T> {
    return this.post("/v1/recall", request);
  }

  sessionResume<T = unknown>(request: SessionResumeRequest = {}): Promise<T> {
    return this.post("/v1/session-resume", request);
  }

  memoryHealth<T = unknown>(): Promise<T> {
    return this.post("/v1/memory-health", {});
  }

  proactive<T = unknown>(request: ProactiveRequest = {}): Promise<T> {
    return this.post("/v1/proactive", request);
  }

  deadlines<T = unknown>(request: ProactiveRequest = {}): Promise<T> {
    return this.post("/v1/deadlines", request);
  }

  anomalies<T = unknown>(request: ProactiveRequest = {}): Promise<T> {
    return this.post("/v1/anomalies", request);
  }

  acknowledgeAnomaly<T = unknown>(request: AnomalyAcknowledgeRequest): Promise<T> {
    return this.post("/v1/anomaly/acknowledge", request);
  }

  graph<T = unknown>(query: GraphQuery): Promise<T> {
    return this.get("/v1/graph", query);
  }

  timeline<T = unknown>(query: LimitQuery = {}): Promise<T> {
    return this.get("/v1/timeline", query);
  }

  pending<T = unknown>(query: LimitQuery = {}): Promise<T> {
    return this.get("/v1/pending", query);
  }

  resolveReview<T = unknown>(request: ReviewResolveRequest): Promise<T> {
    return this.post("/v1/review/resolve", request);
  }

  projectionStatus<T = unknown>(): Promise<T> {
    return this.get("/v1/projection/status");
  }

  projectionRebuild<T = unknown>(): Promise<T> {
    return this.post("/v1/projection/rebuild", {});
  }

  projectionValidate<T = unknown>(): Promise<T> {
    return this.post("/v1/projection/validate", {});
  }

  semanticStatus<T = unknown>(): Promise<T> {
    return this.get("/v1/semantic/status");
  }

  semanticRebuild<T = unknown>(): Promise<T> {
    return this.post("/v1/semantic/rebuild", {});
  }

  private async get<T>(path: string, query?: Record<string, string | number | boolean | undefined>): Promise<T> {
    return this.request<T>("GET", withQuery(this.url(path), query));
  }

  private async post<T>(path: string, body: unknown): Promise<T> {
    return this.request<T>("POST", this.url(path), body);
  }

  private async request<T>(method: string, url: string, body?: unknown): Promise<T> {
    const response = await this.fetchImpl(url, {
      method,
      headers: body === undefined ? undefined : { "content-type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    const payload = await readPayload(response);
    if (!response.ok) {
      throw new NahualiApiError(response.status, payload);
    }
    return payload as T;
  }

  private url(path: string): string {
    return `${this.baseUrl}${path}`;
  }
}

export function createNahualiClient(options: NahualiClientOptions): NahualiClient {
  return new NahualiClient(options);
}

function withQuery(url: string, query?: Record<string, string | number | boolean | undefined>): string {
  if (!query) {
    return url;
  }
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(query)) {
    if (value !== undefined) {
      params.set(key, String(value));
    }
  }
  const serialized = params.toString();
  return serialized ? `${url}?${serialized}` : url;
}

async function readPayload(response: Response): Promise<unknown> {
  const text = await response.text();
  if (!text) {
    return null;
  }
  return JSON.parse(text);
}

function errorBody(body: unknown): NahualiApiErrorBody {
  if (
    typeof body === "object" &&
    body !== null &&
    "error" in body &&
    typeof body.error === "object" &&
    body.error !== null
  ) {
    return body.error as NahualiApiErrorBody;
  }
  return {};
}
