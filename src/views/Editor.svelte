<script lang="ts">
  import { t } from "@/i18n/index.svelte.ts";
  import {
    currentFilePath,
    currentFileName,
    currentView,
    currentPage,
    totalPages,
    zoomLevel,
    isDark,
    editHistory,
    selectedEditId,
    signPlacement,
    ocrResults,
    sidebarCollapsed,
    activeToolTab,
  } from "@/stores";
  import { privacyAuditor } from "@/models/privacy";
  import { Button, Tooltip, Input, Label } from "@/components/ui";
  import { FileText, Loader2, Save, X, SaveAll, Sun, Moon, FolderOpen } from "lucide-svelte";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { readFile, writeFile } from "@tauri-apps/plugin-fs";
  import { invoke } from "@tauri-apps/api/core";
  import { loadPdf, getPageViewport, type PdfDocumentProxy } from "@/pdf-engine";
  import ToolbarTabs from "@/components/editor/ToolbarTabs.svelte";
  import type { ToolCategory, ToolId } from "@/components/editor/ToolbarTabs.svelte";
  import StatusBar from "@/components/editor/StatusBar.svelte";
  import CanvasEditor from "@/components/editor/CanvasEditor.svelte";
  import type { EditOperation } from "@/edit-history";
  import { pushOperation, getAppliedOperations } from "@/edit-history";
  import WatermarkDialog from "@/components/editor/WatermarkDialog.svelte";
  import PageToolsDialog from "@/components/editor/PageToolsDialog.svelte";
  import TextLayer from "@/components/editor/TextLayer.svelte";
  import OcrOverlay from "@/components/editor/OcrOverlay.svelte";
  import ThumbnailSidebar from "@/components/editor/ThumbnailSidebar.svelte";
  import { PanelLeftClose, PanelLeft, Printer, Eye, EyeOff, ScanLine, Layers, Trash, Sparkles, Download, Copy, Check, Table as IconTable, Globe, BookOpen, Info, ChevronLeft, ChevronRight, ZoomIn, ZoomOut } from "lucide-svelte";
  import { printPdf } from "@/print";
  import TabBar from "@/components/editor/TabBar.svelte";
  import { tabsStore } from "@/stores/tabs";
  import ToolboxModal from "@/components/editor/ToolboxModal.svelte";
  import PdfSearchBar from "@/components/editor/PdfSearchBar.svelte";
  import ZenReadingOverlay from "@/components/editor/ZenReadingOverlay.svelte";
  import PdfInfoDialog from "@/components/editor/PdfInfoDialog.svelte";
  import CropTool from "@/components/tools/CropTool.svelte";
  import MetadataTool from "@/components/tools/MetadataTool.svelte";
  import { parseEpub, parseComicBook, formatTextToHtmlPages } from "@/utils/epub-parser";
  import { isEbookFormat, convertEpubToPdf, convertCbzToPdf, convertTextToPdf } from "@/utils/ebook-to-pdf";
  import { cleanOcrText } from "@/utils/text-cleaner";
  import { computeVisiblePages, clampCanvasDimensions, type VirtualWindow } from "@/utils/virtual-viewport";
  import { computeZoomLevel, computeScrollAnchor } from "@/utils/zoom-math";

  export const SUPPORTED_OPEN_FILTERS = [
    {
      name: "所有支持的文档与电子书 (*.pdf, *.epub, *.cbz, *.txt, *.md)",
      extensions: ["pdf", "epub", "cbz", "txt", "md", "markdown"],
    },
    { name: "PDF 文档 (*.pdf)", extensions: ["pdf"] },
    { name: "EPUB 电子书 (*.epub)", extensions: ["epub"] },
    { name: "漫画归档 (*.cbz)", extensions: ["cbz"] },
    { name: "纯文本与 Markdown (*.txt, *.md)", extensions: ["txt", "md", "markdown"] },
  ];

  // ─── Tool state ───────────────────────────────────────────────────────

  let activeCategory = $state<ToolCategory>("page");
  let activeTool = $state<ToolId | null>(null);
  let searchOpen = $state(false);
  let pageDialogTool = $state<ToolId | null>(null);
  let watermarkDialogOpen = $state(false);
  let toolboxVisible = $state(false);
  let thumbnailVisible = $state(true);
  let printDialogOpen = $state(false);
  let printBusy = $state(false);
  let ocrOverlayVisible = $state(true);
  let zenModeOpen = $state(false);
  let pdfInfoOpen = $state(false);
  let cropDialogOpen = $state(false);
  let metadataDialogOpen = $state(false);
  let showZenReminder = $state(false);
  let zenReminderDismissed = $state(
    typeof localStorage !== "undefined" && localStorage.getItem("pdf_seeker_zen_reminder_dismissed") === "true"
  );

  function dismissZenReminder(forever = false) {
    showZenReminder = false;
    if (forever) {
      zenReminderDismissed = true;
      try {
        localStorage.setItem("pdf_seeker_zen_reminder_dismissed", "true");
      } catch (e) {
        // ignore
      }
    }
  }

  function launchZenFromReminder() {
    showZenReminder = false;
    zenModeOpen = true;
  }

  // Print state
  let printPageRange = $state("all");
  let printCustomRange = $state("");
  let printOrientation = $state<"portrait" | "landscape" | "auto">("auto");
  let printScale = $state(1.0);

  async function handlePrint() {
    if (!pdfDoc || printBusy) return;
    printBusy = true;
    try {
      let pages: number[] = [];
      if (printPageRange === "current") {
        pages = [$currentPage];
      } else if (printPageRange === "custom") {
        pages = parsePrintRange(printCustomRange);
      }
      // "all" = empty array = all pages

      await printPdf(pdfDoc, {
        pages,
        orientation: printOrientation,
        scaleFactor: printScale,
      });
      printDialogOpen = false;
    } catch (e) {
      console.error("Print failed:", e);
    } finally {
      printBusy = false;
    }
  }

  function parsePrintRange(range: string): number[] {
    const pages = new Set<number>();
    for (const part of range.split(",")) {
      const trimmed = part.trim();
      if (trimmed.includes("-")) {
        const [a, b] = trimmed.split("-").map(Number);
        if (!isNaN(a) && !isNaN(b)) {
          for (let i = Math.min(a, b); i <= Math.max(a, b); i++) pages.add(i);
        }
      } else {
        const n = Number(trimmed);
        if (!isNaN(n)) pages.add(n);
      }
    }
    return Array.from(pages).sort((a, b) => a - b);
  }

  function handleThumbnailReorder(fromPage: number, toPage: number) {
    // Open reorder dialog with pre-filled values
    pageDialogTool = "reorder";
  }

  // ─── Selection state (from store) ────────────────────────────────────

  const selectedElement = $derived(() => {
    const id = $selectedEditId;
    if (!id) return null;
    const hist = $editHistory;
    return hist.operations.find((o: EditOperation) => o.id === id) ?? null;
  });

  let layersPanelOpen = $state(false);

  const currentAppliedOperations = $derived(() => {
    return getAppliedOperations($editHistory);
  });

  const currentPageOperations = $derived(() => {
    const cur = $currentPage;
    return currentAppliedOperations().filter((o: EditOperation) => o.params.page === cur);
  });

  const showPropertyPanel = $derived(selectedElement() !== null);

  function clearSelection() {
    selectedEditId.set(null);
  }

  function updateSelectedProperty(key: string, value: any) {
    const id = $selectedEditId;
    if (!id) return;
    const hist = $editHistory;
    const idx = hist.operations.findIndex((o: EditOperation) => o.id === id);
    if (idx < 0) return;
    hist.operations[idx] = {
      ...hist.operations[idx],
      params: { ...hist.operations[idx].params, [key]: value },
    };
    editHistory.set({ ...hist });
  }

  function deleteSelected() {
    const id = $selectedEditId;
    if (!id) return;
    deleteOperationById(id);
  }

  function deleteOperationById(id: string) {
    const hist = $editHistory;
    const newOps = hist.operations.filter((o: EditOperation) => o.id !== id);
    editHistory.set({
      operations: newOps,
      currentIndex: Math.min(hist.currentIndex, newOps.length - 1),
    });
    if ($selectedEditId === id) {
      selectedEditId.set(null);
    }
  }

  function clearPageOperations(pageNum: number) {
    const hist = $editHistory;
    const newOps = hist.operations.filter((o: EditOperation) => o.params.page !== pageNum);
    editHistory.set({
      operations: newOps,
      currentIndex: Math.min(hist.currentIndex, newOps.length - 1),
    });
    selectedEditId.set(null);
  }

  function clearAllOperations() {
    const hist = $editHistory;
    editHistory.set({ operations: [], currentIndex: -1 });
    selectedEditId.set(null);
  }

  // ─── Tool action handler ──────────────────────────────────────────────

  function handleToolAction(toolId: ToolId) {
    if (toolId === "batchWorkspace") {
      activeToolTab.set("batch");
      currentView.set("tools");
      return;
    }
    if (toolId === "compressionStudio") {
      activeToolTab.set("compress");
      currentView.set("tools");
      return;
    }
    if (toolId === "privacyAudit") {
      activeToolTab.set("privacy");
      currentView.set("tools");
      return;
    }

    if (toolId === "crop") {
      cropDialogOpen = true;
      return;
    }
    if (toolId === "metadata") {
      metadataDialogOpen = true;
      return;
    }
    if (toolId === "epub2pdf" || toolId === "cbz2pdf" || toolId === "text2pdf" || toolId === "md2pdf") {
      handleMultiFormatImport(toolId);
      return;
    }

    const pageTools: ToolId[] = ["merge", "split", "rotate", "delete", "extractPages", "reorder", "insertPages", "pageNumber", "sanitize", "compress"];
    const otherDialogTools: ToolId[] = ["img2pdf", "pdf2img", "pdf2text", "extractText", "table"];

    if (pageTools.includes(toolId)) {
      pageDialogTool = toolId;
    } else if (toolId === "watermark") {
      watermarkDialogOpen = true;
    } else if (toolId === "sign") {
      handleSignTool();
    } else if (toolId === "ocrEdit") {
      handleOcrEdit();
    } else if (otherDialogTools.includes(toolId)) {
      pageDialogTool = toolId;
    } else {
      // Edit tools: editText, editRect, editHighlight
      activeCategory = "edit";
      activeTool = activeTool === toolId ? null : toolId;
      clearSelection();
    }
  }

  async function handleMultiFormatImport(toolId: ToolId) {
    let extensions: string[] = ["epub"];
    let filterName = "EPUB 电子书 (*.epub)";
    if (toolId === "cbz2pdf") {
      extensions = ["cbz", "zip"];
      filterName = "漫画归档 (*.cbz, *.zip)";
    } else if (toolId === "text2pdf") {
      extensions = ["txt"];
      filterName = "纯文本文件 (*.txt)";
    } else if (toolId === "md2pdf") {
      extensions = ["md", "markdown"];
      filterName = "Markdown 笔记 (*.md)";
    }

    const selected = await open({
      filters: [{ name: filterName, extensions }],
    });
    if (!selected) return;
    const path = typeof selected === "string" ? selected : String(selected);
    const fileName = path.split(/[\\/]/).pop() || "Ebook";

    tabsStore.openTab(path, fileName);
    currentFilePath.set(path);
    currentFileName.set(fileName);
  }

  // ─── Signature ────────────────────────────────────────────────────────

  async function handleSignTool() {
    const selected = await open({
      filters: [{ name: "Image", extensions: ["png", "jpg", "jpeg", "bmp"] }],
    });
    if (!selected) return;
    const path = typeof selected === "string" ? selected : String(selected);
    signPlacement.set({ active: true, imagePath: path });
  }

  function handleSignPlaceEvent(e: Event) {
    const detail = (e as CustomEvent).detail;
    if (!detail || !$currentFilePath) return;
    placeSignature(detail.imagePath || $signPlacement?.imagePath, detail.screenX, detail.screenY, detail.pageNum);
  }

  async function placeSignature(imagePath: string, screenX: number, screenY: number, pageNum: number) {
    if (!$currentFilePath || !imagePath) return;
    try {
      const out = await save({ filters: [{ name: "PDF", extensions: ["pdf"] }] });
      if (!out) return;
      const s = $zoomLevel * baseFitScale;
      const idx = pageNum - 1;
      const pH = basePageHeights[idx] || 842;
      const pdfX = screenX / s;
      const pdfY = pH - screenY / s - 100 / s;
      await invoke("sign_pdf", {
        req: {
          inputPath: $currentFilePath,
          outputPath: out,
          signatureImagePath: imagePath,
          page: pageNum,
          x: pdfX,
          y: pdfY,
          width: 200,
          height: 100,
        },
      });
      currentFilePath.set(out as string);
      currentFileName.set((out as string).split(/[\\/]/).pop() || "Untitled");
    } catch (e) {
      console.error("Sign failed:", e);
    } finally {
      signPlacement.set(null);
    }
  }

  // ─── Watermark ────────────────────────────────────────────────────────

  async function applyWatermark(params: { text: string; fontSize: number; opacity: number; angle: number; color: string }) {
    if (!$currentFilePath) return;
    try {
      const out = await save({ filters: [{ name: "PDF", extensions: ["pdf"] }] });
      if (!out) return;
      await invoke("add_text_watermark", {
        req: {
          inputPath: $currentFilePath,
          outputPath: out,
          text: params.text,
          fontSize: params.fontSize,
          opacity: params.opacity,
          angle: params.angle,
          color: params.color,
        },
      });
      currentFilePath.set(out as string);
      currentFileName.set((out as string).split(/[\\/]/).pop() || "Untitled");
    } catch (e) {
      console.error("Watermark failed:", e);
    }
  }

  // ─── OCR → Edit ─────────────────────────────────────────────────────

  let ocrBusy = $state(false);
  let ocrLanguage = $state("default");

  // ─── Text Layer Edit (select text → Edit → inline replace) ────────────

  let editTextOverlay = $state<{ visible: boolean; x: number; y: number; pageNum: number; fontSize: number; originalText: string; originalWidth: number; originalHeight: number }>({
    visible: false, x: 0, y: 0, pageNum: 1, fontSize: 12, originalText: "", originalWidth: 0, originalHeight: 0,
  });
  let editTextValue = $state("");

  function handleTextLayerEdit(item: { text: string; x: number; y: number; fontSize: number; pageNum: number }) {
    editTextOverlay = {
      visible: true,
      x: item.x,
      y: item.y,
      pageNum: item.pageNum,
      fontSize: item.fontSize,
      originalText: item.text,
      originalWidth: item.text.length * item.fontSize * 0.6,
      originalHeight: item.fontSize,
    };
    editTextValue = item.text;
  }

  function commitEditText() {
    if (!editTextOverlay.visible) return;
    const pH = basePageHeights[editTextOverlay.pageNum - 1] ?? 842;
    const { pageNum, x, y, fontSize, originalWidth, originalHeight } = editTextOverlay;

    // Whiteout the original text area
    editHistory.set(
      pushOperation($editHistory, "addWhiteout", {
        page: pageNum,
        x: x - 1,
        y: pH - y - originalHeight - 1,
        w: originalWidth + 2,
        h: originalHeight + 2,
      }),
    );

    // Add new text (if not empty)
    if (editTextValue.trim()) {
      const newW = editTextValue.length * fontSize * 0.6;
      editHistory.set(
        pushOperation($editHistory, "addText", {
          page: pageNum,
          x,
          y: pH - y - fontSize,
          w: newW,
          h: fontSize,
          text: editTextValue,
          fontSize,
          color: "#000000",
        }),
      );
    }

    editTextOverlay.visible = false;
  }

  const editTextScreenPos = $derived(() => {
    if (!editTextOverlay.visible || !scrollContainer) return { x: 0, y: 0 };
    const pageSlot = document.querySelector('[data-page="' + editTextOverlay.pageNum + '"]');
    const slotRect = pageSlot?.getBoundingClientRect();
    const containerRect = scrollContainer.getBoundingClientRect();
    if (!slotRect || !containerRect) return { x: 0, y: 0 };
    const pH = basePageHeights[editTextOverlay.pageNum - 1] ?? 842;
    const s = effectiveScale;
    return {
      x: slotRect.left - containerRect.left + editTextOverlay.x * s,
      y: slotRect.top - containerRect.top + (pH - editTextOverlay.y - editTextOverlay.originalHeight) * s,
    };
  });

  async function handleOcrEdit() {
    if (!pdfDoc || ocrBusy) return;

    // Check if OCR is configured
    try {
      const configured: boolean = await invoke("ocr_check_configured");
      if (!configured) {
        currentView.set("settings");
        return;
      }
    } catch {}

    ocrBusy = true;
    try {
      try {
        await invoke("ocr_init_engine", { language: ocrLanguage, gpuEnabled: true });
      } catch (e) {
        console.warn("OCR engine init failed, will use existing:", e);
      }

      const page = await pdfDoc.getPage($currentPage);
      const RENDER_SCALE = 2;
      const vp = page.getViewport({ scale: RENDER_SCALE });

      const canvas = document.createElement("canvas");
      canvas.width = Math.floor(vp.width);
      canvas.height = Math.floor(vp.height);
      const ctx = canvas.getContext("2d")!;
      await page.render({ canvasContext: ctx, viewport: vp }).promise;

      const tempDir = await invoke<string>("get_temp_dir");
      const imgPath = `${tempDir}/ocr_edit_${$currentPage}.png`;
      const blob = await new Promise<Blob | null>((r) => canvas.toBlob(r, "image/png"));
      if (!blob) return;
      const ab = await blob.arrayBuffer();
      await invoke("save_image_file", { path: imgPath, data: Array.from(new Uint8Array(ab)) });

      const boxes = await invoke<Array<{ points: [number, number][]; text: string; confidence: number }>>("ocr_recognize", { imagePath: imgPath, language: ocrLanguage });
      if (boxes.length === 0) return;

      // Store in ocrResults store for overlay display
      const currentResults = { ...$ocrResults };
      currentResults[$currentPage] = boxes.map((b) => ({
        points: b.points as [number, number][],
        text: b.text,
        confidence: b.confidence,
      }));
      ocrResults.set(currentResults);
      activeTool = null;
    } catch (e) {
      console.error("OCR edit failed:", e);
    } finally {
      ocrBusy = false;
    }
  }

  // ─── OCR Post-processing & Sandwich PDF Export ─────────────────────

  let ocrCopied = $state(false);
  let exportingSearchablePdf = $state(false);
  let ocrStatusMsg = $state<string | null>(null);

  async function handleCopyCleanedOcrText() {
    const results = $ocrResults;
    const pages = Object.keys(results).map(Number).sort((a, b) => a - b);
    if (pages.length === 0) return;

    let allText = "";
    for (const p of pages) {
      const pageBlocks = results[p] || [];
      const pageRaw = pageBlocks.map((b) => b.text).join("\n");
      const cleaned = cleanOcrText(pageRaw);
      if (cleaned.trim()) {
        allText += (allText ? "\n\n" : "") + cleaned;
      }
    }

    try {
      await navigator.clipboard.writeText(allText);
      ocrCopied = true;
      setTimeout(() => {
        ocrCopied = false;
      }, 2000);
    } catch (e) {
      console.error("Failed to copy cleaned text:", e);
    }
  }

  let tableCopied = $state(false);

  async function handleExportOcrTable() {
    const results = $ocrResults;
    const cur = $currentPage;
    const pageBlocks = results[cur] && results[cur].length ? results[cur] : Object.values(results).flat();
    if (!pageBlocks.length) {
      ocrStatusMsg = "当前页面暂无 OCR 识别数据";
      setTimeout(() => { ocrStatusMsg = null; }, 2000);
      return;
    }

    try {
      const tables: any = await invoke("ocr_detect_tables", {
        boxes: pageBlocks.map((b) => ({
          points: b.points,
          text: b.text,
          confidence: b.confidence ?? 0.95,
        })),
      });

      if (!tables || tables.length === 0) {
        ocrStatusMsg = "当前页面未检测到结构化表格";
        setTimeout(() => { ocrStatusMsg = null; }, 2500);
        return;
      }

      const md = tables.map((t: any) => t.markdown).join("\n\n");
      await navigator.clipboard.writeText(md);
      tableCopied = true;
      ocrStatusMsg = `已成功复制 ${tables.length} 个表格为 Markdown！`;
      setTimeout(() => {
        tableCopied = false;
        ocrStatusMsg = null;
      }, 3000);
    } catch (e) {
      ocrStatusMsg = `表格识别提取失败: ${e}`;
      setTimeout(() => { ocrStatusMsg = null; }, 3000);
    }
  }

  async function handleExportSearchablePdf() {
    if (!$currentFilePath || exportingSearchablePdf) return;
    const results = $ocrResults;
    const pageNums = Object.keys(results).map(Number).sort((a, b) => a - b);
    if (pageNums.length === 0) return;

    exportingSearchablePdf = true;
    ocrStatusMsg = null;
    try {
      const defaultName = ($currentFileName || "document").replace(/\.pdf$/i, "") + "_searchable.pdf";
      const outPath = await save({
        title: "导出双层可搜索 PDF",
        defaultPath: defaultName,
        filters: [{ name: "PDF", extensions: ["pdf"] }],
      });

      if (!outPath) {
        exportingSearchablePdf = false;
        return;
      }

      const pagesData = pageNums.map((p) => {
        const blocks = results[p] || [];
        const baseW = basePageWidths[p - 1] ?? 612;
        const baseH = basePageHeights[p - 1] ?? 842;
        // OCR render scale was 2
        return {
          page: p,
          imageWidth: baseW * 2,
          imageHeight: baseH * 2,
          boxes: blocks.map((b) => ({
            points: b.points,
            text: b.text,
            confidence: b.confidence,
          })),
        };
      });

      await invoke("create_searchable_pdf", {
        req: {
          inputPath: $currentFilePath,
          outputPath: outPath,
          pagesData,
        },
      });

      ocrStatusMsg = "双层可搜索 PDF 导出成功！";
      setTimeout(() => {
        ocrStatusMsg = null;
      }, 4000);
    } catch (e: any) {
      console.error("Export searchable PDF failed:", e);
      ocrStatusMsg = `导出失败: ${e?.message || e}`;
    } finally {
      exportingSearchablePdf = false;
    }
  }

  // ─── Save edits ─────────────────────────────────────────────────────

  let saving = $state(false);

  const hasEdits = $derived(() => {
    const hist = $editHistory;
    return hist.currentIndex >= 0;
  });

  async function handleSaveEdits(overwrite: boolean = false) {
    if (!$currentFilePath || saving) return;
    const isRemote = $currentFilePath.includes("pdf_seeker_remote");
    const isEbook = isEbookFormat($currentFilePath);
    if (!hasEdits() && !isRemote && !isEbook && overwrite) return;

    saving = true;
    try {
      const hist = $editHistory;
      const applied = hist.operations.slice(0, hist.currentIndex + 1);
      const ops = applied.map((op: EditOperation) => ({
        opType: op.type,
        params: op.params,
      }));

      let outputPath: string;
      if (overwrite && !isRemote && !isEbook) {
        outputPath = $currentFilePath;
      } else {
        const defaultName = ($currentFileName || "document")
          .replace(/\.(epub|cbz|cbr|txt|md|markdown|pdf)$/i, "") + ".pdf";
        const out = await save({
          title: "保存 PDF 到本地",
          defaultPath: defaultName,
          filters: [{ name: "PDF 文档", extensions: ["pdf"] }],
        });
        if (!out) {
          saving = false;
          return;
        }
        outputPath = typeof out === "string" ? out : (out as any).path;
      }

      if (ops.length > 0) {
        await invoke("apply_edit_operations", {
          inputPath: $currentFilePath,
          outputPath,
          operations: ops,
        });
      } else {
        const fileBytes = convertedEbookPdfBytes || (await readFile($currentFilePath));
        await writeFile(outputPath, fileBytes);
      }

      const newFileName = outputPath.split(/[\\/]/).pop() || "Untitled";
      currentFilePath.set(outputPath);
      currentFileName.set(newFileName);
      if ($tabsStore.activeTabId) {
        tabsStore.updateTabPath($tabsStore.activeTabId, outputPath, newFileName);
      }
      editHistory.set({ operations: [], currentIndex: -1 });
    } catch (e) {
      console.error("Save edits failed:", e);
    } finally {
      saving = false;
    }
  }

  function handleCloseFile() {
    const state = $tabsStore;
    if (state.activeTabId) {
      tabsStore.closeTab(state.activeTabId);
    }
    const newState = $tabsStore;
    const nextTab = newState.tabs.find((t) => t.id === newState.activeTabId);
    if (nextTab) {
      currentFilePath.set(nextTab.filePath);
      currentFileName.set(nextTab.fileName);
    } else {
      currentFilePath.set(null);
      currentFileName.set("");
    }
    editHistory.set({ operations: [], currentIndex: -1 });
    activeTool = null;
    pageDialogTool = null;
    ocrResults.set({});
  }

  function toggleTheme() {
    const next = !$isDark;
    isDark.set(next);
    if (typeof document !== "undefined") {
      document.documentElement.classList.toggle("dark", next);
    }
  }

  function handleTabChange(filePath: string) {
    editHistory.set({ operations: [], currentIndex: -1 });
    activeTool = null;
    pageDialogTool = null;
    ocrResults.set({});
    if (filePath) {
      currentFilePath.set(filePath);
      currentFileName.set(filePath.split(/[\\/]/).pop() || "Untitled");
    } else {
      currentFilePath.set(null);
      currentFileName.set("");
    }
  }

  function handleTabClose(filePath: string | null) {
    if (filePath) {
      currentFilePath.set(filePath);
      currentFileName.set(filePath.split(/[\\/]/).pop() || "Untitled");
    } else {
      currentFilePath.set(null);
      currentFileName.set("");
    }
    editHistory.set({ operations: [], currentIndex: -1 });
    activeTool = null;
    pageDialogTool = null;
    ocrResults.set({});
  }

  // ─── PDF rendering state ─────────────────────────────────────────────

  let pdfDoc: PdfDocumentProxy | null = $state(null);
  let loading = $state(false);
  let errorMsg = $state("");
  let scrollContainer: HTMLDivElement | undefined = $state(undefined);

  const PAGE_GAP = 8;
  const DPR = window.devicePixelRatio || 1;
  const MAX_CONCURRENT_RENDERS = 4;
  const MAX_CACHE_SIZE = 30;

  let pageHeights: number[] = $state([]);
  let pageWidths: number[] = $state([]);
  let canvasMap = new Map<number, HTMLCanvasElement>();
  let pageSlots = new Map<number, HTMLDivElement>();
  let pageCache = new Map<string, HTMLCanvasElement>();
  let observer: IntersectionObserver | null = null;
  let renderVersion = 0;
  let pendingRenders = 0;
  let renderQueue: number[] = [];
  let zoomDebounceTimer: ReturnType<typeof setTimeout> | null = null;

  // Viewport Virtualization & RenderTask cancellation (pdf.js parity)
  let virtualWindow = $state<VirtualWindow>({
    visibleStart: 1,
    visibleEnd: 1,
    bufferedStart: 1,
    bufferedEnd: 2,
  });
  let activeRenderTasks = new Map<number, any>();

  function cancelRenderTask(pageNum: number) {
    const task = activeRenderTasks.get(pageNum);
    if (task && typeof task.cancel === "function") {
      try {
        task.cancel();
      } catch {}
    }
    activeRenderTasks.delete(pageNum);
  }

  function cancelAllRenderTasks() {
    for (const [_, task] of activeRenderTasks) {
      if (task && typeof task.cancel === "function") {
        try {
          task.cancel();
        } catch {}
      }
    }
    activeRenderTasks.clear();
  }

  let basePageHeights = $state<number[]>([]);
  let basePageWidths = $state<number[]>([]);
  let baseFitScale = 1;

  // ─── Helpers ─────────────────────────────────────────────────────────

  function cacheKey(pageNum: number, zoom: number) {
    return `${pageNum}@${zoom.toFixed(2)}`;
  }

  function pageContainerWidth(): number {
    if (!scrollContainer) return 800;
    return scrollContainer.clientWidth - 48;
  }

  // ─── Document loading ───────────────────────────────────────────────

  let showUrlDialog = $state(false);
  let urlInput = $state("");
  let urlLoading = $state(false);
  let urlError = $state("");

  async function handleOpenFromUrl() {
    const url = urlInput.trim();
    if (!url) return;
    urlLoading = true;
    urlError = "";
    try {
      const res: { localPath: string; fileName: string; fileSize: number } =
        await invoke("download_pdf_from_url", { url });
      tabsStore.openTab(res.localPath, res.fileName);
      currentFilePath.set(res.localPath);
      currentFileName.set(res.fileName);
      try {
        await invoke("add_recent_file", { path: res.localPath });
      } catch (_) {}
      showUrlDialog = false;
      urlInput = "";
    } catch (err: any) {
      urlError = typeof err === "string" ? err : err?.message || String(err);
    } finally {
      urlLoading = false;
    }
  }

  let convertedEbookPdfBytes: Uint8Array | null = null;

  async function handleOpen() {
    const selected = await open({
      multiple: false,
      filters: SUPPORTED_OPEN_FILTERS,
    });
    if (selected) {
      const path = typeof selected === "string" ? selected : String(selected);
      const fileName = path.split(/[\\/]/).pop() || "Untitled";
      tabsStore.openTab(path, fileName);
      currentFilePath.set(path);
      currentFileName.set(fileName);
    }
  }

  function handleDrop(e: DragEvent) {
    e.preventDefault();
    const file: File | undefined = e.dataTransfer?.files[0];
    if (!file) return;
    const name = file.name.toLowerCase();
    const isSupported = name.endsWith(".pdf") || isEbookFormat(name);
    if (!isSupported) return;
    const path = (file as any).path || file.name;
    const fileName = path.split(/[\\/]/).pop() || "Untitled";
    tabsStore.openTab(path, fileName);
    currentFilePath.set(path);
    currentFileName.set(fileName);
  }

  async function loadDocument(path: string) {
    loading = true;
    errorMsg = "";
    pdfDoc = null;
    convertedEbookPdfBytes = null;
    cancelAllRenderTasks();
    renderVersion++;
    pageCache.clear();
    pageHeights = [];
    pageWidths = [];
    basePageHeights = [];
    basePageWidths = [];
    try {
      const rawData = await readFile(path);
      let pdfBytes: Uint8Array;

      const lower = path.toLowerCase();
      if (lower.endsWith(".epub")) {
        pdfBytes = await convertEpubToPdf(rawData, path.split(/[\\/]/).pop()?.replace(/\.epub$/i, ""));
        convertedEbookPdfBytes = pdfBytes;
      } else if (lower.endsWith(".cbz") || lower.endsWith(".zip")) {
        pdfBytes = await convertCbzToPdf(rawData, path.split(/[\\/]/).pop()?.replace(/\.(cbz|zip)$/i, ""));
        convertedEbookPdfBytes = pdfBytes;
      } else if (lower.endsWith(".txt") || lower.endsWith(".md") || lower.endsWith(".markdown")) {
        const text = new TextDecoder().decode(rawData);
        const title = path.split(/[\\/]/).pop()?.replace(/\.[^/.]+$/, "") || "Document";
        pdfBytes = await convertTextToPdf(text, title);
        convertedEbookPdfBytes = pdfBytes;
      } else {
        pdfBytes = new Uint8Array(rawData);
      }

      const doc = await loadPdf(pdfBytes);
      pdfDoc = doc;
      totalPages.set(doc.numPages);
      currentPage.set(1);
      zoomLevel.set(1.0);
      await loadBaseDimensions(doc);
      applyZoomToDimensions(1.0);
      updateVirtualWindow(true);
    } catch (e) {
      errorMsg = String(e);
      pdfDoc = null;
    } finally {
      loading = false;
    }
  }

  async function loadBaseDimensions(doc: PdfDocumentProxy) {
    const containerW = pageContainerWidth();
    const firstVp = await getPageViewport(doc, 1, 1);
    baseFitScale = Math.max(0.1, Math.min(containerW / firstVp.width, 2));
    basePageHeights = [];
    basePageWidths = [];
    for (let i = 1; i <= doc.numPages; i++) {
      const vp = await getPageViewport(doc, i, 1);
      const h = Number.isFinite(vp.height) && vp.height > 0 ? vp.height : 842;
      const w = Number.isFinite(vp.width) && vp.width > 0 ? vp.width : 595;
      basePageHeights.push(h);
      basePageWidths.push(w);
    }
  }

  function applyZoomToDimensions(zoom: number) {
    const s = zoom * baseFitScale;
    pageHeights = basePageHeights.map((h) => Math.max(50, Math.round(h * s)));
    pageWidths = basePageWidths.map((w) => Math.max(50, Math.round(w * s)));
  }

  function evictCache() {
    if (pageCache.size <= MAX_CACHE_SIZE) return;
    const keys = [...pageCache.keys()];
    for (let i = 0; i < keys.length - MAX_CACHE_SIZE; i++) {
      pageCache.delete(keys[i]);
    }
  }

  // ─── Rendering Pipeline (pdf.js Virtualized Windowing) ──────────────────

  async function renderPage(pageNum: number) {
    const doc = pdfDoc;
    if (!doc) return;

    // Only render pages inside the active buffered window
    if (pageNum < virtualWindow.bufferedStart || pageNum > virtualWindow.bufferedEnd) {
      return;
    }

    const canvas = canvasMap.get(pageNum);
    if (!canvas) return;

    const version = renderVersion;
    const z = $zoomLevel;

    try {
      const effectiveScale = z * baseFitScale;
      const key = cacheKey(pageNum, z);
      const cached = pageCache.get(key);
      if (cached) {
        const ctx = canvas.getContext("2d");
        if (ctx) {
          canvas.width = cached.width;
          canvas.height = cached.height;
          ctx.drawImage(cached, 0, 0);
        }
        return;
      }

      if (pendingRenders >= MAX_CONCURRENT_RENDERS) {
        if (!renderQueue.includes(pageNum)) renderQueue.push(pageNum);
        return;
      }

      // Cancel any ongoing stale render task for this specific page
      cancelRenderTask(pageNum);
      pendingRenders++;

      const page = await doc.getPage(pageNum);
      if (renderVersion !== version) {
        pendingRenders--;
        processQueue();
        return;
      }

      const viewport = page.getViewport({ scale: effectiveScale });
      const { canvasWidth, canvasHeight, effectiveDpr } = clampCanvasDimensions(
        viewport.width,
        viewport.height,
        DPR
      );

      const offscreen = document.createElement("canvas");
      offscreen.width = canvasWidth;
      offscreen.height = canvasHeight;
      const offCtx = offscreen.getContext("2d");
      if (!offCtx) {
        pendingRenders--;
        processQueue();
        return;
      }
      offCtx.setTransform(effectiveDpr, 0, 0, effectiveDpr, 0, 0);

      const renderTask = page.render({ canvasContext: offCtx, viewport });
      activeRenderTasks.set(pageNum, renderTask);

      try {
        await renderTask.promise;
      } catch (err: any) {
        if (err?.name === "RenderingCancelledException" || renderVersion !== version) {
          pendingRenders--;
          processQueue();
          return;
        }
        throw err;
      } finally {
        activeRenderTasks.delete(pageNum);
      }

      if (renderVersion !== version) {
        pendingRenders--;
        processQueue();
        return;
      }

      pageCache.set(key, offscreen);
      evictCache();

      const ctx = canvas.getContext("2d");
      if (ctx) {
        canvas.width = canvasWidth;
        canvas.height = canvasHeight;
        ctx.drawImage(offscreen, 0, 0);

        if ($isDark) {
          ctx.fillStyle = "rgba(0, 0, 0, 0.15)";
          ctx.fillRect(0, 0, canvasWidth, canvasHeight);
        }
      }

      pendingRenders--;
      processQueue();
    } catch (e: any) {
      if (e?.name !== "RenderingCancelledException") {
        console.warn(`Render page ${pageNum} error:`, e);
      }
      pendingRenders--;
      processQueue();
    }
  }

  function processQueue() {
    if (renderQueue.length === 0 || pendingRenders >= MAX_CONCURRENT_RENDERS) return;
    renderPage(renderQueue.shift()!);
  }

  function renderVisiblePages() {
    updateVirtualWindow(true);
  }

  // ─── IntersectionObserver & Lifecycle ───────────────────────────────

  function setupObserver() {
    if (observer) observer.disconnect();
    observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          const pageNum = Number((entry.target as HTMLDivElement).dataset.page);
          if (entry.isIntersecting) renderPage(pageNum);
        }
      },
      { root: scrollContainer || null, rootMargin: "400px 0px", threshold: 0 },
    );
  }

  function observePage(node: HTMLDivElement, pageNum: number) {
    const canvas = node.querySelector("canvas") as HTMLCanvasElement;
    if (canvas) {
      canvasMap.set(pageNum, canvas);
      renderPage(pageNum);
    }
    pageSlots.set(pageNum, node);
    if (observer) observer.observe(node);
    return {
      update(newPageNum: number) {
        if (canvas) canvasMap.delete(pageNum);
        pageSlots.delete(pageNum);
        if (observer) observer.unobserve(node);
        pageNum = newPageNum;
        const c = node.querySelector("canvas") as HTMLCanvasElement;
        if (c) {
          canvasMap.set(newPageNum, c);
          renderPage(newPageNum);
        }
        pageSlots.set(newPageNum, node);
        if (observer) observer.observe(node);
      },
      destroy() {
        if (observer) observer.unobserve(node);
        cancelRenderTask(pageNum);
        canvasMap.delete(pageNum);
        pageSlots.delete(pageNum);
      },
    };
  }

  // ─── Virtual Window & Scroll Tracking ───────────────────────────────

  let scrollRaf = 0;

  function onScroll() {
    if (!scrollContainer) return;
    const top = scrollContainer.scrollTop;
    if (top > 300 && !zenReminderDismissed && !zenModeOpen && !showZenReminder) {
      showZenReminder = true;
    }

    if (scrollRaf) cancelAnimationFrame(scrollRaf);
    scrollRaf = requestAnimationFrame(() => {
      updateCurrentPage();
      updateVirtualWindow();
    });
  }

  function updateVirtualWindow(force = false) {
    if (!scrollContainer || pageHeights.length === 0) return;
    const scrollTop = scrollContainer.scrollTop;
    const clientHeight = scrollContainer.clientHeight || 800;
    const nextWin = computeVisiblePages(scrollTop, clientHeight, pageHeights, PAGE_GAP, 1);

    const changed =
      nextWin.bufferedStart !== virtualWindow.bufferedStart ||
      nextWin.bufferedEnd !== virtualWindow.bufferedEnd ||
      nextWin.visibleStart !== virtualWindow.visibleStart ||
      nextWin.visibleEnd !== virtualWindow.visibleEnd;

    if (changed || force) {
      // Cancel tasks for pages that fell out of the active buffer
      for (const [pageNum] of activeRenderTasks) {
        if (pageNum < nextWin.bufferedStart || pageNum > nextWin.bufferedEnd) {
          cancelRenderTask(pageNum);
        }
      }
      virtualWindow = nextWin;

      // Render visible pages first
      for (let p = virtualWindow.visibleStart; p <= virtualWindow.visibleEnd; p++) {
        renderPage(p);
      }
      // Pre-render buffered lookahead pages
      for (let p = virtualWindow.bufferedStart; p <= virtualWindow.bufferedEnd; p++) {
        if (p < virtualWindow.visibleStart || p > virtualWindow.visibleEnd) {
          renderPage(p);
        }
      }
    }
  }

  let isZooming = false;
  let zoomLockTimer: ReturnType<typeof setTimeout> | null = null;

  function markZooming() {
    isZooming = true;
    if (zoomLockTimer) clearTimeout(zoomLockTimer);
    zoomLockTimer = setTimeout(() => {
      isZooming = false;
    }, 280);
  }

  function updateCurrentPage() {
    if (!scrollContainer || pageHeights.length === 0 || isZooming) return;
    const scrollTop = scrollContainer.scrollTop;
    const viewportCenter = scrollTop + (scrollContainer.clientHeight / 2);
    let acc = 0;
    for (let i = 0; i < pageHeights.length; i++) {
      const pageH = pageHeights[i] + PAGE_GAP;
      if (acc + pageH >= viewportCenter) {
        if ($currentPage !== i + 1) currentPage.set(i + 1);
        return;
      }
      acc += pageH;
    }
    if ($currentPage !== pageHeights.length) currentPage.set(pageHeights.length);
  }

  function scrollToPage(pageNum: number) {
    if (!scrollContainer || pageNum < 1 || pageHeights.length === 0) return;
    const clamped = Math.min(pageNum, pageHeights.length);
    let targetTop = 16;
    for (let i = 0; i < clamped - 1; i++) {
      targetTop += (pageHeights[i] || 0) + PAGE_GAP;
    }
    scrollContainer.scrollTo({
      top: Math.max(0, targetTop - 20),
      behavior: "smooth",
    });
  }

  function changeZoom(newZ: number) {
    const oldZ = $zoomLevel;
    const clamped = Math.min(5.0, Math.max(0.25, Math.round(newZ * 20) / 20));
    if (clamped === oldZ || !scrollContainer) return;
    markZooming();
    const rect = scrollContainer.getBoundingClientRect();
    const { targetScrollLeft, targetScrollTop } = computeScrollAnchor(oldZ, clamped, {
      clientX: rect.left + scrollContainer.clientWidth / 2,
      clientY: rect.top + scrollContainer.clientHeight / 2,
      rectLeft: rect.left,
      rectTop: rect.top,
      scrollLeft: scrollContainer.scrollLeft,
      scrollTop: scrollContainer.scrollTop,
    });
    applyZoomToDimensions(clamped);
    zoomLevel.set(clamped);
    requestAnimationFrame(() => {
      if (scrollContainer) {
        scrollContainer.scrollLeft = Math.max(0, targetScrollLeft);
        scrollContainer.scrollTop = Math.max(0, targetScrollTop);
      }
    });
  }

  // ─── Zoom with cursor-center (Trackpad pinch & Linear mouse wheel) ───

  function handleWheel(e: WheelEvent) {
    if (e.ctrlKey || e.metaKey) {
      e.preventDefault();
      e.stopPropagation();
      markZooming();
      const oldZ = $zoomLevel;
      const newZ = computeZoomLevel(oldZ, e);

      if (newZ === oldZ || !scrollContainer) return;

      const rect = scrollContainer.getBoundingClientRect();
      const { targetScrollLeft, targetScrollTop } = computeScrollAnchor(oldZ, newZ, {
        clientX: e.clientX,
        clientY: e.clientY,
        rectLeft: rect.left,
        rectTop: rect.top,
        scrollLeft: scrollContainer.scrollLeft,
        scrollTop: scrollContainer.scrollTop,
      });

      applyZoomToDimensions(newZ);
      zoomLevel.set(newZ);

      requestAnimationFrame(() => {
        if (scrollContainer) {
          scrollContainer.scrollLeft = Math.max(0, targetScrollLeft);
          scrollContainer.scrollTop = Math.max(0, targetScrollTop);
        }
      });
    }
  }

  $effect(() => {
    const el = scrollContainer;
    if (!el) return;
    el.addEventListener("wheel", handleWheel, { passive: false });
    return () => {
      el.removeEventListener("wheel", handleWheel);
    };
  });

  // ─── Keyboard shortcuts ─────────────────────────────────────────────

  function handleKeydown(e: KeyboardEvent) {
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
    // Cmd+Shift+F for Zen Mode
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === "f") {
      e.preventDefault();
      zenModeOpen = !zenModeOpen;
      return;
    }
    // Cmd+I for PDF Info
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "i") {
      e.preventDefault();
      pdfInfoOpen = true;
      return;
    }
    // Ctrl+Tab / Ctrl+Shift+Tab for tab switching
    if ((e.ctrlKey || e.metaKey) && e.key === "Tab") {
      e.preventDefault();
      if (e.shiftKey) {
        tabsStore.activatePrevTab();
      } else {
        tabsStore.activateNextTab();
      }
      const newState = $tabsStore;
      const tab = newState.tabs.find((t) => t.id === newState.activeTabId);
      if (tab) handleTabChange(tab.filePath);
      return;
    }
    // Ctrl+W to close tab
    if ((e.ctrlKey || e.metaKey) && e.key === "w") {
      e.preventDefault();
      handleCloseFile();
      return;
    }
    // Ctrl+F / Cmd+F for PDF keyword search
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "f") {
      e.preventDefault();
      searchOpen = true;
      return;
    }
    switch (e.key) {
      case "ArrowLeft":
      case "PageUp":
        e.preventDefault();
        if ($currentPage > 1) currentPage.set($currentPage - 1);
        break;
      case "ArrowRight":
      case "PageDown":
        e.preventDefault();
        if ($currentPage < $totalPages) currentPage.set($currentPage + 1);
        break;
      case "z":
      case "Z":
        if (!e.ctrlKey && !e.metaKey && !e.altKey && $currentFilePath && !activeTool) {
          e.preventDefault();
          zenModeOpen = true;
        }
        break;
      case "+":
      case "=":
        if (e.ctrlKey || e.metaKey) {
          e.preventDefault();
          changeZoom($zoomLevel + 0.25);
        }
        break;
      case "-":
        if (e.ctrlKey || e.metaKey) {
          e.preventDefault();
          changeZoom($zoomLevel - 0.25);
        }
        break;
      case "0":
        if (e.ctrlKey || e.metaKey) {
          e.preventDefault();
          changeZoom(1.0);
        }
        break;
      case "Escape":
        activeTool = null;
        signPlacement.set(null);
        clearSelection();
        break;
    }
  }

  // ─── Effects ────────────────────────────────────────────────────────

  $effect(() => {
    const path = $currentFilePath;
    if (path) {
      loadDocument(path);
    } else {
      pdfDoc = null;
      totalPages.set(0);
      pageHeights = [];
      pageWidths = [];
      pageCache.clear();
    }
  });

  $effect(() => {
    if (scrollContainer) setupObserver();
    return () => { if (observer) { observer.disconnect(); observer = null; } };
  });

  $effect(() => {
    const el = scrollContainer;
    if (!el) return;
    el.addEventListener("scroll", onScroll, { passive: true });
    return () => el.removeEventListener("scroll", onScroll);
  });

  $effect(() => {
    window.addEventListener("keydown", handleKeydown);
    return () => window.removeEventListener("keydown", handleKeydown);
  });

  $effect(() => {
    void $zoomLevel;
    if (!pdfDoc) return;
    applyZoomToDimensions($zoomLevel);
    if (zoomDebounceTimer) clearTimeout(zoomDebounceTimer);
    zoomDebounceTimer = setTimeout(() => {
      cancelAllRenderTasks();
      renderVersion++;
      updateVirtualWindow(true);
      zoomDebounceTimer = null;
    }, 120);
  });

  // Listen for sign placement events from CanvasEditor
  $effect(() => {
    const el = scrollContainer;
    if (!el) return;
    el.addEventListener("signplace", handleSignPlaceEvent);
    return () => el.removeEventListener("signplace", handleSignPlaceEvent);
  });

  // ─── Derived ────────────────────────────────────────────────────────

  const effectiveScale = $derived($zoomLevel * baseFitScale);

  const containerMaxWidth = $derived(
    pageWidths.length > 0 ? Math.max(...pageWidths) + 48 : "100%",
  );
