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
  } from "@/stores";
  import { Button, Tooltip, Input, Label } from "@/components/ui";
  import { FileText, Loader2, Save, X, SaveAll } from "lucide-svelte";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { readFile } from "@tauri-apps/plugin-fs";
  import { invoke } from "@tauri-apps/api/core";
  import { loadPdf, getPageViewport, type PdfDocumentProxy } from "@/pdf-engine";
  import ToolbarTabs from "@/components/editor/ToolbarTabs.svelte";
  import type { ToolCategory, ToolId } from "@/components/editor/ToolbarTabs.svelte";
  import StatusBar from "@/components/editor/StatusBar.svelte";
  import CanvasEditor from "@/components/editor/CanvasEditor.svelte";
  import type { EditOperation } from "@/edit-history";
  import { pushOperation } from "@/edit-history";
  import WatermarkDialog from "@/components/editor/WatermarkDialog.svelte";
  import PageToolsDialog from "@/components/editor/PageToolsDialog.svelte";
  import TextLayer from "@/components/editor/TextLayer.svelte";
  import OcrOverlay from "@/components/editor/OcrOverlay.svelte";
  import ThumbnailSidebar from "@/components/editor/ThumbnailSidebar.svelte";
  import { PanelLeftClose, PanelLeft, Printer } from "lucide-svelte";
  import { printPdf } from "@/print";
  import TabBar from "@/components/editor/TabBar.svelte";
  import { tabsStore } from "@/stores/tabs";

  // ─── Tool state ───────────────────────────────────────────────────────

  let activeCategory = $state<ToolCategory>("page");
  let activeTool = $state<ToolId | null>(null);
  let pageDialogTool = $state<ToolId | null>(null);
  let watermarkDialogOpen = $state(false);
  let thumbnailVisible = $state(true);
  let printDialogOpen = $state(false);
  let printBusy = $state(false);

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
    const hist = $editHistory;
    const newOps = hist.operations.filter((o: EditOperation) => o.id !== id);
    editHistory.set({ operations: newOps, currentIndex: newOps.length - 1 });
    selectedEditId.set(null);
  }

  // ─── Tool action handler ──────────────────────────────────────────────

  function handleToolAction(toolId: ToolId) {
    const pageTools: ToolId[] = ["merge", "split", "rotate", "delete", "extractPages", "reorder"];
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
      activeTool = activeTool === toolId ? null : toolId;
      clearSelection();
    }
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

  // ─── Add Text (from CanvasEditor textplace event) ────────────────────

  let textInput = $state<{ visible: boolean; x: number; y: number; pageNum: number; fontSize: number; color: string }>({
    visible: false, x: 0, y: 0, pageNum: 1, fontSize: 14, color: "#000000",
  });
  let textInputValue = $state("");

  function handleTextPlaceEvent(e: Event) {
    const detail = (e as CustomEvent).detail;
    if (!detail) return;
    textInput = { visible: true, x: detail.screenX, y: detail.screenY, pageNum: detail.pageNum, fontSize: 14, color: "#000000" };
    textInputValue = "";
  }

  function commitTextInput() {
    if (!textInputValue.trim()) { textInput.visible = false; return; }
    const pageIdx = textInput.pageNum - 1;
    const pH = basePageHeights[pageIdx] ?? 842;
    const s = $zoomLevel * baseFitScale;
    const pdfX = textInput.x / s;
    const pdfY = pH - textInput.y / s - (textInput.fontSize / s);
    const pdfW = textInputValue.length * textInput.fontSize * 0.6;
    const pdfH = textInput.fontSize;
    editHistory.set(
      pushOperation($editHistory, "addText", {
        page: textInput.pageNum, x: pdfX, y: pdfY, w: pdfW, h: pdfH,
        text: textInputValue, fontSize: textInput.fontSize, color: textInput.color,
      }),
    );
    textInput.visible = false;
  }

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

  // ─── Save edits ─────────────────────────────────────────────────────

  let saving = $state(false);

  const hasEdits = $derived(() => {
    const hist = $editHistory;
    return hist.currentIndex >= 0;
  });

  async function handleSaveEdits(overwrite: boolean = false) {
    if (!$currentFilePath || !hasEdits() || saving) return;
    saving = true;
    try {
      const hist = $editHistory;
      const applied = hist.operations.slice(0, hist.currentIndex + 1);
      const ops = applied.map((op: EditOperation) => ({
        opType: op.type,
        params: op.params,
      }));

      let outputPath: string;
      if (overwrite) {
        outputPath = $currentFilePath;
      } else {
        const out = await save({ filters: [{ name: "PDF", extensions: ["pdf"] }] });
        if (!out) { saving = false; return; }
        outputPath = out;
      }

      await invoke("apply_edit_operations", {
        inputPath: $currentFilePath,
        outputPath,
        operations: ops,
      });

      currentFilePath.set(outputPath);
      currentFileName.set(outputPath.split(/[\\/]/).pop() || "Untitled");
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

  async function handleOpen() {
    const selected = await open({
      multiple: false,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
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
    if (!file || !file.name.toLowerCase().endsWith(".pdf")) return;
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
    renderVersion++;
    pageCache.clear();
    pageHeights = [];
    pageWidths = [];
    basePageHeights = [];
    basePageWidths = [];
    try {
      const data = await readFile(path);
      const doc = await loadPdf(new Uint8Array(data));
      pdfDoc = doc;
      totalPages.set(doc.numPages);
      currentPage.set(1);
      zoomLevel.set(1.0);
      renderedZoomLevel = 1.0;
      await loadBaseDimensions(doc);
      applyZoomToDimensions(1.0);
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
      basePageHeights.push(vp.height);
      basePageWidths.push(vp.width);
    }
  }

  function applyZoomToDimensions(zoom: number) {
    const s = zoom * baseFitScale;
    pageHeights = basePageHeights.map((h) => h * s);
    pageWidths = basePageWidths.map((w) => w * s);
  }

  function evictCache() {
    if (pageCache.size <= MAX_CACHE_SIZE) return;
    const keys = [...pageCache.keys()];
    for (let i = 0; i < keys.length - MAX_CACHE_SIZE; i++) {
      pageCache.delete(keys[i]);
    }
  }

  // Tracks the zoom level at which canvases were last rendered.
  // CSS transform = currentZoom / renderedZoom provides instant visual scaling.
  let renderedZoomLevel = $state(1.0);
  const cssTransformScale = $derived($zoomLevel / renderedZoomLevel);

  // ─── Rendering ───────────────────────────────────────────────────────

  async function renderPage(pageNum: number) {
    const doc = pdfDoc;
    const canvas = canvasMap.get(pageNum);
    if (!doc || !canvas) return;

    const version = renderVersion;
    const z = $zoomLevel;

    try {
      const effectiveScale = z * baseFitScale;
      const key = cacheKey(pageNum, z);
      const cached = pageCache.get(key);
      if (cached) {
        const ctx = canvas.getContext("2d")!;
        canvas.width = cached.width;
        canvas.height = cached.height;
        ctx.drawImage(cached, 0, 0);
        return;
      }

      if (pendingRenders >= MAX_CONCURRENT_RENDERS) {
        if (!renderQueue.includes(pageNum)) renderQueue.push(pageNum);
        return;
      }

      pendingRenders++;
      const page = await doc.getPage(pageNum);
      if (renderVersion !== version) { pendingRenders--; processQueue(); return; }

      const viewport = page.getViewport({ scale: effectiveScale });
      const cw = Math.floor(viewport.width * DPR);
      const ch = Math.floor(viewport.height * DPR);

      const offscreen = document.createElement("canvas");
      offscreen.width = cw;
      offscreen.height = ch;
      const offCtx = offscreen.getContext("2d")!;
      offCtx.setTransform(DPR, 0, 0, DPR, 0, 0);
      await page.render({ canvasContext: offCtx, viewport }).promise;

      if (renderVersion !== version) { pendingRenders--; processQueue(); return; }

      pageCache.set(key, offscreen);
      evictCache();

      const ctx = canvas.getContext("2d")!;
      canvas.width = cw;
      canvas.height = ch;
      ctx.drawImage(offscreen, 0, 0);

      if ($isDark) {
        ctx.fillStyle = "rgba(0, 0, 0, 0.15)";
        ctx.fillRect(0, 0, cw, ch);
      }

      pendingRenders--;
      processQueue();
    } catch {
      pendingRenders--;
      processQueue();
    }
  }

  function processQueue() {
    if (renderQueue.length === 0 || pendingRenders >= MAX_CONCURRENT_RENDERS) return;
    renderPage(renderQueue.shift()!);
  }

  function renderVisiblePages() {
    if (!scrollContainer) return;
    const containerRect = scrollContainer.getBoundingClientRect();
    for (const [pageNum, slot] of pageSlots) {
      const rect = slot.getBoundingClientRect();
      if (rect.bottom > containerRect.top - 400 && rect.top < containerRect.bottom + 400) {
        renderPage(pageNum);
      }
    }
  }

  // ─── IntersectionObserver ───────────────────────────────────────────

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
    if (canvas) canvasMap.set(pageNum, canvas);
    pageSlots.set(pageNum, node);
    if (observer) observer.observe(node);
    return {
      update(newPageNum: number) {
        if (canvas) canvasMap.delete(pageNum);
        pageSlots.delete(pageNum);
        if (observer) observer.unobserve(node);
        pageNum = newPageNum;
        const c = node.querySelector("canvas") as HTMLCanvasElement;
        if (c) canvasMap.set(newPageNum, c);
        pageSlots.set(newPageNum, node);
        if (observer) observer.observe(node);
      },
      destroy() {
        if (observer) observer.unobserve(node);
        canvasMap.delete(pageNum);
        pageSlots.delete(pageNum);
      },
    };
  }

  // ─── Scroll → currentPage tracking ─────────────────────────────────

  let scrollRaf = 0;

  function onScroll() {
    if (scrollRaf) cancelAnimationFrame(scrollRaf);
    scrollRaf = requestAnimationFrame(updateCurrentPage);
  }

  function updateCurrentPage() {
    if (!scrollContainer || pageHeights.length === 0) return;
    const scrollTop = scrollContainer.scrollTop;
    let acc = 0;
    for (let i = 0; i < pageHeights.length; i++) {
      acc += pageHeights[i] + PAGE_GAP;
      if (acc > scrollTop + 50) {
        if ($currentPage !== i + 1) currentPage.set(i + 1);
        return;
      }
    }
    if ($currentPage !== pageHeights.length) currentPage.set(pageHeights.length);
  }

  // ─── Zoom with cursor-center (CSS-transform for smoothness) ─────────

  function handleWheel(e: WheelEvent) {
    if (e.ctrlKey || e.metaKey) {
      e.preventDefault();
      e.stopPropagation();
      const oldZ = $zoomLevel;
      const delta = -e.deltaY * 0.002;
      const newZ = Math.max(0.25, Math.min(5, oldZ + delta));
      if (newZ === oldZ) return;

      if (scrollContainer) {
        const rect = scrollContainer.getBoundingClientRect();
        const cursorX = e.clientX - rect.left + scrollContainer.scrollLeft;
        const cursorY = e.clientY - rect.top + scrollContainer.scrollTop;
        const ratio = newZ / oldZ;
        scrollContainer.scrollLeft = cursorX * ratio - (e.clientX - rect.left);
        scrollContainer.scrollTop = cursorY * ratio - (e.clientY - rect.top);
      }

      zoomLevel.set(newZ);
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
      case "+":
      case "=":
        if (e.ctrlKey || e.metaKey) {
          e.preventDefault();
          zoomLevel.set(Math.min(Math.round(($zoomLevel + 0.25) * 20) / 20, 5));
        }
        break;
      case "-":
        if (e.ctrlKey || e.metaKey) {
          e.preventDefault();
          zoomLevel.set(Math.max(Math.round(($zoomLevel - 0.25) * 20) / 20, 0.25));
        }
        break;
      case "0":
        if (e.ctrlKey || e.metaKey) {
          e.preventDefault();
          zoomLevel.set(1.0);
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
    // Debounce canvas re-render — CSS transform provides instant visual feedback
    if (zoomDebounceTimer) clearTimeout(zoomDebounceTimer);
    zoomDebounceTimer = setTimeout(() => {
      renderedZoomLevel = $zoomLevel;
      renderVersion++;
      renderVisiblePages();
      zoomDebounceTimer = null;
    }, 250);
  });

  // Listen for sign placement events from CanvasEditor
  $effect(() => {
    const el = scrollContainer;
    if (!el) return;
    el.addEventListener("signplace", handleSignPlaceEvent);
    return () => el.removeEventListener("signplace", handleSignPlaceEvent);
  });

  // Listen for text placement events from CanvasEditor
  $effect(() => {
    const el = scrollContainer;
    if (!el) return;
    el.addEventListener("textplace", handleTextPlaceEvent);
    return () => el.removeEventListener("textplace", handleTextPlaceEvent);
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
    ontabchange={handleTabChange}
    ontabclose={handleTabClose}
  />

  <!-- Toolbar tabs -->
  <div class="flex items-center">
    {#if $currentFilePath}
      <Tooltip message={thumbnailVisible ? "Hide Thumbnails" : "Show Thumbnails"}>
        <button
          onclick={() => (thumbnailVisible = !thumbnailVisible)}
          class="flex items-center justify-center w-8 h-8 rounded-md text-muted-foreground hover:bg-accent hover:text-accent-foreground transition-colors mr-1"
        >
          {#if thumbnailVisible}
            <PanelLeftClose size={14} />
          {:else}
            <PanelLeft size={14} />
          {/if}
        </button>
      </Tooltip>
    {/if}
    <ToolbarTabs {activeCategory} {activeTool} ontoolaction={handleToolAction} />
    {#if hasEdits() && $currentFilePath}
      <button
        onclick={() => handleSaveEdits(true)}
        disabled={saving}
        class="flex items-center gap-1.5 px-3 py-1.5 mx-2 rounded-md text-xs font-medium bg-primary text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-50 whitespace-nowrap"
      >
        {#if saving}
          <Loader2 size={13} class="animate-spin" />
        {:else}
          <Save size={13} />
        {/if}
        {t("editor.save")}
      </button>
      <button
        onclick={() => handleSaveEdits(false)}
        disabled={saving}
        class="flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium border border-border hover:bg-accent transition-colors disabled:opacity-50 whitespace-nowrap"
      >
        <SaveAll size={13} />
        {t("editor.saveAs")}
      </button>
    {/if}
    {#if $currentFilePath}
      <button
        onclick={handleCloseFile}
        class="flex items-center gap-1.5 px-2 py-1.5 rounded-md text-xs font-medium text-muted-foreground hover:bg-accent hover:text-destructive transition-colors whitespace-nowrap"
      >
        <X size={13} />
      </button>
    {/if}
    {#if $currentFilePath}
      <Tooltip message="Print">
        <button
          onclick={() => (printDialogOpen = true)}
          class="flex items-center gap-1.5 px-2 py-1.5 rounded-md text-xs font-medium text-muted-foreground hover:bg-accent hover:text-accent-foreground transition-colors whitespace-nowrap"
        >
          <Printer size={13} />
        </button>
      </Tooltip>
    {/if}
  </div>

  <!-- Main content area -->
  <div class="flex flex-1 min-h-0 overflow-hidden">
    <!-- Thumbnail sidebar -->
    {#if $currentFilePath}
      <ThumbnailSidebar
        doc={pdfDoc}
        bind:visible={thumbnailVisible}
        onpageclick={(pageNum: number) => {
          if (scrollContainer) {
            const targetSlot = scrollContainer.querySelector('.page-slot:nth-child(' + pageNum + ')');
            if (targetSlot) {
              targetSlot.scrollIntoView({ behavior: "smooth", block: "center" });
            }
          }
        }}
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
        class="flex-1 overflow-auto {$isDark ? 'bg-zinc-900' : 'bg-gray-100'} transition-colors duration-200"
        ondragover={(e) => e.preventDefault()}
        ondrop={handleDrop}
      >
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
          <div
            class="mx-auto py-4 flex flex-col items-center gap-2 {$isDark ? 'bg-zinc-900' : ''}"
            style="max-width: {typeof containerMaxWidth === 'number' ? containerMaxWidth + 'px' : containerMaxWidth};"
          >
            {#each pageHeights as height, i (i)}
              <div
                class="page-slot shrink-0 rounded-sm shadow-lg relative"
                style="width: {pageWidths[i]}px; height: {height}px;"
              >
                <div
                  use:observePage={i + 1}
                  data-page={i + 1}
                  class="absolute inset-0 overflow-hidden"
                  style="transform: scale({cssTransformScale}); transform-origin: top left;"
                >
                  <canvas class="block w-full h-full"></canvas>
                </div>
                <!-- Text layer for mouse selection -->
                {#if pdfDoc}
                  <TextLayer
                    doc={pdfDoc}
                    pageNum={i + 1}
                    scale={effectiveScale}
                    pageHeight={basePageHeights[i] ?? 842}
                    ontextedit={handleTextLayerEdit}
                  />
                {/if}
                <!-- OCR results overlay -->
                {#if $ocrResults[i + 1]?.length}
                  <OcrOverlay
                    blocks={$ocrResults[i + 1]}
                    pageNum={i + 1}
                    scale={effectiveScale}
                    pageHeight={basePageHeights[i] ?? 842}
                    visible={true}
                  />
                {/if}
                <!-- CanvasEditor always mounted -->
                <CanvasEditor
                  activeTool={activeTool as any}
                  pageNum={i + 1}
                  pageWidth={basePageWidths[i] ?? 612}
                  pageHeight={height / effectiveScale}
                  scale={effectiveScale}
                  zIndex={activeTool ? 'z-[10]' : 'z-[5]'}
                />
              </div>
            {/each}
          </div>
        {/if}
      </div>
    {/if}

    <!-- Right property panel (driven by selection, not tool) -->
    {#if showPropertyPanel && selectedElement()}
      {@const sel = selectedElement()!}
      <div class="w-72 border-l border-border bg-card shrink-0 overflow-auto p-4">
        <div class="flex items-center justify-between mb-3">
          <span class="text-sm font-medium">
            {#if sel.type === "addText"}Text
            {:else if sel.type === "addRectangle"}Rectangle
            {:else if sel.type === "addHighlight"}Highlight
            {:else}Element{/if}
          </span>
          <button
            onclick={clearSelection}
            class="text-xs text-muted-foreground hover:text-foreground"
          >X</button>
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

        <div class="mt-4 pt-3 border-t border-border">
          <button
            onclick={deleteSelected}
            class="text-xs text-destructive hover:underline"
          >Delete Element</button>
        </div>
      </div>
    {/if}
  </div>

  <!-- Status bar -->
  {#if $currentFilePath && pageHeights.length > 0 && !loading && !errorMsg}
    <StatusBar />
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

<!-- Text input overlay (for Add Text tool) -->
{#if textInput.visible}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="fixed inset-0 z-[100]" onclick={() => { if (textInput.visible) commitTextInput(); }}>
    <div
      class="absolute"
      style="left: {textInput.x}px; top: {textInput.y}px;"
      onclick={(e) => e.stopPropagation()}
    >
      <div class="flex items-center gap-2 mb-1 bg-white border border-gray-200 rounded-lg shadow-lg px-2 py-1">
        <input
          type="number"
          value={textInput.fontSize}
          min="6" max="120"
          class="w-14 px-1 py-0.5 text-xs rounded border border-input bg-transparent text-center"
          onchange={(e) => textInput.fontSize = Number((e.target as HTMLInputElement).value)}
        >
        <span class="text-[10px] text-muted-foreground">pt</span>
        <input
          type="color"
          value={textInput.color}
          class="w-6 h-6 rounded cursor-pointer border border-input"
          onchange={(e) => textInput.color = (e.target as HTMLInputElement).value}
        >
      </div>
      <textarea
        bind:value={textInputValue}
        autofocus
        rows="1"
        class="block border-2 border-blue-500 bg-white text-black outline-none resize min-w-[120px] px-1 py-0.5 rounded"
        style="font-size: {textInput.fontSize * effectiveScale}px; line-height: 1.2;"
        onclick={(e) => e.stopPropagation()}
        onkeydown={(e) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); commitTextInput(); } if (e.key === "Escape") { textInput.visible = false; } }}
      ></textarea>
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
