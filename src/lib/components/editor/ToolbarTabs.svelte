<script lang="ts">
  import { onMount } from "svelte";
  import { t } from "@/i18n/index.svelte.ts";
  import {
    Layers,
    Pencil,
    ArrowLeftRight,
    Sparkles,
    ChevronDown,
    X,
    RotateCw,
    Merge,
    Split,
    Trash2,
    FileOutput,
    ArrowUpDown,
    FilePlus,
    Hash,
    Type,
    Square,
    Highlighter,
    PenTool,
    Stamp,
    ScanText,
    Image,
    Images,
    FileText,
    Table,
    ShieldAlert,
    Minimize2,
    Cpu,
    ShieldCheck,
  } from "lucide-svelte";

  export type ToolCategory = "page" | "edit" | "convert" | "extract" | "batch";
  export type ToolId =
    | "merge" | "split" | "rotate" | "delete" | "extractPages" | "reorder" | "insertPages"
    | "crop" | "nup" | "reverse" | "duplicate"
    | "pageNumber" | "sanitize" | "compress"
    | "editText" | "editRect" | "editHighlight" | "ocrEdit" | "sign" | "watermark"
    | "imageWatermark" | "searchablePdf" | "redact" | "draw"
    | "img2pdf" | "pdf2img" | "pdf2text"
    | "extractText" | "table" | "extractImages" | "pdf2json"
    | "epub2pdf" | "text2pdf" | "md2pdf" | "cbz2pdf" | "html2pdf" | "svg2pdf"
    | "metadata" | "encrypt" | "decrypt" | "flatten" | "certSign" | "changePermissions"
    | "comparePdf" | "linearize" | "repair" | "deskew" | "grayscale" | "batchExport"
    | "batchWorkspace" | "compressionStudio" | "privacyAudit";

  interface ToolItem {
    id: ToolId;
    label: string;
    labelKey?: string;
    icon: any;
    category: ToolCategory;
    group: number;
    isEdit: boolean;
    description?: string;
  }

  const categories = [
    { id: "page" as ToolCategory, name: "页面管理", labelKey: "categories.page", icon: Layers },
    { id: "edit" as ToolCategory, name: "标注编辑", labelKey: "categories.edit", icon: Pencil },
    { id: "convert" as ToolCategory, name: "转换提取", labelKey: "categories.convert", icon: ArrowLeftRight },
    { id: "batch" as ToolCategory, name: "批处理流水线", labelKey: "nav.tools", icon: Sparkles },
  ];

  const tools: ToolItem[] = [
    // 页面管理
    { id: "rotate", label: "旋转 90°", labelKey: "tools.rotate", icon: RotateCw, category: "page", group: 1, isEdit: false, description: "顺时针永久旋转页面" },
    { id: "reorder", label: "页面重排", labelKey: "tools.reorder", icon: ArrowUpDown, category: "page", group: 1, isEdit: false, description: "调整页面先后顺序" },

    { id: "extractPages", label: "提取页面", labelKey: "tools.extractPages", icon: FileOutput, category: "page", group: 2, isEdit: false, description: "抽取指定页面为新文件" },
    { id: "delete", label: "删除页面", labelKey: "tools.deletePages", icon: Trash2, category: "page", group: 2, isEdit: false, description: "移除指定页码" },

    { id: "merge", label: "合并 PDF", labelKey: "tools.merge", icon: Merge, category: "page", group: 3, isEdit: false, description: "按序组合多个 PDF 文档" },
    { id: "split", label: "拆分 PDF", labelKey: "tools.split", icon: Split, category: "page", group: 3, isEdit: false, description: "按页码范围分割文件" },
    { id: "insertPages", label: "插入页面", labelKey: "tools.insertPages", icon: FilePlus, category: "page", group: 3, isEdit: false, description: "在指定位置插入其他 PDF" },
    { id: "pageNumber", label: "添加页码", labelKey: "tools.pageNumber", icon: Hash, category: "page", group: 3, isEdit: false, description: "批量注入格式化页码" },

    // 标注编辑
    { id: "editText", label: "添加文字", labelKey: "tools.editText", icon: Type, category: "edit", group: 1, isEdit: true, description: "在页面任意位置置入文本" },
    { id: "editRect", label: "矩形框", labelKey: "tools.editRect", icon: Square, category: "edit", group: 1, isEdit: true, description: "绘制矢量矩形标记框" },
    { id: "editHighlight", label: "高亮标注", labelKey: "tools.editHighlight", icon: Highlighter, category: "edit", group: 1, isEdit: true, description: "半透明荧光笔划线高亮" },

    { id: "sign", label: "手写签名", labelKey: "tools.sign", icon: PenTool, category: "edit", group: 2, isEdit: false, description: "盖印电子手写签名图片" },
    { id: "watermark", label: "防伪水印", labelKey: "tools.watermark", icon: Stamp, category: "edit", group: 2, isEdit: false, description: "注入半透明倾斜文字水印" },
    { id: "ocrEdit", label: "OCR 文字识别", labelKey: "tools.ocrEdit", icon: ScanText, category: "edit", group: 2, isEdit: false, description: "本地高精度 OCR 识别与编辑" },

    // 转换提取
    { id: "pdf2img", label: "PDF 转图片", labelKey: "tools.pdf2img", icon: Image, category: "convert", group: 1, isEdit: false, description: "逐页导出为 PNG/JPEG 图像" },
    { id: "img2pdf", label: "图片转 PDF", labelKey: "tools.convertImage", icon: Images, category: "convert", group: 1, isEdit: false, description: "多张图像合并生成 PDF" },
    { id: "pdf2text", label: "提取纯文本", labelKey: "tools.convertText", icon: FileText, category: "convert", group: 1, isEdit: false, description: "解析全文字符串导出 TXT" },

    { id: "table", label: "表格识别", labelKey: "tools.extractTable", icon: Table, category: "convert", group: 2, isEdit: false, description: "提取结构化表格为 CSV/Markdown" },
    { id: "compress", label: "可控压缩", labelKey: "tools.compress", icon: Minimize2, category: "convert", group: 2, isEdit: false, description: "4 档智能优化减小体积" },
    { id: "sanitize", label: "隐私脱敏", labelKey: "tools.sanitize", icon: ShieldAlert, category: "convert", group: 2, isEdit: false, description: "彻底清除私有元数据" },

    // 批处理流水线
    { id: "batchWorkspace", label: "打开批处理工作台", icon: Sparkles, category: "batch", group: 1, isEdit: false, description: "四区流水线，支持多任务、取消与报告导出" },
    { id: "compressionStudio", label: "可控压缩工作室", icon: Minimize2, category: "batch", group: 1, isEdit: false, description: "4 档预设前后对比与节省率测算" },
    { id: "privacyAudit", label: "本地隐私审计面板", icon: ShieldCheck, category: "batch", group: 1, isEdit: false, description: "验证 100% 离线 0 网络流量" },
  ];

  let {
    activeCategory = $bindable("page" as ToolCategory),
    currentCategory,
    activeTool = $bindable(null as ToolId | null),
    ontoolaction,
    onselecttool,
  }: {
    activeCategory?: ToolCategory;
    currentCategory?: ToolCategory;
    activeTool?: ToolId | null;
    ontoolaction?: (toolId: ToolId) => void;
    onselecttool?: (toolId: string) => void;
  } = $props();

  // If currentCategory is explicitly passed from parent and changed, sync it
  let prevPropCategory: ToolCategory | undefined = undefined;
  $effect(() => {
    const incoming = currentCategory;
    if (incoming !== undefined && incoming !== prevPropCategory) {
      prevPropCategory = incoming;
      activeCategory = incoming;
    }
  });

  // Floating Popover State ("小弹窗让用户选择，上下滑动时自动缩小，用户鼠标停留时重现")
  let isPaletteOpen = $state(false);
  let isPaletteMinimized = $state(false);
  let isHovered = $state(false);
  let paletteContainer: HTMLElement | null = $state(null);

  const activeCategoryObj = $derived(
    categories.find((c) => c.id === activeCategory) || categories[0]
  );
  const categoryTools = $derived(tools.filter((t) => t.category === activeCategory));
  const activeToolObj = $derived(tools.find((t) => t.id === activeTool));

  function selectTool(id: ToolId) {
    if (ontoolaction) {
      ontoolaction(id);
    } else if (onselecttool) {
      onselecttool(id);
    } else {
      activeTool = activeTool === id ? null : id;
    }

    // Auto-close popover after choosing action tool
    const tool = tools.find((t) => t.id === id);
    if (!tool?.isEdit) {
      isPaletteOpen = false;
    }
  }

  function toggleCategory(id: ToolCategory) {
    if (activeCategory === id && isPaletteOpen) {
      isPaletteOpen = false;
    } else {
      activeCategory = id;
      isPaletteOpen = true;
      isPaletteMinimized = false;
    }
  }

  function handleScroll() {
    // When document scrolls up/down, automatically shrink the popover to prevent blocking view
    if (isPaletteOpen && !isHovered) {
      isPaletteMinimized = true;
    }
  }

  function handleDocumentClick(e: MouseEvent) {
    if (!isPaletteOpen || !paletteContainer) return;
    const target = e.target as HTMLElement;
    if (paletteContainer.contains(target) || target.closest("[data-cat-btn]")) {
      return;
    }
    isPaletteOpen = false;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && isPaletteOpen) {
      e.preventDefault();
      isPaletteOpen = false;
    }
  }

  function portal(node: HTMLElement) {
    if (typeof document !== "undefined") {
      document.body.appendChild(node);
    }
    return {
      destroy() {
        if (node.parentNode) {
          node.parentNode.removeChild(node);
        }
      },
    };
  }

  onMount(() => {
    window.addEventListener("scroll", handleScroll, true);
    document.addEventListener("mousedown", handleDocumentClick);
    window.addEventListener("keydown", handleKeydown);
    return () => {
      window.removeEventListener("scroll", handleScroll, true);
      document.removeEventListener("mousedown", handleDocumentClick);
      window.removeEventListener("keydown", handleKeydown);
    };
  });
