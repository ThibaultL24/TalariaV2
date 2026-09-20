// sidecar/intuition/publish.ts
// Re-export — single official publish path lives in publisher.ts.

export { publishDebate } from "./publisher.ts";
export type { PublishResult, PublishOk, PublishFail } from "./publisher.ts";
