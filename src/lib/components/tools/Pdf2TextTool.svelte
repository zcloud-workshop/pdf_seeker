<script lang="ts">
  import { save } from "@tauri-apps/plugin-dialog";
  import { writeTextFile } from "@tauri-apps/plugin-fs";
  import { readFile } from "@tauri-apps/plugin-fs";
  import { Button } from "@/components/ui";
  import { Save as Icon_Save, Loader2 as Icon_Loader2 } from "lucide-svelte";
  import { loadPdf } from "@/pdf-engine";

  let { filePath = $bindable() }: { filePath: string | null } = $props();

  let extractedText = $state("");
  let busy = $state(false);
  let resultMsg = $state("");
  let resultOk = $state(false);

  interface TextItem {
    str: string;
    x: number;
    y: number;
    width: number;
    height: number;
    pageNum: number;
  }

  function clusterByY(items: TextItem[], threshold: number): TextItem[][] {
    if (items.length === 0) return [];
    const sorted = [...items].sort((a, b) => b.y - a.y);
    const rows: TextItem[][] = [];
    let currentRow: TextItem[] = [sorted[0]];
    let rowY = sorted[0].y;
    for (let i = 1; i < sorted.length; i++) {
      if (Math.abs(sorted[i].y - rowY) < threshold) {
        currentRow.push(sorted[i]);
      } else {
        rows.push(currentRow.sort((a, b) => a.x - b.x));
        currentRow = [sorted[i]];
        rowY = sorted[i].y;
      }
    }
    rows.push(currentRow.sort((a, b) => a.x - b.x));
    return rows;
  }

  async function executePdf2Text() {
    if (!filePath) return;
    busy = true;
    resultMsg = "";
    extractedText = "";
    try {
      const data = await readFile(filePath);
      const doc = await loadPdf(new Uint8Array(data));
      const allItems: TextItem[] = [];

      for (let p = 1; p <= doc.numPages; p++) {
        const page = await doc.getPage(p);
        const content = await page.getTextContent();
        for (const item of content.items as Array<any>) {
          if (!item.str?.trim()) continue;
          const transform = item.transform || [0, 0, 0, 0, 0, 0];
          const fontSize = Math.abs(transform[0]) || Math.abs(transform[3]) || 12;
          allItems.push({
            str: item.str,
            x: transform[4],
            y: transform[5],
            width: item.width || item.str.length * fontSize * 0.6,
            height: fontSize,
            pageNum: p,
          });
        }
      }

      if (allItems.length === 0) {
        resultMsg = "No text found in this PDF. It may be a scanned/image PDF.";
        resultOk = false;
        return;
      }

      // Cluster by Y to form lines, with spacing between lines
      const heights = allItems.map((i) => i.height);
      const medianH = [...heights].sort((a, b) => a - b)[Math.floor(heights.length / 2)] || 12;
      const rows = clusterByY(allItems, medianH * 0.5);

      // Build structured text: group consecutive rows with same Y into paragraphs
      let text = "";
      let prevY = -Infinity;
      for (const row of rows) {
        const rowY = row[0].y;
        const gap = Math.abs(rowY - prevY);
        if (gap > medianH * 2 && prevY > -Infinity) text += "\n";
        text += row.map((item) => item.str).join(" ") + "\n";
        prevY = rowY;
      }

      extractedText = text.trim();
      const totalPages = new Set(allItems.map((i) => i.pageNum)).size;
      resultMsg = `Extracted from ${totalPages} page(s), ${extractedText.length} chars`;
      resultOk = true;
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
    } finally {
      busy = false;
    }
  }

  async function saveExtractedText() {
    if (!extractedText) return;
    try {
      const out = await save({ filters: [{ name: "Text", extensions: ["txt"] }] });
      if (!out) return;
      const outPath = out as string;
      await writeTextFile(outPath, extractedText);
      resultMsg = `Saved to ${outPath.split(/[\\/]/).pop()}`;
      resultOk = true;
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
    }
  }

  $effect(() => {
    if (filePath) executePdf2Text();
  });
</script>

<div class="space-y-3">
  {#if !filePath}
    <p class="text-sm text-muted-foreground">Open a PDF first.</p>
  {:else}
    <div class="flex items-center gap-2">
      <p class="text-sm text-muted-foreground">Extract text from: <strong>{filePath.split(/[\\/]/).pop()}</strong></p>
      <Button variant="outline" size="sm" onclick={executePdf2Text} disabled={busy}>
        {#if busy}<Icon_Loader2 size={14} class="animate-spin" />{:else}Extract{/if}
      </Button>
    </div>
    {#if extractedText}
      <textarea readonly value={extractedText} class="w-full h-64 p-3 rounded-lg border border-input bg-muted text-sm font-mono resize-y whitespace-pre-wrap"></textarea>
      <div class="flex gap-2">
        <Button variant="outline" size="sm" onclick={saveExtractedText}>
          <Icon_Save size={14} class="mr-1.5" />Save as .txt
        </Button>
        <Button variant="outline" size="sm" onclick={() => navigator.clipboard.writeText(extractedText)}>Copy All</Button>
      </div>
    {/if}
  {/if}
  {#if resultMsg}
    <div class="flex items-center gap-2 p-3 rounded-lg border {resultOk ? 'border-green-500/30 bg-green-500/5' : 'border-destructive/30 bg-destructive/5'}">
      <span class="text-sm {resultOk ? 'text-green-700' : 'text-destructive'}">{resultMsg}</span>
    </div>
  {/if}
</div>
