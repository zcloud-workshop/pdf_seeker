<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { invoke } from "@tauri-apps/api/core";
  import { readFile } from "@tauri-apps/plugin-fs";
  import { Button, Input } from "@/components/ui";
  import { FileUp as Icon_FileUp, ImageDown as Icon_ImageDown, Loader2 as Icon_Loader2 } from "lucide-svelte";
  import { loadPdf, renderPageToCanvas } from "@/pdf-engine";

  let { filePath = $bindable() }: { filePath: string | null } = $props();

  let pdf2imgPageRange = $state<"all" | "custom">("all");
  let pdf2imgPages = $state("");
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

  async function executePdf2Img() {
    if (!filePath) return;
    busy = true;
    resultMsg = "";
    try {
      const selected = await open({ directory: true });
      if (!selected) return;
      const outputDir = String(selected);
      const data = await readFile(filePath);
      const doc = await loadPdf(new Uint8Array(data));

      let pagesToConvert: number[];
      if (pdf2imgPageRange === "all") {
        pagesToConvert = Array.from({ length: doc.numPages }, (_, i) => i + 1);
      } else {
        pagesToConvert = parsePageRanges(pdf2imgPages);
      }

      const baseName = (filePath.split(/[\\/]/).pop() || "page").replace(/\.pdf$/i, "");
      for (const pageNum of pagesToConvert) {
        const canvas = document.createElement("canvas");
        await renderPageToCanvas(doc, pageNum, canvas, 2);
        const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob((b) => resolve(b), "image/png"));
        if (!blob) continue;
        const arrayBuffer = await blob.arrayBuffer();
        const fileName = `${baseName}_page_${pageNum}.png`;
        await invoke("save_image_file", { path: `${outputDir}/${fileName}`, data: Array.from(new Uint8Array(arrayBuffer)) });
      }

      resultMsg = `Exported ${pagesToConvert.length} page(s) as PNG`;
      resultOk = true;
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
    } finally {
      busy = false;
    }
  }
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
    <p class="text-sm text-muted-foreground">Convert to images: <strong>{filePath.split(/[\\/]/).pop()}</strong></p>
    <div class="flex gap-2">
      <Button variant={pdf2imgPageRange === "all" ? "default" : "outline"} size="sm" onclick={() => (pdf2imgPageRange = "all")}>All pages</Button>
      <Button variant={pdf2imgPageRange === "custom" ? "default" : "outline"} size="sm" onclick={() => (pdf2imgPageRange = "custom")}>Custom range</Button>
    </div>
    {#if pdf2imgPageRange === "custom"}
      <Input value={pdf2imgPages} onchange={(e) => (pdf2imgPages = (e.target as HTMLInputElement).value)} placeholder="e.g. 1,3,5-7" />
    {/if}
    <Button onclick={executePdf2Img} disabled={busy}>
      {#if busy}<Icon_Loader2 size={14} class="animate-spin" />{:else}<Icon_ImageDown size={14} class="mr-1.5" />Export as PNG{/if}
    </Button>
  {/if}
  {#if resultMsg}
    <div class="flex items-center gap-2 p-3 rounded-lg border {resultOk ? 'border-green-500/30 bg-green-500/5' : 'border-destructive/30 bg-destructive/5'}">
      <span class="text-sm {resultOk ? 'text-green-700' : 'text-destructive'}">{resultMsg}</span>
    </div>
  {/if}
</div>
