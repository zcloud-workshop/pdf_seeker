/// <reference lib="webworker" />
/**
 * Diff worker: runs diffPagesCore off the main thread with WORKER_LIMITS.
 *
 * Task length and memory are bounded by those limits; a new "run" (or a
 * "cancel") supersedes the active one, which aborts at the next page
 * boundary (pages yield to the message queue via setTimeout).
 */

import {
  diffPagesCore,
  WORKER_LIMITS,
  type DiffWorkerRequest,
} from "./page-diff";

let activeRunId = 0;
let lastYield = Date.now();

async function yieldToMessages(): Promise<void> {
  if (Date.now() - lastYield < 20) return;
  lastYield = Date.now();
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
}

const ctx = self as unknown as DedicatedWorkerGlobalScope;

ctx.onmessage = async (ev: MessageEvent<DiffWorkerRequest>) => {
  const msg = ev.data;
  if (!msg || typeof msg !== "object") return;
  if (msg.type === "cancel") {
    activeRunId = 0;
    return;
  }
  if (msg.type !== "run") return;
  const id = msg.id;
  activeRunId = id;
  lastYield = Date.now();
  const outcome = await diffPagesCore(msg.textsA, msg.textsB, WORKER_LIMITS, {
    isCurrent: () => activeRunId === id,
    yieldIfBusy: yieldToMessages,
  });
  if (activeRunId !== id) return; // superseded while running
  ctx.postMessage({ type: "result", id, outcome });
};
