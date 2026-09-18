import { describe, it, expect } from "vitest";

export interface TestToolDef {
  id: string;
  nameZh: string;
  nameEn: string;
  descZh: string;
  descEn: string;
  category: "page" | "edit" | "convert" | "extract";
  keywords: string[];
}

export const TOOLS_CATALOG: TestToolDef[] = [
  {
    id: "reorder",
    nameZh: "页面排序",
    nameEn: "Reorder Pages",
    descZh: "可视化拖拽重新排列页面顺序",
    descEn: "Visually drag and drop to reorder pages",
    category: "page",
    keywords: ["sort", "reorder", "arrange", "drag", "顺序", "排序", "重排"],
  },
  {
    id: "rotate",
    nameZh: "旋转页面",
    nameEn: "Rotate Pages",
    descZh: "顺时针或逆时针旋转当前页或全部页面",
    descEn: "Rotate current or all pages 90°, 180°, 270°",
    category: "page",
    keywords: ["rotate", "orientation", "turn", "旋转", "翻转", "方向"],
  },
  {
    id: "merge",
    nameZh: "合并 PDF",
    nameEn: "Merge PDF",
    descZh: "将多个 PDF 文档按指定顺序合并为一个文件",
    descEn: "Combine multiple PDF documents into one",
    category: "page",
    keywords: ["merge", "combine", "join", "concat", "合并", "拼接", "组合"],
  },
  {
    id: "split",
    nameZh: "拆分 PDF",
    nameEn: "Split PDF",
    descZh: "按单页或页码区间将文档拆分为多个独立文件",
    descEn: "Split document into single pages or ranges",
    category: "page",
    keywords: ["split", "separate", "divide", "range", "拆分", "分割", "按页拆分"],
  },
  {
    id: "addPageNumbers",
    nameZh: "添加页码",
    nameEn: "Add Page Numbers",
    descZh: "在顶部或底部批量添加格式化连续页码",
    descEn: "Batch add formatted page numbers to headers or footers",
    category: "page",
    keywords: ["page number", "header", "footer", "numbering", "页码", "编页", "页脚", "页眉"],
  },
  {
    id: "text",
    nameZh: "添加文字",
    nameEn: "Add Text",
    descZh: "在页面任意位置插入自定义文本、修改字号与颜色",
    descEn: "Insert custom text with font size and color controls",
    category: "edit",
    keywords: ["text", "type", "font", "write", "文字", "文本", "打字"],
  },
  {
    id: "watermark",
    nameZh: "添加水印",
    nameEn: "Add Watermark",
    descZh: "添加全页面半透明旋转文本水印",
    descEn: "Add semi-transparent rotated text watermark across pages",
    category: "edit",
    keywords: ["watermark", "stamp", "protect", "mark", "水印", "防伪", "标识"],
  },
  {
    id: "ocr",
    nameZh: "OCR 识别",
    nameEn: "OCR Text Recognition",
    descZh: "高精度扫描提取图像文字并生成可编辑替换层",
    descEn: "High-accuracy image text recognition with editable overlay",
    category: "edit",
    keywords: ["ocr", "scan", "recognize", "text", "识别", "扫描", "文字识别"],
  },
  {
    id: "pdf2img",
    nameZh: "PDF 转图片",
    nameEn: "PDF to Images",
    descZh: "将 PDF 各页面高质量导出为 PNG 图像",
    descEn: "Export PDF pages as high-quality PNG images",
    category: "convert",
    keywords: ["image", "png", "jpg", "render", "转图片", "导出图片"],
  },
  {
    id: "sanitize",
    nameZh: "隐私元数据脱敏",
    nameEn: "Sanitize & Scrub",
    descZh: "一键清除作者、标题、创建工具与隐藏元数据历史",
    descEn: "Strip author, title, XMP metadata and revision history",
    category: "extract",
    keywords: ["privacy", "sanitize", "metadata", "clean", "scrub", "脱敏", "隐私", "清除元数据"],
  },
  {
    id: "compress",
    nameZh: "文档压缩",
    nameEn: "Compress PDF",
    descZh: "优化内容流与孤立对象，无损减小文件体积",
    descEn: "Optimize streams and objects for lossless size reduction",
    category: "extract",
    keywords: ["compress", "size", "shrink", "optimize", "压缩", "瘦身", "减小体积"],
  },
];

