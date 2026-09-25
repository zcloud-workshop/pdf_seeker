<script lang="ts">
  import { t } from "@/i18n/index.svelte.ts";
  import {
    currentPage,
    isDark,
    isFullscreen,
    outlineVisible,
    totalPages,
    zoomLevel,
    type Tab,
  } from "@/stores";
  import { Button, Tooltip } from "@/components/ui";
  import {
    ZoomIn,
    ZoomOut,
    ChevronLeft,
    ChevronRight,
    ChevronDown,
    FileText,
    Loader2,
    Maximize,
    Minimize,
    BookOpen,
    Pencil,
    Plus,
    Trash2,
    ArrowUp,
    ArrowDown,
    IndentIncrease,
    IndentDecrease,
    Check,
    X,
  } from "lucide-svelte";
  import { readFile, writeFile } from "@tauri-apps/plugin-fs";
  import { invoke } from "@tauri-apps/api/core";
  import {
    loadPdf,
    getPageViewport,
    type PdfDocumentProxy,
  } from "@/pdf-engine";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onDestroy } from "svelte";

  // ─── Props ──────────────────────────────────────────────────────────

  let {
    tab,
    active = false,
    initialPage,
  }: { tab: Tab; active?: boolean; initialPage?: number } = $props();

  /** Expose the scroll container so other components (e.g. PDF compare) can sync scrolling */
  export function getScrollContainer(): HTMLDivElement | undefined {
    return scrollContainer;
  }

  // ─── State ───────────────────────────────────────────────────────────

  let pdfDoc: PdfDocumentProxy | null = null;
  let loading = $state(false);
  let errorMsg = $state("");
  let controlsVisible = $state(true);
  let hideTimer: ReturnType<typeof setTimeout> | null = null;
  let scrollContainer: HTMLDivElement | undefined = $state(undefined);

  // Per-tab view state (survives tab switches)
  let zoom = $state(1.0);
  let pageNo = $state(1);
  let pageCount = $state(0);

  const PAGE_GAP = 8;
  const DPR = window.devicePixelRatio || 1;
  const MAX_CONCURRENT_RENDERS = 4;
  const MAX_CACHE_ENTRIES = 40;

  let pageHeights: number[] = $state([]);
  let pageWidths: number[] = $state([]);
  let canvasMap = new Map<number, HTMLCanvasElement>();
  let pageSlots = new Map<number, HTMLDivElement>();
  let pageCache = new Map<string, HTMLCanvasElement>();
  let observer: IntersectionObserver | null = null;
  let renderVersion = 0;
  let pendingRenders = 0;
  let renderQueue: number[] = [];
  let zoomRaf = 0;

  // Cached base dimensions at pdf scale=1
  let basePageHeights: number[] = [];
  let basePageWidths: number[] = [];
  let baseFitScale = 1;

  // ─── Helpers ─────────────────────────────────────────────────────────

  function cacheKey(pageNum: number, z: number) {
    return `${pageNum}@${z.toFixed(2)}`;
  }

  function cachePut(key: string, canvas: HTMLCanvasElement) {
    pageCache.set(key, canvas);
    while (pageCache.size > MAX_CACHE_ENTRIES) {
      const oldest = pageCache.keys().next().value;
      if (oldest === undefined) break;
      pageCache.delete(oldest);
    }
  }

  function pageContainerWidth(): number {
    if (!scrollContainer) return 800;
    return scrollContainer.clientWidth - 48;
  }

  // ─── Document loading ───────────────────────────────────────────────

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
    outlineNodes = [];
    editingOutline = false;
    try {
      const data = await readFile(path);
      const doc = await loadPdf(new Uint8Array(data));
      pdfDoc = doc;
      pageCount = doc.numPages;
      pageNo = 1;
      zoom = 1.0;
      await loadBaseDimensions(doc);
      applyZoomToDimensions(1.0);
      const outline = await readOutline(doc);
      outlineNodes = outline;
      savedOutlineJson = JSON.stringify(outline);
      outlineUndoSnapshot = null;
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

  function applyZoomToDimensions(z: number) {
    const s = z * baseFitScale;
    pageHeights = basePageHeights.map((h) => h * s);
    pageWidths = basePageWidths.map((w) => w * s);
  }

  // ─── Rendering ───────────────────────────────────────────────────────

  async function renderPage(pageNum: number) {
    const doc = pdfDoc;
    const canvas = canvasMap.get(pageNum);
    if (!doc || !canvas) return;

    const version = renderVersion;
    const z = zoom;

    try {
      const effectiveScale = z * baseFitScale;

      // Check cache
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
      if (renderVersion !== version) {
        pendingRenders--;
        processQueue();
        return;
      }

      const viewport = page.getViewport({ scale: effectiveScale });
      const cw = Math.floor(viewport.width * DPR);
      const ch = Math.floor(viewport.height * DPR);

      const offscreen = document.createElement("canvas");
      offscreen.width = cw;
      offscreen.height = ch;
      const offCtx = offscreen.getContext("2d")!;
      offCtx.setTransform(DPR, 0, 0, DPR, 0, 0);

      await page.render({ canvasContext: offCtx, viewport }).promise;

      if (renderVersion !== version) {
        pendingRenders--;
        processQueue();
        return;
      }

      cachePut(key, offscreen);

      // Draw to visible canvas
      const ctx = canvas.getContext("2d")!;
      canvas.width = cw;
      canvas.height = ch;
      ctx.drawImage(offscreen, 0, 0);

      pendingRenders--;
      processQueue();
    } catch {
      pendingRenders--;
      processQueue();
    }
  }

  function processQueue() {
    if (renderQueue.length === 0 || pendingRenders >= MAX_CONCURRENT_RENDERS)
      return;
    const pageNum = renderQueue.shift()!;
    renderPage(pageNum);
  }

  function renderVisiblePages() {
    if (!scrollContainer) return;
    const containerRect = scrollContainer.getBoundingClientRect();
    for (const [pageNum, slot] of pageSlots) {
      const rect = slot.getBoundingClientRect();
      if (
        rect.bottom > containerRect.top - 400 &&
        rect.top < containerRect.bottom + 400
      ) {
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
          const pageNum = Number(
            (entry.target as HTMLDivElement).dataset.page,
          );
          if (entry.isIntersecting) {
            renderPage(pageNum);
          }
        }
      },
      {
        root: scrollContainer || null,
        rootMargin: "400px 0px",
        threshold: 0,
      },
    );
  }

  // ─── Svelte action: observe each page slot ──────────────────────────

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

  // ─── Scroll → page tracking ─────────────────────────────────────────

  let scrollRaf = 0;

  function onScroll() {
    if (scrollRaf) cancelAnimationFrame(scrollRaf);
    scrollRaf = requestAnimationFrame(updateCurrentPage);
  }

  function updateCurrentPage() {
    if (!scrollContainer || pageHeights.length === 0) return;
    const top = scrollContainer.scrollTop;
    let acc = 0;
    for (let i = 0; i < pageHeights.length; i++) {
      acc += pageHeights[i] + PAGE_GAP;
      if (acc > top + 50) {
        if (pageNo !== i + 1) pageNo = i + 1;
        return;
      }
    }
    if (pageNo !== pageHeights.length) {
      pageNo = pageHeights.length;
    }
  }

  // ─── Navigation ─────────────────────────────────────────────────────

  function scrollToPage(n: number) {
    const slot = pageSlots.get(n);
    if (slot) slot.scrollIntoView({ behavior: "smooth", block: "start" });
  }

  function prevPage() {
    if (pageNo > 1) scrollToPage(pageNo - 1);
  }

  function nextPage() {
    if (pageNo < pageCount) scrollToPage(pageNo + 1);
  }

  // ─── Zoom ───────────────────────────────────────────────────────────
  // Wheel/pinch: continuous values, instant layout, rAF-debounced render
  // Buttons: snapped to 25% steps, instant everything

  function handleWheel(e: WheelEvent) {
    if (e.ctrlKey || e.metaKey) {
      e.preventDefault();
      e.stopPropagation();
      const delta = -e.deltaY * 0.002;
      const newZ = Math.max(0.25, Math.min(5, zoom + delta));
      zoom = newZ;
      applyZoomToDimensions(newZ);

      cancelAnimationFrame(zoomRaf);
      zoomRaf = requestAnimationFrame(() => {
        renderVersion++;
        pageCache.clear();
        renderVisiblePages();
      });
    }
  }

  function applyZoomButton(newZ: number) {
    zoom = newZ;
    applyZoomToDimensions(newZ);
    renderVersion++;
    pageCache.clear();
    renderVisiblePages();
  }

  function zoomIn() {
    applyZoomButton(Math.min(Math.round((zoom + 0.25) * 20) / 20, 5));
  }

  function zoomOut() {
    applyZoomButton(Math.max(Math.round((zoom - 0.25) * 20) / 20, 0.25));
  }

  function resetZoom() {
    applyZoomButton(1.0);
  }

  // ─── Outline (bookmarks) ────────────────────────────────────────────

  interface OutlineNode {
    id: number;
    title: string;
    pageNum: number | null;
    expanded: boolean;
    children: OutlineNode[];
  }

  let outlineNodes = $state<OutlineNode[]>([]);
  let editingOutline = $state(false);
  let selectedNodeId = $state<number | null>(null);
  let renamingId = $state<number | null>(null);
  let renameDraft = $state("");
  let newBookmarkTitle = $state("");
  let outlineSaving = $state(false);
  let outlineError = $state("");
  let outlineUndoSnapshot = $state<Uint8Array | null>(null);
  let savedOutlineJson = "[]";
  let outlineIdSeq = 1;

  async function readOutline(doc: PdfDocumentProxy): Promise<OutlineNode[]> {
    try {
      const raw = await doc.getOutline();
      if (!raw) return [];
      const map = async (
        items: { title: string; dest: unknown; items: unknown[] | null }[],
      ): Promise<OutlineNode[]> =>
        Promise.all(
          items.map(async (it) => ({
            id: outlineIdSeq++,
            title: it.title,
            pageNum: await resolveOutlinePage(doc, it.dest),
            expanded: true,
            children: await map((it.items || []) as never),
          })),
        );
      return await map(raw as never);
    } catch {
      return [];
    }
  }

  async function resolveOutlinePage(
    doc: PdfDocumentProxy,
    dest: unknown,
  ): Promise<number | null> {
    try {
      let d = dest;
      if (typeof d === "string") d = await doc.getDestination(d);
      if (!Array.isArray(d)) return null;
      const ref = d[0];
      if (!ref || typeof ref !== "object") return null;
      return (await doc.getPageIndex(ref as { num: number; gen: number })) + 1;
    } catch {
      return null;
    }
  }

  interface OutlineRow {
    node: OutlineNode;
    depth: number;
  }

  const outlineRows = $derived.by(() => {
    const rows: OutlineRow[] = [];
    const walk = (nodes: OutlineNode[], depth: number) => {
      for (const n of nodes) {
        rows.push({ node: n, depth });
        if (n.expanded && n.children.length > 0) walk(n.children, depth + 1);
      }
    };
    walk(outlineNodes, 0);
    return rows;
  });

  function locate(
    id: number,
  ): { list: OutlineNode[]; index: number; parent: OutlineNode | null } | null {
    const search = (
      list: OutlineNode[],
      parent: OutlineNode | null,
    ): { list: OutlineNode[]; index: number; parent: OutlineNode | null } | null => {
      const index = list.findIndex((n) => n.id === id);
      if (index >= 0) return { list, index, parent };
      for (const n of list) {
        const r = search(n.children, n);
        if (r) return r;
      }
      return null;
    };
    return search(outlineNodes, null);
  }

  function moveNode(id: number, dir: -1 | 1) {
    const loc = locate(id);
    if (!loc) return;
    const j = loc.index + dir;
    if (j < 0 || j >= loc.list.length) return;
    const tmp = loc.list[loc.index];
    loc.list[loc.index] = loc.list[j];
    loc.list[j] = tmp;
  }

  function promoteNode(id: number) {
    const loc = locate(id);
    if (!loc || !loc.parent) return;
    const parentLoc = locate(loc.parent.id);
    if (!parentLoc) return;
    const [node] = loc.list.splice(loc.index, 1);
    parentLoc.list.splice(parentLoc.index + 1, 0, node);
  }

  function demoteNode(id: number) {
    const loc = locate(id);
    if (!loc || loc.index === 0) return;
    const prev = loc.list[loc.index - 1];
    const [node] = loc.list.splice(loc.index, 1);
    prev.children.push(node);
    prev.expanded = true;
  }

  function deleteNode(id: number) {
    const loc = locate(id);
    if (loc) {
      loc.list.splice(loc.index, 1);
      if (selectedNodeId === id) selectedNodeId = null;
    }
  }

  function addBookmarkAtCurrentPage() {
    const title = newBookmarkTitle.trim();
    if (!title) return;
    const node: OutlineNode = {
      id: outlineIdSeq++,
      title,
      pageNum: pageNo,
      expanded: true,
      children: [],
    };
    if (selectedNodeId != null) {
      const loc = locate(selectedNodeId);
      if (loc) {
        loc.list[loc.index].expanded = true;
        loc.list[loc.index].children.push(node);
        newBookmarkTitle = "";
        return;
      }
    }
    outlineNodes.push(node);
    newBookmarkTitle = "";
  }

  function startRename(node: OutlineNode) {
    renamingId = node.id;
    renameDraft = node.title;
  }

  function commitRename() {
    if (renamingId != null) {
      const loc = locate(renamingId);
      if (loc && renameDraft.trim()) loc.list[loc.index].title = renameDraft.trim();
    }
    renamingId = null;
  }

  function cancelOutlineEdit() {
    outlineNodes = JSON.parse(savedOutlineJson);
    editingOutline = false;
    selectedNodeId = null;
    renamingId = null;
    outlineError = "";
  }

  function toInputItems(nodes: OutlineNode[]): unknown[] {
    // Nodes whose destination page cannot be resolved are dropped on write
    return nodes
      .filter((n) => n.pageNum != null)
      .map((n) => ({
        title: n.title,
        page: n.pageNum,
        children: toInputItems(n.children),
      }));
  }

  async function applyOutlineEdit() {
    outlineSaving = true;
    outlineError = "";
    try {
      const snapshot = await readFile(tab.path);
      const items = toInputItems(outlineNodes);
      await invoke("set_outline", {
        req: { inputPath: tab.path, outputPath: tab.path, items },
      });
      outlineUndoSnapshot = new Uint8Array(snapshot);
      editingOutline = false;
      selectedNodeId = null;
      renamingId = null;
      await loadDocument(tab.path);
    } catch (e) {
      outlineError = String(e);
    } finally {
      outlineSaving = false;
    }
  }

  async function undoOutlineEdit() {
    if (!outlineUndoSnapshot) return;
    const snap = outlineUndoSnapshot;
    outlineUndoSnapshot = null;
    try {
      await writeFile(tab.path, snap);
      await loadDocument(tab.path);
    } catch {
      outlineUndoSnapshot = snap;
    }
  }

  // ─── Fullscreen ─────────────────────────────────────────────────────

  async function toggleFullscreen() {
    const win = getCurrentWindow();
    const current = await win.isFullscreen();
    await win.setFullscreen(!current);
  }

  // ─── Keyboard shortcuts (active tab only) ───────────────────────────

  async function handleKeydown(e: KeyboardEvent) {
    if (!active) return;
    if (
      e.target instanceof HTMLInputElement ||
      e.target instanceof HTMLTextAreaElement
    )
      return;
    switch (e.key) {
      case "ArrowLeft":
      case "PageUp":
        e.preventDefault();
        prevPage();
        break;
      case "ArrowRight":
      case "PageDown":
        e.preventDefault();
        nextPage();
        break;
      case "+":
      case "=":
        if (e.ctrlKey || e.metaKey) {
          e.preventDefault();
          zoomIn();
        }
        break;
      case "-":
        if (e.ctrlKey || e.metaKey) {
          e.preventDefault();
          zoomOut();
        }
        break;
      case "0":
        if (e.ctrlKey || e.metaKey) {
          e.preventDefault();
          resetZoom();
        }
        break;
      case "z":
      case "Z":
        if (
          (e.ctrlKey || e.metaKey) &&
          !e.shiftKey &&
          outlineUndoSnapshot
        ) {
          e.preventDefault();
          undoOutlineEdit();
        }
        break;
      case "F11":
        e.preventDefault();
        toggleFullscreen();
        break;
      case "Escape":
        if (document.fullscreenElement || (await isTauriFullscreen())) {
          getCurrentWindow().setFullscreen(false);
        }
        break;
    }
  }

  async function isTauriFullscreen(): Promise<boolean> {
    try {
      return await getCurrentWindow().isFullscreen();
    } catch {
      return false;
    }
  }

  // ─── Auto-hide controls in fullscreen ───────────────────────────────

  function handleMouseMove() {
    controlsVisible = true;
    if (hideTimer) clearTimeout(hideTimer);
    hideTimer = setTimeout(async () => {
      try {
        const fs = await getCurrentWindow().isFullscreen();
        if (fs) controlsVisible = false;
      } catch {}
    }, 2500);
  }

  // ─── Effects ────────────────────────────────────────────────────────

  // Jump to initialPage once pages are laid out (used by PDF compare)
  $effect(() => {
    const target = initialPage;
    if (target == null) return;
    let rafId = 0;
    let tries = 0;
    const check = () => {
      if (pageHeights.length > 0) {
        rafId = requestAnimationFrame(() => scrollToPage(target));
      } else if (tries++ < 600) {
        rafId = requestAnimationFrame(check);
      }
    };
    rafId = requestAnimationFrame(check);
    return () => cancelAnimationFrame(rafId);
  });

  // Load once on mount — a tab's path never changes (openTab dedupes paths)
  $effect(() => {
    loadDocument(tab.path);
  });

  $effect(() => {
    if (scrollContainer) setupObserver();
    return () => {
      if (observer) {
        observer.disconnect();
        observer = null;
      }
    };
  });

  $effect(() => {
    const el = scrollContainer;
    if (!el) return;
    el.addEventListener("scroll", onScroll, { passive: true });
    return () => el.removeEventListener("scroll", onScroll);
  });

  // Push per-tab view state to the global stores for the active tab only
  $effect(() => {
    if (!active) return;
    zoomLevel.set(zoom);
    currentPage.set(pageNo);
    totalPages.set(pageCount);
  });

  // Re-render visible pages + restore globals when this tab becomes active
  $effect(() => {
    if (!active) return;
    const raf = requestAnimationFrame(() => renderVisiblePages());
    return () => cancelAnimationFrame(raf);
  });

  $effect(() => {
    if (!active) return;
    let live = true;
    const poll = setInterval(async () => {
      if (!live) return;
      try {
        const fs = await getCurrentWindow().isFullscreen();
        isFullscreen.set(fs);
      } catch {}
    }, 500);
    return () => {
      live = false;
      clearInterval(poll);
    };
  });

  $effect(() => {
    if (!active) return;
    window.addEventListener("keydown", handleKeydown);
    return () => window.removeEventListener("keydown", handleKeydown);
  });

  onDestroy(() => {
    if (hideTimer) clearTimeout(hideTimer);
    observer?.disconnect();
    canvasMap.clear();
    pageSlots.clear();
    pageCache.clear();
    renderQueue.length = 0;
    renderVersion++;
    pdfDoc?.destroy();
    pdfDoc = null;
  });

  // ─── Derived ────────────────────────────────────────────────────────

  const containerMaxWidth = $derived(
    pageWidths.length > 0 ? Math.max(...pageWidths) + 48 : "100%",
  );
