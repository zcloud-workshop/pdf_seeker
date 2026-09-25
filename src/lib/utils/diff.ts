/** Line-level LCS diff (no external deps, offline-safe). */

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

/** Classic LCS table diff over two line arrays. O(n·m) — fine for page-sized texts. */
export function diffLines(a: string[], b: string[]): DiffOp[] {
  const n = a.length;
  const m = b.length;
  // lcs[i][j] = LCS length of a[i..] and b[j..]
  const lcs: Int32Array[] = Array.from({ length: n + 1 }, () =>
    new Int32Array(m + 1),
  );
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      lcs[i][j] =
        a[i] === b[j]
          ? lcs[i + 1][j + 1] + 1
          : Math.max(lcs[i + 1][j], lcs[i][j + 1]);
    }
  }

  const ops: DiffOp[] = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (a[i] === b[j]) {
      ops.push({ type: "same", a: a[i], b: b[j] });
      i++;
      j++;
    } else if (lcs[i + 1][j] >= lcs[i][j + 1]) {
      ops.push({ type: "del", a: a[i] });
      i++;
    } else {
      ops.push({ type: "add", b: b[j] });
      j++;
    }
  }
  while (i < n) ops.push({ type: "del", a: a[i++] });
  while (j < m) ops.push({ type: "add", b: b[j++] });
  return ops;
}

export function opsHaveChanges(ops: DiffOp[]): boolean {
  return ops.some((op) => op.type !== "same");
}
