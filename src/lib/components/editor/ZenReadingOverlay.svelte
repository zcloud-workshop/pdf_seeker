<script lang="ts">
  import type { PdfDocumentProxy } from "@/pdf-engine";
  import { onMount, onDestroy } from "svelte";
  import {
    X,
    ChevronLeft,
    ChevronRight,
    Columns2,
    FileText,
    ArrowUpDown,
    ZoomIn,
    ZoomOut,
    Sparkles,
    Compass,
    Undo2,
    Hand,
  } from "lucide-svelte";
  import { Tooltip } from "@/components/ui";
  import { computeZoomLevel, computeScrollAnchor } from "@/utils/zoom-math";

  let {
    doc,
    initialPage = 1,
    fileName = "",
    onclose,
  }: {
    doc: PdfDocumentProxy | null;
    initialPage?: number;
    fileName?: string;
    onclose: (page: number) => void;
  } = $props();

  type LayoutMode = "facing" | "single" | "continuous";
  type ThemeMode = "parchment" | "paper" | "dark" | "default";

  let activePage = $state(0);
  $effect(() => {
    activePage = initialPage;
  });

  let layoutMode = $state<LayoutMode>("facing");
  let theme = $state<ThemeMode>("parchment");

  // ─── Instant-Response Two-Tier Zoom State ───
  let targetZoom = $state(1.0);
  let renderedZoom = $state(1.0);
  let livePinchFactor = $state(1.0);
  let isPinching = $state(false);

  const visualScale = $derived((targetZoom / renderedZoom) * livePinchFactor);

  let isZooming = $state(false);
  let zoomLockTimer: ReturnType<typeof setTimeout> | null = null;

  function markZooming() {
    isZooming = true;
    if (zoomLockTimer) clearTimeout(zoomLockTimer);
    zoomLockTimer = setTimeout(() => {
      isZooming = false;
    }, 300);
  }

  // ─── 3D Simulation Flip State ───
  let bookFlipEnabled = $state(true);
  let flipAnimation = $state<"none" | "next" | "prev">("none");
  let flipAnimTimer: any = null;
  let isFlipping = $state(false);

  // ─── Page Jump & History Back State ───
  let showJumpDialog = $state(false);
  let jumpInputVal = $state(1);
  let jumpHistory = $state<number[]>([]);

  let hudVisible = $state(true);
  let hudTimeout: any = null;

  // ─── Discrete Physical Gesture Tracking ("划一下是一页，抬起划一下是一页") ───
  let wheelAccumulator = 0;
  let gestureConsumed = false;
  let wheelIdleTimer: any = null;
  let zoomDebounceTimer: any = null;

  // ─── Mouse Hand / Drag-to-Pan (鼠标保持单击抓取改变页面位置) ───
  let isGrabbing = $state(false);
  let grabStartX = 0;
  let grabStartY = 0;
  let grabStartScrollTop = 0;
  let grabStartScrollLeft = 0;
  let panOffsetX = $state(0);
  let panOffsetY = $state(0);
  let grabStartPanX = 0;
  let grabStartPanY = 0;

  function resetPan() {
    panOffsetX = 0;
    panOffsetY = 0;
  }

  function handleGrabMouseDown(e: MouseEvent) {
    // Only primary mouse button (left button)
    if (e.button !== 0) return;
    // Do not intercept clicks on interactive buttons, inputs, links, or HUD controls
    if ((e.target as HTMLElement).closest("button, input, a, select, [data-zen-ctrl]")) return;

    isGrabbing = true;
    grabStartX = e.clientX;
    grabStartY = e.clientY;

    if (layoutMode === "continuous") {
      if (continuousContainer) {
        grabStartScrollTop = continuousContainer.scrollTop;
        grabStartScrollLeft = continuousContainer.scrollLeft;
      }
    } else {
      grabStartPanX = panOffsetX;
      grabStartPanY = panOffsetY;
    }
  }

  function handleGrabMouseMove(e: MouseEvent) {
    handleMouseMove(); // Keep HUD responsive

    if (!isGrabbing) return;

    e.preventDefault();
    const dx = e.clientX - grabStartX;
    const dy = e.clientY - grabStartY;

    if (layoutMode === "continuous") {
      if (continuousContainer) {
        continuousContainer.scrollTop = grabStartScrollTop - dy;
        continuousContainer.scrollLeft = grabStartScrollLeft - dx;
      }
    } else {
      panOffsetX = grabStartPanX + dx;
      panOffsetY = grabStartPanY + dy;
    }
  }

  function handleGrabMouseUp() {
    if (isGrabbing) {
      isGrabbing = false;
    }
  }

  // Canvases
  let canvasLeft: HTMLCanvasElement | undefined = $state(undefined);
  let canvasRight: HTMLCanvasElement | undefined = $state(undefined);
  let canvasSingleUnder: HTMLCanvasElement | undefined = $state(undefined);
  let continuousContainer: HTMLDivElement | undefined = $state(undefined);

  // In-memory pre-rendered bitmap cache for facing / single mode
  const pageCache = new Map<number, { canvas: HTMLCanvasElement; scale: number; seq: number }>();
  let renderSeq = 0;

  const total = $derived(doc?.numPages || 1);

  // Continuous mode state
  const continuousSlotMap = new Map<number, { slot: HTMLElement; canvas: HTMLCanvasElement; placeholder: HTMLElement }>();
  const renderedPagesInContinuous = new Set<number>();
  let continuousObserver: IntersectionObserver | null = null;
  let baseAspectRatio = $state(1.414);

  // Auto-hide HUD on mouse inactivity
  function handleMouseMove() {
    hudVisible = true;
    if (hudTimeout) clearTimeout(hudTimeout);
    hudTimeout = setTimeout(() => {
      if (!showJumpDialog) {
        hudVisible = false;
      }
    }, 2800);
  }

  // ─── Layout Switching ───

  function setLayoutMode(mode: LayoutMode) {
    resetPan();
    if (mode === "facing" && activePage % 2 === 0) {
      activePage = Math.max(1, activePage - 1);
    }
    layoutMode = mode;
    if (mode === "continuous") {
      setTimeout(() => {
        setupContinuousObserver();
        scrollToContinuousPage(activePage);
      }, 50);
    }
  }

  // ─── Realistic 3D Simulation / Clean 2D Flip (Single-Phase Only, No Secondary Animation) ───

  function executePageFlip(targetPage: number, direction: "next" | "prev") {
    if (activePage === targetPage) return;
    resetPan();
    if (flipAnimTimer) clearTimeout(flipAnimTimer);

    if (layoutMode === "single") {
      if (bookFlipEnabled) {
        isFlipping = true;
        flipAnimation = direction;

        // Pre-render the target page on the underlying canvas before starting peel animation
        if (canvasSingleUnder) {
          drawPageToCanvas(canvasSingleUnder, targetPage, renderSeq);
        }

        flipAnimTimer = setTimeout(() => {
          activePage = targetPage;
          if (canvasLeft) {
            drawPageToCanvas(canvasLeft, targetPage, renderSeq);
          }
          flipAnimation = "none";
          isFlipping = false;
        }, 360);
        return;
      } else {
        // Fast 2D flat slide
        activePage = targetPage;
        flipAnimation = direction;
        flipAnimTimer = setTimeout(() => {
          flipAnimation = "none";
        }, 130);
        return;
      }
    }

    // Facing mode 3D flip (Single clean motion without rebound second animation)
    if (bookFlipEnabled) {
      isFlipping = true;
      flipAnimation = direction;

      flipAnimTimer = setTimeout(() => {
        activePage = targetPage;
        flipAnimation = "none";
        isFlipping = false;
      }, 380);
    } else {
      activePage = targetPage;
      flipAnimation = direction;
      flipAnimTimer = setTimeout(() => {
        flipAnimation = "none";
      }, 130);
    }
  }

  function nextPage() {
    if (layoutMode === "continuous") {
      if (activePage < total) {
        scrollToContinuousPage(activePage + 1);
      }
      return;
    }

    const step = layoutMode === "facing" ? 2 : 1;
    if (activePage + step <= total || activePage < total) {
      const target = Math.min(total, activePage + step);
      executePageFlip(target, "next");
    }
  }

  function prevPage() {
    if (layoutMode === "continuous") {
      if (activePage > 1) {
        scrollToContinuousPage(activePage - 1);
      }
      return;
    }

    const step = layoutMode === "facing" ? 2 : 1;
    if (activePage - step >= 1 || activePage > 1) {
      const target = Math.max(1, activePage - step);
      executePageFlip(target, "prev");
    }
  }

  // ─── Page Jump & History Navigation ───

  function openJumpDialog() {
    jumpInputVal = activePage;
    showJumpDialog = true;
    hudVisible = true;
  }

  function executeJump(target: number) {
    const clamped = Math.max(1, Math.min(total, target));
    showJumpDialog = false;
    if (clamped === activePage) return;

    resetPan();
    jumpHistory = [...jumpHistory, activePage];

    if (layoutMode === "continuous") {
      scrollToContinuousPage(clamped);
    } else {
      if (layoutMode === "facing" && clamped % 2 === 0) {
        activePage = Math.max(1, clamped - 1);
      } else {
        activePage = clamped;
      }
      triggerCrispReRender();
    }
  }

  function handleJumpBack() {
    if (jumpHistory.length === 0) return;
    const prev = jumpHistory[jumpHistory.length - 1];
    jumpHistory = jumpHistory.slice(0, -1);

    resetPan();
    if (layoutMode === "continuous") {
      scrollToContinuousPage(prev);
    } else {
      activePage = prev;
      triggerCrispReRender();
    }
  }

  // ─── Discrete Gesture Tracking ("划一下是一页，抬起划一下是一页") ───

  function handleWheel(e: WheelEvent) {
    // 1. Pinch-to-zoom / Ctrl+Wheel
    if (e.ctrlKey || e.metaKey) {
      e.preventDefault();
      e.stopPropagation();

      // Continuous mode: Cursor-centered zoom identical to Editor.svelte
      if (layoutMode === "continuous" && continuousContainer) {
        markZooming();
        const oldZ = targetZoom;
        const newZ = computeZoomLevel(oldZ, e, { minZoom: 0.5, maxZoom: 3.0 });
        if (newZ !== oldZ) {
          const rect = continuousContainer.getBoundingClientRect();
          const { targetScrollLeft, targetScrollTop } = computeScrollAnchor(oldZ, newZ, {
            clientX: e.clientX,
            clientY: e.clientY,
            rectLeft: rect.left,
            rectTop: rect.top,
            scrollLeft: continuousContainer.scrollLeft,
            scrollTop: continuousContainer.scrollTop,
          });

          targetZoom = newZ;
          requestAnimationFrame(() => {
            if (continuousContainer) {
              continuousContainer.scrollLeft = Math.max(0, targetScrollLeft);
              continuousContainer.scrollTop = Math.max(0, targetScrollTop);
            }
          });

          if (zoomDebounceTimer) clearTimeout(zoomDebounceTimer);
          zoomDebounceTimer = setTimeout(() => {
            triggerCrispReRender();
          }, 160);
        }
        return;
      }

      // Single / Facing Mode: Smooth GPU Pinch
      isPinching = true;
      const delta = e.deltaY;
      const factor = Math.exp(-delta * 0.012);
      livePinchFactor = Math.min(3.0, Math.max(0.5, livePinchFactor * factor));

      if (zoomDebounceTimer) clearTimeout(zoomDebounceTimer);
      zoomDebounceTimer = setTimeout(() => {
        const nextTarget = Math.min(3.0, Math.max(0.5, +(targetZoom * livePinchFactor).toFixed(2)));
        targetZoom = nextTarget;
        livePinchFactor = 1.0;
        isPinching = false;
        triggerCrispReRender();
      }, 220);
      return;
    }

    // 2. Continuous Mode: Native 120Hz smooth scrolling
    if (layoutMode === "continuous") {
      return;
    }

    // 3. Single / Facing Mode: Discrete physical swipe
    e.preventDefault();

    if (gestureConsumed || isFlipping) {
      if (wheelIdleTimer) clearTimeout(wheelIdleTimer);
      wheelIdleTimer = setTimeout(() => {
        gestureConsumed = false;
        wheelAccumulator = 0;
      }, 190);
      return;
    }

    const dominantDelta = Math.abs(e.deltaY) >= Math.abs(e.deltaX) ? e.deltaY : e.deltaX;
    wheelAccumulator += dominantDelta;

    if (wheelIdleTimer) clearTimeout(wheelIdleTimer);
    wheelIdleTimer = setTimeout(() => {
      wheelAccumulator = 0;
      gestureConsumed = false;
    }, 190);

    const THRESHOLD = 35;
    if (wheelAccumulator > THRESHOLD) {
      wheelAccumulator = 0;
      gestureConsumed = true;
      nextPage();
    } else if (wheelAccumulator < -THRESHOLD) {
      wheelAccumulator = 0;
      gestureConsumed = true;
      prevPage();
    }
  }

  // ─── Zoom Controls ───

  function setZoom(newZoom: number) {
    const oldZ = targetZoom;
    const clamped = Math.min(3.0, Math.max(0.5, +newZoom.toFixed(2)));
    if (Math.abs(clamped - oldZ) < 0.005) return;

    if (layoutMode === "continuous" && continuousContainer) {
      markZooming();
      const rect = continuousContainer.getBoundingClientRect();
      const { targetScrollLeft, targetScrollTop } = computeScrollAnchor(oldZ, clamped, {
        clientX: rect.left + continuousContainer.clientWidth / 2,
        clientY: rect.top + continuousContainer.clientHeight / 2,
        rectLeft: rect.left,
        rectTop: rect.top,
        scrollLeft: continuousContainer.scrollLeft,
        scrollTop: continuousContainer.scrollTop,
      });

      targetZoom = clamped;
      requestAnimationFrame(() => {
        if (continuousContainer) {
          continuousContainer.scrollLeft = Math.max(0, targetScrollLeft);
          continuousContainer.scrollTop = Math.max(0, targetScrollTop);
        }
      });
    } else {
      targetZoom = clamped;
    }

    if (zoomDebounceTimer) clearTimeout(zoomDebounceTimer);
    zoomDebounceTimer = setTimeout(() => {
      triggerCrispReRender();
    }, 180);
  }

  function resetZoom() {
    resetPan();
    if (layoutMode === "continuous") {
      setZoom(1.0);
    } else {
      targetZoom = 1.0;
      livePinchFactor = 1.0;
      if (zoomDebounceTimer) clearTimeout(zoomDebounceTimer);
      triggerCrispReRender();
    }
  }

  function triggerCrispReRender() {
    renderedZoom = targetZoom;
    const currentSeq = ++renderSeq;
    if (layoutMode === "single" && canvasLeft && activePage <= total) {
      drawPageToCanvas(canvasLeft, activePage, currentSeq);
      if (canvasSingleUnder) {
        drawPageToCanvas(canvasSingleUnder, Math.min(total, activePage + 1), currentSeq);
      }
    } else if (layoutMode === "facing") {
      if (canvasLeft && activePage <= total) {
        drawPageToCanvas(canvasLeft, activePage, currentSeq);
      }
      if (canvasRight && activePage + 1 <= total) {
        drawPageToCanvas(canvasRight, activePage + 1, currentSeq);
      }
    } else if (layoutMode === "continuous") {
      renderedPagesInContinuous.clear();
      continuousSlotMap.forEach((_, p) => {
        if (Math.abs(p - activePage) <= 3) {
          renderContinuousPage(p);
        }
      });
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (showJumpDialog) {
      if (e.key === "Escape") {
        e.preventDefault();
        showJumpDialog = false;
      }
      return;
    }

    if (e.key === "Escape") {
      e.preventDefault();
      onclose(activePage);
    } else if (e.key === "g" || e.key === "G") {
      e.preventDefault();
      openJumpDialog();
    } else if ((e.key === "b" || e.key === "B") && jumpHistory.length > 0) {
      e.preventDefault();
      handleJumpBack();
    } else if (e.key === "ArrowRight" || e.key === "PageDown" || (e.key === " " && !e.shiftKey)) {
      e.preventDefault();
      nextPage();
    } else if (e.key === "ArrowLeft" || e.key === "PageUp" || (e.key === " " && e.shiftKey)) {
      e.preventDefault();
      prevPage();
    } else if (e.key === "Home") {
      e.preventDefault();
      if (layoutMode === "continuous") scrollToContinuousPage(1);
      else {
        activePage = 1;
        triggerCrispReRender();
      }
    } else if (e.key === "End") {
      e.preventDefault();
      if (layoutMode === "continuous") scrollToContinuousPage(total);
      else {
        activePage = layoutMode === "facing" && total % 2 === 0 ? total - 1 : total;
        triggerCrispReRender();
      }
    } else if (e.key === "+" || e.key === "=") {
      setZoom(targetZoom + 0.15);
    } else if (e.key === "-") {
      setZoom(targetZoom - 0.15);
    } else if (e.key === "0") {
      resetZoom();
    }
  }

  // ─── Offscreen Canvas Render & Cache ───

  async function getRenderedPageCanvas(pageNum: number, fitScale: number, currentSeq: number): Promise<HTMLCanvasElement | null> {
    if (!doc || pageNum < 1 || pageNum > total) return null;

    const cached = pageCache.get(pageNum);
    if (cached && Math.abs(cached.scale - fitScale) < 0.02 && cached.seq === currentSeq) {
      return cached.canvas;
    }

    try {
      const page = await doc.getPage(pageNum);
      if (currentSeq !== renderSeq) return null;

      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      const vp = page.getViewport({ scale: fitScale });

      if (pageNum === 1 && vp.width > 0) {
        baseAspectRatio = vp.height / vp.width;
      }

      const offscreen = document.createElement("canvas");
      offscreen.width = Math.floor(vp.width * dpr);
      offscreen.height = Math.floor(vp.height * dpr);
      offscreen.style.width = `${Math.floor(vp.width)}px`;
      offscreen.style.height = `${Math.floor(vp.height)}px`;

      const ctx = offscreen.getContext("2d");
      if (!ctx) return null;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

      await page.render({
        canvasContext: ctx,
        viewport: vp,
      }).promise;

      pageCache.set(pageNum, { canvas: offscreen, scale: fitScale, seq: currentSeq });

      if (pageCache.size > 14) {
        const oldestKey = pageCache.keys().next().value;
        if (oldestKey !== undefined) pageCache.delete(oldestKey);
      }

      return offscreen;
    } catch {
      return null;
    }
  }

  async function drawPageToCanvas(targetCanvas: HTMLCanvasElement, pageNum: number, currentSeq: number) {
    if (!doc || pageNum < 1 || pageNum > total) return;
    try {
      const page = await doc.getPage(pageNum);
      const baseVp = page.getViewport({ scale: 1.0 });

      const availableH = window.innerHeight - 100;
      const availableW = layoutMode === "facing" ? (window.innerWidth - 90) / 2 : window.innerWidth - 90;
      const fitScale = Math.min(availableH / baseVp.height, availableW / baseVp.width) * renderedZoom;

      const cached = await getRenderedPageCanvas(pageNum, fitScale, currentSeq);
      if (!cached || currentSeq !== renderSeq) return;

      targetCanvas.width = cached.width;
      targetCanvas.height = cached.height;
      targetCanvas.style.width = cached.style.width;
      targetCanvas.style.height = cached.style.height;

      const ctx = targetCanvas.getContext("2d");
      if (!ctx) return;
      ctx.drawImage(cached, 0, 0);

      preRenderLookahead(pageNum, fitScale, currentSeq);
    } catch {}
  }

  function preRenderLookahead(centerPage: number, fitScale: number, currentSeq: number) {
    const lookaheads = [centerPage + 1, centerPage + 2, centerPage - 1, centerPage - 2];
    for (const p of lookaheads) {
      if (p >= 1 && p <= total && !pageCache.has(p)) {
        getRenderedPageCanvas(p, fitScale, currentSeq);
      }
    }
  }

  // ─── Robust Continuous Mode Rendering (Completely Independent of renderSeq) ───

  function initContinuousSlot(node: HTMLDivElement, pageNum: number) {
    const canvas = node.querySelector("canvas");
    const placeholder = node.querySelector("[data-zen-placeholder]") as HTMLElement;
    if (!canvas) return;

    continuousSlotMap.set(pageNum, { slot: node, canvas, placeholder });

    if (continuousObserver) {
      continuousObserver.observe(node);
    }

    // Render initially visible pages immediately on mount
    if (Math.abs(pageNum - activePage) <= 3) {
      renderContinuousPage(pageNum);
    }

    return {
      destroy() {
        continuousSlotMap.delete(pageNum);
        if (continuousObserver) {
          continuousObserver.unobserve(node);
        }
      },
    };
  }

  let continuousScrollRaf: number | null = null;
  function handleContinuousScroll() {
    if (isZooming || layoutMode !== "continuous" || !continuousContainer) return;
    if (continuousScrollRaf) cancelAnimationFrame(continuousScrollRaf);
    continuousScrollRaf = requestAnimationFrame(() => {
      if (isZooming || !continuousContainer) return;
      const midY = continuousContainer.scrollTop + continuousContainer.clientHeight / 2;
      let closestPage = activePage;
      let minDistance = Infinity;

      continuousSlotMap.forEach(({ slot }, pageNum) => {
        const top = slot.offsetTop;
        const h = slot.offsetHeight;
        const center = top + h / 2;
        const dist = Math.abs(midY - center);
        if (dist < minDistance) {
          minDistance = dist;
          closestPage = pageNum;
        }
      });

      if (closestPage !== activePage && closestPage >= 1 && closestPage <= total) {
        activePage = closestPage;
      }
    });
  }

  function setupContinuousObserver() {
    if (continuousObserver) {
      continuousObserver.disconnect();
      continuousObserver = null;
    }
    if (!continuousContainer || layoutMode !== "continuous") return;

    continuousObserver = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          const pageNum = Number(entry.target.getAttribute("data-zen-slot"));
          if (!pageNum) return;
          if (entry.isIntersecting) {
            renderContinuousPage(pageNum);
          }
        });
      },
      {
        root: continuousContainer,
        rootMargin: "800px 0px", // 800px buffer to pre-load before scrolling into view
        threshold: 0,
      }
    );

    continuousSlotMap.forEach(({ slot }) => {
      continuousObserver?.observe(slot);
    });
  }

  async function renderContinuousPage(pageNum: number) {
    if (!doc || pageNum < 1 || pageNum > total) return;
    const item = continuousSlotMap.get(pageNum);
    if (!item) return;

    if (renderedPagesInContinuous.has(pageNum)) return;
    renderedPagesInContinuous.add(pageNum);

    try {
      const page = await doc.getPage(pageNum);
      const baseVp = page.getViewport({ scale: 1.0 });
      if (pageNum === 1 && baseVp.width > 0) {
        baseAspectRatio = baseVp.height / baseVp.width;
      }

      const availableW = Math.min(window.innerWidth - 120, 920) * targetZoom;
      const fitScale = availableW / baseVp.width;
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      const vp = page.getViewport({ scale: fitScale });

      item.canvas.width = Math.floor(vp.width * dpr);
      item.canvas.height = Math.floor(vp.height * dpr);
      item.canvas.style.width = `${Math.floor(vp.width)}px`;
      item.canvas.style.height = `${Math.floor(vp.height)}px`;

      const ctx = item.canvas.getContext("2d");
      if (!ctx) return;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

      await page.render({
        canvasContext: ctx,
        viewport: vp,
      }).promise;

      if (item.placeholder) {
        item.placeholder.style.display = "none";
      }
    } catch {
      renderedPagesInContinuous.delete(pageNum);
    }
  }

  function scrollToContinuousPage(pageNum: number) {
    if (!continuousContainer) return;
    const targetSlot = continuousContainer.querySelector(`[data-zen-slot="${pageNum}"]`);
    if (targetSlot) {
      targetSlot.scrollIntoView({ behavior: "smooth", block: "start" });
    }
    activePage = pageNum;
  }

  // ─── Reactive Lifecycle Effects ───

  $effect(() => {
    const currentSeq = ++renderSeq;
    void activePage;
    void layoutMode;
    void renderedZoom;

    if (layoutMode === "single" && canvasLeft && activePage <= total) {
      drawPageToCanvas(canvasLeft, activePage, currentSeq);
      if (canvasSingleUnder) {
        drawPageToCanvas(canvasSingleUnder, Math.min(total, activePage + 1), currentSeq);
      }
    } else if (layoutMode === "facing") {
      if (canvasLeft && activePage <= total) {
        drawPageToCanvas(canvasLeft, activePage, currentSeq);
      }
      if (canvasRight && activePage + 1 <= total) {
        drawPageToCanvas(canvasRight, activePage + 1, currentSeq);
      }
    }
  });

  onDestroy(() => {
    if (hudTimeout) clearTimeout(hudTimeout);
    if (wheelIdleTimer) clearTimeout(wheelIdleTimer);
    if (flipAnimTimer) clearTimeout(flipAnimTimer);
    if (zoomDebounceTimer) clearTimeout(zoomDebounceTimer);
    if (continuousObserver) continuousObserver.disconnect();
    pageCache.clear();
    continuousSlotMap.clear();
    renderedPagesInContinuous.clear();
  });

  const themeBgClasses = $derived(() => {
    switch (theme) {
      case "parchment":
        return "bg-[#f4ebd0] text-[#3d2e1a]";
      case "paper":
        return "bg-[#d8ebd4] text-[#1b381e]";
      case "dark":
        return "bg-[#141416] text-[#e0e0e0]";
      default:
        return "bg-[#f2f2f4] text-neutral-900";
    }
  });

  const continuousSlotWidth = $derived(Math.min(window.innerWidth - 120, 920) * targetZoom);
  const continuousSlotHeight = $derived(continuousSlotWidth * baseAspectRatio);