</script>

<div class="flex items-center gap-2 select-none py-0.5 relative">
  <!-- Category Switcher: Segmented Control -->
  <div class="flex items-center bg-muted/70 p-0.5 rounded-lg border border-border/60 shrink-0">
    {#each categories as cat}
      {@const CatIcon = cat.icon}
      {@const isActive = activeCategory === cat.id}
      {@const isOpen = isPaletteOpen && activeCategory === cat.id}
      <button
        type="button"
        data-cat-btn={cat.id}
        onclick={() => toggleCategory(cat.id)}
        class="flex items-center gap-1.5 px-2.5 h-6 rounded-md text-xs font-medium transition-all whitespace-nowrap cursor-pointer {isActive
          ? 'bg-background text-primary font-semibold shadow-xs'
          : 'text-muted-foreground hover:text-foreground'}"
        title="点击展开 {cat.name} 功能选择面板"
      >
        <CatIcon size={12} class={isActive ? 'text-primary' : ''} />
        <span>{cat.name}</span>
        <ChevronDown size={11} class="transition-transform duration-200 opacity-60 {isOpen ? 'rotate-180 text-primary opacity-100' : ''}" />
      </button>
    {/each}
  </div>

  <!-- If an edit tool is currently active, show a dismissable pill in the top bar -->
  {#if activeTool && activeToolObj}
    {@const ActiveToolIcon = activeToolObj.icon}
    <div class="flex items-center gap-1.5 px-2 h-6 rounded-md text-xs bg-primary/15 text-primary border border-primary/30 font-medium shrink-0 animate-in fade-in">
      <ActiveToolIcon size={12} />
      <span>{activeToolObj.label} (编辑中)</span>
      <button
        type="button"
        onclick={() => (activeTool = null)}
        class="hover:bg-primary/20 rounded p-0.5 ml-0.5 cursor-pointer text-primary"
        title="退出当前工具"
      >
        <X size={10} />
      </button>
    </div>
  {/if}

</div>

<!-- Floating Tool Palette Popover ("小弹窗让用户选择，上下滑动时自动缩小，用户鼠标停留时重现") -->
{#if isPaletteOpen}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    use:portal
    bind:this={paletteContainer}
    role="region"
    aria-label="工具选择面板"
    onmouseenter={() => {
      isHovered = true;
      if (isPaletteMinimized) {
        isPaletteMinimized = false;
      }
    }}
    onmouseleave={() => {
      isHovered = false;
    }}
    class="fixed top-[48px] left-1/2 -translate-x-1/2 z-[9999] transition-all duration-200"
  >
    {#if isPaletteMinimized}
      {@const CatIcon = activeCategoryObj.icon}
      <!-- Minimized Slim Pill: Automatically shrunk during scrolling so PDF is unobstructed -->
      <button
        type="button"
        onclick={() => { isPaletteMinimized = false; isHovered = true; }}
        class="flex items-center gap-2 px-3.5 h-7 rounded-full bg-card/90 backdrop-blur-md border border-border/80 shadow-xl text-xs cursor-pointer select-none text-foreground hover:scale-105 hover:bg-card transition-all duration-150 animate-in fade-in"
        title="悬停或点击重现完整工具菜单"
      >
        <CatIcon size={13} class="text-primary" />
        <span class="font-medium text-xs">{activeCategoryObj.name}</span>
        <span class="text-[10px] text-muted-foreground font-mono">({categoryTools.length}项)</span>
        <span class="text-[10px] bg-primary/15 text-primary px-1.5 py-0.5 rounded-full font-medium ml-1">
          悬停重现
        </span>
      </button>
    {:else}
      {@const CatIcon = activeCategoryObj.icon}
      <!-- Full Expanded Palette: All tools in this category in a clean grid -->
      <div
        class="w-[520px] max-w-[94vw] bg-card/95 backdrop-blur-xl border border-border/80 shadow-2xl rounded-2xl p-3.5 flex flex-col gap-2.5 text-foreground animate-in zoom-in-95 duration-150 select-none"
      >
        <!-- Popover Header -->
        <div class="flex items-center justify-between border-b border-border/60 pb-2">
          <div class="flex items-center gap-2">
            <div class="p-1 rounded-md bg-primary/10 text-primary">
              <CatIcon size={14} />
            </div>
            <span class="text-xs font-semibold">{activeCategoryObj.name}</span>
            <span class="text-[10px] text-muted-foreground">共 {categoryTools.length} 项功能</span>
          </div>

          <div class="flex items-center gap-2">
            <span class="text-[10px] text-muted-foreground/80 font-mono hidden sm:inline">
              上下滑动自动最小化 · 悬停重现
            </span>
            <button
              type="button"
              onclick={() => (isPaletteOpen = false)}
              class="p-1 rounded-md text-muted-foreground hover:bg-accent hover:text-foreground transition-colors cursor-pointer"
              title="关闭面板 (Esc)"
            >
              <X size={14} />
            </button>
          </div>
        </div>

        <!-- Grid of Tool Cards (100% visible, no horizontal scrolling needed!) -->
        <div class="grid grid-cols-2 sm:grid-cols-3 gap-1.5 max-h-[320px] overflow-y-auto pr-0.5">
          {#each categoryTools as tool}
            {@const ToolIcon = tool.icon}
            {@const isActive = tool.isEdit && activeTool === tool.id}
            <button
              type="button"
              onclick={() => selectTool(tool.id)}
              class="flex items-center gap-2 p-2 rounded-xl text-left transition-all border {isActive
                ? 'bg-primary text-primary-foreground border-primary shadow-xs font-medium'
                : 'border-border/40 bg-background/60 hover:border-border hover:bg-accent/80 text-foreground/90'} cursor-pointer group"
              title={tool.description || tool.label}
            >
              <div class="p-1.5 rounded-lg {isActive ? 'bg-white/20 text-white' : 'bg-muted text-primary group-hover:bg-primary/10'} transition-colors shrink-0">
                <ToolIcon size={14} />
              </div>
              <div class="flex flex-col min-w-0 flex-1">
                <span class="text-xs font-medium truncate {isActive ? 'text-white' : ''}">{tool.label}</span>
                {#if tool.description}
                  <span class="text-[10px] truncate {isActive ? 'text-white/80' : 'text-muted-foreground'}">{tool.description}</span>
                {/if}
              </div>
            </button>
          {/each}
        </div>
      </div>
    {/if}
  </div>
{/if}
