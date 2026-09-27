/** Line-level diff (no external deps, offline-safe).
 *
 * Compatibility wrapper: the algorithm now lives in @/comparison/diff-core
 * (linear-space Myers with a step budget) and the page-level orchestration
 * in @/comparison/page-diff. Compare.svelte consumes those directly; this
 * module keeps the historical sync API for any other caller.
 */

export { splitLines, DiffBudgetError } from "../comparison/diff-core";
export type { DiffOp } from "../comparison/diff-core";

import { diffLinesBounded, type DiffOp } from "../comparison/diff-core";
import { MAIN_THREAD_LIMITS } from "../comparison/page-diff";

/**
 * Classic diff over two line arrays. O(N+M) memory; work is capped by the
 * main-thread step budget and throws DiffBudgetError beyond it.
 */
export function diffLines(a: string[], b: string[]): DiffOp[] {
  return diffLinesBounded(a, b, MAIN_THREAD_LIMITS.maxPageSteps);
}

export function opsHaveChanges(ops: DiffOp[]): boolean {
  return ops.some((op) => op.type !== "same");
}