</script>

<div class="flex flex-col h-full">
  <!-- Tab bar -->
  <TabBar
    onopenfile={handleOpen}
    onopenurl={() => { showUrlDialog = true; urlError = ""; }}
    ontabchange={handleTabChange}
    ontabclose={handleTabClose}
  />

  <!-- Unified Integrated PDF Command Bar (Single sleek 42px row - No overlap, no redundancy) -->
  <header class="flex items-center justify-between px-3 h-[42px] border-b border-border bg-card shrink-0 select-none gap-2 overflow-x-auto scrollbar-none z-10">
    <!-- Left: Sidebar Toggle, Open File, Page Navigation & Zoom -->
    <div class="flex items-center gap-1 shrink-0">
      {#if $currentFilePath}
        <!-- Toggle main app sidebar -->
        <Tooltip message={$sidebarCollapsed ? "展开主导航" : "收起主导航"} placement="bottom">
          <button
            onclick={() => sidebarCollapsed.update((v) => !v)}
            class="flex items-center justify-center w-7 h-7 rounded-md text-muted-foreground hover:bg-accent hover:text-accent-foreground transition-colors shrink-0"
          >
            {#if $sidebarCollapsed}
              <PanelLeft size={15} />
            {:else}
              <PanelLeftClose size={15} />
            {/if}
          </button>
        </Tooltip>

        <!-- Open other file -->
        <Tooltip message={t("toolbar.openFile")} placement="bottom">
          <button
            onclick={handleOpen}
            class="flex items-center justify-center w-7 h-7 rounded-md text-muted-foreground hover:bg-accent hover:text-accent-foreground transition-colors shrink-0"
          >
            <FolderOpen size={15} />
          </button>
        </Tooltip>

        <div class="h-4 w-px bg-border/60 mx-0.5 shrink-0"></div>

        <!-- Thumbnail sidebar toggle -->
        <Tooltip message={thumbnailVisible ? "收起页面缩略图" : "展开页面缩略图"} placement="bottom">
          <button
            onclick={() => (thumbnailVisible = !thumbnailVisible)}
            class="flex items-center justify-center w-7 h-7 rounded-md text-muted-foreground hover:bg-accent hover:text-accent-foreground transition-colors shrink-0"
          >
            <Layers size={14} class="opacity-75" />
          </button>
        </Tooltip>

        <div class="h-4 w-px bg-border/60 mx-0.5 shrink-0"></div>

        <!-- Page navigation -->
        <div class="flex items-center shrink-0">
          <Tooltip message="上一页 (PageUp / ←)" placement="bottom">
            <button
              onclick={() => scrollToPage($currentPage - 1)}
              disabled={$currentPage <= 1}
              class="flex items-center justify-center w-6 h-6 rounded-md text-muted-foreground hover:bg-accent hover:text-accent-foreground disabled:opacity-40 transition-colors shrink-0"
            >
              <ChevronLeft size={14} />
            </button>
          </Tooltip>

          <span class="text-xs font-medium tabular-nums min-w-[3.4rem] text-center text-muted-foreground select-none whitespace-nowrap shrink-0">
            {$currentPage} / {$totalPages || 1}
          </span>

          <Tooltip message="下一页 (PageDown / →)" placement="bottom">
            <button
              onclick={() => scrollToPage($currentPage + 1)}
              disabled={$currentPage >= $totalPages}
              class="flex items-center justify-center w-6 h-6 rounded-md text-muted-foreground hover:bg-accent hover:text-accent-foreground disabled:opacity-40 transition-colors shrink-0"
            >
              <ChevronRight size={14} />
            </button>
          </Tooltip>
        </div>

        <div class="h-4 w-px bg-border/60 mx-0.5 shrink-0"></div>

        <!-- Zoom controls -->
        <div class="flex items-center shrink-0">
          <Tooltip message="缩小 (⌘-)" placement="bottom">
            <button
              onclick={() => changeZoom($zoomLevel - 0.25)}
              class="flex items-center justify-center w-6 h-6 rounded-md text-muted-foreground hover:bg-accent hover:text-accent-foreground transition-colors shrink-0"
            >
              <ZoomOut size={14} />
            </button>
          </Tooltip>

          <Tooltip message="点击重置为 100% (⌘0)" placement="bottom">
            <button
              onclick={() => changeZoom(1.0)}
              class="text-xs font-medium tabular-nums px-1.5 py-0.5 rounded-md text-muted-foreground hover:bg-accent hover:text-accent-foreground transition-colors min-w-[3.2rem] text-center whitespace-nowrap shrink-0"
            >
              {Math.round($zoomLevel * 100)}%
            </button>
          </Tooltip>

          <Tooltip message="放大 (⌘+)" placement="bottom">
            <button
              onclick={() => changeZoom($zoomLevel + 0.25)}
              class="flex items-center justify-center w-6 h-6 rounded-md text-muted-foreground hover:bg-accent hover:text-accent-foreground transition-colors shrink-0"
            >
              <ZoomIn size={14} />
            </button>
          </Tooltip>
        </div>
      {/if}
    </div>

    <!-- Center: Inline Bento Category Strip (Clean 4 Categories without overlap) -->
    <div class="flex items-center justify-center shrink-0 relative px-1">
      {#if $currentFilePath}
        <ToolbarTabs
          bind:activeCategory
          bind:activeTool
          ontoolaction={handleToolAction}
        />
      {/if}
    </div>

    <!-- Right: Unified Actions, Zen Reading & Toolbox -->
    <div class="flex items-center gap-1.5 shrink-0">
      {#if $currentFilePath && (hasEdits() || isEbookFormat($currentFilePath))}
        {#if !isEbookFormat($currentFilePath)}
          <button
            onclick={() => handleSaveEdits(true)}
            disabled={saving}
            class="flex items-center gap-1 px-2.5 py-1 rounded-md text-xs font-medium bg-primary text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-50 whitespace-nowrap shadow-xs shrink-0"
          >
            {#if saving}
              <Loader2 size={12} class="animate-spin" />
            {:else}
              <Save size={12} />
            {/if}
            <span>{t("editor.save")}</span>
          </button>
        {/if}
        <button
          onclick={() => handleSaveEdits(false)}
          disabled={saving}
          class="flex items-center gap-1 px-2.5 py-1 rounded-md text-xs font-medium border border-border hover:bg-accent transition-colors disabled:opacity-50 whitespace-nowrap shrink-0 {isEbookFormat($currentFilePath) ? 'bg-primary/10 border-primary/30 text-primary font-semibold' : ''}"
          title="将当前文档/电子书保存为 PDF 文件"
        >
          <SaveAll size={12} />
          <span>{isEbookFormat($currentFilePath) ? "导出为 PDF" : t("editor.saveAs")}</span>
        </button>
        <div class="h-4 w-px bg-border/60 mx-0.5 shrink-0"></div>
      {/if}

      {#if $currentFilePath}
        <!-- 统一规范的核心阅读模式入口 -->
        <Tooltip message="开启全屏沉浸阅读 (快捷键 Z 或 ⌘Shift+F)" placement="bottom">
          <button
            onclick={() => (zenModeOpen = true)}
            class="flex items-center gap-1.5 px-3 py-1 rounded-full text-xs font-semibold bg-primary text-primary-foreground shadow-xs hover:brightness-110 active:scale-95 transition-all whitespace-nowrap shrink-0"
          >
            <BookOpen size={13} class="shrink-0" />
            <span>沉浸阅读</span>
            <kbd class="px-1 py-0.2 text-[10px] font-mono bg-primary-foreground/20 text-primary-foreground rounded">Z</kbd>
          </button>
        </Tooltip>

        <!-- 全部 50+ 工具箱 (⌘K) -->
        <Tooltip message="全部 50+ 工具箱 (⌘K)" placement="bottom">
          <button
            onclick={() => (toolboxVisible = true)}
            class="flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-xs font-medium text-primary hover:bg-primary/10 border border-primary/25 transition-colors whitespace-nowrap shadow-2xs shrink-0"
          >
            <Sparkles size={13} />
            <span>工具箱</span>
            <kbd class="ml-0.5 px-1 py-0.2 rounded bg-primary/15 text-[10px] font-mono leading-none">⌘K</kbd>
          </button>
        </Tooltip>

        <div class="h-4 w-px bg-border/60 mx-0.5 shrink-0"></div>

        <!-- 打印文档 (⌘P) -->
        <Tooltip message="打印文档 (⌘P)" placement="bottom">
          <button
            onclick={() => (printDialogOpen = true)}
            class="flex items-center justify-center w-7 h-7 rounded-md text-muted-foreground hover:bg-accent hover:text-accent-foreground transition-colors shrink-0"
          >
            <Printer size={14} />
          </button>
        </Tooltip>

        <!-- 批注与图层管理 -->
        <Tooltip message="批注与图层面板" placement="bottom">
          <button
            onclick={() => (layersPanelOpen = !layersPanelOpen)}
            class="flex items-center gap-1 px-2 h-7 rounded-md text-xs font-medium transition-colors shrink-0 {layersPanelOpen ? 'bg-primary text-primary-foreground' : 'text-muted-foreground hover:bg-accent hover:text-accent-foreground'}"
          >
            <span>图层</span>
            {#if currentPageOperations().length > 0}
              <span class="px-1 py-0.2 text-[10px] bg-primary/20 rounded-full font-mono">{currentPageOperations().length}</span>
            {/if}
          </button>
        </Tooltip>

        <div class="h-4 w-px bg-border/60 mx-0.5 shrink-0"></div>

        <!-- 文档文件名 + 属性胶囊 (⌘I) -->
        <div class="flex items-center gap-1 bg-accent/30 hover:bg-accent/50 px-2 py-0.5 rounded-lg border border-border/50 text-xs transition-colors max-w-[150px] shrink-0">
          <span
            class="text-xs font-medium text-foreground truncate"
            title={$currentFilePath}
          >
            {$currentFileName}
          </span>
          <Tooltip message="文档属性 (⌘I)" placement="bottom">
            <button
              onclick={() => (pdfInfoOpen = true)}
              class="p-0.5 rounded text-muted-foreground hover:text-foreground transition-colors shrink-0"
            >
              <Info size={12} />
            </button>
          </Tooltip>
        </div>

        <!-- 主题切换 -->
        <Tooltip message={$isDark ? "切换明亮模式" : "切换暗黑模式"} placement="bottom">
          <button
            onclick={toggleTheme}
            class="flex items-center justify-center w-7 h-7 rounded-md text-muted-foreground hover:bg-accent hover:text-accent-foreground transition-colors shrink-0"
          >
            {#if $isDark}
              <Sun size={15} />
            {:else}
              <Moon size={15} />
            {/if}
          </button>
        </Tooltip>

        <!-- 关闭当前文档 -->
        <Tooltip message="关闭文档 (⌘W)" placement="bottom">
          <button
            onclick={handleCloseFile}
            class="flex items-center justify-center w-7 h-7 rounded-md text-muted-foreground hover:bg-destructive/10 hover:text-destructive transition-colors shrink-0"
          >
            <X size={15} />
          </button>
        </Tooltip>
      {/if}
    </div>
  </header>

  <!-- Main content area -->
  <div class="flex flex-1 min-h-0 min-w-0 overflow-hidden relative">
    <!-- Thumbnail sidebar -->
    {#if $currentFilePath}
      <ThumbnailSidebar
        doc={pdfDoc}
        bind:visible={thumbnailVisible}
        onpageclick={(pageNum: number) => scrollToPage(pageNum)}
        onreorder={handleThumbnailReorder}
      />
    {/if}

    <!-- PDF canvas area -->
    {#if !$currentFilePath}
      <div class="flex-1 flex flex-col items-center justify-center gap-4">
        <FileText size={48} class="text-muted-foreground/30" />
        <p class="text-muted-foreground">{t("viewer.noFile")}</p>
        <Button variant="outline" onclick={handleOpen}>
          {t("toolbar.openFile")}
        </Button>
      </div>
    {:else}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        bind:this={scrollContainer}
        class="flex-1 min-w-0 overflow-y-auto overflow-x-auto relative {$isDark ? 'bg-zinc-900' : 'bg-gray-100'} transition-colors duration-200"
        ondragover={(e) => e.preventDefault()}
        ondrop={handleDrop}
      >
        <PdfSearchBar
          doc={pdfDoc}
          bind:open={searchOpen}
          onmatchchange={(matches, idx) => {
            if (matches.length > 0 && idx >= 0) {
              const m = matches[idx];
              if (m) {
                scrollToPage(m.pageNum);
              }
            }
          }}
        />

        {#if loading}
          <div class="flex flex-col items-center justify-center h-full gap-3">
            <Loader2 size={32} class="text-muted-foreground animate-spin" />
            <span class="text-sm text-muted-foreground">{t("app.loading")}</span>
          </div>
        {:else if errorMsg}
          <div class="flex flex-col items-center justify-center h-full gap-3">
            <FileText size={32} class="text-destructive/50" />
            <span class="text-sm text-destructive">{errorMsg}</span>
          </div>
        {:else if pageHeights.length > 0}
          <!-- Floating OCR control bar when OCR results exist -->
          {#if Object.keys($ocrResults).length > 0}
            <div class="sticky top-3 z-30 flex flex-col items-center pointer-events-none mb-2 gap-1.5">
              <div class="pointer-events-auto bg-card/95 backdrop-blur-md border border-border shadow-lg rounded-full px-4 py-1.5 flex items-center gap-3 text-xs">
                <span class="flex items-center gap-1.5 font-medium text-foreground">
                  <ScanLine size={14} class="text-primary" />
                  OCR 识别 ({Object.values($ocrResults).flat().length} 处)
                </span>
                <div class="h-3 w-px bg-border"></div>
                <button
                  class="hover:text-primary transition-colors flex items-center gap-1 text-muted-foreground font-medium"
                  onclick={handleCopyCleanedOcrText}
                  title="智能合并断行与中西文空格排版并复制"
                >
                  {#if ocrCopied}
                    <Check size={13} class="text-green-500" />
                    <span class="text-green-500 font-medium">已复制清洗文本</span>
                  {:else}
                    <Sparkles size={13} class="text-amber-500" />
                    <span>智能清洗复制</span>
                  {/if}
                </button>
                <div class="h-3 w-px bg-border"></div>
                <button
                  class="hover:text-primary transition-colors flex items-center gap-1 text-muted-foreground font-medium disabled:opacity-50"
                  disabled={exportingSearchablePdf}
                  onclick={handleExportSearchablePdf}
                  title="利用隐形文本层生成可划词检索复制的双层 Sandwich PDF"
                >
                  {#if exportingSearchablePdf}
                    <Loader2 size={13} class="animate-spin text-primary" />
                    <span>生成中...</span>
                  {:else}
                    <Download size={13} class="text-primary" />
                    <span>导出可搜索 PDF</span>
                  {/if}
                </button>
                <div class="h-3 w-px bg-border"></div>
                <button
                  class="hover:text-primary transition-colors flex items-center gap-1 text-muted-foreground font-medium"
                  onclick={handleExportOcrTable}
                  title="自动识别页面表格结构并复制为 Markdown 格式"
                >
                  {#if tableCopied}
                    <Check size={13} class="text-green-500" />
                    <span class="text-green-500 font-medium">已复制表格</span>
                  {:else}
                    <IconTable size={13} class="text-indigo-500" />
                    <span>导出表格 (MD)</span>
                  {/if}
                </button>
                <div class="h-3 w-px bg-border"></div>
                <button
                  class="hover:text-primary transition-colors flex items-center gap-1 text-muted-foreground"
                  onclick={() => (ocrOverlayVisible = !ocrOverlayVisible)}
                >
                  {#if ocrOverlayVisible}
                    <EyeOff size={13} /> 隐藏识别框
                  {:else}
                    <Eye size={13} /> 显示识别框
                  {/if}
                </button>
                <div class="h-3 w-px bg-border"></div>
                <button
                  class="text-destructive hover:text-destructive/80 transition-colors flex items-center gap-1 font-medium"
                  onclick={() => { ocrResults.set({}); }}
                >
                  <X size={13} /> 清除
                </button>
              </div>

              {#if ocrStatusMsg}
                <div class="pointer-events-auto text-xs px-3 py-1 rounded-full bg-primary/10 text-primary border border-primary/20 shadow-sm animate-in fade-in slide-in-from-top-1">
                  {ocrStatusMsg}
                </div>
              {/if}
            </div>
          {/if}

          <div
            class="mx-auto py-4 flex flex-col items-center gap-2 {$isDark ? 'bg-zinc-900' : ''}"
            style="max-width: {typeof containerMaxWidth === 'number' ? containerMaxWidth + 'px' : containerMaxWidth};"
          >
            {#each pageHeights as height, i (i)}
              {@const pageNum = i + 1}
              {@const isBuffered = pageNum >= virtualWindow.bufferedStart && pageNum <= virtualWindow.bufferedEnd}
              <div
                class="page-slot shrink-0 rounded-sm shadow-lg relative bg-card transition-shadow"
                style="width: {pageWidths[i]}px; height: {height}px;"
                data-page={pageNum}
              >
                {#if isBuffered}
                  <div
                    use:observePage={pageNum}
                    data-page={pageNum}
                    class="absolute inset-0 overflow-hidden"
                  >
                    <canvas class="block w-full h-full"></canvas>
                  </div>
                  <!-- Text layer for mouse selection -->
                  {#if pdfDoc}
                    <TextLayer
                      doc={pdfDoc}
                      pageNum={pageNum}
                      scale={effectiveScale}
                      pageHeight={basePageHeights[i] ?? 842}
                      ontextedit={handleTextLayerEdit}
                    />
                  {/if}
                  <!-- OCR results overlay -->
                  {#if $ocrResults[pageNum]?.length}
                    <OcrOverlay
                      blocks={$ocrResults[pageNum]}
                      pageNum={pageNum}
                      scale={effectiveScale}
                      pageHeight={basePageHeights[i] ?? 842}
                      visible={ocrOverlayVisible}
                    />
                  {/if}
                  <!-- CanvasEditor mounted for buffered pages -->
                  <CanvasEditor
                    activeTool={activeTool as any}
                    pageNum={pageNum}
                    pageWidth={basePageWidths[i] ?? 612}
                    pageHeight={height / effectiveScale}
                    scale={effectiveScale}
                    zIndex={activeTool ? 'z-[10]' : 'z-[5]'}
                  />
                {:else}
                  <!-- Virtualized lightweight skeleton placeholder: preserves geometry & scroll position with zero GPU burden -->
                  <div class="absolute inset-0 flex items-center justify-center bg-muted/10 border border-border/20 rounded-sm select-none pointer-events-none">
                    <span class="text-xs text-muted-foreground/40 font-mono tracking-wider">Page {pageNum}</span>
                  </div>
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      </div>
    {/if}

    <!-- Right property & layers panel (driven by selection or toggle) -->
    {#if selectedElement() || layersPanelOpen}
      {@const sel = selectedElement()}
      <div class="w-80 border-l border-border bg-card shrink-0 flex flex-col h-full overflow-hidden text-sm">
        <!-- Header -->
        <div class="flex items-center justify-between px-4 py-3 border-b border-border bg-muted/30">
          <div class="flex items-center gap-2">
            <Layers class="h-4 w-4 text-primary" />
            <span class="font-medium text-xs uppercase tracking-wider text-foreground">
              {sel ? "Element Properties" : "Page Layers"}
            </span>
          </div>
          <button
            onclick={() => {
              if (sel) clearSelection();
              else layersPanelOpen = false;
            }}
            class="text-xs text-muted-foreground hover:text-foreground p-1 rounded hover:bg-muted"
            title="Close"
          >✕</button>
        </div>

        <div class="flex-1 overflow-y-auto p-4 space-y-5">
          <!-- Selected Element Properties section (if an element is selected) -->
          {#if sel}
            <div class="space-y-3 pb-4 border-b border-border">
              <div class="flex items-center justify-between">
                <span class="text-xs font-semibold text-muted-foreground uppercase">
                  {#if sel.type === "addText"}Text Element
                  {:else if sel.type === "addRectangle"}Rectangle Element
                  {:else if sel.type === "addHighlight"}Highlight Element
                  {:else if sel.type === "addWhiteout"}Whiteout Element
                  {:else}Element Properties{/if}
                </span>
                <button
                  onclick={clearSelection}
                  class="text-[11px] text-muted-foreground hover:text-foreground"
                >Deselect</button>
              </div>

              {#if sel.type === "addText"}
                <div class="space-y-3">
                  <div class="space-y-1">
                    <label class="text-xs text-muted-foreground">Content</label>
                    <textarea
                      rows="2"
                      class="w-full p-2 rounded-lg border border-input bg-transparent text-sm resize-y"
                      value={sel.params.text ?? ""}
                      onchange={(e) => updateSelectedProperty("text", (e.target as HTMLTextAreaElement).value)}
                    ></textarea>
                  </div>
                  <div class="grid grid-cols-2 gap-2">
                    <div class="space-y-1">
                      <label class="text-xs text-muted-foreground">Font Size</label>
                      <input
                        type="number"
                        value={sel.params.fontSize ?? 16}
                        min="6" max="120"
                        class="w-full px-2 py-1 text-sm rounded border border-input bg-transparent"
                        onchange={(e) => updateSelectedProperty("fontSize", Number((e.target as HTMLInputElement).value))}
                      >
                    </div>
                    <div class="space-y-1">
                      <label class="text-xs text-muted-foreground">Color</label>
                      <input
                        type="color"
                        value={sel.params.color ?? "#000000"}
                        class="w-full h-8 rounded cursor-pointer border border-input"
                        onchange={(e) => updateSelectedProperty("color", (e.target as HTMLInputElement).value)}
                      >
                    </div>
                  </div>
                </div>
              {:else if sel.type === "addRectangle"}
                <div class="space-y-3">
                  <div class="grid grid-cols-2 gap-2">
                    <div class="space-y-1">
                      <label class="text-xs text-muted-foreground">Border Color</label>
                      <input
                        type="color"
                        value={sel.params.borderColor ?? "#000000"}
                        class="w-full h-8 rounded cursor-pointer border border-input"
                        onchange={(e) => updateSelectedProperty("borderColor", (e.target as HTMLInputElement).value)}
                      >
                    </div>
                    <div class="space-y-1">
                      <label class="text-xs text-muted-foreground">Border Width</label>
                      <input
                        type="number"
                        value={sel.params.borderWidth ?? 2}
                        min="1" max="20"
                        class="w-full px-2 py-1 text-sm rounded border border-input bg-transparent"
                        onchange={(e) => updateSelectedProperty("borderWidth", Number((e.target as HTMLInputElement).value))}
                      >
                    </div>
                  </div>
                  <label class="flex items-center gap-2 text-sm cursor-pointer">
                    <input
                      type="checkbox"
                      checked={sel.params.hasFill ?? false}
                      class="rounded"
                      onchange={(e) => updateSelectedProperty("hasFill", (e.target as HTMLInputElement).checked)}
                    >
                    <span class="text-xs text-muted-foreground">Fill</span>
                  </label>
                  {#if sel.params.hasFill}
                    <div class="space-y-1">
                      <label class="text-xs text-muted-foreground">Fill Color</label>
                      <input
                        type="color"
                        value={sel.params.fillColor ?? "#ffffff"}
                        class="w-full h-8 rounded cursor-pointer border border-input"
                        onchange={(e) => updateSelectedProperty("fillColor", (e.target as HTMLInputElement).value)}
                      >
                    </div>
                  {/if}
                </div>
              {:else if sel.type === "addHighlight"}
                <div class="space-y-3">
                  <div class="space-y-1">
                    <label class="text-xs text-muted-foreground">Color</label>
                    <input
                      type="color"
                      value={sel.params.color ?? "#ffff00"}
                      class="w-full h-8 rounded cursor-pointer border border-input"
                      onchange={(e) => updateSelectedProperty("color", (e.target as HTMLInputElement).value)}
                    >
                  </div>
                  <div class="space-y-1">
                    <label class="text-xs text-muted-foreground">Opacity: {Math.round((sel.params.opacity ?? 0.4) * 100)}%</label>
                    <input
                      type="range"
                      min="0.1" max="1" step="0.05"
                      value={sel.params.opacity ?? 0.4}
                      class="w-full"
                      oninput={(e) => updateSelectedProperty("opacity", Number((e.target as HTMLInputElement).value))}
                    >
                  </div>
                </div>
              {/if}

              <div class="pt-2">
                <button
                  onclick={deleteSelected}
                  class="text-xs text-destructive hover:underline flex items-center gap-1.5"
                >
                  <Trash class="h-3.5 w-3.5" />
                  Delete Selected Element
                </button>
              </div>
            </div>
          {/if}

          <!-- Scribus-style Page Elements / Layers List -->
          <div class="space-y-3">
            <div class="flex items-center justify-between">
              <span class="text-xs font-semibold text-muted-foreground uppercase">
                Page {$currentPage} Elements ({currentPageOperations().length})
              </span>
              {#if currentPageOperations().length > 0}
                <button
                  onclick={() => clearPageOperations($currentPage)}
                  class="text-[11px] text-destructive hover:underline"
                  title="Clear all annotations on current page"
                >
                  Clear Page
                </button>
              {/if}
            </div>

            {#if currentPageOperations().length === 0}
              <div class="py-6 text-center text-xs text-muted-foreground border border-dashed rounded-lg p-4">
                No annotations or modifications on Page {$currentPage}.
                <div class="mt-1 text-[11px] opacity-75">
                  Use Text, Shape, or Draw to add elements.
                </div>
              </div>
            {:else}
              <div class="space-y-1.5 max-h-[300px] overflow-y-auto pr-1">
                {#each currentPageOperations() as op (op.id)}
                  <div
                    class="group flex items-center justify-between p-2 rounded-lg border text-xs transition-colors cursor-pointer {op.id === $selectedEditId ? 'border-primary bg-primary/10' : 'border-border/60 hover:bg-muted/50'}"
                    onclick={() => selectedEditId.set(op.id)}
                  >
                    <div class="flex items-center gap-2 min-w-0 flex-1">
                      <span class="px-1.5 py-0.5 rounded font-mono text-[10px] font-semibold {op.type === 'addText' ? 'bg-blue-500/10 text-blue-500' : op.type === 'addRectangle' ? 'bg-green-500/10 text-green-500' : op.type === 'addHighlight' ? 'bg-yellow-500/10 text-yellow-600' : 'bg-muted text-muted-foreground'}">
                        {op.type === 'addText' ? 'T' : op.type === 'addRectangle' ? '▢' : op.type === 'addHighlight' ? 'H' : 'Op'}
                      </span>
                      <span class="truncate font-medium">
                        {#if op.type === 'addText'}
                          "{op.params.text || 'Text'}"
                        {:else if op.type === 'addRectangle'}
                          Rectangle
                        {:else if op.type === 'addHighlight'}
                          Highlight
                        {:else if op.type === 'addWhiteout'}
                          Whiteout
                        {:else}
                          {op.type}
                        {/if}
                      </span>
                    </div>

                    <button
                      onclick={(e) => {
                        e.stopPropagation();
                        deleteOperationById(op.id);
                      }}
                      class="opacity-0 group-hover:opacity-100 p-1 rounded hover:text-destructive hover:bg-destructive/10 transition-opacity"
                      title="Delete layer"
                    >
                      <Trash class="h-3.5 w-3.5" />
                    </button>
                  </div>
                {/each}
              </div>
            {/if}

            <!-- Document global summary -->
            {#if currentAppliedOperations().length > 0}
              <div class="pt-3 border-t border-border flex items-center justify-between text-[11px] text-muted-foreground">
                <span>Total applied: {currentAppliedOperations().length} in document</span>
                <button
                  onclick={clearAllOperations}
                  class="text-destructive hover:underline"
                >
                  Clear All
                </button>
              </div>
            {/if}
          </div>
        </div>
      </div>
    {/if}
  </div>

  <!-- Status bar -->
  {#if $currentFilePath && pageHeights.length > 0 && !loading && !errorMsg}
    <StatusBar
      onopensearch={() => (searchOpen = true)}
      onopenzen={() => (zenModeOpen = true)}
    />
  {/if}
</div>

<!-- Watermark dialog -->
{#if watermarkDialogOpen}
  <WatermarkDialog
    onclose={() => (watermarkDialogOpen = false)}
    onapply={(params) => { watermarkDialogOpen = false; applyWatermark(params); }}
  />
{/if}

<!-- OCR loading indicator -->
{#if ocrBusy}
  <div class="fixed inset-0 bg-black/20 flex items-center justify-center z-50 pointer-events-none">
    <div class="bg-card border border-border rounded-lg px-4 py-3 shadow-lg flex items-center gap-2">
      <Loader2 size={16} class="animate-spin text-primary" />
      <span class="text-sm">OCR recognizing...</span>
    </div>
  </div>
{/if}

<!-- Page tools dialog -->
{#if pageDialogTool}
  <PageToolsDialog
    tool={pageDialogTool}
    filePath={$currentFilePath}
    onclose={() => (pageDialogTool = null)}
    onfilechanged={(path) => {
      currentFilePath.set(path);
      currentFileName.set(path.split(/[\\/]/).pop() || "Untitled");
      pageDialogTool = null;
    }}
  />
{/if}

<!-- Print dialog -->
{#if printDialogOpen}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="fixed inset-0 bg-black/40 flex items-center justify-center z-50"
    onclick={(e) => { if (e.target === e.currentTarget) printDialogOpen = false; }}
  >
    <div class="bg-card border border-border rounded-xl shadow-xl w-[380px] flex flex-col">
      <div class="flex items-center justify-between px-5 py-3 border-b border-border shrink-0">
        <h2 class="text-sm font-medium">{t("print.title")}</h2>
        <button onclick={() => (printDialogOpen = false)} class="text-xs text-muted-foreground hover:text-foreground">X</button>
      </div>
      <div class="p-5 space-y-4">
        <!-- Page range -->
        <div class="space-y-2">
          <Label class="text-xs text-muted-foreground">{t("print.pageRange")}</Label>
          <div class="flex gap-1.5">
            <Button variant={printPageRange === "all" ? "default" : "outline"} size="sm" onclick={() => (printPageRange = "all")}>{t("print.allPages")}</Button>
            <Button variant={printPageRange === "current" ? "default" : "outline"} size="sm" onclick={() => (printPageRange = "current")}>{t("print.currentPage")}</Button>
            <Button variant={printPageRange === "custom" ? "default" : "outline"} size="sm" onclick={() => (printPageRange = "custom")}>{t("print.customRange")}</Button>
          </div>
          {#if printPageRange === "custom"}
            <Input
              value={printCustomRange}
              oninput={(e) => (printCustomRange = (e.target as HTMLInputElement).value)}
              placeholder="e.g. 1-3, 5, 7-10"
              class="mt-1"
            />
          {/if}
        </div>
        <!-- Orientation -->
        <div class="space-y-2">
          <Label class="text-xs text-muted-foreground">{t("print.orientation")}</Label>
          <div class="flex gap-1.5">
            <Button variant={printOrientation === "auto" ? "default" : "outline"} size="sm" onclick={() => (printOrientation = "auto")}>{t("print.auto")}</Button>
            <Button variant={printOrientation === "portrait" ? "default" : "outline"} size="sm" onclick={() => (printOrientation = "portrait")}>{t("print.portrait")}</Button>
            <Button variant={printOrientation === "landscape" ? "default" : "outline"} size="sm" onclick={() => (printOrientation = "landscape")}>{t("print.landscape")}</Button>
          </div>
        </div>
        <!-- Scale -->
        <div class="space-y-2">
          <Label class="text-xs text-muted-foreground">{t("print.scale")}: {Math.round(printScale * 100)}%</Label>
          <input
            type="range"
            min="0.5"
            max="2"
            step="0.1"
            bind:value={printScale}
            class="w-full"
          />
          <div class="flex justify-between text-[10px] text-muted-foreground">
            <span>50%</span><span>100%</span><span>200%</span>
          </div>
        </div>
        <!-- Print button -->
        <Button onclick={handlePrint} disabled={printBusy} class="w-full">
          {#if printBusy}
            <span class="flex items-center gap-1.5"><Loader2 size={14} class="animate-spin" />{t("print.printing")}</span>
          {:else}
            <span class="flex items-center gap-1.5"><Printer size={14} />{t("print.print")}</span>
          {/if}
        </Button>
      </div>
    </div>
  </div>
{/if}

<!-- Edit text overlay (for TextLayer Edit button) -->
{#if editTextOverlay.visible}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="fixed inset-0 z-[100]" onclick={() => { if (editTextOverlay.visible) commitEditText(); }}>
    <div
      class="absolute"
      style="left: {editTextScreenPos().x}px; top: {editTextScreenPos().y}px;"
      onclick={(e) => e.stopPropagation()}
    >
      <textarea
        bind:value={editTextValue}
        autofocus
        rows="1"
        class="block border-2 border-blue-500 bg-white text-black outline-none resize min-w-[100px] px-1 py-0.5 rounded shadow-lg"
        style="font-size: {editTextOverlay.fontSize * effectiveScale}px; line-height: 1.2; min-height: {editTextOverlay.originalHeight * effectiveScale}px;"
        onclick={(e) => e.stopPropagation()}
        onkeydown={(e) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); commitEditText(); } if (e.key === "Escape") { editTextOverlay.visible = false; } }}
      ></textarea>
    </div>
  </div>
{/if}

<!-- Global ⌘K Tool Finder Shortcut -->
<svelte:window onkeydown={(e) => {
  if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
    e.preventDefault();
    toolboxVisible = !toolboxVisible;
  }
}} />

<!-- Toolbox Modal -->
<ToolboxModal bind:visible={toolboxVisible} onselecttool={handleToolAction} />

{#if showUrlDialog}
  <!-- URL Input Dialog Modal -->
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm animate-in fade-in">
    <div class="w-full max-w-md p-6 bg-card border border-border rounded-2xl shadow-2xl space-y-4">
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-2 text-foreground font-semibold">
          <Globe size={18} class="text-primary" />
          <span>从网络网址打开 PDF</span>
        </div>
        <button
          onclick={() => (showUrlDialog = false)}
          class="p-1 rounded-md text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
        >
          <X size={16} />
        </button>
      </div>

      <p class="text-xs text-muted-foreground leading-relaxed">
        输入远程 PDF 文件的完整 HTTP 或 HTTPS 地址。下载后将在新标签页中打开，在编辑或批注后可随时保存至本地。
      </p>

      <div class="space-y-1.5">
        <input
          bind:value={urlInput}
          onkeydown={(e) => {
            if (e.key === 'Enter') handleOpenFromUrl();
            if (e.key === 'Escape') showUrlDialog = false;
          }}
          type="url"
          placeholder="https://example.com/sample.pdf"
          class="w-full px-3.5 py-2.5 bg-muted/40 border border-border rounded-xl text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-primary/40"
        />
        {#if urlError}
          <p class="text-xs text-destructive font-medium px-1 pt-1">{urlError}</p>
        {/if}
      </div>

      <div class="flex items-center justify-end gap-2.5 pt-2">
        <Button variant="ghost" size="sm" onclick={() => (showUrlDialog = false)}>
          取消
        </Button>
        <Button
          variant="default"
          size="sm"
          onclick={handleOpenFromUrl}
          disabled={urlLoading || !urlInput.trim()}
          class="gap-1.5"
        >
          {#if urlLoading}
            <Loader2 size={14} class="animate-spin" />
            <span>下载中...</span>
          {:else}
            <span>下载并打开</span>
          {/if}
        </Button>
      </div>
    </div>
  </div>
{/if}

{#if zenModeOpen && pdfDoc}
  <ZenReadingOverlay
    doc={pdfDoc}
    initialPage={$currentPage}
    fileName={$currentFileName}
    onclose={(page: number) => {
      zenModeOpen = false;
      if (page >= 1 && page <= $totalPages) {
        currentPage.set(page);
        scrollToPage(page);
      }
    }}
  />
{/if}

<PdfInfoDialog bind:open={pdfInfoOpen} />

{#if cropDialogOpen}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-xs p-4 animate-in fade-in duration-150 select-none"
    onclick={() => (cropDialogOpen = false)}
  >
    <div
      class="bg-card border border-border rounded-2xl shadow-2xl w-full max-w-2xl overflow-hidden"
      onclick={(e) => e.stopPropagation()}
    >
      <CropTool
        doc={pdfDoc}
        currentPage={$currentPage}
        totalPages={$totalPages}
        oncancel={() => (cropDialogOpen = false)}
        onapplycrop={({ pageRange, top, right, bottom, left }) => {
          cropDialogOpen = false;
        }}
      />
    </div>
  </div>
{/if}

{#if metadataDialogOpen}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-xs p-4 animate-in fade-in duration-150 select-none"
    onclick={() => (metadataDialogOpen = false)}
  >
    <div
      class="bg-card border border-border rounded-2xl shadow-2xl w-full max-w-lg overflow-hidden"
      onclick={(e) => e.stopPropagation()}
    >
      <MetadataTool
        initialMetadata={{
          title: $currentFileName.replace(/\.pdf$/i, ""),
          author: "",
          subject: "",
          keywords: "",
          creator: "PDF Seeker",
          producer: "lopdf / Rust",
        }}
        oncancel={() => (metadataDialogOpen = false)}
        onsave={(meta) => {
          metadataDialogOpen = false;
        }}
      />
    </div>
  </div>
{/if}

{#if showZenReminder && !zenReminderDismissed && !zenModeOpen && $currentFilePath}
  <aside
    aria-label="沉浸式阅读提示"
    class="fixed bottom-6 right-6 z-50 max-w-sm p-4 bg-card/95 backdrop-blur-md border border-primary/40 rounded-xl shadow-2xl flex flex-col gap-2.5 animate-in fade-in slide-in-from-bottom-3 duration-200 select-none"
  >
    <div class="flex items-start justify-between gap-3">
      <div class="flex items-center gap-2.5">
        <div class="w-8 h-8 rounded-lg bg-primary/15 text-primary flex items-center justify-center shrink-0">
          <BookOpen size={16} />
        </div>
        <div>
          <h4 class="text-xs font-bold text-foreground">开启沉浸阅读模式？</h4>
          <p class="text-[11px] text-muted-foreground leading-snug mt-0.5">
            检测到长文档翻阅。沉浸模式支持 3D 仿真翻页、双页开本、无干扰阅读与护眼背景。
          </p>
        </div>
      </div>
      <button
        onclick={() => dismissZenReminder(false)}
        class="text-muted-foreground hover:text-foreground p-1 rounded-md transition-colors"
        title="关闭本次提示"
      >
        <X size={13} />
      </button>
    </div>

    <div class="flex items-center justify-end gap-2 pt-2 border-t border-border/50">
      <button
        onclick={() => dismissZenReminder(true)}
        class="text-[11px] text-muted-foreground hover:text-foreground px-2 py-1 rounded transition-colors"
      >
        不再提醒
      </button>
      <button
        onclick={launchZenFromReminder}
        class="flex items-center gap-1.5 px-3 py-1 rounded-lg text-xs font-semibold bg-primary text-primary-foreground hover:bg-primary/90 transition-all shadow-xs"
      >
        <span>立即体验</span>
        <kbd class="px-1 py-0.2 text-[10px] font-mono bg-primary-foreground/20 rounded">Z</kbd>
      </button>
    </div>
  </aside>
{/if}

