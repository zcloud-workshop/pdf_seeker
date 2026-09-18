import { describe, it, expect } from "vitest";

interface SearchMatch {
  pageNum: number;
  text: string;
  index: number;
}

function findMatches(pages: string[], query: string, caseSensitive: boolean): SearchMatch[] {
  const q = query.trim();
  if (!q) return [];
  const results: SearchMatch[] = [];
  const target = caseSensitive ? q : q.toLowerCase();

  for (let i = 0; i < pages.length; i++) {
    const pageNum = i + 1;
    const pageText = pages[i];
    const compareText = caseSensitive ? pageText : pageText.toLowerCase();
    let startPos = 0;
    let foundIdx = 0;

    while ((foundIdx = compareText.indexOf(target, startPos)) !== -1) {
      results.push({
        pageNum,
        text: pageText.slice(foundIdx, foundIdx + q.length),
        index: results.length,
      });
      startPos = foundIdx + Math.max(1, target.length);
    }
  }

  return results;
}

describe("PDF Keyword Full-text Search", () => {
  const samplePages = [
    "PDF Seeker is a lightweight modern desktop pdf toolbox.",
    "Page 2 discusses OCR text recognition and table structure extraction.",
    "Page 3 summarizes the PDF Seeker roadmap for 2026.",
  ];

  it("finds case-insensitive occurrences across multiple pages", () => {
    const matches = findMatches(samplePages, "pdf", false);
    expect(matches.length).toBe(3);
    expect(matches[0].pageNum).toBe(1);
    expect(matches[1].pageNum).toBe(1);
    expect(matches[2].pageNum).toBe(3);
  });

  it("respects case sensitivity when enabled", () => {
    const matchesLower = findMatches(samplePages, "pdf", true);
    expect(matchesLower.length).toBe(1); // Only "pdf toolbox" on page 1

    const matchesUpper = findMatches(samplePages, "PDF", true);
    expect(matchesUpper.length).toBe(2); // "PDF Seeker" on page 1 & 3
  });

  it("handles empty queries gracefully", () => {
    const matches = findMatches(samplePages, "   ", false);
    expect(matches).toEqual([]);
  });

  it("returns correct page indices and offsets", () => {
    const matches = findMatches(samplePages, "table", false);
    expect(matches.length).toBe(1);
    expect(matches[0].pageNum).toBe(2);
    expect(matches[0].text).toBe("table");
  });
});
