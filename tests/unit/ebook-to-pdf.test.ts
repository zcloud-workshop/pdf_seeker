import { describe, it, expect } from "vitest";
import { isEbookFormat, buildPdfFromJpegs } from "@/utils/ebook-to-pdf";

describe("Ebook to PDF Converter", () => {
  it("correctly identifies supported ebook and document formats", () => {
    expect(isEbookFormat("novel.epub")).toBe(true);
    expect(isEbookFormat("comic.cbz")).toBe(true);
    expect(isEbookFormat("story.txt")).toBe(true);
    expect(isEbookFormat("readme.md")).toBe(true);
    expect(isEbookFormat("document.pdf")).toBe(false);
    expect(isEbookFormat("image.png")).toBe(false);
  });

  it("builds a valid PDF 1.4 binary buffer from mock jpeg page specs", () => {
    // Minimal mock 1x1 JPEG bytes: FFD8 ... FFD9
    const mockJpeg = new Uint8Array([0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10, 0x4a, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0xff, 0xd9]);

    const pages = [
      { jpegBytes: mockJpeg, width: 800, height: 1100 },
      { jpegBytes: mockJpeg, width: 800, height: 1100 },
    ];

    const pdfBytes = buildPdfFromJpegs(pages);
    expect(pdfBytes).toBeInstanceOf(Uint8Array);
    expect(pdfBytes.length).toBeGreaterThan(100);

    const pdfText = new TextDecoder().decode(pdfBytes);
    expect(pdfText).toContain("%PDF-1.4");
    expect(pdfText).toContain("/Type /Catalog");
    expect(pdfText).toContain("/Type /Pages");
    expect(pdfText).toContain("/Count 2");
    expect(pdfText).toContain("/Filter /DCTDecode");
    expect(pdfText).toContain("startxref");
    expect(pdfText).toContain("%%EOF");
  });

  it("handles empty pages gracefully by returning valid blank PDF", () => {
    const emptyPdf = buildPdfFromJpegs([]);
    const text = new TextDecoder().decode(emptyPdf);
    expect(text).toContain("%PDF-1.4");
    expect(text).toContain("%%EOF");
  });
});

