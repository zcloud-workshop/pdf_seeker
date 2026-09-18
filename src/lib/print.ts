import type { PdfDocumentProxy } from "@/pdf-engine";

export interface PrintOptions {
  pages?: number[];
  orientation?: "portrait" | "landscape" | "auto";
  scaleFactor?: number;
}

export async function printPdf(
  doc: PdfDocumentProxy,
  options: PrintOptions = {},
): Promise<void> {
  const {
    pages = [],
    orientation = "auto",
    scaleFactor = 1.0,
  } = options;

  const pagesToRender = pages.length > 0
    ? pages.filter(p => p >= 1 && p <= doc.numPages)
    : Array.from({ length: doc.numPages }, (_, i) => i + 1);

  if (pagesToRender.length === 0) return;

  // Render pages to images
  const pageImages: string[] = [];
  for (const pageNum of pagesToRender) {
    const page = await doc.getPage(pageNum);
    const vp = page.getViewport({ scale: 2 * scaleFactor });
    const canvas = document.createElement("canvas");
    canvas.width = Math.floor(vp.width);
    canvas.height = Math.floor(vp.height);
    const ctx = canvas.getContext("2d")!;
    await page.render({ canvasContext: ctx, viewport: vp }).promise;
    pageImages.push(canvas.toDataURL("image/png"));
  }

  // Detect orientation
  const firstPage = await doc.getPage(pagesToRender[0]);
  const firstVp = firstPage.getViewport({ scale: 1 });
  const isLandscape = firstVp.width > firstVp.height;
  const effectiveOrientation = orientation === "auto"
    ? (isLandscape ? "landscape" : "portrait")
    : orientation;

  // Build print HTML
  const html = `<!DOCTYPE html>
<html>
<head>
  <style>
    * { margin: 0; padding: 0; box-sizing: border-box; }
    @page { size: ${effectiveOrientation}; margin: 8mm; }
    body { display: flex; flex-direction: column; align-items: center; }
    .page { page-break-after: always; width: 100%; display: flex; justify-content: center; align-items: center; }
    .page:last-child { page-break-after: auto; }
    .page img { max-width: 100%; max-height: 98vh; object-fit: contain; }
    @media print { .page { page-break-after: always; } .page:last-child { page-break-after: auto; } }
  </style>
</head>
<body>
${pageImages.map(src => `  <div class="page"><img src="${src}" /></div>`).join("\n")}
  <script>
    window.onload = function() {
      setTimeout(function() { window.print(); }, 300);
    };
  </script>
</body>
</html>`;

  // Use Tauri's WebviewWindow to open a print window
  const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
  const printWin = new WebviewWindow("print-window", {
    url: "data:text/html;charset=utf-8," + encodeURIComponent(html),
    title: "Print",
    width: 800,
    height: 600,
    resizable: true,
    center: true,
  });

  printWin.once("tauri://created", () => {
    // Window created successfully
  });

  printWin.once("tauri://error", (e) => {
    console.error("Print window error:", e);
  });
}
