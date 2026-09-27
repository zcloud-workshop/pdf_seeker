/** Strict page-range parsing (R15). Rejects inputs the old parseInt-based
 * parser silently accepted or truncated: `3abc`, `0`, empty items, reversed
 * or out-of-bounds ranges, and selections whose expansion exceeds a budget.
 * Pure logic — runs directly under node in tests/remediation-03. */

export interface PageRangeResult {
  pages?: number[];
  error?: string;
}

/** Max pages a single selection may expand to before it is rejected. */
export const MAX_EXPAND_PAGES = 2000;

const PART_RE = /^(\d+)(?:\s*-\s*(\d+))?$/;

function inBounds(n: number, totalPages: number | undefined, what: string): string | null {
  if (n < 1) return `${what} ${n}: page numbers start at 1`;
  if (totalPages !== undefined && n > totalPages) {
    return `${what} ${n}: document only has ${totalPages} page(s)`;
  }
  return null;
}

/** Parse a page selection like "1,3,5-7". Returns sorted unique pages, or a
 * human-readable error. `totalPages` bounds-checks when known. */
export function parsePageSelection(
  input: string,
  totalPages?: number,
  maxPages: number = MAX_EXPAND_PAGES,
): PageRangeResult {
  const trimmed = input.trim();
  if (!trimmed) return { error: "No pages entered" };

  const pages: number[] = [];
  for (const rawPart of trimmed.split(",")) {
    const part = rawPart.trim();
    const m = part.match(PART_RE);
    if (!m) return { error: `Invalid page "${rawPart.trim()}"` };
    if (m[2] === undefined) {
      const n = parseInt(m[1], 10);
      const err = inBounds(n, totalPages, "Page");
      if (err) return { error: err };
      pages.push(n);
    } else {
      const a = parseInt(m[1], 10);
      const b = parseInt(m[2], 10);
      if (a > b) return { error: `Range ${a}-${b}: start is after the end` };
      let err = inBounds(a, totalPages, "Range start");
      if (err) return { error: err };
      err = inBounds(b, totalPages, "Range end");
      if (err) return { error: err };
      for (let i = a; i <= b; i++) pages.push(i);
    }
  }

  const unique = [...new Set(pages)].sort((x, y) => x - y);
  if (unique.length === 0) return { error: "No pages selected" };
  if (unique.length > maxPages) {
    return { error: `Selection expands to ${unique.length} pages (limit ${maxPages})` };
  }
  return { pages: unique };
}

export interface RangeGroupsResult {
  groups?: number[][];
  error?: string;
}

/** Parse split-style range groups like "1-3,4-6,7". Each comma part becomes
 * one output group; syntax and (when known) bounds are validated up front. */
export function parseRangeGroups(
  input: string,
  totalPages?: number,
  maxPages: number = MAX_EXPAND_PAGES,
): RangeGroupsResult {
  const trimmed = input.trim();
  if (!trimmed) return { error: "No ranges entered" };

  const groups: number[][] = [];
  let total = 0;
  for (const rawPart of trimmed.split(",")) {
    const part = rawPart.trim();
    const m = part.match(PART_RE);
    if (!m) return { error: `Invalid range "${rawPart.trim()}"` };
    if (m[2] === undefined) {
      const n = parseInt(m[1], 10);
      const err = inBounds(n, totalPages, "Page");
      if (err) return { error: err };
      groups.push([n]);
      total += 1;
    } else {
      const a = parseInt(m[1], 10);
      const b = parseInt(m[2], 10);
      if (a > b) return { error: `Range ${a}-${b}: start is after the end` };
      let err = inBounds(a, totalPages, "Range start");
      if (err) return { error: err };
      err = inBounds(b, totalPages, "Range end");
      if (err) return { error: err };
      const group: number[] = [];
      for (let i = a; i <= b; i++) group.push(i);
      groups.push(group);
      total += group.length;
    }
  }
  if (total === 0) return { error: "No pages in ranges" };
  if (total > maxPages) {
    return { error: `Ranges expand to ${total} pages (limit ${maxPages})` };
  }
  return { groups };
}
