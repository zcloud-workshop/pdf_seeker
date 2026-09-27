/**
 * Remediation 04 — diff behavior tests.
 *
 * Verifies the linear-space Myers core produces valid, minimal diffs
 * (checked against a classic full-matrix LCS reference), handles the
 * documented edge cases, and enforces the step budget instead of
 * freezing.
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  diffLinesCounted,
  diffLinesBounded,
  splitLines,
  DiffBudgetError,
  type DiffOp,
} from "../../src/lib/comparison/diff-core";

// ─── Reference implementation (classic O(n·m) LCS length) ──────────────

function lcsLen(a: string[], b: string[]): number {
  const stride = b.length + 1;
  const dp = new Int32Array((a.length + 1) * stride);
  for (let i = a.length - 1; i >= 0; i--) {
    for (let j = b.length - 1; j >= 0; j--) {
      dp[i * stride + j] =
        a[i] === b[j]
          ? dp[(i + 1) * stride + j + 1] + 1
          : Math.max(dp[(i + 1) * stride + j], dp[i * stride + j + 1]);
    }
  }
  return dp[0];
}

function assertValidOps(a: string[], b: string[], ops: DiffOp[]): void {
  const ra: string[] = [];
  const rb: string[] = [];
  for (const op of ops) {
    if (op.type === "same") {
      assert.equal(op.a, op.b, "same op must carry equal lines");
      ra.push(op.a!);
      rb.push(op.b!);
    } else if (op.type === "del") {
      ra.push(op.a!);
    } else {
      rb.push(op.b!);
    }
  }
  assert.deepEqual(ra, a, "ops must reconstruct side A");
  assert.deepEqual(rb, b, "ops must reconstruct side B");
}

function assertMinimal(a: string[], b: string[], ops: DiffOp[]): void {
  const edits = ops.filter((op) => op.type !== "same").length;
  assert.equal(
    edits,
    a.length + b.length - 2 * lcsLen(a, b),
    "edit count must match the LCS optimum",
  );
}

// Deterministic PRNG so failures reproduce.
function mulberry32(seed: number): () => number {
  let t = seed >>> 0;
  return () => {
    t = (t + 0x6d2b79f5) >>> 0;
    let x = Math.imul(t ^ (t >>> 15), 1 | t);
    x = (x + Math.imul(x ^ (x >>> 7), 61 | x)) ^ x;
    return ((x ^ (x >>> 14)) >>> 0) / 4294967296;
  };
}

// ─── Property tests ─────────────────────────────────────────────────────

test("random small inputs: valid and minimal vs reference LCS", () => {
  const rng = mulberry32(20260927);
  const alphabet = ["alpha", "beta", "gamma", "delta", ""];
  for (let iter = 0; iter < 500; iter++) {
    const n = Math.floor(rng() * 30);
    const m = Math.floor(rng() * 30);
    const a = Array.from({ length: n }, () => alphabet[Math.floor(rng() * alphabet.length)]);
    const b = Array.from({ length: m }, () => alphabet[Math.floor(rng() * alphabet.length)]);
    const { ops } = diffLinesCounted(a, b, 2_000_000);
    assertValidOps(a, b, ops);
    assertMinimal(a, b, ops);
  }
});

test("random medium inputs: valid and minimal vs reference LCS", () => {
  const rng = mulberry32(42);
  const alphabet = ["l0", "l1", "l2"];
  for (let iter = 0; iter < 25; iter++) {
    const n = 120 + Math.floor(rng() * 80);
    const m = 120 + Math.floor(rng() * 80);
    const a = Array.from({ length: n }, () => alphabet[Math.floor(rng() * alphabet.length)]);
    const b = Array.from({ length: m }, () => alphabet[Math.floor(rng() * alphabet.length)]);
    const { ops } = diffLinesCounted(a, b, 10_000_000);
    assertValidOps(a, b, ops);
    assertMinimal(a, b, ops);
  }
});

// ─── Edge cases ─────────────────────────────────────────────────────────

test("identical inputs produce only same ops with near-zero steps", () => {
  const a = Array.from({ length: 20_000 }, (_, i) => `line-${i}`);
  const { ops, steps } = diffLinesCounted(a, [...a], 100_000);
  assert.equal(ops.length, a.length);
  assert.ok(ops.every((op) => op.type === "same"));
  assert.equal(steps, 0, "common prefix/suffix trim should avoid the grid walk");
});

test("empty inputs", () => {
  assert.deepEqual(diffLinesBounded([], [], 100), []);
  const addOps = diffLinesBounded([], ["x", "y"], 100);
  assert.deepEqual(addOps, [
    { type: "add", b: "x" },
    { type: "add", b: "y" },
  ]);
  const delOps = diffLinesBounded(["x"], [], 100);
  assert.deepEqual(delOps, [{ type: "del", a: "x" }]);
});

test("inputs with no common line del all then add all", () => {
  const ops = diffLinesBounded(["a1", "a2"], ["b1", "b2", "b3"], 100_000);
  assert.deepEqual(ops, [
    { type: "del", a: "a1" },
    { type: "del", a: "a2" },
    { type: "add", b: "b1" },
    { type: "add", b: "b2" },
    { type: "add", b: "b3" },
  ]);
});

test("single changed line between long equal sections", () => {
  const a = Array.from({ length: 500 }, (_, i) => `common-${i}`);
  const b = [...a];
  b[250] = "CHANGED";
  const ops = diffLinesBounded(a, b, 5_000_000);
  assertValidOps(a, b, ops);
  assertMinimal(a, b, ops);
  assert.equal(ops.filter((op) => op.type !== "same").length, 2);
});

test("extremely unbalanced inputs stay bounded and valid", () => {
  const a = Array.from({ length: 3_000 }, (_, i) => `only-a-${i}`);
  const b = ["x", "y", "z"];
  const { ops, steps } = diffLinesCounted(a, b, 50_000_000);
  assertValidOps(a, b, ops);
  assertMinimal(a, b, ops);
  // No-common inputs resolve through ran-off diagonals, not the full grid.
  assert.ok(steps < 5_000_000, `steps unexpectedly high: ${steps}`);
});

test("splitLines normalizes CRLF and CR", () => {
  assert.deepEqual(splitLines("a\r\nb\rc\nd"), ["a", "b", "c", "d"]);
});

// ─── Budget enforcement ─────────────────────────────────────────────────

test("step budget throws DiffBudgetError instead of running forever", () => {
  const rng = mulberry32(7);
  const a = Array.from({ length: 400 }, () => `a${Math.floor(rng() * 1000)}`);
  const b = Array.from({ length: 400 }, () => `b${Math.floor(rng() * 1000)}`);
  assert.throws(() => diffLinesBounded(a, b, 10), DiffBudgetError);
});

test("budget large enough for page-sized text completes", () => {
  const page = Array.from({ length: 2_000 }, (_, i) => `text line ${i}`);
  const { ops } = diffLinesCounted(page, [...page.slice(0, 100), ...page.slice(101)], 8_000_000);
  assert.ok(ops.length > 0);
});
