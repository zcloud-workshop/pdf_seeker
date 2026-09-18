/**
 * Shared Zoom & Cursor Anchoring Math
 * Used by Editor.svelte and ZenReadingOverlay.svelte for 100% consistent cursor-centered zooming
 */

export interface WheelZoomEvent {
  ctrlKey?: boolean;
  metaKey?: boolean;
  deltaMode?: number;
  deltaY: number;
}

export interface ScrollViewport {
  clientX: number;
  clientY: number;
  rectLeft: number;
  rectTop: number;
  scrollLeft: number;
  scrollTop: number;
}

export interface ZoomOptions {
  step?: number;
  pinchSensitivity?: number;
  minZoom?: number;
  maxZoom?: number;
}

/**
 * Computes next zoom level based on trackpad pinch (smooth exponential) or mouse wheel (discrete steps).
 */
export function computeZoomLevel(
  oldZoom: number,
  event: WheelZoomEvent,
  options: ZoomOptions = {}
): number {
  if (!event.ctrlKey && !event.metaKey) return oldZoom;

  const minZoom = options.minZoom ?? 0.25;
  const maxZoom = options.maxZoom ?? 5.0;
  const pinchSensitivity = options.pinchSensitivity ?? 0.016;
  const step = options.step ?? 0.15;

  let newZoom = oldZoom;
  const deltaMode = event.deltaMode ?? 0;
  // Mac trackpad pinch has deltaMode === 0 and fine-grained deltas (typically < 45)
  const isTrackpadPinch = event.ctrlKey && deltaMode === 0 && Math.abs(event.deltaY) < 45;

  if (isTrackpadPinch) {
    const factor = Math.exp(-event.deltaY * pinchSensitivity);
    newZoom = oldZoom * factor;
  } else {
    // Discrete stepped mouse wheel
    const direction = event.deltaY < 0 ? 1 : -1;
    newZoom = Math.round((oldZoom + direction * step) * 100) / 100;
  }

  // Clamp within bounds
  newZoom = Math.min(maxZoom, Math.max(minZoom, newZoom));
  if (Math.abs(newZoom - oldZoom) < 0.0005) return oldZoom;
  return newZoom;
}

/**
 * Computes the target scroll positions so the exact point under the mouse cursor remains motionless.
 */
export function computeScrollAnchor(
  oldZoom: number,
  newZoom: number,
  vp: ScrollViewport
): {
  targetScrollLeft: number;
  targetScrollTop: number;
  newScrollLeft: number;
  newScrollTop: number;
} {
  const cursorX = vp.clientX - vp.rectLeft + vp.scrollLeft;
  const cursorY = vp.clientY - vp.rectTop + vp.scrollTop;
  const ratio = newZoom / oldZoom;

  const targetScrollLeft = cursorX * ratio - (vp.clientX - vp.rectLeft);
  const targetScrollTop = cursorY * ratio - (vp.clientY - vp.rectTop);

  return {
    targetScrollLeft,
    targetScrollTop,
    newScrollLeft: targetScrollLeft,
    newScrollTop: targetScrollTop,
  };
}