</script>

<div
  class="flex h-full"
  onmousemove={handleMouseMove}
  role="application"
>
  {#if $outlineVisible}
    <aside class="w-64 shrink-0 border-r border-border bg-card flex flex-col overflow-hidden">
      <div class="flex items-center justify-between h-9 px-3 border-b border-border shrink-0">
        <span class="text-xs font-medium text-foreground">{t("viewer.outline")}</span>
        <div class="flex items-center gap-0.5">
          {#if !editingOutline}
            <button
              class="p-1 rounded hover:bg-accent text-muted-foreground hover:text-foreground"
              title={t("viewer.outlineEdit")}
              onclick={() => (editingOutline = true)}
            >
              <Pencil size={13} />
            </button>
          {:else}
            <button
              class="p-1 rounded hover:bg-accent text-muted-foreground hover:text-foreground disabled:opacity-50"
              title={t("viewer.outlineApply")}
              disabled={outlineSaving}
              onclick={applyOutlineEdit}
            >
              {#if outlineSaving}
                <Loader2 size={13} class="animate-spin" />
              {:else}
                <Check size={13} />
              {/if}
            </button>
            <button
              class="p-1 rounded hover:bg-accent text-muted-foreground hover:text-foreground"
              title={t("viewer.outlineCancel")}
              onclick={cancelOutlineEdit}
            >
              <X size={13} />
            </button>
          {/if}
          <button
            class="p-1 rounded hover:bg-accent text-muted-foreground hover:text-foreground"
            title={t("tabs.close")}
            onclick={() => outlineVisible.set(false)}
          >
            <X size={13} />
          </button>
        </div>
      </div>

      <div class="flex-1 overflow-auto py-1">
        {#if outlineRows.length === 0}
          <p class="text-xs text-muted-foreground px-3 py-2">{t("viewer.outlineEmpty")}</p>
        {:else}
          {#each outlineRows as row (row.node.id)}
            <div
              class="group flex items-center gap-0.5 pr-1 rounded-sm {selectedNodeId === row.node.id
                ? 'bg-accent'
                : 'hover:bg-accent/40'}"
              style="padding-left: {row.depth * 14 + 4}px"
            >
              {#if row.node.children.length > 0}
                <button
                  class="p-0.5 shrink-0 text-muted-foreground hover:text-foreground"
                  title={row.node.expanded ? t("viewer.outlineCollapse") : t("viewer.outlineExpand")}
                  onclick={() => (row.node.expanded = !row.node.expanded)}
                >
                  <ChevronDown
                    size={12}
                    class="transition-transform {row.node.expanded ? '' : '-rotate-90'}"
                  />
                </button>
              {:else}
                <span class="w-[18px] shrink-0"></span>
              {/if}

              {#if renamingId === row.node.id}
                <input
                  class="flex-1 min-w-0 h-6 px-1 rounded border border-input bg-background text-xs"
                  bind:value={renameDraft}
                  onkeydown={(e) => {
                    if (e.key === 'Enter') commitRename();
                    if (e.key === 'Escape') renamingId = null;
                  }}
                  onblur={commitRename}
                />
              {:else}
                <button
                  class="flex-1 min-w-0 text-left truncate text-xs {row.node.pageNum == null
                    ? 'text-muted-foreground/50 italic'
                    : 'text-foreground'}"
                  title={row.node.title}
                  onclick={() => {
                    if (editingOutline) {
                      selectedNodeId = row.node.id;
                    } else if (row.node.pageNum != null) {
                      scrollToPage(row.node.pageNum);
                    }
                  }}
                  ondblclick={() => editingOutline && startRename(row.node)}
                >
                  {row.node.title}
                </button>
                {#if row.node.pageNum != null}
                  <span class="text-[10px] text-muted-foreground/70 tabular-nums shrink-0">
                    {row.node.pageNum}
                  </span>
                {/if}
              {/if}

              {#if editingOutline && renamingId !== row.node.id}
                <span
                  class="flex items-center shrink-0 opacity-0 group-hover:opacity-100 transition-opacity"
                >
                  <button
                    class="p-0.5 text-muted-foreground hover:text-foreground disabled:opacity-30"
                    title={t("viewer.outlineMoveUp")}
                    onclick={() => moveNode(row.node.id, -1)}
                  >
                    <ArrowUp size={11} />
                  </button>
                  <button
                    class="p-0.5 text-muted-foreground hover:text-foreground disabled:opacity-30"
                    title={t("viewer.outlineMoveDown")}
                    onclick={() => moveNode(row.node.id, 1)}
                  >
                    <ArrowDown size={11} />
                  </button>
                  <button
                    class="p-0.5 text-muted-foreground hover:text-foreground"
                    title={t("viewer.outlinePromote")}
                    onclick={() => promoteNode(row.node.id)}
                  >
                    <IndentDecrease size={11} />
                  </button>
                  <button
                    class="p-0.5 text-muted-foreground hover:text-foreground"
                    title={t("viewer.outlineDemote")}
                    onclick={() => demoteNode(row.node.id)}
                  >
                    <IndentIncrease size={11} />
                  </button>
                  <button
                    class="p-0.5 text-muted-foreground hover:text-destructive"
                    title={t("viewer.outlineDelete")}
                    onclick={() => deleteNode(row.node.id)}
                  >
                    <Trash2 size={11} />
                  </button>
                </span>
              {/if}
            </div>
          {/each}
        {/if}
      </div>

      {#if editingOutline}
        <div class="border-t border-border p-2 space-y-1.5 shrink-0">
          <div class="flex gap-1">
            <input
              class="flex-1 min-w-0 h-7 px-2 rounded border border-input bg-background text-xs"
              placeholder={t("viewer.outlineAddTitle")}
              bind:value={newBookmarkTitle}
              onkeydown={(e) => e.key === 'Enter' && addBookmarkAtCurrentPage()}
            />
            <button
              class="shrink-0 px-1.5 h-7 rounded border border-input hover:bg-accent text-xs flex items-center gap-1"
              title={t("viewer.outlineAdd")}
              onclick={addBookmarkAtCurrentPage}
            >
              <Plus size={12} />
              {pageNo}
            </button>
          </div>
          {#if outlineError}
            <p class="text-[11px] text-destructive break-all">{outlineError}</p>
          {/if}
          {#if outlineUndoSnapshot}
            <p class="text-[10px] text-muted-foreground">{t("viewer.outlineUndoHint")}</p>
          {/if}
        </div>
      {/if}
    </aside>
  {/if}

  <div class="flex flex-col flex-1 min-w-0">
  <!-- PDF continuous scroll area -->
  <div
    bind:this={scrollContainer}
    class="flex-1 overflow-auto {$isDark
      ? 'bg-zinc-900'
      : 'bg-gray-100'} transition-colors duration-200"
    onwheel={handleWheel}
  >
    {#if loading}
      <div class="flex flex-col items-center justify-center h-full gap-3">
        <Loader2 size={32} class="text-muted-foreground animate-spin" />
        <span class="text-sm text-muted-foreground"
          >{t("app.loading")}</span
        >
      </div>
    {:else if errorMsg}
      <div class="flex flex-col items-center justify-center h-full gap-3">
        <FileText size={32} class="text-destructive/50" />
        <span class="text-sm text-destructive">{errorMsg}</span>
      </div>
    {:else if pageHeights.length > 0}
      <div
        class="mx-auto py-4 flex flex-col items-center gap-2"
        style="max-width: {typeof containerMaxWidth === 'number'
          ? containerMaxWidth + 'px'
          : containerMaxWidth}; filter: {$isDark
          ? 'invert(0.92) hue-rotate(180deg)'
          : 'none'};"
      >
        {#each pageHeights as height, i (i)}
          <div
            use:observePage={i + 1}
            data-page={i + 1}
            class="page-slot shrink-0 rounded-sm shadow-lg overflow-hidden"
            style="width: {pageWidths[i]}px; height: {height}px;"
          >
            <canvas class="block w-full h-full"></canvas>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  <!-- Bottom controls bar -->
  {#if !loading && !errorMsg && pageHeights.length > 0}
    <div
      class="flex items-center justify-center h-11 px-4 border-t border-border bg-card gap-1 shrink-0 transition-opacity duration-300 {controlsVisible
        ? 'opacity-100'
        : 'opacity-0 pointer-events-none'}"
    >
      <Tooltip message={t("viewer.outline")}>
        <Button
          variant="ghost"
          size="icon"
          class={$outlineVisible ? "text-foreground bg-accent" : ""}
          onclick={() => outlineVisible.update((v) => !v)}
        >
          <BookOpen size={16} />
        </Button>
      </Tooltip>

      <div class="w-px h-5 bg-border mx-1"></div>

      <Tooltip message="Previous Page (← / PgUp)">
        <Button
          variant="ghost"
          size="icon"
          onclick={prevPage}
          disabled={pageNo <= 1}
        >
          <ChevronLeft size={16} />
        </Button>
      </Tooltip>

      <span
        class="text-xs text-muted-foreground min-w-[90px] text-center tabular-nums"
      >
        {pageNo} / {pageCount}
      </span>

      <Tooltip message="Next Page (→ / PgDn)">
        <Button
          variant="ghost"
          size="icon"
          onclick={nextPage}
          disabled={pageNo >= pageCount}
        >
          <ChevronRight size={16} />
        </Button>
      </Tooltip>

      <div class="w-px h-5 bg-border mx-1"></div>

      <Tooltip message="Zoom Out (Ctrl+-)">
        <Button variant="ghost" size="icon" onclick={zoomOut}>
          <ZoomOut size={16} />
        </Button>
      </Tooltip>

      <button
        onclick={resetZoom}
        class="text-xs text-muted-foreground hover:text-foreground min-w-[48px] text-center tabular-nums transition-colors px-1"
        title="Reset Zoom (Ctrl+0)"
      >
        {Math.round(zoom * 100)}%
      </button>

      <Tooltip message="Zoom In (Ctrl++)">
        <Button variant="ghost" size="icon" onclick={zoomIn}>
          <ZoomIn size={16} />
        </Button>
      </Tooltip>

      <div class="flex-1"></div>

      <Tooltip
        message={$isFullscreen
          ? "Exit Fullscreen (F11)"
          : "Fullscreen (F11)"}
      >
        <Button variant="ghost" size="icon" onclick={toggleFullscreen}>
          {#if $isFullscreen}
            <Minimize size={16} />
          {:else}
            <Maximize size={16} />
          {/if}
        </Button>
      </Tooltip>
    </div>
  {/if}
  </div>
</div>
