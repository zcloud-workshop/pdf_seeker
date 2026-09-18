<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { currentFilePath, currentFileName } from "@/stores";
  import type { PdfInfo } from "@/types";
  import { Button } from "@/components/ui";
  import {
    Info,
    FileText,
    Copy,
    Check,
    Lock,
    Unlock,
    Calendar,
    User,
    Layers,
    HardDrive,
    X,
    FolderCheck,
  } from "lucide-svelte";

  let {
    open = $bindable(false),
  }: {
    open?: boolean;
  } = $props();

  let info = $state<PdfInfo | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let copiedPath = $state(false);

  $effect(() => {
    if (open && $currentFilePath) {
      loadInfo($currentFilePath);
    }
  });

  async function loadInfo(path: string) {
    loading = true;
    error = null;
    try {
      info = await invoke<PdfInfo>("get_pdf_info", { path });
    } catch (e: any) {
      error = e?.message || String(e);
    } finally {
      loading = false;
    }
  }

  function formatBytes(bytes: number): string {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
  }

  function handleCopyPath() {
    if ($currentFilePath) {
      navigator.clipboard.writeText($currentFilePath);
      copiedPath = true;
      setTimeout(() => (copiedPath = false), 1800);
    }
  }
</script>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4 animate-in fade-in select-none"
    onclick={() => (open = false)}
    role="presentation"
  >
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div
      class="bg-card border border-border rounded-2xl shadow-2xl w-full max-w-lg overflow-hidden flex flex-col max-h-[85vh] animate-in zoom-in-95 duration-150"
      onclick={(e) => e.stopPropagation()}
      role="dialog"
      aria-modal="true"
      tabindex="-1"
    >
      <!-- Header -->
      <div class="flex items-center justify-between px-6 py-4 border-b border-border/60 bg-muted/30">
        <div class="flex items-center gap-2.5">
          <div class="p-2 rounded-lg bg-primary/10 text-primary">
            <Info size={20} />
          </div>
          <div>
            <h2 class="text-base font-semibold text-foreground">文档属性与信息 (PDF Info)</h2>
            <p class="text-xs text-muted-foreground mt-0.5">查看当前文档的技术规格与元数据</p>
          </div>
        </div>
        <Button variant="ghost" size="icon" onclick={() => (open = false)} class="rounded-full h-8 w-8">
          <X size={16} />
        </Button>
      </div>

      <!-- Body -->
      <div class="p-6 space-y-5 overflow-y-auto">
        <!-- File Name & Path Card -->
        <div class="p-3.5 rounded-xl border border-border/80 bg-accent/20 space-y-2">
          <div class="flex items-center justify-between">
            <div class="text-xs font-semibold text-muted-foreground uppercase tracking-wider">本地存储位置</div>
            {#if $currentFilePath?.includes("pdf_seeker_remote")}
              <span class="text-[10px] font-medium px-2 py-0.5 rounded-full bg-blue-500/10 text-blue-500 border border-blue-500/20">
                网络缓存文档
              </span>
            {:else}
              <span class="text-[10px] font-medium px-2 py-0.5 rounded-full bg-emerald-500/10 text-emerald-500 border border-emerald-500/20">
                本地持久文件
              </span>
            {/if}
          </div>
          <div class="font-medium text-sm text-foreground break-all">{$currentFileName}</div>
          <div class="flex items-center gap-2">
            <code class="text-xs font-mono bg-background/80 px-2 py-1 rounded border border-border text-muted-foreground truncate flex-1 select-text">
              {$currentFilePath}
            </code>
            <Button
              variant="outline"
              size="sm"
              onclick={handleCopyPath}
              class="shrink-0 h-7 text-xs gap-1 px-2.5"
            >
              {#if copiedPath}
                <Check size={12} class="text-green-500" />
                <span class="text-green-500">已复制</span>
              {:else}
                <Copy size={12} />
                <span>复制路径</span>
              {/if}
            </Button>
          </div>
          {#if $currentFilePath?.includes("pdf_seeker_remote")}
            <p class="text-[11px] text-muted-foreground/80 leading-normal">
              💡 提示：从网站 URL 打开的 PDF 已经安全缓存于系统本地临时目录，您可以随时在顶部使用「导出/另存为」保存到任意本地磁盘位置。
            </p>
          {/if}
        </div>

        {#if loading}
          <div class="py-8 flex flex-col items-center justify-center gap-2 text-muted-foreground">
            <div class="w-6 h-6 border-2 border-primary border-t-transparent rounded-full animate-spin"></div>
            <span class="text-xs">正在分析 PDF 元数据...</span>
          </div>
        {:else if error}
          <div class="p-4 rounded-xl bg-destructive/10 text-destructive text-sm border border-destructive/20">
            读取信息失败：{error}
          </div>
        {:else if info}
          <!-- Specs Grid -->
          <div class="grid grid-cols-2 gap-3">
            <div class="p-3 rounded-xl border border-border/60 bg-card flex items-center gap-3">
              <div class="p-2 rounded-lg bg-blue-500/10 text-blue-500">
                <Layers size={18} />
              </div>
              <div>
                <div class="text-[11px] text-muted-foreground font-medium">总页数</div>
                <div class="text-sm font-semibold text-foreground">{info.pages} 页</div>
              </div>
            </div>

            <div class="p-3 rounded-xl border border-border/60 bg-card flex items-center gap-3">
              <div class="p-2 rounded-lg bg-emerald-500/10 text-emerald-500">
                <HardDrive size={18} />
              </div>
              <div>
                <div class="text-[11px] text-muted-foreground font-medium">文件大小</div>
                <div class="text-sm font-semibold text-foreground">{formatBytes(info.fileSize)}</div>
              </div>
            </div>

            <div class="p-3 rounded-xl border border-border/60 bg-card flex items-center gap-3">
              <div class="p-2 rounded-lg bg-indigo-500/10 text-indigo-500">
                <FileText size={18} />
              </div>
              <div>
                <div class="text-[11px] text-muted-foreground font-medium">标准版式尺寸</div>
                <div class="text-xs font-semibold text-foreground truncate max-w-[130px]" title={info.pageSize || "未知"}>
                  {info.pageSize || "自适应"}
                </div>
              </div>
            </div>

            <div class="p-3 rounded-xl border border-border/60 bg-card flex items-center gap-3">
              <div class="p-2 rounded-lg {info.isEncrypted ? 'bg-amber-500/10 text-amber-500' : 'bg-green-500/10 text-green-500'}">
                {#if info.isEncrypted}
                  <Lock size={18} />
                {:else}
                  <Unlock size={18} />
                {/if}
              </div>
              <div>
                <div class="text-[11px] text-muted-foreground font-medium">安全性与加密</div>
                <div class="text-sm font-semibold text-foreground">
                  {info.isEncrypted ? "已密码加密保护" : "无加密 (可自由编辑)"}
                </div>
              </div>
            </div>
          </div>

          <!-- Metadata Details Table -->
          <div class="space-y-2 pt-1">
            <div class="text-xs font-semibold text-muted-foreground uppercase tracking-wider">元数据属性</div>
            <div class="rounded-xl border border-border/70 overflow-hidden divide-y divide-border/50 text-xs">
              <div class="flex px-3.5 py-2.5 bg-card hover:bg-muted/30 transition-colors">
                <span class="w-24 shrink-0 text-muted-foreground">PDF 规范版本</span>
                <span class="font-mono font-medium text-foreground">{info.pdfVersion ? `PDF ${info.pdfVersion}` : "1.4+"}</span>
              </div>
              {#if info.title}
                <div class="flex px-3.5 py-2.5 bg-card hover:bg-muted/30 transition-colors">
                  <span class="w-24 shrink-0 text-muted-foreground">标题 (Title)</span>
                  <span class="font-medium text-foreground break-all">{info.title}</span>
                </div>
              {/if}
              {#if info.author}
                <div class="flex px-3.5 py-2.5 bg-card hover:bg-muted/30 transition-colors">
                  <span class="w-24 shrink-0 text-muted-foreground">作者 (Author)</span>
                  <span class="font-medium text-foreground">{info.author}</span>
                </div>
              {/if}
              {#if info.creator}
                <div class="flex px-3.5 py-2.5 bg-card hover:bg-muted/30 transition-colors">
                  <span class="w-24 shrink-0 text-muted-foreground">创建工具</span>
                  <span class="text-foreground truncate">{info.creator}</span>
                </div>
              {/if}
              {#if info.producer}
                <div class="flex px-3.5 py-2.5 bg-card hover:bg-muted/30 transition-colors">
                  <span class="w-24 shrink-0 text-muted-foreground">编码生成器</span>
                  <span class="text-foreground truncate">{info.producer}</span>
                </div>
              {/if}
              {#if info.creationDate}
                <div class="flex px-3.5 py-2.5 bg-card hover:bg-muted/30 transition-colors">
                  <span class="w-24 shrink-0 text-muted-foreground">创建日期</span>
                  <span class="font-mono text-muted-foreground">{info.creationDate}</span>
                </div>
              {/if}
              {#if info.modDate}
                <div class="flex px-3.5 py-2.5 bg-card hover:bg-muted/30 transition-colors">
                  <span class="w-24 shrink-0 text-muted-foreground">最后修改</span>
                  <span class="font-mono text-muted-foreground">{info.modDate}</span>
                </div>
              {/if}
              {#if info.keywords}
                <div class="flex px-3.5 py-2.5 bg-card hover:bg-muted/30 transition-colors">
                  <span class="w-24 shrink-0 text-muted-foreground">关键词</span>
                  <span class="text-foreground">{info.keywords}</span>
                </div>
              {/if}
            </div>
          </div>
        {/if}
      </div>

      <!-- Footer -->
      <div class="px-6 py-3 border-t border-border/60 bg-muted/20 flex justify-end">
        <Button variant="default" size="sm" onclick={() => (open = false)}>
          完成
        </Button>
      </div>
    </div>
  </div>
{/if}
