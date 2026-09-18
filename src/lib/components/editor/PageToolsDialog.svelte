<script lang="ts">
  import { Button } from "@/components/ui";
  import ProgressBar from "@/components/ui/ProgressBar.svelte";
  import type { ProgressInfo } from "@/components/ui/ProgressBar.svelte";
  import { Loader2 } from "lucide-svelte";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { readFile, writeTextFile } from "@tauri-apps/plugin-fs";
  import { invoke } from "@tauri-apps/api/core";
  import { loadPdf } from "@/pdf-engine";
  import { currentView, currentPage } from "@/stores";
  import type { ToolId } from "./ToolbarTabs.svelte";
  import { cleanOcrText } from "@/utils/text-cleaner";
  import { COMPRESSION_PRESETS, type CompressionPresetInfo } from "@/models/compression";
  import { executeSmartPdfCompression } from "@/utils/pdf-compressor";

  const compressionPresetList: CompressionPresetInfo[] = Object.values(COMPRESSION_PRESETS);

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

  // Insert pages state
  let insertSourceFile = $state("");
  let insertPosition = $state(0);

  // Page number state (Stirling-PDF style)
  let pageNumberFormat = $state("Page {n} of {total}");
  let pageNumberPosition = $state("bottom-center");
  let pageNumberStartPage = $state(1);
  let pageNumberStartNum = $state(1);
  let pageNumberFontSize = $state(10);
  let pageNumberMargin = $state(30);
  let pageNumberColor = $state("#333333");
  let compressPreset = $state<"balanced" | "high_quality" | "max_compression" | "grayscale">("balanced");

  // Extract text state
  let extractedText = $state("");
  let extractMethod = $state<"built-in" | "ocr" | "">("");
  let textPageScope = $state<"all" | "current" | "custom">("all");
  let textCustomRange = $state("");

  // Table state
  let tableResult = $state("");
  let tableFormat = $state<"csv" | "md">("csv");
  let tablePageScope = $state<"current" | "all">("current");
  let currentPageNum = $state(1);
  let tableCount = $state(0);
  let tableViewIdx = $state(0);

  $effect(() => {
    if ($currentPage) {
      currentPageNum = $currentPage;
    }
  });

  const toolTitle: Record<string, string> = {
    merge: "Merge PDFs",
    split: "Split PDF",
    rotate: "Rotate PDF",
    delete: "Delete Pages",
    extractPages: "Extract Pages",
    reorder: "Reorder Pages",
    insertPages: "Insert Pages",
    pageNumber: "Add Page Numbers",
    sanitize: "Sanitize PDF (Privacy)",
    compress: "Compress PDF",
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

  async function executeInsertPages() {
    if (!filePath || !insertSourceFile) return;
    busy = true;
    resultMsg = "";
    try {
      const out = await pickSave();
      if (!out) { busy = false; return; }
      await invoke("insert_pages", {
        req: { inputPath: filePath, sourcePath: insertSourceFile, outputPath: out, insertPosition },
      });
      onfilechanged(out as string);
      setResult(`Inserted pages from ${baseName(insertSourceFile)}`, true);
    } catch (e) {
      setResult(String(e), false);
    } finally {
      busy = false;
    }
  }

  // ─── Stirling-PDF Utilities ──────────────────────────────────────

  async function executeAddPageNumbers() {
    if (!filePath) return;
    busy = true;
    resultMsg = "";
    try {
      const out = await pickSave();
      if (!out) { busy = false; return; }
      await invoke("add_page_numbers", {
        req: {
          inputPath: filePath,
          outputPath: out,
          format: pageNumberFormat,
          position: pageNumberPosition,
          startPage: Number(pageNumberStartPage),
          startNumber: Number(pageNumberStartNum),
          fontSize: Number(pageNumberFontSize),
          margin: Number(pageNumberMargin),
          color: pageNumberColor,
        },
      });
      setResult(`Page numbers added successfully to ${baseName(out)}!`, true);
      onfilechanged(out as string);
    } catch (e: any) {
      setResult("Failed to add page numbers: " + (e?.message || String(e)), false);
    } finally {
      busy = false;
    }
  }

  async function executeSanitize() {
    if (!filePath) return;
    busy = true;
    resultMsg = "";
    try {
      const out = await pickSave();
      if (!out) { busy = false; return; }
      await invoke("sanitize_pdf", {
        inputPath: filePath,
        outputPath: out,
      });
      setResult(`Document sanitized! Metadata and trackers stripped safely.`, true);
      onfilechanged(out as string);
    } catch (e: any) {
      setResult("Failed to sanitize PDF: " + (e?.message || String(e)), false);
    } finally {
      busy = false;
    }
  }

  async function executeCompress() {
    if (!filePath) return;
    busy = true;
    resultMsg = "";
    try {
      const out = await pickSave();
      if (!out) { busy = false; return; }
      const res = await executeSmartPdfCompression(
        filePath,
        out,
        compressPreset
      );
      const orig = res.originalSize;
      const comp = res.compressedSize;
      const origKB = (orig / 1024).toFixed(1);
      const compKB = (comp / 1024).toFixed(1);
      const ratio = res.ratio;
      setResult(`Compression complete! Reduced from ${origKB}KB to ${compKB}KB (${ratio.toFixed(1)}% saved).`, true);
      onfilechanged(out as string);
    } catch (e: any) {
      setResult("Failed to compress PDF: " + (e?.message || String(e)), false);
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
      const data = await readFile(filePath);
      const doc = await loadPdf(new Uint8Array(data));

      let targetPages: number[] = [];
      if (textPageScope === "all") {
        targetPages = Array.from({ length: doc.numPages }, (_, i) => i + 1);
      } else if (textPageScope === "current") {
        const cur = Math.max(1, Math.min(doc.numPages, currentPageNum || $currentPage || 1));
        targetPages = [cur];
      } else {
        targetPages = parsePageRange(textCustomRange).filter((p) => p >= 1 && p <= doc.numPages);
        if (targetPages.length === 0) {
          setResult("Please enter valid page numbers (e.g. 1-3, 5)", false);
          busy = false;
          return;
        }
      }

      // If user selected "all", we can try backend fast extraction first
      if (textPageScope === "all") {
        try {
          const res = await invoke<{ text: string; pages: number }>("extract_text", { path: filePath });
          if (res && res.text && res.text.trim().length > 0) {
            extractedText = res.text.trim();
            extractMethod = "built-in";
            setResult(`Extracted ${extractedText.length} characters across ${res.pages} page(s)`, true);
            return;
          }
        } catch (nativeErr) {
          console.warn("Backend extract_text fallback to pdfjs:", nativeErr);
        }
      }

      // Client-side pdfjs extraction: strictly per target-page, with ordered lines
      progress = { current: 0, total: targetPages.length, startTime: Date.now(), avgMsPerPage: 0 };
      const pageResults: string[] = [];
      let totalChars = 0;
      let step = 0;

      for (const p of targetPages) {
        step++;
        progress.current = step;
        const page = await doc.getPage(p);
        const content = await page.getTextContent();
        const items: Array<{ str: string; x: number; y: number; w: number; h: number }> = [];

        for (const item of content.items as Array<any>) {
          if (!item.str || item.str.length === 0) continue;
          const transform = item.transform || [0, 0, 0, 0, 0, 0];
          const fontSize = Math.abs(transform[0]) || Math.abs(transform[3]) || 12;
          items.push({
            str: item.str,
            x: transform[4],
            y: transform[5],
            w: item.width || item.str.length * fontSize * 0.6,
            h: fontSize,
          });
        }

        if (items.length > 0) {
          const heights = items.map((i) => i.h);
          const medianH = [...heights].sort((a, b) => a - b)[Math.floor(heights.length / 2)] || 12;
          const rows = clusterByYSimple(items, medianH * 0.6);
          let pageText = "";
          let prevY = -Infinity;
          for (const row of rows) {
            const rowY = row[0].y;
            if (prevY > -Infinity && Math.abs(rowY - prevY) > medianH * 1.8) {
              pageText += "\n";
            }
            pageText += row.map((item) => item.str).join(" ").trim() + "\n";
            prevY = rowY;
          }
          const trimmed = pageText.trim();
          if (trimmed) {
            pageResults.push(`--- Page ${p} ---\n${trimmed}`);
            totalChars += trimmed.length;
          }
        }
        progress.avgMsPerPage = (Date.now() - progress.startTime) / step;
      }

      if (totalChars > 0) {
        extractedText = pageResults.join("\n\n");
        extractMethod = "built-in";
        setResult(`Extracted ${totalChars} characters across ${targetPages.length} page(s)`, true);
        return;
      }

      // Fallback to OCR for target pages
      setResult("No text stream found. Running OCR on selected pages...", true);
      const allText: string[] = [];
      step = 0;
      for (const p of targetPages) {
        step++;
        progress.current = step;
        const pageText = await ocrPage(doc, p);
        if (pageText) allText.push(`--- Page ${p} (OCR) ---\n${pageText}`);
        progress.avgMsPerPage = (Date.now() - progress.startTime) / step;
      }

      if (allText.length === 0) {
        setResult("No text found in selected page(s).", false);
      } else {
        extractedText = allText.join("\n\n");
        extractMethod = "ocr";
        setResult(`OCR extracted ${allText.length} page(s), ${extractedText.length} characters`, true);
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
    const raw = boxes.map((b) => b.text).join("\n");
    return cleanOcrText(raw);
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

      const curPageValid = Math.max(1, Math.min(doc.numPages, currentPageNum || $currentPage || 1));
      const pages = tablePageScope === "current" ? [curPageValid] : Array.from({ length: doc.numPages }, (_, i) => i + 1);
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
      if (!item.str || item.str.trim().length === 0) continue;
      const transform = item.transform || [0, 0, 0, 0, 0, 0];
      const fontSize = Math.abs(transform[0]) || Math.abs(transform[3]) || 12;
      items.push({
        str: item.str.trim(),
        x: transform[4],
        y: transform[5],
        w: item.width || item.str.length * fontSize * 0.6,
        h: fontSize,
      });
    }

    if (items.length === 0) {
      const ocrText = await ocrPage(doc, pageNum);
      if (!ocrText) return [];
      return [{ pageNum, grid: ocrText.split("\n").map((line) => [line]), header: `--- Page ${pageNum} (OCR) ---` }];
    }

    // Cluster items into rows by Y coordinate
    const heights = items.map((i) => i.h);
    const medianH = [...heights].sort((a, b) => a - b)[Math.floor(heights.length / 2)] || 12;
    const allRows = clusterByYSimple(items, medianH * 0.7);

    if (allRows.length < 2) return [];

    // Filter candidate table rows: rows with at least 2 horizontally distinct items
    const candidateRows: typeof allRows = [];
    for (const r of allRows) {
      if (r.length >= 2) {
        // Check if items span across horizontal distance
        const minX = Math.min(...r.map((it) => it.x));
        const maxX = Math.max(...r.map((it) => it.x + it.w));
        if (maxX - minX > medianH * 3) {
          candidateRows.push(r);
        }
      }
    }

    if (candidateRows.length < 2) return [];

    // Adaptive column detection: gather item start and end coordinates
    // Cluster X coordinates with a dynamic tolerance
    const colTolerance = Math.max(16, medianH * 1.4);
    interface ColumnCluster {
      center: number;
      minX: number;
      maxX: number;
      count: number;
    }
    const clusters: ColumnCluster[] = [];

    for (const row of candidateRows) {
      for (const it of row) {
        const x = it.x;
        let matched = false;
        for (const c of clusters) {
          if (Math.abs(c.center - x) <= colTolerance) {
            c.center = (c.center * c.count + x) / (c.count + 1);
            c.minX = Math.min(c.minX, x);
            c.maxX = Math.max(c.maxX, x + it.w);
            c.count++;
            matched = true;
            break;
          }
        }
        if (!matched) {
          clusters.push({
            center: x,
            minX: x,
            maxX: x + it.w,
            count: 1,
          });
        }
      }
    }

    // Filter clusters that appear in at least 20% of candidate rows or >= 2 rows
    const minRowSupport = Math.max(2, Math.floor(candidateRows.length * 0.2));
    const validCols = clusters
      .filter((c) => c.count >= minRowSupport)
      .sort((a, b) => a.center - b.center);

    if (validCols.length < 2) return [];

    // Build column boundaries from centers
    const boundaries: Array<{ start: number; end: number }> = [];
    for (let i = 0; i < validCols.length; i++) {
      const prevEnd = i > 0 ? (validCols[i - 1].center + validCols[i].center) / 2 : 0;
      const nextEnd = i < validCols.length - 1 ? (validCols[i].center + validCols[i + 1].center) / 2 : 99999;
      boundaries.push({ start: prevEnd, end: nextEnd });
    }

    // Build row entries
    interface RowWithY { cells: string[]; y: number; nonEmpty: number; }
    const rowEntries: RowWithY[] = [];

    for (const row of candidateRows) {
      const cells = new Array(boundaries.length).fill("");
      for (const item of row) {
        const itemMid = item.x + item.w * 0.2; // slight bias towards left
        let colIdx = boundaries.findIndex((b) => itemMid >= b.start && itemMid < b.end);
        if (colIdx < 0) {
          // Nearest column fallback
          let minDist = Infinity;
          for (let b = 0; b < validCols.length; b++) {
            const d = Math.abs(validCols[b].center - item.x);
            if (d < minDist) {
              minDist = d;
              colIdx = b;
            }
          }
        }
        if (colIdx >= 0) {
          cells[colIdx] = cells[colIdx] ? `${cells[colIdx]} ${item.str}` : item.str;
        }
      }

      const nonEmpty = cells.filter((c) => c.trim().length > 0).length;
      if (nonEmpty >= 2) {
        const avgY = row.reduce((s, it) => s + it.y, 0) / row.length;
        rowEntries.push({ cells, y: avgY, nonEmpty });
      }
    }

    if (rowEntries.length < 2) return [];

    // Group rows into tables based on vertical gap between rows
    const yGaps: number[] = [];
    for (let i = 1; i < rowEntries.length; i++) {
      yGaps.push(Math.abs(rowEntries[i - 1].y - rowEntries[i].y));
    }
    const medianGap = [...yGaps].sort((a, b) => a - b)[Math.floor(yGaps.length / 2)] || medianH;
    const gapThreshold = medianGap * 2.2;

    const tables: Array<{ pageNum: number; grid: string[][]; header: string }> = [];
    let currentGroup: RowWithY[] = [rowEntries[0]];

    for (let i = 1; i < rowEntries.length; i++) {
      const gap = yGaps[i - 1];
      if (gap > gapThreshold) {
        if (currentGroup.length >= 2) {
          tables.push({
            pageNum,
            grid: currentGroup.map((r) => r.cells),
            header: `--- Page ${pageNum}, Table ${tables.length + 1} ---`,
          });
        }
        currentGroup = [rowEntries[i]];
      } else {
        currentGroup.push(rowEntries[i]);
      }
    }

    if (currentGroup.length >= 2) {
      tables.push({
        pageNum,
        grid: currentGroup.map((r) => r.cells),
        header: `--- Page ${pageNum}, Table ${tables.length + 1} ---`,
      });
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
      {:else if tool === "insertPages"}
        <div class="space-y-3">
          <p class="text-xs text-muted-foreground">Insert pages into: <strong>{baseName(filePath!)}</strong></p>
          <p class="text-xs text-muted-foreground/60">Select a PDF file to insert, then choose the position.</p>
          <div class="flex gap-2">
            <Button variant="outline" size="sm" onclick={async () => {
              const sel = await open({ multiple: false, filters: [{ name: "PDF", extensions: ["pdf"] }] });
              if (sel) insertSourceFile = String(sel);
            }}>Select Source PDF</Button>
          </div>
          {#if insertSourceFile}
            <p class="text-xs text-muted-foreground">Source: <strong>{baseName(insertSourceFile)}</strong></p>
            <div class="space-y-1">
              <label class="text-xs text-muted-foreground">Insert at position (after page)</label>
              <input type="number" bind:value={insertPosition} min="0" class="w-24 px-2 py-1 text-sm rounded border border-input bg-transparent text-center">
              <p class="text-[10px] text-muted-foreground/60">0 = before first page</p>
            </div>
            <Button size="sm" onclick={executeInsertPages} disabled={busy}>
              {#if busy}<Loader2 size={14} class="animate-spin" />{:else}Insert Pages{/if}
            </Button>
          {/if}
        </div>
      {:else if tool === "pageNumber"}
        <div class="space-y-4">
          <p class="text-xs text-muted-foreground">Add page numbers to: <strong>{baseName(filePath!)}</strong></p>

          <div class="grid grid-cols-2 gap-3">
            <div class="space-y-1">
              <label class="text-xs text-muted-foreground">Number Format</label>
              <select bind:value={pageNumberFormat} class="w-full px-2 py-1 text-sm rounded border border-input bg-transparent">
                <option value={'Page {n} of {total}'}>Page {'{n}'} of {'{total}'}</option>
                <option value={'{n} / {total}'}>{'{n}'} / {'{total}'}</option>
                <option value={'Page {n}'}>Page {'{n}'}</option>
                <option value={'{n}'}>{'{n}'}</option>
                <option value={'- {n} -'}>- {'{n}'} -</option>
              </select>
            </div>

            <div class="space-y-1">
              <label class="text-xs text-muted-foreground">Position</label>
              <select bind:value={pageNumberPosition} class="w-full px-2 py-1 text-sm rounded border border-input bg-transparent">
                <option value="bottom-center">Bottom Center</option>
                <option value="bottom-right">Bottom Right</option>
                <option value="bottom-left">Bottom Left</option>
                <option value="top-center">Top Center</option>
                <option value="top-right">Top Right</option>
                <option value="top-left">Top Left</option>
              </select>
            </div>
          </div>

          <div class="grid grid-cols-4 gap-2">
            <div class="space-y-1">
              <label class="text-xs text-muted-foreground">Start Page</label>
              <input type="number" bind:value={pageNumberStartPage} min="1" class="w-full px-2 py-1 text-sm rounded border border-input bg-transparent text-center">
            </div>
            <div class="space-y-1">
              <label class="text-xs text-muted-foreground">First Number</label>
              <input type="number" bind:value={pageNumberStartNum} min="1" class="w-full px-2 py-1 text-sm rounded border border-input bg-transparent text-center">
            </div>
            <div class="space-y-1">
              <label class="text-xs text-muted-foreground">Font Size</label>
              <input type="number" bind:value={pageNumberFontSize} min="6" max="36" class="w-full px-2 py-1 text-sm rounded border border-input bg-transparent text-center">
            </div>
            <div class="space-y-1">
              <label class="text-xs text-muted-foreground">Color</label>
              <input type="color" bind:value={pageNumberColor} class="w-full h-8 rounded cursor-pointer border border-input">
            </div>
          </div>

          <Button size="sm" onclick={executeAddPageNumbers} disabled={busy}>
            {#if busy}<Loader2 size={14} class="animate-spin" />{:else}Apply Page Numbers & Save{/if}
          </Button>
        </div>
      {:else if tool === "sanitize"}
        <div class="space-y-4">
          <p class="text-xs text-muted-foreground">Sanitize document: <strong>{baseName(filePath!)}</strong></p>
          <div class="rounded-lg border border-border/80 bg-muted/20 p-3 text-xs space-y-2">
            <div class="font-medium text-foreground">Stirling-PDF Privacy Sanitizer will remove:</div>
            <ul class="list-disc list-inside space-y-1 text-muted-foreground">
              <li>Document metadata (Author, Title, Creator, Producer, Dates)</li>
              <li>Embedded XML/XMP metadata and application tracking streams</li>
              <li>Private PieceInfo editing logs from Adobe/third-party tools</li>
              <li>Automated actions (OpenAction, Javascript, Additional Actions)</li>
              <li>Unreferenced orphaned objects (fully scrubbed and compressed)</li>
            </ul>
          </div>
          <Button size="sm" onclick={executeSanitize} disabled={busy}>
            {#if busy}<Loader2 size={14} class="animate-spin" />{:else}Sanitize & Save As...{/if}
          </Button>
        </div>
      {:else if tool === "compress"}
        <div class="space-y-4">
          <p class="text-xs text-muted-foreground">Compress document: <strong>{baseName(filePath!)}</strong></p>

          <div class="space-y-2">
            <span class="text-xs font-semibold text-foreground">Select Compression Strategy:</span>
            <div class="grid grid-cols-2 gap-2">
              {#each compressionPresetList as preset}
                <button
                  type="button"
                  onclick={() => (compressPreset = preset.id)}
                  class="p-2.5 rounded-lg border text-left transition-all {compressPreset === preset.id
                    ? 'border-emerald-500 bg-emerald-500/10 shadow-xs'
                    : 'border-border bg-card hover:border-border/80'}"
                >
                  <div class="flex items-center justify-between mb-1">
                    <span class="text-xs font-semibold text-foreground">{preset.name}</span>
                    <span class="text-[10px] px-1.5 py-0.5 rounded bg-emerald-500/20 text-emerald-600 font-mono">{preset.badge}</span>
                  </div>
                  <p class="text-[11px] text-muted-foreground line-clamp-2 leading-relaxed">{preset.description}</p>
                </button>
              {/each}
            </div>
          </div>

          <div class="rounded-lg border border-border/80 bg-muted/20 p-3 text-xs space-y-1.5 text-muted-foreground">
            <div class="font-medium text-foreground text-xs flex items-center justify-between">
              <span>Flate Stream & Object Optimization:</span>
              <span class="text-[11px] font-mono text-emerald-600 font-semibold">100% Offline Atomic Write</span>
            </div>
            <p class="text-[11px]">Consolidates fonts, deflates streams and cleans orphaned structures without losing vector crispness.</p>
          </div>

          <Button size="sm" class="bg-emerald-600 hover:bg-emerald-700 text-white" onclick={executeCompress} disabled={busy}>
            {#if busy}<Loader2 size={14} class="animate-spin" />{:else}Compress & Save As...{/if}
          </Button>
        </div>
      {:else if tool === "pdf2text" || tool === "extractText"}
        <div class="space-y-3">
          <p class="text-xs text-muted-foreground">Extract text from: <strong>{baseName(filePath!)}</strong></p>
          <div class="flex flex-col gap-2">
            <div class="flex gap-1 items-center flex-wrap">
              <span class="text-xs text-muted-foreground mr-1">Scope:</span>
              <Button variant={textPageScope === "all" ? "default" : "outline"} size="sm" onclick={() => (textPageScope = "all")}>All pages</Button>
              <Button variant={textPageScope === "current" ? "default" : "outline"} size="sm" onclick={() => (textPageScope = "current")}>Current page ({currentPageNum})</Button>
              <Button variant={textPageScope === "custom" ? "default" : "outline"} size="sm" onclick={() => (textPageScope = "custom")}>Custom range</Button>
            </div>
            {#if textPageScope === "custom"}
              <div class="flex items-center gap-2">
                <input
                  type="text"
                  bind:value={textCustomRange}
                  placeholder="e.g. 1-3, 5"
                  class="w-48 px-2 py-1 text-xs rounded border border-input bg-transparent text-foreground font-mono"
                />
              </div>
            {/if}
          </div>
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
