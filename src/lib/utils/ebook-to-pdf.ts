/**
 * Pure client-side E-Book (EPUB, CBZ, TXT, Markdown) to Standard PDF Converter.
 * 100% offline, 0 network, generating standard PDF 1.4 with high-fidelity layout.
 */

import { parseEpub, parseComicBook, type EpubBook, type ComicBook } from "./epub-parser";

export interface PageImageSpec {
  jpegBytes: Uint8Array;
  width: number;
  height: number;
}

/**
 * Checks if a filename represents an ebook or readable text document that can be converted.
 */
export function isEbookFormat(fileName: string): boolean {
  const ext = fileName.split(".").pop()?.toLowerCase() || "";
  return ["epub", "cbz", "cbr", "txt", "md", "markdown"].includes(ext);
}

/**
 * Assemble multiple JPEG images into a valid, standard PDF 1.4 binary buffer.
 * Compatible with all PDF viewers including PDF.js and Adobe Acrobat.
 */
export function buildPdfFromJpegs(pages: PageImageSpec[]): Uint8Array {
  if (pages.length === 0) {
    // Return empty 1-page blank PDF
    return createBlankPdf();
  }

  const chunks: Uint8Array[] = [];
  const offsets: number[] = [];
  let currentOffset = 0;

  const encoder = new TextEncoder();
  function writeStr(str: string) {
    const bytes = encoder.encode(str);
    chunks.push(bytes);
    currentOffset += bytes.length;
  }

  function writeBytes(bytes: Uint8Array) {
    chunks.push(bytes);
    currentOffset += bytes.length;
  }

  // 1. Header
  writeStr("%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");

  // Objects layout:
  // 1 0 obj: Catalog
  // 2 0 obj: Pages
  // For each page i (0-based):
  //   Page obj: 3 + i*3
  //   Image XObject: 4 + i*3
  //   Contents obj: 5 + i*3
  const totalPages = pages.length;
  const pageObjIds: number[] = [];
  for (let i = 0; i < totalPages; i++) {
    pageObjIds.push(3 + i * 3);
  }

  // 1 0 obj: Catalog
  offsets[1] = currentOffset;
  writeStr("1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

  // 2 0 obj: Pages
  offsets[2] = currentOffset;
  const kidsStr = pageObjIds.map((id) => `${id} 0 R`).join(" ");
  writeStr(`2 0 obj\n<< /Type /Pages /Kids [${kidsStr}] /Count ${totalPages} >>\nendobj\n`);

  // For each page, write Page, Image, and Contents
  for (let i = 0; i < totalPages; i++) {
    const page = pages[i];
    const pageId = 3 + i * 3;
    const imageId = 4 + i * 3;
    const contentId = 5 + i * 3;

    // Page object
    offsets[pageId] = currentOffset;
    writeStr(
      `${pageId} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 ${page.width} ${page.height}] ` +
      `/Resources << /XObject << /Im1 ${imageId} 0 R >> >> /Contents ${contentId} 0 R >>\nendobj\n`
    );

    // Image XObject
    offsets[imageId] = currentOffset;
    writeStr(
      `${imageId} 0 obj\n<< /Type /XObject /Subtype /Image /Width ${page.width} /Height ${page.height} ` +
      `/ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode /Length ${page.jpegBytes.length} >>\nstream\n`
    );
    writeBytes(page.jpegBytes);
    writeStr("\nendstream\nendobj\n");

    // Contents stream (draws image scaled to fill page)
    offsets[contentId] = currentOffset;
    const contentStream = `q ${page.width} 0 0 ${page.height} 0 0 cm /Im1 Do Q\n`;
    const contentLen = encoder.encode(contentStream).length;
    writeStr(`${contentId} 0 obj\n<< /Length ${contentLen} >>\nstream\n${contentStream}endstream\nendobj\n`);
  }

  // XRef table
  const startXref = currentOffset;
  const totalObjects = 2 + totalPages * 3;
  writeStr(`xref\n0 ${totalObjects + 1}\n`);
  writeStr("0000000000 65535 f \n");

  for (let id = 1; id <= totalObjects; id++) {
    const offset = offsets[id] || 0;
    const padded = String(offset).padStart(10, "0");
    writeStr(`${padded} 00000 n \n`);
  }

  // Trailer
  writeStr(`trailer\n<< /Size ${totalObjects + 1} /Root 1 0 R >>\nstartxref\n${startXref}\n%%EOF\n`);

  // Concatenate all chunks
  const totalLength = chunks.reduce((acc, c) => acc + c.length, 0);
  const result = new Uint8Array(totalLength);
  let pos = 0;
  for (const c of chunks) {
    result.set(c, pos);
    pos += c.length;
  }

  return result;
}

/**
 * Creates a minimal 1-page blank PDF
 */
function createBlankPdf(): Uint8Array {
  const raw = `%PDF-1.4
1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj
2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj
3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] >> endobj
xref
0 4
0000000000 65535 f 
0000000009 00000 n 
0000000056 00000 n 
0000000115 00000 n 
trailer << /Size 4 /Root 1 0 R >>
startxref
185
%%EOF`;
  return new TextEncoder().encode(raw);
}

/**
 * Converts a Comic Archive (.cbz or .zip of images) into a standard PDF byte buffer.
 */
export async function convertCbzToPdf(buffer: ArrayBuffer | Uint8Array, title = "Comic"): Promise<Uint8Array> {
  const comic = await parseComicBook(buffer, title);
  if (comic.images.length === 0) {
    return createBlankPdf();
  }

  const pages: PageImageSpec[] = [];

  for (const img of comic.images) {
    const spec = await dataUrlToJpegSpec(img.dataUrl);
    if (spec) {
      pages.push(spec);
    }
  }

  return buildPdfFromJpegs(pages);
}

/**
 * Renders structured text or chapters into high-resolution book pages using an offscreen canvas,
 * returning standard PDF bytes.
 */
export async function convertEpubToPdf(buffer: ArrayBuffer | Uint8Array, fallbackTitle = "E-Book"): Promise<Uint8Array> {
  const book = await parseEpub(buffer);
  const title = book.title || fallbackTitle;
  const author = book.creator || "Unknown Author";

  const pages: PageImageSpec[] = [];

  // 1. Generate elegant Book Cover Page
  const coverSpec = renderBookCover(title, author);
  if (coverSpec) {
    pages.push(coverSpec);
  }

  // 2. Typeset each chapter
  let globalPageNum = 1;
  const pageWidth = 794;  // Standard A4 at 96 DPI
  const pageHeight = 1123;
  const marginX = 80;
  const marginTop = 90;
  const marginBottom = 80;
  const contentWidth = pageWidth - marginX * 2;
  const contentHeight = pageHeight - marginTop - marginBottom;

  const canvas = document.createElement("canvas");
  canvas.width = pageWidth * 2; // 2x retina clarity
  canvas.height = pageHeight * 2;
  const ctx = canvas.getContext("2d");
  if (!ctx) return createBlankPdf();

  for (const chapter of book.chapters) {
    const text = chapter.plainText || "";
    if (!text.trim()) continue;

    // Break chapter into paragraphs
    const paragraphs = text
      .split(/\r?\n+/)
      .map((p) => p.trim())
      .filter(Boolean);

    let currentLines: Array<{ text: string; isHeading?: boolean }> = [];
    currentLines.push({ text: chapter.title, isHeading: true });

    for (const para of paragraphs) {
      // Word wrap paragraph to lines
      ctx.font = "32px Georgia, serif"; // 2x scaled
      const words = para.split(""); // Character/word based wrap for multilingual support
      let line = "    "; // 2-char indent

      for (let w = 0; w < words.length; w++) {
        const char = words[w];
        const testLine = line + char;
        const metrics = ctx.measureText(testLine);
        if (metrics.width > contentWidth * 2 && line.trim().length > 0) {
          currentLines.push({ text: line });
          line = char;
        } else {
          line = testLine;
        }
      }
      if (line.trim().length > 0) {
        currentLines.push({ text: line });
      }
      // Add small paragraph spacer
      currentLines.push({ text: "" });
    }

    // Now paginate currentLines onto pages
    let lineIdx = 0;
    while (lineIdx < currentLines.length) {
      // Clear canvas with warm reader white background
      ctx.fillStyle = "#fafaf9"; // Warm paper
      ctx.fillRect(0, 0, canvas.width, canvas.height);

      // Header: Book title and Chapter title
      ctx.fillStyle = "#a8a29e";
      ctx.font = "20px system-ui, sans-serif";
      ctx.textAlign = "left";
      ctx.fillText(title, marginX * 2, 50 * 2);
      ctx.textAlign = "right";
      ctx.fillText(chapter.title.slice(0, 30), (pageWidth - marginX) * 2, 50 * 2);

      // Thin divider
      ctx.strokeStyle = "#e7e5e4";
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      ctx.moveTo(marginX * 2, 60 * 2);
      ctx.lineTo((pageWidth - marginX) * 2, 60 * 2);
      ctx.stroke();

      // Body lines
      let curY = marginTop * 2;
      const maxY = (pageHeight - marginBottom) * 2;

      while (lineIdx < currentLines.length && curY < maxY) {
        const item = currentLines[lineIdx];
        if (item.isHeading) {
          ctx.fillStyle = "#1c1917";
          ctx.font = "bold 44px Georgia, serif";
          ctx.textAlign = "left";
          curY += 20;
          ctx.fillText(item.text, marginX * 2, curY);
          curY += 60;
        } else if (item.text === "") {
          curY += 24; // paragraph spacing
        } else {
          ctx.fillStyle = "#292524";
          ctx.font = "32px Georgia, serif";
          ctx.textAlign = "left";
          ctx.fillText(item.text, marginX * 2, curY);
          curY += 46; // line height
        }
        lineIdx++;
      }

      // Footer: Page Number
      ctx.fillStyle = "#a8a29e";
      ctx.font = "22px system-ui, sans-serif";
      ctx.textAlign = "center";
      ctx.fillText(`- ${globalPageNum} -`, pageWidth, (pageHeight - 35) * 2);

      // Export canvas to JPEG
      const spec = canvasToJpegSpec(canvas, pageWidth, pageHeight);
      if (spec) {
        pages.push(spec);
        globalPageNum++;
      }
    }
  }

  if (pages.length === 0) {
    return createBlankPdf();
  }

  return buildPdfFromJpegs(pages);
}

/**
 * Converts plain text or Markdown to standard PDF bytes.
 */
export async function convertTextToPdf(text: string, title = "Text Document"): Promise<Uint8Array> {
  const fakeEpubBuffer = new Uint8Array(0);
  const pages: PageImageSpec[] = [];

  const pageWidth = 794;
  const pageHeight = 1123;
  const marginX = 70;
  const marginTop = 80;
  const marginBottom = 80;
  const contentWidth = pageWidth - marginX * 2;

  const canvas = document.createElement("canvas");
  canvas.width = pageWidth * 2;
  canvas.height = pageHeight * 2;
  const ctx = canvas.getContext("2d");
  if (!ctx) return createBlankPdf();

  const lines = text.split(/\r?\n/);
  const formattedLines: Array<{ text: string; isHeading?: boolean }> = [];
  formattedLines.push({ text: title, isHeading: true });

  for (const line of lines) {
    const trimmed = line.trim();
    if (!trimmed) {
      formattedLines.push({ text: "" });
      continue;
    }
    const isHead = trimmed.startsWith("#");
    const cleanText = trimmed.replace(/^#+\s*/, "");

    ctx.font = isHead ? "bold 38px system-ui, sans-serif" : "30px system-ui, sans-serif";
    const words = cleanText.split("");
    let curLine = "";
    for (const ch of words) {
      const test = curLine + ch;
      if (ctx.measureText(test).width > contentWidth * 2 && curLine.length > 0) {
        formattedLines.push({ text: curLine, isHeading: isHead });
        curLine = ch;
      } else {
        curLine = test;
      }
    }
    if (curLine.length > 0) {
      formattedLines.push({ text: curLine, isHeading: isHead });
    }
  }

  let lineIdx = 0;
  let pageNum = 1;
  while (lineIdx < formattedLines.length) {
    ctx.fillStyle = "#ffffff";
    ctx.fillRect(0, 0, canvas.width, canvas.height);

    // Header
    ctx.fillStyle = "#94a3b8";
    ctx.font = "20px system-ui, sans-serif";
    ctx.textAlign = "left";
    ctx.fillText(title, marginX * 2, 45 * 2);

    let curY = marginTop * 2;
    const maxY = (pageHeight - marginBottom) * 2;

    while (lineIdx < formattedLines.length && curY < maxY) {
      const item = formattedLines[lineIdx];
      if (item.isHeading) {
        ctx.fillStyle = "#0f172a";
        ctx.font = "bold 40px system-ui, sans-serif";
        ctx.textAlign = "left";
        curY += 20;
        ctx.fillText(item.text, marginX * 2, curY);
        curY += 56;
      } else if (item.text === "") {
        curY += 22;
      } else {
        ctx.fillStyle = "#334155";
        ctx.font = "30px system-ui, sans-serif";
        ctx.textAlign = "left";
        ctx.fillText(item.text, marginX * 2, curY);
        curY += 44;
      }
      lineIdx++;
    }

    // Footer
    ctx.fillStyle = "#94a3b8";
    ctx.font = "20px system-ui, sans-serif";
    ctx.textAlign = "center";
    ctx.fillText(`${pageNum}`, pageWidth, (pageHeight - 35) * 2);

    const spec = canvasToJpegSpec(canvas, pageWidth, pageHeight);
    if (spec) {
      pages.push(spec);
      pageNum++;
    }
  }

  return buildPdfFromJpegs(pages);
}

/**
 * Helper: render a stylish Book Cover Page
 */
function renderBookCover(title: string, author: string): PageImageSpec | null {
  const pageWidth = 794;
  const pageHeight = 1123;
  const canvas = document.createElement("canvas");
  canvas.width = pageWidth * 2;
  canvas.height = pageHeight * 2;
  const ctx = canvas.getContext("2d");
  if (!ctx) return null;

  // Rich gradient book cover background
  const grad = ctx.createLinearGradient(0, 0, canvas.width, canvas.height);
  grad.addColorStop(0, "#1e293b");
  grad.addColorStop(1, "#0f172a");
  ctx.fillStyle = grad;
  ctx.fillRect(0, 0, canvas.width, canvas.height);

  // Elegant golden decorative frame
  ctx.strokeStyle = "#e2b168";
  ctx.lineWidth = 3;
  ctx.strokeRect(60 * 2, 60 * 2, (pageWidth - 120) * 2, (pageHeight - 120) * 2);

  ctx.lineWidth = 1;
  ctx.strokeRect(68 * 2, 68 * 2, (pageWidth - 136) * 2, (pageHeight - 136) * 2);

  // Title
  ctx.fillStyle = "#f8fafc";
  ctx.font = "bold 64px Georgia, serif";
  ctx.textAlign = "center";

  // Wrap title if too long
  const titleWords = title.split("");
  let line = "";
  let y = 380 * 2;
  for (const ch of titleWords) {
    if (ctx.measureText(line + ch).width > (pageWidth - 180) * 2) {
      ctx.fillText(line, pageWidth, y);
      y += 80;
      line = ch;
    } else {
      line += ch;
    }
  }
  if (line) {
    ctx.fillText(line, pageWidth, y);
  }

  // Divider
  ctx.strokeStyle = "#e2b168";
  ctx.lineWidth = 2;
  ctx.beginPath();
  ctx.moveTo((pageWidth - 120) * 2, y + 60);
  ctx.lineTo((pageWidth + 120) * 2, y + 60);
  ctx.stroke();

  // Author
  ctx.fillStyle = "#cbd5e1";
  ctx.font = "italic 36px Georgia, serif";
  ctx.fillText(author, pageWidth, y + 140);

  // Bottom Badge
  ctx.fillStyle = "#e2b168";
  ctx.font = "24px system-ui, sans-serif";
  ctx.fillText("PDF SEEKER E-BOOK EDITION", pageWidth, (pageHeight - 100) * 2);

  return canvasToJpegSpec(canvas, pageWidth, pageHeight);
}

/**
 * Helper: Convert HTML Canvas to PageImageSpec
 */
function canvasToJpegSpec(canvas: HTMLCanvasElement, targetW: number, targetH: number): PageImageSpec | null {
  try {
    const dataUrl = canvas.toDataURL("image/jpeg", 0.90);
    const base64 = dataUrl.split(",")[1];
    const binary = atob(base64);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) {
      bytes[i] = binary.charCodeAt(i);
    }
    return {
      jpegBytes: bytes,
      width: targetW,
      height: targetH,
    };
  } catch (_) {
    return null;
  }
}

/**
 * Helper: Convert Image DataURL to PageImageSpec
 */
async function dataUrlToJpegSpec(dataUrl: string): Promise<PageImageSpec | null> {
  return new Promise((resolve) => {
    const img = new Image();
    img.onload = () => {
      const canvas = document.createElement("canvas");
      canvas.width = img.naturalWidth || img.width || 800;
      canvas.height = img.naturalHeight || img.height || 1200;
      const ctx = canvas.getContext("2d");
      if (!ctx) {
        resolve(null);
        return;
      }
      ctx.fillStyle = "#ffffff";
      ctx.fillRect(0, 0, canvas.width, canvas.height);
      ctx.drawImage(img, 0, 0);

      const spec = canvasToJpegSpec(canvas, canvas.width, canvas.height);
      resolve(spec);
    };
    img.onerror = () => resolve(null);
    img.src = dataUrl;
  });
}

