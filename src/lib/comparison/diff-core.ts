/**
 * Line-level diff core: linear-space Myers O(ND) (Myers 1986 §4b, the
 * formulation popularized by diff-match-patch) with an explicit step budget.
 *
 * Memory is O(n + m) per bisection level, replacing the previous full-matrix
 * LCS that allocated an O(n·m) Int32 grid up front on the main thread.
 * Work is bounded by a step counter; exceeding it throws DiffBudgetError so
 * callers can degrade explicitly instead of freezing.
 */

export type DiffOp = {
  type: "same" | "add" | "del";
  /** Line from the A side (present for "same" and "del") */
  a?: string;
  /** Line from the B side (present for "same" and "add") */
  b?: string;
};

export function splitLines(text: string): string[] {
  return text.replace(/\r\n?/g, "\n").split("\n");
}

export class DiffBudgetError extends Error {
  constructor(public readonly reason: "steps") {
    super(`diff step budget exceeded (${reason})`);
    this.name = "DiffBudgetError";
  }
}

export interface StepCounter {
  n: number;
}

type BisectResult = { x: number; y: number } | "budget" | "no-common";

/**
 * Find the middle snake of a[a0..a1) vs b[b0..b1). Returns the split point
 * (absolute indices), "budget" when the step budget runs out, or "no-common"
 * when the two segments share no line at all.
 */
function bisect(
  a: string[],
  b: string[],
  a0: number,
  a1: number,
  b0: number,
  b1: number,
  maxSteps: number,
  steps: StepCounter,
): BisectResult {
  const n = a1 - a0;
  const m = b1 - b0;
  const maxD = Math.ceil((n + m) / 2);
  const vOffset = maxD;
  const vLength = 2 * maxD;
  const v1 = new Int32Array(vLength).fill(-1);
  const v2 = new Int32Array(vLength).fill(-1);
  v1[vOffset + 1] = 0;
  v2[vOffset + 1] = 0;
  const delta = n - m;
  // If the total number of lines is odd, the front path collides with the
  // reverse path; otherwise the collision is detected on the reverse pass.
  const front = delta % 2 !== 0;
  // Offsets for start and end of the k loops; prevent walking off the grid.
  let k1start = 0;
  let k1end = 0;
  let k2start = 0;
  let k2end = 0;
  for (let d = 0; d < maxD; d++) {
    // Walk the front path one step.
    for (let k1 = -d + k1start; k1 <= d - k1end; k1 += 2) {
      if (++steps.n > maxSteps) return "budget";
      const k1Offset = vOffset + k1;
      let x1: number;
      if (k1 === -d || (k1 !== d && v1[k1Offset - 1] < v1[k1Offset + 1])) {
        x1 = v1[k1Offset + 1];
      } else {
        x1 = v1[k1Offset - 1] + 1;
      }
      let y1 = x1 - k1;
      while (x1 < n && y1 < m && a[a0 + x1] === b[b0 + y1]) {
        x1++;
        y1++;
        if (++steps.n > maxSteps) return "budget";
      }
      v1[k1Offset] = x1;
      if (x1 > n) {
        k1end += 2;
      } else if (y1 > m) {
        k1start += 2;
      } else if (front) {
        const k2Offset = vOffset + delta - k1;
        if (k2Offset >= 0 && k2Offset < vLength && v2[k2Offset] !== -1) {
          const x2 = n - v2[k2Offset];
          if (x1 >= x2) return { x: a0 + x1, y: b0 + y1 };
        }
      }
    }
    // Walk the reverse path one step.
    for (let k2 = -d + k2start; k2 <= d - k2end; k2 += 2) {
      if (++steps.n > maxSteps) return "budget";
      const k2Offset = vOffset + k2;
      let x2: number;
      if (k2 === -d || (k2 !== d && v2[k2Offset - 1] < v2[k2Offset + 1])) {
        x2 = v2[k2Offset + 1];
      } else {
        x2 = v2[k2Offset - 1] + 1;
      }
      let y2 = x2 - k2;
      while (x2 < n && y2 < m && a[a1 - 1 - x2] === b[b1 - 1 - y2]) {
        x2++;
        y2++;
        if (++steps.n > maxSteps) return "budget";
      }
      v2[k2Offset] = x2;
      if (x2 > n) {
        k2end += 2;
      } else if (y2 > m) {
        k2start += 2;
      } else if (!front) {
        const k1Offset = vOffset + delta - k2;
        if (k1Offset >= 0 && k1Offset < vLength && v1[k1Offset] !== -1) {
          const x1 = v1[k1Offset];
          const y1 = vOffset + x1 - k1Offset;
          // Mirror x2/y2 onto the top-left coordinate system.
          const x2m = n - x2;
          if (x1 >= x2m) return { x: a0 + x1, y: b0 + y1 };
        }
      }
    }
  }
  // Loop exhausted without overlap: no common line at all.
  return "no-common";
}

function pushAll(out: DiffOp[], ops: DiffOp[]): void {
  for (const op of ops) out.push(op);
}

function walk(
  out: DiffOp[],
  a: string[],
  a0: number,
  a1: number,
  b: string[],
  b0: number,
  b1: number,
  maxSteps: number,
  steps: StepCounter,
): void {
  // Common prefix.
  while (a0 < a1 && b0 < b1 && a[a0] === b[b0]) {
    out.push({ type: "same", a: a[a0], b: b[b0] });
    a0++;
    b0++;
  }
  // Common suffix, collected backwards and appended after the recursive split.
  const suffix: DiffOp[] = [];
  while (a1 > a0 && b1 > b0 && a[a1 - 1] === b[b1 - 1]) {
    a1--;
    b1--;
    suffix.push({ type: "same", a: a[a1], b: b[b1] });
  }
  suffix.reverse();
  const n = a1 - a0;
  const m = b1 - b0;
  if (n === 0 && m === 0) {
    pushAll(out, suffix);
    return;
  }
  if (m === 0) {
    for (let i = a0; i < a1; i++) out.push({ type: "del", a: a[i] });
    pushAll(out, suffix);
    return;
  }
  if (n === 0) {
    for (let j = b0; j < b1; j++) out.push({ type: "add", b: b[j] });
    pushAll(out, suffix);
    return;
  }
  const mid = bisect(a, b, a0, a1, b0, b1, maxSteps, steps);
  if (mid === "budget") throw new DiffBudgetError("steps");
  if (mid === "no-common") {
    for (let i = a0; i < a1; i++) out.push({ type: "del", a: a[i] });
    for (let j = b0; j < b1; j++) out.push({ type: "add", b: b[j] });
    pushAll(out, suffix);
    return;
  }
  walk(out, a, a0, mid.x, b, b0, mid.y, maxSteps, steps);
  walk(out, a, mid.x, a1, b, mid.y, b1, maxSteps, steps);
  pushAll(out, suffix);
}

/** Diff two line arrays, returning ops plus the number of algorithm steps used. */
export function diffLinesCounted(
  a: string[],
  b: string[],
  maxSteps: number,
): { ops: DiffOp[]; steps: number } {
  const ops: DiffOp[] = [];
  const steps: StepCounter = { n: 0 };
  walk(ops, a, 0, a.length, b, 0, b.length, maxSteps, steps);
  return { ops, steps: steps.n };
}

/** Diff two line arrays; throws DiffBudgetError when maxSteps is exceeded. */
export function diffLinesBounded(
  a: string[],
  b: string[],
  maxSteps: number,
): DiffOp[] {
  return diffLinesCounted(a, b, maxSteps).ops;
}
