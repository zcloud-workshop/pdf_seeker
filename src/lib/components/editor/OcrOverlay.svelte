<script lang="ts">
  import type { OcrBlock } from "@/stores";
  import { pushOperation } from "@/edit-history";
  import { editHistory } from "@/stores";

  let {
    blocks,
    pageNum,
    scale,
    pageHeight,
    visible,
  }: {
    blocks: OcrBlock[];
    pageNum: number;
    scale: number;
    pageHeight: number;
    visible: boolean;
  } = $props();

  const OCR_RENDER_SCALE = 2;

  // Track edited blocks: blockIndex → { newText }
  let editedBlocks = $state<Record<number, { newText: string }>>({});
  let editingIdx = $state<number | null>(null);
  let editText = $state("");

  function blockBounds(pts: [number, number][]) {
    const xs = pts.map((p) => p[0]);
    const ys = pts.map((p) => p[1]);
    const minX = Math.min(...xs);
    const maxX = Math.max(...xs);
    const minY = Math.min(...ys);
    const maxY = Math.max(...ys);
    return { minX, maxX, minY, maxY };
  }

  function blockScreenRect(block: OcrBlock) {
    const b = blockBounds(block.points);
    const sx = (b.minX / OCR_RENDER_SCALE) * scale;
    const sy = (b.minY / OCR_RENDER_SCALE) * scale;
    const sw = ((b.maxX - b.minX) / OCR_RENDER_SCALE) * scale;
    const sh = ((b.maxY - b.minY) / OCR_RENDER_SCALE) * scale;
    return { sx, sy, sw, sh };
  }

  function startEdit(idx: number) {
    if (editedBlocks[idx]) return; // already edited
    editingIdx = idx;
    editText = blocks[idx].text;
  }

  function commitEdit(idx: number) {
    if (editingIdx === null) return;

    const block = blocks[idx];
    const b = blockBounds(block.points);

    // PDF coordinates in points (bottom-left origin)
    const pdfX = b.minX / OCR_RENDER_SCALE;
    const pdfW = (b.maxX - b.minX) / OCR_RENDER_SCALE;
    const pdfH = (b.maxY - b.minY) / OCR_RENDER_SCALE;
    const pdfY = pageHeight - (b.maxY / OCR_RENDER_SCALE);
    const fontSize = Math.max(8, Math.round(pdfH * 0.85));

    let hist = $editHistory;

    if (editText.trim()) {
      // 1. Whiteout original text
      hist = pushOperation(hist, "addWhiteout", {
        page: pageNum,
        x: pdfX - 1,
        y: pdfY - 1,
        w: pdfW + 2,
        h: pdfH + 2,
      });

      // 2. Add replacement text
      hist = pushOperation(hist, "addText", {
        page: pageNum,
        x: pdfX,
        y: pdfY,
        w: pdfW,
        h: pdfH,
        text: editText,
        fontSize,
        color: "#000000",
      });

      editHistory.set(hist);
      editedBlocks[idx] = { newText: editText };
    } else {
      // Deleted: just whiteout
      hist = pushOperation(hist, "addWhiteout", {
        page: pageNum,
        x: pdfX - 1,
        y: pdfY - 1,
        w: pdfW + 2,
        h: pdfH + 2,
      });
      editHistory.set(hist);
      editedBlocks[idx] = { newText: "" };
    }

    editingIdx = null;
  }

  function cancelEdit() {
    editingIdx = null;
    editText = "";
  }

  function revertEdit(idx: number) {
    delete editedBlocks[idx];
    editedBlocks = { ...editedBlocks };
  }
</script>

{#if visible && blocks.length > 0}
  <div class="absolute inset-0 z-[6] pointer-events-auto select-none">
    {#each blocks as block, i (i)}
      {@const { sx, sy, sw, sh } = blockScreenRect(block)}
      {@const edited = editedBlocks[i]}
      {#if editingIdx === i}
        <!-- Edit mode: inline text input replacing the block -->
        <div class="absolute" style="left:{sx}px; top:{sy}px; z-index:30;">
          <textarea
            bind:value={editText}
            autofocus
            rows="1"
            class="px-1.5 py-0.5 text-xs bg-card border-2 border-primary text-foreground shadow-lg rounded outline-none resize min-w-[60px]"
            style="font-size: {Math.max(11, sh * 0.9)}px; line-height: 1.2; width: {Math.max(sw + 10, 80)}px; height: {Math.max(sh + 4, 24)}px;"
            onkeydown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                commitEdit(i);
              } else if (e.key === "Escape") {
                cancelEdit();
              }
            }}
            onblur={() => commitEdit(i)}
          ></textarea>
        </div>
      {:else if edited}
        <!-- Edited block: shows new text with revert option -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="absolute group cursor-pointer bg-card/80 border border-primary/40 rounded-sm"
          style="left:{sx}px; top:{sy}px; width:{sw}px; height:{sh}px;"
          onclick={() => startEdit(i)}
        >
          {#if edited.newText}
            <span
              class="block px-0.5 overflow-hidden text-foreground font-sans font-medium leading-tight truncate"
              style="font-size: {Math.max(9, Math.min(sh * 0.85, 14))}px;"
            >{edited.newText}</span>
          {/if}
          <!-- Revert button on hover -->
          <button
            class="absolute -top-1.5 -right-1.5 w-4 h-4 rounded-full bg-destructive text-destructive-foreground text-[10px] flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity shadow"
            onclick={(e) => { e.stopPropagation(); revertEdit(i); }}
            title="Revert"
          >✕</button>
        </div>
      {:else}
        <!-- Unedited OCR block: clean, subtle highlight on hover only -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="absolute border border-blue-500/20 hover:border-blue-500 hover:bg-blue-500/10 rounded-sm cursor-text transition-all overflow-hidden group"
          style="left:{sx}px; top:{sy}px; width:{sw}px; height:{sh}px;"
          onclick={() => startEdit(i)}
          title="{block.text} (Click to edit)"
        >
          <span
            class="hidden group-hover:block absolute bottom-full left-0 mb-1 z-30 px-1.5 py-0.5 bg-zinc-900 text-white text-[10px] rounded shadow-md pointer-events-none whitespace-nowrap"
          >{block.text}</span>
        </div>
      {/if}
    {/each}
  </div>
{/if}
