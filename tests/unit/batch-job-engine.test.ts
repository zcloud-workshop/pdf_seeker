import { describe, it, expect } from "vitest";
import {
  resolveOutputPath,
  formatBytes,
  generateJobReport,
  type JobSummary,
} from "@/models/job-engine";

describe("Batch Job Engine & File Policy", () => {
  it("formats bytes accurately across B, KB, MB, and GB", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(2048)).toBe("2.0 KB");
    expect(formatBytes(1024 * 1024 * 3.5)).toBe("3.50 MB");
    expect(formatBytes(1024 * 1024 * 1024 * 2)).toBe("2.00 GB");
  });

  describe("resolveOutputPath", () => {
    it("resolves output path in same directory with default template", () => {
      const input = "/Users/docs/report.pdf";
      const out = resolveOutputPath(input, "same", "{name}_{suffix}", "rotated", "pdf", "replace");
      expect(out).toBe("/Users/docs/report_rotated.pdf");
    });

    it("resolves output path in custom output directory", () => {
      const input = "/Users/docs/sample.pdf";
      const out = resolveOutputPath(input, "/Users/output", "{name}_{suffix}", "compressed", "pdf", "replace");
      expect(out).toBe("/Users/output/sample_compressed.pdf");
    });

    it("resolves text and image extensions correctly", () => {
      const input = "/Users/docs/contract.pdf";
      const txtOut = resolveOutputPath(input, "same", "{name}_{suffix}", "extracted", "txt", "replace");
      expect(txtOut).toBe("/Users/docs/contract_extracted.txt");

      const imgOut = resolveOutputPath(input, "same", "{name}_{suffix}", "page", "png", "replace");
      expect(imgOut).toBe("/Users/docs/contract_page.png");
    });

    it("handles rename conflict strategy by appending sequence numbers", () => {
      const existing = new Set<string>(["/Users/docs/invoice_compressed.pdf"]);
      const out = resolveOutputPath(
        "/Users/docs/invoice.pdf",
        "same",
        "{name}_{suffix}",
        "compressed",
        "pdf",
        "rename",
        existing
      );
      expect(out).toBe("/Users/docs/invoice_compressed_1.pdf");
    });

    it("handles multiple rename conflict collisions safely", () => {
      const existing = new Set<string>([
        "/Users/docs/invoice_compressed.pdf",
        "/Users/docs/invoice_compressed_1.pdf",
        "/Users/docs/invoice_compressed_2.pdf",
      ]);
      const out = resolveOutputPath(
        "/Users/docs/invoice.pdf",
        "same",
        "{name}_{suffix}",
        "compressed",
        "pdf",
        "rename",
        existing
      );
      expect(out).toBe("/Users/docs/invoice_compressed_3.pdf");
    });

    it("throws error when conflictStrategy is fail and file exists", () => {
      const existing = new Set<string>(["/Users/docs/file_out.pdf"]);
      expect(() => {
        resolveOutputPath("/Users/docs/file.pdf", "same", "{name}_out", "", "pdf", "fail", existing);
      }).toThrow(/already exists/);
    });
  });

  describe("generateJobReport", () => {
    const mockSummary: JobSummary = {
      id: "job_123",
      recipeName: "批量压缩优化",
      recipeType: "compress",
      totalFiles: 2,
      completedFiles: 2,
      failedFiles: 0,
      skippedFiles: 0,
      startTime: 100000,
      endTime: 102500,
      totalOriginalBytes: 10000000,
      totalOutputBytes: 6000000,
      bytesSaved: 4000000,
      items: [
        {
          filePath: "/path/to/doc1.pdf",
          fileName: "doc1.pdf",
          fileSizeBytes: 6000000,
          status: "done",
          outputPath: "/path/to/doc1_compressed.pdf",
          outputSizeBytes: 3600000,
          ratio: 40.0,
          durationMs: 1200,
          message: "节省 40.0%",
        },
        {
          filePath: "/path/to/doc2.pdf",
          fileName: "doc2.pdf",
          fileSizeBytes: 4000000,
          status: "done",
          outputPath: "/path/to/doc2_compressed.pdf",
          outputSizeBytes: 2400000,
          ratio: 40.0,
          durationMs: 1100,
          message: "节省 40.0%",
        },
      ],
    };

    it("generates structured CSV report", () => {
      const csv = generateJobReport(mockSummary, "csv");
      expect(csv).toContain("Index,File Name,Status");
      expect(csv).toContain('"doc1.pdf",done,6000000,3600000,40.0%');
      expect(csv).toContain('"doc2.pdf",done,4000000,2400000,40.0%');
    });

    it("generates readable Markdown summary", () => {
      const md = generateJobReport(mockSummary, "markdown");
      expect(md).toContain("# PDF Seeker 批处理任务报告");
      expect(md).toContain("批量压缩优化");
      expect(md).toContain("`doc1.pdf`");
      expect(md).toContain("`doc2.pdf`");
      expect(md).toContain("| 1 | `doc1.pdf` | **done**");
    });
  });
});

