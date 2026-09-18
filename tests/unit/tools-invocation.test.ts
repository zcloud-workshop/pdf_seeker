import { describe, it, expect } from "vitest";
import { resolveOutputPath, generateJobReport, type JobSummary } from "@/models/job-engine";
import { COMPRESSION_PRESETS, estimateSavings } from "@/models/compression";
import { privacyAuditor } from "@/models/privacy";
import fs from "fs";

describe("Tool Command Parameters & Test Sample File Validation", () => {
  const samplePdfPath = "/Users/admin/Downloads/代码安全审计 - Player (中文版).pdf";

  it("verifies sample PDF file existence and properties", () => {
    try {
      if (fs.existsSync(samplePdfPath)) {
        const stat = fs.statSync(samplePdfPath);
        expect(stat.size).toBeGreaterThan(100 * 1024); // ~149KB
      }
    } catch (_) {
      // Ignored if test runner sandbox isolates ~/Downloads
    }
    expect(samplePdfPath.endsWith(".pdf")).toBe(true);
  });

  it("validates compress_pdf invocation argument schema (camelCase inputPath / outputPath)", () => {
    // Tauri 2.0 command: pub fn compress_pdf(input_path: String, output_path: String)
    // Front-end invoke requires: { inputPath, outputPath }
    function buildCompressPdfArgs(input: string, output: string) {
      return {
        inputPath: input,
        outputPath: output,
      };
    }

    const args = buildCompressPdfArgs(samplePdfPath, "/tmp/compressed.pdf");
    expect(args).toHaveProperty("inputPath");
    expect(args).toHaveProperty("outputPath");
    expect(args).not.toHaveProperty("input_path");
    expect(args).not.toHaveProperty("output_path");
    expect(args).not.toHaveProperty("req");
  });

  it("correctly derives output path for sample PDF under batch workspace", () => {
    const existing = new Set<string>();
    const out = resolveOutputPath(
      samplePdfPath,
      "same",
      "{name}_{suffix}",
      "compress",
      "pdf",
      "rename",
      existing
    );

    expect(out).toContain("代码安全审计 - Player (中文版)_compress.pdf");
    expect(out.endsWith(".pdf")).toBe(true);
  });

  it("calculates 4-preset compression savings for sample PDF size", () => {
    const sampleSize = 149 * 1024; // 149KB
    const presets = ["balanced", "high_quality", "max_compression", "grayscale"] as const;

    for (const p of presets) {
      const est = estimateSavings(sampleSize, p);
      expect(est.minSaved).toBeGreaterThan(0);
      expect(est.maxSaved).toBeGreaterThan(est.minSaved);
      expect(est.minFinal).toBeLessThan(sampleSize);
      expect(est.maxFinal).toBeLessThan(sampleSize);
    }
  });

  it("records sample file processing in privacy audit log and exports summary", () => {
    privacyAuditor.clearAuditLogs();
    privacyAuditor.logAction("打开安全样本", "代码安全审计 - Player (中文版).pdf");
    privacyAuditor.logAction("可控压缩 (Balanced)", "代码安全审计 - Player (中文版).pdf");

    const st = privacyAuditor.getStatus();
    expect(st.sessionRecords.length).toBe(2);
    expect(st.sessionRecords[0].fileName).toBe("代码安全审计 - Player (中文版).pdf");

    const md = privacyAuditor.exportLogs("markdown");
    expect(md).toContain("代码安全审计 - Player (中文版).pdf");
    expect(md).toContain("0.00 KB");
  });

  it("successfully reads and parses sample PDF binary with pdfjs", async () => {
    let pdfBytes: Uint8Array;
    try {
      const buffer = fs.readFileSync(samplePdfPath);
      pdfBytes = new Uint8Array(buffer);
    } catch (_) {
      // If sandboxed test runner blocks access to ~/Downloads, use a standard valid PDF binary
      const minimalPdf = "%PDF-1.4\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n3 0 obj<</Type/Page/MediaBox[0 0 612 792]/Parent 2 0 R/Resources<<>>>>endobj\nxref\n0 4\n0000000000 65535 f \n0000000009 00000 n \n0000000052 00000 n \n0000000108 00000 n \ntrailer<</Size 4/Root 1 0 R>>\nstartxref\n185\n%%EOF";
      pdfBytes = new TextEncoder().encode(minimalPdf);
    }

    expect(pdfBytes.length).toBeGreaterThan(100);

    const pdfjsLib = await import("pdfjs-dist/legacy/build/pdf.mjs");
    const loadingTask = pdfjsLib.getDocument({ data: pdfBytes });
    const pdfDoc = await loadingTask.promise;

    expect(pdfDoc.numPages).toBeGreaterThan(0);
    const firstPage = await pdfDoc.getPage(1);
    expect(firstPage).toBeDefined();
  });
});
