/**
 * Smart Local PDF Compression Engine (Stirling-PDF grade).
 * 
 * Provides real, measurable, and tangible size reductions by:
 * - Smart visual downsampling and high-efficiency DCT/JPEG stream encoding
 * - Grayscale color-channel conversion for archival documents
 * - Elimination of bloated uncompressed bitmap streams and dead metadata
 * - Maintaining 100% offline local operation with 0 network usage
 */

import { buildPdfFromJpegs, type PageImageSpec } from "./ebook-to-pdf";
import { readFile, writeFile } from "@tauri-apps/plugin-fs";
import { invoke } from "@tauri-apps/api/core";

export interface CompressionProfile {
  scale: number;
  quality: number;
  grayscale: boolean;
}

export const PRESET_PROFILES: Record<string, CompressionProfile> = {
  high_quality: { scale: 1.8, quality: 0.85, grayscale: false },
  balanced: { scale: 1.4, quality: 0.70, grayscale: false },
  max_compression: { scale: 1.0, quality: 0.50, grayscale: false },
  grayscale: { scale: 1.3, quality: 0.65, grayscale: true },
};

export interface CompressionExecutionResult {
  originalSize: number;
  compressedSize: number;
  ratio: number;
  outputPath: string;
}

/**
 * Compresses an input PDF file according to the chosen preset strategy,
 * writing the newly compacted PDF to target outputPath.
 */
export async function executeSmartPdfCompression(
  inputPath: string,
  outputPath: string,
  preset: string = "balanced",
  onProgress?: (progressPercent: number, current: number, total: number) => void,
  imageCount?: number
): Promise<CompressionExecutionResult> {
  const profile = PRESET_PROFILES[preset] || PRESET_PROFILES.balanced;

  // If document is pure vector/text (0 images), rasterization would bloat file size and blur text.
  // Directly execute high-performance backend lossless structural compression.
  if (imageCount === 0) {
    if (onProgress) onProgress(50, 1, 1);
    const res = await invoke<{
      original_size?: number;
      originalSize?: number;
      compressed_size?: number;
      compressedSize?: number;
      ratio?: number;
    }>("compress_pdf", {
      inputPath,
      outputPath,
    });
    if (onProgress) onProgress(100, 1, 1);
    const orig = res.original_size ?? res.originalSize ?? 0;
    const comp = res.compressed_size ?? res.compressedSize ?? orig;
    const ratio = res.ratio ?? (orig > 0 ? ((orig - comp) / orig) * 100 : 0);
    return {
      originalSize: orig,
      compressedSize: comp,
      ratio,
      outputPath,
    };
  }

  let originalBytes: Uint8Array;
  try {
    const raw = await readFile(inputPath);
    originalBytes = new Uint8Array(raw);
  } catch (err) {
    // If frontend file reading is blocked, fallback to backend command
    const res = await invoke<{ original_size?: number; originalSize?: number; compressed_size?: number; compressedSize?: number; ratio?: number }>("compress_pdf", {
      inputPath,
      outputPath,
    });
    const orig = res.original_size ?? res.originalSize ?? 0;
    const comp = res.compressed_size ?? res.compressedSize ?? orig;
    const ratio = res.ratio ?? (orig > 0 ? ((orig - comp) / orig) * 100 : 0);
    return {
      originalSize: orig,
      compressedSize: comp,
      ratio,
      outputPath,
    };
  }

  const origSize = originalBytes.length;

  try {
    const pdfjsLib = await import("pdfjs-dist/legacy/build/pdf.mjs");
    const loadingTask = pdfjsLib.getDocument({ data: originalBytes });
    const pdfDoc = await loadingTask.promise;
    const totalPages = pdfDoc.numPages;

    const pageImages: PageImageSpec[] = [];

    for (let pageNum = 1; pageNum <= totalPages; pageNum++) {
      const page = await pdfDoc.getPage(pageNum);
      const vp = page.getViewport({ scale: profile.scale });

      const canvas = document.createElement("canvas");
      canvas.width = Math.max(1, Math.floor(vp.width));
      canvas.height = Math.max(1, Math.floor(vp.height));
      const ctx = canvas.getContext("2d");

      if (!ctx) {
        throw new Error("Unable to initialize canvas 2D context");
      }

      // Draw white background
      ctx.fillStyle = "#ffffff";
      ctx.fillRect(0, 0, canvas.width, canvas.height);

      await page.render({ canvasContext: ctx, viewport: vp }).promise;

      // Handle Grayscale transformation if specified
      if (profile.grayscale) {
        const imgData = ctx.getImageData(0, 0, canvas.width, canvas.height);
        const data = imgData.data;
        for (let i = 0; i < data.length; i += 4) {
          const gray = Math.round(data[i] * 0.299 + data[i + 1] * 0.587 + data[i + 2] * 0.114);
          data[i] = gray;
          data[i + 1] = gray;
          data[i + 2] = gray;
        }
        ctx.putImageData(imgData, 0, 0);
      }

      // Convert page canvas to highly-compressed JPEG binary
      const jpegBlob = await new Promise<Blob | null>((resolve) => {
        canvas.toBlob(resolve, "image/jpeg", profile.quality);
      });

      if (!jpegBlob) {
        throw new Error(`Failed to encode compressed page ${pageNum}`);
      }

      const ab = await jpegBlob.arrayBuffer();
      const baseVp = page.getViewport({ scale: 1.0 });
      const origW = baseVp.width || 595;
      const origH = baseVp.height || 842;

      pageImages.push({
        jpegBytes: new Uint8Array(ab),
        width: origW,
        height: origH,
      });

      if (onProgress) {
        const percent = Math.round((pageNum / totalPages) * 100);
        onProgress(percent, pageNum, totalPages);
      }
    }

    // Assemble clean, compact PDF 1.4
    const compressedPdfBytes = buildPdfFromJpegs(pageImages);
    const compressedSize = compressedPdfBytes.length;

    // If compressed size is indeed smaller, save it
    // If for some reason it's larger (e.g. 1-page pure text with 0 images), keep whichever is better
    if (compressedSize < origSize || origSize === 0) {
      await writeFile(outputPath, compressedPdfBytes);
      const ratio = origSize > 0 ? ((origSize - compressedSize) / origSize) * 100 : 0;
      return {
        originalSize: origSize,
        compressedSize,
        ratio,
        outputPath,
      };
    } else {
      // Fallback to transactional structural compress via backend
      const res = await invoke<{ original_size?: number; originalSize?: number; compressed_size?: number; compressedSize?: number; ratio?: number }>("compress_pdf", {
        inputPath,
        outputPath,
      });
      const comp = res.compressed_size ?? res.compressedSize ?? origSize;
      const ratio = res.ratio ?? (origSize > 0 ? ((origSize - comp) / origSize) * 100 : 0);
      return {
        originalSize: origSize,
        compressedSize: comp,
        ratio,
        outputPath,
      };
    }
  } catch (renderError) {
    // If DOM or canvas fails, fall back cleanly to backend lopdf compression
    const res = await invoke<{ original_size?: number; originalSize?: number; compressed_size?: number; compressedSize?: number; ratio?: number }>("compress_pdf", {
      inputPath,
      outputPath,
    });
    const orig = res.original_size ?? res.originalSize ?? origSize;
    const comp = res.compressed_size ?? res.compressedSize ?? orig;
    const ratio = res.ratio ?? (orig > 0 ? ((orig - comp) / orig) * 100 : 0);
    return {
      originalSize: orig,
      compressedSize: comp,
      ratio,
      outputPath,
    };
  }
}
