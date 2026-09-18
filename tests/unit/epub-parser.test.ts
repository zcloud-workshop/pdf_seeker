import { describe, it, expect } from "vitest";
import { formatTextToHtmlPages, MiniZip } from "@/utils/epub-parser";

describe("EPUB & Multi-format Parser", () => {
  it("formats text and markdown to structured HTML pages", () => {
    const text = `# Chapter 1: Introduction\n\nThis is a sample document for PDF Seeker.\n\n## Section 1.1\n- Feature A\n- Feature B`;
    const html = formatTextToHtmlPages(text, "Test Book");

    expect(html).toContain("<h1");
    expect(html).toContain("Test Book");
    expect(html).toContain("Chapter 1: Introduction");
    expect(html).toContain("Section 1.1");
    expect(html).toContain("<li");
    expect(html).toContain("Feature A");
  });

  it("handles empty or special character texts cleanly without XSS", () => {
    const dangerousText = `<script>alert(1)</script> & "special"`;
    const html = formatTextToHtmlPages(dangerousText, "Secure Title");

    expect(html).not.toContain("<script>");
    expect(html).toContain("&lt;script&gt;");
    expect(html).toContain("&amp;");
  });

  it("MiniZip handles invalid buffers safely without crashing", async () => {
    const fakeZip = new Uint8Array([0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0]);
    const zip = new MiniZip(fakeZip);
    const entries = await zip.getEntries();
    expect(entries.size).toBe(0);
  });
});