export function filterTools(
  tools: TestToolDef[],
  category: "all" | "page" | "edit" | "convert" | "extract",
  query: string,
  locale: "zh" | "en" = "zh"
): TestToolDef[] {
  const q = query.trim().toLowerCase();
  return tools.filter((tool) => {
    if (category !== "all" && tool.category !== category) {
      return false;
    }
    if (!q) return true;

    const name = (locale === "zh" ? tool.nameZh : tool.nameEn).toLowerCase();
    const desc = (locale === "zh" ? tool.descZh : tool.descEn).toLowerCase();
    const matchKeyword = tool.keywords.some((k) => k.toLowerCase().includes(q));

    return name.includes(q) || desc.includes(q) || matchKeyword;
  });
}

describe("Toolbox Modal Search and Filtering", () => {
  it("returns all tools when search query is empty and category is all", () => {
    const results = filterTools(TOOLS_CATALOG, "all", "");
    expect(results.length).toBe(TOOLS_CATALOG.length);
  });

  it("filters tools by category correctly", () => {
    const pageTools = filterTools(TOOLS_CATALOG, "page", "");
    expect(pageTools.every((t) => t.category === "page")).toBe(true);
    expect(pageTools.map((t) => t.id)).toContain("reorder");
    expect(pageTools.map((t) => t.id)).toContain("addPageNumbers");
    expect(pageTools.map((t) => t.id)).not.toContain("compress");

    const extractTools = filterTools(TOOLS_CATALOG, "extract", "");
    expect(extractTools.map((t) => t.id)).toEqual(["sanitize", "compress"]);
  });

  it("finds tools by Chinese keyword queries", () => {
    const pageNumResults = filterTools(TOOLS_CATALOG, "all", "页码");
    expect(pageNumResults.length).toBeGreaterThan(0);
    expect(pageNumResults.map((t) => t.id)).toContain("addPageNumbers");

    const sanitizeResults = filterTools(TOOLS_CATALOG, "all", "脱敏");
    expect(sanitizeResults.length).toBe(1);
    expect(sanitizeResults[0].id).toBe("sanitize");

    const compressResults = filterTools(TOOLS_CATALOG, "all", "压缩");
    expect(compressResults.length).toBe(1);
    expect(compressResults[0].id).toBe("compress");
  });

  it("finds tools by English and technical keywords", () => {
    const ocrResults = filterTools(TOOLS_CATALOG, "all", "ocr");
    expect(ocrResults.some((t) => t.id === "ocr")).toBe(true);

    const rotateResults = filterTools(TOOLS_CATALOG, "all", "rotate");
    expect(rotateResults.some((t) => t.id === "rotate")).toBe(true);

    const scrubResults = filterTools(TOOLS_CATALOG, "all", "scrub");
    expect(scrubResults.some((t) => t.id === "sanitize")).toBe(true);
  });

  it("combines category filter and search query", () => {
    // "page" category + searching for "split" -> should find "split"
    const splitInPage = filterTools(TOOLS_CATALOG, "page", "split");
    expect(splitInPage.length).toBe(1);
    expect(splitInPage[0].id).toBe("split");

    // "edit" category + searching for "split" -> should be empty
    const splitInEdit = filterTools(TOOLS_CATALOG, "edit", "split");
    expect(splitInEdit.length).toBe(0);
  });

  it("returns empty array for non-matching queries", () => {
    const noResults = filterTools(TOOLS_CATALOG, "all", "xyz123_nonexistent");
    expect(noResults).toEqual([]);
  });
});
