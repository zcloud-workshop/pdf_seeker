<script lang="ts">
  import { t } from "@/i18n/index.svelte.ts";
  import {
    Layers,
    Pencil,
    ArrowLeftRight,
    FileSearch,
    ScanText,
  } from "lucide-svelte";

  export type ToolCategory = "page" | "edit" | "convert" | "extract";
  export type ToolId =
    | "merge" | "split" | "rotate" | "delete" | "extractPages" | "reorder"
    | "editText" | "editRect" | "editHighlight" | "ocrEdit" | "sign" | "watermark"
    | "img2pdf" | "pdf2img" | "pdf2text"
    | "extractText" | "table";

  const categories = [
    { id: "page" as ToolCategory, labelKey: "tools.merge", icon: Layers },
    { id: "edit" as ToolCategory, labelKey: "tools.editText", icon: Pencil },
    { id: "convert" as ToolCategory, labelKey: "tools.convertImage", icon: ArrowLeftRight },
    { id: "extract" as ToolCategory, labelKey: "tools.convertText", icon: FileSearch },
  ];

  const tools = [
    { id: "merge" as ToolId, labelKey: "tools.merge", icon: Layers, category: "page" as ToolCategory, isEdit: false },
    { id: "split" as ToolId, labelKey: "tools.split", icon: Layers, category: "page", isEdit: false },
    { id: "rotate" as ToolId, labelKey: "tools.rotate", icon: Layers, category: "page", isEdit: false },
    { id: "delete" as ToolId, labelKey: "tools.deletePages", icon: Layers, category: "page", isEdit: false },
    { id: "extractPages" as ToolId, labelKey: "tools.extractPages", icon: Layers, category: "page", isEdit: false },
    { id: "reorder" as ToolId, labelKey: "tools.reorder", icon: Layers, category: "page", isEdit: false },
    { id: "editText" as ToolId, labelKey: "tools.editText", icon: Pencil, category: "edit" as ToolCategory, isEdit: true },
    { id: "editRect" as ToolId, labelKey: "tools.editRect", icon: Pencil, category: "edit", isEdit: true },
    { id: "editHighlight" as ToolId, labelKey: "tools.editHighlight", icon: Pencil, category: "edit", isEdit: true },
    { id: "ocrEdit" as ToolId, labelKey: "tools.ocrEdit", icon: ScanText, category: "edit" as ToolCategory, isEdit: false },
    { id: "sign" as ToolId, labelKey: "tools.sign", icon: Pencil, category: "edit", isEdit: false },
    { id: "watermark" as ToolId, labelKey: "tools.watermark", icon: Pencil, category: "edit", isEdit: false },
    { id: "img2pdf" as ToolId, labelKey: "tools.convertImage", icon: ArrowLeftRight, category: "convert" as ToolCategory, isEdit: false },
    { id: "pdf2img" as ToolId, labelKey: "tools.pdf2img", icon: ArrowLeftRight, category: "convert", isEdit: false },
    { id: "pdf2text" as ToolId, labelKey: "tools.convertText", icon: ArrowLeftRight, category: "convert", isEdit: false },
    { id: "extractText" as ToolId, labelKey: "tools.convertText", icon: FileSearch, category: "extract" as ToolCategory, isEdit: false },
    { id: "table" as ToolId, labelKey: "tools.extractTable", icon: FileSearch, category: "extract" as ToolCategory, isEdit: false },
  ];

  let {
    activeCategory = $bindable("page" as ToolCategory),
    activeTool = $bindable(null as ToolId | null),
    ontoolaction,
  }: {
    activeCategory?: ToolCategory;
    activeTool?: ToolId | null;
    ontoolaction?: (toolId: ToolId) => void;
  } = $props();

  const categoryTools = $derived(tools.filter((t) => t.category === activeCategory));

  function selectTool(id: ToolId) {
    if (ontoolaction) {
      ontoolaction(id);
    } else {
      activeTool = activeTool === id ? null : id;
    }
  }

  function selectCategory(id: string) {
    activeCategory = id as ToolCategory;
    if (ontoolaction) return;
    activeTool = null;
  }
</script>

<div class="flex items-center h-10 px-2 border-b border-border bg-card gap-1 shrink-0 select-none overflow-x-auto">
  {#each categories as cat}
    {@const CatIcon = cat.icon}
    <button
      onclick={() => selectCategory(cat.id)}
      class="flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium transition-colors whitespace-nowrap {activeCategory === cat.id
        ? 'bg-primary/10 text-primary'
        : 'text-muted-foreground hover:bg-accent hover:text-accent-foreground'}"
    >
      <CatIcon size={14} />
      <span>{t(cat.labelKey)}</span>
    </button>
  {/each}

  <div class="w-px h-5 bg-border mx-1 shrink-0"></div>

  {#each categoryTools as tool}
    {@const ToolIcon = tool.icon}
    {@const isActive = tool.isEdit && activeTool === tool.id}
    <button
      onclick={() => selectTool(tool.id)}
      class="flex items-center gap-1 px-2 py-1.5 rounded-md text-xs transition-colors whitespace-nowrap {isActive
        ? 'bg-primary text-primary-foreground'
        : 'text-muted-foreground hover:bg-accent hover:text-accent-foreground'}"
    >
      <ToolIcon size={13} />
      <span>{t(tool.labelKey)}</span>
    </button>
  {/each}
</div>
