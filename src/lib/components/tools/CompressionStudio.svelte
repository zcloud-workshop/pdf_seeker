<script lang="ts">
  import { onMount } from "svelte";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { invoke } from "@tauri-apps/api/core";
  import { Button, Label } from "@/components/ui";
  import {
    Minimize2,
    FolderOpen,
    CheckCircle2,
    AlertCircle,
    Loader2,
    Sparkles,
    ArrowRight,
    TrendingDown,
    HardDrive,
    ShieldCheck,
    ExternalLink,
  } from "lucide-svelte";
  import {
    COMPRESSION_PRESETS,
    estimateSavings,
    formatCompressionSummary,
    type CompressionPresetInfo,
  } from "@/models/compression";
  import { formatBytes } from "@/models/job-engine";
  import { privacyAuditor } from "@/models/privacy";
  import { currentFilePath, currentFileName, currentView } from "@/stores";
  import { executeSmartPdfCompression } from "@/utils/pdf-compressor";

  const presetList: CompressionPresetInfo[] = Object.values(COMPRESSION_PRESETS);

  let filePath = $state<string | null>($currentFilePath || null);
  let fileName = $state<string>(
    $currentFileName || ($currentFilePath ? $currentFilePath.replace(/\\/g, "/").split("/").pop() || "document.pdf" : "")
  );
  let fileSizeBytes = $state<number>(0);
  let imageCount = $state<number | null>(null);
  let selectedPreset = $state<CompressionPresetInfo["id"]>("balanced");

  let isCompressing = $state(false);
  let compressionProgress = $state(0);
  let result = $state<{
    originalSize: number;
    compressedSize: number;
    ratio: number;
    outputPath: string;
  } | null>(null);
  let errorMsg = $state<string | null>(null);

  let currentPreset = $derived(COMPRESSION_PRESETS[selectedPreset]);
  let estimated = $derived(
    fileSizeBytes > 0
      ? estimateSavings(fileSizeBytes, selectedPreset, imageCount ?? undefined)
      : {
          minSaving: 0,
          maxSaving: 0,
          minFinal: 0,
          maxFinal: 0,
          ratioRange: `${currentPreset.estimatedSavingMin}% ~ ${currentPreset.estimatedSavingMax}%`,
          isVectorTextOnly: false,
        }
  );

  async function fetchFileInfo(path: string) {
    try {
      const info = await invoke<any>("get_pdf_info", { path });
      fileSizeBytes = info.fileSize ?? info.file_size ?? 0;
      imageCount = info.imageCount ?? info.image_count ?? 0;
    } catch (_) {
      fileSizeBytes = 0;
      imageCount = null;
    }
  }

  $effect(() => {
    if ($currentFilePath && !filePath) {
      filePath = $currentFilePath;
      fileName = $currentFileName || $currentFilePath.replace(/\\/g, "/").split("/").pop() || "document.pdf";
      fetchFileInfo($currentFilePath);
    }
  });

  onMount(() => {
    if (filePath) {
      fetchFileInfo(filePath);
    }
  });

  async function handlePickFile() {
    const selected = await open({
      multiple: false,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
    if (selected && typeof selected === "string") {
      filePath = selected;
      fileName = selected.replace(/\\/g, "/").split("/").pop() || "document.pdf";
      result = null;
      errorMsg = null;
      await fetchFileInfo(selected);
    }
  }

  function handleOpenInViewer() {
    if (result?.outputPath) {
      currentFilePath.set(result.outputPath);
      currentFileName.set(result.outputPath.replace(/\\/g, "/").split("/").pop() || "compressed.pdf");
      currentView.set("editor");
    }
  }

  async function executeCompression() {
    if (!filePath || isCompressing) return;
    isCompressing = true;
    errorMsg = null;

    try {
      const norm = filePath.replace(/\\/g, "/");
      const dir = norm.substring(0, norm.lastIndexOf("/"));
      const rawName = fileName.replace(/\.pdf$/i, "");
      const suggestedOut = `${dir}/${rawName}_${selectedPreset}_compressed.pdf`;

      const targetPath = await save({
        defaultPath: suggestedOut,
        filters: [{ name: "PDF", extensions: ["pdf"] }],
      });

      if (!targetPath) {
        isCompressing = false;
        return;
      }

      compressionProgress = 0;
      const res = await executeSmartPdfCompression(
        filePath,
        targetPath,
        selectedPreset,
        (percent) => {
          compressionProgress = percent;
        },
        imageCount ?? undefined
      );

      result = res;
      privacyAuditor.logAction(`可控压缩优化 (${currentPreset.name} 节省 ${res.ratio.toFixed(1)}%)`, fileName);
    } catch (e: any) {
      errorMsg = e?.message || String(e);
    } finally {
      isCompressing = false;
    }
  }
</script>

<div class="h-full flex flex-col bg-background select-none overflow-y-auto p-5">
  <div class="max-w-4xl w-full mx-auto space-y-5">
    <!-- Header Banner -->
    <div class="flex items-center justify-between pb-2 border-b border-border">
      <div>
        <div class="flex items-center gap-2">
          <span class="p-1.5 rounded-lg bg-emerald-500/10 text-emerald-500">
            <Minimize2 size={20} />
          </span>
          <h2 class="text-xl font-bold tracking-tight text-foreground">可控压缩工作台 (Compression Studio)</h2>
          <span class="text-xs px-2 py-0.5 rounded-full bg-emerald-500/10 text-emerald-600 font-medium">无损 / 高压缩比</span>
        </div>
        <p class="text-xs text-muted-foreground mt-1">
          采用 Flate 编码与流去重算法，提供 4 档智能压缩策略，压缩前后体积精准可视对比。
        </p>
      </div>
    </div>

    <!-- Step 1: Pick File -->
    <div class="p-4 rounded-xl border border-border bg-card shadow-sm flex items-center justify-between gap-4">
      <div class="flex items-center gap-3 min-w-0">
        <div class="p-2.5 rounded-lg bg-primary/10 text-primary shrink-0">
          <HardDrive size={22} />
        </div>
        <div class="min-w-0">
          <div class="text-xs font-medium text-muted-foreground uppercase tracking-wider">源文件</div>
          {#if filePath}
            <div class="text-sm font-semibold text-foreground truncate mt-0.5" title={filePath}>{fileName}</div>
            <div class="text-xs text-muted-foreground font-mono mt-0.5">原始大小: {formatBytes(fileSizeBytes)}</div>
          {:else}
            <div class="text-sm text-muted-foreground mt-0.5">尚未选择待压缩的 PDF 文件</div>
          {/if}
        </div>
      </div>

      <Button variant={filePath ? "outline" : "default"} size="sm" onclick={handlePickFile} class="gap-1.5 text-xs shrink-0">
        <FolderOpen size={14} />
        <span>{filePath ? "更换文件" : "选取 PDF 文件"}</span>
      </Button>
    </div>

    <!-- Step 2: Choose Presets -->
    <div class="space-y-3">
      <div class="flex items-center justify-between">
        <Label class="text-sm font-semibold text-foreground">选择压缩预设策略</Label>
        <span class="text-xs text-muted-foreground">根据您的阅读与归档场景智能选取</span>
      </div>

      <div class="grid grid-cols-1 md:grid-cols-2 gap-3.5">
        {#each presetList as preset}
          <button
            onclick={() => (selectedPreset = preset.id)}
            class="flex flex-col p-4 rounded-xl border text-left transition-all relative overflow-hidden group {selectedPreset === preset.id
              ? 'border-emerald-500 bg-emerald-500/10 shadow-sm'
              : 'border-border bg-card hover:border-border/80'}"
          >
            <div class="flex items-center justify-between w-full mb-2">
              <span class="text-sm font-bold text-foreground group-hover:text-emerald-500 transition-colors">
                {preset.name}
              </span>
              <span class="text-[11px] px-2 py-0.5 rounded-full font-mono font-medium {selectedPreset === preset.id ? 'bg-emerald-500 text-white' : 'bg-muted text-muted-foreground'}">
                预计减少 {preset.estimatedSavingMin}%~{preset.estimatedSavingMax}%
              </span>
            </div>
            <p class="text-xs text-muted-foreground leading-relaxed mb-3">
              {preset.description}
            </p>
            <div class="mt-auto space-y-1 pt-2 border-t border-border/50 text-[11px] text-muted-foreground/80">
              {#each preset.features as f}
                <div class="flex items-center gap-1.5">
                  <span class="w-1 h-1 rounded-full bg-emerald-500 shrink-0"></span>
                  <span>{f}</span>
                </div>
              {/each}
            </div>
          </button>
        {/each}
      </div>
    </div>

    <!-- Estimation & Execution Card -->
    {#if filePath && estimated}
      <div class="p-4 rounded-xl border border-emerald-500/40 bg-emerald-500/5 shadow-sm space-y-3">
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-2 text-xs font-semibold text-emerald-600">
            <Sparkles size={15} />
            <span>智能预期估算 ({currentPreset.name})</span>
          </div>
          <span class="text-xs text-emerald-600 font-medium">节省区间: {estimated.ratioRange}</span>
        </div>

        <div class="grid grid-cols-3 gap-3 text-center py-2 bg-card rounded-lg border border-border">
          <div>
            <div class="text-[11px] text-muted-foreground">当前体积</div>
            <div class="text-sm font-bold font-mono text-foreground mt-0.5">{formatBytes(fileSizeBytes)}</div>
          </div>
          <div class="flex items-center justify-center text-muted-foreground">
            <ArrowRight size={16} />
          </div>
          <div>
            <div class="text-[11px] text-muted-foreground">预估后体积</div>
            <div class="text-sm font-bold font-mono text-emerald-600 mt-0.5">
              {formatBytes(estimated.minFinal)} ~ {formatBytes(estimated.maxFinal)}
            </div>
          </div>
        </div>

        {#if estimated.isVectorTextOnly}
          <div class="flex items-start gap-2 p-2.5 rounded-lg bg-blue-500/10 border border-blue-500/20 text-xs text-blue-700 dark:text-blue-300">
            <Sparkles size={14} class="shrink-0 mt-0.5 text-blue-500" />
            <div>
              <span class="font-semibold">原生矢量文字排版：</span>
              <span>检测到当前文档无位图图片，内容已高度流压缩。系统将进行无损结构精简，保护字型矢量高清度，避免粗暴图片化导致字迹模糊与体积反弹。</span>
            </div>
          </div>
        {/if}

        <div class="text-[10px] text-muted-foreground/70 px-0.5">
          * 容量换算标准：软件内部按 1024 进制 (KiB)，操作系统访达通常按 1000 进制 (KB) 显示
        </div>

        <div class="flex items-center justify-between pt-1">
          <div class="text-xs text-muted-foreground flex items-center gap-1">
            <ShieldCheck size={13} class="text-emerald-500" />
            <span>采用临时事务写入，原文件绝不覆盖</span>
          </div>
          <Button size="sm" class="gap-1.5 text-xs px-5 bg-emerald-600 hover:bg-emerald-700" onclick={executeCompression} disabled={isCompressing}>
            {#if isCompressing}
              <Loader2 size={14} class="animate-spin" />
              <span>正在压缩优化 {compressionProgress > 0 ? `${compressionProgress}%` : '...'}</span>
            {:else}
              <Minimize2 size={14} />
              <span>开始压缩并另存为...</span>
            {/if}
          </Button>
        </div>
      </div>
    {/if}

    <!-- Completed Result Card -->
    {#if result}
      <div class="p-4 rounded-xl border border-emerald-500 bg-emerald-500/10 shadow-sm space-y-3">
        <div class="flex items-center gap-2 text-emerald-600 font-semibold text-sm">
          <CheckCircle2 size={18} />
          <span>压缩成功！显著优化了文档体积</span>
        </div>
        <div class="grid grid-cols-3 gap-3 text-center py-2 bg-card rounded-lg border border-border">
          <div>
            <div class="text-xs text-muted-foreground">原始大小</div>
            <div class="text-sm font-bold font-mono text-foreground mt-0.5">{formatBytes(result.originalSize)}</div>
          </div>
          <div>
            <div class="text-xs text-muted-foreground">压缩后大小</div>
            <div class="text-sm font-bold font-mono text-emerald-600 mt-0.5">{formatBytes(result.compressedSize)}</div>
          </div>
          <div>
            <div class="text-xs text-muted-foreground">节省比例</div>
            <div class="text-sm font-bold font-mono text-emerald-600 mt-0.5">-{result.ratio.toFixed(1)}%</div>
          </div>
        </div>
        <div class="flex items-center justify-between pt-1 gap-2">
          <div class="text-xs text-muted-foreground font-mono truncate max-w-md" title={result.outputPath}>
            已保存至: {result.outputPath}
          </div>
          <Button variant="outline" size="sm" class="gap-1.5 text-xs shrink-0" onclick={handleOpenInViewer}>
            <ExternalLink size={13} />
            <span>在查看器中打开</span>
          </Button>
        </div>
      </div>
    {/if}

    {#if errorMsg}
      <div class="p-3.5 rounded-xl border border-rose-500/40 bg-rose-500/10 text-xs text-rose-500 flex items-center gap-2">
        <AlertCircle size={16} class="shrink-0" />
        <span>{errorMsg}</span>
      </div>
    {/if}
  </div>
</div>
