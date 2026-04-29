<script lang="ts">
  import { Button } from "@/components/ui";
  import ProgressBar from "@/components/ui/ProgressBar.svelte";
  import type { ProgressInfo } from "@/components/ui/ProgressBar.svelte";
  import { Loader2 } from "lucide-svelte";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { readFile, writeTextFile } from "@tauri-apps/plugin-fs";
  import { invoke } from "@tauri-apps/api/core";
  import { loadPdf } from "@/pdf-engine";
  import { currentView } from "@/stores";
  import type { ToolId } from "./ToolbarTabs.svelte";

  let {
    tool,
    filePath,
    onclose,
    onfilechanged,
  }: {
    tool: ToolId;
    filePath: string | null;
    onclose: () => void;
    onfilechanged: (path: string) => void;
  } = $props();

  let busy = $state(false);
  let resultMsg = $state("");
  let resultOk = $state(false);
  let progress = $state<ProgressInfo>({ current: 0, total: 0, startTime: 0, avgMsPerPage: 0 });

  // Merge state
  let mergeFiles = $state<string[]>([]);

  // Split state
  let splitMode = $state<"single" | "range">("single");
  let splitRanges = $state("");

  // Rotate state
  let rotateAngle = $state(90);

  // Delete/Extract pages state
  let pageRange = $state("");

  // Reorder state
  let reorderOrder = $state("");

  // Extract text state
  let extractedText = $state("");
  let extractMethod = $state<"built-in" | "ocr" | "">("");

  // Table state
  let tableResult = $state("");
  let tableFormat = $state<"csv" | "md">("csv");
  let tablePageScope = $state<"current" | "all">("current");
  let currentPageNum = $state(1);
  let tableCount = $state(0);
  let tableViewIdx = $state(0);

  const toolTitle: Record<string, string> = {
    merge: "Merge PDFs",
    split: "Split PDF",
    rotate: "Rotate PDF",
    delete: "Delete Pages",
    extractPages: "Extract Pages",
    reorder: "Reorder Pages",
    img2pdf: "Image to PDF",
    pdf2img: "PDF to Image",
    pdf2text: "PDF to Text",
    extractText: "Extract Text",
    table: "Extract Table",
  };

  // ─── Helpers ─────────────────────────────────────────────────────

  async function pickSave() {
    return await save({ filters: [{ name: "PDF", extensions: ["pdf"] }] });
  }

  async function pickFiles() {
    return await open({ multiple: true, filters: [{ name: "PDF", extensions: ["pdf"] }] });
  }

  async function pickDir() {
    return await open({ directory: true });
  }

  function baseName(p: string) {
    return p.split(/[\\/]/).pop() || "file";
  }

  function nameNoExt(p: string) {
    return baseName(p).replace(/\.pdf$/i, "");
  }

  function setResult(msg: string, ok: boolean) {
    resultMsg = msg;
    resultOk = ok;
  }

  // ─── Tool implementations ────────────────────────────────────────

  async function executeMerge() {
    if (mergeFiles.length < 2) return;
    busy = true;
    resultMsg = "";
    try {
      const out = await pickSave();
      if (!out) { busy = false; return; }
      await invoke("merge_pdfs", { paths: mergeFiles, outputPath: out });
      onfilechanged(out as string);
      setResult(`Merged ${mergeFiles.length} files`, true);
    } catch (e) {
      setResult(String(e), false);
    } finally {
      busy = false;
    }
  }

  async function executeSplit() {
    if (!filePath) return;
    busy = true;
    resultMsg = "";
    try {
      const dir = await pickDir();
      if (!dir) { busy = false; return; }
      const outDir = dir as string;
      await invoke("split_pdf", {
        req: { inputPath: filePath, outputDir: outDir, mode: splitMode, ranges: splitRanges || undefined },
      });
      setResult(`Split to ${outDir}`, true);
    } catch (e) {
      setResult(String(e), false);
    } finally {
      busy = false;
    }
  }

  async function executeRotate() {
    if (!filePath) return;
    busy = true;
    resultMsg = "";
    try {
      const out = await pickSave();
      if (!out) { busy = false; return; }
      await invoke("rotate_pdf", {
        req: { inputPath: filePath, outputPath: out, angle: rotateAngle },
      });
      onfilechanged(out as string);
      setResult(`Rotated ${rotateAngle} deg`, true);
    } catch (e) {
      setResult(String(e), false);
    } finally {
      busy = false;
    }
  }

  async function executeDeletePages() {
    if (!filePath || !pageRange.trim()) return;
    busy = true;
    resultMsg = "";
    try {
      const out = await pickSave();
      if (!out) { busy = false; return; }
      const pages = parsePageRange(pageRange);
      await invoke("delete_pages_pdf", {
        req: { inputPath: filePath, pagesToDelete: pages, outputPath: out },
      });
      onfilechanged(out as string);
      setResult(`Deleted pages ${pageRange}`, true);
    } catch (e) {
      setResult(String(e), false);
    } finally {
      busy = false;
    }
  }

  async function executeExtractPages() {
    if (!filePath || !pageRange.trim()) return;
    busy = true;
    resultMsg = "";
    try {
      const out = await pickSave();
      if (!out) { busy = false; return; }
      const pages = parsePageRange(pageRange);
      await invoke("extract_pages_pdf", {
        req: { inputPath: filePath, pagesToExtract: pages, outputPath: out },
      });
      onfilechanged(out as string);
      setResult(`Extracted pages ${pageRange}`, true);
    } catch (e) {
      setResult(String(e), false);
    } finally {
      busy = false;
    }
  }

  async function executeReorder() {
    if (!filePath || !reorderOrder.trim()) return;
    busy = true;
    resultMsg = "";
    try {
      const out = await pickSave();
      if (!out) { busy = false; return; }
      const newOrder = reorderOrder.split(",").map((s) => parseInt(s.trim())).filter((n) => !isNaN(n));
      await invoke("reorder_pages", {
        req: { inputPath: filePath, outputPath: out, newOrder },
      });
      onfilechanged(out as string);
      setResult(`Reordered ${newOrder.length} pages`, true);
    } catch (e) {
      setResult(String(e), false);
    } finally {
      busy = false;
    }
  }

  // ─── Smart Extract Text (pdfjs + OCR fallback) ──────────────────

  async function executeExtractText() {
    if (!filePath) return;
    busy = true;
    resultMsg = "";
    extractedText = "";
    extractMethod = "";
    try {
      // Smart triage: analyze PDF to choose strategy
      const analysis = await invoke<{
        totalPages: number;
        textPages: number;
        imagePages: number;
        recommendation: string;
      }>("ocr_analyze_pdf", { filePath });

      if (analysis.recommendation === "text") {
        // Fast path: pdfjs text extraction only
        const data = await readFile(filePath);
        const doc = await loadPdf(new Uint8Array(data));
        progress = { current: 0, total: doc.numPages, startTime: Date.now(), avgMsPerPage: 0 };

        const allItems: Array<{ str: string; x: number; y: number; width: number; height: number; pageNum: number }> = [];
        for (let p = 1; p <= doc.numPages; p++) {
          progress.current = p;
          const page = await doc.getPage(p);
          const content = await page.getTextContent();
          for (const item of content.items as Array<any>) {
            if (!item.str?.trim()) continue;
            const transform = item.transform || [0, 0, 0, 0, 0, 0];
            const fontSize = Math.abs(transform[0]) || Math.abs(transform[3]) || 12;
            allItems.push({ str: item.str, x: transform[4], y: transform[5], width: item.width || item.str.length * fontSize * 0.6, height: fontSize, pageNum: p });
          }
          progress.avgMsPerPage = (Date.now() - progress.startTime) / p;
        }

        if (allItems.length > 0) {
          const heights = allItems.map((i) => i.height);
          const medianH = [...heights].sort((a, b) => a - b)[Math.floor(heights.length / 2)] || 12;
          const rows = clusterByY(allItems, medianH * 0.5);
          let text = "";
          let prevY = -Infinity;
          for (const row of rows) {
            const rowY = row[0].y;
            if (Math.abs(rowY - prevY) > medianH * 2 && prevY > -Infinity) text += "\n";
            text += row.map((item) => item.str).join(" ") + "\n";
            prevY = rowY;
          }
          extractedText = text.trim();
          if (extractedText.length >= 50) {
            extractMethod = "built-in";
            setResult(`Extracted ${extractedText.length} chars (built-in)`, true);
            return;
          }
        }
      }

      // OCR path (for scanned or mixed PDFs)
      const data = await readFile(filePath);
      const doc = await loadPdf(new Uint8Array(data));
      progress = { current: 0, total: doc.numPages, startTime: Date.now(), avgMsPerPage: 0 };

      const allText: string[] = [];
      for (let p = 1; p <= doc.numPages; p++) {
        progress.current = p;
        const pageText = await ocrPage(doc, p);
        if (pageText) allText.push(`--- Page ${p} ---\n${pageText}`);
        progress.avgMsPerPage = (Date.now() - progress.startTime) / p;
      }

      if (allText.length === 0) {
        setResult("No text found in this PDF.", false);
      } else {
        extractedText = allText.join("\n\n");
        extractMethod = "ocr";
        setResult(`OCR extracted ${allText.length} page(s), ${extractedText.length} chars`, true);
      }
    } catch (e) {
      setResult(String(e), false);
    } finally {
      busy = false;
      progress = { current: 0, total: 0, startTime: 0, avgMsPerPage: 0 };
    }
  }

  async function ocrPage(doc: any, pageNum: number): Promise<string> {
    try {
      const configured: boolean = await invoke("ocr_check_configured");
      if (!configured) {
        currentView.set("settings");
        return "";
      }
    } catch {}
    try {
      await invoke("ocr_init_engine", { language: "default", gpuEnabled: true });
    } catch (e) {
      // Engine may already be initialized
    }
    const page = await doc.getPage(pageNum);
    const canvas = document.createElement("canvas");
    const vp = page.getViewport({ scale: 2 });
    canvas.width = Math.floor(vp.width);
    canvas.height = Math.floor(vp.height);
    const ctx = canvas.getContext("2d")!;
    await page.render({ canvasContext: ctx, viewport: vp }).promise;

    const tempDir = await invoke<string>("get_temp_dir");
    const imgPath = `${tempDir}/ocr_page_${pageNum}.png`;
    const blob = await new Promise<Blob | null>((r) => canvas.toBlob(r, "image/png"));
    if (!blob) return "";
    const ab = await blob.arrayBuffer();
    await invoke("save_image_file", { path: imgPath, data: Array.from(new Uint8Array(ab)) });
    const boxes = await invoke<Array<{ text: string; confidence: number }>>("ocr_recognize", { imagePath: imgPath });
    return boxes.map((b) => b.text).join("\n");
  }

  async function saveText() {
    if (!extractedText && !tableResult) return;
    const content = extractedText || tableResult;
    try {
      const ext = tool === "table" ? (tableFormat === "csv" ? "csv" : "md") : "txt";
      const out = await save({ filters: [{ name: ext.toUpperCase(), extensions: [ext] }] });
      if (!out) return;
      await writeTextFile(out as string, content);
      setResult(`Saved to ${baseName(out as string)}`, true);
    } catch (e) {
      setResult(String(e), false);
    }
  }

  // ─── Table Extraction ────────────────────────────────────────────

  // Store extracted tables for navigation
  let extractedTables: Array<{ pageNum: number; grid: string[][]; header: string }> = $state([]);

  async function executeTableExtract() {
    if (!filePath) return;
    busy = true;
    resultMsg = "";
    tableResult = "";
    extractedTables = [];
    tableCount = 0;
    tableViewIdx = 0;
    try {
      const data = await readFile(filePath);
      const doc = await loadPdf(new Uint8Array(data));

      const pages = tablePageScope === "current" ? [currentPageNum] : Array.from({ length: doc.numPages }, (_, i) => i + 1);
      progress = { current: 0, total: pages.length, startTime: Date.now(), avgMsPerPage: 0 };

      for (const pageNum of pages) {
        progress.current++;
        const pageTables = await extractTablesFromPage(doc, pageNum);
        for (const t of pageTables) {
          extractedTables.push(t);
        }
      }

      tableCount = extractedTables.length;
      if (tableCount === 0) {
        setResult("No table structure detected on selected pages.", false);
      } else {
        tableViewIdx = 0;
        formatCurrentTable();
        setResult(`Found ${tableCount} table(s) on ${pages.length} page(s)`, true);
      }
    } catch (e) {
      setResult(String(e), false);
    } finally {
      busy = false;
      progress = { current: 0, total: 0, startTime: 0, avgMsPerPage: 0 };
    }
  }

  function formatCurrentTable() {
    if (extractedTables.length === 0) { tableResult = ""; return; }
    const t = extractedTables[tableViewIdx];
    const grid = t.grid;
    const header = t.header;
    if (tableFormat === "csv") {
      const rows = grid.map((row) => row.map((cell) => `"${cell.replace(/"/g, '""')}"`).join(","));
      tableResult = header + "\n" + rows.join("\n");
    } else {
      const rows = grid.map((row) => "| " + row.join(" | ") + " |");
      const sep = "| " + grid[0].map(() => "---").join(" | ") + " |";
      tableResult = header + "\n" + rows[0] + "\n" + sep + "\n" + rows.slice(1).join("\n");
    }
  }

  $effect(() => {
    if (extractedTables.length > 0) formatCurrentTable();
  });

  async function extractTablesFromPage(doc: any, pageNum: number): Promise<Array<{ pageNum: number; grid: string[][]; header: string }>> {
    const page = await doc.getPage(pageNum);
    const content = await page.getTextContent();
    const items: Array<{ str: string; x: number; y: number; w: number; h: number }> = [];

    for (const item of content.items as Array<any>) {
      if (!item.str?.trim()) continue;
      const transform = item.transform || [0, 0, 0, 0, 0, 0];
      const fontSize = Math.abs(transform[0]) || Math.abs(transform[3]) || 12;
      items.push({ str: item.str.trim(), x: transform[4], y: transform[5], w: item.width || item.str.length * fontSize * 0.6, h: fontSize });
    }

    if (items.length === 0) {
      const ocrText = await ocrPage(doc, pageNum);
      if (!ocrText) return [];
      return [{ pageNum, grid: ocrText.split("\n").map((line) => [line]), header: `--- Page ${pageNum} (OCR) ---` }];
    }

    // Cluster items into rows by Y coordinate
    const heights = items.map((i) => i.h);
    const medianH = [...heights].sort((a, b) => a - b)[Math.floor(heights.length / 2)] || 12;
    const rows = clusterByYSimple(items, medianH * 0.7);

    if (rows.length < 2) return [];

    // Detect column separators: find X positions that appear consistently across rows
    const xPositions = new Map<number, number>();
    for (const row of rows) {
      const seen = new Set<number>();
      for (const item of row) {
        const pos = Math.round(item.x);
        if (!seen.has(pos)) {
          seen.add(pos);
          xPositions.set(pos, (xPositions.get(pos) || 0) + 1);
        }
      }
    }

    // Find column start positions that appear in multiple rows
    const minRowsForCol = Math.ceil(rows.length * 0.4);
    const colStarts = [...xPositions.entries()]
      .filter(([_, count]) => count >= minRowsForCol)
      .map(([pos]) => pos)
      .sort((a, b) => a - b);

    if (colStarts.length < 2) return [];

    // Determine column boundaries from start positions
    const colBoundaries: { start: number; end: number }[] = [];
    for (let i = 0; i < colStarts.length; i++) {
      const start = colStarts[i];
      const end = i + 1 < colStarts.length ? colStarts[i + 1] : 9999;
      colBoundaries.push({ start, end });
    }

    // Build grids - detect separate tables by Y-gap between rows
    // A row is a table row if it has items in at least 2 columns
    interface RowWithY { cells: string[]; y: number; nonEmpty: number; }
    const rowEntries: RowWithY[] = [];
    for (const row of rows) {
      const cells: string[] = [];
      for (const col of colBoundaries) {
        const matching = row.filter((item) => item.x >= col.start - 5 && item.x < col.end);
        cells.push(matching.map((m) => m.str).join(" ").trim());
      }
      const nonEmpty = cells.filter((c) => c.length > 0).length;
      if (nonEmpty >= 2) {
        const avgY = row.reduce((s, item) => s + item.y, 0) / row.length;
        rowEntries.push({ cells, y: avgY, nonEmpty });
      }
    }

    if (rowEntries.length < 2) return [];

    // Split into separate tables when Y-gap exceeds 1.5x median row height
    const yGaps: number[] = [];
    for (let i = 1; i < rowEntries.length; i++) {
      yGaps.push(Math.abs(rowEntries[i - 1].y - rowEntries[i].y));
    }
    const medianGap = [...yGaps].sort((a, b) => a - b)[Math.floor(yGaps.length / 2)] || medianH;
    const gapThreshold = medianGap * 1.8;

    const tables: Array<{ pageNum: number; grid: string[][]; header: string }> = [];
    let currentGroup: RowWithY[] = [rowEntries[0]];

    for (let i = 1; i < rowEntries.length; i++) {
      const gap = yGaps[i - 1];
      if (gap > gapThreshold) {
        // Finalize current table
        if (currentGroup.length >= 2) {
          const maxCols = Math.max(...currentGroup.map((r) => r.cells.length));
          const grid = currentGroup.map((r) => {
            const row = [...r.cells];
            while (row.length < maxCols) row.push("");
            return row;
          });
          tables.push({ pageNum, grid, header: `--- Page ${pageNum}, Table ${tables.length + 1} ---` });
        }
        currentGroup = [rowEntries[i]];
      } else {
        currentGroup.push(rowEntries[i]);
      }
    }
    // Finalize last group
    if (currentGroup.length >= 2) {
      const maxCols = Math.max(...currentGroup.map((r) => r.cells.length));
      const grid = currentGroup.map((r) => {
        const row = [...r.cells];
        while (row.length < maxCols) row.push("");
        return row;
      });
      tables.push({ pageNum, grid, header: `--- Page ${pageNum}, Table ${tables.length + 1} ---` });
    }

    return tables;
  }

  function clusterByYSimple(items: Array<{ str: string; x: number; y: number; w: number; h: number }>, threshold: number) {
    if (items.length === 0) return [];
    const sorted = [...items].sort((a, b) => b.y - a.y);
    const rows: typeof items[] = [];
    let currentRow: typeof items = [sorted[0]];
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

  function clusterByY(items: Array<{ str: string; x: number; y: number; width: number; height: number; pageNum: number }>, threshold: number) {
    if (items.length === 0) return [];
    const sorted = [...items].sort((a, b) => b.y - a.y);
    const rows: typeof items[] = [];
    let currentRow: typeof items = [sorted[0]];
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

  function parsePageRange(range: string): number[] {
    const pages = new Set<number>();
    for (const part of range.split(",")) {
      const trimmed = part.trim();
      if (trimmed.includes("-")) {
        const [a, b] = trimmed.split("-").map(Number);
        if (!isNaN(a) && !isNaN(b)) {
          for (let i = Math.min(a, b); i <= Math.max(a, b); i++) pages.add(i);
        }
      } else {
        const n = Number(trimmed);
        if (!isNaN(n)) pages.add(n);
      }
    }
    return Array.from(pages).sort((a, b) => a - b);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") onclose();
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="fixed inset-0 bg-black/40 flex items-center justify-center z-50"
  onclick={(e) => { if (e.target === e.currentTarget) onclose(); }}
>
  <div class="bg-card border border-border rounded-xl shadow-xl w-[500px] max-h-[80vh] flex flex-col">
    <!-- Header -->
    <div class="flex items-center justify-between px-5 py-3 border-b border-border shrink-0">
      <h2 class="text-sm font-medium">{toolTitle[tool] ?? tool}</h2>
      <button onclick={onclose} class="text-xs text-muted-foreground hover:text-foreground">X</button>
    </div>

    <!-- Body -->
    <div class="overflow-auto p-5 space-y-3 flex-1">
      {#if !filePath && tool !== "merge" && tool !== "img2pdf"}
        <p class="text-sm text-muted-foreground">Open a PDF first.</p>
      {:else if tool === "merge"}
        <div class="space-y-3">
          <p class="text-xs text-muted-foreground">Select PDF files to merge (order matters).</p>
          <div class="flex gap-2">
            <Button variant="outline" size="sm" onclick={async () => {
              const sel = await pickFiles();
              if (sel) mergeFiles = [...mergeFiles, ...(Array.isArray(sel) ? sel.map(String) : [String(sel)])];
            }}>Add Files</Button>
            {#if mergeFiles.length > 0}
              <Button variant="ghost" size="sm" onclick={() => (mergeFiles = [])}>Clear</Button>
            {/if}
          </div>
          {#if mergeFiles.length > 0}
            <div class="space-y-1 max-h-40 overflow-auto">
              {#each mergeFiles as f, i}
                <div class="flex items-center gap-2 text-xs">
                  <span class="text-muted-foreground">{i + 1}.</span>
                  <span class="flex-1 truncate">{baseName(f)}</span>
                  <button onclick={() => (mergeFiles = mergeFiles.filter((_, j) => j !== i))} class="text-destructive hover:underline">x</button>
                </div>
              {/each}
            </div>
            <Button size="sm" onclick={executeMerge} disabled={busy || mergeFiles.length < 2}>
              {#if busy}<Loader2 size={14} class="animate-spin" />{:else}Merge{/if}
            </Button>
          {/if}
        </div>
      {:else if tool === "split"}
        <div class="space-y-3">
          <p class="text-xs text-muted-foreground">Split: <strong>{baseName(filePath!)}</strong></p>
          <div class="flex gap-2">
            <Button variant={splitMode === "single" ? "default" : "outline"} size="sm" onclick={() => (splitMode = "single")}>Each page</Button>
            <Button variant={splitMode === "range" ? "default" : "outline"} size="sm" onclick={() => (splitMode = "range")}>By range</Button>
          </div>
          {#if splitMode === "range"}
            <input type="text" bind:value={splitRanges} placeholder="1-3,4-6" class="w-full px-2 py-1 text-sm rounded border border-input bg-transparent">
          {/if}
          <Button size="sm" onclick={executeSplit} disabled={busy}>
            {#if busy}<Loader2 size={14} class="animate-spin" />{:else}Split{/if}
          </Button>
        </div>
      {:else if tool === "rotate"}
        <div class="space-y-3">
          <p class="text-xs text-muted-foreground">Rotate: <strong>{baseName(filePath!)}</strong></p>
          <div class="flex gap-2">
            {#each [90, 180, 270] as angle}
              <Button variant={rotateAngle === angle ? "default" : "outline"} size="sm" onclick={() => (rotateAngle = angle)}>{angle} deg</Button>
            {/each}
          </div>
          <Button size="sm" onclick={executeRotate} disabled={busy}>
            {#if busy}<Loader2 size={14} class="animate-spin" />{:else}Rotate & Save{/if}
          </Button>
        </div>
      {:else if tool === "delete"}
        <div class="space-y-3">
          <p class="text-xs text-muted-foreground">Delete pages from: <strong>{baseName(filePath!)}</strong></p>
          <input type="text" bind:value={pageRange} placeholder="e.g. 1,3,5-7" class="w-full px-2 py-1 text-sm rounded border border-input bg-transparent">
          <Button size="sm" onclick={executeDeletePages} disabled={busy || !pageRange.trim()}>
            {#if busy}<Loader2 size={14} class="animate-spin" />{:else}Delete & Save{/if}
          </Button>
        </div>
      {:else if tool === "extractPages"}
        <div class="space-y-3">
          <p class="text-xs text-muted-foreground">Extract pages from: <strong>{baseName(filePath!)}</strong></p>
          <input type="text" bind:value={pageRange} placeholder="e.g. 1,3,5-7" class="w-full px-2 py-1 text-sm rounded border border-input bg-transparent">
          <Button size="sm" onclick={executeExtractPages} disabled={busy || !pageRange.trim()}>
            {#if busy}<Loader2 size={14} class="animate-spin" />{:else}Extract & Save{/if}
          </Button>
        </div>
      {:else if tool === "reorder"}
        <div class="space-y-3">
          <p class="text-xs text-muted-foreground">Reorder: <strong>{baseName(filePath!)}</strong></p>
          <p class="text-xs text-muted-foreground/60">Enter new page order (comma-separated)</p>
          <input type="text" bind:value={reorderOrder} placeholder="e.g. 3,1,2,4" class="w-full px-2 py-1 text-sm rounded border border-input bg-transparent">
          <Button size="sm" onclick={executeReorder} disabled={busy || !reorderOrder.trim()}>
            {#if busy}<Loader2 size={14} class="animate-spin" />{:else}Reorder & Save{/if}
          </Button>
        </div>
      {:else if tool === "pdf2text" || tool === "extractText"}
        <div class="space-y-3">
          <p class="text-xs text-muted-foreground">Extract text from: <strong>{baseName(filePath!)}</strong></p>
          <p class="text-xs text-muted-foreground/60">Automatically uses OCR for scanned documents.</p>
          <Button size="sm" onclick={executeExtractText} disabled={busy}>
            {#if busy}
              <span class="flex items-center gap-1.5"><Loader2 size={14} class="animate-spin" />Extracting{#if progress.total > 0} ({progress.current}/{progress.total}){/if}</span>
            {:else}Extract{/if}
          </Button>
          {#if extractedText}
            {#if extractMethod}
              <span class="inline-block text-[10px] px-1.5 py-0.5 rounded-full {extractMethod === "ocr" ? "bg-blue-100 text-blue-700" : "bg-green-100 text-green-700"}">{extractMethod === "ocr" ? "OCR" : "Built-in"}</span>
            {/if}
            <textarea readonly value={extractedText} class="w-full h-48 p-3 rounded-lg border border-input bg-muted text-sm font-mono resize-y whitespace-pre-wrap"></textarea>
            <div class="flex gap-2">
              <Button variant="outline" size="sm" onclick={saveText}>Save as .txt</Button>
              <Button variant="outline" size="sm" onclick={() => navigator.clipboard.writeText(extractedText)}>Copy All</Button>
            </div>
          {/if}
        </div>
      {:else if tool === "pdf2img"}
        <div class="space-y-3">
          <p class="text-xs text-muted-foreground">Convert: <strong>{baseName(filePath!)}</strong></p>
          <Button variant="outline" size="sm" onclick={async () => {
            if (!filePath) return;
            busy = true;
            try {
              const dir = await pickDir();
              if (!dir) { busy = false; return; }
              const outDir = dir as string;
              const data = await readFile(filePath);
              const doc = await loadPdf(new Uint8Array(data));
              progress = { current: 0, total: doc.numPages, startTime: Date.now(), avgMsPerPage: 0 };
              for (let p = 1; p <= doc.numPages; p++) {
                progress.current = p;
                const page = await doc.getPage(p);
                const canvas = document.createElement("canvas");
                const vp = page.getViewport({ scale: 2 });
                canvas.width = Math.floor(vp.width);
                canvas.height = Math.floor(vp.height);
                const ctx = canvas.getContext("2d")!;
                await page.render({ canvasContext: ctx, viewport: vp }).promise;
                const blob = await new Promise<Blob | null>((r) => canvas.toBlob(r, "image/png"));
                if (blob) {
                  const ab = await blob.arrayBuffer();
                  await invoke("save_image_file", { path: `${outDir}/${nameNoExt(filePath)}_page_${p}.png`, data: Array.from(new Uint8Array(ab)) });
                }
                progress.avgMsPerPage = (Date.now() - progress.startTime) / p;
              }
              setResult(`Exported ${doc.numPages} pages as PNG`, true);
            } catch (e) {
              setResult(String(e), false);
            } finally {
              busy = false;
              progress = { current: 0, total: 0, startTime: 0, avgMsPerPage: 0 };
            }
          }} disabled={busy}>
            {#if busy}
              <span class="flex items-center gap-1.5"><Loader2 size={14} class="animate-spin" />Exporting{#if progress.total > 0} ({progress.current}/{progress.total}){/if}</span>
            {:else}Export as PNG{/if}
          </Button>
        </div>
      {:else if tool === "img2pdf"}
        <div class="space-y-3">
          <p class="text-xs text-muted-foreground">Convert images to PDF.</p>
          <Button variant="outline" size="sm" onclick={async () => {
            busy = true;
            try {
              const sel = await open({ multiple: true, filters: [{ name: "Image", extensions: ["png", "jpg", "jpeg", "bmp", "webp"] }] });
              if (!sel) { busy = false; return; }
              const paths = Array.isArray(sel) ? sel.map(String) : [String(sel)];
              const out = await pickSave();
              if (!out) { busy = false; return; }
              await invoke("image_to_pdf", { req: { imagePaths: paths, outputPath: out } });
              onfilechanged(out as string);
              setResult(`Converted ${paths.length} images to PDF`, true);
            } catch (e) {
              setResult(String(e), false);
            } finally {
              busy = false;
            }
          }} disabled={busy}>
            {#if busy}<Loader2 size={14} class="animate-spin" />{:else}Select Images{/if}
          </Button>
        </div>
      {:else if tool === "table"}
        <div class="space-y-3">
          <p class="text-xs text-muted-foreground">Extract table from: <strong>{baseName(filePath!)}</strong></p>
          <div class="flex gap-3 items-center">
            <div class="flex gap-1">
              <Button variant={tablePageScope === "current" ? "default" : "outline"} size="sm" onclick={() => (tablePageScope = "current")}>Current page</Button>
              <Button variant={tablePageScope === "all" ? "default" : "outline"} size="sm" onclick={() => (tablePageScope = "all")}>All pages</Button>
            </div>
            {#if tablePageScope === "current"}
              <input type="number" bind:value={currentPageNum} min="1" class="w-16 px-1.5 py-1 text-sm rounded border border-input bg-transparent text-center">
            {/if}
          </div>
          <div class="flex gap-2 items-center">
            <span class="text-xs text-muted-foreground">Format:</span>
            <div class="flex gap-1">
              <Button variant={tableFormat === "csv" ? "default" : "outline"} size="sm" onclick={() => (tableFormat = "csv")}>CSV</Button>
              <Button variant={tableFormat === "md" ? "default" : "outline"} size="sm" onclick={() => (tableFormat = "md")}>Markdown</Button>
            </div>
          </div>
          <Button size="sm" onclick={executeTableExtract} disabled={busy}>
            {#if busy}
              <span class="flex items-center gap-1.5"><Loader2 size={14} class="animate-spin" />Extracting{#if progress.total > 0} ({progress.current}/{progress.total}){/if}</span>
            {:else}Extract Table{/if}
          </Button>
          {#if tableCount > 0}
            <div class="flex items-center gap-2">
              <span class="text-xs text-muted-foreground">Found {tableCount} table(s):</span>
              <button
                onclick={() => { if (tableViewIdx > 0) { tableViewIdx--; formatCurrentTable(); } }}
                disabled={tableViewIdx <= 0}
                class="text-xs px-1.5 py-0.5 rounded border border-input hover:bg-accent disabled:opacity-30"
              >&lt;</button>
              <span class="text-xs font-medium">{tableViewIdx + 1} / {tableCount}</span>
              <button
                onclick={() => { if (tableViewIdx < tableCount - 1) { tableViewIdx++; formatCurrentTable(); } }}
                disabled={tableViewIdx >= tableCount - 1}
                class="text-xs px-1.5 py-0.5 rounded border border-input hover:bg-accent disabled:opacity-30"
              >&gt;</button>
            </div>
          {/if}
          {#if tableResult}
            <textarea readonly value={tableResult} class="w-full h-48 p-3 rounded-lg border border-input bg-muted text-xs font-mono resize-y whitespace-pre-wrap leading-relaxed"></textarea>
            <div class="flex gap-2">
              <Button variant="outline" size="sm" onclick={saveText}>Save as {tableFormat === "csv" ? ".csv" : ".md"}</Button>
              <Button variant="outline" size="sm" onclick={() => navigator.clipboard.writeText(tableResult)}>Copy All</Button>
            </div>
          {/if}
        </div>
      {/if}

      {#if resultMsg}
        <div class="flex items-center gap-2 p-3 rounded-lg border {resultOk ? 'border-green-500/30 bg-green-500/5' : 'border-destructive/30 bg-destructive/5'}">
          <span class="text-sm {resultOk ? 'text-green-700' : 'text-destructive'}">{resultMsg}</span>
        </div>
      {/if}
    </div>
  </div>
</div>
