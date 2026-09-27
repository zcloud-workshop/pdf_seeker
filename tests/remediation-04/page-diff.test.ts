/**
 * Remediation 04 — page pairing, budgets and cancellation tests.
 *
 * Verifies: same-page-number pairing with onlyA/onlyB, the noText state
 * for pages without a text layer on either side, pre-allocation size
 * budgets, per-page/run budgets, wall-clock limits, and cooperative
 * cancellation between pages.
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  diffPagesCore,
  WORKER_LIMITS,
  type DiffRunLimits,
} from "../../src/lib/comparison/page-diff";

function lines(n: number, prefix = "l"): string {
  return Array.from({ length: n }, (_, i) => `${prefix}${i}`).join("\n");
}

const alwaysCurrent = { isCurrent: () => true };

test("pairs by page number and classifies onlyA/onlyB with raw text", async () => {
  const outcome = await diffPagesCore(
    ["p1", "p2", "p3"],
    ["p1", "q2"],
    WORKER_LIMITS,
    alwaysCurrent,
  );
  assert.equal(outcome.canceled, false);
  assert.deepEqual(
    outcome.pages.map((p) => [p.index, p.status]),
    [
      [1, "same"],
      [2, "changed"],
      [3, "onlyA"],
    ],
  );
  assert.equal(outcome.pages[2].textA, "p3");
});

test("both sides blank text becomes noText, never same", async () => {
  const outcome = await diffPagesCore(
    ["", "   \n\t ", "real"],
    ["", "\n", "real"],
    WORKER_LIMITS,
    alwaysCurrent,
  );
  assert.equal(outcome.pages[0].status, "noText");
  assert.equal(outcome.pages[1].status, "noText");
  assert.equal(outcome.pages[2].status, "same");
  assert.equal(outcome.pages[0].ops, null);
});

test("one blank side against real text is a normal changed page", async () => {
  const outcome = await diffPagesCore([""], ["content"], WORKER_LIMITS, alwaysCurrent);
  assert.equal(outcome.pages[0].status, "changed");
});

test("identical multi-page documents report all same", async () => {
  const texts = [lines(50), lines(200), lines(10)];
  const outcome = await diffPagesCore(texts, [...texts], WORKER_LIMITS, alwaysCurrent);
  assert.ok(outcome.pages.every((p) => p.status === "same"));
  assert.equal(outcome.skippedCount, 0);
  assert.equal(outcome.stoppedEarly, false);
});

test("page exceeding the char budget is skipped before allocation", async () => {
  const limits: DiffRunLimits = { ...WORKER_LIMITS, maxPageChars: 100 };
  const big = lines(200); // > 100 chars combined with anything
  const outcome = await diffPagesCore([big, "ok"], ["ok", "ok"], limits, alwaysCurrent);
  assert.equal(outcome.pages[0].status, "skipped");
  assert.equal(outcome.pages[0].skipReason, "size");
  assert.equal(outcome.pages[1].status, "same");
  assert.equal(outcome.skippedCount, 1);
});

test("page exceeding the line budget is skipped before diffing", async () => {
  const limits: DiffRunLimits = { ...WORKER_LIMITS, maxPageLines: 10 };
  const many = lines(20);
  const outcome = await diffPagesCore([many, "a"], [many, "b"], limits, alwaysCurrent);
  assert.equal(outcome.pages[0].status, "skipped");
  assert.equal(outcome.pages[0].skipReason, "size");
  assert.equal(outcome.pages[1].status, "changed");
});

test("page exceeding the step budget is marked skipped(steps)", async () => {
  const limits: DiffRunLimits = { ...WORKER_LIMITS, maxPageSteps: 5 };
  const a = Array.from({ length: 60 }, (_, i) => `a${i}`).join("\n");
  const b = Array.from({ length: 60 }, (_, i) => `b${i}`).join("\n");
  const outcome = await diffPagesCore([a], [b], limits, alwaysCurrent);
  assert.equal(outcome.pages[0].status, "skipped");
  assert.equal(outcome.pages[0].skipReason, "steps");
});

test("run-wide step budget stops early and skips remaining comparable pages", async () => {
  const limits: DiffRunLimits = { ...WORKER_LIMITS, maxTotalSteps: 0 };
  const outcome = await diffPagesCore(
    ["x", "only-a-page"],
    ["y"],
    limits,
    alwaysCurrent,
  );
  // Structural statuses are still classified without diffing.
  assert.equal(outcome.pages[0].status, "skipped");
  assert.equal(outcome.pages[0].skipReason, "total");
  assert.equal(outcome.pages[1].status, "onlyA");
  assert.equal(outcome.stoppedEarly, true);
});

test("wall-clock budget skips pages once elapsed", async () => {
  let tick = 0;
  const limits: DiffRunLimits = { ...WORKER_LIMITS, maxRunMs: 100 };
  const outcome = await diffPagesCore(
    ["a", "b"],
    ["c", "d"],
    limits,
    { isCurrent: () => true, now: () => (tick += 1000) },
  );
  assert.ok(outcome.pages.every((p) => p.status === "skipped"));
  assert.equal(outcome.pages[0].skipReason, "time");
  assert.equal(outcome.stoppedEarly, true);
});

test("cancellation between pages aborts with canceled outcome", async () => {
  let current = true;
  const yields: number[] = [];
  const outcome = await diffPagesCore(
    ["p1", "p2", "p3"],
    ["p1", "p2", "p3"],
    WORKER_LIMITS,
    {
      isCurrent: () => current,
      yieldIfBusy: async () => {
        yields.push(1);
        current = false; // superseded right after the first yield
      },
    },
  );
  assert.equal(outcome.canceled, true);
  assert.equal(outcome.stoppedEarly, true);
  assert.ok(outcome.pages.length < 3, "canceled run must stop early");
  assert.ok(yields.length >= 1);
});

test("hooks are consulted for every comparable page", async () => {
  let checks = 0;
  await diffPagesCore(
    ["a", "b", "c"],
    ["a", "b", "c"],
    WORKER_LIMITS,
    {
      isCurrent: () => {
        checks++;
        return true;
      },
      yieldIfBusy: async () => {},
    },
  );
  assert.equal(checks, 3);
});
