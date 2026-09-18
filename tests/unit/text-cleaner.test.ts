import { describe, it, expect } from "vitest";
import {
  cleanHyphenation,
  mergeParagraphs,
  normalizeCjkSpacing,
  cleanOcrText,
  isCjk,
} from "../../src/lib/utils/text-cleaner";

describe("Text Post-Processing Pipeline (Umi-OCR parity)", () => {
  it("detects CJK characters correctly", () => {
    expect(isCjk("中")).toBe(true);
    expect(isCjk("文")).toBe(true);
    expect(isCjk("a")).toBe(false);
    expect(isCjk("1")).toBe(false);
    expect(isCjk(" ")).toBe(false);
  });

  it("removes cross-line hyphenation while preserving compound words", () => {
    const input = "The inter-\n   national organization was founded.";
    expect(cleanHyphenation(input)).toBe("The international organization was founded.");

    const compound = "state-of-the-art technology";
    expect(cleanHyphenation(compound)).toBe("state-of-the-art technology");
  });

  it("merges broken paragraph lines correctly for Latin and CJK", () => {
    const latin =
      "This is a single sentence that was split\nacross two lines by the scanner.\nAnd this is a new sentence.";
    const latinMerged = mergeParagraphs(latin);
    expect(latinMerged).toContain(
      "This is a single sentence that was split across two lines by the scanner."
    );
    expect(latinMerged).toContain("And this is a new sentence.");

    const cjk = "这是第一行中文内容需要继续\n拼接在一起而不要产生多余空格。这是下一句。";
    const cjkMerged = mergeParagraphs(cjk);
    expect(cjkMerged).toContain(
      "这是第一行中文内容需要继续拼接在一起而不要产生多余空格。"
    );
  });

  it("normalizes spacing between CJK and alphanumeric tokens", () => {
    const mixed = "PDF工具箱2026版本在macOS运行良好";
    expect(normalizeCjkSpacing(mixed)).toBe(
      "PDF 工具箱 2026 版本在 macOS 运行良好"
    );
  });

  it("runs the entire cleanOcrText pipeline end-to-end", () => {
    const raw = "The docu-\n  ment was created using PDF工具箱2026\nand scanned perfectly.";
    const cleaned = cleanOcrText(raw);
    expect(cleaned).toContain("The document was created using PDF 工具箱 2026 and scanned perfectly.");
  });
});

