<script lang="ts">
  import { t } from "@/i18n/index.svelte.ts";
  import { Button, Tooltip } from "@/components/ui";
  import {
    FolderOpen,
    FileText,
    Merge,
    Split,
    RotateCw,
    FileOutput,
    Trash2,
    FilePlus,
    Hash,
    Image,
    Images,
    ShieldAlert,
    Minimize2,
    PenTool,
    Stamp,
    ScanText,
    Table,
    ArrowUpDown,
    CheckCircle2,
    X,
    ExternalLink,
  } from "lucide-svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { currentView, currentFilePath, currentFileName, activeToolTab, type ToolTabMode } from "@/stores";
  import PageToolsDialog from "@/components/editor/PageToolsDialog.svelte";
  import type { ToolId } from "@/components/editor/ToolbarTabs.svelte";
  import BatchWorkspace from "@/components/tools/BatchWorkspace.svelte";
  import CompressionStudio from "@/components/tools/CompressionStudio.svelte";
  import PrivacyAuditPanel from "@/components/tools/PrivacyAuditPanel.svelte";
  import { Layers, ShieldCheck } from "lucide-svelte";

  let selectedFilePath = $state<string | null>($currentFilePath || null);
  let activeTool = $state<ToolId | null>(null);

  $effect(() => {
    if ($currentFilePath && !selectedFilePath) {
      selectedFilePath = $currentFilePath;
    }
  });

  let toolViewMode = $state<ToolTabMode>($activeToolTab || "tools");

  $effect(() => {
    const tab = $activeToolTab;
    if (tab && tab !== toolViewMode) {
      toolViewMode = tab;
    }
  });

  function switchToolMode(mode: ToolTabMode) {
    toolViewMode = mode;
    activeToolTab.set(mode);
  }

  interface ToolCard {
    id: ToolId;
    title: string;
    description: string;
    icon: any;
    color: string;
    badge?: string;
  }

  interface ToolSection {
    title: string;
    tools: ToolCard[];
  }

  const sections: ToolSection[] = [
    {
      title: "页面组织 (Page Organize)",
      tools: [
        { id: "merge", title: "合并 PDF", description: "将多个 PDF 文档按顺序组合成一个新文件", icon: Merge, color: "text-blue-500 bg-blue-500/10" },
        { id: "split", title: "拆分 PDF", description: "提取单页或按自定义页码范围分割文件", icon: Split, color: "text-indigo-500 bg-indigo-500/10" },
        { id: "rotate", title: "旋转页面", description: "永久旋转全部或特定页面 90°、180° 或 270°", icon: RotateCw, color: "text-emerald-500 bg-emerald-500/10" },
        { id: "extractPages", title: "提取页面", description: "将指定页面独立导出为新的 PDF 文件", icon: FileOutput, color: "text-cyan-500 bg-cyan-500/10" },
        { id: "delete", title: "删除页面", description: "移除指定页码并重新生成紧凑文档", icon: Trash2, color: "text-rose-500 bg-rose-500/10" },
        { id: "reorder", title: "页面重排", description: "通过自定义序列灵活调整页面先后次序", icon: ArrowUpDown, color: "text-violet-500 bg-violet-500/10" },
        { id: "insertPages", title: "插入页面", description: "在目标位置无缝插入其他 PDF 页面", icon: FilePlus, color: "text-amber-500 bg-amber-500/10" },
        { id: "pageNumber", title: "添加页码", description: "批量注入格式化页码（支持多种版式位置）", icon: Hash, color: "text-teal-500 bg-teal-500/10" },
      ],
    },
    {
      title: "格式转换 (Conversion)",
      tools: [
        { id: "pdf2img", title: "PDF 转图片", description: "将 PDF 逐页渲染导出为高清晰度 PNG 图像", icon: Image, color: "text-purple-500 bg-purple-500/10" },
        { id: "img2pdf", title: "图片转 PDF", description: "快速将多张图片图片按顺序打包合成 PDF", icon: Images, color: "text-pink-500 bg-pink-500/10" },
        { id: "pdf2text", title: "提取纯文本", description: "解析导出全文字符串内容至 .txt 文本文件", icon: FileText, color: "text-orange-500 bg-orange-500/10" },
      ],
    },
    {
      title: "安全与优化 (Security & Optimize)",
      tools: [
        { id: "watermark", title: "添加水印", description: "注入半透明文本水印以防伪保护版权", icon: Stamp, color: "text-red-500 bg-red-500/10" },
        { id: "compress", title: "压缩优化", description: "清理冗余对象与流压缩以大幅缩减文件体积", icon: Minimize2, color: "text-green-500 bg-green-500/10" },
        { id: "sanitize", title: "隐私脱敏", description: "彻底清除文档作者、元数据、隐藏操作与私有信息", icon: ShieldAlert, color: "text-amber-600 bg-amber-500/10", badge: "安全" },
        { id: "sign", title: "手写签名", description: "添加数字印章或手写签名图片至文档指定页", icon: PenTool, color: "text-blue-600 bg-blue-500/10" },
      ],
    },
    {
      title: "智能提取 (OCR & Data)",
      tools: [
        { id: "ocrEdit", title: "OCR 文字识别", description: "本地离线高精度光学字符识别与文本提取", icon: ScanText, color: "text-indigo-600 bg-indigo-500/10", badge: "AI" },
        { id: "table", title: "表格结构化识别", description: "自动检测表格行列结构并导出 Markdown/CSV", icon: Table, color: "text-emerald-600 bg-emerald-500/10", badge: "AI" },
      ],
    },
  ];

  async function handlePickFile() {
    const selected = await open({
      multiple: false,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
    if (selected) {
      selectedFilePath = typeof selected === "string" ? selected : (selected as any).path;
    }
  }

  async function handleCardClick(toolId: ToolId) {
    if (toolId === "img2pdf" || toolId === "merge") {
      activeTool = toolId;
      return;
    }

    if (!selectedFilePath) {
      const selected = await open({
        multiple: false,
        filters: [{ name: "PDF", extensions: ["pdf"] }],
      });
      if (selected) {
        selectedFilePath = typeof selected === "string" ? selected : (selected as any).path;
        activeTool = toolId;
      }
    } else {
      activeTool = toolId;
    }
  }

  function handleOpenInViewer() {
    if (selectedFilePath) {
      currentFilePath.set(selectedFilePath);
      currentFileName.set(selectedFilePath.split(/[\\/]/).pop() || "Untitled");
      currentView.set("editor");
    }
  }
</script>

<div class="h-full flex flex-col bg-background select-none overflow-hidden">
  <!-- Top Segmented Bar -->
  <div class="px-6 pt-4 pb-2 border-b border-border bg-card/50 shrink-0 flex items-center justify-between">
    <div class="flex items-center gap-1.5 p-1 rounded-xl bg-muted/60 border border-border/80">
      <button
        onclick={() => switchToolMode("tools")}
        class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all {toolViewMode === 'tools'
          ? 'bg-card text-foreground shadow-xs'
          : 'text-muted-foreground hover:text-foreground'}"
      >
        <FolderOpen size={14} />
        <span>独立工具箱</span>
      </button>

      <button
        onclick={() => switchToolMode("batch")}
        class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all {toolViewMode === 'batch'
          ? 'bg-card text-foreground shadow-xs'
          : 'text-muted-foreground hover:text-foreground'}"
      >
        <Layers size={14} class="text-primary" />
        <span>批处理工作台</span>
      </button>

      <button
        onclick={() => switchToolMode("compress")}
        class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all {toolViewMode === 'compress'
          ? 'bg-card text-foreground shadow-xs'
          : 'text-muted-foreground hover:text-foreground'}"
      >
        <Minimize2 size={14} class="text-emerald-500" />
        <span>可控压缩工作室</span>
      </button>

      <button
        onclick={() => switchToolMode("privacy")}
        class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all {toolViewMode === 'privacy'
          ? 'bg-card text-foreground shadow-xs'
          : 'text-muted-foreground hover:text-foreground'}"
      >
        <ShieldCheck size={14} class="text-emerald-600" />
        <span>本地隐私审计</span>
      </button>
    </div>
  </div>

  {#if toolViewMode === "batch"}
    <div class="flex-1 overflow-hidden">
      <BatchWorkspace />
    </div>
  {:else if toolViewMode === "compress"}
    <div class="flex-1 overflow-hidden">
      <CompressionStudio />
    </div>
  {:else if toolViewMode === "privacy"}
    <div class="flex-1 overflow-hidden">
      <PrivacyAuditPanel />
    </div>
  {:else}
    <div class="flex-1 overflow-y-auto p-6">
      <div class="max-w-5xl mx-auto space-y-6">
        <!-- Header -->
        <div class="flex items-center justify-between">
          <div>
            <h1 class="text-2xl font-bold tracking-tight text-foreground">PDF 独立工具中心</h1>
            <p class="text-sm text-muted-foreground mt-0.5">无需预先载入查看器，直接选择本地文件快速执行各种批处理与编辑工具</p>
          </div>
        </div>

        <!-- Active File Selection Banner -->
        <div class="p-4 rounded-xl border border-border bg-card shadow-sm flex items-center justify-between gap-4">
      <div class="flex items-center gap-3 min-w-0">
        <div class="p-2.5 rounded-lg bg-primary/10 text-primary shrink-0">
          <FolderOpen size={22} />
        </div>
        <div class="min-w-0">
          <div class="text-xs font-medium text-muted-foreground uppercase tracking-wider">当前处理文档</div>
          {#if selectedFilePath}
            <div class="text-sm font-semibold text-foreground truncate mt-0.5" title={selectedFilePath}>
              {selectedFilePath.split(/[\\/]/).pop()}
            </div>
            <div class="text-xs text-muted-foreground truncate font-mono mt-0.5">{selectedFilePath}</div>
          {:else}
            <div class="text-sm text-muted-foreground mt-0.5">尚未选择文件（点击右侧按钮选取，或直接点击下方任意工具卡片）</div>
          {/if}
        </div>
      </div>

      <div class="flex items-center gap-2 shrink-0">
        {#if selectedFilePath}
          <Button variant="outline" size="sm" onclick={handleOpenInViewer} class="gap-1.5 text-xs">
            <ExternalLink size={14} />
            <span>在查看器中打开</span>
          </Button>
          <Button variant="ghost" size="icon" onclick={() => (selectedFilePath = null)} title="清除已选文件">
            <X size={16} />
          </Button>
        {/if}
        <Button variant={selectedFilePath ? "outline" : "default"} size="sm" onclick={handlePickFile} class="gap-1.5 text-xs">
          <FolderOpen size={14} />
          <span>{selectedFilePath ? "更换文件" : "选取 PDF 文件"}</span>
        </Button>
      </div>
    </div>

    <!-- Tool Sections Grid -->
    {#each sections as sec}
      <div class="space-y-3">
        <h2 class="text-sm font-semibold text-muted-foreground tracking-wider uppercase">{sec.title}</h2>
        <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3.5">
          {#each sec.tools as tool}
            {@const Icon = tool.icon}
            <button
              onclick={() => handleCardClick(tool.id)}
              class="group flex flex-col p-4 rounded-xl border border-border bg-card hover:bg-accent/40 hover:border-primary/40 transition-all text-left shadow-sm hover:shadow relative overflow-hidden"
            >
              <div class="flex items-center justify-between w-full mb-2.5">
                <div class="p-2.5 rounded-lg {tool.color} group-hover:scale-105 transition-transform">
                  <Icon size={20} />
                </div>
                {#if tool.badge}
                  <span class="text-[10px] font-bold px-1.5 py-0.5 rounded bg-primary/10 text-primary font-mono uppercase">
                    {tool.badge}
                  </span>
                {/if}
              </div>
              <h3 class="text-sm font-semibold text-foreground group-hover:text-primary transition-colors">
                {tool.title}
              </h3>
              <p class="text-xs text-muted-foreground mt-1 line-clamp-2 leading-relaxed">
                {tool.description}
              </p>
            </button>
          {/each}
        </div>
      </div>
    {/each}
      </div>
    </div>
  {/if}
</div>

<!-- Modal Dialog for Tool Runner -->
{#if activeTool}
  <PageToolsDialog
    tool={activeTool}
    filePath={selectedFilePath}
    onclose={() => (activeTool = null)}
    onfilechanged={(newPath) => {
      selectedFilePath = newPath;
    }}
  />
{/if}

