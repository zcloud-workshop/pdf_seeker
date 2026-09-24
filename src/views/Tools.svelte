<script lang="ts">
  import { t } from "@/i18n/index.svelte.ts";
  import { currentView, currentFilePath, isDark } from "@/stores";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { invoke } from "@tauri-apps/api/core";
  import { readFile, writeTextFile, writeFile } from "@tauri-apps/plugin-fs";
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
    ListOrdered as Icon_ListOrdered,
    Undo2 as Icon_Undo2,
    Redo2 as Icon_Redo2,
    Crop as Icon_Crop,
    StickyNote as Icon_StickyNote,
    ClipboardList as Icon_ClipboardList,
    Replace as Icon_Replace,
    ShieldCheck as Icon_ShieldCheck,
  } from "lucide-svelte";
  import { loadPdf, renderPageToCanvas, type PdfDocumentProxy } from "@/pdf-engine";
  import { tick } from "svelte";

  type ToolId =
    | "merge" | "split" | "rotate" | "reorder" | "delete" | "extractPages"
    | "compress" | "watermark" | "img2pdf" | "pdf2img"
    | "pdf2text" | "sign" | "ocr" | "table"
    | "editText" | "editRect" | "editHighlight" | "crop" | "annotate" | "form"
    | "replaceText" | "security";

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
    return !!id && ["editText", "editRect", "editHighlight", "crop", "annotate", "sign", "watermark"].includes(id);
  }

  function setPreviewPage(p: number) {
    if (p < 1 || p > previewPageCount) return;
    switch (activeTool) {
      case "editText": editTextPage = p; break;
      case "editRect": editRectPage = p; break;
      case "editHighlight": editHlPage = p; break;
      case "crop": cropPage = p; break;
      case "annotate": annotPage = p; break;
      case "sign": signPage = p; break;
      case "table": tablePage = p; break;
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
    { id: "reorder", icon: Icon_ListOrdered, labelKey: "tools.reorder", ready: true, hasPreview: true },
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
    { id: "crop", icon: Icon_Crop, labelKey: "tools.crop", ready: true, hasPreview: true },
    { id: "annotate", icon: Icon_StickyNote, labelKey: "tools.annotate", ready: true, hasPreview: true },
    { id: "form", icon: Icon_ClipboardList, labelKey: "tools.form", ready: true, hasPreview: false },
    { id: "replaceText", icon: Icon_Replace, labelKey: "tools.replaceText", ready: true, hasPreview: false },
    { id: "security", icon: Icon_ShieldCheck, labelKey: "tools.security", ready: true, hasPreview: false },
    { id: "table", icon: Icon_Table, labelKey: "tools.extractTable", ready: true, hasPreview: true },
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
      pageOrder = Array.from({ length: doc.numPages }, (_, i) => i + 1);
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
          pageOrder[i] ?? i + 1,
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

    // --- Crop overlay: dim outside the crop area ---
    if (activeTool === "crop" && cropW > 0 && cropH > 0) {
      const cx = toX(cropX);
      const cy = toY(cropY + cropH);
      const cw = cropW * scale;
      const ch = cropH * scale;
      ctx.save();
      ctx.fillStyle = "rgba(0,0,0,0.45)";
      ctx.fillRect(0, 0, vp.width, cy);
      ctx.fillRect(0, cy + ch, vp.width, vp.height - cy - ch);
      ctx.fillRect(0, cy, cx, ch);
      ctx.fillRect(cx + cw, cy, vp.width - cx - cw, ch);
      ctx.strokeStyle = "#3b82f6";
      ctx.lineWidth = 2;
      ctx.setLineDash([6, 4]);
      ctx.strokeRect(cx, cy, cw, ch);
      ctx.restore();
    }

    // --- Annotation overlay ---
    if (activeTool === "annotate" && annotW > 0 && annotH > 0) {
      const cx = toX(annotX);
      const cy = toY(annotY + annotH);
      const cw = annotW * scale;
      const ch = annotH * scale;
      if (annotType === "highlight") {
        ctx.globalAlpha = annotOpacity;
        ctx.fillStyle = annotColor;
        ctx.fillRect(cx, cy, cw, ch);
        ctx.globalAlpha = 1.0;
      } else if (annotType === "underline") {
        ctx.strokeStyle = annotColor;
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.moveTo(cx, cy + ch - 1);
        ctx.lineTo(cx + cw, cy + ch - 1);
        ctx.stroke();
      } else {
        // Sticky note icon: filled square with folded corner
        ctx.fillStyle = annotColor;
        ctx.fillRect(cx, cy, cw, ch);
        ctx.strokeStyle = "rgba(0,0,0,0.4)";
        ctx.lineWidth = 1;
        ctx.strokeRect(cx, cy, cw, ch);
        ctx.beginPath();
        ctx.moveTo(cx + cw * 0.65, cy);
        ctx.lineTo(cx + cw, cy + ch * 0.35);
        ctx.lineTo(cx + cw * 0.65, cy + ch * 0.35);
        ctx.closePath();
        ctx.fillStyle = "rgba(0,0,0,0.15)";
        ctx.fill();
      }
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
    void cropPage; void cropX; void cropY; void cropW; void cropH;
    void annotPage; void annotType; void annotX; void annotY; void annotW; void annotH; void annotColor; void annotOpacity;
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
      case "crop":
        // Crop needs an actual drag; a plain click keeps the current rect
        if (!isClick) {
          cropX = pdfX;
          cropY = pdfBottom;
          cropW = pdfRight - pdfX;
          cropH = pdfTop - pdfBottom;
        }
        break;
      case "annotate":
        if (annotType === "note") {
          // Sticky note: fixed-size icon placed at the click point
          annotX = toPdfX(ds.startX);
          annotY = toPdfY(ds.startY) - 24;
          annotW = 24;
          annotH = 24;
        } else if (annotType === "underline") {
          annotX = pdfX;
          annotY = isClick ? pdfBottom - 12 : pdfBottom;
          annotW = isClick ? 200 : pdfRight - pdfX;
          annotH = isClick ? 12 : Math.max(6, pdfTop - pdfBottom);
        } else {
          annotX = pdfX;
          annotY = isClick ? pdfBottom - 20 : pdfBottom;
          annotW = isClick ? 200 : pdfRight - pdfX;
          annotH = isClick ? 20 : pdfTop - pdfBottom;
        }
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
      mergeFiles = [...new Set([...mergeFiles, ...paths])];
    }
  }

  async function addMergeFolder() {
    const selected = await open({ directory: true });
    if (!selected) return;
    try {
      const files = await invoke<string[]>("list_dir_files", {
        dir: String(selected),
        extensions: ["pdf"],
      });
      if (files.length === 0) {
        resultMsg = "No PDF files found in the selected folder";
        resultOk = false;
        return;
      }
      mergeFiles = [...new Set([...mergeFiles, ...files])];
      resultMsg = `Added ${files.length} PDF(s) from folder`;
      resultOk = true;
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
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

  // ==================== Reorder Pages ====================

  let pageOrder = $state<number[]>([]);
  let reorderDragFrom = $state<number | null>(null);
  let reorderDropOn = $state<number | null>(null);

  const orderChanged = $derived(pageOrder.some((p, i) => p !== i + 1));

  function onReorderDragStart(e: DragEvent, index: number) {
    // macOS WebKit won't start a drag without setData
    e.dataTransfer?.setData("text/plain", String(index));
    if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
    reorderDragFrom = index;
  }

  function onReorderDragOver(e: DragEvent, index: number) {
    if (reorderDragFrom === null) return;
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
    reorderDropOn = index;
  }

  function onReorderDragLeave() {
    reorderDropOn = null;
  }

  function onReorderDrop(e: DragEvent, index: number) {
    e.preventDefault();
    const from = reorderDragFrom;
    reorderDragFrom = null;
    reorderDropOn = null;
    if (from === null || from === index) return;
    const moved = pageOrder.splice(from, 1)[0];
    pageOrder.splice(index, 0, moved);
    renderThumbnails();
  }

  function onReorderDragEnd() {
    reorderDragFrom = null;
    reorderDropOn = null;
  }

  async function resetOrder() {
    pageOrder = Array.from({ length: previewPageCount }, (_, i) => i + 1);
    await renderThumbnails();
  }

  async function executeReorder() {
    const path = $currentFilePath;
    if (!path || !orderChanged) return;
    busy = true;
    resultMsg = "";
    try {
      const out = await save({
        filters: [{ name: "PDF", extensions: ["pdf"] }],
      });
      if (!out) return;
      const outPath = out as string;
      await invoke("reorder_pages", {
        req: { inputPath: path, outputPath: outPath, newOrder: pageOrder },
      });
      resultMsg = `Reordered pages → ${outPath.split(/[\\/]/).pop()}`;
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
    if (!$currentFilePath) return;
    await applyEditInPlace(
      "add_text_watermark",
      {
        text: watermarkText, fontSize: watermarkFontSize,
        opacity: watermarkOpacity, angle: watermarkAngle, color: watermarkColor,
      },
      "Watermark added (in place — Ctrl/Cmd+Z to undo)",
    );
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
      img2pdfFiles = [...new Set([...img2pdfFiles, ...paths])];
    }
  }

  async function addImg2PdfFolder() {
    const selected = await open({ directory: true });
    if (!selected) return;
    try {
      const files = await invoke<string[]>("list_dir_files", {
        dir: String(selected),
        extensions: ["jpg", "jpeg", "png", "bmp", "webp"],
      });
      if (files.length === 0) {
        resultMsg = "No image files found in the selected folder";
        resultOk = false;
        return;
      }
      img2pdfFiles = [...new Set([...img2pdfFiles, ...files])];
      resultMsg = `Added ${files.length} image(s) from folder`;
      resultOk = true;
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
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
    if (!$currentFilePath || !signImagePath) return;
    await applyEditInPlace(
      "sign_pdf",
      {
        signatureImagePath: signImagePath,
        page: signPage, x: signX, y: signY,
        width: signWidth, height: signHeight,
      },
      `Signature added to page ${signPage} (in place — Ctrl/Cmd+Z to undo)`,
    );
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

  // ==================== Table Extraction ====================

  let tablePage = $state(1);
  let tableCsv = $state("");

  function csvEscape(value: string): string {
    return /[",\n\r]/.test(value) ? `"${value.replace(/"/g, '""')}"` : value;
  }

  async function executeTable() {
    const path = $currentFilePath;
    if (!path) return;
    busy = true;
    resultMsg = "";
    tableCsv = "";
    try {
      const data = await readFile(path);
      const doc = await loadPdf(new Uint8Array(data));
      const page = await doc.getPage(Math.min(tablePage, doc.numPages));
      tablePage = Math.min(tablePage, doc.numPages);
      const content = await page.getTextContent();

      // Collect positioned text items (PDF coords, y-axis points up)
      const items = (content.items as Array<Record<string, unknown>>)
        .filter((it) => typeof it.str === "string" && String(it.str).trim())
        .map((it) => ({
          str: String(it.str),
          x: (it.transform as number[])[4],
          y: (it.transform as number[])[5],
          w: typeof it.width === "number" ? it.width : 0,
          h: typeof it.height === "number" && it.height > 0 ? it.height : 10,
        }));
      if (items.length === 0) {
        resultMsg = `No text found on page ${tablePage}`;
        resultOk = false;
        return;
      }

      const heights = items.map((i) => i.h).sort((a, b) => a - b);
      const medianH = heights[Math.floor(heights.length / 2)] || 10;

      // Group items into rows by y coordinate
      items.sort((a, b) => b.y - a.y);
      const rows: typeof items[] = [];
      let rowY: number | null = null;
      for (const it of items) {
        if (rowY === null || Math.abs(it.y - rowY) > medianH * 0.5) {
          rows.push([it]);
          rowY = it.y;
        } else {
          rows[rows.length - 1].push(it);
        }
      }

      // Split each row into cells where the horizontal gap is large
      const cellGap = Math.max(5, medianH * 0.9);
      const wordGap = medianH * 0.15;
      const table: string[][] = rows.map((row) => {
        row.sort((a, b) => a.x - b.x);
        const cells: string[] = [];
        let cell = "";
        let prevEnd: number | null = null;
        for (const it of row) {
          if (prevEnd !== null && it.x - prevEnd > cellGap) {
            cells.push(cell.trim());
            cell = "";
          } else if (prevEnd !== null && it.x - prevEnd > wordGap && cell && !cell.endsWith(" ")) {
            cell += " ";
          }
          cell += it.str;
          prevEnd = Math.max(prevEnd ?? 0, it.x + it.w);
        }
        cells.push(cell.trim());
        return cells;
      });

      const maxCols = Math.max(...table.map((r) => r.length));
      if (maxCols < 2 || table.length < 1) {
        resultMsg = `No table detected on page ${tablePage} (found ${maxCols} column(s))`;
        resultOk = false;
        return;
      }

      tableCsv = table.map((r) => r.map(csvEscape).join(",")).join("\n");
      resultMsg = `Detected ${table.length} rows × ${maxCols} columns on page ${tablePage}`;
      resultOk = true;
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
    } finally {
      busy = false;
    }
  }

  async function saveTableCsv() {
    if (!tableCsv) return;
    try {
      const out = await save({ filters: [{ name: "CSV", extensions: ["csv"] }] });
      if (!out) return;
      await writeTextFile(out as string, tableCsv);
      resultMsg = `Saved to ${(out as string).split(/[\\/]/).pop()}`;
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
    }
  }

  // ==================== Edit tools: Undo / Redo ====================
  // In-place editing with byte snapshots of the current file.

  const MAX_UNDO = 30;
  let undoStack = $state<Uint8Array[]>([]);
  let redoStack = $state<Uint8Array[]>([]);

  const canUndo = $derived(undoStack.length > 0);
  const canRedo = $derived(redoStack.length > 0);

  $effect(() => {
    // Reset history when the edited file changes
    if ($currentFilePath) {
      undoStack = [];
      redoStack = [];
    }
  });

  async function applyEditInPlace(command: string, req: Record<string, unknown>, successMsg: string) {
    const path = $currentFilePath;
    if (!path) return;
    busy = true;
    resultMsg = "";
    let pushed = false;
    try {
      const snapshot = await readFile(path);
      undoStack.push(snapshot);
      pushed = true;
      if (undoStack.length > MAX_UNDO) undoStack.shift();
      redoStack = [];
      await invoke(command, { req: { ...req, inputPath: path, outputPath: path } });
      resultMsg = successMsg;
      resultOk = true;
      await loadThumbnails();
    } catch (e) {
      // Restore the pre-edit snapshot if the command may have partially written the file
      if (pushed) {
        const snap = undoStack.pop()!;
        try { await writeFile(path, snap); } catch { /* keep the original error */ }
      }
      resultMsg = String(e);
      resultOk = false;
    } finally {
      busy = false;
    }
  }

  async function undoEdit() {
    const path = $currentFilePath;
    if (!path || undoStack.length === 0) return;
    try {
      redoStack.push(await readFile(path));
      const prev = undoStack.pop()!;
      await writeFile(path, prev);
      await loadThumbnails();
      resultMsg = "Undone";
      resultOk = true;
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
    }
  }

  async function redoEdit() {
    const path = $currentFilePath;
    if (!path || redoStack.length === 0) return;
    try {
      undoStack.push(await readFile(path));
      const next = redoStack.pop()!;
      await writeFile(path, next);
      await loadThumbnails();
      resultMsg = "Redone";
      resultOk = true;
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
    }
  }

  function onEditKeydown(e: KeyboardEvent) {
    if (!isEditingTool(activeTool)) return;
    const mod = e.metaKey || e.ctrlKey;
    if (!mod || e.key.toLowerCase() !== "z") return;
    e.preventDefault();
    if (e.shiftKey) redoEdit();
    else undoEdit();
  }

  // ==================== Edit: Add Text ====================

  let editText = $state("");
  let editFontSize = $state(12);
  let editTextColor = $state("#000000");
  let editTextPage = $state(1);
  let editTextX = $state(72);
  let editTextY = $state(720);

  async function executeEditText() {
    if (!$currentFilePath || !editText.trim()) return;
    await applyEditInPlace(
      "add_text_to_page",
      { text: editText, page: editTextPage, x: editTextX, y: editTextY, fontSize: editFontSize, color: editTextColor },
      `Text added to page ${editTextPage} (in place — Ctrl/Cmd+Z to undo)`,
    );
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
    if (!$currentFilePath) return;
    await applyEditInPlace(
      "add_rectangle",
      { page: editRectPage, x: editRectX, y: editRectY, width: editRectW, height: editRectH, borderColor: editRectBorder, fillColor: editRectHasFill ? editRectFill : null, borderWidth: editRectBorderW },
      `Rectangle added to page ${editRectPage} (in place — Ctrl/Cmd+Z to undo)`,
    );
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
    if (!$currentFilePath) return;
    await applyEditInPlace(
      "add_highlight",
      { page: editHlPage, x: editHlX, y: editHlY, width: editHlW, height: editHlH, color: editHlColor, opacity: editHlOpacity },
      `Highlight added to page ${editHlPage} (in place — Ctrl/Cmd+Z to undo)`,
    );
  }

  // ==================== Edit: Crop ====================

  let cropPage = $state(1);
  let cropX = $state(0);
  let cropY = $state(0);
  let cropW = $state(0);
  let cropH = $state(0);
  let cropAllPages = $state(false);

  async function executeCrop() {
    if (!$currentFilePath) return;
    if (cropW <= 0 || cropH <= 0) return;
    const pages = cropAllPages
      ? Array.from({ length: previewPageCount }, (_, i) => i + 1)
      : [cropPage];
    await applyEditInPlace(
      "crop_pages",
      { pages, x: cropX, y: cropY, width: cropW, height: cropH },
      `Cropped ${cropAllPages ? "all pages" : `page ${cropPage}`} (in place — Ctrl/Cmd+Z to undo)`,
    );
  }

  // ==================== Edit: Annotations ====================

  let annotType = $state<"highlight" | "underline" | "note">("highlight");
  let annotPage = $state(1);
  let annotX = $state(100);
  let annotY = $state(100);
  let annotW = $state(200);
  let annotH = $state(20);
  let annotColor = $state("#ffd54f");
  let annotOpacity = $state(0.4);
  let annotText = $state("");

  async function executeAnnotate() {
    if (!$currentFilePath) return;
    if (annotW <= 0 || annotH <= 0) return;
    await applyEditInPlace(
      "add_annotation",
      {
        page: annotPage,
        annotType,
        x: annotX,
        y: annotY,
        width: annotW,
        height: annotH,
        color: annotColor,
        opacity: annotOpacity,
        content: annotType === "note" ? annotText : "",
      },
      `${annotType === "note" ? "Sticky note" : annotType === "underline" ? "Underline" : "Highlight"} annotation added to page ${annotPage} (Ctrl/Cmd+Z to undo)`,
    );
  }

  // ==================== Form Filling ====================

  type FormFieldInfo = {
    name: string;
    fieldType: string;
    value: string;
    options: string[];
    pageIndex: number;
  };
  let formFields = $state<FormFieldInfo[]>([]);
  let formValues = $state<Record<string, string>>({});
  let formLoaded = $state(false);

  async function loadFormFields() {
    const path = $currentFilePath;
    if (!path) return;
    busy = true;
    resultMsg = "";
    try {
      const fields = await invoke<FormFieldInfo[]>("get_form_fields", { path });
      formFields = fields;
      const initial: Record<string, string> = {};
      for (const f of fields) {
        if (f.fieldType === "checkbox") {
          initial[f.name] = f.value === "Yes" || f.value === "true" ? "true" : "false";
        } else {
          initial[f.name] = f.value ?? "";
        }
      }
      formValues = initial;
      formLoaded = true;
      resultMsg = `Loaded ${fields.length} form field(s)`;
      resultOk = true;
    } catch (e) {
      formFields = [];
      formLoaded = true;
      resultMsg = String(e);
      resultOk = false;
    } finally {
      busy = false;
    }
  }

  async function executeFillForm() {
    const path = $currentFilePath;
    if (!path) return;
    busy = true;
    resultMsg = "";
    try {
      const out = await save({ filters: [{ name: "PDF", extensions: ["pdf"] }] });
      if (!out) return;
      const outPath = out as string;
      // Skip untouched text fields (empty) so existing values are preserved
      const values = formFields
        .map((f) => ({ name: f.name, value: formValues[f.name] ?? "" }))
        .filter((v) => v.value !== "");
      await invoke("fill_form", {
        req: { inputPath: path, outputPath: outPath, values },
      });
      resultMsg = `Form filled → ${outPath.split(/[\\/]/).pop()}`;
      resultOk = true;
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
    } finally {
      busy = false;
    }
  }

  // ==================== Replace Text (overlay style) ====================

  type TextMatch = {
    page: number;
    x: number;
    y: number;
    width: number;
    height: number;
    baselineY: number;
    fontSize: number;
    text: string;
    selected: boolean;
  };
  let replaceQuery = $state("");
  let replaceWith = $state("");
  let replaceMatches = $state<TextMatch[]>([]);
  let replaceSearching = $state(false);

  async function findReplaceMatches() {
    const path = $currentFilePath;
    const query = replaceQuery.trim();
    if (!path || !query) return;
    replaceSearching = true;
    resultMsg = "";
    try {
      const data = await readFile(path);
      const doc = await loadPdf(new Uint8Array(data));
      const q = query.toLowerCase();
      const matches: TextMatch[] = [];

      for (let i = 1; i <= doc.numPages; i++) {
        const page = await doc.getPage(i);
        const content = await page.getTextContent();

        // Build page text + per-char positions (PDF coords, y-axis up)
        let pageText = "";
        const chars: Array<{ x: number; xEnd: number; baselineY: number; fontSize: number }> = [];
        for (const item of content.items as Array<Record<string, unknown>>) {
          const str = typeof item.str === "string" ? (item.str as string) : "";
          if (!str) continue;
          const tr = item.transform as number[];
          const fontSize = Math.hypot(tr[1], tr[3]) || Math.abs(tr[3]) || 10;
          if (Math.abs(tr[1]) > 0.001) {
            // Rotated text: placeholder chars keep indices aligned with pageText
            for (const _ of str) {
              pageText += "\u0000";
              chars.push({ x: NaN, xEnd: NaN, baselineY: NaN, fontSize });
            }
            continue;
          }
          const width = typeof item.width === "number" ? (item.width as number) : str.length * fontSize * 0.5;
          const per = str.length ? width / str.length : 0;
          for (let k = 0; k < str.length; k++) {
            chars.push({ x: tr[4] + k * per, xEnd: tr[4] + (k + 1) * per, baselineY: tr[5], fontSize });
            pageText += str[k];
          }
        }

        const lower = pageText.toLowerCase();
        let pos = 0;
        while ((pos = lower.indexOf(q, pos)) !== -1) {
          const seg = chars.slice(pos, pos + query.length);
          if (seg.length === query.length && seg.every((c) => Number.isFinite(c.x))) {
            const minX = Math.min(...seg.map((c) => c.x));
            const maxX = Math.max(...seg.map((c) => c.xEnd));
            const first = seg[0];
            const bottom = first.baselineY - first.fontSize * 0.25;
            const top = first.baselineY + first.fontSize * 1.0;
            matches.push({
              page: i,
              x: minX - 1,
              y: bottom,
              width: maxX - minX + 2,
              height: top - bottom,
              baselineY: first.baselineY,
              fontSize: first.fontSize,
              text: pageText.slice(pos, pos + query.length),
              selected: true,
            });
          }
          pos += query.length;
        }
      }

      replaceMatches = matches;
      resultMsg = matches.length ? `Found ${matches.length} match(es)` : "No matches found";
      resultOk = matches.length > 0;
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
    } finally {
      replaceSearching = false;
    }
  }

  async function executeReplaceText() {
    const selected = replaceMatches.filter((m) => m.selected);
    if (!selected.length) return;
    await applyEditInPlace(
      "replace_text",
      {
        replacements: selected.map((m) => ({
          page: m.page,
          coverX: m.x,
          coverY: m.y,
          coverWidth: m.width,
          coverHeight: m.height,
          baselineY: m.baselineY,
          fontSize: m.fontSize,
          newText: replaceWith,
        })),
      },
      `Replaced ${selected.length} occurrence(s) (Ctrl/Cmd+Z to undo)`,
    );
  }

  // ==================== Security: Encrypt / Decrypt ====================

  let securityMode = $state<"encrypt" | "decrypt">("encrypt");
  let secOwnerPassword = $state("");
  let secUserPassword = $state("");
  let secAllowPrinting = $state(true);
  let secAllowModifying = $state(true);
  let secAllowCopying = $state(true);
  let secAllowAnnotating = $state(true);
  let decryptPassword = $state("");

  async function executeEncryptPdf() {
    const path = $currentFilePath;
    if (!path || !secOwnerPassword) return;
    busy = true;
    resultMsg = "";
    try {
      const out = await save({ filters: [{ name: "PDF", extensions: ["pdf"] }] });
      if (!out) return;
      const outPath = out as string;
      await invoke("encrypt_pdf", {
        req: {
          inputPath: path,
          outputPath: outPath,
          ownerPassword: secOwnerPassword,
          userPassword: secUserPassword || null,
          allowPrinting: secAllowPrinting,
          allowModifying: secAllowModifying,
          allowCopying: secAllowCopying,
          allowAnnotating: secAllowAnnotating,
        },
      });
      resultMsg = `Encrypted → ${outPath.split(/[\\/]/).pop()}`;
      resultOk = true;
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
    } finally {
      busy = false;
    }
  }

  async function executeDecryptPdf() {
    const path = $currentFilePath;
    if (!path) return;
    busy = true;
    resultMsg = "";
    try {
      const out = await save({ filters: [{ name: "PDF", extensions: ["pdf"] }] });
      if (!out) return;
      const outPath = out as string;
      await invoke("decrypt_pdf", {
        req: { inputPath: path, outputPath: outPath, password: decryptPassword || null },
      });
      resultMsg = `Decrypted → ${outPath.split(/[\\/]/).pop()}`;
      resultOk = true;
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
    } finally {
      busy = false;
    }
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
    if (activeTool === "reorder") {
      if (
        reorderDragFrom !== null &&
        reorderDropOn === pageNum - 1 &&
        reorderDragFrom !== reorderDropOn
      )
        return `${base} border-blue-500`;
      return `${base} border-transparent hover:border-border`;
    }
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
    if (activeTool === "crop" && pageNum === cropPage) return `${base} border-blue-500`;
    if (activeTool === "annotate" && pageNum === annotPage) return `${base} border-blue-500`;
    if (activeTool === "sign" && pageNum === signPage) return `${base} border-blue-500`;
    if (activeTool === "table" && pageNum === tablePage) return `${base} border-blue-500`;
    return `${base} border-transparent`;
  }

  const previewPage = $derived(
    activeTool === "editText" ? editTextPage :
    activeTool === "editRect" ? editRectPage :
    activeTool === "editHighlight" ? editHlPage :
    activeTool === "crop" ? cropPage :
    activeTool === "annotate" ? annotPage :
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

<svelte:window onkeydown={onEditKeydown} />

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
            <div class="flex gap-2">
              <Button
                variant="outline"
                size="sm"
                onclick={addMergeFiles}
                class="gap-1.5"
              >
                <Icon_FileUp size={14} />
                Add Files
              </Button>
              <Button
                variant="outline"
                size="sm"
                onclick={addMergeFolder}
                class="gap-1.5"
              >
                <Icon_FolderOpen size={14} />
                Add Folder
              </Button>
            </div>
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

        <!-- Reorder Pages -->
        {#if activeTool === "reorder"}
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
                Drag thumbnails below to rearrange pages of:
                <strong
                  >{$currentFilePath.split(/[\\/]/).pop()}</strong
                >
              </p>
              <div class="flex gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  onclick={resetOrder}
                  disabled={busy || !orderChanged}
                >
                  Reset
                </Button>
                <Button
                  onclick={executeReorder}
                  disabled={busy || !orderChanged}
                >
                  {#if busy}
                    <Icon_Loader2 size={14} class="animate-spin" />
                  {:else}
                    <Icon_Save size={14} class="mr-1.5" />
                    Save Order
                  {/if}
                </Button>
              </div>
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
                  Add Watermark
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
            <div class="flex gap-2">
              <Button variant="outline" size="sm" onclick={addImg2PdfFiles} class="gap-1.5">
                <Icon_FileUp size={14} />
                Add Images
              </Button>
              <Button variant="outline" size="sm" onclick={addImg2PdfFolder} class="gap-1.5">
                <Icon_FolderOpen size={14} />
                Add Folder
              </Button>
            </div>
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
                  Sign
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

        <!-- Table Extraction -->
        {#if activeTool === "table"}
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
                Detect and extract a table as CSV from:
                <strong>{$currentFilePath.split(/[\\/]/).pop()}</strong>
              </p>
              <div class="flex items-end gap-2">
                <div class="space-y-1 w-28">
                  <Label>Page</Label>
                  <Input
                    type="number"
                    min="1"
                    max={previewPageCount || 1}
                    value={tablePage}
                    onchange={(e) => (tablePage = parseInt((e.target as HTMLInputElement).value) || 1)}
                  />
                </div>
                <Button onclick={executeTable} disabled={busy}>
                  {#if busy}
                    <Icon_Loader2 size={14} class="animate-spin" />
                  {:else}
                    <Icon_Table size={14} class="mr-1.5" />
                    Extract Table
                  {/if}
                </Button>
              </div>
              <p class="text-xs text-muted-foreground">
                Click a thumbnail below to pick the page. Works best on text-based PDFs.
              </p>
              {#if tableCsv}
                <textarea
                  readonly
                  rows="8"
                  class="w-full p-2 rounded-lg border border-input bg-transparent text-sm font-mono resize-y"
                  value={tableCsv}
                ></textarea>
                <Button variant="outline" size="sm" onclick={saveTableCsv}>
                  <Icon_Save size={14} class="mr-1.5" />
                  Save as CSV
                </Button>
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
                {#if busy}<Icon_Loader2 size={14} class="animate-spin" />{:else}<Icon_Save size={14} class="mr-1.5" />Add Text}{/if}
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
                {#if busy}<Icon_Loader2 size={14} class="animate-spin" />{:else}<Icon_Save size={14} class="mr-1.5" />Add Rectangle}{/if}
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
                {#if busy}<Icon_Loader2 size={14} class="animate-spin" />{:else}<Icon_Save size={14} class="mr-1.5" />Add Highlight}{/if}
              </Button>
            {/if}
          </div>
        {/if}

        {#if activeTool === "crop"}
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
                Drag on the preview to select the area to keep:
                <strong>{$currentFilePath.split(/[\\/]/).pop()}</strong>
              </p>
              <div class="grid grid-cols-2 gap-3">
                <div class="space-y-1"><Label>Page</Label><Input type="number" min="1" value={cropPage} onchange={(e) => (cropPage = parseInt((e.target as HTMLInputElement).value) || 1)} /></div>
                <div class="space-y-1 flex items-end">
                  <label class="flex items-center gap-2 text-sm cursor-pointer">
                    <input type="checkbox" bind:checked={cropAllPages} class="accent-blue-500" />
                    Apply to all pages
                  </label>
                </div>
                <div class="space-y-1"><Label>X</Label><Input type="number" value={cropX} onchange={(e) => (cropX = parseFloat((e.target as HTMLInputElement).value) || 0)} /></div>
                <div class="space-y-1"><Label>Y</Label><Input type="number" value={cropY} onchange={(e) => (cropY = parseFloat((e.target as HTMLInputElement).value) || 0)} /></div>
                <div class="space-y-1"><Label>Width</Label><Input type="number" value={cropW} onchange={(e) => (cropW = parseFloat((e.target as HTMLInputElement).value) || 0)} /></div>
                <div class="space-y-1"><Label>Height</Label><Input type="number" value={cropH} onchange={(e) => (cropH = parseFloat((e.target as HTMLInputElement).value) || 0)} /></div>
              </div>
              <Button onclick={executeCrop} disabled={busy || cropW <= 0 || cropH <= 0}>
                {#if busy}<Icon_Loader2 size={14} class="animate-spin" />{:else}<Icon_Crop size={14} class="mr-1.5" />{cropAllPages ? "Crop All Pages" : "Crop Page"}{/if}
              </Button>
            {/if}
          </div>
        {/if}

        {#if activeTool === "annotate"}
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
                Add annotation to <strong>{$currentFilePath.split(/[\\/]/).pop()}</strong> — real PDF annotations, visible in any reader.
              </p>
              <div class="flex gap-2">
                {#each [["highlight", "Highlight"], ["underline", "Underline"], ["note", "Sticky Note"]] as [tp, label]}
                  <button
                    class="px-3 py-1.5 rounded-md border text-sm transition-colors {annotType === tp ? 'bg-primary text-primary-foreground border-primary' : 'border-border hover:bg-accent'}"
                    onclick={() => {
                      annotType = tp as typeof annotType;
                      if (tp === "note") { annotW = 24; annotH = 24; }
                    }}
                  >{label}</button>
                {/each}
              </div>
              <div class="grid grid-cols-2 gap-3">
                <div class="space-y-1"><Label>Page</Label><Input type="number" min="1" value={annotPage} onchange={(e) => (annotPage = parseInt((e.target as HTMLInputElement).value) || 1)} /></div>
                {#if annotType === "highlight"}
                  <div class="space-y-1"><Label>Opacity (0-1)</Label><Input type="number" step="0.05" min="0" max="1" value={annotOpacity} onchange={(e) => (annotOpacity = parseFloat((e.target as HTMLInputElement).value) || 0.4)} /></div>
                {/if}
                <div class="space-y-1"><Label>X</Label><Input type="number" value={annotX} onchange={(e) => (annotX = parseFloat((e.target as HTMLInputElement).value) || 0)} /></div>
                <div class="space-y-1"><Label>Y</Label><Input type="number" value={annotY} onchange={(e) => (annotY = parseFloat((e.target as HTMLInputElement).value) || 0)} /></div>
                <div class="space-y-1"><Label>Width</Label><Input type="number" value={annotW} onchange={(e) => (annotW = parseFloat((e.target as HTMLInputElement).value) || 0)} /></div>
                <div class="space-y-1"><Label>Height</Label><Input type="number" value={annotH} onchange={(e) => (annotH = parseFloat((e.target as HTMLInputElement).value) || 0)} /></div>
              </div>
              <div class="space-y-1">
                <Label>Color</Label>
                <div class="flex items-center gap-2">
                  <input type="color" bind:value={annotColor} class="w-8 h-8 rounded cursor-pointer" />
                  <Input value={annotColor} onchange={(e) => (annotColor = (e.target as HTMLInputElement).value)} class="w-24" />
                </div>
              </div>
              {#if annotType === "note"}
                <div class="space-y-1">
                  <Label>Note text</Label>
                  <textarea
                    bind:value={annotText}
                    rows="3"
                    class="flex w-full rounded-md border border-input bg-background px-3 py-2 text-sm shadow-sm placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
                    placeholder="Text shown when the note is opened"
                  ></textarea>
                </div>
              {/if}
              <Button onclick={executeAnnotate} disabled={busy || annotW <= 0 || annotH <= 0}>
                {#if busy}<Icon_Loader2 size={14} class="animate-spin" />{:else}<Icon_StickyNote size={14} class="mr-1.5" />Add Annotation}{/if}
              </Button>
            {/if}
          </div>
        {/if}

        {#if activeTool === "form"}
          <div class="space-y-3">
            {#if !$currentFilePath}
              <p class="text-sm text-muted-foreground">Open a PDF first.</p>
              <div class="flex gap-2">
                <Button variant="outline" size="sm" onclick={openFileForTool}>
                  <Icon_FileUp size={14} class="mr-1.5" />
                  Open PDF
                </Button>
              </div>
            {:else if !formLoaded}
              <p class="text-sm text-muted-foreground">
                Read AcroForm fields from <strong>{$currentFilePath.split(/[\\/]/).pop()}</strong>
              </p>
              <Button onclick={loadFormFields} disabled={busy}>
                {#if busy}<Icon_Loader2 size={14} class="animate-spin" />{:else}<Icon_ClipboardList size={14} class="mr-1.5" />Load Form Fields}{/if}
              </Button>
            {:else if formFields.length === 0}
              <p class="text-sm text-muted-foreground">No fillable form fields found in this PDF.</p>
              <Button variant="outline" size="sm" onclick={loadFormFields} disabled={busy}>Reload</Button>
            {:else}
              <p class="text-sm text-muted-foreground">
                {formFields.length} field(s) — fill and save as a new file:
              </p>
              <div class="space-y-3 max-h-96 overflow-auto pr-1">
                {#each formFields as f (f.name)}
                  {#if f.fieldType === "text"}
                    <div class="space-y-1">
                      <Label>{f.name}</Label>
                      <Input bind:value={formValues[f.name]} placeholder={f.name} />
                    </div>
                  {:else if f.fieldType === "checkbox"}
                    <label class="flex items-center gap-2 text-sm cursor-pointer">
                      <input
                        type="checkbox"
                        class="accent-blue-500"
                        checked={formValues[f.name] === "true"}
                        onchange={(e) => (formValues[f.name] = (e.target as HTMLInputElement).checked ? "true" : "false")}
                      />
                      {f.name}
                    </label>
                  {:else if f.fieldType === "radio" || f.fieldType === "choice"}
                    <div class="space-y-1">
                      <Label>{f.name}</Label>
                      <select
                        bind:value={formValues[f.name]}
                        class="flex w-full rounded-md border border-input bg-background px-3 py-1.5 text-sm shadow-sm focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
                      >
                        {#each f.options as opt (opt)}
                          <option value={opt}>{opt}</option>
                        {/each}
                      </select>
                    </div>
                  {:else}
                    <p class="text-xs text-muted-foreground">{f.name} ({f.fieldType}) — not fillable</p>
                  {/if}
                {/each}
              </div>
              <div class="flex gap-2">
                <Button onclick={executeFillForm} disabled={busy}>
                  {#if busy}<Icon_Loader2 size={14} class="animate-spin" />{:else}<Icon_Save size={14} class="mr-1.5" />Fill &amp; Save As}{/if}
                </Button>
                <Button variant="outline" size="sm" onclick={loadFormFields} disabled={busy}>Reload</Button>
              </div>
            {/if}
          </div>
        {/if}

        {#if activeTool === "replaceText"}
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
                Overlay replacement in <strong>{$currentFilePath.split(/[\\/]/).pop()}</strong> — original text is covered and new text drawn at the same position.
              </p>
              <div class="grid grid-cols-2 gap-3">
                <div class="space-y-1"><Label>Find</Label><Input bind:value={replaceQuery} placeholder="text to find" /></div>
                <div class="space-y-1"><Label>Replace with</Label><Input bind:value={replaceWith} placeholder="replacement text" /></div>
              </div>
              <Button onclick={findReplaceMatches} disabled={busy || replaceSearching || !replaceQuery.trim()}>
                {#if replaceSearching}<Icon_Loader2 size={14} class="animate-spin" />{:else}<Icon_Replace size={14} class="mr-1.5" />Find Matches}{/if}
              </Button>
              {#if replaceMatches.length > 0}
                <div class="flex items-center justify-between">
                  <span class="text-sm text-muted-foreground">{replaceMatches.filter((m) => m.selected).length} / {replaceMatches.length} selected</span>
                  <div class="flex gap-2">
                    <button class="text-xs underline text-muted-foreground hover:text-foreground"
                      onclick={() => (replaceMatches = replaceMatches.map((m) => ({ ...m, selected: true })))}
                    >Select all</button>
                    <button class="text-xs underline text-muted-foreground hover:text-foreground"
                      onclick={() => (replaceMatches = replaceMatches.map((m) => ({ ...m, selected: false })))}
                    >Clear</button>
                  </div>
                </div>
                <div class="space-y-1.5 max-h-72 overflow-auto pr-1">
                  {#each replaceMatches as m, idx (idx)}
                    <label class="flex items-center gap-2 text-sm cursor-pointer p-1.5 rounded hover:bg-accent">
                      <input
                        type="checkbox"
                        class="accent-blue-500"
                        checked={m.selected}
                        onchange={(e) => (replaceMatches = replaceMatches.map((mm, i) => i === idx ? { ...mm, selected: (e.target as HTMLInputElement).checked } : mm))}
                      />
                      <span class="text-xs text-muted-foreground shrink-0">p.{m.page}</span>
                      <span class="truncate font-mono">“{m.text}”</span>
                    </label>
                  {/each}
                </div>
                <Button onclick={executeReplaceText} disabled={busy || replaceMatches.every((m) => !m.selected)}>
                  {#if busy}<Icon_Loader2 size={14} class="animate-spin" />{:else}<Icon_Replace size={14} class="mr-1.5" />Replace Selected}{/if}
                </Button>
              {/if}
            {/if}
          </div>
        {/if}

        {#if activeTool === "security"}
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
                <strong>{$currentFilePath.split(/[\\/]/).pop()}</strong>
              </p>
              <div class="flex gap-2">
                {#each [["encrypt", "Encrypt"], ["decrypt", "Decrypt"]] as [mode, label]}
                  <button
                    class="px-3 py-1.5 rounded-md border text-sm transition-colors {securityMode === mode ? 'bg-primary text-primary-foreground border-primary' : 'border-border hover:bg-accent'}"
                    onclick={() => (securityMode = mode as typeof securityMode)}
                  >{label}</button>
                {/each}
              </div>

              {#if securityMode === "encrypt"}
                <div class="space-y-1">
                  <Label>Owner password (required)</Label>
                  <Input type="password" bind:value={secOwnerPassword} autocomplete="new-password" />
                </div>
                <div class="space-y-1">
                  <Label>User password (optional — needed to open the file)</Label>
                  <Input type="password" bind:value={secUserPassword} autocomplete="new-password" />
                </div>
                <div class="space-y-1.5">
                  <Label>Permissions</Label>
                  <label class="flex items-center gap-2 text-sm cursor-pointer"><input type="checkbox" bind:checked={secAllowPrinting} class="accent-blue-500" />Allow printing</label>
                  <label class="flex items-center gap-2 text-sm cursor-pointer"><input type="checkbox" bind:checked={secAllowModifying} class="accent-blue-500" />Allow modifying</label>
                  <label class="flex items-center gap-2 text-sm cursor-pointer"><input type="checkbox" bind:checked={secAllowCopying} class="accent-blue-500" />Allow copying text</label>
                  <label class="flex items-center gap-2 text-sm cursor-pointer"><input type="checkbox" bind:checked={secAllowAnnotating} class="accent-blue-500" />Allow annotating</label>
                </div>
                <p class="text-xs text-muted-foreground">RC4 128-bit standard security handler (PDF V2/R3)</p>
                <Button onclick={executeEncryptPdf} disabled={busy || !secOwnerPassword}>
                  {#if busy}<Icon_Loader2 size={14} class="animate-spin" />{:else}<Icon_ShieldCheck size={14} class="mr-1.5" />Encrypt &amp; Save As}{/if}
                </Button>
              {:else}
                <div class="space-y-1">
                  <Label>Password (user or owner; leave empty for password-less user access)</Label>
                  <Input type="password" bind:value={decryptPassword} autocomplete="off" />
                </div>
                <p class="text-xs text-muted-foreground">Only RC4-encrypted PDFs (V2/R3) are supported</p>
                <Button onclick={executeDecryptPdf} disabled={busy}>
                  {#if busy}<Icon_Loader2 size={14} class="animate-spin" />{:else}<Icon_ShieldCheck size={14} class="mr-1.5" />Decrypt &amp; Save As}{/if}
                </Button>
              {/if}
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
              <div class="flex items-center gap-2">
                <button
                  class="flex items-center gap-1 px-2 py-1 rounded border border-border text-xs hover:bg-accent disabled:opacity-40"
                  disabled={!canUndo || busy}
                  onclick={undoEdit}
                  title="Undo (Ctrl/Cmd+Z)"
                >
                  <Icon_Undo2 size={13} />
                  Undo
                </button>
                <button
                  class="flex items-center gap-1 px-2 py-1 rounded border border-border text-xs hover:bg-accent disabled:opacity-40"
                  disabled={!canRedo || busy}
                  onclick={redoEdit}
                  title="Redo (Ctrl/Cmd+Shift+Z)"
                >
                  <Icon_Redo2 size={13} />
                  Redo
                </button>
                {#if thumbLoading}
                  <Icon_Loader2 size={14} class="animate-spin text-muted-foreground" />
                {/if}
              </div>
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
                  draggable={activeTool === "reorder"}
                  ondragstart={(e) => { if (activeTool === "reorder") onReorderDragStart(e, i); }}
                  ondragover={(e) => { if (activeTool === "reorder") onReorderDragOver(e, i); }}
                  ondragleave={() => { if (activeTool === "reorder") onReorderDragLeave(); }}
                  ondrop={(e) => { if (activeTool === "reorder") onReorderDrop(e, i); }}
                  ondragend={() => { if (activeTool === "reorder") onReorderDragEnd(); }}
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
                    {pageOrder[i] ?? i + 1}
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
