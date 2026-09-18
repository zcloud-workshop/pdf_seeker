<script lang="ts">
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { invoke } from "@tauri-apps/api/core";
  import { Button, Input, Label } from "@/components/ui";
  import {
    FileUp as Icon_FileUp,
    Save as Icon_Save,
    Loader2 as Icon_Loader2,
    Undo2 as Icon_Undo2,
    Redo2 as Icon_Redo2,
    Plus as Icon_Plus,
    Trash2 as Icon_Trash2,
  } from "lucide-svelte";
  import { editHistory, canUndoStore, canRedoStore } from "@/stores";
  import {
    pushOperation,
    undo,
    redo,
    getAppliedOperations,
    clearHistory,
    type EditOperation,
  } from "@/edit-history";
  import { onMount } from "svelte";

  let { filePath = $bindable(), previewPage = $bindable(1), mode }: { filePath: string | null; previewPage?: number; mode: "editText" | "editRect" | "editHighlight" } = $props();

  // --- Edit Text ---
  let editText = $state("");
  let editFontSize = $state(12);
  let editTextColor = $state("#000000");
  let editTextX = $state(72);
  let editTextY = $state(720);

  // --- Edit Rectangle ---
  let editRectX = $state(100);
  let editRectY = $state(100);
  let editRectW = $state(200);
  let editRectH = $state(50);
  let editRectBorder = $state("#000000");
  let editRectFill = $state("#ffffff");
  let editRectHasFill = $state(false);
  let editRectBorderW = $state(1);

  // --- Edit Highlight ---
  let editHlX = $state(100);
  let editHlY = $state(100);
  let editHlW = $state(200);
  let editHlH = $state(20);
  let editHlColor = $state("#ffff00");
  let editHlOpacity = $state(0.4);

  let busy = $state(false);
  let resultMsg = $state("");
  let resultOk = $state(false);

  function getCurrentParams(): Record<string, any> {
    if (mode === "editText") return { text: editText, x: editTextX, y: editTextY, fontSize: editFontSize, color: editTextColor, page: previewPage };
    if (mode === "editRect") return { x: editRectX, y: editRectY, w: editRectW, h: editRectH, borderColor: editRectBorder, fillColor: editRectFill, hasFill: editRectHasFill, borderWidth: editRectBorderW, page: previewPage };
    return { x: editHlX, y: editHlY, w: editHlW, h: editHlH, color: editHlColor, opacity: editHlOpacity, page: previewPage };
  }

  function getOpType(): EditOperation["type"] {
    if (mode === "editText") return "addText";
    if (mode === "editRect") return "addRectangle";
    return "addHighlight";
  }

  function addOperation() {
    if (mode === "editText" && !editText.trim()) return;
    editHistory.set(pushOperation($editHistory, getOpType(), getCurrentParams()));
  }

  function handleUndo() {
    const result = undo($editHistory);
    editHistory.set(result.history);
  }

  function handleRedo() {
    const result = redo($editHistory);
    editHistory.set(result.history);
  }

  function handleClearHistory() {
    editHistory.set(clearHistory());
  }

  async function saveAll() {
    if (!filePath) return;
    const ops = getAppliedOperations($editHistory);
    if (ops.length === 0) return;
    busy = true;
    resultMsg = "";
    try {
      const out = await save({ filters: [{ name: "PDF", extensions: ["pdf"] }] });
      if (!out) return;
      const editOps = ops.map(op => ({
        op_type: op.type === "addText" ? "addText" : op.type === "addRectangle" ? "addRectangle" : "addHighlight",
        params: op.params,
      }));
      await invoke("apply_edit_operations", { req: { inputPath: filePath, outputPath: out, operations: editOps } });
      resultMsg = `Applied ${ops.length} operation(s) → ${(out as string).split(/[\\/]/).pop()}`;
      resultOk = true;
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
    } finally {
      busy = false;
    }
  }

  async function openFileForTool() {
    const selected = await open({ filters: [{ name: "PDF", extensions: ["pdf"] }] });
    if (selected) filePath = String(selected);
  }

  function getParams() {
    if (mode === "editText") return { tool: "editText" as const, params: { text: editText, x: editTextX, y: editTextY, fontSize: editFontSize, color: editTextColor } };
    if (mode === "editRect") return { tool: "editRect" as const, params: { x: editRectX, y: editRectY, w: editRectW, h: editRectH, borderColor: editRectBorder, fillColor: editRectFill, hasFill: editRectHasFill, borderWidth: editRectBorderW } };
    return { tool: "editHighlight" as const, params: { x: editHlX, y: editHlY, w: editHlW, h: editHlH, color: editHlColor, opacity: editHlOpacity } };
  }

  export function getEditTextParams() { return getParams().params; }
  export function getEditRectParams() { return getParams().params; }
  export function getEditHighlightParams() { return getParams().params; }

  export function updateFromPointer(tool: string, params: { pdfX: number; pdfY: number; pdfRight: number; pdfTop: number; isClick: boolean; pageNum: number }) {
    if (tool === "editText") {
      if (params.isClick) { editTextX = params.pdfX; editTextY = params.pdfY; }
      else { editTextX = params.pdfX; editTextY = params.pdfY; editFontSize = Math.max(8, Math.round((params.pdfTop - params.pdfY) / 792 * 72 * 2)); }
    } else if (tool === "editRect") {
      editRectX = params.pdfX;
      editRectY = params.isClick ? params.pdfY - 50 : params.pdfY;
      editRectW = params.isClick ? 200 : params.pdfRight - params.pdfX;
      editRectH = params.isClick ? 50 : params.pdfTop - params.pdfY;
    } else if (tool === "editHighlight") {
      editHlX = params.pdfX;
      editHlY = params.isClick ? params.pdfY - 20 : params.pdfY;
      editHlW = params.isClick ? 200 : params.pdfRight - params.pdfX;
      editHlH = params.isClick ? 20 : params.pdfTop - params.pdfY;
    }
  }

  // Keyboard shortcuts: Ctrl+Z / Ctrl+Shift+Z
  onMount(() => {
    function onKeyDown(e: KeyboardEvent) {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      if ((e.metaKey || e.ctrlKey) && e.key === "z" && !e.shiftKey) {
        e.preventDefault();
        handleUndo();
      } else if ((e.metaKey || e.ctrlKey) && e.key === "z" && e.shiftKey) {
        e.preventDefault();
        handleRedo();
      } else if ((e.metaKey || e.ctrlKey) && e.key === "y") {
        e.preventDefault();
        handleRedo();
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  });

  const opCount = $derived(getAppliedOperations($editHistory).length);
</script>

<div class="space-y-3">
  {#if !filePath}
    <p class="text-sm text-muted-foreground">Open a PDF first.</p>
    <div class="flex gap-2">
      <Button variant="outline" size="sm" onclick={openFileForTool}>
        <Icon_FileUp size={14} class="mr-1.5" />Open PDF
      </Button>
    </div>
  {:else}
    <!-- Undo/Redo toolbar -->
    <div class="flex items-center gap-2">
      <Button variant="outline" size="sm" onclick={handleUndo} disabled={!$canUndoStore}>
        <Icon_Undo2 size={14} class="mr-1" />Undo
      </Button>
      <Button variant="outline" size="sm" onclick={handleRedo} disabled={!$canRedoStore}>
        <Icon_Redo2 size={14} class="mr-1" />Redo
      </Button>
      {#if opCount > 0}
        <span class="text-xs text-muted-foreground ml-auto">{opCount} operation(s)</span>
        <Button variant="ghost" size="sm" onclick={handleClearHistory} class="text-destructive hover:text-destructive">
          <Icon_Trash2 size={14} />
        </Button>
      {/if}
    </div>

    <p class="text-sm text-muted-foreground">Edit: <strong>{filePath.split(/[\\/]/).pop()}</strong></p>

    {#if mode === "editText"}
    <div class="space-y-1">
      <Label>Text</Label>
      <textarea value={editText} onchange={(e) => (editText = (e.target as HTMLTextAreaElement).value)} rows="2" class="w-full p-2 rounded-lg border border-input bg-transparent text-sm resize-y" placeholder="Enter text to add..."></textarea>
    </div>
    <div class="grid grid-cols-3 gap-3">
      <div class="space-y-1"><Label>Page</Label><Input type="number" min="1" value={previewPage} onchange={(e) => (previewPage = parseInt((e.target as HTMLInputElement).value) || 1)} /></div>
      <div class="space-y-1"><Label>Font Size</Label><Input type="number" value={editFontSize} onchange={(e) => (editFontSize = parseFloat((e.target as HTMLInputElement).value) || 12)} /></div>
      <div class="space-y-1">
        <Label>Color</Label>
        <div class="flex items-center gap-2">
          <input type="color" bind:value={editTextColor} class="w-8 h-8 rounded cursor-pointer" />
          <Input value={editTextColor} onchange={(e) => (editTextColor = (e.target as HTMLInputElement).value)} class="flex-1" />
        </div>
      </div>
    </div>
    <p class="text-xs text-muted-foreground">Position (PDF coordinates: origin at bottom-left)</p>
    <div class="grid grid-cols-2 gap-3">
      <div class="space-y-1"><Label>X</Label><Input type="number" value={editTextX} onchange={(e) => (editTextX = parseFloat((e.target as HTMLInputElement).value) || 0)} /></div>
      <div class="space-y-1"><Label>Y</Label><Input type="number" value={editTextY} onchange={(e) => (editTextY = parseFloat((e.target as HTMLInputElement).value) || 0)} /></div>
    </div>

    {:else if mode === "editRect"}
    <div class="grid grid-cols-2 gap-3">
      <div class="space-y-1"><Label>Page</Label><Input type="number" min="1" value={previewPage} onchange={(e) => (previewPage = parseInt((e.target as HTMLInputElement).value) || 1)} /></div>
      <div class="space-y-1"><Label>Border Width</Label><Input type="number" value={editRectBorderW} onchange={(e) => (editRectBorderW = parseFloat((e.target as HTMLInputElement).value) || 1)} /></div>
      <div class="space-y-1"><Label>X</Label><Input type="number" value={editRectX} onchange={(e) => (editRectX = parseFloat((e.target as HTMLInputElement).value) || 0)} /></div>
      <div class="space-y-1"><Label>Y</Label><Input type="number" value={editRectY} onchange={(e) => (editRectY = parseFloat((e.target as HTMLInputElement).value) || 0)} /></div>
      <div class="space-y-1"><Label>Width</Label><Input type="number" value={editRectW} onchange={(e) => (editRectW = parseFloat((e.target as HTMLInputElement).value) || 200)} /></div>
      <div class="space-y-1"><Label>Height</Label><Input type="number" value={editRectH} onchange={(e) => (editRectH = parseFloat((e.target as HTMLInputElement).value) || 50)} /></div>
    </div>
    <div class="flex items-center gap-4">
      <div class="space-y-1">
        <Label>Border Color</Label>
        <div class="flex items-center gap-2">
          <input type="color" bind:value={editRectBorder} class="w-8 h-8 rounded cursor-pointer" />
          <Input value={editRectBorder} onchange={(e) => (editRectBorder = (e.target as HTMLInputElement).value)} class="w-24" />
        </div>
      </div>
      <label class="flex items-center gap-2 text-sm cursor-pointer">
        <input type="checkbox" bind:checked={editRectHasFill} class="rounded" />Fill
      </label>
      {#if editRectHasFill}
        <div class="space-y-1">
          <Label>Fill Color</Label>
          <div class="flex items-center gap-2">
            <input type="color" bind:value={editRectFill} class="w-8 h-8 rounded cursor-pointer" />
            <Input value={editRectFill} onchange={(e) => (editRectFill = (e.target as HTMLInputElement).value)} class="w-24" />
          </div>
        </div>
      {/if}
    </div>

    {:else if mode === "editHighlight"}
    <div class="grid grid-cols-2 gap-3">
      <div class="space-y-1"><Label>Page</Label><Input type="number" min="1" value={previewPage} onchange={(e) => (previewPage = parseInt((e.target as HTMLInputElement).value) || 1)} /></div>
      <div class="space-y-1"><Label>Opacity (0-1)</Label><Input type="number" step="0.05" min="0" max="1" value={editHlOpacity} onchange={(e) => (editHlOpacity = parseFloat((e.target as HTMLInputElement).value) || 0.4)} /></div>
      <div class="space-y-1"><Label>X</Label><Input type="number" value={editHlX} onchange={(e) => (editHlX = parseFloat((e.target as HTMLInputElement).value) || 0)} /></div>
      <div class="space-y-1"><Label>Y</Label><Input type="number" value={editHlY} onchange={(e) => (editHlY = parseFloat((e.target as HTMLInputElement).value) || 0)} /></div>
      <div class="space-y-1"><Label>Width</Label><Input type="number" value={editHlW} onchange={(e) => (editHlW = parseFloat((e.target as HTMLInputElement).value) || 200)} /></div>
      <div class="space-y-1"><Label>Height</Label><Input type="number" value={editHlH} onchange={(e) => (editHlH = parseFloat((e.target as HTMLInputElement).value) || 20)} /></div>
    </div>
    <div class="space-y-1">
      <Label>Color</Label>
      <div class="flex items-center gap-2">
        <input type="color" bind:value={editHlColor} class="w-8 h-8 rounded cursor-pointer" />
        <Input value={editHlColor} onchange={(e) => (editHlColor = (e.target as HTMLInputElement).value)} class="w-24" />
      </div>
    </div>

    {/if}

    <!-- Add to history + Save buttons -->
    <div class="flex items-center gap-2">
      <Button onclick={addOperation} disabled={mode === "editText" && !editText.trim()}>
        <Icon_Plus size={14} class="mr-1.5" />Add
      </Button>
      <Button onclick={saveAll} disabled={busy || opCount === 0}>
        {#if busy}<Icon_Loader2 size={14} class="animate-spin" />{:else}<Icon_Save size={14} class="mr-1.5" />Save All{/if}
      </Button>
    </div>

    <!-- Operation history list -->
    {#if opCount > 0}
      <div class="space-y-1">
        <span class="text-xs font-medium text-muted-foreground">History:</span>
        <div class="max-h-32 overflow-auto space-y-1">
          {#each getAppliedOperations($editHistory) as op, i}
            <div class="flex items-center gap-2 text-xs px-2 py-1 rounded {i === $editHistory.currentIndex ? 'bg-primary/10 text-primary' : 'text-muted-foreground'}">
              <span class="font-mono">{i + 1}.</span>
              <span>{op.type === "addText" ? "Text" : op.type === "addRectangle" ? "Rect" : "Highlight"}</span>
              <span class="text-muted-foreground/60">p{op.params.page ?? "?"}</span>
            </div>
          {/each}
        </div>
      </div>
    {/if}
  {/if}
  {#if resultMsg}
    <div class="flex items-center gap-2 p-3 rounded-lg border {resultOk ? 'border-green-500/30 bg-green-500/5' : 'border-destructive/30 bg-destructive/5'}">
      <span class="text-sm {resultOk ? 'text-green-700' : 'text-destructive'}">{resultMsg}</span>
    </div>
  {/if}
</div>
