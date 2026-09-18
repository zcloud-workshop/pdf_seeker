<script lang="ts">
  import { t } from "@/i18n/index.svelte.ts";
  import { open } from "@tauri-apps/plugin-dialog";
  import { Button, Tooltip } from "@/components/ui";
  import {
    FolderOpen,
    Save,
    Download,
    Moon,
    Sun,
    Maximize,
    Minimize,
    PanelLeftClose,
    PanelLeft,
    Info,
    X,
  } from "lucide-svelte";
  import {
    currentFilePath,
    currentFileName,
    currentView,
    isDark,
    toggleTheme,
    isFullscreen,
    sidebarCollapsed,
  } from "@/stores";
  import { invoke } from "@tauri-apps/api/core";
  import PdfInfoDialog from "@/components/editor/PdfInfoDialog.svelte";

  let infoDialogOpen = $state(false);

  function handleCloseDocument() {
    currentFilePath.set("");
    currentFileName.set("");
    currentView.set("home");
  }

  function handleKeydown(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key === "i" && $currentFilePath) {
      e.preventDefault();
      infoDialogOpen = true;
    }
  }

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
      const path = typeof selected === "string" ? selected : String(selected);
      currentFilePath.set(path);
      currentFileName.set(path.split(/[\\/]/).pop() || "Untitled");
      currentView.set("editor");
      try {
        await invoke("add_recent_file", { path });
      } catch (_) {}
    }
  }


  function toggleFullscreen() {
    if (document.fullscreenElement) {
      document.exitFullscreen();
    } else {
      document.documentElement.requestFullscreen();
    }
  }

  $effect(() => {
    const handler = () => isFullscreen.set(!!document.fullscreenElement);
    document.addEventListener("fullscreenchange", handler);
    return () => document.removeEventListener("fullscreenchange", handler);
  });
</script>

<header
  class="flex items-center h-12 px-3 border-b border-border bg-card gap-1 shrink-0 select-none"
>
  <Tooltip message="Toggle Sidebar">
    <Button variant="ghost" size="icon" onclick={() => sidebarCollapsed.update((v) => !v)}>
      {#if $sidebarCollapsed}
        <PanelLeft size={18} />
      {:else}
        <PanelLeftClose size={18} />
      {/if}
    </Button>
  </Tooltip>

  <Tooltip message={t("toolbar.openFile")}>
    <Button variant="ghost" size="sm" onclick={handleOpen} class="gap-1.5">
      <FolderOpen size={16} />
      <span class="text-xs">{t("toolbar.openFile")}</span>
    </Button>
  </Tooltip>

  <Tooltip message={t("toolbar.saveFile")}>
    <Button variant="ghost" size="icon" disabled>
      <Save size={16} />
    </Button>
  </Tooltip>

  <Tooltip message={t("toolbar.export")}>
    <Button variant="ghost" size="icon" disabled>
      <Download size={16} />
    </Button>
  </Tooltip>

  <div class="flex-1"></div>

  {#if $currentFilePath}
    <div class="flex items-center gap-1.5 bg-accent/30 hover:bg-accent/50 px-2.5 py-1 rounded-lg border border-border/50 text-xs transition-colors mr-2">
      <span
        class="text-xs font-medium text-foreground truncate max-w-[180px]"
        title={$currentFilePath}
      >
        {$currentFileName}
      </span>
      <Tooltip message="文档属性 (⌘I)">
        <button
          onclick={() => (infoDialogOpen = true)}
          class="p-1 rounded-md text-muted-foreground hover:text-foreground hover:bg-background/80 transition-colors"
        >
          <Info size={13} />
        </button>
      </Tooltip>
      <div class="h-3 w-px bg-border/60"></div>
      <Tooltip message="关闭当前文档">
        <button
          onclick={handleCloseDocument}
          class="p-1 rounded-md text-muted-foreground hover:text-destructive hover:bg-destructive/10 transition-colors"
        >
          <X size={13} />
        </button>
      </Tooltip>
    </div>
  {/if}

  <Tooltip message={$isDark ? t("settings.themeLight") : t("settings.themeDark")}>
    <Button variant="ghost" size="icon" onclick={toggleTheme}>
      {#if $isDark}
        <Sun size={18} />
      {:else}
        <Moon size={18} />
      {/if}
    </Button>
  </Tooltip>
</header>

<svelte:window onkeydown={handleKeydown} />

<PdfInfoDialog bind:open={infoDialogOpen} />
