<script lang="ts">
  import { editHistory, canUndoStore, canRedoStore, selectedEditId, signPlacement } from "@/stores";
  import { pushOperation, undo, redo, getAppliedOperations } from "@/edit-history";
  import type { EditOperation } from "@/edit-history";
  import { Edit3, Trash2, Palette } from "lucide-svelte";

  export type EditTool = "editText" | "editRect" | "editHighlight";

  interface CanvasElement {
    id: string;
    op: EditOperation;
    x: number;
    y: number;
    w: number;
    h: number;
  }

  let {
    activeTool,
    pageNum,
    pageWidth,
    pageHeight,
    scale,
    zIndex = "z-[5]",
  }: {
    activeTool: EditTool | null;
    pageNum: number;
    pageWidth: number;
    pageHeight: number;
    scale: number;
    zIndex?: string;
  } = $props();

  let drawing = $state(false);
  let drawStart = $state({ x: 0, y: 0 });
  let drawCurrent = $state({ x: 0, y: 0 });
  let selectedId = $state<string | null>(null);
  let dragging = $state(false);
  let dragOffset = $state({ x: 0, y: 0 });
  let dragDelta = $state({ dx: 0, dy: 0 });
  let contextMenu = $state<{
    x: number;
    y: number;
    element: CanvasElement;
  } | null>(null);
  let signPreviewPos = $state({ x: -100, y: -100 });

  // In-place text placement for this page
  let inlineTextInput = $state<{
    x: number;
    y: number;
    fontSize: number;
    color: string;
    text: string;
  } | null>(null);

  // Current drawing defaults
  let currentRectBorder = $state("#000000");
  let currentRectFill = $state("#ffffff");
  let currentRectHasFill = $state(false);
  let currentRectBorderW = $state(2);
  let currentHlColor = $state("#ffff00");
  let currentHlOpacity = $state(0.4);

  const pageElements = $derived(() => {
    const ops = getAppliedOperations($editHistory);
    return ops
      .filter((op: EditOperation) => op.params.page === pageNum)
      .map((op: EditOperation): CanvasElement => {
        const p = op.params;
        const s = scale;
        const sx = p.x * s;
        const sy = (pageHeight - p.y - (p.h || 0)) * s;
        const sw = (p.w || 100) * s;
        const sh = (p.h || 20) * s;
        return { id: op.id, op, x: sx, y: sy, w: sw, h: sh };
      });
  });

  const hasElements = $derived(pageElements().length > 0);
  const isSignMode = $derived($signPlacement?.active === true);

  const capturesPointer = $derived(
    !!activeTool || isSignMode || drawing || dragging || !!selectedId || !!inlineTextInput
  );

  const cursorStyle = $derived(() => {
    if (isSignMode) return "copy";
    if (!activeTool) return "default";
    if (activeTool === "editText") return "text";
    return "crosshair";
  });

  // Sync selection with global store
  $effect(() => {
    selectedEditId.set(selectedId);
  });

  // Clear local selection when global store is cleared
  $effect(() => {
    const globalId = $selectedEditId;
    if (globalId === null && selectedId !== null) {
      selectedId = null;
    }
  });

  // ─── Pointer handlers ──────────────────────────────────────────────

  function onPointerDown(e: PointerEvent) {
    if (e.button === 2) return; // Right click is handled by oncontextmenu

    contextMenu = null;
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;

    // Signature placement
    if (isSignMode) {
      signPlacement.set(null); // exit sign mode
      signPlaceEvent(x, y);
      return;
    }

    // Check if clicking on an existing element
    const clicked = pageElements().find(
      (el) => x >= el.x && x <= el.x + el.w && y >= el.y && y <= el.y + el.h,
    );

    if (clicked) {
      selectedId = clicked.id;
      dragging = true;
      dragDelta = { dx: 0, dy: 0 };
      dragOffset = { x: x - clicked.x, y: y - clicked.y };
      (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
      return;
    }

    selectedId = null;

    if (!activeTool) return;

    if (activeTool === "editText") {
      if (inlineTextInput && inlineTextInput.text.trim()) {
        commitInlineText();
      }
      inlineTextInput = {
        x,
        y,
        fontSize: 14,
        color: "#000000",
        text: "",
      };
      return;
    }

    drawing = true;
    drawStart = { x, y };
    drawCurrent = { x, y };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function onPointerMove(e: PointerEvent) {
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;

    if (isSignMode) {
      signPreviewPos = { x, y };
      return;
    }

    if (dragging && selectedId) {
      const el = pageElements().find((el) => el.id === selectedId);
      if (el) {
        dragDelta = {
          dx: x - dragOffset.x - el.x,
          dy: y - dragOffset.y - el.y,
        };
      }
      return;
    }
    if (!drawing) return;
    drawCurrent = { x, y };
  }

  function onPointerUp(e: PointerEvent) {
    if (dragging) {
      dragging = false;
      if (selectedId && (dragDelta.dx !== 0 || dragDelta.dy !== 0)) {
        const el = pageElements().find((el) => el.id === selectedId);
        if (el) {
          const s = scale;
          const finalX = el.x + dragDelta.dx;
          const finalY = el.y + dragDelta.dy;
          const newPdfX = finalX / s;
          const newPdfY = pageHeight - (finalY / s) - (el.h / s);
          const hist = $editHistory;
          const idx = hist.operations.findIndex((o) => o.id === selectedId);
          if (idx >= 0) {
            hist.operations[idx] = {
              ...hist.operations[idx],
              params: { ...hist.operations[idx].params, x: newPdfX, y: newPdfY },
            };
            editHistory.set({ ...hist });
          }
        }
      }
      dragDelta = { dx: 0, dy: 0 };
      return;
    }
    if (!drawing) return;
    drawing = false;

    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const endX = e.clientX - rect.left;
    const endY = e.clientY - rect.top;
    const s = scale;
    const sx = Math.min(drawStart.x, endX);
    const sy = Math.min(drawStart.y, endY);
    const sw = Math.abs(endX - drawStart.x);
    const sh = Math.abs(endY - drawStart.y);
    if (sw < 5 && sh < 5) return;

    const pdfX = sx / s;
    const pdfW = sw / s;
    const pdfH = sh / s;
    const pdfY = pageHeight - sy / s - pdfH;

    if (activeTool === "editRect") {
      editHistory.set(
        pushOperation($editHistory, "addRectangle", {
          page: pageNum, x: pdfX, y: pdfY, w: pdfW, h: pdfH,
          borderColor: currentRectBorder, fillColor: currentRectHasFill ? currentRectFill : null,
          hasFill: currentRectHasFill, borderWidth: currentRectBorderW,
        }),
      );
    } else if (activeTool === "editHighlight") {
      editHistory.set(
        pushOperation($editHistory, "addHighlight", {
          page: pageNum, x: pdfX, y: pdfY, w: pdfW, h: pdfH,
          color: currentHlColor, opacity: currentHlOpacity,
        }),
      );
    }
  }

  // ─── Context Menu & Element Modification ──────────────────────────

  function handleContextMenu(e: MouseEvent, el: CanvasElement) {
    e.preventDefault();
    e.stopPropagation();
    selectedId = el.id;
    const container = (e.currentTarget as HTMLElement).closest("[data-canvas-page]") as HTMLElement | null;
    const rect = container ? container.getBoundingClientRect() : (e.currentTarget as HTMLElement).getBoundingClientRect();
    contextMenu = {
      x: Math.max(10, Math.min(rect.width - 160, e.clientX - rect.left)),
      y: Math.max(10, Math.min(rect.height - 180, e.clientY - rect.top)),
      element: el,
    };
  }

  function deleteElement(id: string) {
    const hist = $editHistory;
    const newOps = hist.operations.filter((o: EditOperation) => o.id !== id);
    editHistory.set({
      operations: newOps,
      currentIndex: Math.min(hist.currentIndex, newOps.length - 1),
    });
    if (selectedId === id) selectedId = null;
    contextMenu = null;
  }

  function updateFontSize(delta: number) {
    if (!contextMenu || contextMenu.element.op.type !== "addText") return;
    const id = contextMenu.element.id;
    const hist = $editHistory;
    const idx = hist.operations.findIndex((o) => o.id === id);
    if (idx >= 0) {
      const curSize = hist.operations[idx].params.fontSize || 14;
      const newSize = Math.max(8, Math.min(72, curSize + delta));
      hist.operations[idx] = {
        ...hist.operations[idx],
        params: { ...hist.operations[idx].params, fontSize: newSize, h: newSize },
      };
      editHistory.set({ ...hist });
    }
  }

  function updateColor(color: string) {
    if (!contextMenu) return;
    const id = contextMenu.element.id;
    const hist = $editHistory;
    const idx = hist.operations.findIndex((o) => o.id === id);
    if (idx >= 0) {
      hist.operations[idx] = {
        ...hist.operations[idx],
        params: { ...hist.operations[idx].params, color },
      };
      editHistory.set({ ...hist });
    }
  }

  function startEditSelectedText() {
    if (!contextMenu || contextMenu.element.op.type !== "addText") return;
    const el = contextMenu.element;
    const id = el.id;
    inlineTextInput = {
      x: el.x,
      y: el.y,
      fontSize: el.op.params.fontSize || 14,
      color: el.op.params.color || "#000000",
      text: el.op.params.text || "",
    };
    deleteElement(id);
    contextMenu = null;
  }

  // ─── Undo/Redo/Delete ─────────────────────────────────────────────

  function handleUndo() {
    const result = undo($editHistory);
    editHistory.set(result.history);
    selectedId = null;
    contextMenu = null;
  }

  function handleRedo() {
    const result = redo($editHistory);
    editHistory.set(result.history);
    selectedId = null;
    contextMenu = null;
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
    if (e.key === "Delete" || e.key === "Backspace") {
      if (selectedId && !inlineTextInput) {
        e.preventDefault();
        deleteElement(selectedId);
      }
    }
    if (e.key === "Escape") {
      selectedId = null;
      contextMenu = null;
      inlineTextInput = null;
    }
    if ((e.ctrlKey || e.metaKey) && e.key === "z" && !e.shiftKey) {
      e.preventDefault();
      handleUndo();
    }
    if ((e.ctrlKey || e.metaKey) && (e.key === "y" || (e.key === "z" && e.shiftKey))) {
      e.preventDefault();
      handleRedo();
    }
  }

  // ─── Sign placement event ──────────────────────────────────────────
  // Dispatched via custom DOM event for parent to handle
  function signPlaceEvent(screenX: number, screenY: number) {
    const el = document.querySelector(`[data-canvas-page="${pageNum}"]`);
    if (el) {
      el.dispatchEvent(new CustomEvent("signplace", {
        detail: { pageNum, screenX, screenY },
        bubbles: true,
      }));
    }
  }

  // ─── Commit inline text placement ──────────────────────────────────
  function commitInlineText() {
    if (!inlineTextInput || !inlineTextInput.text.trim()) {
      inlineTextInput = null;
      return;
    }
    const s = scale;
    const text = inlineTextInput.text.trim();
    const fontSize = inlineTextInput.fontSize;
    const color = inlineTextInput.color;

    const pdfX = inlineTextInput.x / s;
    const pdfY = pageHeight - (inlineTextInput.y / s) - fontSize;
    const pdfW = text.length * fontSize * 0.6;
    const pdfH = fontSize;

    const newHistory = pushOperation($editHistory, "addText", {
      page: pageNum,
      x: pdfX,
      y: pdfY,
      w: pdfW,
      h: pdfH,
      text,
      fontSize,
      color,
    });
    editHistory.set(newHistory);
    const lastOp = newHistory.operations[newHistory.currentIndex];
    if (lastOp) {
      selectedId = lastOp.id;
    }
    inlineTextInput = null;
  }
</script>

<svelte:window onkeydown={onKeydown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="absolute inset-0 {zIndex} {capturesPointer ? 'pointer-events-auto' : 'pointer-events-none'}"
  style="cursor: {cursorStyle()};"
  onpointerdown={onPointerDown}
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
  data-canvas-page={pageNum}
>
  {#each pageElements() as el (el.id)}
    {@const isSelected = selectedId === el.id}
    {@const curX = isSelected && dragging ? el.x + dragDelta.dx : el.x}
    {@const curY = isSelected && dragging ? el.y + dragDelta.dy : el.y}
    {#if el.op.type === "addText"}
      <div
        class="absolute border border-transparent hover:border-blue-400 transition-colors whitespace-nowrap cursor-move select-none"
        style="left:{curX}px; top:{curY}px; font-size:{el.op.params.fontSize * scale}px; color:{el.op.params.color || '#000'}; line-height: 1.2;"
        onclick={() => (selectedId = el.id)}
        oncontextmenu={(e) => handleContextMenu(e, el)}
        role="button"
        tabindex="0"
      >
        {el.op.params.text}
        {#if isSelected}
          <div class="absolute -inset-1 border-2 border-blue-500 rounded-sm pointer-events-none shadow-sm">
            <span
              class="absolute -top-2.5 -right-2.5 w-4 h-4 bg-red-500 hover:bg-red-600 text-white rounded-full flex items-center justify-center text-[10px] pointer-events-auto cursor-pointer shadow"
              onclick={(e) => { e.stopPropagation(); deleteElement(el.id); }}
              role="button"
              tabindex="0"
              title="Delete"
            >✕</span>
          </div>
        {/if}
      </div>
    {:else if el.op.type === "addRectangle"}
      <div
        class="absolute border-2 transition-colors cursor-move"
        style="left:{curX}px; top:{curY}px; width:{el.w}px; height:{el.h}px; border-color:{el.op.params.borderColor || '#000'}; background:{el.op.params.hasFill ? (el.op.params.fillColor || '#fff') : 'transparent'};"
        onclick={() => (selectedId = el.id)}
        oncontextmenu={(e) => handleContextMenu(e, el)}
        role="button"
        tabindex="0"
      >
        {#if isSelected}
          <div class="absolute -inset-1 border-2 border-blue-500 pointer-events-none">
            <span
              class="absolute -top-2.5 -right-2.5 w-4 h-4 bg-red-500 hover:bg-red-600 text-white rounded-full flex items-center justify-center text-[10px] pointer-events-auto cursor-pointer shadow"
              onclick={(e) => { e.stopPropagation(); deleteElement(el.id); }}
              role="button"
              tabindex="0"
              title="Delete"
            >✕</span>
          </div>
        {/if}
      </div>
    {:else if el.op.type === "addHighlight"}
      <div
        class="absolute rounded-sm transition-colors cursor-move"
        style="left:{curX}px; top:{curY}px; width:{el.w}px; height:{el.h}px; background:{el.op.params.color || '#ffff00'}; opacity:{el.op.params.opacity ?? 0.4};"
        onclick={() => (selectedId = el.id)}
        oncontextmenu={(e) => handleContextMenu(e, el)}
        role="button"
        tabindex="0"
      >
        {#if isSelected}
          <div class="absolute -inset-1 border-2 border-blue-500 rounded-sm pointer-events-none">
            <span
              class="absolute -top-2.5 -right-2.5 w-4 h-4 bg-red-500 hover:bg-red-600 text-white rounded-full flex items-center justify-center text-[10px] pointer-events-auto cursor-pointer shadow"
              onclick={(e) => { e.stopPropagation(); deleteElement(el.id); }}
              role="button"
              tabindex="0"
              title="Delete"
            >✕</span>
          </div>
        {/if}
      </div>
    {:else if el.op.type === "addWhiteout"}
      <div
        class="absolute pointer-events-none"
        style="left:{curX}px; top:{curY}px; width:{el.w}px; height:{el.h}px; background: white;"
      ></div>
    {/if}
  {/each}

  {#if drawing}
    {#if activeTool === "editRect"}
      <div
        class="absolute border-2 border-dashed pointer-events-none"
        style="left:{Math.min(drawStart.x, drawCurrent.x)}px; top:{Math.min(drawStart.y, drawCurrent.y)}px; width:{Math.abs(drawCurrent.x - drawStart.x)}px; height:{Math.abs(drawCurrent.y - drawStart.y)}px; border-color:{currentRectBorder};"
      ></div>
    {:else if activeTool === "editHighlight"}
      <div
        class="absolute rounded-sm pointer-events-none"
        style="left:{Math.min(drawStart.x, drawCurrent.x)}px; top:{Math.min(drawStart.y, drawCurrent.y)}px; width:{Math.abs(drawCurrent.x - drawStart.x)}px; height:{Math.abs(drawCurrent.y - drawStart.y)}px; background:{currentHlColor}; opacity:{currentHlOpacity};"
      ></div>
    {/if}
  {/if}

  {#if isSignMode && $signPlacement?.imagePath}
    <!-- svelte-ignore a11y_img_redundant_alt -->
    <img
      src={$signPlacement.imagePath.startsWith("file://") ? $signPlacement.imagePath : `file://${$signPlacement.imagePath}`}
      alt="signature"
      class="absolute pointer-events-none opacity-50"
      style="left:{signPreviewPos.x}px; top:{signPreviewPos.y}px; max-width: 200px; max-height: 100px; object-fit: contain; z-index: 60;"
    />
  {/if}

  {#if inlineTextInput}
    <!-- Inline text placement input in page coordinates -->
    <div
      class="absolute z-[80] flex flex-col gap-1.5 pointer-events-auto"
      style="left: {inlineTextInput.x}px; top: {inlineTextInput.y}px;"
      onpointerdown={(e) => e.stopPropagation()}
    >
      <div class="flex items-center gap-2 bg-card/95 backdrop-blur-sm border border-border rounded-md shadow-lg px-2.5 py-1 select-none">
        <span class="text-[11px] text-muted-foreground font-medium">Size:</span>
        <input
          type="number"
          min="6"
          max="120"
          bind:value={inlineTextInput.fontSize}
          class="w-12 px-1 py-0.5 text-xs rounded border border-input bg-transparent text-center text-foreground"
        />
        <span class="text-[11px] text-muted-foreground font-medium">Color:</span>
        <input
          type="color"
          bind:value={inlineTextInput.color}
          class="w-5 h-5 rounded cursor-pointer border-0 bg-transparent p-0"
        />
        <button
          type="button"
          class="text-[11px] font-medium bg-primary text-primary-foreground px-2 py-0.5 rounded hover:opacity-90 transition-opacity"
          onclick={(e) => { e.stopPropagation(); commitInlineText(); }}
        >
          Done
        </button>
        <button
          type="button"
          class="text-[11px] text-muted-foreground hover:text-foreground px-1 py-0.5"
          onclick={(e) => { e.stopPropagation(); inlineTextInput = null; }}
        >
          ✕
        </button>
      </div>
      <input
        type="text"
        autofocus
        bind:value={inlineTextInput.text}
        placeholder="Type text here..."
        class="px-2 py-1 rounded border-2 border-primary bg-background text-foreground text-sm shadow-md outline-none min-w-[160px]"
        style="font-size: {Math.max(12, inlineTextInput.fontSize * scale)}px; color: {inlineTextInput.color};"
        onkeydown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            commitInlineText();
          } else if (e.key === "Escape") {
            inlineTextInput = null;
          }
        }}
      />
    </div>
  {/if}

  {#if contextMenu}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="absolute z-[90] bg-card/95 backdrop-blur-md border border-border rounded-lg shadow-xl py-1 px-1 min-w-[140px] text-xs flex flex-col gap-0.5 pointer-events-auto"
      style="left: {contextMenu.x}px; top: {contextMenu.y}px;"
      onclick={(e) => e.stopPropagation()}
    >
      {#if contextMenu.element.op.type === "addText"}
        <button
          type="button"
          class="flex items-center gap-2 px-2 py-1.5 rounded hover:bg-accent hover:text-accent-foreground text-left w-full text-foreground"
          onclick={startEditSelectedText}
        >
          <Edit3 size={13} class="text-primary" />
          编辑文字
        </button>
        <div class="flex items-center justify-between px-2 py-1 text-muted-foreground border-t border-border mt-0.5 pt-1">
          <span class="text-[10px]">字号</span>
          <div class="flex items-center gap-1">
            <button
              type="button"
              class="w-5 h-5 rounded hover:bg-accent flex items-center justify-center font-bold text-foreground"
              onclick={() => updateFontSize(-2)}
              title="减小字号"
            >-</button>
            <span class="w-5 text-center font-mono text-foreground">{contextMenu.element.op.params.fontSize || 14}</span>
            <button
              type="button"
              class="w-5 h-5 rounded hover:bg-accent flex items-center justify-center font-bold text-foreground"
              onclick={() => updateFontSize(2)}
              title="增大字号"
            >+</button>
          </div>
        </div>
        <div class="flex items-center gap-1.5 px-2 py-1 border-t border-border mt-0.5 pt-1">
          <Palette size={11} class="text-muted-foreground" />
          {#each ["#000000", "#ef4444", "#3b82f6", "#10b981", "#f59e0b"] as c}
            <button
              type="button"
              class="w-3.5 h-3.5 rounded-full border border-border hover:scale-125 transition-transform"
              style="background-color: {c};"
              onclick={() => updateColor(c)}
            ></button>
          {/each}
        </div>
      {/if}
      <button
        type="button"
        class="flex items-center gap-2 px-2 py-1.5 rounded hover:bg-destructive/10 text-destructive text-left w-full border-t border-border mt-0.5 pt-1 font-medium"
        onclick={() => deleteElement(contextMenu!.element.id)}
      >
        <Trash2 size={13} />
        删除此项
      </button>
    </div>
  {/if}
</div>

{#if hasElements || $canUndoStore || $canRedoStore}
  <div class="absolute top-2 left-2 flex items-center gap-1 bg-card/90 backdrop-blur-sm border border-border rounded-lg px-2 py-1 shadow-sm z-10">
    <button
      onclick={handleUndo}
      disabled={!$canUndoStore}
      class="text-xs px-1.5 py-1 rounded hover:bg-accent disabled:opacity-30"
    >Undo</button>
    <button
      onclick={handleRedo}
      disabled={!$canRedoStore}
      class="text-xs px-1.5 py-1 rounded hover:bg-accent disabled:opacity-30"
    >Redo</button>
    {#if hasElements}
      <span class="text-xs text-muted-foreground ml-1">{pageElements().length}</span>
    {/if}
  </div>
{/if}
