<script lang="ts">
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { invoke } from "@tauri-apps/api/core";
  import { Button, Input, Label } from "@/components/ui";
  import { FileUp as Icon_FileUp, Save as Icon_Save, Loader2 as Icon_Loader2 } from "lucide-svelte";

  let { filePath = $bindable() }: { filePath: string | null } = $props();

  let extractPagesInput = $state("");
  let selectedExtractPages = $state<Set<number>>(new Set());
  let busy = $state(false);
  let resultMsg = $state("");
  let resultOk = $state(false);

  async function openFileForTool() {
    const selected = await open({ filters: [{ name: "PDF", extensions: ["pdf"] }] });
    if (selected) filePath = String(selected);
  }

  function parsePageRanges(input: string): number[] {
    const pages: number[] = [];
    for (const part of input.split(",")) {
      const trimmed = part.trim();
      if (!trimmed) continue;
      const rangeMatch = trimmed.match(/^(\d+)-(\d+)$/);
      if (rangeMatch) {
        const s = parseInt(rangeMatch[1]);
        const e = parseInt(rangeMatch[2]);
        if (s <= e) for (let i = s; i <= e; i++) pages.push(i);
      } else {
        const num = parseInt(trimmed);
        if (!isNaN(num) && num > 0) pages.push(num);
      }
    }
    return [...new Set(pages)].sort((a, b) => a - b);
  }

  export function toggleExtractPage(pageNum: number) {
    if (selectedExtractPages.has(pageNum)) {
      selectedExtractPages = new Set([...selectedExtractPages].filter((p) => p !== pageNum));
    } else {
      selectedExtractPages = new Set([...selectedExtractPages, pageNum]);
    }
    extractPagesInput = [...selectedExtractPages].sort((a, b) => a - b).join(", ");
  }

  function applyExtractInput() {
    selectedExtractPages = new Set(parsePageRanges(extractPagesInput));
  }

  async function executeExtractPages() {
    if (!filePath || selectedExtractPages.size === 0) return;
    busy = true;
    resultMsg = "";
    try {
      const out = await save({ filters: [{ name: "PDF", extensions: ["pdf"] }] });
      if (!out) return;
      const outPath = out as string;
      const pages = [...selectedExtractPages].sort((a, b) => a - b);
      await invoke("extract_pages_pdf", { req: { inputPath: filePath, outputPath: outPath, pagesToExtract: pages } });
      resultMsg = `Extracted ${pages.length} page(s) → ${outPath.split(/[\\/]/).pop()}`;
      resultOk = true;
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
    } finally {
      busy = false;
    }
  }

  export function getSelectedPages() { return selectedExtractPages; }
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
    <p class="text-sm text-muted-foreground">Extract pages from: <strong>{filePath.split(/[\\/]/).pop()}</strong></p>
    <div class="space-y-1.5">
      <div class="flex items-center gap-2">
        <Label class="shrink-0">Pages</Label>
        <Input value={extractPagesInput} onchange={(e) => { extractPagesInput = (e.target as HTMLInputElement).value; applyExtractInput(); }} placeholder="e.g. 1,3,5-7" class="flex-1" />
      </div>
      <div class="flex items-center gap-2 text-xs text-muted-foreground">
        <span>Click thumbnails or type page numbers.</span>
        {#if selectedExtractPages.size > 0}
          <span class="text-green-600 font-medium">{selectedExtractPages.size} page(s) selected</span>
        {/if}
      </div>
    </div>
    <Button onclick={executeExtractPages} disabled={busy || selectedExtractPages.size === 0}>
      {#if busy}<Icon_Loader2 size={14} class="animate-spin" />{:else}<Icon_Save size={14} class="mr-1.5" />Extract & Save{/if}
    </Button>
  {/if}
  {#if resultMsg}
    <div class="flex items-center gap-2 p-3 rounded-lg border {resultOk ? 'border-green-500/30 bg-green-500/5' : 'border-destructive/30 bg-destructive/5'}">
      <span class="text-sm {resultOk ? 'text-green-700' : 'text-destructive'}">{resultMsg}</span>
    </div>
  {/if}
</div>
