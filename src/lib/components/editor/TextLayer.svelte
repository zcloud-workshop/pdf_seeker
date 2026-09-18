<script lang="ts">
  import type { PdfDocumentProxy } from "@/pdf-engine";
  import { Copy, Pencil } from "lucide-svelte";

  let {
    doc,
    pageNum,
    scale,
    pageHeight,
    ontextedit,
  }: {
    doc: PdfDocumentProxy;
    pageNum: number;
    scale: number;
    pageHeight: number;
    ontextedit?: (item: { text: string; x: number; y: number; fontSize: number; pageNum: number }) => void;
  } = $props();

  let items = $state<Array<{
    str: string;
    x: number;
    y: number;
    fontSize: number;
    width: number;
  }>>([]);
  let ready = $state(false);

  $effect(() => {
    let cancelled = false;
    async function load() {
      if (!doc) { items = []; ready = false; return; }
      try {
        const page = await doc.getPage(pageNum);
        const content = await page.getTextContent();
        if (cancelled) return;
        const result: typeof items = [];
        for (const item of content.items as Array<any>) {
          if (!item.str?.trim()) continue;
          const t = item.transform || [0, 0, 0, 0, 0, 0];
          const fontSize = Math.abs(t[0]) || Math.abs(t[3]) || 12;
          result.push({
            str: item.str,
            x: t[4],
            y: t[5],
            fontSize,
            width: item.width || item.str.length * fontSize * 0.6,
          });
        }
        items = result;
        ready = true;
      } catch {
        items = [];
        ready = false;
      }
    }
    load();
    return () => { cancelled = true; };
  });

  let selectionInfo = $state<{ text: string; x: number; y: number; fontSize: number; rect: DOMRect } | null>(null);

  function handleMouseup(e: MouseEvent) {
    if ((e.target as HTMLElement).closest(".text-layer-actions")) return;

    const sel = window.getSelection();
    if (!sel || sel.isCollapsed || !sel.rangeCount) {
      selectionInfo = null;
      return;
    }
    const range = sel.getRangeAt(0);
    const container = range.startContainer.parentElement;
    if (!container?.dataset?.idx) { selectionInfo = null; return; }

    const idx = Number(container.dataset.idx);
    const item = items[idx];
    if (!item) { selectionInfo = null; return; }

    const rect = range.getBoundingClientRect();
    selectionInfo = { text: sel.toString(), x: item.x, y: item.y, fontSize: item.fontSize, rect };
  }

  function handleEdit() {
    if (!selectionInfo || !ontextedit) return;
    ontextedit({
      text: selectionInfo.text,
      x: selectionInfo.x,
      y: selectionInfo.y,
      fontSize: selectionInfo.fontSize,
      pageNum,
    });
    window.getSelection()?.removeAllRanges();
    selectionInfo = null;
  }

  function handleCopy() {
    if (!selectionInfo) return;
    navigator.clipboard.writeText(selectionInfo.text);
  }

  function dismiss() {
    window.getSelection()?.removeAllRanges();
    selectionInfo = null;
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="absolute inset-0 z-[5] pointer-events-auto"
  style="cursor: text;"
  onmouseup={handleMouseup}
>
  {#if ready && items.length > 0}
    {#each items as item, i (i)}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <span
        data-idx={i}
        class="absolute"
        style="
          left: {item.x * scale}px;
          top: {(pageHeight - item.y - item.fontSize) * scale}px;
          font-size: {item.fontSize * scale}px;
          color: transparent;
          user-select: text;
          white-space: pre;
          line-height: 1;
        "
      >{item.str}</span>
    {/each}
  {/if}

  <!-- Floating action bar near selection -->
  {#if selectionInfo}
    {@const r = selectionInfo.rect}
    {@const pageEl = document.querySelector(`[data-page="${pageNum}"]`)}
    {@const pageRect = pageEl?.getBoundingClientRect()}
    {@const left = pageRect ? (r.left - pageRect.left + r.width / 2) : r.left}
    {@const top = pageRect ? (r.top - pageRect.top - 8) : r.top}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="text-layer-actions absolute flex items-center gap-1 bg-white border border-gray-200 rounded-lg shadow-xl px-1 py-1 z-[50]"
      style="left: {left}px; top: {top}px; transform: translate(-50%, -100%);"
    >
      <button
        onclick={(e) => { e.stopPropagation(); handleCopy(); }}
        class="flex items-center gap-1 px-2.5 py-1.5 rounded-md text-xs font-medium text-gray-700 hover:bg-gray-100 transition-colors"
      >
        <Copy size={13} />
        Copy
      </button>
      <div class="w-px h-4 bg-gray-200"></div>
      <button
        onclick={(e) => { e.stopPropagation(); handleEdit(); }}
        class="flex items-center gap-1 px-2.5 py-1.5 rounded-md text-xs font-medium text-blue-600 bg-blue-50 hover:bg-blue-100 transition-colors"
      >
        <Pencil size={13} />
        Edit
      </button>
      <div class="w-px h-4 bg-gray-200"></div>
      <button
        onclick={(e) => { e.stopPropagation(); dismiss(); }}
        class="px-1.5 py-1.5 rounded-md text-xs text-gray-400 hover:text-gray-600 hover:bg-gray-100 transition-colors"
      >
        x
      </button>
      <!-- Arrow pointing down -->
      <div class="absolute left-1/2 -translate-x-1/2 top-full w-2 h-2 bg-white border-r border-b border-gray-200 rotate-45 -mt-1"></div>
    </div>
  {/if}
</div>
