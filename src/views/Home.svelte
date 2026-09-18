<script lang="ts">
  import { t } from "@/i18n/index.svelte.ts";
  import { Button } from "@/components/ui";
  import { FolderOpen, FileText, Wrench, Globe, Loader2, X } from "lucide-svelte";
  import { onMount } from "svelte";
  import { currentView, currentFilePath, currentFileName } from "@/stores";
  import { open } from "@tauri-apps/plugin-dialog";
  import { invoke } from "@tauri-apps/api/core";

  let recentFiles: string[] = $state([]);
  let showUrlDialog = $state(false);
  let urlInput = $state("");
  let urlLoading = $state(false);
  let urlError = $state("");

  async function handleOpenFromUrl() {
    const url = urlInput.trim();
    if (!url) return;
    urlLoading = true;
    urlError = "";
    try {
      const res: { localPath: string; fileName: string; fileSize: number } =
        await invoke("download_pdf_from_url", { url });
      currentFilePath.set(res.localPath);
      currentFileName.set(res.fileName);
      currentView.set("editor");
      try {
        await invoke("add_recent_file", { path: res.localPath });
      } catch (_) {}
      showUrlDialog = false;
      urlInput = "";
    } catch (err: any) {
      urlError = typeof err === "string" ? err : err?.message || String(err);
    } finally {
      urlLoading = false;
    }
  }

  async function loadRecent() {
    try {
      recentFiles = await invoke("get_recent_files");
    } catch (_) {
      recentFiles = [];
    }
  }

  import { isEbookFormat } from "@/utils/ebook-to-pdf";

  async function handleOpen() {
    const selected = await open({
      multiple: false,
      filters: [
        {
          name: "所有支持的文档与电子书 (*.pdf, *.epub, *.cbz, *.txt, *.md)",
          extensions: ["pdf", "epub", "cbz", "txt", "md", "markdown"],
        },
        { name: "PDF 文档 (*.pdf)", extensions: ["pdf"] },
        { name: "EPUB 电子书 (*.epub)", extensions: ["epub"] },
        { name: "漫画归档 (*.cbz)", extensions: ["cbz"] },
        { name: "纯文本与 Markdown (*.txt, *.md)", extensions: ["txt", "md", "markdown"] },
      ],
    });
    if (selected) {
      const path = typeof selected === "string" ? selected : (selected as any).path;
      currentFilePath.set(path);
      currentView.set("editor");
      try {
        await invoke("add_recent_file", { path });
        await loadRecent();
      } catch (_) {}
    }
  }

  async function handleClearRecent() {
    try {
      await invoke("clear_recent_files");
      recentFiles = [];
    } catch (_) {}
  }

  let isDragOver = $state(false);

  function handleDragOver(e: DragEvent) {
    e.preventDefault();
    e.dataTransfer!.dropEffect = "copy";
    isDragOver = true;
  }

  function handleDragLeave() {
    isDragOver = false;
  }

  async function handleDrop(e: DragEvent) {
    e.preventDefault();
    isDragOver = false;
    const file: File | undefined = e.dataTransfer?.files[0];
    if (!file) return;
    const name = file.name.toLowerCase();
    const isSupported = name.endsWith(".pdf") || isEbookFormat(name);
    if (!isSupported) return;
    const path = (file as any).path || file.name;
    currentFilePath.set(path);
    currentView.set("editor");
    try {
      await invoke("add_recent_file", { path });
      await loadRecent();
    } catch (_) {}
  }

  async function openRecent(path: string) {
    currentFilePath.set(path);
    currentView.set("editor");
    try {
      await invoke("add_recent_file", { path });
    } catch (_) {}
  }

  onMount(() => {
    loadRecent();
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="flex flex-col items-center justify-center h-full gap-8 p-8 transition-colors {isDragOver ? 'bg-primary/5 ring-2 ring-primary/30 ring-inset' : ''}"
  ondragover={handleDragOver}
  ondragleave={handleDragLeave}
  ondrop={handleDrop}
>
  <!-- Welcome area -->
  <div class="text-center space-y-2">
    <h1 class="text-3xl font-bold text-foreground">PDF Seeker</h1>
    <p class="text-muted-foreground">{t("viewer.openFileHint")}</p>
  </div>

  <!-- Open file and URL buttons -->
  <div class="flex items-center gap-3">
    <Button size="lg" onclick={handleOpen} class="gap-2 shadow-sm">
      <FolderOpen size={20} />
      <span>{t("toolbar.openFile")}</span>
    </Button>
    <Button
      size="lg"
      variant="outline"
      onclick={() => {
        showUrlDialog = true;
        urlError = "";
      }}
      class="gap-2 shadow-sm"
    >
      <Globe size={18} class="text-primary" />
      <span>从网址打开</span>
    </Button>
  </div>

  <!-- Quick actions -->
  <div class="flex gap-4 mt-2">
    <button
      onclick={() => currentView.set("tools")}
      class="flex flex-col items-center gap-2 p-4 rounded-xl border border-border hover:bg-accent/60 transition-all w-32 shadow-sm hover:shadow"
    >
      <Wrench size={28} class="text-primary" />
      <span class="text-sm font-medium text-foreground">{t("nav.tools")}</span>
    </button>
  </div>

  <!-- Recent files -->
  {#if recentFiles.length > 0}
    <div class="w-full max-w-md mt-4">
      <div class="flex items-center justify-between mb-2">
        <h2 class="text-sm font-medium text-foreground">{t("recent.title")}</h2>
        <button
          onclick={handleClearRecent}
          class="text-xs text-muted-foreground hover:text-foreground"
        >
          {t("recent.clear")}
        </button>
      </div>
      <div class="space-y-1">
        {#each recentFiles as file}
          <button
            onclick={() => openRecent(file)}
            class="w-full flex items-center gap-2 px-3 py-2 rounded-lg text-sm text-left hover:bg-accent transition-colors"
          >
            <FileText size={16} class="text-muted-foreground shrink-0" />
            <span class="truncate text-muted-foreground">{file.split(/[\\/]/).pop()}</span>
          </button>
        {/each}
      </div>
    </div>
  {/if}
</div>

{#if showUrlDialog}
  <!-- URL Input Dialog Modal -->
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm animate-in fade-in">
    <div class="w-full max-w-md p-6 bg-card border border-border rounded-2xl shadow-2xl space-y-4">
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-2 text-foreground font-semibold">
          <Globe size={18} class="text-primary" />
          <span>从网络网址打开 PDF</span>
        </div>
        <button
          onclick={() => (showUrlDialog = false)}
          class="p-1 rounded-md text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
        >
          <X size={16} />
        </button>
      </div>

      <p class="text-xs text-muted-foreground leading-relaxed">
        输入远程 PDF 文件的完整 HTTP 或 HTTPS 地址。下载后将在查看器中直接载入，您可以在编辑或批注后保存至本地。
      </p>

      <div class="space-y-1.5">
        <input
          bind:value={urlInput}
          onkeydown={(e) => {
            if (e.key === 'Enter') handleOpenFromUrl();
            if (e.key === 'Escape') showUrlDialog = false;
          }}
          type="url"
          placeholder="https://example.com/sample.pdf"
          class="w-full px-3.5 py-2.5 bg-muted/40 border border-border rounded-xl text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-primary/40"
        />
        {#if urlError}
          <p class="text-xs text-destructive font-medium px-1 pt-1">{urlError}</p>
        {/if}
      </div>

      <div class="flex items-center justify-end gap-2.5 pt-2">
        <Button variant="ghost" size="sm" onclick={() => (showUrlDialog = false)}>
          取消
        </Button>
        <Button
          variant="default"
          size="sm"
          onclick={handleOpenFromUrl}
          disabled={urlLoading || !urlInput.trim()}
          class="gap-1.5"
        >
          {#if urlLoading}
            <Loader2 size={14} class="animate-spin" />
            <span>下载中...</span>
          {:else}
            <span>下载并打开</span>
          {/if}
        </Button>
      </div>
    </div>
  </div>
{/if}
