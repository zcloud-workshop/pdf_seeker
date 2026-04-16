<script lang="ts">
  import { t } from "@/i18n/index.svelte.ts";
  import { currentView, currentFilePath, isDark } from "@/stores";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { invoke } from "@tauri-apps/api/core";
  import { readFile, writeTextFile } from "@tauri-apps/plugin-fs";
  import { Button, Input, Label } from "@/components/ui";
  import {
    Merge as Icon_Merge,
    RotateCw as Icon_RotateCw,
    Trash2 as Icon_Trash2,
    Minimize2 as Icon_Minimize2,
    Droplets as Icon_Droplets,
    Image as Icon_Image,
    FileText as Icon_FileText,
    PenTool as Icon_PenTool,
    ScanLine as Icon_ScanLine,
    Table as Icon_Table,
    FileUp as Icon_FileUp,
    CheckCircle as Icon_CheckCircle,
    AlertCircle as Icon_AlertCircle,
    Loader2 as Icon_Loader2,
    ArrowLeft as Icon_ArrowLeft,
    Save as Icon_Save,
    X as Icon_X,
    Scissors as Icon_Scissors,
    FileOutput as Icon_FileOutput,
    ImageDown as Icon_ImageDown,
    FolderOpen as Icon_FolderOpen,
    Pencil as Icon_Pencil,
    Square as Icon_Square,
    Highlighter as Icon_Highlighter,
  } from "lucide-svelte";
  import { loadPdf, renderPageToCanvas, type PdfDocumentProxy } from "@/pdf-engine";
  import { tick } from "svelte";

  type ToolId =
    | "merge" | "split" | "rotate" | "delete" | "extractPages"
    | "compress" | "watermark" | "img2pdf" | "pdf2img"
    | "pdf2text" | "sign" | "ocr" | "table"
    | "editText" | "editRect" | "editHighlight";

  let activeTool: ToolId | null = $state(null);
  let busy = $state(false);
  let resultMsg = $state("");
  let resultOk = $state(false);

  // --- Thumbnail preview state ---
  let thumbDoc = $state<PdfDocumentProxy | null>(null);
  let thumbContainer = $state<HTMLDivElement | undefined>(undefined);
  let previewPageCount = $state(0);
  let thumbLoading = $state(false);
  let renderVersion = 0;
  let pageDims = $state<Array<{ w: number; h: number }>>([]);
  let dragState = $state<{
    active: boolean;
    pageNum: number;
    startX: number;
    startY: number;
    curX: number;
    curY: number;
  } | null>(null);
  let largeCanvas = $state<HTMLCanvasElement | undefined>(undefined);
  let thumbStripContainer = $state<HTMLDivElement | undefined>(undefined);

  function isEditingTool(id: ToolId | null): boolean {
    return !!id && ["editText", "editRect", "editHighlight", "sign", "watermark"].includes(id);
  }

  function setPreviewPage(p: number) {
    if (p < 1 || p > previewPageCount) return;
    switch (activeTool) {
      case "editText": editTextPage = p; break;
      case "editRect": editRectPage = p; break;
      case "editHighlight": editHlPage = p; break;
      case "sign": signPage = p; break;
    }
  }


  const toolDefs: {
    id: ToolId;
    icon: typeof Icon_Merge;
    labelKey: string;
    ready: boolean;
    hasPreview: boolean;
  }[] = [
    { id: "merge", icon: Icon_Merge, labelKey: "tools.merge", ready: true, hasPreview: false },
    { id: "split", icon: Icon_Scissors, labelKey: "tools.split", ready: true, hasPreview: true },
    { id: "rotate", icon: Icon_RotateCw, labelKey: "tools.rotate", ready: true, hasPreview: true },
    { id: "delete", icon: Icon_Trash2, labelKey: "tools.deletePages", ready: true, hasPreview: true },
    { id: "extractPages", icon: Icon_FileOutput, labelKey: "tools.extractPages", ready: true, hasPreview: true },
    { id: "compress", icon: Icon_Minimize2, labelKey: "tools.compress", ready: true, hasPreview: false },
    { id: "watermark", icon: Icon_Droplets, labelKey: "tools.watermark", ready: true, hasPreview: true },
    { id: "img2pdf", icon: Icon_Image, labelKey: "tools.convertImage", ready: true, hasPreview: false },
    { id: "pdf2img", icon: Icon_ImageDown, labelKey: "tools.pdf2img", ready: true, hasPreview: false },
    { id: "pdf2text", icon: Icon_FileText, labelKey: "tools.convertText", ready: true, hasPreview: false },
    { id: "sign", icon: Icon_PenTool, labelKey: "tools.sign", ready: true, hasPreview: true },
    { id: "ocr", icon: Icon_ScanLine, labelKey: "tools.ocr", ready: true, hasPreview: false },
    { id: "editText", icon: Icon_Pencil, labelKey: "tools.editText", ready: true, hasPreview: true },
    { id: "editRect", icon: Icon_Square, labelKey: "tools.editRect", ready: true, hasPreview: true },
    { id: "editHighlight", icon: Icon_Highlighter, labelKey: "tools.editHighlight", ready: true, hasPreview: true },
    { id: "table", icon: Icon_Table, labelKey: "tools.extractTable", ready: false, hasPreview: false },
  ];

  // ==================== Thumbnail preview ====================

  async function loadThumbnails() {
    const path = $currentFilePath;
    if (!path) return;
    thumbLoading = true;
    try {
      const data = await readFile(path);
      const doc = await loadPdf(new Uint8Array(data));
      thumbDoc = doc;
      previewPageCount = doc.numPages;
      deletedPages = new Set();
      pageDims = [];
      for (let i = 1; i <= doc.numPages; i++) {
        const p = await doc.getPage(i);
        const vp = p.getViewport({ scale: 1 });
        pageDims.push({ w: vp.width, h: vp.height });
      }
      await tick();
      if (isEditingTool(activeTool)) {
        await renderThumbStrip();
        await renderLargePreview();
      } else {
        await renderThumbnails();
      }
    } catch (_) {
      thumbDoc = null;
      previewPageCount = 0;
    } finally {
      thumbLoading = false;
    }
  }

  async function renderThumbnails() {
    if (!thumbDoc || !thumbContainer) return;
    const version = ++renderVersion;
    const canvases = thumbContainer.querySelectorAll("canvas");
    for (let i = 0; i < canvases.length; i++) {
      if (renderVersion !== version) return;
      try {
        await renderPageToCanvas(
          thumbDoc,
          i + 1,
          canvases[i] as HTMLCanvasElement,
          0.3,
        );
      } catch (_) {}
    }
  }

  // ==================== Large Preview for Editing Tools ====================

  async function renderThumbStrip() {
    if (!thumbDoc || !thumbStripContainer) return;
    const version = ++renderVersion;
    const canvases = thumbStripContainer.querySelectorAll("canvas");
    for (let i = 0; i < canvases.length; i++) {
      if (renderVersion !== version) return;
      try {
        await renderPageToCanvas(thumbDoc, i + 1, canvases[i] as HTMLCanvasElement, 0.15);
      } catch (_) {}
    }
  }

  async function renderLargePreview() {
    if (!thumbDoc || !largeCanvas || !isEditingTool(activeTool)) return;
    const dim = pageDims[previewPage - 1];
    if (!dim) return;

    const MAX_W = 700;
    const scale = Math.min(1.0, MAX_W / dim.w);
    const page = await thumbDoc.getPage(previewPage);
    const vp = page.getViewport({ scale });
    const dpr = window.devicePixelRatio || 1;
    const canvas = largeCanvas;

    canvas.width = Math.floor(vp.width * dpr);
    canvas.height = Math.floor(vp.height * dpr);
    canvas.style.width = `${Math.floor(vp.width)}px`;
    canvas.style.height = `${Math.floor(vp.height)}px`;

    const ctx = canvas.getContext("2d")!;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    await page.render({ canvasContext: ctx, viewport: vp }).promise;

    // Reset transform after pdfjs render
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

    const pageH = vp.height / scale;
    const toX = (pdfX: number) => pdfX * scale;
    const toY = (pdfY: number) => (pageH - pdfY) * scale;

    // --- Edit Text overlay ---
    if (activeTool === "editText" && editText.trim()) {
      ctx.font = `${editFontSize * scale}px Helvetica, sans-serif`;
      ctx.fillStyle = editTextColor;
      ctx.fillText(editText, toX(editTextX), toY(editTextY));
    }

    // --- Rectangle overlay ---
    if (activeTool === "editRect") {
      const cx = toX(editRectX);
      const cy = toY(editRectY + editRectH);
      const cw = editRectW * scale;
      const ch = editRectH * scale;
      if (editRectHasFill) {
        ctx.fillStyle = editRectFill;
        ctx.fillRect(cx, cy, cw, ch);
      }
      ctx.strokeStyle = editRectBorder;
      ctx.lineWidth = editRectBorderW;
      ctx.strokeRect(cx, cy, cw, ch);
    }

    // --- Highlight overlay ---
    if (activeTool === "editHighlight") {
      const cx = toX(editHlX);
      const cy = toY(editHlY + editHlH);
      const cw = editHlW * scale;
      const ch = editHlH * scale;
      ctx.globalAlpha = editHlOpacity;
      ctx.fillStyle = editHlColor;
      ctx.fillRect(cx, cy, cw, ch);
      ctx.globalAlpha = 1.0;
    }

    // --- Watermark overlay ---
    if (activeTool === "watermark" && watermarkText.trim()) {
      const centerX = vp.width / 2;
      const centerY = vp.height / 2;
      ctx.save();
      ctx.globalAlpha = watermarkOpacity;
      ctx.translate(centerX, centerY);
      ctx.rotate((watermarkAngle * Math.PI) / 180);
      ctx.font = `${watermarkFontSize * scale}px Helvetica, sans-serif`;
      ctx.fillStyle = watermarkColor;
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      ctx.fillText(watermarkText, 0, 0);
      ctx.restore();
      ctx.globalAlpha = 1.0;
    }

    // --- Signature overlay ---
    if (activeTool === "sign" && signImagePath) {
      try {
        const img = new Image();
        img.src = `file://${signImagePath}`;
        await img.decode();
        ctx.drawImage(img, toX(signX), toY(signY + signHeight), signWidth * scale, signHeight * scale);
      } catch (_) {}
    }
  }

  // Reactive: re-render large preview when tool params or page change
  $effect(() => {
    if (!activeTool || !thumbDoc || !isEditingTool(activeTool)) return;

    void editText; void editTextPage; void editTextX; void editTextY; void editFontSize; void editTextColor;
    void editRectPage; void editRectX; void editRectY; void editRectW; void editRectH;
    void editRectBorder; void editRectFill; void editRectHasFill; void editRectBorderW;
    void editHlPage; void editHlX; void editHlY; void editHlW; void editHlH; void editHlColor; void editHlOpacity;
    void watermarkText; void watermarkFontSize; void watermarkAngle; void watermarkOpacity; void watermarkColor;
    void signImagePath; void signPage; void signX; void signY; void signWidth; void signHeight;
    void previewPage;

    renderLargePreview();
  });

  // ==================== Mouse Selection on Large Canvas ====================

  function onLargePointerDown(e: PointerEvent) {
    if (!largeCanvas || !isEditingTool(activeTool)) return;
    const rect = largeCanvas.getBoundingClientRect();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    dragState = {
      active: true,
      pageNum: previewPage,
      startX: e.clientX - rect.left,
      startY: e.clientY - rect.top,
      curX: e.clientX - rect.left,
      curY: e.clientY - rect.top,
    };
  }

  function onLargePointerMove(e: PointerEvent) {
    if (!dragState?.active || !largeCanvas) return;
    const rect = largeCanvas.getBoundingClientRect();
    dragState = {
      ...dragState,
      curX: Math.max(0, Math.min(e.clientX - rect.left, rect.width)),
      curY: Math.max(0, Math.min(e.clientY - rect.top, rect.height)),
    };
  }

  function onLargePointerUp(_e: PointerEvent) {
    if (!dragState?.active || !largeCanvas) return;
    const ds = { ...dragState };
    dragState = null;

    const dim = pageDims[ds.pageNum - 1];
    if (!dim) return;
    const rect = largeCanvas.getBoundingClientRect();

    const x1 = Math.min(ds.startX, ds.curX);
    const y1 = Math.min(ds.startY, ds.curY);
    const x2 = Math.max(ds.startX, ds.curX);
    const y2 = Math.max(ds.startY, ds.curY);
    const dragDist = Math.hypot(x2 - x1, y2 - y1);
    const isClick = dragDist < 5;

    const toPdfX = (dx: number) => Math.round(dx / rect.width * dim.w);
    const toPdfY = (dy: number) => Math.round((rect.height - dy) / rect.height * dim.h);

    const pdfX = toPdfX(x1);
    const pdfRight = toPdfX(x2);
    const pdfTop = toPdfY(y1);
    const pdfBottom = toPdfY(y2);

    switch (activeTool) {
      case "editText":
        if (isClick) {
          editTextX = toPdfX(ds.startX);
          editTextY = toPdfY(ds.startY);
        } else {
          editTextX = pdfX;
          editTextY = pdfBottom;
          editFontSize = Math.max(8, Math.round((pdfTop - pdfBottom) / dim.h * 72 * 2));
        }
        break;
      case "editRect":
        editRectX = pdfX;
        editRectY = isClick ? pdfBottom - 50 : pdfBottom;
        editRectW = isClick ? 200 : pdfRight - pdfX;
        editRectH = isClick ? 50 : pdfTop - pdfBottom;
        break;
      case "editHighlight":
        editHlX = pdfX;
        editHlY = isClick ? pdfBottom - 20 : pdfBottom;
        editHlW = isClick ? 200 : pdfRight - pdfX;
        editHlH = isClick ? 20 : pdfTop - pdfBottom;
        break;
      case "sign":
        signX = pdfX;
        signY = isClick ? pdfBottom - 50 : pdfBottom;
        signWidth = isClick ? 150 : Math.max(50, pdfRight - pdfX);
        signHeight = isClick ? 50 : Math.max(20, pdfTop - pdfBottom);
        break;
    }
  }

  // ==================== Merge ====================

  let mergeFiles: string[] = $state([]);

  async function addMergeFiles() {
    const selected = await open({
      multiple: true,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
    if (selected) {
      const paths = Array.isArray(selected)
        ? selected.map(String)
        : [String(selected)];
      mergeFiles = [...mergeFiles, ...paths];
    }
  }

  function removeMergeFile(index: number) {
    mergeFiles = mergeFiles.filter((_, i) => i !== index);
  }

  async function executeMerge() {
    if (mergeFiles.length < 2) return;
    busy = true;
    resultMsg = "";
    try {
      const out = await save({
        filters: [{ name: "PDF", extensions: ["pdf"] }],
      });
      if (!out) return;
      const outPath = out as string;
      // Tauri v2: top-level snake_case params auto-convert to camelCase
      await invoke("merge_pdfs", {
        paths: mergeFiles,
        outputPath: outPath,
      });
      resultMsg = `Merged ${mergeFiles.length} files → ${outPath.split(/[\\/]/).pop()}`;
      resultOk = true;
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
    } finally {
      busy = false;
    }
  }

  // ==================== Rotate ====================

  let rotateAngle = $state(90);

  async function executeRotate() {
    const path = $currentFilePath;
    if (!path) return;
    busy = true;
    resultMsg = "";
    try {
      const out = await save({
        filters: [{ name: "PDF", extensions: ["pdf"] }],
      });
      if (!out) return;
      const outPath = out as string;
      // Struct fields use camelCase via #[serde(rename_all = "camelCase")]
      await invoke("rotate_pdf", {
        req: { inputPath: path, outputPath: outPath, angle: rotateAngle },
      });
      resultMsg = `Rotated ${rotateAngle}° → ${outPath.split(/[\\/]/).pop()}`;
      resultOk = true;
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
    } finally {
      busy = false;
    }
  }

  // ==================== Delete Pages ====================

  let deletedPages = $state<Set<number>>(new Set());
  let deletePagesInput = $state("");

  function toggleDeletePage(pageNum: number) {
    if (deletedPages.has(pageNum)) {
      deletedPages = new Set([...deletedPages].filter((p) => p !== pageNum));
    } else {
      deletedPages = new Set([...deletedPages, pageNum]);
    }
    deletePagesInput = [...deletedPages].sort((a, b) => a - b).join(", ");
  }

  function selectAllPages() {
    deletedPages = new Set(
      Array.from({ length: previewPageCount }, (_, i) => i + 1),
    );
    deletePagesInput = [...deletedPages].join(", ");
  }

  function clearSelection() {
    deletedPages = new Set();
    deletePagesInput = "";
  }

  function parsePageRanges(input: string): number[] {
    const pages: number[] = [];
    for (const part of input.split(",")) {
      const trimmed = part.trim();
      if (!trimmed) continue;
      const rangeMatch = trimmed.match(/^(\d+)-(\d+)$/);
      if (rangeMatch) {
        const s = parseInt(rangeMatch[1]);
        const e = parseInt(rangeMatch[2]);
        if (s <= e) {
          for (let i = s; i <= e; i++) pages.push(i);
        }
      } else {
        const num = parseInt(trimmed);
        if (!isNaN(num) && num > 0) pages.push(num);
      }
    }
    return [...new Set(pages)].sort((a, b) => a - b);
  }

  function applyDeleteInput() {
    deletedPages = new Set(parsePageRanges(deletePagesInput));
  }

  async function executeDeletePages() {
    const path = $currentFilePath;
    if (!path || deletedPages.size === 0) return;
    const pages = [...deletedPages].sort((a, b) => a - b);
    busy = true;
    resultMsg = "";
    try {
      const out = await save({
        filters: [{ name: "PDF", extensions: ["pdf"] }],
      });
      if (!out) return;
      const outPath = out as string;
      await invoke("delete_pages", {
        req: { inputPath: path, outputPath: outPath, pagesToDelete: pages },
      });
      resultMsg = `Deleted ${pages.length} page(s) → ${outPath.split(/[\\/]/).pop()}`;
      resultOk = true;
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
    } finally {
      busy = false;
    }
  }

  // ==================== PDF to Text ====================

  let extractedText = $state("");

  async function executePdf2Text() {
    const path = $currentFilePath;
    if (!path) return;
    busy = true;
    resultMsg = "";
    try {
      // Single-word param: no camelCase conversion needed
      const data = await invoke<{ text: string; pages: number }>(
        "extract_text",
        { path },
      );
      extractedText = data.text;
      resultMsg = `Extracted ${data.pages} page(s), ${data.text.length} chars`;
      resultOk = true;
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
    } finally {
      busy = false;
    }
  }

  async function saveExtractedText() {
    if (!extractedText) return;
    try {
      const out = await save({
        filters: [{ name: "Text", extensions: ["txt"] }],
      });
      if (!out) return;
      const outPath = out as string;
      await writeTextFile(outPath, extractedText);
      resultMsg = `Saved to ${outPath.split(/[\\/]/).pop()}`;
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
    }
  }

  // ==================== Split ====================

  let splitMode = $state<"single" | "range">("single");
  let splitRanges = $state("");

  async function executeSplit() {
    const path = $currentFilePath;
    if (!path) return;
    busy = true;
    resultMsg = "";
    try {
      const selected = await open({ directory: true });
      if (!selected) return;
      const outputDir = String(selected);
      const outPaths = await invoke<string[]>("split_pdf", {
        req: { inputPath: path, outputDir, mode: splitMode, ranges: splitMode === "range" ? splitRanges : null },
      });
      resultMsg = `Split into ${outPaths.length} file(s)`;
      resultOk = true;
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
    } finally {
      busy = false;
    }
  }

  // ==================== Extract Pages ====================

  let extractPagesInput = $state("");
  let selectedExtractPages = $state<Set<number>>(new Set());

  function toggleExtractPage(pageNum: number) {
    if (selectedExtractPages.has(pageNum)) {
      selectedExtractPages = new Set([...selectedExtractPages].filter((p) => p !== pageNum));
    } else {
      selectedExtractPages = new Set([...selectedExtractPages, pageNum]);
    }
    extractPagesInput = [...selectedExtractPages].sort((a, b) => a - b).join(", ");
  }

  function applyExtractInput() {
    selectedExtractPages = new Set(parsePageRanges(extractPagesInput));
  }

  async function executeExtractPages() {
    const path = $currentFilePath;
    if (!path || selectedExtractPages.size === 0) return;
    busy = true;
    resultMsg = "";
    try {
      const out = await save({ filters: [{ name: "PDF", extensions: ["pdf"] }] });
      if (!out) return;
      const outPath = out as string;
      const pages = [...selectedExtractPages].sort((a, b) => a - b);
      await invoke("extract_pages_pdf", {
        req: { inputPath: path, outputPath: outPath, pagesToExtract: pages },
      });
      resultMsg = `Extracted ${pages.length} page(s) → ${outPath.split(/[\\/]/).pop()}`;
      resultOk = true;
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
    } finally {
      busy = false;
    }
  }

  // ==================== Compress ====================

  let compressResult = $state<{ originalSize: number; compressedSize: number; ratio: number } | null>(null);

  function formatBytes(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  async function executeCompress() {
    const path = $currentFilePath;
    if (!path) return;
    busy = true;
    resultMsg = "";
    compressResult = null;
    try {
      const out = await save({ filters: [{ name: "PDF", extensions: ["pdf"] }] });
      if (!out) return;
      const outPath = out as string;
      const data = await invoke<{ originalSize: number; compressedSize: number; ratio: number }>(
        "compress_pdf",
        { inputPath: path, outputPath: outPath },
      );
      compressResult = data;
      resultMsg = `${formatBytes(data.originalSize)} → ${formatBytes(data.compressedSize)} (${data.ratio.toFixed(1)}% smaller)`;
      resultOk = true;
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
    } finally {
      busy = false;
    }
  }

  // ==================== Watermark ====================

  let watermarkText = $state("WATERMARK");
  let watermarkFontSize = $state(48);
  let watermarkOpacity = $state(0.3);
  let watermarkAngle = $state(-45);
  let watermarkColor = $state("#888888");

  async function executeWatermark() {
    const path = $currentFilePath;
    if (!path) return;
    busy = true;
    resultMsg = "";
    try {
      const out = await save({ filters: [{ name: "PDF", extensions: ["pdf"] }] });
      if (!out) return;
      const outPath = out as string;
      await invoke("add_text_watermark", {
        req: {
          inputPath: path, outputPath: outPath,
          text: watermarkText, fontSize: watermarkFontSize,
          opacity: watermarkOpacity, angle: watermarkAngle, color: watermarkColor,
        },
      });
      resultMsg = `Watermark added → ${outPath.split(/[\\/]/).pop()}`;
      resultOk = true;
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
    } finally {
      busy = false;
    }
  }

  // ==================== Image to PDF ====================

  let img2pdfFiles = $state<string[]>([]);

  async function addImg2PdfFiles() {
    const selected = await open({
      multiple: true,
      filters: [{ name: "Images", extensions: ["jpg", "jpeg", "png", "bmp", "webp"] }],
    });
    if (selected) {
      const paths = Array.isArray(selected) ? selected.map(String) : [String(selected)];
      img2pdfFiles = [...img2pdfFiles, ...paths];
    }
  }

  function removeImg2PdfFile(index: number) {
    img2pdfFiles = img2pdfFiles.filter((_, i) => i !== index);
  }

  async function executeImg2Pdf() {
    if (img2pdfFiles.length === 0) return;
    busy = true;
    resultMsg = "";
    try {
      const out = await save({ filters: [{ name: "PDF", extensions: ["pdf"] }] });
      if (!out) return;
      const outPath = out as string;
      await invoke("images_to_pdf", { req: { imagePaths: img2pdfFiles, outputPath: outPath } });
      resultMsg = `${img2pdfFiles.length} image(s) → ${outPath.split(/[\\/]/).pop()}`;
      resultOk = true;
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
    } finally {
      busy = false;
    }
  }

  // ==================== PDF to Image ====================

  let pdf2imgPageRange = $state<"all" | "custom">("all");
  let pdf2imgPages = $state("");

  async function executePdf2Img() {
    const path = $currentFilePath;
    if (!path) return;
    busy = true;
    resultMsg = "";
    try {
      const selected = await open({ directory: true });
      if (!selected) return;
      const outputDir = String(selected);

      const data = await readFile(path);
      const doc = await loadPdf(new Uint8Array(data));

      let pagesToConvert: number[];
      if (pdf2imgPageRange === "all") {
        pagesToConvert = Array.from({ length: doc.numPages }, (_, i) => i + 1);
      } else {
        pagesToConvert = parsePageRanges(pdf2imgPages);
      }

      const baseName = (path.split(/[\\/]/).pop() || "page").replace(/\.pdf$/i, "");
      for (const pageNum of pagesToConvert) {
        const canvas = document.createElement("canvas");
        await renderPageToCanvas(doc, pageNum, canvas, 2);
        const blob = await new Promise<Blob | null>((resolve) =>
          canvas.toBlob((b) => resolve(b), "image/png"),
        );
        if (!blob) continue;
        const arrayBuffer = await blob.arrayBuffer();
        const fileName = `${baseName}_page_${pageNum}.png`;
        await invoke("save_image_file", { path: `${outputDir}/${fileName}`, data: Array.from(new Uint8Array(arrayBuffer)) });
      }

      resultMsg = `Exported ${pagesToConvert.length} page(s) as PNG`;
      resultOk = true;
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
    } finally {
      busy = false;
    }
  }

  // ==================== Sign PDF ====================

  let signImagePath = $state("");
  let signPage = $state(1);
  let signX = $state(400);
  let signY = $state(50);
  let signWidth = $state(150);
  let signHeight = $state(50);

  async function selectSignImage() {
    const selected = await open({
      filters: [{ name: "Images", extensions: ["jpg", "jpeg", "png"] }],
    });
    if (selected) {
      signImagePath = String(selected);
    }
  }

  async function executeSign() {
    const path = $currentFilePath;
    if (!path || !signImagePath) return;
    busy = true;
    resultMsg = "";
    try {
      const out = await save({ filters: [{ name: "PDF", extensions: ["pdf"] }] });
      if (!out) return;
      const outPath = out as string;
      await invoke("sign_pdf", {
        req: {
          inputPath: path, outputPath: outPath,
          signatureImagePath: signImagePath,
          page: signPage, x: signX, y: signY,
          width: signWidth, height: signHeight,
        },
      });
      resultMsg = `Signature added to page ${signPage} → ${outPath.split(/[\\/]/).pop()}`;
      resultOk = true;
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
    } finally {
      busy = false;
    }
  }

  // ==================== OCR ====================

  let ocrLanguage = $state("eng");
  let ocrAvailable = $state<boolean | null>(null);
  let ocrText = $state("");

  async function checkOcr() {
    try {
      ocrAvailable = await invoke<boolean>("check_tesseract_available");
    } catch {
      ocrAvailable = false;
    }
  }

  async function executeOcr() {
    const path = $currentFilePath;
    if (!path) return;
    busy = true;
    resultMsg = "";
    ocrText = "";
    try {
      const data = await readFile(path);
      const doc = await loadPdf(new Uint8Array(data));

      // Phase 1: Try pdfjs-dist built-in text extraction (lightweight, no external deps)
      let totalText = "";
      let textFound = false;
      for (let i = 1; i <= doc.numPages; i++) {
        const page = await doc.getPage(i);
        const content = await page.getTextContent();
        const pageText = content.items
          .map((item: any) => item.str)
          .filter((s: string) => s.trim())
          .join(" ");
        if (pageText.trim()) {
          textFound = true;
          totalText += `\n--- Page ${i} ---\n${pageText}\n`;
        }
      }

      if (textFound && totalText.trim().length > 10) {
        // Sufficient text extracted via pdfjs-dist, no Tesseract needed
        ocrText = totalText;
        resultMsg = `Extracted ${doc.numPages} page(s), ${totalText.length} chars (built-in)`;
        resultOk = true;
      } else {
        // Phase 2: Fall back to Tesseract OCR for scanned/image PDFs
        if (ocrAvailable === null) await checkOcr();
        if (!ocrAvailable) {
          ocrText = totalText;
          resultMsg = "This PDF contains mostly images. Install Tesseract for OCR: https://github.com/tesseract-ocr/tesseract";
          resultOk = totalText.length > 0;
          return;
        }

        const tempDir = await invoke<string>("get_temp_dir", {});
        for (let i = 1; i <= doc.numPages; i++) {
          const canvas = document.createElement("canvas");
          await renderPageToCanvas(doc, i, canvas, 2);
          const blob = await new Promise<Blob | null>((resolve) =>
            canvas.toBlob((b) => resolve(b), "image/png"),
          );
          if (!blob) continue;
          const ab = await blob.arrayBuffer();
          const paddedNum = String(i).padStart(4, "0");
          await invoke("save_image_file", { path: `${tempDir}/ocr_page_${paddedNum}.png`, data: Array.from(new Uint8Array(ab)) });
        }

        const ocrResult = await invoke<{ text: string; pages: number }>("ocr_extract_from_images", {
          req: { imageDir: tempDir, language: ocrLanguage },
        });
        ocrText = ocrResult.text;
        resultMsg = `OCR extracted ${ocrResult.pages} page(s), ${ocrResult.text.length} chars (Tesseract)`;
        resultOk = true;
      }
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
    } finally {
      busy = false;
    }
  }

  async function saveOcrText() {
    if (!ocrText) return;
    try {
      const out = await save({ filters: [{ name: "Text", extensions: ["txt"] }] });
      if (!out) return;
      await writeTextFile(out as string, ocrText);
      resultMsg = `Saved to ${(out as string).split(/[\\/]/).pop()}`;
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
    }
  }

  // ==================== Edit: Add Text ====================

  let editText = $state("");
  let editFontSize = $state(12);
  let editTextColor = $state("#000000");
  let editTextPage = $state(1);
  let editTextX = $state(72);
  let editTextY = $state(720);

  async function executeEditText() {
    const path = $currentFilePath;
    if (!path || !editText.trim()) return;
    busy = true;
    resultMsg = "";
    try {
      const out = await save({ filters: [{ name: "PDF", extensions: ["pdf"] }] });
      if (!out) return;
      const outPath = out as string;
      await invoke("add_text_to_page", {
        req: { inputPath: path, outputPath: outPath, text: editText, page: editTextPage, x: editTextX, y: editTextY, fontSize: editFontSize, color: editTextColor },
      });
      resultMsg = `Text added to page ${editTextPage} → ${outPath.split(/[\\/]/).pop()}`;
      resultOk = true;
    } catch (e) { resultMsg = String(e); resultOk = false; } finally { busy = false; }
  }

  // ==================== Edit: Rectangle ====================

  let editRectPage = $state(1);
  let editRectX = $state(100);
  let editRectY = $state(100);
  let editRectW = $state(200);
  let editRectH = $state(50);
  let editRectBorder = $state("#000000");
  let editRectFill = $state("#ffffff");
  let editRectHasFill = $state(false);
  let editRectBorderW = $state(1);

  async function executeEditRect() {
    const path = $currentFilePath;
    if (!path) return;
    busy = true;
    resultMsg = "";
    try {
      const out = await save({ filters: [{ name: "PDF", extensions: ["pdf"] }] });
      if (!out) return;
      const outPath = out as string;
      await invoke("add_rectangle", {
        req: { inputPath: path, outputPath: outPath, page: editRectPage, x: editRectX, y: editRectY, width: editRectW, height: editRectH, borderColor: editRectBorder, fillColor: editRectHasFill ? editRectFill : null, borderWidth: editRectBorderW },
      });
      resultMsg = `Rectangle added to page ${editRectPage} → ${outPath.split(/[\\/]/).pop()}`;
      resultOk = true;
    } catch (e) { resultMsg = String(e); resultOk = false; } finally { busy = false; }
  }

  // ==================== Edit: Highlight ====================

  let editHlPage = $state(1);
  let editHlX = $state(100);
  let editHlY = $state(100);
  let editHlW = $state(200);
  let editHlH = $state(20);
  let editHlColor = $state("#ffff00");
  let editHlOpacity = $state(0.4);

  async function executeEditHighlight() {
    const path = $currentFilePath;
    if (!path) return;
    busy = true;
    resultMsg = "";
    try {
      const out = await save({ filters: [{ name: "PDF", extensions: ["pdf"] }] });
      if (!out) return;
      const outPath = out as string;
      await invoke("add_highlight", {
        req: { inputPath: path, outputPath: outPath, page: editHlPage, x: editHlX, y: editHlY, width: editHlW, height: editHlH, color: editHlColor, opacity: editHlOpacity },
      });
      resultMsg = `Highlight added to page ${editHlPage} → ${outPath.split(/[\\/]/).pop()}`;
      resultOk = true;
    } catch (e) { resultMsg = String(e); resultOk = false; } finally { busy = false; }
  }

  // ==================== Navigation ====================

  async function openFileForTool() {
    const selected = await open({
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
    if (selected) {
      const path = typeof selected === "string" ? selected : String(selected);
      currentFilePath.set(path);
    }
  }

  function openCurrentInViewer() {
    if ($currentFilePath) currentView.set("viewer");
  }

  function selectTool(id: ToolId) {
    activeTool = id;
    resultMsg = "";
    extractedText = "";
    ocrText = "";
    compressResult = null;
    const def = toolDefs.find((t) => t.id === id);
    if (def?.hasPreview && $currentFilePath) {
      loadThumbnails();
    } else {
      thumbDoc = null;
      previewPageCount = 0;
    }
    if (id === "pdf2text" && $currentFilePath) executePdf2Text();
    if (id === "ocr") checkOcr();
  }

  function getThumbClasses(pageNum: number): string {
    const base =
      "relative rounded-lg overflow-hidden border-2 transition-all cursor-pointer hover:ring-2 hover:ring-primary/20";
    if (activeTool === "delete") {
      return deletedPages.has(pageNum)
        ? `${base} border-red-500`
        : `${base} border-transparent hover:border-border`;
    }
    if (activeTool === "extractPages") {
      return selectedExtractPages.has(pageNum)
        ? `${base} border-green-500`
        : `${base} border-transparent hover:border-border`;
    }
    if (activeTool === "editText" && pageNum === editTextPage) return `${base} border-blue-500`;
    if (activeTool === "editRect" && pageNum === editRectPage) return `${base} border-blue-500`;
    if (activeTool === "editHighlight" && pageNum === editHlPage) return `${base} border-blue-500`;
    if (activeTool === "sign" && pageNum === signPage) return `${base} border-blue-500`;
    return `${base} border-transparent`;
  }

  const previewPage = $derived(
    activeTool === "editText" ? editTextPage :
    activeTool === "editRect" ? editRectPage :
    activeTool === "editHighlight" ? editHlPage :
    activeTool === "sign" ? signPage :
    1
  );

  const showPreview = $derived(
    thumbDoc &&
      previewPageCount > 0 &&
      activeTool !== null &&
      toolDefs.find((t) => t.id === activeTool)?.hasPreview,
  );
</script>

<div class="flex flex-col h-full">
  <!-- Header -->
  <div
    class="flex items-center h-12 px-6 border-b border-border bg-card shrink-0"
  >
    {#if activeTool}
      <button
        onclick={() => (activeTool = null)}
        class="flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground transition-colors"
      >
        <Icon_ArrowLeft size={16} />
        <span>{t("nav.tools")}</span>
      </button>
      <span class="mx-2 text-border">/</span>
      <span class="text-sm font-medium text-foreground">
        {t(toolDefs.find((t) => t.id === activeTool)?.labelKey ?? "")}
      </span>
    {:else}
      <h1 class="text-base font-semibold text-foreground">
        {t("nav.tools")}
      </h1>
    {/if}
  </div>

  <div class="flex-1 overflow-auto">
    {#if !activeTool}
      <!-- Tool Grid -->
      <div class="p-6 grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 gap-3">
        {#each toolDefs as tool}
          {@const Icon = tool.icon}
          <button
            onclick={() => selectTool(tool.id)}
            class="flex flex-col items-center gap-3 p-5 rounded-xl border border-border hover:bg-accent hover:border-accent-foreground/20 transition-all group {tool.ready
              ? ''
              : 'opacity-60'}"
          >
            <div
              class="p-3 rounded-lg bg-muted group-hover:bg-primary/10 transition-colors"
            >
              <Icon
                size={22}
                class="text-muted-foreground group-hover:text-primary transition-colors"
              />
            </div>
            <span
              class="text-sm text-muted-foreground group-hover:text-foreground transition-colors"
            >
              {t(tool.labelKey)}
            </span>
            {#if !tool.ready}
              <span class="text-[10px] text-muted-foreground/60"
                >Coming soon</span
              >
            {/if}
          </button>
        {/each}
      </div>
    {:else}
      <!-- Tool Panel -->
      <div
        class="p-6 {showPreview ? 'max-w-6xl' : 'max-w-2xl'} mx-auto space-y-4"
      >
        <!-- Merge -->
        {#if activeTool === "merge"}
          <div class="space-y-3">
            <p class="text-sm text-muted-foreground">
              Select PDF files to merge in order.
            </p>
            <Button
              variant="outline"
              size="sm"
              onclick={addMergeFiles}
              class="gap-1.5"
            >
              <Icon_FileUp size={14} />
              Add Files
            </Button>
            {#if mergeFiles.length > 0}
              <div class="space-y-1">
                {#each mergeFiles as file, i}
                  <div
                    class="flex items-center gap-2 px-3 py-2 rounded-lg bg-muted text-sm"
                  >
                    <span class="text-muted-foreground w-6">{i + 1}.</span>
                    <span class="flex-1 truncate text-foreground">
                      {file.split(/[\\/]/).pop()}
                    </span>
                    <button
                      onclick={() => removeMergeFile(i)}
                      class="text-muted-foreground hover:text-destructive text-xs"
                    >
                      Remove
                    </button>
                  </div>
                {/each}
              </div>
              <Button
                onclick={executeMerge}
                disabled={busy || mergeFiles.length < 2}
              >
                {#if busy}
                  <Icon_Loader2 size={14} class="animate-spin" />
                {:else}
                  Merge ({mergeFiles.length} files)
                {/if}
              </Button>
            {/if}
          </div>
        {/if}

        <!-- Rotate -->
        {#if activeTool === "rotate"}
          <div class="space-y-3">
            {#if !$currentFilePath}
              <p class="text-sm text-muted-foreground">Open a PDF first.</p>
              <div class="flex gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  onclick={openFileForTool}
                >
                  <Icon_FileUp size={14} class="mr-1.5" />
                  Open PDF
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  onclick={openCurrentInViewer}
                >
                  Go to Viewer
                </Button>
              </div>
            {:else}
              <p class="text-sm text-muted-foreground">
                Rotate:
                <strong
                  >{$currentFilePath.split(/[\\/]/).pop()}</strong
                >
              </p>
              <div class="flex gap-2">
                {#each [90, 180, 270] as angle}
                  <Button
                    variant={rotateAngle === angle ? "default" : "outline"}
                    size="sm"
                    onclick={() => (rotateAngle = angle)}
                  >
                    {angle}°
                  </Button>
                {/each}
              </div>
              <Button onclick={executeRotate} disabled={busy}>
                {#if busy}
                  <Icon_Loader2 size={14} class="animate-spin" />
                {:else}
                  <Icon_Save size={14} class="mr-1.5" />
                  Rotate & Save
                {/if}
              </Button>
            {/if}
          </div>
        {/if}

        <!-- Delete Pages -->
        {#if activeTool === "delete"}
          <div class="space-y-3">
            {#if !$currentFilePath}
              <p class="text-sm text-muted-foreground">Open a PDF first.</p>
              <div class="flex gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  onclick={openFileForTool}
                >
                  <Icon_FileUp size={14} class="mr-1.5" />
                  Open PDF
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  onclick={openCurrentInViewer}
                >
                  Go to Viewer
                </Button>
              </div>
            {:else}
              <p class="text-sm text-muted-foreground">
                Delete pages from:
                <strong
                  >{$currentFilePath.split(/[\\/]/).pop()}</strong
                >
              </p>
              <div class="space-y-1.5">
                <div class="flex items-center gap-2">
                  <Label class="shrink-0">Pages</Label>
                  <Input
                    value={deletePagesInput}
                    onchange={(e) => {
                      deletePagesInput = (e.target as HTMLInputElement).value;
                      applyDeleteInput();
                    }}
                    placeholder="e.g. 1,3,5-7"
                    class="flex-1"
                  />
                </div>
                <div class="flex items-center gap-2 text-xs text-muted-foreground">
                  <span
                    >Click thumbnails or type page numbers (supports ranges).</span
                  >
                  {#if deletedPages.size > 0}
                    <span class="text-destructive font-medium">
                      {deletedPages.size} page(s) selected
                    </span>
                  {/if}
                </div>
              </div>
              <div class="flex gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  onclick={selectAllPages}
                >
                  Select All
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  onclick={clearSelection}
                >
                  Clear
                </Button>
                <Button
                  onclick={executeDeletePages}
                  disabled={busy || deletedPages.size === 0}
                >
                  {#if busy}
                    <Icon_Loader2 size={14} class="animate-spin" />
                  {:else}
                    <Icon_Save size={14} class="mr-1.5" />
                    Delete & Save
                  {/if}
                </Button>
              </div>
            {/if}
          </div>
        {/if}

        <!-- PDF to Text -->
        {#if activeTool === "pdf2text"}
          <div class="space-y-3">
            {#if !$currentFilePath}
              <p class="text-sm text-muted-foreground">Open a PDF first.</p>
              <div class="flex gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  onclick={openFileForTool}
                >
                  <Icon_FileUp size={14} class="mr-1.5" />
                  Open PDF
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  onclick={openCurrentInViewer}
                >
                  Go to Viewer
                </Button>
              </div>
            {:else}
              <div class="flex items-center gap-2">
                <p class="text-sm text-muted-foreground">
                  Extract text from:
                  <strong
                    >{$currentFilePath.split(/[\\/]/).pop()}</strong
                  >
                </p>
                <Button
                  variant="outline"
                  size="sm"
                  onclick={executePdf2Text}
                  disabled={busy}
                >
                  {#if busy}
                    <Icon_Loader2 size={14} class="animate-spin" />
                  {:else}
                    Extract
                  {/if}
                </Button>
              </div>
              {#if extractedText}
                <textarea
                  readonly
                  value={extractedText}
                  class="w-full h-64 p-3 rounded-lg border border-input bg-muted text-sm font-mono resize-y"
                ></textarea>
                <div class="flex gap-2">
                  <Button
                    variant="outline"
                    size="sm"
                    onclick={saveExtractedText}
                  >
                    <Icon_Save size={14} class="mr-1.5" />
                    Save as .txt
                  </Button>
                  <Button
                    variant="outline"
                    size="sm"
                    onclick={() => navigator.clipboard.writeText(extractedText)}
                  >
                    Copy All
                  </Button>
                </div>
              {/if}
            {/if}
          </div>
        {/if}

        <!-- Split -->
        {#if activeTool === "split"}
          <div class="space-y-3">
            {#if !$currentFilePath}
              <p class="text-sm text-muted-foreground">Open a PDF first.</p>
              <div class="flex gap-2">
                <Button variant="outline" size="sm" onclick={openFileForTool}>
                  <Icon_FileUp size={14} class="mr-1.5" />
                  Open PDF
                </Button>
              </div>
            {:else}
              <p class="text-sm text-muted-foreground">
                Split:
                <strong>{$currentFilePath.split(/[\\/]/).pop()}</strong>
              </p>
              <div class="flex gap-2">
                <Button
                  variant={splitMode === "single" ? "default" : "outline"}
                  size="sm"
                  onclick={() => (splitMode = "single")}
                >
                  One file per page
                </Button>
                <Button
                  variant={splitMode === "range" ? "default" : "outline"}
                  size="sm"
                  onclick={() => (splitMode = "range")}
                >
                  By ranges
                </Button>
              </div>
              {#if splitMode === "range"}
                <div class="flex items-center gap-2">
                  <Label class="shrink-0">Ranges</Label>
                  <Input
                    value={splitRanges}
                    onchange={(e) => (splitRanges = (e.target as HTMLInputElement).value)}
                    placeholder="e.g. 1-3,4-6,7-10"
                    class="flex-1"
                  />
                </div>
              {/if}
              <Button onclick={executeSplit} disabled={busy || (splitMode === "range" && !splitRanges.trim())}>
                {#if busy}
                  <Icon_Loader2 size={14} class="animate-spin" />
                {:else}
                  <Icon_FolderOpen size={14} class="mr-1.5" />
                  Split &amp; Choose Output Folder
                {/if}
              </Button>
            {/if}
          </div>
        {/if}

        <!-- Extract Pages -->
        {#if activeTool === "extractPages"}
          <div class="space-y-3">
            {#if !$currentFilePath}
              <p class="text-sm text-muted-foreground">Open a PDF first.</p>
              <div class="flex gap-2">
                <Button variant="outline" size="sm" onclick={openFileForTool}>
                  <Icon_FileUp size={14} class="mr-1.5" />
                  Open PDF
                </Button>
              </div>
            {:else}
              <p class="text-sm text-muted-foreground">
                Extract pages from:
                <strong>{$currentFilePath.split(/[\\/]/).pop()}</strong>
              </p>
              <div class="space-y-1.5">
                <div class="flex items-center gap-2">
                  <Label class="shrink-0">Pages</Label>
                  <Input
                    value={extractPagesInput}
                    onchange={(e) => {
                      extractPagesInput = (e.target as HTMLInputElement).value;
                      applyExtractInput();
                    }}
                    placeholder="e.g. 1,3,5-7"
                    class="flex-1"
                  />
                </div>
                <div class="flex items-center gap-2 text-xs text-muted-foreground">
                  <span>Click thumbnails or type page numbers.</span>
                  {#if selectedExtractPages.size > 0}
                    <span class="text-green-600 font-medium">
                      {selectedExtractPages.size} page(s) selected
                    </span>
                  {/if}
                </div>
              </div>
              <Button
                onclick={executeExtractPages}
                disabled={busy || selectedExtractPages.size === 0}
              >
                {#if busy}
                  <Icon_Loader2 size={14} class="animate-spin" />
                {:else}
                  <Icon_Save size={14} class="mr-1.5" />
                  Extract &amp; Save
                {/if}
              </Button>
            {/if}
          </div>
        {/if}

        <!-- Compress -->
        {#if activeTool === "compress"}
          <div class="space-y-3">
            {#if !$currentFilePath}
              <p class="text-sm text-muted-foreground">Open a PDF first.</p>
              <div class="flex gap-2">
                <Button variant="outline" size="sm" onclick={openFileForTool}>
                  <Icon_FileUp size={14} class="mr-1.5" />
                  Open PDF
                </Button>
              </div>
            {:else}
              <p class="text-sm text-muted-foreground">
                Compress:
                <strong>{$currentFilePath.split(/[\\/]/).pop()}</strong>
              </p>
              <Button onclick={executeCompress} disabled={busy}>
                {#if busy}
                  <Icon_Loader2 size={14} class="animate-spin" />
                {:else}
                  <Icon_Minimize2 size={14} class="mr-1.5" />
                  Compress &amp; Save
                {/if}
              </Button>
              {#if compressResult}
                <div class="p-3 rounded-lg border border-border bg-muted text-sm space-y-1">
                  <div class="flex justify-between">
                    <span class="text-muted-foreground">Original:</span>
                    <span class="font-medium">{formatBytes(compressResult.originalSize)}</span>
                  </div>
                  <div class="flex justify-between">
                    <span class="text-muted-foreground">Compressed:</span>
                    <span class="font-medium">{formatBytes(compressResult.compressedSize)}</span>
                  </div>
                  <div class="flex justify-between">
                    <span class="text-muted-foreground">Saved:</span>
                    <span class="font-medium text-green-600">{compressResult.ratio.toFixed(1)}%</span>
                  </div>
                </div>
              {/if}
            {/if}
          </div>
        {/if}

        <!-- Watermark -->
        {#if activeTool === "watermark"}
          <div class="space-y-3">
            {#if !$currentFilePath}
              <p class="text-sm text-muted-foreground">Open a PDF first.</p>
              <div class="flex gap-2">
                <Button variant="outline" size="sm" onclick={openFileForTool}>
                  <Icon_FileUp size={14} class="mr-1.5" />
                  Open PDF
                </Button>
              </div>
            {:else}
              <p class="text-sm text-muted-foreground">
                Add watermark to:
                <strong>{$currentFilePath.split(/[\\/]/).pop()}</strong>
              </p>
              <div class="flex items-center gap-2">
                <Label class="shrink-0">Text</Label>
                <Input value={watermarkText} onchange={(e) => (watermarkText = (e.target as HTMLInputElement).value)} class="flex-1" />
              </div>
              <div class="grid grid-cols-2 gap-3">
                <div class="space-y-1">
                  <Label>Font Size</Label>
                  <Input type="number" value={watermarkFontSize} onchange={(e) => (watermarkFontSize = parseFloat((e.target as HTMLInputElement).value) || 48)} />
                </div>
                <div class="space-y-1">
                  <Label>Angle</Label>
                  <Input type="number" value={watermarkAngle} onchange={(e) => (watermarkAngle = parseFloat((e.target as HTMLInputElement).value) || -45)} />
                </div>
                <div class="space-y-1">
                  <Label>Opacity (0-1)</Label>
                  <Input type="number" step="0.05" min="0" max="1" value={watermarkOpacity} onchange={(e) => (watermarkOpacity = parseFloat((e.target as HTMLInputElement).value) || 0.3)} />
                </div>
                <div class="space-y-1">
                  <Label>Color</Label>
                  <div class="flex items-center gap-2">
                    <input type="color" bind:value={watermarkColor} class="w-8 h-8 rounded cursor-pointer" />
                    <Input value={watermarkColor} onchange={(e) => (watermarkColor = (e.target as HTMLInputElement).value)} class="flex-1" />
                  </div>
                </div>
              </div>
              <Button onclick={executeWatermark} disabled={busy || !watermarkText.trim()}>
                {#if busy}
                  <Icon_Loader2 size={14} class="animate-spin" />
                {:else}
                  <Icon_Save size={14} class="mr-1.5" />
                  Add Watermark &amp; Save
                {/if}
              </Button>
            {/if}
          </div>
        {/if}

        <!-- Image to PDF -->
        {#if activeTool === "img2pdf"}
          <div class="space-y-3">
            <p class="text-sm text-muted-foreground">
              Select images to convert to a single PDF.
            </p>
            <Button variant="outline" size="sm" onclick={addImg2PdfFiles} class="gap-1.5">
              <Icon_FileUp size={14} />
              Add Images
            </Button>
            {#if img2pdfFiles.length > 0}
              <div class="space-y-1">
                {#each img2pdfFiles as file, i}
                  <div class="flex items-center gap-2 px-3 py-2 rounded-lg bg-muted text-sm">
                    <span class="text-muted-foreground w-6">{i + 1}.</span>
                    <span class="flex-1 truncate text-foreground">
                      {file.split(/[\\/]/).pop()}
                    </span>
                    <button
                      onclick={() => removeImg2PdfFile(i)}
                      class="text-muted-foreground hover:text-destructive text-xs"
                    >
                      Remove
                    </button>
                  </div>
                {/each}
              </div>
              <Button onclick={executeImg2Pdf} disabled={busy}>
                {#if busy}
                  <Icon_Loader2 size={14} class="animate-spin" />
                {:else}
                  Convert to PDF ({img2pdfFiles.length} images)
                {/if}
              </Button>
            {/if}
          </div>
        {/if}

        <!-- PDF to Image -->
        {#if activeTool === "pdf2img"}
          <div class="space-y-3">
            {#if !$currentFilePath}
              <p class="text-sm text-muted-foreground">Open a PDF first.</p>
              <div class="flex gap-2">
                <Button variant="outline" size="sm" onclick={openFileForTool}>
                  <Icon_FileUp size={14} class="mr-1.5" />
                  Open PDF
                </Button>
              </div>
            {:else}
              <p class="text-sm text-muted-foreground">
                Convert to images:
                <strong>{$currentFilePath.split(/[\\/]/).pop()}</strong>
              </p>
              <div class="flex gap-2">
                <Button
                  variant={pdf2imgPageRange === "all" ? "default" : "outline"}
                  size="sm"
                  onclick={() => (pdf2imgPageRange = "all")}
                >
                  All pages
                </Button>
                <Button
                  variant={pdf2imgPageRange === "custom" ? "default" : "outline"}
                  size="sm"
                  onclick={() => (pdf2imgPageRange = "custom")}
                >
                  Custom range
                </Button>
              </div>
              {#if pdf2imgPageRange === "custom"}
                <Input
                  value={pdf2imgPages}
                  onchange={(e) => (pdf2imgPages = (e.target as HTMLInputElement).value)}
                  placeholder="e.g. 1,3,5-7"
                />
              {/if}
              <Button onclick={executePdf2Img} disabled={busy}>
                {#if busy}
                  <Icon_Loader2 size={14} class="animate-spin" />
                {:else}
                  <Icon_ImageDown size={14} class="mr-1.5" />
                  Export as PNG
                {/if}
              </Button>
            {/if}
          </div>
        {/if}

        <!-- Sign PDF -->
        {#if activeTool === "sign"}
          <div class="space-y-3">
            {#if !$currentFilePath}
              <p class="text-sm text-muted-foreground">Open a PDF first.</p>
              <div class="flex gap-2">
                <Button variant="outline" size="sm" onclick={openFileForTool}>
                  <Icon_FileUp size={14} class="mr-1.5" />
                  Open PDF
                </Button>
              </div>
            {:else}
              <p class="text-sm text-muted-foreground">
                Sign:
                <strong>{$currentFilePath.split(/[\\/]/).pop()}</strong>
              </p>
              <Button variant="outline" size="sm" onclick={selectSignImage} class="gap-1.5">
                <Icon_FileUp size={14} />
                {signImagePath ? signImagePath.split(/[\\/]/).pop() : "Select Signature Image"}
              </Button>
              <div class="grid grid-cols-2 gap-3">
                <div class="space-y-1">
                  <Label>Page</Label>
                  <Input type="number" min="1" value={signPage} onchange={(e) => (signPage = parseInt((e.target as HTMLInputElement).value) || 1)} />
                </div>
                <div class="space-y-1"></div>
                <div class="space-y-1">
                  <Label>X Position</Label>
                  <Input type="number" value={signX} onchange={(e) => (signX = parseFloat((e.target as HTMLInputElement).value) || 0)} />
                </div>
                <div class="space-y-1">
                  <Label>Y Position</Label>
                  <Input type="number" value={signY} onchange={(e) => (signY = parseFloat((e.target as HTMLInputElement).value) || 0)} />
                </div>
                <div class="space-y-1">
                  <Label>Width</Label>
                  <Input type="number" value={signWidth} onchange={(e) => (signWidth = parseFloat((e.target as HTMLInputElement).value) || 150)} />
                </div>
                <div class="space-y-1">
                  <Label>Height</Label>
                  <Input type="number" value={signHeight} onchange={(e) => (signHeight = parseFloat((e.target as HTMLInputElement).value) || 50)} />
                </div>
              </div>
              <Button onclick={executeSign} disabled={busy || !signImagePath}>
                {#if busy}
                  <Icon_Loader2 size={14} class="animate-spin" />
                {:else}
                  <Icon_Save size={14} class="mr-1.5" />
                  Sign &amp; Save
                {/if}
              </Button>
            {/if}
          </div>
        {/if}

        <!-- OCR -->
        {#if activeTool === "ocr"}
          <div class="space-y-3">
            {#if !$currentFilePath}
              <p class="text-sm text-muted-foreground">Open a PDF first.</p>
              <div class="flex gap-2">
                <Button variant="outline" size="sm" onclick={openFileForTool}>
                  <Icon_FileUp size={14} class="mr-1.5" />
                  Open PDF
                </Button>
              </div>
            {:else}
              <p class="text-sm text-muted-foreground">
                OCR text extraction from:
                <strong>{$currentFilePath.split(/[\\/]/).pop()}</strong>
              </p>
              <div class="flex items-center gap-3">
                <div class="flex items-center gap-2">
                  <Label class="shrink-0">Language</Label>
                  <Input value={ocrLanguage} onchange={(e) => (ocrLanguage = (e.target as HTMLInputElement).value)} placeholder="e.g. eng, chi_sim, eng+chi_sim" class="w-48" />
                </div>
                <span class="text-xs {ocrAvailable === true ? 'text-green-600' : ocrAvailable === false ? 'text-destructive' : 'text-muted-foreground'}">
                  {ocrAvailable === true ? 'Tesseract ready' : ocrAvailable === false ? 'Tesseract not found' : 'Checking...'}
                </span>
              </div>
              {#if ocrAvailable === false}
                <p class="text-xs text-muted-foreground">
                  Install Tesseract OCR from <a href="https://github.com/tesseract-ocr/tesseract" target="_blank" rel="noopener" class="underline text-primary">github.com/tesseract-ocr/tesseract</a>
                </p>
              {/if}
              <Button onclick={executeOcr} disabled={busy || ocrAvailable === false}>
                {#if busy}
                  <Icon_Loader2 size={14} class="animate-spin" />
                {:else}
                  <Icon_ScanLine size={14} class="mr-1.5" />
                  Run OCR
                {/if}
              </Button>
              {#if ocrText}
                <textarea
                  readonly
                  value={ocrText}
                  class="w-full h-64 p-3 rounded-lg border border-input bg-muted text-sm font-mono resize-y"
                ></textarea>
                <div class="flex gap-2">
                  <Button variant="outline" size="sm" onclick={saveOcrText}>
                    <Icon_Save size={14} class="mr-1.5" />
                    Save as .txt
                  </Button>
                  <Button variant="outline" size="sm" onclick={() => navigator.clipboard.writeText(ocrText)}>
                    Copy All
                  </Button>
                </div>
              {/if}
            {/if}
          </div>
        {/if}

        <!-- Edit: Add Text -->
        {#if activeTool === "editText"}
          <div class="space-y-3">
            {#if !$currentFilePath}
              <p class="text-sm text-muted-foreground">Open a PDF first.</p>
              <div class="flex gap-2">
                <Button variant="outline" size="sm" onclick={openFileForTool}>
                  <Icon_FileUp size={14} class="mr-1.5" />
                  Open PDF
                </Button>
              </div>
            {:else}
              <p class="text-sm text-muted-foreground">
                Add text to:
                <strong>{$currentFilePath.split(/[\\/]/).pop()}</strong>
              </p>
              <div class="space-y-1">
                <Label>Text</Label>
                <textarea
                  value={editText}
                  onchange={(e) => (editText = (e.target as HTMLTextAreaElement).value)}
                  rows="2"
                  class="w-full p-2 rounded-lg border border-input bg-transparent text-sm resize-y"
                  placeholder="Enter text to add..."
                ></textarea>
              </div>
              <div class="grid grid-cols-3 gap-3">
                <div class="space-y-1">
                  <Label>Page</Label>
                  <Input type="number" min="1" value={editTextPage} onchange={(e) => (editTextPage = parseInt((e.target as HTMLInputElement).value) || 1)} />
                </div>
                <div class="space-y-1">
                  <Label>Font Size</Label>
                  <Input type="number" value={editFontSize} onchange={(e) => (editFontSize = parseFloat((e.target as HTMLInputElement).value) || 12)} />
                </div>
                <div class="space-y-1">
                  <Label>Color</Label>
                  <div class="flex items-center gap-2">
                    <input type="color" bind:value={editTextColor} class="w-8 h-8 rounded cursor-pointer" />
                    <Input value={editTextColor} onchange={(e) => (editTextColor = (e.target as HTMLInputElement).value)} class="flex-1" />
                  </div>
                </div>
              </div>
              <p class="text-xs text-muted-foreground">Position (PDF coordinates: origin at bottom-left)</p>
              <div class="grid grid-cols-2 gap-3">
                <div class="space-y-1">
                  <Label>X</Label>
                  <Input type="number" value={editTextX} onchange={(e) => (editTextX = parseFloat((e.target as HTMLInputElement).value) || 0)} />
                </div>
                <div class="space-y-1">
                  <Label>Y</Label>
                  <Input type="number" value={editTextY} onchange={(e) => (editTextY = parseFloat((e.target as HTMLInputElement).value) || 0)} />
                </div>
              </div>
              <Button onclick={executeEditText} disabled={busy || !editText.trim()}>
                {#if busy}<Icon_Loader2 size={14} class="animate-spin" />{:else}<Icon_Save size={14} class="mr-1.5" />Add Text & Save}{/if}
              </Button>
            {/if}
          </div>
        {/if}

        <!-- Edit: Rectangle -->
        {#if activeTool === "editRect"}
          <div class="space-y-3">
            {#if !$currentFilePath}
              <p class="text-sm text-muted-foreground">Open a PDF first.</p>
              <div class="flex gap-2">
                <Button variant="outline" size="sm" onclick={openFileForTool}>
                  <Icon_FileUp size={14} class="mr-1.5" />
                  Open PDF
                </Button>
              </div>
            {:else}
              <p class="text-sm text-muted-foreground">
                Add rectangle to:
                <strong>{$currentFilePath.split(/[\\/]/).pop()}</strong>
              </p>
              <div class="grid grid-cols-2 gap-3">
                <div class="space-y-1"><Label>Page</Label><Input type="number" min="1" value={editRectPage} onchange={(e) => (editRectPage = parseInt((e.target as HTMLInputElement).value) || 1)} /></div>
                <div class="space-y-1"><Label>Border Width</Label><Input type="number" value={editRectBorderW} onchange={(e) => (editRectBorderW = parseFloat((e.target as HTMLInputElement).value) || 1)} /></div>
                <div class="space-y-1"><Label>X</Label><Input type="number" value={editRectX} onchange={(e) => (editRectX = parseFloat((e.target as HTMLInputElement).value) || 0)} /></div>
                <div class="space-y-1"><Label>Y</Label><Input type="number" value={editRectY} onchange={(e) => (editRectY = parseFloat((e.target as HTMLInputElement).value) || 0)} /></div>
                <div class="space-y-1"><Label>Width</Label><Input type="number" value={editRectW} onchange={(e) => (editRectW = parseFloat((e.target as HTMLInputElement).value) || 200)} /></div>
                <div class="space-y-1"><Label>Height</Label><Input type="number" value={editRectH} onchange={(e) => (editRectH = parseFloat((e.target as HTMLInputElement).value) || 50)} /></div>
              </div>
              <div class="flex items-center gap-4">
                <div class="space-y-1">
                  <Label>Border Color</Label>
                  <div class="flex items-center gap-2">
                    <input type="color" bind:value={editRectBorder} class="w-8 h-8 rounded cursor-pointer" />
                    <Input value={editRectBorder} onchange={(e) => (editRectBorder = (e.target as HTMLInputElement).value)} class="w-24" />
                  </div>
                </div>
                <label class="flex items-center gap-2 text-sm cursor-pointer">
                  <input type="checkbox" bind:checked={editRectHasFill} class="rounded" />
                  Fill
                </label>
                {#if editRectHasFill}
                  <div class="space-y-1">
                    <Label>Fill Color</Label>
                    <div class="flex items-center gap-2">
                      <input type="color" bind:value={editRectFill} class="w-8 h-8 rounded cursor-pointer" />
                      <Input value={editRectFill} onchange={(e) => (editRectFill = (e.target as HTMLInputElement).value)} class="w-24" />
                    </div>
                  </div>
                {/if}
              </div>
              <Button onclick={executeEditRect} disabled={busy}>
                {#if busy}<Icon_Loader2 size={14} class="animate-spin" />{:else}<Icon_Save size={14} class="mr-1.5" />Add Rectangle & Save}{/if}
              </Button>
            {/if}
          </div>
        {/if}

        <!-- Edit: Highlight -->
        {#if activeTool === "editHighlight"}
          <div class="space-y-3">
            {#if !$currentFilePath}
              <p class="text-sm text-muted-foreground">Open a PDF first.</p>
              <div class="flex gap-2">
                <Button variant="outline" size="sm" onclick={openFileForTool}>
                  <Icon_FileUp size={14} class="mr-1.5" />
                  Open PDF
                </Button>
              </div>
            {:else}
              <p class="text-sm text-muted-foreground">
                Add highlight to:
                <strong>{$currentFilePath.split(/[\\/]/).pop()}</strong>
              </p>
              <div class="grid grid-cols-2 gap-3">
                <div class="space-y-1"><Label>Page</Label><Input type="number" min="1" value={editHlPage} onchange={(e) => (editHlPage = parseInt((e.target as HTMLInputElement).value) || 1)} /></div>
                <div class="space-y-1"><Label>Opacity (0-1)</Label><Input type="number" step="0.05" min="0" max="1" value={editHlOpacity} onchange={(e) => (editHlOpacity = parseFloat((e.target as HTMLInputElement).value) || 0.4)} /></div>
                <div class="space-y-1"><Label>X</Label><Input type="number" value={editHlX} onchange={(e) => (editHlX = parseFloat((e.target as HTMLInputElement).value) || 0)} /></div>
                <div class="space-y-1"><Label>Y</Label><Input type="number" value={editHlY} onchange={(e) => (editHlY = parseFloat((e.target as HTMLInputElement).value) || 0)} /></div>
                <div class="space-y-1"><Label>Width</Label><Input type="number" value={editHlW} onchange={(e) => (editHlW = parseFloat((e.target as HTMLInputElement).value) || 200)} /></div>
                <div class="space-y-1"><Label>Height</Label><Input type="number" value={editHlH} onchange={(e) => (editHlH = parseFloat((e.target as HTMLInputElement).value) || 20)} /></div>
              </div>
              <div class="space-y-1">
                <Label>Color</Label>
                <div class="flex items-center gap-2">
                  <input type="color" bind:value={editHlColor} class="w-8 h-8 rounded cursor-pointer" />
                  <Input value={editHlColor} onchange={(e) => (editHlColor = (e.target as HTMLInputElement).value)} class="w-24" />
                </div>
              </div>
              <Button onclick={executeEditHighlight} disabled={busy}>
                {#if busy}<Icon_Loader2 size={14} class="animate-spin" />{:else}<Icon_Save size={14} class="mr-1.5" />Add Highlight & Save}{/if}
              </Button>
            {/if}
          </div>
        {/if}


        <!-- Large Preview for editing tools -->
        {#if showPreview && isEditingTool(activeTool)}
          <div class="border-t border-border pt-4 mt-2">
            <div class="flex items-center justify-between mb-2">
              <span class="text-sm font-medium text-muted-foreground">
                {t("tools.preview")} — {previewPage} / {previewPageCount}
              </span>
              {#if thumbLoading}
                <Icon_Loader2 size={14} class="animate-spin text-muted-foreground" />
              {/if}
            </div>
            <div class="flex justify-center overflow-auto border rounded-lg p-2"
                 style:filter={$isDark ? "invert(0.92) hue-rotate(180deg)" : "none"}>
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div class="relative"
                   onpointerdown={onLargePointerDown}
                   onpointermove={onLargePointerMove}
                   onpointerup={onLargePointerUp}
                   style:cursor="crosshair">
                <canvas bind:this={largeCanvas} class="block rounded"></canvas>
                {#if dragState?.active}
                  {@const sx = Math.min(dragState.startX, dragState.curX)}
                  {@const sy = Math.min(dragState.startY, dragState.curY)}
                  {@const sw = Math.abs(dragState.curX - dragState.startX)}
                  {@const sh = Math.abs(dragState.curY - dragState.startY)}
                  <div
                    class="absolute pointer-events-none border-2 rounded-sm"
                    style="left:{sx}px;top:{sy}px;width:{sw}px;height:{sh}px;border-color:rgba(59,130,246,0.8);background:rgba(59,130,246,0.12);"
                  ></div>
                {/if}
              </div>
            </div>
            <div class="flex items-center justify-center gap-3 mt-3">
              <button
                class="px-2 py-1 rounded border border-border text-sm hover:bg-accent disabled:opacity-40"
                disabled={previewPage <= 1}
                onclick={() => setPreviewPage(previewPage - 1)}
              >←</button>
              <span class="text-sm text-muted-foreground">{previewPage} / {previewPageCount}</span>
              <button
                class="px-2 py-1 rounded border border-border text-sm hover:bg-accent disabled:opacity-40"
                disabled={previewPage >= previewPageCount}
                onclick={() => setPreviewPage(previewPage + 1)}
              >→</button>
            </div>
            <div
              bind:this={thumbStripContainer}
              class="flex gap-2 mt-3 overflow-auto p-1"
              style:filter={$isDark ? "invert(0.92) hue-rotate(180deg)" : "none"}
            >
              {#each Array(previewPageCount) as _, i}
                <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
                <div
                  class="shrink-0 rounded border-2 {i + 1 === previewPage ? 'border-blue-500' : 'border-transparent hover:border-border'}"
                  role="button"
                  tabindex="0"
                  onclick={() => setPreviewPage(i + 1)}
                  onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') setPreviewPage(i + 1); }}
                >
                  <canvas class="block"></canvas>
                  <span class="text-[10px] text-center block">{i + 1}</span>
                </div>
              {/each}
            </div>
          </div>
        <!-- Thumbnail grid for split/delete/extract/rotate -->
        {:else if showPreview}
          <div class="border-t border-border pt-4 mt-2">
            <div
              class="flex items-center justify-between mb-3"
            >
              <span class="text-sm font-medium text-muted-foreground">
                {t("tools.preview")} ({previewPageCount}
                {previewPageCount === 1 ? "page" : "pages"})
              </span>
              {#if thumbLoading}
                <Icon_Loader2 size={14} class="animate-spin text-muted-foreground" />
              {/if}
            </div>
            <div
              bind:this={thumbContainer}
              class="grid grid-cols-3 sm:grid-cols-4 md:grid-cols-5 lg:grid-cols-6 gap-3 max-h-[55vh] overflow-auto p-1"
              style:filter={$isDark
                ? "invert(0.92) hue-rotate(180deg)"
                : "none"}
            >
              {#each Array(previewPageCount) as _, i}
                <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
                <div
                  class={getThumbClasses(i + 1)}
                  onclick={() => {
                    if (activeTool === "delete") toggleDeletePage(i + 1);
                    if (activeTool === "extractPages") toggleExtractPage(i + 1);
                  }}
                >
                  {#if activeTool === "rotate"}
                    <div
                      class="flex items-center justify-center p-1"
                      style="min-height: 140px;"
                    >
                      <canvas
                        class="max-w-full transition-transform duration-200"
                        style="transform: rotate({rotateAngle}deg);"
                      ></canvas>
                    </div>
                  {:else}
                    <canvas class="block w-full"></canvas>
                  {/if}
                  <span
                    class="absolute top-1 left-1 text-[10px] leading-none bg-black/60 text-white px-1.5 py-0.5 rounded font-medium"
                  >
                    {i + 1}
                  </span>
                  {#if activeTool === "delete" && deletedPages.has(i + 1)}
                    <div
                      class="absolute inset-0 bg-red-500/25 flex items-center justify-center"
                    >
                      <div
                        class="bg-red-500 text-white rounded-full p-1"
                      >
                        <Icon_X size={16} />
                      </div>
                    </div>
                  {/if}
                  {#if activeTool === "extractPages" && selectedExtractPages.has(i + 1)}
                    <div
                      class="absolute inset-0 bg-green-500/25 flex items-center justify-center"
                    >
                      <div
                        class="bg-green-500 text-white rounded-full p-1"
                      >
                        <Icon_CheckCircle size={16} />
                      </div>
                    </div>
                  {/if}
                </div>
              {/each}
            </div>
          </div>
        {/if}

        <!-- Result -->
        {#if resultMsg}
          <div
            class="flex items-center gap-2 p-3 rounded-lg border {resultOk
              ? 'border-green-500/30 bg-green-500/5'
              : 'border-destructive/30 bg-destructive/5'}"
          >
            {#if resultOk}
              <Icon_CheckCircle
                size={16}
                class="text-green-600 shrink-0"
              />
            {:else}
              <Icon_AlertCircle
                size={16}
                class="text-destructive shrink-0"
              />
            {/if}
            <span
              class="text-sm {resultOk
                ? 'text-green-700'
                : 'text-destructive'}"
            >
              {resultMsg}
            </span>
          </div>
        {/if}
      </div>
    {/if}
  </div>
</div>
