<script lang="ts">
  import { Button, Input, Label } from "@/components/ui";
  import { Shield, Sparkles, Check, Trash2, Tag, User, FileText, Calendar } from "lucide-svelte";

  export interface PdfMetadata {
    title: string;
    author: string;
    subject: string;
    keywords: string;
    creator: string;
    producer: string;
    creationDate?: string;
    modificationDate?: string;
  }

  let {
    initialMetadata = {
      title: "",
      author: "",
      subject: "",
      keywords: "",
      creator: "PDF Seeker",
      producer: "lopdf / Rust",
    },
    onsave,
    oncancel,
  }: {
    initialMetadata?: PdfMetadata;
    onsave?: (meta: PdfMetadata) => void;
    oncancel?: () => void;
  } = $props();

  let metadata = $state<PdfMetadata>({ ...initialMetadata });

  function handleClearAll() {
    metadata = {
      title: "",
      author: "",
      subject: "",
      keywords: "",
      creator: "",
      producer: "",
    };
  }

  function handleSave() {
    if (onsave) onsave(metadata);
  }
</script>

<div class="flex flex-col gap-4 p-4 max-w-lg mx-auto text-foreground select-none">
  <div class="flex items-center justify-between border-b border-border pb-3">
    <div class="flex items-center gap-2">
      <div class="w-8 h-8 rounded-lg bg-primary/15 text-primary flex items-center justify-center">
        <Tag size={16} />
      </div>
      <div>
        <h3 class="text-sm font-semibold">元数据与文档属性管理 (Metadata Editor)</h3>
        <p class="text-xs text-muted-foreground">对标 Stirling PDF，编辑标准 PDF 属性或一键隐私彻底脱敏</p>
      </div>
    </div>
  </div>

  <div class="flex flex-col gap-3">
    <div class="space-y-1">
      <Label class="text-xs font-medium flex items-center gap-1.5">
        <FileText size={12} class="text-muted-foreground" />
        <span>文档标题 (Title)</span>
      </Label>
      <Input bind:value={metadata.title} placeholder="例如：2026 年度技术报告" class="h-8 text-xs" />
    </div>

    <div class="grid grid-cols-2 gap-3">
      <div class="space-y-1">
        <Label class="text-xs font-medium flex items-center gap-1.5">
          <User size={12} class="text-muted-foreground" />
          <span>作者 (Author)</span>
        </Label>
        <Input bind:value={metadata.author} placeholder="例如：研发架构组" class="h-8 text-xs" />
      </div>
      <div class="space-y-1">
        <Label class="text-xs font-medium flex items-center gap-1.5">
          <Tag size={12} class="text-muted-foreground" />
          <span>主题 (Subject)</span>
        </Label>
        <Input bind:value={metadata.subject} placeholder="例如：架构设计与实现" class="h-8 text-xs" />
      </div>
    </div>

    <div class="space-y-1">
      <Label class="text-xs font-medium flex items-center gap-1.5">
        <Sparkles size={12} class="text-muted-foreground" />
        <span>关键词 (Keywords) - 逗号分隔</span>
      </Label>
      <Input bind:value={metadata.keywords} placeholder="PDF, 工具箱, 离线, 安全" class="h-8 text-xs" />
    </div>

    <div class="grid grid-cols-2 gap-3">
      <div class="space-y-1">
        <Label class="text-xs font-medium text-muted-foreground">创建工具 (Creator)</Label>
        <Input bind:value={metadata.creator} class="h-8 text-xs text-muted-foreground" />
      </div>
      <div class="space-y-1">
        <Label class="text-xs font-medium text-muted-foreground">生成引擎 (Producer)</Label>
        <Input bind:value={metadata.producer} class="h-8 text-xs text-muted-foreground" />
      </div>
    </div>
  </div>

  <div class="flex items-center justify-between pt-3 border-t border-border/60 mt-1">
    <button
      onclick={handleClearAll}
      class="flex items-center gap-1 text-xs text-rose-500 hover:text-rose-600 transition-colors"
      title="一键清空所有元数据，消除个人隐私痕迹"
    >
      <Trash2 size={13} />
      <span>一键脱敏清空</span>
    </button>

    <div class="flex items-center gap-2">
      {#if oncancel}
        <Button variant="ghost" size="sm" onclick={oncancel}>
          取消
        </Button>
      {/if}
      <Button variant="default" size="sm" onclick={handleSave} class="gap-1.5">
        <Check size={14} />
        <span>保存元数据</span>
      </Button>
    </div>
  </div>
</div>

