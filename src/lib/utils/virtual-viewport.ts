/**
 * Viewport Virtualization & Windowing utility inspired by pdf.js.
 * Calculates which pages should be actively mounted and rendered
 * given the container scroll position, viewport height, and page dimensions.
 */

export interface VirtualWindow {
  /** 1-based index of the first page visible in the viewport */
  visibleStart: number;
  /** 1-based index of the last page visible in the viewport */
  visibleEnd: number;
  /** 1-based index of the first buffered page (including pre-render lookahead) */
  bufferedStart: number;
  /** 1-based index of the last buffered page (including pre-render lookahead) */
  bufferedEnd: number;
}

/**
 * Computes which pages fall within the visible viewport and buffer window.
 *
 * @param scrollTop Current scroll offset in pixels
 * @param viewportHeight Height of the scroll container in pixels
 * @param pageHeights Array of scaled heights for each page (in pixels)
 * @param pageGap Gap between pages in pixels (default 8)
 * @param bufferPages Number of buffer pages to keep rendered above/below visible area (default 1)
 */
export function computeVisiblePages(
  scrollTop: number,
  viewportHeight: number,
  pageHeights: number[],
  pageGap = 8,
  bufferPages = 1
): VirtualWindow {
  const total = pageHeights.length;
  if (total === 0) {
    return { visibleStart: 0, visibleEnd: 0, bufferedStart: 0, bufferedEnd: 0 };
  }

  // Pre-calculate page vertical offsets [top, bottom]
  let currentY = 0;
  let visibleStart = -1;
  let visibleEnd = -1;

  const viewportTop = Math.max(0, scrollTop);
  const viewportBottom = viewportTop + Math.max(1, viewportHeight);

  for (let i = 0; i < total; i++) {
    const pageTop = currentY;
    const pageBottom = pageTop + pageHeights[i];

    // Check if page intersects with viewport
    if (pageBottom >= viewportTop && pageTop <= viewportBottom) {
      if (visibleStart === -1) {
        visibleStart = i + 1; // 1-based
      }
      visibleEnd = i + 1; // 1-based
    }

    currentY = pageBottom + pageGap;
  }

  // Fallbacks if scroll position is slightly out of bounds
  if (visibleStart === -1) {
    if (viewportTop <= 0) {
      visibleStart = 1;
      visibleEnd = 1;
    } else {
      visibleStart = total;
      visibleEnd = total;
    }
  }

  const bufferedStart = Math.max(1, visibleStart - bufferPages);
  const bufferedEnd = Math.min(total, visibleEnd + bufferPages);

  return {
    visibleStart,
    visibleEnd,
    bufferedStart,
    bufferedEnd,
  };
}

/**
 * Clamps canvas pixel dimensions to prevent mobile or WebKit canvas allocation crashes
 * when zoom level and devicePixelRatio are both high.
 *
 * Max canvas dimension is commonly 4096px (standard safe across WebKit/Gecko/Blink).
 */
export function clampCanvasDimensions(
  width: number,
  height: number,
  dpr: number,
  maxDimension = 4096
): { canvasWidth: number; canvasHeight: number; effectiveDpr: number } {
  let effectiveDpr = dpr;
  let cw = Math.floor(width * effectiveDpr);
  let ch = Math.floor(height * effectiveDpr);

  const maxSide = Math.max(cw, ch);
  if (maxSide > maxDimension) {
    const scale = maxDimension / maxSide;
    effectiveDpr = Math.max(1, effectiveDpr * scale);
    cw = Math.floor(width * effectiveDpr);
    ch = Math.floor(height * effectiveDpr);
  }

  return {
    canvasWidth: Math.max(1, cw),
    canvasHeight: Math.max(1, ch),
    effectiveDpr,
  };
}

