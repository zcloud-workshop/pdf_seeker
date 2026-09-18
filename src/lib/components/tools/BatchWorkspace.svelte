<script lang="ts">
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { invoke } from "@tauri-apps/api/core";
  import { readFile, writeTextFile } from "@tauri-apps/plugin-fs";
  import { Button, Input, Label, Tooltip } from "@/components/ui";
  import { t } from "@/i18n/index.svelte.ts";
  import { loadPdf } from "@/pdf-engine";
  import {
    Plus,
    Trash2,
    Play,
    Square,
    RotateCcw,
    FolderOpen,
    FolderOutput,
    FileText,
    CheckCircle2,
    AlertCircle,
    Loader2,
    Download,
    Layers,
    FileUp,
    Minimize2,
    Stamp,
    Image,
    Sparkles,
    ShieldCheck,
  } from "lucide-svelte";
  import {
    type RecipeType,
    type RecipeSpec,
    type BatchInputFile,
    type JobItemResult,
    type JobSummary,
    type ConflictStrategy,
    resolveOutputPath,
    formatBytes,
    generateJobReport,
  } from "@/models/job-engine";
  import { COMPRESSION_PRESETS, estimateSavings, type CompressionPresetInfo } from "@/models/compression";
  import { privacyAuditor } from "@/models/privacy";
  import { currentFilePath, currentFileName } from "@/stores";
  import { executeSmartPdfCompression } from "@/utils/pdf-compressor";

  const presetList: CompressionPresetInfo[] = Object.values(COMPRESSION_PRESETS);

  // --- Input Files (Zone 1) ---
  let inputFiles = $state<BatchInputFile[]>([]);

  // --- Recipe & Params (Zone 2) ---
  let selectedRecipeType = $state<RecipeType>("compress");

  // Recipe parameters
  let rotateAngle = $state<90 | 180 | 270>(90);
  let watermarkText = $state("CONFIDENTIAL");
  let watermarkFontSize = $state(48);
  let watermarkOpacity = $state(0.3);
  let watermarkColor = $state("#cccccc");
  let watermarkAngle = $state(45);
  let imgScale = $state<1 | 1.5 | 2>(2);
  let imgFormat = $state<"png" | "jpeg">("png");
  let compressPreset = $state<"balanced" | "high_quality" | "max_compression" | "grayscale">("balanced");

  // Output policies
  let outputDirMode = $state<"same" | "custom">("same");
  let customOutputDir = $state<string>("");
  let namingTemplate = $state("{name}_{suffix}");
  let conflictStrategy = $state<ConflictStrategy>("rename");

  // --- Live Execution State (Zone 3) ---
  let isRunning = $state(false);
  let cancelRequested = $state(false);
  let currentIndex = $state(0);
  let startTime = $state(0);
  let items = $state<JobItemResult[]>([]);

  // --- Completed Summary (Zone 4) ---
  let completedSummary = $state<JobSummary | null>(null);

  // Derived metrics
  let totalFiles = $derived(inputFiles.length);
  let progressPercent = $derived(
    totalFiles > 0 ? Math.round((currentIndex / totalFiles) * 100) : 0
  );
  let totalInputBytes = $derived(inputFiles.reduce((acc, f) => acc + f.sizeBytes, 0));

  let elapsedTimeStr = $derived.by(() => {
    if (!isRunning && startTime === 0) return "0s";
    const ms = Date.now() - startTime;
    const s = Math.round(ms / 1000);
    if (s < 60) return `${s}s`;
    return `${Math.floor(s / 60)}m ${s % 60}s`;
  });

  // --- File Management ---
  async function handleAddCurrentFile() {
    if (!$currentFilePath) return;
    const path = $currentFilePath;
    if (inputFiles.some((f) => f.path === path)) return;

    const norm = path.replace(/\\/g, "/");
    const name = $currentFileName || norm.split("/").pop() || "document.pdf";
    let size = 0;
    try {
      const info = await invoke<any>("get_pdf_info", { path });
      size = info.fileSize ?? info.file_size ?? 0;
    } catch (_) {
      // fallback
    }
    inputFiles = [...inputFiles, { path, name, sizeBytes: size }];
    resetQueue();
  }

  async function handleAddFiles() {
    const selected = await open({
      multiple: true,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
    if (!selected) return;

    const paths = Array.isArray(selected) ? selected.map(String) : [String(selected)];
    for (const path of paths) {
      if (!inputFiles.some((f) => f.path === path)) {
        const norm = path.replace(/\\/g, "/");
        const name = norm.split("/").pop() || "document.pdf";
        let size = 0;
        try {
          const info = await invoke<any>("get_pdf_info", { path });
          size = info.fileSize ?? info.file_size ?? 0;
        } catch (_) {
          // fallback
        }
        inputFiles = [...inputFiles, { path, name, sizeBytes: size }];
      }
    }
    resetQueue();
  }

  function handleRemoveFile(index: number) {
    inputFiles = inputFiles.filter((_, i) => i !== index);
    resetQueue();
  }

  function handleClearFiles() {
    inputFiles = [];
    resetQueue();
  }

  function resetQueue() {
    items = inputFiles.map((f) => ({
      filePath: f.path,
      fileName: f.name,
      fileSizeBytes: f.sizeBytes,
      status: "pending",
    }));
    currentIndex = 0;
    completedSummary = null;
  }

  async function handlePickOutputDir() {
    const dir = await open({ directory: true });
    if (dir && typeof dir === "string") {
      customOutputDir = dir;
      outputDirMode = "custom";
    }
  }

  // --- Execution Engine (Unified Job Runner) ---
  async function startBatchJob() {
    if (inputFiles.length === 0 || isRunning) return;

    isRunning = true;
    cancelRequested = false;
    startTime = Date.now();
    currentIndex = 0;

    resetQueue();

    const existingOutputs = new Set<string>();
    let totalOutputBytes = 0;
    let completedCount = 0;
    let failedCount = 0;
    let skippedCount = 0;

    for (let i = 0; i < inputFiles.length; i++) {
      if (cancelRequested) {
        items[i].status = "cancelled";
        items[i].message = "用户手动取消";
        continue;
      }

      currentIndex = i + 1;
      items[i].status = "running";
      const startItemTime = Date.now();
      const file = inputFiles[i];

      try {
        const targetDir = outputDirMode === "custom" && customOutputDir ? customOutputDir : "same";
        let outSuffix = selectedRecipeType;
        let outExt = "pdf";

        if (selectedRecipeType === "pdf2text") outExt = "txt";
        if (selectedRecipeType === "pdf2img") outExt = imgFormat;

        const outputPath = resolveOutputPath(
          file.path,
          targetDir,
          namingTemplate,
          outSuffix,
          outExt,
          conflictStrategy,
          existingOutputs
        );
        existingOutputs.add(outputPath);

        // Run operation
        switch (selectedRecipeType) {
          case "rotate": {
            await invoke("rotate_pdf", {
              req: { inputPath: file.path, outputPath, angle: rotateAngle },
            });
            items[i].outputPath = outputPath;
            items[i].outputSizeBytes = file.sizeBytes; // Approx
            items[i].message = `已旋转 ${rotateAngle}°`;
            break;
          }
          case "watermark": {
            await invoke("add_text_watermark", {
              req: {
                inputPath: file.path,
                outputPath,
                text: watermarkText,
                fontSize: watermarkFontSize,
                opacity: watermarkOpacity,
                color: watermarkColor,
                angle: watermarkAngle,
              },
            });
            items[i].outputPath = outputPath;
            items[i].message = "已添加防伪水印";
            break;
          }
          case "compress": {
            const res = await executeSmartPdfCompression(
              file.path,
              outputPath,
              compressPreset
            );
            items[i].outputPath = outputPath;
            items[i].outputSizeBytes = res.compressedSize;
            items[i].ratio = res.ratio;
            totalOutputBytes += res.compressedSize;
            items[i].message = `节省 ${res.ratio.toFixed(1)}% (${formatBytes(res.originalSize)} → ${formatBytes(res.compressedSize)})`;
            break;
          }
          case "pdf2text": {
            const data = await invoke<{ text: string; pages: number }>("extract_text", { path: file.path });
            await writeTextFile(outputPath, data.text);
            items[i].outputPath = outputPath;
            items[i].outputSizeBytes = data.text.length;
            items[i].message = `提取 ${data.pages} 页，${data.text.length} 字符`;
            break;
          }
          case "pdf2img": {
            const bytes = await readFile(file.path);
            const doc = await loadPdf(new Uint8Array(bytes));
            for (let p = 1; p <= doc.numPages; p++) {
              if (cancelRequested) break;
              const page = await doc.getPage(p);
              const vp = page.getViewport({ scale: imgScale });
              const canvas = document.createElement("canvas");
              canvas.width = Math.floor(vp.width);
              canvas.height = Math.floor(vp.height);
              const ctx = canvas.getContext("2d")!;
              await page.render({ canvasContext: ctx, viewport: vp }).promise;
              const blob = await new Promise<Blob | null>((r) => canvas.toBlob(r, `image/${imgFormat}`));
              if (blob) {
                const ab = await blob.arrayBuffer();
                const pageOut = outputPath.replace(new RegExp(`\\.${imgFormat}$`), `_p${p}.${imgFormat}`);
                await invoke("save_image_file", {
                  path: pageOut,
                  data: Array.from(new Uint8Array(ab)),
                });
              }
            }
            items[i].outputPath = outputPath;
            items[i].message = `导出 ${doc.numPages} 张 ${imgFormat.toUpperCase()} 图像`;
            break;
          }
        }

        items[i].status = "done";
        items[i].durationMs = Date.now() - startItemTime;
        completedCount++;
        privacyAuditor.logAction(`批量流水线: ${selectedRecipeType}`, file.name);
      } catch (err: any) {
        items[i].status = "error";
        items[i].error = err?.message || String(err);
        failedCount++;
      }
    }

    isRunning = false;
    currentIndex = inputFiles.length;

    // Build final summary
    const endTime = Date.now();
    const recipeNames: Record<RecipeType, string> = {
      rotate: "批量旋转",
      watermark: "批量水印",
      compress: "批量压缩优化",
      pdf2text: "批量提取纯文本",
      pdf2img: "批量转图片",
      extract: "批量抽取页面",
    };

    completedSummary = {
      id: "job_" + Math.random().toString(36).substring(2, 9),
      recipeName: recipeNames[selectedRecipeType],
      recipeType: selectedRecipeType,
      totalFiles: inputFiles.length,
      completedFiles: completedCount,
      failedFiles: failedCount,
      skippedFiles: skippedCount,
      startTime,
      endTime,
      totalOriginalBytes: totalInputBytes,
      totalOutputBytes: totalOutputBytes || totalInputBytes,
      bytesSaved: Math.max(0, totalInputBytes - totalOutputBytes),
      items: [...items],
    };
  }

  // Retry a single failed item
  async function retrySingleItem(index: number) {
    if (isRunning || index < 0 || index >= items.length) return;
    const file = inputFiles[index];
    items[index].status = "running";
    items[index].error = undefined;
    const startItemTime = Date.now();

    try {
      const targetDir = outputDirMode === "custom" && customOutputDir ? customOutputDir : "same";
      const outputPath = resolveOutputPath(
        file.path,
        targetDir,
        namingTemplate,
        selectedRecipeType,
        "pdf",
        conflictStrategy
      );

      if (selectedRecipeType === "compress") {
        const res = await executeSmartPdfCompression(
          file.path,
          outputPath,
          compressPreset
        );
        items[index].outputPath = outputPath;
        items[index].outputSizeBytes = res.compressedSize;
        items[index].ratio = res.ratio;
        items[index].message = `重试成功: 节省 ${res.ratio.toFixed(1)}%`;
      } else {
        await invoke("rotate_pdf", {
          req: { inputPath: file.path, outputPath, angle: rotateAngle },
        });
        items[index].outputPath = outputPath;
        items[index].message = "重试成功";
      }

      items[index].status = "done";
      items[index].durationMs = Date.now() - startItemTime;
    } catch (e: any) {
      items[index].status = "error";
      items[index].error = e?.message || String(e);
    }
  }

  // Export report
  async function handleExportReport(format: "csv" | "markdown") {
    if (!completedSummary) return;
    const content = generateJobReport(completedSummary, format);
    const filePath = await save({
      filters: [
        {
          name: format === "csv" ? "CSV 报表" : "Markdown 报告",
          extensions: [format === "csv" ? "csv" : "md"],
        },
      ],
      defaultPath: `PDF_Seeker_Batch_Report_${new Date().toISOString().slice(0, 10)}.${format === "csv" ? "csv" : "md"}`,
    });
    if (filePath) {
      await writeTextFile(filePath, content);
    }
  }
</script>

<div class="h-full flex flex-col bg-background select-none overflow-y-auto p-5">
  <div class="max-w-6xl w-full mx-auto space-y-5">
    <!-- Header Banner -->
    <div class="flex items-center justify-between pb-2 border-b border-border">
      <div>
        <div class="flex items-center gap-2">
          <span class="p-1.5 rounded-lg bg-primary/10 text-primary">
            <Layers size={20} />
          </span>
          <h2 class="text-xl font-bold tracking-tight text-foreground">批处理工作台 (Batch Workspace)</h2>
          <span class="text-xs px-2 py-0.5 rounded-full bg-emerald-500/10 text-emerald-500 font-medium flex items-center gap-1">
            <ShieldCheck size={12} />
            100% 离线事务写入
          </span>
        </div>
        <p class="text-xs text-muted-foreground mt-1">
          将重复流水线保存为配方，批量执行旋转、水印、压缩与转换；支持故障隔离、单项重试与可复现报告导出。
        </p>
      </div>

      {#if completedSummary}
        <div class="flex items-center gap-2">
          <Button variant="outline" size="sm" class="gap-1.5 text-xs" onclick={() => handleExportReport("csv")}>
            <Download size={14} />
            <span>导出 CSV 报表</span>
          </Button>
          <Button variant="outline" size="sm" class="gap-1.5 text-xs" onclick={() => handleExportReport("markdown")}>
            <Download size={14} />
            <span>导出 Markdown 总结</span>
          </Button>
        </div>
      {/if}
    </div>

    <!-- 4-Zone Grid -->
    <div class="grid grid-cols-1 lg:grid-cols-12 gap-5">
      <!-- ZONE 1: Input Files Management (Left Column 5 cols) -->
      <div class="lg:col-span-5 flex flex-col gap-3.5 bg-card border border-border rounded-xl p-4 shadow-sm">
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-2">
            <FileUp size={16} class="text-primary" />
            <span class="text-sm font-semibold text-foreground">输入文件清单</span>
            <span class="text-xs text-muted-foreground">({inputFiles.length} 个)</span>
          </div>
          <div class="flex items-center gap-1.5">
            {#if $currentFilePath && !inputFiles.some((f) => f.path === $currentFilePath)}
              <Button variant="secondary" size="sm" class="h-7 text-xs gap-1" onclick={handleAddCurrentFile} disabled={isRunning}>
                <Plus size={13} />
                <span>当前文档</span>
              </Button>
            {/if}
            <Button variant="outline" size="sm" class="h-7 text-xs gap-1" onclick={handleAddFiles} disabled={isRunning}>
              <Plus size={13} />
              <span>添加 PDF</span>
            </Button>
            {#if inputFiles.length > 0}
              <Button variant="ghost" size="sm" class="h-7 px-2 text-rose-500 hover:text-rose-600 hover:bg-rose-500/10 text-xs" onclick={handleClearFiles} disabled={isRunning}>
                <Trash2 size={13} />
              </Button>
            {/if}
          </div>
        </div>

        <!-- Files Scroll Container -->
        <div class="flex-1 min-h-[260px] max-h-[380px] overflow-y-auto rounded-lg border border-border/80 bg-muted/20 p-2 space-y-1.5">
          {#if inputFiles.length === 0}
            <div class="h-full min-h-[220px] flex flex-col items-center justify-center text-center p-4 text-muted-foreground">
              <FolderOpen size={36} class="stroke-1 text-muted-foreground/50 mb-2" />
              <p class="text-xs font-medium">尚未添加 PDF 文件</p>
              <p class="text-[11px] text-muted-foreground/70 mt-0.5">点击右上角“添加 PDF”或选取多份文档</p>
              {#if $currentFilePath}
                <Button variant="outline" size="sm" class="mt-3 h-7 text-xs gap-1.5 text-primary" onclick={handleAddCurrentFile}>
                  <Plus size={13} />
                  <span>添加当前正在查看的文档</span>
                </Button>
              {/if}
            </div>
          {:else}
            {#each inputFiles as file, idx}
              <div class="flex items-center justify-between p-2 rounded-md bg-card border border-border text-xs group hover:border-primary/40 transition-colors">
                <div class="flex items-center gap-2 min-w-0 pr-2">
                  <span class="text-[10px] font-mono text-muted-foreground w-4 text-center">{idx + 1}</span>
                  <FileText size={15} class="text-primary shrink-0" />
                  <div class="min-w-0">
                    <div class="font-medium text-foreground truncate" title={file.path}>{file.name}</div>
                    <div class="text-[10px] text-muted-foreground">{formatBytes(file.sizeBytes)}</div>
                  </div>
                </div>
                {#if !isRunning}
                  <button
                    onclick={() => handleRemoveFile(idx)}
                    class="opacity-0 group-hover:opacity-100 p-1 rounded hover:bg-rose-500/10 text-muted-foreground hover:text-rose-500 transition-all shrink-0"
                    title="移除此项"
                  >
                    <Trash2 size={13} />
                  </button>
                {/if}
              </div>
            {/each}
          {/if}
        </div>

        {#if inputFiles.length > 0}
          <div class="flex items-center justify-between text-xs text-muted-foreground px-1 pt-1 border-t border-border/60">
            <span>总待处理容量:</span>
            <span class="font-mono font-medium text-foreground">{formatBytes(totalInputBytes)}</span>
          </div>
        {/if}
      </div>

      <!-- ZONE 2: Recipe & Rules Configuration (Right Column 7 cols) -->
      <div class="lg:col-span-7 flex flex-col gap-4 bg-card border border-border rounded-xl p-4 shadow-sm">
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-2">
            <Sparkles size={16} class="text-amber-500" />
            <span class="text-sm font-semibold text-foreground">配方与规则定制 (Recipe & Policy)</span>
          </div>
        </div>

        <!-- Recipe Selector Chips -->
        <div class="grid grid-cols-2 sm:grid-cols-4 gap-2">
          <button
            onclick={() => (selectedRecipeType = "compress")}
            class="flex flex-col items-center p-2.5 rounded-lg border text-xs transition-all {selectedRecipeType === 'compress'
              ? 'border-primary bg-primary/10 text-primary font-semibold shadow-xs'
              : 'border-border bg-background text-muted-foreground hover:border-border/80'}"
          >
            <Minimize2 size={16} class="mb-1" />
            <span>批量压缩</span>
          </button>

          <button
            onclick={() => (selectedRecipeType = "rotate")}
            class="flex flex-col items-center p-2.5 rounded-lg border text-xs transition-all {selectedRecipeType === 'rotate'
              ? 'border-primary bg-primary/10 text-primary font-semibold shadow-xs'
              : 'border-border bg-background text-muted-foreground hover:border-border/80'}"
          >
            <RotateCcw size={16} class="mb-1" />
            <span>批量旋转</span>
          </button>

          <button
            onclick={() => (selectedRecipeType = "watermark")}
            class="flex flex-col items-center p-2.5 rounded-lg border text-xs transition-all {selectedRecipeType === 'watermark'
              ? 'border-primary bg-primary/10 text-primary font-semibold shadow-xs'
              : 'border-border bg-background text-muted-foreground hover:border-border/80'}"
          >
            <Stamp size={16} class="mb-1" />
            <span>批量水印</span>
          </button>

          <button
            onclick={() => (selectedRecipeType = "pdf2text")}
            class="flex flex-col items-center p-2.5 rounded-lg border text-xs transition-all {selectedRecipeType === 'pdf2text'
              ? 'border-primary bg-primary/10 text-primary font-semibold shadow-xs'
              : 'border-border bg-background text-muted-foreground hover:border-border/80'}"
          >
            <FileText size={16} class="mb-1" />
            <span>提取纯文本</span>
          </button>
        </div>

        <!-- Dynamic Recipe Configuration Details -->
        <div class="p-3.5 rounded-lg border border-border/80 bg-muted/20 space-y-3 text-xs">
          {#if selectedRecipeType === "compress"}
            <div>
              <Label class="text-xs font-medium mb-1.5 block">压缩优化预设策略：</Label>
              <div class="grid grid-cols-2 gap-2">
                {#each presetList as preset}
                  <button
                    onclick={() => (compressPreset = preset.id)}
                    class="p-2.5 rounded-md border text-left transition-all {compressPreset === preset.id
                      ? 'border-emerald-500 bg-emerald-500/10 text-foreground'
                      : 'border-border bg-card text-muted-foreground hover:border-border/80'}"
                  >
                    <div class="flex items-center justify-between mb-1">
                      <span class="font-semibold text-foreground text-xs">{preset.name}</span>
                      <span class="text-[10px] px-1 py-0.2 rounded bg-emerald-500/20 text-emerald-600 font-mono">{preset.badge}</span>
                    </div>
                    <p class="text-[11px] text-muted-foreground line-clamp-2 leading-relaxed">{preset.description}</p>
                  </button>
                {/each}
              </div>
            </div>
          {:else if selectedRecipeType === "rotate"}
            <div class="space-y-2">
              <Label class="text-xs font-medium block">旋转方向与角度：</Label>
              <div class="flex gap-2">
                <Button size="sm" variant={rotateAngle === 90 ? "default" : "outline"} onclick={() => (rotateAngle = 90)}>顺时针 90°</Button>
                <Button size="sm" variant={rotateAngle === 180 ? "default" : "outline"} onclick={() => (rotateAngle = 180)}>倒转 180°</Button>
                <Button size="sm" variant={rotateAngle === 270 ? "default" : "outline"} onclick={() => (rotateAngle = 270)}>逆时针 90° (270°)</Button>
              </div>
            </div>
          {:else if selectedRecipeType === "watermark"}
            <div class="grid grid-cols-2 gap-3">
              <div>
                <Label class="text-[11px] font-medium block mb-1">水印文字：</Label>
                <input
                  type="text"
                  bind:value={watermarkText}
                  class="w-full px-2.5 py-1.5 text-xs rounded border border-input bg-card text-foreground"
                />
              </div>
              <div>
                <Label class="text-[11px] font-medium block mb-1">不透明度 ({Math.round(watermarkOpacity * 100)}%)：</Label>
                <input
                  type="range"
                  min="0.1"
                  max="0.8"
                  step="0.05"
                  bind:value={watermarkOpacity}
                  class="w-full h-2 rounded bg-muted accent-primary"
                />
              </div>
            </div>
          {:else if selectedRecipeType === "pdf2text"}
            <p class="text-muted-foreground">提取每份 PDF 全部页面的字符串内容，并自动保存为规整的 `.txt` 文本文件。</p>
          {/if}

          <!-- Output Policies -->
          <div class="pt-2 border-t border-border/60 grid grid-cols-2 gap-3">
            <div>
              <Label class="text-[11px] font-medium block mb-1">命名模版 (支持 {'{name}'}, {'{suffix}'})：</Label>
              <input
                type="text"
                bind:value={namingTemplate}
                placeholder="{'{name}_{suffix}'}"
                class="w-full px-2 py-1 text-xs rounded border border-input bg-card text-foreground font-mono"
              />
            </div>
            <div>
              <Label class="text-[11px] font-medium block mb-1">同名冲突处理：</Label>
              <select
                bind:value={conflictStrategy}
                class="w-full px-2 py-1 text-xs rounded border border-input bg-card text-foreground"
              >
                <option value="rename">自动附加序号重命名 (_1, _2)</option>
                <option value="replace">覆盖同名文件</option>
                <option value="fail">已存在则跳过</option>
              </select>
            </div>
          </div>
        </div>

        <!-- Action Runner Controls -->
        <div class="flex items-center justify-between pt-1">
          <div class="text-xs text-muted-foreground">
            {#if isRunning}
              <span class="text-primary font-medium flex items-center gap-1.5">
                <Loader2 size={14} class="animate-spin" />
                正在处理 {currentIndex} / {totalFiles} (已耗时 {elapsedTimeStr})
              </span>
            {:else if inputFiles.length > 0}
              <span>就绪: {totalFiles} 个文档待处理</span>
            {:else}
              <span>请先在左侧添加 PDF 文档</span>
            {/if}
          </div>

          <div class="flex items-center gap-2">
            {#if isRunning}
              <Button variant="destructive" size="sm" class="gap-1.5 text-xs" onclick={() => (cancelRequested = true)}>
                <Square size={14} />
                <span>中止执行</span>
              </Button>
            {:else}
              <Button size="sm" class="gap-1.5 text-xs px-4" onclick={startBatchJob} disabled={inputFiles.length === 0}>
                <Play size={14} />
                <span>开始批量执行流水线</span>
              </Button>
            {/if}
          </div>
        </div>
      </div>
    </div>

    <!-- ZONE 3 & 4: Queue, Progress & Results (Bottom Wide Table) -->
    {#if items.length > 0}
      <div class="bg-card border border-border rounded-xl p-4 shadow-sm space-y-3">
        <!-- Progress Bar -->
        <div class="flex items-center justify-between text-xs">
          <span class="font-semibold text-foreground">任务处理进度 ({progressPercent}%)</span>
          <span class="font-mono text-muted-foreground">{currentIndex} / {totalFiles} 完成</span>
        </div>
        <div class="w-full h-2 rounded-full bg-muted overflow-hidden">
          <div
            class="h-full bg-primary transition-all duration-200"
            style="width: {progressPercent}%;"
          ></div>
        </div>

        <!-- Items Table -->
        <div class="overflow-x-auto rounded-lg border border-border">
          <table class="w-full text-xs text-left">
            <thead class="bg-muted/50 border-b border-border text-muted-foreground font-medium">
              <tr>
                <th class="p-2.5 w-10 text-center">#</th>
                <th class="p-2.5">文件名</th>
                <th class="p-2.5 w-24">原大小</th>
                <th class="p-2.5 w-28">状态</th>
                <th class="p-2.5">产物详情</th>
                <th class="p-2.5 w-20 text-right">操作</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-border">
              {#each items as item, idx}
                <tr class="hover:bg-muted/20 transition-colors">
                  <td class="p-2.5 text-center font-mono text-muted-foreground">{idx + 1}</td>
                  <td class="p-2.5 font-medium text-foreground max-w-[200px] truncate" title={item.filePath}>
                    {item.fileName}
                  </td>
                  <td class="p-2.5 font-mono text-muted-foreground">{formatBytes(item.fileSizeBytes)}</td>
                  <td class="p-2.5">
                    {#if item.status === "pending"}
                      <span class="inline-flex items-center gap-1 text-muted-foreground">
                        <span class="w-1.5 h-1.5 rounded-full bg-muted-foreground"></span> 等待中
                      </span>
                    {:else if item.status === "running"}
                      <span class="inline-flex items-center gap-1 text-primary font-medium">
                        <Loader2 size={12} class="animate-spin" /> 处理中...
                      </span>
                    {:else if item.status === "done"}
                      <span class="inline-flex items-center gap-1 text-emerald-500 font-medium">
                        <CheckCircle2 size={13} /> 完成
                      </span>
                    {:else if item.status === "error"}
                      <span class="inline-flex items-center gap-1 text-rose-500 font-medium" title={item.error}>
                        <AlertCircle size={13} /> 失败
                      </span>
                    {:else if item.status === "cancelled"}
                      <span class="text-muted-foreground">已取消</span>
                    {/if}
                  </td>
                  <td class="p-2.5 text-muted-foreground">
                    {#if item.error}
                      <span class="text-rose-500 line-clamp-1">{item.error}</span>
                    {:else if item.message}
                      <span class="line-clamp-1 text-foreground/90 font-mono text-[11px]">{item.message}</span>
                    {:else}
                      -
                    {/if}
                  </td>
                  <td class="p-2.5 text-right">
                    {#if item.status === "error" && !isRunning}
                      <Button variant="ghost" size="sm" class="h-6 px-2 text-[11px] text-amber-500 hover:text-amber-600 hover:bg-amber-500/10" onclick={() => retrySingleItem(idx)}>
                        重试
                      </Button>
                    {:else if item.status === "done"}
                      <span class="text-[10px] text-muted-foreground font-mono">{item.durationMs || 0}ms</span>
                    {/if}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      </div>
    {/if}
  </div>
</div>
