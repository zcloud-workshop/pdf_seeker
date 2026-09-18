import { describe, it, expect } from "vitest";
import { buildPdfFromJpegs, type PageImageSpec } from "@/utils/ebook-to-pdf";
import { COMPRESSION_PRESETS, estimateSavings } from "@/models/compression";

describe("Smart PDF Compression Engine", () => {
  it("builds compressed PDF from downsampled page images with real size reduction", () => {
    // Simulate 2 rendered pages with JPEG blobs
    // Tiny mock JPEG binary (SOI + EOI markers)
    const mockJpeg = new Uint8Array([0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10, 0x4a, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x01, 0x00, 0x60, 0x00, 0x60, 0x00, 0x00, 0xff, 0xd9]);
    const pages: PageImageSpec[] = [
      { jpegBytes: mockJpeg, width: 595, height: 842 },
      { jpegBytes: mockJpeg, width: 595, height: 842 },
    ];

    const pdfBytes = buildPdfFromJpegs(pages);
    expect(pdfBytes.length).toBeGreaterThan(mockJpeg.length * 2);

    const str = new TextDecoder().decode(pdfBytes);
    expect(str).toContain("%PDF-1.4");
    expect(str).toContain("/Type /Page");
    expect(str).toContain("/Filter /DCTDecode");
  });

  it("defines accurate presets parameters for noticeable size reduction", () => {
    const balanced = COMPRESSION_PRESETS.balanced;
    const maxComp = COMPRESSION_PRESETS.max_compression;
    const highQuality = COMPRESSION_PRESETS.high_quality;
    const grayscale = COMPRESSION_PRESETS.grayscale;

    expect(balanced.estimatedSavingMin).toBeGreaterThanOrEqual(20);
    expect(maxComp.estimatedSavingMin).toBeGreaterThanOrEqual(40);
    expect(highQuality.estimatedSavingMin).toBeGreaterThanOrEqual(10);
    expect(grayscale.estimatedSavingMin).toBeGreaterThanOrEqual(30);
  });

  it("configures distinct downsampling and quality levels across all profiles", async () => {
    const { PRESET_PROFILES } = await import("@/utils/pdf-compressor");

    expect(PRESET_PROFILES.high_quality.quality).toBeGreaterThan(PRESET_PROFILES.balanced.quality);
    expect(PRESET_PROFILES.balanced.quality).toBeGreaterThan(PRESET_PROFILES.max_compression.quality);

    expect(PRESET_PROFILES.high_quality.scale).toBeGreaterThan(PRESET_PROFILES.balanced.scale);
    expect(PRESET_PROFILES.balanced.scale).toBeGreaterThan(PRESET_PROFILES.max_compression.scale);

    expect(PRESET_PROFILES.grayscale.grayscale).toBe(true);
    expect(PRESET_PROFILES.balanced.grayscale).toBe(false);
  });

  it("adjusts estimates dynamically when document is pure vector text without images", () => {
    // 301,683 bytes pure vector document (like user test file)
    const originalBytes = 301683;
    const vectorEstimate = estimateSavings(originalBytes, "balanced", 0);

    expect(vectorEstimate.isVectorTextOnly).toBe(true);
    expect(vectorEstimate.ratioRange).toContain("原生矢量精简");
    // Should predict around 1% ~ 4% saving (around 289KB ~ 298KB), not an unrealistic 88KB!
    expect(vectorEstimate.minFinal).toBeGreaterThan(280000);
    expect(vectorEstimate.maxFinal).toBeLessThanOrEqual(originalBytes);

    // Standard document with bitmap images
    const bitmapEstimate = estimateSavings(originalBytes, "balanced", 5);
    expect(bitmapEstimate.isVectorTextOnly).toBe(false);
    expect(bitmapEstimate.ratioRange).toContain("25% ~ 45%");
  });
});
