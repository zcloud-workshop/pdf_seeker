<script lang="ts">
  import { t } from "@/i18n/index.svelte.ts";
  import { Button } from "@/components/ui";
  import { FolderOpen, FileText, Wrench } from "lucide-svelte";
  import { currentView, currentFilePath } from "@/stores";
  import { open } from "@tauri-apps/plugin-dialog";
  import { invoke } from "@tauri-apps/api/core";

  let recentFiles: string[] = $state([]);

  async function loadRecent() {
    try {
      recentFiles = await invoke("get_recent_files");
    } catch (_) {
      recentFiles = [];
    }
  }

  async function handleOpen() {
    const selected = await open({
      multiple: false,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
    if (selected) {
      const path = typeof selected === "string" ? selected : selected.path;
      currentFilePath.set(path);
      currentView.set("viewer");
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

  async function openRecent(path: string) {
    currentFilePath.set(path);
    currentView.set("viewer");
    try {
      await invoke("add_recent_file", { path });
    } catch (_) {}
  }

  $effect(() => {
    loadRecent();
  });
</script>

<div class="flex flex-col items-center justify-center h-full gap-8 p-8">
  <!-- Welcome area -->
  <div class="text-center space-y-2">
    <h1 class="text-3xl font-bold text-foreground">PDF Seeker</h1>
    <p class="text-muted-foreground">{t("viewer.openFileHint")}</p>
  </div>

  <!-- Open file button -->
  <Button size="lg" onclick={handleOpen} class="gap-2">
    <FolderOpen size={20} />
    <span>{t("toolbar.openFile")}</span>
  </Button>

  <!-- Quick actions -->
  <div class="flex gap-4 mt-4">
    <button
      onclick={() => (currentView.set("tools"))}
      class="flex flex-col items-center gap-2 p-4 rounded-xl border border-border hover:bg-accent transition-colors w-32"
    >
      <Wrench size={28} class="text-muted-foreground" />
      <span class="text-sm text-muted-foreground">{t("nav.tools")}</span>
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
