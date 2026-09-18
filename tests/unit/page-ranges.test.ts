import { describe, it, expect } from "vitest";

export function parsePageRangeString(rangeStr: string, maxPages: number): number[] {
  const pagesSet = new Set<number>();
  if (!rangeStr || !rangeStr.trim()) return [];

  const parts = rangeStr.split(",");
  for (const part of parts) {
    const trimmed = part.trim();
    if (!trimmed) continue;

    if (trimmed.includes("-")) {
      const [startStr, endStr] = trimmed.split("-").map((s) => s.trim());
      const start = parseInt(startStr, 10);
      const end = parseInt(endStr, 10);

      if (!isNaN(start) && !isNaN(end)) {
        const from = Math.max(1, Math.min(start, end));
        const to = Math.min(maxPages, Math.max(start, end));
        for (let p = from; p <= to; p++) {
          pagesSet.add(p);
        }
      }
    } else {
      const single = parseInt(trimmed, 10);
      if (!isNaN(single) && single >= 1 && single <= maxPages) {
        pagesSet.add(single);
      }
    }
  }

  return Array.from(pagesSet).sort((a, b) => a - b);
}

describe("Page Range Parser Logic", () => {
  const TOTAL_PAGES = 10;

  it("parses single page numbers", () => {
    expect(parsePageRangeString("1", TOTAL_PAGES)).toEqual([1]);
    expect(parsePageRangeString("5", TOTAL_PAGES)).toEqual([5]);
  });

  it("parses comma-separated page numbers", () => {
    expect(parsePageRangeString("1, 3, 7", TOTAL_PAGES)).toEqual([1, 3, 7]);
    expect(parsePageRangeString(" 2 ,  4 , 6 ", TOTAL_PAGES)).toEqual([2, 4, 6]);
  });

  it("parses hypenated page intervals", () => {
    expect(parsePageRangeString("2-5", TOTAL_PAGES)).toEqual([2, 3, 4, 5]);
    // Inverted range should automatically normalize
    expect(parsePageRangeString("5-2", TOTAL_PAGES)).toEqual([2, 3, 4, 5]);
  });

  it("parses composite complex range expressions", () => {
    const res = parsePageRangeString("1-3, 5, 8-10", TOTAL_PAGES);
    expect(res).toEqual([1, 2, 3, 5, 8, 9, 10]);
  });

  it("deduplicates overlapping intervals and sorts ascending", () => {
    const res = parsePageRangeString("1-4, 3-6, 2", TOTAL_PAGES);
    expect(res).toEqual([1, 2, 3, 4, 5, 6]);
  });

  it("strictly filters out-of-bounds pages (> maxPages or < 1)", () => {
    const res = parsePageRangeString("0, 5, 12, 15-20", TOTAL_PAGES);
    expect(res).toEqual([5]);

    const bounded = parsePageRangeString("8-15", TOTAL_PAGES);
    expect(bounded).toEqual([8, 9, 10]);
  });

  it("returns empty array for invalid or empty inputs", () => {
    expect(parsePageRangeString("", TOTAL_PAGES)).toEqual([]);
    expect(parsePageRangeString("   ", TOTAL_PAGES)).toEqual([]);
    expect(parsePageRangeString("abc, foo-bar", TOTAL_PAGES)).toEqual([]);
  });
});

