/**
 * Page-level comparison: pairs pages by page number, applies per-page diff
 * budgets before any allocation, and reports explicit degraded states
 * ("noText" / "skipped") instead of pretending inputs are equal.
 *
 * The core is engine-agnostic: the web worker and the main-thread fallback
 * both call diffPagesCore with different limits and hooks.
 */

import {
  diffLinesCounted,
  splitLines,
  DiffBudgetError,
  type DiffOp,
} from "./diff-core";

export type PageStatus =
  | "same"
  | "changed"
  | "onlyA"
  | "onlyB"
  /** Both sides have no extractable text layer — visual equality NOT assessed. */
  | "noText"
  /** Over budget or run limit — not compared. */
  | "skipped";

export type SkipReason = "size" | "steps" | "total" | "time";

export interface PageDiff {
  /** 1-based page number */
  index: number;
  status: PageStatus;
  /** Line ops for same/changed pages; null otherwise. */
  ops: DiffOp[] | null;
  skipReason?: SkipReason;
  /** Raw page text for onlyA/onlyB display. */
  textA?: string;
  textB?: string;
}

export interface DiffRunLimits {
  /** Per-page combined character budget, checked before any splitting/allocation. */
  maxPageChars: number;
  /** Per-page combined line budget, checked before the diff arrays are used. */
  maxPageLines: number;
  /** Per-page algorithm step budget. */
  maxPageSteps: number;
  /** Whole-run step budget across all pages. */
  maxTotalSteps: number;
  /** Whole-run wall-clock budget in milliseconds. */
  maxRunMs: number;
}

/** Budgets used by the web worker (off the main thread). */
export const WORKER_LIMITS: DiffRunLimits = {
  maxPageChars: 4_000_000,
  maxPageLines: 200_000,
  maxPageSteps: 8_000_000,
  maxTotalSteps: 64_000_000,
  maxRunMs: 15_000,
};

/** Stricter budgets for the synchronous main-thread fallback. */
export const MAIN_THREAD_LIMITS: DiffRunLimits = {
  maxPageChars: 400_000,
  maxPageLines: 20_000,
  maxPageSteps: 500_000,
  maxTotalSteps: 2_000_000,
  maxRunMs: 4_000,
};

export interface DiffRunOutcome {
  pages: PageDiff[];
  /** True when the run stopped before reaching the last page (budget/time/cancel). */
  stoppedEarly: boolean;
  skippedCount: number;
  canceled: boolean;
  /** True when the worker engine failed and the main thread took over. */
  engineFailed: boolean;
}

export interface RunHooks {
  /** Return false to cancel the run as soon as possible. */
  isCurrent: () => boolean;
  /** Called between pages; implementations may yield to drain queued messages. */
  yieldIfBusy?: () => Promise<void>;
  now?: () => number;
}

export function isBlankPageText(text: string): boolean {
  return text.trim().length === 0;
}

export async function diffPagesCore(
  textsA: string[],
  textsB: string[],
  limits: DiffRunLimits,
  hooks: RunHooks,
): Promise<DiffRunOutcome> {
  const now = hooks.now ?? Date.now;
  const start = now();
  const pages: PageDiff[] = [];
  let totalSteps = 0;
  let skippedCount = 0;
  let stoppedEarly = false;

  const max = Math.max(textsA.length, textsB.length);
  for (let i = 0; i < max; i++) {
    const hasA = i < textsA.length;
    const hasB = i < textsB.length;
    if (hasA && !hasB) {
      pages.push({ index: i + 1, status: "onlyA", ops: null, textA: textsA[i] });
      continue;
    }
    if (!hasA && hasB) {
      pages.push({ index: i + 1, status: "onlyB", ops: null, textB: textsB[i] });
      continue;
    }
    const ta = textsA[i];
    const tb = textsB[i];
    if (isBlankPageText(ta) && isBlankPageText(tb)) {
      pages.push({ index: i + 1, status: "noText", ops: null });
      continue;
    }

    // Pre-allocation budget: reject on raw size before touching the text.
    if (ta.length + tb.length > limits.maxPageChars) {
      pages.push({ index: i + 1, status: "skipped", ops: null, skipReason: "size" });
      skippedCount++;
      continue;
    }

    // Whole-run budgets: once tripped, remaining comparable pages are skipped.
    if (totalSteps >= limits.maxTotalSteps || now() - start >= limits.maxRunMs) {
      stoppedEarly = true;
      pages.push({
        index: i + 1,
        status: "skipped",
        ops: null,
        skipReason: totalSteps >= limits.maxTotalSteps ? "total" : "time",
      });
      skippedCount++;
      continue;
    }

    if (hooks.yieldIfBusy) await hooks.yieldIfBusy();
    if (!hooks.isCurrent()) {
      return { pages, stoppedEarly: true, skippedCount, canceled: true, engineFailed: false };
    }

    const la = splitLines(ta);
    const lb = splitLines(tb);
    if (la.length + lb.length > limits.maxPageLines) {
      pages.push({ index: i + 1, status: "skipped", ops: null, skipReason: "size" });
      skippedCount++;
      continue;
    }
    try {
      const { ops, steps } = diffLinesCounted(la, lb, limits.maxPageSteps);
      totalSteps += steps;
      pages.push({
        index: i + 1,
        status: ops.some((op) => op.type !== "same") ? "changed" : "same",
        ops,
      });
    } catch (e) {
      if (e instanceof DiffBudgetError) {
        pages.push({ index: i + 1, status: "skipped", ops: null, skipReason: "steps" });
        skippedCount++;
      } else {
        throw e;
      }
    }
  }
  return { pages, stoppedEarly, skippedCount, canceled: false, engineFailed: false };
}

// ─── Worker message protocol (shared by diff-worker.ts and diff-service.ts) ──

export type DiffWorkerRequest =
  | { type: "run"; id: number; textsA: string[]; textsB: string[] }
  | { type: "cancel" };

export type DiffWorkerResponse = {
  type: "result";
  id: number;
  outcome: DiffRunOutcome;
};
