import { describe, expect, test } from "bun:test";

import { NahualiApiError, createNahualiClient } from "../src/index";

const CLIENT_ENDPOINTS = [
  "/v1/status",
  "/v1/openapi.json",
  "/v1/episode",
  "/v1/claim",
  "/v1/link",
  "/v1/procedure",
  "/v1/intention",
  "/v1/intention/update",
  "/v1/intention/status",
  "/v1/intention/reconcile",
  "/v1/goal-progress",
  "/v1/recall",
  "/v1/session-resume",
  "/v1/memory-health",
  "/v1/proactive",
  "/v1/deadlines",
  "/v1/anomalies",
  "/v1/anomaly/acknowledge",
  "/v1/graph",
  "/v1/timeline",
  "/v1/pending",
  "/v1/review/resolve",
  "/v1/projection/status",
  "/v1/projection/rebuild",
  "/v1/projection/validate",
  "/v1/semantic/status",
  "/v1/semantic/rebuild",
] as const;

describe("NahualiClient", () => {
  test("sends JSON requests to the local beta API", async () => {
    const calls: Request[] = [];
    const client = createNahualiClient({
      baseUrl: "http://127.0.0.1:7070/",
      fetch: async (input, init) => {
        const request = new Request(input, init);
        calls.push(request);
        return Response.json({ ok: true });
      },
    });

    await client.episode({
      content: "Lena owns the release notes.",
      tags: ["product"],
      mentions: ["Lena"],
    });

    expect(calls).toHaveLength(1);
    expect(calls[0].url).toBe("http://127.0.0.1:7070/v1/episode");
    expect(calls[0].method).toBe("POST");
    expect(calls[0].headers.get("content-type")).toBe("application/json");
    await expect(calls[0].json()).resolves.toEqual({
      content: "Lena owns the release notes.",
      tags: ["product"],
      mentions: ["Lena"],
    });
  });

  test("covers typed record and review endpoints", async () => {
    const calls: Request[] = [];
    const client = createNahualiClient({
      baseUrl: "http://127.0.0.1:7070",
      fetch: async (input, init) => {
        const request = new Request(input, init);
        calls.push(request);
        return Response.json({ ok: true });
      },
    });

    await client.link({
      from: "Lena",
      relation: "owns",
      to: "release notes",
      confidence: 0.9,
    });
    await client.procedure({
      name: "release-check",
      body: "Run the RC gate before cutting a beta.",
    });
    await client.resolveReview({
      review_id: "review_123",
      note: "Accepted after operator validation.",
      dry_run: true,
    });

    expect(calls.map((request) => request.url)).toEqual([
      "http://127.0.0.1:7070/v1/link",
      "http://127.0.0.1:7070/v1/procedure",
      "http://127.0.0.1:7070/v1/review/resolve",
    ]);
    expect(calls.map((request) => request.method)).toEqual(["POST", "POST", "POST"]);
    await expect(calls[0].json()).resolves.toEqual({
      from: "Lena",
      relation: "owns",
      to: "release notes",
      confidence: 0.9,
    });
    await expect(calls[1].json()).resolves.toEqual({
      name: "release-check",
      body: "Run the RC gate before cutting a beta.",
    });
    await expect(calls[2].json()).resolves.toEqual({
      review_id: "review_123",
      note: "Accepted after operator validation.",
      dry_run: true,
    });
  });

  test("serializes query endpoints without sending a body", async () => {
    const calls: Request[] = [];
    const client = createNahualiClient({
      baseUrl: "http://127.0.0.1:7070",
      fetch: async (input, init) => {
        const request = new Request(input, init);
        calls.push(request);
        return Response.json({ nodes: [] });
      },
    });

    await client.graph({ seed: "Lena", depth: 2, limit: 20 });

    expect(calls[0].url).toBe("http://127.0.0.1:7070/v1/graph?seed=Lena&depth=2&limit=20");
    expect(calls[0].method).toBe("GET");
    expect(calls[0].headers.get("content-type")).toBeNull();
    await expect(calls[0].text()).resolves.toBe("");
  });

  test("throws structured API errors", async () => {
    const client = createNahualiClient({
      baseUrl: "http://127.0.0.1:7070",
      fetch: async () =>
        Response.json(
          { error: { code: "empty_content", message: "memory content cannot be empty" } },
          { status: 400 },
        ),
    });

    await expect(client.episode({ content: " " })).rejects.toMatchObject({
      name: "NahualiApiError",
      status: 400,
      code: "empty_content",
      message: "memory content cannot be empty",
    } satisfies Partial<NahualiApiError>);
  });

  test("covers routes advertised by the frozen OpenAPI contract", async () => {
    const openapi = await Bun.file(new URL("../../../crates/nahuali-api/openapi.json", import.meta.url)).json();
    const paths = Object.keys(openapi.paths);
    const clientEndpoints = new Set<string>(CLIENT_ENDPOINTS);

    for (const endpoint of CLIENT_ENDPOINTS) {
      expect(paths).toContain(endpoint);
    }
    expect(paths.filter((path) => !clientEndpoints.has(path))).toEqual([]);
  });
});
