import * as pdfjsLib from "pdfjs-dist";
import type { PDFDocumentProxy, PDFPageProxy } from "pdfjs-dist";

// Extend PDFPageProxy to include getTextContent which exists at runtime
declare module "pdfjs-dist" {
  interface PDFPageProxy {
    getTextContent(params?: { includeMarkedContent?: boolean; disableNormalization?: boolean }): Promise<{
      items: Array<{ str: string; dir: string; width: number; height: number; transform: number[]; fontName: string; hasEOL: boolean }>;
      styles: Record<string, { fontFamily: string; ascent: number; descent: number; vertical: boolean }>;
    }>;
  }
}

// Bundle the worker via Vite's ?url import
pdfjsLib.GlobalWorkerOptions.workerSrc = new URL(
  "pdfjs-dist/build/pdf.worker.min.mjs",
  import.meta.url,
).toString();

export type PdfDocumentProxy = PDFDocumentProxy;

export async function loadPdf(data: Uint8Array): Promise<PdfDocumentProxy> {
  const doc = await pdfjsLib.getDocument({ data: data.slice() }).promise;
  return doc;
}

export async function getPageViewport(
  doc: PdfDocumentProxy,
  pageNum: number,
  scale: number,
): Promise<{ width: number; height: number }> {
  const page = await doc.getPage(pageNum);
  const vp = page.getViewport({ scale });
  return { width: vp.width, height: vp.height };
}

export async function renderPageToCanvas(
  doc: PdfDocumentProxy,
  pageNum: number,
  canvas: HTMLCanvasElement,
  scale: number,
): Promise<void> {
  const page = await doc.getPage(pageNum);
  const viewport = page.getViewport({ scale });
  const outputScale = window.devicePixelRatio || 1;

  canvas.width = Math.floor(viewport.width * outputScale);
  canvas.height = Math.floor(viewport.height * outputScale);
  canvas.style.width = `${Math.floor(viewport.width)}px`;
  canvas.style.height = `${Math.floor(viewport.height)}px`;

  const ctx = canvas.getContext("2d")!;
  ctx.setTransform(outputScale, 0, 0, outputScale, 0, 0);

  await page.render({ canvasContext: ctx, viewport }).promise;
}
