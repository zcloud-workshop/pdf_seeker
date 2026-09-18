<script lang="ts">
  import type { PdfDocumentProxy } from "@/pdf-engine";
  import { currentPage, totalPages, isDark } from "@/stores";
  import { t } from "@/i18n/index.svelte.ts";

  let {
    doc,
    visible = $bindable(true),
    onpageclick,
    onreorder,
  }: {
    doc: PdfDocumentProxy | null;
    visible?: boolean;
    onpageclick?: (pageNum: number) => void;
    onreorder?: (fromPage: number, toPage: number) => void;
  } = $props();

  const THUMB_WIDTH = 120;
  const THUMB_GAP = 6;
  const VIRTUAL_THRESHOLD = 100;
  const THUMB_RENDER_HEIGHT = 170;
  const DPR = window.devicePixelRatio || 1;

  let containerEl: HTMLDivElement | undefined = $state(undefined);
  let thumbCanvases = new Map<number, HTMLCanvasElement>();
  let thumbSlots = new Map<number, HTMLDivElement>();
  let thumbCache = new Map<string, HTMLCanvasElement>();
  let thumbObserver: IntersectionObserver | null = null;
  let thumbRenderQueue: number[] = [];
  let thumbPendingRenders = 0;
  const THUMB_MAX_CONCURRENT = 3;
  let renderVersion = 0;

  let scrollTop = $state(0);
  let containerHeight = $state(400);
  let pageAspectRatios = $state<number[]>([]);

  // Drag state
  let dragFromPage = $state<number | null>(null);
  let dragOverPage = $state<number | null>(null);
  let dragOverPosition = $state<"before" | "after">("after");

  $effect(() => {
    if (!doc) {
      pageAspectRatios = [];
      thumbCache.clear();
      renderVersion++;
      return;
    }
    renderVersion++;
    thumbCache.clear();
    loadAspectRatios(doc);
  });

  async function loadAspectRatios(doc: PdfDocumentProxy) {
    const ratios: number[] = [];
    for (let i = 1; i <= doc.numPages; i++) {
      try {
        const page = await doc.getPage(i);
        const vp = page.getViewport({ scale: 1 });
        const rawRatio = vp.width / vp.height;
        ratios.push(Number.isFinite(rawRatio) && rawRatio > 0 ? Math.max(0.35, Math.min(rawRatio, 2.8)) : 0.707);
      } catch {
        ratios.push(0.707);
      }
    }
    pageAspectRatios = ratios;
  }

  function thumbHeight(pageIdx: number): number {
    const ratio = pageAspectRatios[pageIdx] ?? 0.707;
    const safeRatio = Number.isFinite(ratio) && ratio > 0 ? Math.max(0.35, Math.min(ratio, 2.8)) : 0.707;
    const h = Math.floor(THUMB_WIDTH / safeRatio);
    return Math.max(50, Math.min(h, 240));
  }

  function thumbTotalHeight(): number {
    let total = 0;
    for (let i = 0; i < pageAspectRatios.length; i++) {
      total += thumbHeight(i) + THUMB_GAP + 18;
    }
    return total;
  }

  function thumbOffsetY(pageIdx: number): number {
    let offset = 0;
    for (let i = 0; i < pageIdx; i++) {
      offset += thumbHeight(i) + THUMB_GAP + 18;
    }
    return offset;
  }

  let visibleRange = $derived.by(() => {
    const total = pageAspectRatios.length;
    if (total === 0 || total <= VIRTUAL_THRESHOLD) return { start: 0, end: total };
    let start = 0;
    let acc = 0;
    for (let i = 0; i < total; i++) {
      const h = thumbHeight(i) + THUMB_GAP + 18;
      if (acc + h >= scrollTop - THUMB_RENDER_HEIGHT) { start = i; break; }
      acc += h;
      if (i === total - 1) start = total;
    }
    let end = start;
    let accEnd = 0;
    for (let i = start; i < total; i++) {
      accEnd += thumbHeight(i) + THUMB_GAP + 18;
      end = i + 1;
      if (accEnd >= containerHeight + THUMB_RENDER_HEIGHT * 2) break;
    }
    return { start, end };
  });

  function onContainerScroll() {
    if (containerEl) {
      scrollTop = containerEl.scrollTop;
    }
  }

  async function renderThumbnail(pageNum: number) {
    const d = doc;
    const canvas = thumbCanvases.get(pageNum);
    if (!d || !canvas) return;

    const version = renderVersion;
    const key = `${pageNum}`;
    const cached = thumbCache.get(key);
    if (cached) {
      const ctx = canvas.getContext("2d")!;
      canvas.width = cached.width;
      canvas.height = cached.height;
      ctx.drawImage(cached, 0, 0);
      return;
    }

    if (thumbPendingRenders >= THUMB_MAX_CONCURRENT) {
      if (!thumbRenderQueue.includes(pageNum)) thumbRenderQueue.push(pageNum);
      return;
    }

    thumbPendingRenders++;
    try {
      const page = await d.getPage(pageNum);
      if (renderVersion !== version) { thumbPendingRenders--; processThumbQueue(); return; }
      const vp = page.getViewport({ scale: 1 });
      const scale = THUMB_WIDTH / vp.width;
      const renderVp = page.getViewport({ scale });
      const cw = Math.floor(renderVp.width * DPR);
      const ch = Math.floor(renderVp.height * DPR);

      const offscreen = document.createElement("canvas");
      offscreen.width = cw;
      offscreen.height = ch;
      const ctx = offscreen.getContext("2d")!;
      ctx.setTransform(DPR, 0, 0, DPR, 0, 0);
      await page.render({ canvasContext: ctx, viewport: renderVp }).promise;

      if (renderVersion !== version) { thumbPendingRenders--; processThumbQueue(); return; }

      thumbCache.set(key, offscreen);
      if (thumbCache.size > 200) {
        const keys = [...thumbCache.keys()];
        for (let i = 0; i < keys.length - 150; i++) thumbCache.delete(keys[i]);
      }

      const targetCtx = canvas.getContext("2d")!;
      canvas.width = cw;
      canvas.height = ch;
      targetCtx.drawImage(offscreen, 0, 0);

      if ($isDark) {
        targetCtx.fillStyle = "rgba(0, 0, 0, 0.15)";
        targetCtx.fillRect(0, 0, cw, ch);
      }
    } catch {
      // skip failed thumbnail
    } finally {
      thumbPendingRenders--;
      processThumbQueue();
    }
  }

  function processThumbQueue() {
    if (thumbRenderQueue.length === 0 || thumbPendingRenders >= THUMB_MAX_CONCURRENT) return;
    renderThumbnail(thumbRenderQueue.shift()!);
  }

  function setupThumbObserver() {
    if (thumbObserver) thumbObserver.disconnect();
    thumbObserver = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            const pageNum = Number((entry.target as HTMLDivElement).dataset.page);
            renderThumbnail(pageNum);
          }
        }
      },
      { root: containerEl || null, rootMargin: "200px 0px", threshold: 0 },
    );
  }

  function observeThumb(node: HTMLDivElement, pageNum: number) {
    const canvas = node.querySelector("canvas") as HTMLCanvasElement;
    if (canvas) thumbCanvases.set(pageNum, canvas);
    thumbSlots.set(pageNum, node);
    if (thumbObserver) thumbObserver.observe(node);
    // Render immediately if visible
    renderThumbnail(pageNum);
    return {
      update() {},
      destroy() {
        if (thumbObserver) thumbObserver.unobserve(node);
        thumbCanvases.delete(pageNum);
        thumbSlots.delete(pageNum);
      },
    };
  }

  $effect(() => {
    if (containerEl) {
      setupThumbObserver();
      containerHeight = containerEl.clientHeight;
      // Re-render all visible thumbs when doc changes
      for (const [pageNum, slot] of thumbSlots) {
        if (thumbObserver) thumbObserver.observe(slot);
        renderThumbnail(pageNum);
      }
    }
    return () => {
      if (thumbObserver) { thumbObserver.disconnect(); thumbObserver = null; }
    };
  });

  function onDragStart(e: DragEvent, pageNum: number) {
    dragFromPage = pageNum;
    if (e.dataTransfer) {
      e.dataTransfer.effectAllowed = "move";
      e.dataTransfer.setData("text/plain", String(pageNum));
    }
  }

  function onDragOver(e: DragEvent, pageNum: number) {
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
    dragOverPage = pageNum;
    const el = e.currentTarget as HTMLDivElement;
    const rect = el.getBoundingClientRect();
    const mid = rect.top + rect.height / 2;
    dragOverPosition = e.clientY < mid ? "before" : "after";
  }

  function onDragEnd() {
    dragFromPage = null;
    dragOverPage = null;
  }

  function onDrop(e: DragEvent, pageNum: number) {
    e.preventDefault();
    if (dragFromPage !== null && dragFromPage !== pageNum && onreorder) {
      onreorder(dragFromPage, pageNum);
    }
    dragFromPage = null;
    dragOverPage = null;
  }

  let isUserScrollingSidebar = $state(false);
  let sidebarScrollTimer: any = null;

  function onSidebarWheel() {
    isUserScrollingSidebar = true;
    if (sidebarScrollTimer) clearTimeout(sidebarScrollTimer);
    sidebarScrollTimer = setTimeout(() => {
      isUserScrollingSidebar = false;
    }, 1500);
  }

  let browsingPage = $derived.by(() => {
    if (pageAspectRatios.length === 0) return 1;
    const midY = scrollTop + containerHeight / 2;
    let acc = 0;
    for (let i = 0; i < pageAspectRatios.length; i++) {
      const h = thumbHeight(i) + THUMB_GAP + 18;
      if (midY >= acc && midY < acc + h) return i + 1;
      acc += h;
    }
    return pageAspectRatios.length;
  });

  function handleClick(pageNum: number) {
    isUserScrollingSidebar = false;
    currentPage.set(pageNum);
    if (onpageclick) onpageclick(pageNum);
  }

  function scrollToCurrentPage() {
    if (!containerEl || pageAspectRatios.length === 0) return;
    if (isUserScrollingSidebar) return;
    const page = $currentPage;
    if (page < 1 || page > pageAspectRatios.length) return;
    const targetY = thumbOffsetY(page - 1);
    const viewH = containerHeight;
    const itemH = thumbHeight(page - 1) + THUMB_GAP + 18;
    if (targetY < scrollTop || targetY + itemH > scrollTop + viewH) {
      containerEl.scrollTo({
        top: Math.max(0, targetY - viewH / 2 + itemH / 2),
        behavior: "smooth",
      });
    }
  }

  $effect(() => {
    void $currentPage;
    if (doc) scrollToCurrentPage();
  });

  function thumbClasses(isActive: boolean, isAbsolute = false): string {
    const ring = isActive ? "ring-1 ring-primary" : "ring-transparent";
    const pos = isAbsolute ? "absolute left-1.5 right-1.5" : "relative w-full mb-1";
    return `group ${pos} rounded cursor-pointer transition-all duration-150 outline-none focus:ring-1 focus:ring-primary/50 ${ring}`;
  }

  function labelClasses(isActive: boolean): string {
    const bg = isActive ? "bg-primary/20" : "";
    const text = isActive ? "text-primary" : "text-muted-foreground";
    return `absolute bottom-0.5 left-0 right-0 text-center ${bg} ${text}`;
  }
