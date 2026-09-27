/**
 * Diff service: owns the diff worker, keeps at most one active run, and
 * falls back to bounded main-thread diffing when workers are unavailable
 * (e.g. a future CSP restriction). A new run supersedes the previous one,
 * which resolves as null so callers can drop stale results.
 */

import {
  diffPagesCore,
  MAIN_THREAD_LIMITS,
  type DiffRunOutcome,
  type DiffWorkerRequest,
  type DiffWorkerResponse,
} from "./page-diff";

export interface DiffService {
  /**
   * Diff two page-text arrays. Resolves with the outcome, or null when the
   * run was superseded by a newer run / dispose.
   */
  run(textsA: string[], textsB: string[]): Promise<DiffRunOutcome | null>;
  dispose(): void;
}

interface PendingRun {
  id: number;
  textsA: string[];
  textsB: string[];
  resolve: (outcome: DiffRunOutcome | null) => void;
}

export function createDiffService(): DiffService {
  let worker: Worker | null = null;
  let workerBroken = false;
  let seq = 0;
  let inlineToken = 0;
  let inlineLastYield = 0;
  let pending: PendingRun | null = null;

  function ensureWorker(): Worker | null {
    if (workerBroken) return null;
    if (worker) return worker;
    try {
      const w = new Worker(new URL("./diff-worker.ts", import.meta.url), {
        type: "module",
      });
      w.onmessage = (ev: MessageEvent<DiffWorkerResponse>) => {
        const msg = ev.data;
        if (!msg || msg.type !== "result" || !pending || msg.id !== pending.id)
          return;
        const p = pending;
        pending = null;
        p.resolve(msg.outcome);
      };
      w.onerror = () => {
        // Worker died mid-run: terminate, mark broken, retry on main thread.
        w.terminate();
        if (worker === w) worker = null;
        workerBroken = true;
        const p = pending;
        pending = null;
        if (p) void runInline(p.textsA, p.textsB).then(p.resolve);
      };
      worker = w;
      return w;
    } catch {
      workerBroken = true;
      return null;
    }
  }

  async function runInline(
    textsA: string[],
    textsB: string[],
  ): Promise<DiffRunOutcome | null> {
    const token = inlineToken;
    const outcome = await diffPagesCore(textsA, textsB, MAIN_THREAD_LIMITS, {
      isCurrent: () => token === inlineToken,
      yieldIfBusy: async () => {
        if (Date.now() - inlineLastYield < 20) return;
        inlineLastYield = Date.now();
        await new Promise<void>((resolve) => setTimeout(resolve, 0));
      },
    });
    if (outcome.canceled || token !== inlineToken) return null;
    return { ...outcome, engineFailed: true };
  }

  return {
    run(textsA: string[], textsB: string[]): Promise<DiffRunOutcome | null> {
      const id = ++seq;
      inlineToken++;
      if (pending) {
        const p = pending;
        pending = null;
        p.resolve(null); // supersede: caller of the old run discards
      }
      const w = ensureWorker();
      if (!w) return runInline(textsA, textsB);
      if (worker) worker.postMessage({ type: "cancel" } satisfies DiffWorkerRequest);
      return new Promise<DiffRunOutcome | null>((resolve) => {
        pending = { id, textsA, textsB, resolve };
        w.postMessage({
          type: "run",
          id,
          textsA,
          textsB,
        } satisfies DiffWorkerRequest);
      });
    },
    dispose() {
      inlineToken++;
      if (pending) {
        const p = pending;
        pending = null;
        p.resolve(null);
      }
      worker?.terminate();
      worker = null;
    },
  };
}
