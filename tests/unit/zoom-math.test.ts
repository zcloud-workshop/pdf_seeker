import { describe, it, expect } from "vitest";
import { computeZoomLevel, computeScrollAnchor } from "@/utils/zoom-math";

describe("Zoom Math and Viewport Anchoring", () => {
  it("ignores wheel events without ctrlKey or metaKey", () => {
    const res = computeZoomLevel(1.0, { ctrlKey: false, metaKey: false, deltaMode: 0, deltaY: -10 });
    expect(res).toBe(1.0);
  });

  it("handles trackpad pinch zooming in smoothly", () => {
    const oldZ = 1.0;
    // deltaY = -5 (spreading fingers apart / pinch out to zoom in)
    const newZ = computeZoomLevel(oldZ, { ctrlKey: true, deltaMode: 0, deltaY: -5 });
    expect(newZ).toBeGreaterThan(oldZ);
    expect(newZ).toBeCloseTo(1.0 * Math.exp(5 * 0.016), 4);
  });

  it("handles trackpad pinch zooming out smoothly", () => {
    const oldZ = 1.5;
    // deltaY = +5 (pinching fingers together to zoom out)
    const newZ = computeZoomLevel(oldZ, { ctrlKey: true, deltaMode: 0, deltaY: 5 });
    expect(newZ).toBeLessThan(oldZ);
    expect(newZ).toBeCloseTo(1.5 * Math.exp(-5 * 0.016), 4);
  });

  it("performs linear 0.15 step for mouse wheel", () => {
    const oldZ = 1.0;
    // Mouse wheel up (deltaMode = 1 or deltaMode = 0 with large delta)
    const zoomedIn = computeZoomLevel(oldZ, { ctrlKey: true, deltaMode: 1, deltaY: -1 });
    expect(zoomedIn).toBe(1.15);

    const zoomedOut = computeZoomLevel(oldZ, { ctrlKey: true, deltaMode: 1, deltaY: 1 });
    expect(zoomedOut).toBe(0.85);
  });

  it("strictly respects lower clamp (0.25) and upper clamp (5.0)", () => {
    // Attempt to zoom out beyond minimum
    const tooSmall = computeZoomLevel(0.28, { ctrlKey: true, deltaMode: 1, deltaY: 100 });
    expect(tooSmall).toBe(0.25);

    // Attempt to zoom in beyond maximum
    const tooLarge = computeZoomLevel(4.9, { ctrlKey: true, deltaMode: 1, deltaY: -100 });
    expect(tooLarge).toBe(5.0);
  });

  it("discards negligible sub-pixel jitter (< 0.0005)", () => {
    const oldZ = 1.0;
    // Micro delta that results in < 0.0005 change
    const jitter = computeZoomLevel(oldZ, { ctrlKey: true, deltaMode: 0, deltaY: 0.0001 });
    expect(jitter).toBe(oldZ);
  });

  it("anchors scroll coordinates around the mouse cursor", () => {
    const oldZ = 1.0;
    const newZ = 2.0; // 2x magnification
    const viewport = {
      clientX: 200,
      clientY: 150,
      rectLeft: 50,
      rectTop: 50,
      scrollLeft: 100,
      scrollTop: 100,
    };

    const { newScrollLeft, newScrollTop } = computeScrollAnchor(oldZ, newZ, viewport);
    // cursorX = 200 - 50 + 100 = 250
    // newScrollLeft = 250 * 2 - (200 - 50) = 500 - 150 = 350
    expect(newScrollLeft).toBe(350);

    // cursorY = 150 - 50 + 100 = 200
    // newScrollTop = 200 * 2 - (150 - 50) = 400 - 100 = 300
    expect(newScrollTop).toBe(300);
  });
});