</script>

{#if visible && doc && pageAspectRatios.length > 0}
  <aside
    class="flex flex-col h-full border-r border-border bg-card shrink-0 select-none overflow-hidden"
    style="width: {THUMB_WIDTH + 24}px;"
    onwheel={onSidebarWheel}
  >
    <div class="flex items-center justify-between px-2.5 h-8 border-b border-border shrink-0">
      <span class="text-[10px] font-medium text-muted-foreground uppercase tracking-wider">
        {t("thumbnails.title")}
      </span>
      {#if isUserScrollingSidebar && browsingPage !== $currentPage}
        <span class="text-[10px] font-mono text-primary font-medium bg-primary/10 px-1.5 py-0.5 rounded">
          P.{browsingPage}
        </span>
      {:else}
        <span class="text-[10px] font-mono text-muted-foreground/70">
          {$currentPage}/{pageAspectRatios.length}
        </span>
      {/if}
    </div>
    <div
      bind:this={containerEl}
      class="flex-1 overflow-y-auto overflow-x-hidden py-1 px-1.5"
      onscroll={onContainerScroll}
    >
      {#if pageAspectRatios.length <= VIRTUAL_THRESHOLD}
        {#each pageAspectRatios as ratio, i (i + 1)}
          {@const pageNum = i + 1}
          {@const h = thumbHeight(i)}
          {@const isActive = $currentPage === pageNum}
          <button
            class={thumbClasses(isActive)}
            onclick={() => handleClick(pageNum)}
            draggable="true"
            ondragstart={(e) => onDragStart(e, pageNum)}
            ondragover={(e) => onDragOver(e, pageNum)}
            ondrop={(e) => onDrop(e, pageNum)}
            ondragend={onDragEnd}
            title="{t('viewer.page')} {pageNum}"
          >
            <div
              use:observeThumb={pageNum}
              data-page={pageNum}
              class="w-full rounded overflow-hidden shadow-sm border border-border/50 group-hover:border-primary/30 transition-colors"
              style="height: {h}px;"
            >
              <canvas class="block w-full h-full"></canvas>
            </div>
            <div class={labelClasses(isActive)}>
              <span class="text-[9px] font-medium px-1 py-0.5 rounded-sm">
                {pageNum}
              </span>
            </div>
            {#if dragOverPage === pageNum && dragFromPage !== null && dragFromPage !== pageNum}
              <div
                class="absolute left-0 right-0 z-10 bg-primary/60 h-0.5"
                style={dragOverPosition === "before" ? "top: -1px;" : "bottom: -1px;"}
              ></div>
            {/if}
          </button>
        {/each}
      {:else}
        <div style="height: {thumbTotalHeight()}px; position: relative;">
          {#each pageAspectRatios as ratio, i (i + 1)}
            {@const pageNum = i + 1}
            {@const h = thumbHeight(i)}
            {@const offset = thumbOffsetY(i)}
            {@const isActive = $currentPage === pageNum}
            {#if i >= visibleRange.start && i < visibleRange.end}
              <button
                class={thumbClasses(isActive, true)}
                style="top: {offset}px;"
                onclick={() => handleClick(pageNum)}
                draggable="true"
                ondragstart={(e) => onDragStart(e, pageNum)}
                ondragover={(e) => onDragOver(e, pageNum)}
                ondrop={(e) => onDrop(e, pageNum)}
                ondragend={onDragEnd}
                title="{t('viewer.page')} {pageNum}"
              >
                <div
                  use:observeThumb={pageNum}
                  data-page={pageNum}
                  class="w-full rounded overflow-hidden shadow-sm border border-border/50 group-hover:border-primary/30 transition-colors"
                  style="height: {h}px;"
                >
                  <canvas class="block w-full h-full"></canvas>
                </div>
                <div class={labelClasses(isActive)}>
                  <span class="text-[9px] font-medium px-1 py-0.5 rounded-sm">
                    {pageNum}
                  </span>
                </div>
                {#if dragOverPage === pageNum && dragFromPage !== null && dragFromPage !== pageNum}
                  <div
                    class="absolute left-0 right-0 z-10 bg-primary/60 h-0.5"
                    style={dragOverPosition === "before" ? "top: -1px;" : "bottom: -1px;"}
                  ></div>
                {/if}
              </button>
            {/if}
          {/each}
        </div>
      {/if}
    </div>
  </aside>
{/if}
