import { describe, it, expect } from "vitest";
import {
  computeVisiblePages,
  clampCanvasDimensions,
} from "../../src/lib/utils/virtual-viewport";

describe("Virtual Viewport Windowing (pdf.js parity)", () => {
  const pageHeights = [800, 800, 800, 800, 800]; // 5 pages, each 800px high
  const gap = 8;
  // Page 1: 0 - 800
  // Page 2: 808 - 1608
  // Page 3: 1616 - 2416
  // Page 4: 2424 - 3224
  // Page 5: 3232 - 4032

  it("handles empty pages array safely", () => {
    const res = computeVisiblePages(0, 600, []);
    expect(res).toEqual({
      visibleStart: 0,
      visibleEnd: 0,
      bufferedStart: 0,
      bufferedEnd: 0,
    });
  });

  it("detects the first page at scroll top 0", () => {
    const res = computeVisiblePages(0, 600, pageHeights, gap, 1);
    expect(res.visibleStart).toBe(1);
    expect(res.visibleEnd).toBe(1);
    expect(res.bufferedStart).toBe(1);
    expect(res.bufferedEnd).toBe(2);
  });

  it("detects multiple visible pages spanning the viewport", () => {
    // Viewport from 600 to 1400 touches page 1 (ends at 800) and page 2 (starts at 808)
    const res = computeVisiblePages(600, 800, pageHeights, gap, 1);
    expect(res.visibleStart).toBe(1);
    expect(res.visibleEnd).toBe(2);
    expect(res.bufferedStart).toBe(1);
    expect(res.bufferedEnd).toBe(3);
  });

  it("detects middle page with buffer on both sides", () => {
    // Viewport from 1700 to 2300 is inside page 3
    const res = computeVisiblePages(1700, 600, pageHeights, gap, 1);
    expect(res.visibleStart).toBe(3);
    expect(res.visibleEnd).toBe(3);
    expect(res.bufferedStart).toBe(2);
    expect(res.bufferedEnd).toBe(4);
  });

  it("clamps buffered window at the end of the document", () => {
    // Viewport near the bottom (page 5)
    const res = computeVisiblePages(3500, 600, pageHeights, gap, 2);
    expect(res.visibleStart).toBe(5);
    expect(res.visibleEnd).toBe(5);
    expect(res.bufferedStart).toBe(3);
    expect(res.bufferedEnd).toBe(5);
  });

  it("respects custom lookahead buffer count", () => {
    const res = computeVisiblePages(1700, 600, pageHeights, gap, 2);
    expect(res.visibleStart).toBe(3);
    expect(res.visibleEnd).toBe(3);
    expect(res.bufferedStart).toBe(1);
    expect(res.bufferedEnd).toBe(5);
  });
});

describe("Canvas Allocation Clamping (HiDPI & safe memory)", () => {
  it("keeps standard dimensions untouched if under limit", () => {
    const { canvasWidth, canvasHeight, effectiveDpr } = clampCanvasDimensions(
      600,
      800,
      2,
      4096
    );
    expect(canvasWidth).toBe(1200);
    expect(canvasHeight).toBe(1600);
    expect(effectiveDpr).toBe(2);
  });

  it("clamps extreme dimensions to maxDimension safely", () => {
    // 3000 x 4000 at DPR 2 = 6000 x 8000, exceeds 4096
    const { canvasWidth, canvasHeight, effectiveDpr } = clampCanvasDimensions(
      3000,
      4000,
      2,
      4096
    );
    expect(canvasHeight).toBe(4096);
    expect(canvasWidth).toBe(Math.floor(3000 * effectiveDpr));
    expect(effectiveDpr).toBeCloseTo(1.024, 2);
  });
});
