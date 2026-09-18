/**
 * Pure client-side EPUB, TXT, Markdown, and Comic (CBZ) format parser & converter.
 * Fully offline, 0 network, utilizing standard Web APIs (DecompressionStream & DOMParser).
 */

export interface EpubChapter {
  id: string;
  title: string;
  href: string;
  html: string;
  plainText: string;
}

export interface EpubBook {
  title: string;
  creator: string;
  language: string;
  coverImage?: string; // base64 / data URL
  chapters: EpubChapter[];
  totalWords: number;
}

export interface ComicBook {
  title: string;
  pageCount: number;
  images: Array<{ name: string; dataUrl: string }>;
}

/**
 * Minimalist pure JS ZIP reader for EPUB and CBZ archives.
 */
export class MiniZip {
  private view: DataView;
  private bytes: Uint8Array;

  constructor(buffer: ArrayBuffer | Uint8Array) {
    if (buffer instanceof Uint8Array) {
      this.bytes = buffer;
      this.view = new DataView(buffer.buffer, buffer.byteOffset, buffer.byteLength);
    } else {
      this.bytes = new Uint8Array(buffer);
      this.view = new DataView(buffer);
    }
  }

  /**
   * List all entries in the ZIP archive.
   */
  async getEntries(): Promise<Map<string, { method: number; compressed: Uint8Array; uncompressedSize: number }>> {
    const entries = new Map<string, { method: number; compressed: Uint8Array; uncompressedSize: number }>();
    const len = this.bytes.length;
    let offset = 0;

    while (offset < len - 30) {
      const sig = this.view.getUint32(offset, true);
      if (sig !== 0x04034b50) {
        // Not a local file header, might have reached central directory or padding
        break;
      }

      const method = this.view.getUint16(offset + 8, true);
      const compressedSize = this.view.getUint32(offset + 18, true);
      const uncompressedSize = this.view.getUint32(offset + 22, true);
      const fileNameLen = this.view.getUint16(offset + 26, true);
      const extraLen = this.view.getUint16(offset + 28, true);

      const fileNameBytes = this.bytes.subarray(offset + 30, offset + 30 + fileNameLen);
      const fileName = new TextDecoder().decode(fileNameBytes);

      const dataStart = offset + 30 + fileNameLen + extraLen;
      const compressed = this.bytes.subarray(dataStart, dataStart + compressedSize);

      entries.set(fileName, { method, compressed, uncompressedSize });
      offset = dataStart + compressedSize;
    }

    return entries;
  }

  /**
   * Read and decompress a specific file entry by path.
   */
  async readText(path: string, entries: Map<string, { method: number; compressed: Uint8Array; uncompressedSize: number }>): Promise<string> {
    const entry = entries.get(path) || entries.get(path.startsWith("/") ? path.slice(1) : path);
    if (!entry) return "";

    const raw = await this.decompress(entry.compressed, entry.method);
    return new TextDecoder().decode(raw);
  }

  async readBinary(path: string, entries: Map<string, { method: number; compressed: Uint8Array; uncompressedSize: number }>): Promise<Uint8Array> {
    const entry = entries.get(path) || entries.get(path.startsWith("/") ? path.slice(1) : path);
    if (!entry) return new Uint8Array(0);

    return this.decompress(entry.compressed, entry.method);
  }

  private async decompress(compressed: Uint8Array, method: number): Promise<Uint8Array> {
    if (method === 0) {
      // Stored (no compression)
      return compressed;
    }
    if (method === 8) {
      // Deflate
      if (typeof DecompressionStream !== "undefined") {
        try {
          const ds = new DecompressionStream("deflate-raw");
          const writer = ds.writable.getWriter();
          writer.write(compressed as any);
          writer.close();
          const response = new Response(ds.readable);
          const arrayBuffer = await response.arrayBuffer();
          return new Uint8Array(arrayBuffer);
        } catch (e) {
          console.warn("DecompressionStream error, returning raw fallback:", e);
        }
      }
    }
    return compressed;
  }
}

/**
 * Parse an EPUB file into structured chapters and book metadata.
 */
