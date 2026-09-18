import { describe, it, expect } from "vitest";
import {
  COMPRESSION_PRESETS,
  estimateSavings,
  formatCompressionSummary,
} from "@/models/compression";

describe("Compression Presets & Estimation (Phase 2)", () => {
  it("defines all four required presets with features and badges", () => {
    expect(COMPRESSION_PRESETS.balanced).toBeDefined();
    expect(COMPRESSION_PRESETS.high_quality).toBeDefined();
    expect(COMPRESSION_PRESETS.max_compression).toBeDefined();
    expect(COMPRESSION_PRESETS.grayscale).toBeDefined();

    expect(COMPRESSION_PRESETS.balanced.badge).toBe("推荐");
    expect(COMPRESSION_PRESETS.high_quality.badge).toBe("无损");
    expect(COMPRESSION_PRESETS.max_compression.badge).toBe("极致");
    expect(COMPRESSION_PRESETS.grayscale.badge).toBe("归档");
  });

  it("calculates realistic estimated savings for balanced preset", () => {
    const originalBytes = 10 * 1024 * 1024; // 10MB
    const est = estimateSavings(originalBytes, "balanced");

    expect(est.ratioRange).toBe("25% ~ 45%");
    expect(est.minSaved).toBe(Math.round(originalBytes * 0.25));
    expect(est.maxSaved).toBe(Math.round(originalBytes * 0.45));
    expect(est.minFinal).toBeLessThan(originalBytes);
    expect(est.maxFinal).toBeLessThan(originalBytes);
  });

  it("calculates conservative savings for high_quality preset", () => {
    const originalBytes = 5 * 1024 * 1024; // 5MB
    const est = estimateSavings(originalBytes, "high_quality");

    expect(est.ratioRange).toBe("10% ~ 25%");
    expect(est.minSaved).toBeLessThan(est.maxSaved);
  });

  it("calculates maximum savings for max_compression preset", () => {
    const originalBytes = 20 * 1024 * 1024; // 20MB
    const est = estimateSavings(originalBytes, "max_compression");

    expect(est.ratioRange).toBe("40% ~ 70%");
    expect(est.maxSaved).toBe(Math.round(originalBytes * 0.70));
  });

  it("formats compression summary correctly", () => {
    const original = 10000;
    const compressed = 6000;
    const summary = formatCompressionSummary(original, compressed);

    expect(summary.ratioPercent).toBe(40);
    expect(summary.bytesSaved).toBe(4000);
    expect(summary.summaryText).toContain("节省 40.0%");
  });
});