</script>

<svelte:window
  onkeydown={handleKeydown}
  onmousemove={handleGrabMouseMove}
  onmouseup={handleGrabMouseUp}
  onmouseleave={handleGrabMouseUp}
/>

<!-- Fullscreen Zen Mode Container -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="fixed inset-0 z-50 flex flex-col select-none {themeBgClasses()} transition-colors duration-300 overflow-hidden"
  onmousemove={handleMouseMove}
  onwheel={handleWheel}
>
  <!-- High-Contrast Top-Right Quick Exit Pill -->
  <button
    onclick={() => onclose(activePage)}
    class="fixed top-4 right-5 z-[70] flex items-center gap-1.5 px-3.5 py-1.5 rounded-full bg-neutral-900/90 hover:bg-rose-600 text-white text-xs font-semibold shadow-2xl border border-white/20 backdrop-blur-md transition-all duration-200 hover:scale-105 active:scale-95 select-none cursor-pointer"
    title="退出沉浸阅读 (快捷键: Esc)"
  >
    <X size={14} class="stroke-[2.5]" />
    <span>退出沉浸</span>
    <kbd class="ml-0.5 px-1 py-0.2 rounded text-[10px] bg-white/20 font-mono">Esc</kbd>
  </button>

  <!-- Top Floating HUD -->
  <div
    class="absolute top-0 left-0 right-32 z-50 flex items-center justify-between px-6 py-3 transition-all duration-300 pointer-events-none {hudVisible
      ? 'opacity-100 translate-y-0'
      : 'opacity-0 -translate-y-4'}"
  >
    <!-- Left: Document Title, Interactive Jump Pill, and History Return -->
    <div class="flex items-center gap-2.5 pointer-events-auto bg-card/95 backdrop-blur-md px-4 py-2 rounded-2xl border border-border/80 shadow-xl text-foreground">
      <span class="text-xs font-bold text-primary px-2 py-0.5 rounded-full bg-primary/10 tracking-wide uppercase">
        Zen Reading
      </span>
      <span class="text-xs font-semibold truncate max-w-xs">{fileName || "沉浸式阅读"}</span>

      <!-- Interactive Jump to Page Trigger -->
      <Tooltip message="点击或按 G 快速跳转指定页数">
        <button
          onclick={openJumpDialog}
          class="flex items-center gap-1 px-2.5 py-0.5 rounded-lg bg-accent/60 hover:bg-primary/15 hover:text-primary border border-border/60 transition-colors text-[11px] font-mono cursor-pointer"
        >
          <Compass size={12} class="text-primary" />
          <span>{activePage}{layoutMode === "facing" && activePage + 1 <= total ? `-${activePage + 1}` : ""} / {total}</span>
        </button>
      </Tooltip>

      <!-- One-Click Return to Previous Page (一键返回上一页) -->
      {#if jumpHistory.length > 0}
        <button
          onclick={handleJumpBack}
          class="flex items-center gap-1 px-2.5 py-0.5 rounded-lg bg-amber-500/20 hover:bg-amber-500/35 border border-amber-500/40 text-amber-900 dark:text-amber-200 text-[11px] font-semibold transition-all cursor-pointer animate-in fade-in"
          title="返回跳转前位置 (快捷键: B)"
        >
          <Undo2 size={12} />
          <span>返回 P.{jumpHistory[jumpHistory.length - 1]}</span>
          <kbd class="ml-0.5 px-1 py-0.2 rounded text-[9px] bg-black/10 dark:bg-white/10 font-mono">B</kbd>
        </button>
      {/if}
    </div>

    <!-- Center: Visual Controls (Layout Mode, 3D Flip, Zoom, Themes) -->
    <div class="flex items-center gap-1.5 pointer-events-auto bg-card/95 backdrop-blur-md px-3 py-1.5 rounded-2xl border border-border/80 shadow-xl text-foreground">
      <!-- Layout Modes -->
      <Tooltip message="双页对开 (经典图书排版)">
        <button
          onclick={() => setLayoutMode("facing")}
          class="p-1.5 rounded-xl transition-colors {layoutMode === 'facing' ? 'bg-primary text-primary-foreground shadow-sm' : 'hover:bg-accent text-muted-foreground'}"
          title="双页对开"
        >
          <Columns2 size={16} />
        </button>
      </Tooltip>
      <Tooltip message="单页仿真 (经典单页大图模式)">
        <button
          onclick={() => setLayoutMode("single")}
          class="p-1.5 rounded-xl transition-colors {layoutMode === 'single' ? 'bg-primary text-primary-foreground shadow-sm' : 'hover:bg-accent text-muted-foreground'}"
          title="单页仿真"
        >
          <FileText size={16} />
        </button>
      </Tooltip>
      <Tooltip message="长卷上下滑动模式">
        <button
          onclick={() => setLayoutMode("continuous")}
          class="p-1.5 rounded-xl transition-colors {layoutMode === 'continuous' ? 'bg-primary text-primary-foreground shadow-sm' : 'hover:bg-accent text-muted-foreground'}"
          title="长卷上下平滑连续模式"
        >
          <ArrowUpDown size={16} />
        </button>
      </Tooltip>

      <div class="h-4 w-px bg-border/80 mx-1"></div>

      <!-- Book-like 3D Flip Animation Toggle -->
      {#if layoutMode !== "continuous"}
        <Tooltip message={bookFlipEnabled ? "已开启真实 3D 仿真拟物翻书 (带纸张曲面与透视光影)" : "已切换为极简平面滑动 (无 3D 效果)"}>
          <button
            onclick={() => (bookFlipEnabled = !bookFlipEnabled)}
            class="flex items-center gap-1 px-2.5 py-1 rounded-xl text-xs font-medium transition-all {bookFlipEnabled ? 'bg-primary/20 text-primary border border-primary/40 font-semibold shadow-xs' : 'hover:bg-accent text-muted-foreground opacity-75'}"
            title="3D 仿真翻书"
          >
            <Sparkles size={13} class={bookFlipEnabled ? "animate-pulse" : ""} />
            <span class="text-[11px]">{bookFlipEnabled ? "3D仿真开" : "3D仿真关"}</span>
          </button>
        </Tooltip>
        <div class="h-4 w-px bg-border/80 mx-1"></div>
      {/if}

      <!-- Hand Drag & Pan Tool Badge -->
      <Tooltip message="抓手平移模式：鼠标保持单击拖拽即可自由抓取移动页面位置 (双击恢复居中)">
        <div class="flex items-center gap-1 px-2 py-1 rounded-xl text-xs bg-primary/10 text-primary border border-primary/25 font-medium select-none">
          <Hand size={13} />
          <span class="text-[11px]">按住抓取平移</span>
        </div>
      </Tooltip>
      <div class="h-4 w-px bg-border/80 mx-1"></div>

      <!-- Zoom Controls -->
      <button
        onclick={() => setZoom(targetZoom - 0.15)}
        class="p-1.5 rounded-xl hover:bg-accent text-muted-foreground transition-colors"
        title="缩小 (-)"
      >
        <ZoomOut size={16} />
      </button>
      <button
        onclick={resetZoom}
        class="text-xs font-mono w-12 text-center font-medium hover:text-primary transition-colors cursor-pointer"
        title="缩放比例 (点击重置为 100%)"
      >
        {Math.round(targetZoom * livePinchFactor * 100)}%
      </button>
      <button
        onclick={() => setZoom(targetZoom + 0.15)}
        class="p-1.5 rounded-xl hover:bg-accent text-muted-foreground transition-colors"
        title="放大 (+)"
      >
        <ZoomIn size={16} />
      </button>

      <div class="h-4 w-px bg-border/80 mx-1"></div>

      <!-- Eye-Care Themes -->
      <Tooltip message="羊皮纸暖黄 (Sepia)">
        <button
          aria-label="羊皮纸暖黄主题"
          onclick={() => (theme = "parchment")}
          class="w-5 h-5 rounded-full border border-amber-800/40 bg-[#f4ebd0] {theme === 'parchment' ? 'ring-2 ring-primary ring-offset-1 scale-110' : ''} transition-all"
        ></button>
      </Tooltip>
      <Tooltip message="护眼豆沙绿 (Tea Green)">
        <button
          aria-label="护眼豆沙绿主题"
          onclick={() => (theme = "paper")}
          class="w-5 h-5 rounded-full border border-emerald-800/40 bg-[#d8ebd4] {theme === 'paper' ? 'ring-2 ring-primary ring-offset-1 scale-110' : ''} transition-all"
        ></button>
      </Tooltip>
      <Tooltip message="极简黑白 (Classic Light)">
        <button
          aria-label="极简黑白浅色主题"
          onclick={() => (theme = "default")}
          class="w-5 h-5 rounded-full border border-neutral-400 bg-white {theme === 'default' ? 'ring-2 ring-primary ring-offset-1 scale-110' : ''} transition-all"
        ></button>
      </Tooltip>
      <Tooltip message="暗夜深邃 (OLED Dark)">
        <button
          aria-label="暗夜深邃深色主题"
          onclick={() => (theme = "dark")}
          class="w-5 h-5 rounded-full border border-neutral-700 bg-neutral-950 {theme === 'dark' ? 'ring-2 ring-primary ring-offset-1 scale-110' : ''} transition-all"
        ></button>
      </Tooltip>
    </div>
  </div>

  <!-- Main Viewing Area with Hand Drag & Pan (按住鼠标左键自由抓取移动页面) -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="flex-1 flex items-center justify-center p-4 overflow-hidden relative min-h-0 {isGrabbing ? 'cursor-grabbing select-none' : 'cursor-grab'}"
    onmousedown={handleGrabMouseDown}
    ondblclick={(e) => {
      if (!(e.target as HTMLElement).closest("button, input, a, select")) {
        resetPan();
        resetZoom();
      }
    }}
  >
    {#if layoutMode === "continuous"}
      <!-- 120Hz Native Vertical Continuous Scrolling with Zero-Latency GPU Zoom -->
      <div
        bind:this={continuousContainer}
        onscroll={handleContinuousScroll}
        onmousedown={handleGrabMouseDown}
        class="w-full h-full overflow-y-auto select-none {isGrabbing ? 'cursor-grabbing' : 'cursor-grab'}"
      >
        <div class="flex flex-col items-center py-12 space-y-6">
          {#each Array(total) as _, i}
            {@const pageNum = i + 1}
            <div
              data-zen-slot={pageNum}
              class="relative shadow-2xl rounded-sm overflow-hidden bg-white shrink-0 flex items-center justify-center"
              style="width: {continuousSlotWidth}px; min-height: {continuousSlotHeight}px;"
              use:initContinuousSlot={pageNum}
            >
              <canvas class="block" style="width: 100%; height: auto; display: block;"></canvas>
              <div
                data-zen-placeholder
                class="absolute inset-0 flex items-center justify-center text-xs text-muted-foreground/30 font-mono bg-white pointer-events-none"
              >
                P.{pageNum}
              </div>
              <div class="absolute bottom-1 right-2 text-[10px] text-neutral-400 font-mono select-none">
                P.{pageNum}
              </div>
            </div>
          {/each}
        </div>
      </div>
    {:else}
      <!-- Single / Facing Page View with High-Fidelity 3D Simulation -->
      <!-- Prev Click Zone / Arrow -->
      <button
        onclick={prevPage}
        disabled={activePage <= 1}
        class="absolute left-6 top-1/2 -translate-y-1/2 p-3.5 rounded-full bg-black/15 hover:bg-black/35 text-foreground disabled:opacity-0 transition-all z-30 backdrop-blur-md shadow-lg cursor-pointer"
        title="上一页 (Left / PageUp / 向上轻扫)"
      >
        <ChevronLeft size={28} />
      </button>

      <!-- Book Container with Perspective & Instant GPU Zoom Transform & Panning Offset -->
      <div
        class="relative flex items-center justify-center max-h-full max-w-full"
        style="
          perspective: 2400px;
          transform: translate({panOffsetX}px, {panOffsetY}px) scale({visualScale});
          transform-origin: center center;
          transition: {isGrabbing || isPinching ? 'none' : 'transform 120ms ease-out'};
        "
      >
        <!-- Single Page View with Dual-Layer Physical 3D Peel Turn -->
        {#if layoutMode === "single"}
          <div
            class="relative flex items-center justify-center"
            style="perspective: 2400px;"
          >
            <!-- Base Page (underneath, visible during peel turn) -->
            <div
              class="relative shadow-2xl rounded-sm bg-white overflow-hidden"
              style="
                box-shadow: 0 16px 40px -10px rgba(0,0,0,0.35), 0 0 1px rgba(0,0,0,0.2);
              "
            >
              <canvas bind:this={canvasSingleUnder} class="block"></canvas>
            </div>

            <!-- Top Page (curls away to reveal base page) -->
            <div
              class="absolute inset-0 shadow-2xl rounded-sm bg-white overflow-hidden"
              style="
                box-shadow: 0 16px 40px -10px rgba(0,0,0,0.35), 0 0 1px rgba(0,0,0,0.2);
                transform-origin: {flipAnimation === 'next' ? 'left center' : 'right center'};
                transform-style: preserve-3d;
                transform: {
                  bookFlipEnabled
                    ? (flipAnimation === 'next' ? 'perspective(2000px) rotateY(-80deg) scale(0.95)' : flipAnimation === 'prev' ? 'perspective(2000px) rotateY(80deg) scale(0.95)' : 'perspective(2000px) rotateY(0deg) scale(1)')
                    : (flipAnimation === 'next' ? 'translateX(-24px)' : flipAnimation === 'prev' ? 'translateX(24px)' : 'translateX(0)')
                };
                opacity: {
                  bookFlipEnabled
                    ? (flipAnimation !== 'none' ? 0.2 : 1)
                    : (flipAnimation !== 'none' ? 0.8 : 1)
                };
                transition: {
                  bookFlipEnabled
                    ? (flipAnimation !== 'none' ? 'transform 360ms cubic-bezier(0.22, 1, 0.36, 1), opacity 360ms ease, box-shadow 360ms ease' : 'none')
                    : (flipAnimation !== 'none' ? 'transform 130ms ease-out, opacity 130ms ease-out' : 'none')
                };
                pointer-events: {flipAnimation !== 'none' ? 'none' : 'auto'};
              "
            >
              <canvas bind:this={canvasLeft} class="block"></canvas>
              <!-- Dynamic Paper Curl Highlight Sheen -->
              {#if bookFlipEnabled && flipAnimation !== 'none'}
                <div
                  class="absolute inset-0 pointer-events-none transition-opacity duration-300"
                  style="background: linear-gradient(to {flipAnimation === 'next' ? 'right' : 'left'}, rgba(0,0,0,0.28) 0%, rgba(255,255,255,0.35) 50%, rgba(0,0,0,0.2) 100%);"
                ></div>
              {/if}
            </div>
          </div>
        {:else if layoutMode === "facing"}
          <!-- Two-Page Facing Book Format (真实图书双页对开) -->
          <div
            class="flex items-center shadow-2xl rounded-md bg-white border border-black/10 overflow-visible relative"
            style="box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.4), 0 0 2px rgba(0, 0, 0, 0.2);"
          >
            <!-- Left Page -->
            <div
              class="relative bg-white"
              style="
                transform-origin: right center;
                transform-style: preserve-3d;
                transform: {
                  bookFlipEnabled
                    ? (flipAnimation === 'prev' ? 'perspective(2200px) rotateY(65deg) scale(0.98)' : 'perspective(2200px) rotateY(0deg)')
                    : (flipAnimation === 'prev' ? 'translateX(18px)' : 'translateX(0)')
                };
                transition: {
                  bookFlipEnabled
                    ? (flipAnimation !== 'none' ? 'transform 380ms cubic-bezier(0.22, 1, 0.36, 1), box-shadow 380ms ease' : 'none')
                    : (flipAnimation !== 'none' ? 'transform 130ms ease-out' : 'none')
                };
                box-shadow: {
                  bookFlipEnabled && flipAnimation === 'prev'
                    ? '15px 20px 35px -10px rgba(0,0,0,0.45)'
                    : 'none'
                };
                z-index: {flipAnimation === 'prev' ? 25 : 1};
              "
            >
              <canvas bind:this={canvasLeft} class="block"></canvas>
              <!-- 3D Paper Curl Dynamic Sheen (翻起时的立体曲面光影) -->
              {#if bookFlipEnabled && flipAnimation === 'prev'}
                <div
                  class="absolute inset-0 pointer-events-none transition-opacity duration-300"
                  style="background: linear-gradient(to left, rgba(0,0,0,0.28) 0%, rgba(255,255,255,0.22) 50%, rgba(0,0,0,0.15) 100%);"
                ></div>
              {/if}
              <!-- Left Spine Shadow Gradient (中缝深邃立体渐变) -->
              <div
                class="absolute top-0 bottom-0 right-0 w-8 pointer-events-none"
                style="background: linear-gradient(to left, rgba(0,0,0,0.24) 0%, rgba(0,0,0,0.06) 50%, transparent 100%);"
              ></div>
            </div>

            <!-- Authentic Center Book Spine (真实书脊立体中缝) -->
            <div
              class="w-1.5 self-stretch z-10 shrink-0"
              style="background: linear-gradient(to right, #2a2a2a 0%, #666666 50%, #2a2a2a 100%); box-shadow: inset 0 0 4px rgba(0,0,0,0.8);"
            ></div>

            <!-- Right Page -->
            {#if activePage + 1 <= total}
              <div
                class="relative bg-white"
                style="
                  transform-origin: left center;
                  transform-style: preserve-3d;
                  transform: {
                    bookFlipEnabled
                      ? (flipAnimation === 'next' ? 'perspective(2200px) rotateY(-65deg) scale(0.98)' : 'perspective(2200px) rotateY(0deg)')
                      : (flipAnimation === 'next' ? 'translateX(-18px)' : 'translateX(0)')
                  };
                  transition: {
                    bookFlipEnabled
                      ? (flipAnimation !== 'none' ? 'transform 380ms cubic-bezier(0.22, 1, 0.36, 1), box-shadow 380ms ease' : 'none')
                      : (flipAnimation !== 'none' ? 'transform 130ms ease-out' : 'none')
                  };
                  box-shadow: {
                    bookFlipEnabled && flipAnimation === 'next'
                      ? '-15px 20px 35px -10px rgba(0,0,0,0.45)'
                      : 'none'
                  };
                  z-index: {flipAnimation === 'next' ? 25 : 1};
                "
              >
                <canvas bind:this={canvasRight} class="block"></canvas>
                <!-- 3D Paper Curl Dynamic Sheen (翻起时的立体曲面光影) -->
                {#if bookFlipEnabled && flipAnimation === 'next'}
                  <div
                    class="absolute inset-0 pointer-events-none transition-opacity duration-300"
                    style="background: linear-gradient(to right, rgba(0,0,0,0.28) 0%, rgba(255,255,255,0.22) 50%, rgba(0,0,0,0.15) 100%);"
                  ></div>
                {/if}
                <!-- Right Spine Shadow Gradient (中缝深邃立体渐变) -->
                <div
                  class="absolute top-0 bottom-0 left-0 w-8 pointer-events-none"
                  style="background: linear-gradient(to right, rgba(0,0,0,0.24) 0%, rgba(0,0,0,0.06) 50%, transparent 100%);"
                ></div>
              </div>
            {/if}
          </div>
        {/if}
      </div>

      <!-- Next Click Zone / Arrow -->
      <button
        onclick={nextPage}
        disabled={activePage >= total || (layoutMode === "facing" && activePage + 1 >= total)}
        class="absolute right-6 top-1/2 -translate-y-1/2 p-3.5 rounded-full bg-black/15 hover:bg-black/35 text-foreground disabled:opacity-0 transition-all z-30 backdrop-blur-md shadow-lg cursor-pointer"
        title="下一页 (Right / Space / 向下轻扫)"
      >
        <ChevronRight size={28} />
      </button>
    {/if}
  </div>

  <!-- Bottom Reading Progress Bar -->
  <div
    class="h-1.5 w-full bg-black/10 relative transition-opacity duration-300 {hudVisible ? 'opacity-100' : 'opacity-40'}"
  >
    <div
      class="h-full bg-primary transition-all duration-200"
      style="width: {(activePage / total) * 100}%"
    ></div>
  </div>
</div>

<!-- Modal: Jump to Page Dialog (快捷键: G) -->
{#if showJumpDialog}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div
    class="fixed inset-0 z-[80] flex items-center justify-center bg-black/40 backdrop-blur-xs"
    onclick={() => (showJumpDialog = false)}
  >
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div
      class="bg-card/95 backdrop-blur-md border border-border/80 rounded-2xl shadow-2xl p-4 w-72 flex flex-col gap-3 text-foreground animate-in zoom-in-95 duration-150"
      onclick={(e) => e.stopPropagation()}
    >
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-1.5 text-xs font-semibold">
          <Compass size={14} class="text-primary" />
          <span>跳转到指定页数</span>
        </div>
        <button
          onclick={() => (showJumpDialog = false)}
          class="p-1 rounded-lg hover:bg-accent text-muted-foreground transition-colors cursor-pointer"
        >
          <X size={14} />
        </button>
      </div>

      <div class="flex items-center gap-2">
        <input
          type="number"
          min="1"
          max={total}
          bind:value={jumpInputVal}
          onkeydown={(e) => {
            if (e.key === "Enter") executeJump(jumpInputVal);
            if (e.key === "Escape") showJumpDialog = false;
          }}
          class="flex-1 bg-background border border-border rounded-xl px-3 py-1.5 text-sm font-mono focus:outline-none focus:ring-2 focus:ring-primary text-center"
          autofocus
        />
        <span class="text-xs text-muted-foreground font-mono">/ {total}</span>
      </div>

      <div class="flex items-center justify-end gap-2 pt-1">
        <button
          onclick={() => (showJumpDialog = false)}
          class="px-3 py-1 text-xs rounded-xl hover:bg-accent text-muted-foreground transition-colors cursor-pointer"
        >
          取消
        </button>
        <button
          onclick={() => executeJump(jumpInputVal)}
          class="px-3.5 py-1 text-xs rounded-xl bg-primary text-primary-foreground font-semibold shadow-sm hover:opacity-90 transition-opacity cursor-pointer"
        >
          跳转
        </button>
      </div>
    </div>
  </div>
{/if}