export async function parseEpub(buffer: ArrayBuffer | Uint8Array): Promise<EpubBook> {
  const zip = new MiniZip(buffer);
  const entries = await zip.getEntries();

  // 1. Find META-INF/container.xml
  const containerXml = await zip.readText("META-INF/container.xml", entries);
  let opfPath = "OEBPS/content.opf";

  if (containerXml) {
    const parser = new DOMParser();
    const doc = parser.parseFromString(containerXml, "application/xml");
    const rootfile = doc.querySelector("rootfile");
    if (rootfile && rootfile.getAttribute("full-path")) {
      opfPath = rootfile.getAttribute("full-path")!;
    }
  }

  // 2. Parse OPF package file
  const opfXml = await zip.readText(opfPath, entries);
  const opfDir = opfPath.includes("/") ? opfPath.slice(0, opfPath.lastIndexOf("/") + 1) : "";

  let title = "Unknown Title";
  let creator = "Unknown Author";
  let language = "en";

  const manifestMap = new Map<string, { href: string; mediaType: string }>();
  const spineItems: string[] = [];

  if (opfXml) {
    const parser = new DOMParser();
    const doc = parser.parseFromString(opfXml, "application/xml");

    title = doc.querySelector("title, dc\\:title")?.textContent || "Unknown Title";
    creator = doc.querySelector("creator, dc\\:creator")?.textContent || "Unknown Author";
    language = doc.querySelector("language, dc\\:language")?.textContent || "en";

    const itemNodes = doc.querySelectorAll("manifest > item");
    itemNodes.forEach((node) => {
      const id = node.getAttribute("id");
      const href = node.getAttribute("href");
      const mediaType = node.getAttribute("media-type") || "";
      if (id && href) {
        manifestMap.set(id, { href: opfDir + href, mediaType });
      }
    });

    const itemrefNodes = doc.querySelectorAll("spine > itemref");
    itemrefNodes.forEach((node) => {
      const idref = node.getAttribute("idref");
      if (idref) spineItems.push(idref);
    });
  }

  // 3. Extract and parse chapters in spine order
  const chapters: EpubChapter[] = [];
  let totalWords = 0;

  for (const idref of spineItems) {
    const manifestItem = manifestMap.get(idref);
    if (!manifestItem) continue;

    const htmlContent = await zip.readText(manifestItem.href, entries);
    if (!htmlContent) continue;

    const parser = new DOMParser();
    const doc = parser.parseFromString(htmlContent, "text/html");

    // Extract heading or title
    const chapterTitle =
      doc.querySelector("h1, h2, h3, title")?.textContent?.trim() ||
      `Chapter ${chapters.length + 1}`;

    const plainText = doc.body.textContent || "";
    totalWords += plainText.split(/\s+/).filter(Boolean).length;

    chapters.push({
      id: idref,
      title: chapterTitle,
      href: manifestItem.href,
      html: doc.body.innerHTML,
      plainText: plainText.trim(),
    });
  }

  return {
    title,
    creator,
    language,
    chapters,
    totalWords,
  };
}

/**
 * Parse a Comic Archive (.cbz or .zip of images) into pages.
 */
export async function parseComicBook(buffer: ArrayBuffer | Uint8Array, title = "Comic Document"): Promise<ComicBook> {
  const zip = new MiniZip(buffer);
  const entries = await zip.getEntries();
  const images: Array<{ name: string; dataUrl: string }> = [];

  const imageRegex = /\.(png|jpe?g|webp|gif|bmp)$/i;

  const imageEntries = Array.from(entries.keys())
    .filter((name) => imageRegex.test(name) && !name.startsWith("__MACOSX"))
    .sort((a, b) => a.localeCompare(b, undefined, { numeric: true, sensitivity: "base" }));

  for (const name of imageEntries) {
    const raw = await zip.readBinary(name, entries);
    if (raw.length > 0) {
      let mime = "image/jpeg";
      if (name.toLowerCase().endsWith(".png")) mime = "image/png";
      else if (name.toLowerCase().endsWith(".webp")) mime = "image/webp";

      // Convert Uint8Array to base64
      let binary = "";
      const len = raw.byteLength;
      for (let i = 0; i < len; i++) {
        binary += String.fromCharCode(raw[i]);
      }
      const base64 = btoa(binary);
      images.push({
        name,
        dataUrl: `data:${mime};base64,${base64}`,
      });
    }
  }

  return {
    title,
    pageCount: images.length,
    images,
  };
}

/**
 * Convert plain text or markdown to formatted printable HTML pages.
 */
export function formatTextToHtmlPages(content: string, title = "Document"): string {
  const escaped = content
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");

  // Simple paragraph & heading markdown formatting
  const lines = escaped.split(/\r?\n/);
  let html = `<div style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; line-height: 1.7; color: #1e293b; padding: 40px; max-width: 800px; margin: 0 auto;">`;
  html += `<h1 style="border-bottom: 2px solid #e2e8f0; padding-bottom: 12px; margin-bottom: 24px;">${title}</h1>`;

  for (const line of lines) {
    const trimmed = line.trim();
    if (!trimmed) {
      html += `<div style="height: 12px;"></div>`;
    } else if (trimmed.startsWith("# ")) {
      html += `<h2 style="font-size: 20px; font-weight: 700; margin-top: 24px; margin-bottom: 12px; color: #0f172a;">${trimmed.slice(2)}</h2>`;
    } else if (trimmed.startsWith("## ")) {
      html += `<h3 style="font-size: 16px; font-weight: 600; margin-top: 18px; margin-bottom: 8px; color: #334155;">${trimmed.slice(3)}</h3>`;
    } else if (trimmed.startsWith("- ") || trimmed.startsWith("* ")) {
      html += `<li style="margin-left: 20px; margin-bottom: 4px;">${trimmed.slice(2)}</li>`;
    } else {
      html += `<p style="margin-bottom: 10px;">${trimmed}</p>`;
    }
  }

  html += `</div>`;
  return html;
}

