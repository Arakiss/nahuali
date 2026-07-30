/** Exercise Nahuali's HTTP trust contract with Bun's native fetch. */

type JsonObject = Record<string, unknown>;

function require(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message);
}

function requireObject(value: unknown, path: string): JsonObject {
  require(value !== null && typeof value === "object" && !Array.isArray(value), `${path} must be an object`);
  return value as JsonObject;
}

function requireStringArray(value: unknown, path: string): string[] {
  require(Array.isArray(value), `${path} must be an array`);
  require(value.every((item) => typeof item === "string"), `${path} must contain strings`);
  return value as string[];
}

function validateLoopbackUrl(rawUrl: string): string {
  const url = new URL(rawUrl);
  require(url.protocol === "http:" || url.protocol === "https:", "NAHUALI_API_URL must use HTTP or HTTPS");
  require(
    url.hostname === "127.0.0.1" || url.hostname === "[::1]" || url.hostname === "localhost",
    "This example intentionally connects only to a loopback address",
  );
  require(url.pathname === "/", "NAHUALI_API_URL must not include a path");
  return rawUrl.replace(/\/$/, "");
}

async function postJson(baseUrl: string, path: string, payload: JsonObject): Promise<JsonObject> {
  const response = await fetch(`${baseUrl}${path}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(payload),
  });
  const text = await response.text();
  require(response.ok, `${path} returned HTTP ${response.status}: ${text}`);
  let body: unknown;
  try {
    body = JSON.parse(text);
  } catch {
    throw new Error(`${path} returned invalid JSON`);
  }
  return requireObject(body, path);
}

function validateAuthority(value: unknown, path: string): JsonObject {
  const authority = requireObject(value, path);
  require(typeof authority.mode === "string", `${path}.mode must be a string`);
  require(
    typeof authority.score === "number" && authority.score >= 0 && authority.score <= 1,
    `${path}.score must be a number in the 0..1 range`,
  );
  require(typeof authority.can_trust === "boolean", `${path}.can_trust must be boolean`);
  requireStringArray(authority.reasons, `${path}.reasons`);
  requireStringArray(authority.signal_kinds, `${path}.signal_kinds`);
  return authority;
}

function validateHealth(value: unknown, path: string): JsonObject {
  const health = requireObject(value, path);
  require(Array.isArray(health.signals), `${path}.signals must be an array`);
  for (const [index, rawSignal] of health.signals.entries()) {
    const signalPath = `${path}.signals[${index}]`;
    const signal = requireObject(rawSignal, signalPath);
    require(typeof signal.kind === "string", `${signalPath}.kind must be a string`);
    requireStringArray(signal.dimensions, `${signalPath}.dimensions`);
    require(typeof signal.severity === "string", `${signalPath}.severity must be a string`);
    require(typeof signal.message === "string", `${signalPath}.message must be a string`);
    requireStringArray(signal.evidence_ids, `${signalPath}.evidence_ids`);
  }
  requireStringArray(health.warnings, `${path}.warnings`);
  return health;
}

function validateResultTrust(value: unknown, path: string): JsonObject {
  const trust = requireObject(value, path);
  require(typeof trust.mode === "string", `${path}.mode must be a string`);
  require(
    typeof trust.score === "number" && trust.score >= 0 && trust.score <= 1,
    `${path}.score must be a number in the 0..1 range`,
  );
  require(typeof trust.can_trust === "boolean", `${path}.can_trust must be boolean`);
  requireStringArray(trust.reasons, `${path}.reasons`);
  requireStringArray(trust.signal_kinds, `${path}.signal_kinds`);
  return trust;
}

function validateRecallResults(recall: JsonObject, path: string): void {
  require(Array.isArray(recall.lexical_results), `${path}.lexical_results must be an array`);
  for (const [index, rawResult] of recall.lexical_results.entries()) {
    const resultPath = `${path}.lexical_results[${index}]`;
    const result = requireObject(rawResult, resultPath);
    require(typeof result.id === "string", `${resultPath}.id must be a string`);
    require(
      result.evidence_id === null || typeof result.evidence_id === "string",
      `${resultPath}.evidence_id must be a string or null`,
    );
    validateResultTrust(result.trust, `${resultPath}.trust`);
  }
}

function findResult(recall: JsonObject, resultId: string): JsonObject {
  require(Array.isArray(recall.lexical_results), "recall.lexical_results must be an array");
  for (const rawResult of recall.lexical_results) {
    const result = requireObject(rawResult, "recall.lexical_results[]");
    if (result.id === resultId) return result;
  }
  throw new Error(`recall did not return expected result ${resultId}`);
}

async function main(): Promise<void> {
  const baseUrl = validateLoopbackUrl(process.env.NAHUALI_API_URL ?? "http://127.0.0.1:7070");
  const runId = process.env.NAHUALI_EXAMPLE_RUN_ID ?? `typescript_${process.pid}_${Date.now()}`;
  const subject = `TypeScript HTTP example ${runId}`;
  const predicate = "rollout mode";

  const episode = await postJson(baseUrl, "/v1/episode", {
    content: `Synthetic observation for ${subject}: rollout mode is assisted.`,
    tags: ["http-example", "synthetic"],
  });
  require(typeof episode.id === "string" && episode.id.length > 0, "episode.id must be a string");
  const episodeId = episode.id;

  const supportedClaim = await postJson(baseUrl, "/v1/claim", {
    subject,
    predicate,
    object: "assisted",
    source_episode_id: episodeId,
    confidence: 0.92,
  });
  require(
    typeof supportedClaim.id === "string" && supportedClaim.id.length > 0,
    "supported claim.id must be a string",
  );
  const supportedClaimId = supportedClaim.id;

  const supportedRecall = await postJson(baseUrl, "/v1/recall", {
    query: `${subject} rollout mode assisted`,
    limit: 10,
    require_evidence: true,
  });
  const supportedAuthority = validateAuthority(supportedRecall.authority, "supported_recall.authority");
  validateHealth(supportedRecall.health, "supported_recall.health");
  validateRecallResults(supportedRecall, "supported_recall");
  require(
    supportedAuthority.mode === "certify" && supportedAuthority.can_trust === true,
    "the clean, evidence-backed store should certify",
  );
  const supportedResult = findResult(supportedRecall, supportedClaimId);
  require(
    supportedResult.evidence_id === episodeId,
    "the supported result must cite its source episode",
  );
  const supportedTrust = validateResultTrust(supportedResult.trust, "supported_result.trust");
  require(
    supportedTrust.mode === "certify" && supportedTrust.can_trust === true,
    "the evidence-backed result should certify",
  );

  const unsupportedClaim = await postJson(baseUrl, "/v1/claim", {
    subject,
    predicate,
    object: "manual",
    confidence: 0.91,
  });
  require(
    typeof unsupportedClaim.id === "string" && unsupportedClaim.id.length > 0,
    "unsupported claim.id must be a string",
  );
  const unsupportedClaimId = unsupportedClaim.id;

  const guardedRecall = await postJson(baseUrl, "/v1/recall", {
    query: `${subject} rollout mode manual`,
    limit: 10,
  });
  const guardedAuthority = validateAuthority(guardedRecall.authority, "guarded_recall.authority");
  const guardedHealth = validateHealth(guardedRecall.health, "guarded_recall.health");
  validateRecallResults(guardedRecall, "guarded_recall");
  require(
    guardedAuthority.can_trust === false && guardedAuthority.mode !== "certify",
    "an unsupported competing assertion must prevent store certification",
  );
  require(Array.isArray(guardedHealth.signals), "guarded_recall.health.signals must be an array");
  const healthSignalKinds = new Set(
    guardedHealth.signals.map((rawSignal) => requireObject(rawSignal, "guarded_recall.health.signals[]").kind),
  );
  require(healthSignalKinds.has("unsupported_fact"), "guarded health must identify the unsupported assertion");
  require(healthSignalKinds.has("conflicting_fact"), "guarded health must identify the competing values");

  const unsupportedResult = findResult(guardedRecall, unsupportedClaimId);
  require(
    unsupportedResult.evidence_id === null,
    "the unsupported result must not invent an evidence identifier",
  );
  const unsupportedTrust = validateResultTrust(unsupportedResult.trust, "unsupported_result.trust");
  require(
    unsupportedTrust.can_trust === false && unsupportedTrust.mode !== "certify",
    "the unsupported competing result must carry a non-trust verdict",
  );

  console.log(
    JSON.stringify(
      {
        client: "typescript",
        evidence_backed_result: {
          store_mode: supportedAuthority.mode,
          result_mode: supportedTrust.mode,
          evidence_id_present: true,
        },
        synthetic_competing_assertion: {
          store_mode: guardedAuthority.mode,
          store_can_trust: guardedAuthority.can_trust,
          result_mode: unsupportedTrust.mode,
          result_can_trust: unsupportedTrust.can_trust,
          signal_kinds: unsupportedTrust.signal_kinds,
        },
      },
      null,
      2,
    ),
  );
}

await main();
