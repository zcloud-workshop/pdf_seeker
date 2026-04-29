<script lang="ts">
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { invoke } from "@tauri-apps/api/core";
  import { Button, Input, Label, Tooltip } from "@/components/ui";
  import { t } from "@/i18n/index.svelte.ts";
  import {
    FolderOpen as Icon_FolderOpen,
    FileUp as Icon_FileUp,
    Loader2 as Icon_Loader2,
    CheckCircle as Icon_CheckCircle,
    AlertCircle as Icon_AlertCircle,
    Play,
    Square,
    RotateCcw,
    Trash2,
    FolderOutput,
  } from "lucide-svelte";
  import { readFile } from "@tauri-apps/plugin-fs";
  import { loadPdf } from "@/pdf-engine";

  type BatchOp =
    | "rotate"
    | "pdf2text"
    | "pdf2img"
    | "watermark"
    | "compress";

  let pdfFiles = $state<string[]>([]);
  let selectedOp = $state<BatchOp>("rotate");
  let busy = $state(false);
  let cancelled = $state(false);
  let cancelRequested = $state(false);

  // Rotate params
  let rotateAngle = $state(90);

  // Watermark params
  let watermarkText = $state("CONFIDENTIAL");
  let watermarkFontSize = $state(48);
  let watermarkOpacity = $state(0.3);
  let watermarkColor = $state("#cccccc");

  // Progress
  let currentFileIndex = $state(0);
  let totalFiles = $state(0);
  let startTime = $state(0);

  interface BatchResult {
    file: string;
    status: "pending" | "running" | "done" | "error" | "cancelled";
    result?: string;
    error?: string;
  }

  let results = $state<BatchResult[]>([]);

  let overallProgress = $derived(
    totalFiles > 0 ? Math.round((currentFileIndex / totalFiles) * 100) : 0,
  );

  let elapsedTime = $derived.by(() => {
    if (!busy && currentFileIndex === 0) return "";
    const ms = startTime > 0 ? Date.now() - startTime : 0;
    const s = Math.round(ms / 1000);
    if (s < 60) return `${s}s`;
    return `${Math.floor(s / 60)}m ${s % 60}s`;
  });

  // History
  let history = $state<Array<{ op: string; files: number; time: string; date: string }>>([]);

  async function addFiles() {
    const selected = await open({
      multiple: true,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
    if (selected) {
      const paths = Array.isArray(selected) ? selected.map(String) : [String(selected)];
      pdfFiles = [...pdfFiles, ...paths];
      results = pdfFiles.map((f) => ({ file: f, status: "pending" as const }));
    }
  }

  function removeFile(index: number) {
    pdfFiles = pdfFiles.filter((_, i) => i !== index);
    results = pdfFiles.map((f) => ({ file: f, status: "pending" as const }));
  }

  function clearFiles() {
    pdfFiles = [];
    results = [];
  }

  async function pickOutputDir() {
    return await open({ directory: true });
  }

  function baseName(p: string) {
    return p.split(/[\\/]/).pop() || "file";
  }

  function nameNoExt(p: string) {
    return baseName(p).replace(/\.pdf$/i, "");
  }

  function outputDir(path: string): string {
    const lastSep = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
    return lastSep > 0 ? path.substring(0, lastSep) : ".";
  }

  async function executeBatch() {
    if (pdfFiles.length === 0) return;
    busy = true;
    cancelled = false;
    cancelRequested = false;
    currentFileIndex = 0;
    totalFiles = pdfFiles.length;
    startTime = Date.now();
    results = pdfFiles.map((f) => ({ file: f, status: "pending" as const }));

    let completed = 0;
    let failed = 0;

    for (let i = 0; i < pdfFiles.length; i++) {
      if (cancelRequested) {
        results[i] = { file: pdfFiles[i], status: "cancelled" };
        continue;
      }

      results[i] = { file: pdfFiles[i], status: "running" };
      currentFileIndex = i + 1;

      try {
        const filePath = pdfFiles[i];
        const dir = outputDir(filePath);
        const name = nameNoExt(filePath);

        switch (selectedOp) {
          case "rotate": {
            const outPath = `${dir}/${name}_rotated.pdf`;
            await invoke("rotate_pdf", {
              req: { inputPath: filePath, outputPath: outPath, angle: rotateAngle },
            });
            results[i] = { file: filePath, status: "done", result: `Rotated ${rotateAngle}deg` };
            break;
          }
          case "pdf2text": {
            const data = await invoke<{ text: string; pages: number }>("extract_text", { path: filePath });
            const { writeTextFile } = await import("@tauri-apps/plugin-fs");
            await writeTextFile(`${dir}/${name}.txt`, data.text);
            results[i] = { file: filePath, status: "done", result: `${data.pages}p, ${data.text.length} chars` };
            break;
          }
          case "pdf2img": {
            const fileData = await readFile(filePath);
            const doc = await loadPdf(new Uint8Array(fileData));
            for (let p = 1; p <= doc.numPages; p++) {
              if (cancelRequested) break;
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
                await invoke("save_image_file", {
                  path: `${dir}/${name}_page_${p}.png`,
                  data: Array.from(new Uint8Array(ab)),
                });
              }
            }
            results[i] = {
              file: filePath,
              status: cancelRequested ? "cancelled" : "done",
              result: `${doc.numPages} pages -> PNG`,
            };
            break;
          }
          case "watermark": {
            const outPath = `${dir}/${name}_watermarked.pdf`;
            await invoke("add_text_watermark", {
              req: {
                inputPath: filePath,
                outputPath: outPath,
                text: watermarkText,
                fontSize: watermarkFontSize,
                opacity: watermarkOpacity,
                angle: 45,
                color: watermarkColor,
              },
            });
            results[i] = { file: filePath, status: "done", result: `Watermarked` };
            break;
          }
          case "compress": {
            const outPath = `${dir}/${name}_compressed.pdf`;
            const res = await invoke<{ originalSize: number; compressedSize: number; ratio: number }>("compress_pdf", {
              req: { inputPath: filePath, outputPath: outPath },
            });
            results[i] = {
              file: filePath,
              status: "done",
              result: `${formatBytes(res.originalSize)} -> ${formatBytes(res.compressedSize)} (${Math.round(res.ratio * 100)}%)`,
            };
            break;
          }
        }
        if (results[i].status === "done") completed++;
      } catch (e) {
        results[i] = { file: pdfFiles[i], status: "error", error: String(e) };
        failed++;
      }
    }

    currentFileIndex = totalFiles;
    busy = false;

    // Save to history
    const opNames: Record<BatchOp, string> = {
      rotate: "Rotate",
      pdf2text: "PDF to Text",
      pdf2img: "PDF to Image",
      watermark: "Watermark",
      compress: "Compress",
    };
    history = [
      {
        op: opNames[selectedOp],
        files: pdfFiles.length,
        time: elapsedTime,
        date: new Date().toLocaleTimeString(),
      },
      ...history.slice(0, 19),
    ];
  }

  function requestCancel() {
    cancelRequested = true;
  }

  function formatBytes(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  const opLabels: Record<BatchOp, string> = {
    rotate: "tools.rotate",
    pdf2text: "tools.convertText",
    pdf2img: "tools.pdf2img",
    watermark: "tools.watermark",
    compress: "tools.compress",
  };
</script>

<div class="space-y-3">
  <p class="text-sm text-muted-foreground">{t("tools.batchDesc")}</p>

  <!-- File selection -->
  <div class="flex gap-2">
    <Button variant="outline" size="sm" onclick={addFiles} class="gap-1.5" disabled={busy}>
      <Icon_FileUp size={14} />{t("batch.addFiles")}
    </Button>
    {#if pdfFiles.length > 0}
      <Button variant="ghost" size="sm" onclick={clearFiles} disabled={busy}>
        <Trash2 size={13} />
      </Button>
    {/if}
  </div>

  {#if pdfFiles.length > 0}
    <p class="text-xs text-muted-foreground">{pdfFiles.length} {t("batch.filesSelected")}</p>

    <!-- Operation selector -->
    <div class="space-y-1.5">
      <Label class="text-xs text-muted-foreground">{t("batch.operation")}</Label>
      <div class="flex flex-wrap gap-1.5">
        {#each Object.entries(opLabels) as [op, labelKey]}
          <Button
            variant={selectedOp === op ? "default" : "outline"}
            size="sm"
            onclick={() => (selectedOp = op as BatchOp)}
            disabled={busy}
          >
            {t(labelKey)}
          </Button>
        {/each}
      </div>
    </div>

    <!-- Operation-specific params -->
    {#if selectedOp === "rotate"}
      <div class="flex items-center gap-2">
        <Label class="shrink-0 text-xs">Angle</Label>
        <div class="flex gap-1">
          {#each [90, 180, 270] as angle}
            <Button variant={rotateAngle === angle ? "default" : "outline"} size="sm" onclick={() => (rotateAngle = angle)} disabled={busy}>{angle}deg</Button>
          {/each}
        </div>
      </div>
    {/if}

    {#if selectedOp === "watermark"}
      <div class="space-y-2">
        <div class="space-y-1">
          <Label class="text-xs text-muted-foreground">{t("tools.watermark")}</Label>
          <Input bind:value={watermarkText} class="w-full" disabled={busy} />
        </div>
        <div class="grid grid-cols-3 gap-2">
          <div class="space-y-1">
            <Label class="text-[10px] text-muted-foreground">Size</Label>
            <Input type="number" bind:value={watermarkFontSize} min="8" max="200" class="w-full" disabled={busy} />
          </div>
          <div class="space-y-1">
            <Label class="text-[10px] text-muted-foreground">Opacity</Label>
            <Input type="number" bind:value={watermarkOpacity} min="0.05" max="1" step="0.05" class="w-full" disabled={busy} />
          </div>
          <div class="space-y-1">
            <Label class="text-[10px] text-muted-foreground">Color</Label>
            <input type="color" bind:value={watermarkColor} class="w-full h-8 rounded cursor-pointer border border-input" disabled={busy} />
          </div>
        </div>
      </div>
    {/if}

    <!-- Execute / Cancel -->
    <div class="flex gap-2">
      {#if busy}
        <Button variant="destructive" size="sm" onclick={requestCancel} class="gap-1.5">
          <Square size={13} />{t("batch.cancel")}
        </Button>
      {:else}
        <Button onclick={executeBatch} class="gap-1.5">
          <Play size={13} />{t("batch.start")}
        </Button>
      {/if}
      {#if busy}
        <div class="flex-1 flex items-center gap-2">
          <div class="flex-1 bg-muted rounded-full h-1.5 overflow-hidden">
            <div class="bg-primary h-1.5 rounded-full transition-all duration-300" style="width: {overallProgress}%"></div>
          </div>
          <span class="text-xs text-muted-foreground tabular-nums whitespace-nowrap">{overallProgress}%</span>
          <span class="text-xs text-muted-foreground/60 tabular-nums">{elapsedTime}</span>
        </div>
      {/if}
    </div>

    <!-- Results -->
    {#if results.length > 0}
      <div class="space-y-1 max-h-[40vh] overflow-auto">
        {#each results as r, i}
          <div class="flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs {r.status === 'done'
              ? 'text-green-600 dark:text-green-400'
              : r.status === 'error'
                ? 'text-destructive'
                : r.status === 'running'
                  ? 'text-muted-foreground'
                  : r.status === 'cancelled'
                    ? 'text-amber-600 dark:text-amber-400'
                    : 'text-muted-foreground'}">
            {#if r.status === "running"}
              <Icon_Loader2 size={12} class="animate-spin" />
            {:else if r.status === "done"}
              <Icon_CheckCircle size={12} />
            {:else if r.status === "error"}
              <Icon_AlertCircle size={12} />
            {:else if r.status === "cancelled"}
              <Square size={10} />
            {:else}
              <span class="w-3"></span>
            {/if}
            <span class="flex-1 truncate">{baseName(r.file)}</span>
            <span class="text-[10px] shrink-0">{r.result || r.error || ""}</span>
          </div>
        {/each}
      </div>
    {/if}
  {/if}

  <!-- History -->
  {#if history.length > 0}
    <div class="pt-2 border-t border-border">
      <div class="flex items-center justify-between mb-1.5">
        <span class="text-xs font-medium text-muted-foreground">{t("batch.history")}</span>
        {#if history.length > 0}
          <button onclick={() => (history = [])} class="text-[10px] text-muted-foreground/60 hover:text-muted-foreground">{t("batch.clearHistory")}</button>
        {/if}
      </div>
      <div class="space-y-0.5">
        {#each history as h}
          <div class="flex items-center gap-2 text-[10px] text-muted-foreground/70">
            <Icon_CheckCircle size={10} class="text-green-500/50" />
            <span>{h.op}</span>
            <span>{h.files} files</span>
            <span class="flex-1"></span>
            <span>{h.time}</span>
            <span>{h.date}</span>
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>
