# Nahuali JavaScript

`@nahuali/client` is an unpublished beta client for the local Nahuali HTTP API.

The package is not published to npm and is marked `private`. It exists to keep
the June beta API contract usable from TypeScript without creating a hosted
client model or registry publication path.

## Usage

```ts
import { createNahualiClient } from "@nahuali/client";

const nahuali = createNahualiClient({ baseUrl: "http://127.0.0.1:7070" });

await nahuali.episode({
  content: "Lena owns the release notes.",
  tags: ["product"],
  mentions: ["Lena"],
});

const recall = await nahuali.recall({
  query: "Lena release",
  require_evidence: true,
});
```

## Scope

The client is a thin fetch wrapper over `/v1/*` endpoints exposed by
`nahuali-api`. It does not include auth, tenants, billing, sync, hosted control
plane behavior, or npm publication metadata.

Contract tests assert that the client methods map to paths in
`crates/nahuali-api/openapi.json`.

```bash
bun test --cwd packages/js
```
