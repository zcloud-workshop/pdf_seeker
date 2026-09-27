<script lang="ts">
  import { t } from "@/i18n/index.svelte.ts";
  import { open } from "@tauri-apps/plugin-dialog";
  import { Button, Tooltip } from "@/components/ui";
  import {
    FolderOpen,
    Moon,
    Sun,
    Maximize,
    Minimize,
    PanelLeftClose,
    PanelLeft,
  } from "lucide-svelte";
  import {
    currentFileName,
    currentView,
    isDark,
    isFullscreen,
    openTab,
    sidebarCollapsed,
  } from "@/stores";
  import { invoke } from "@tauri-apps/api/core";
  import { setThemePreference, themeSaveError } from "@/settings";
  let themeSaving = $state(false);

  async function handleOpen() {
    const selected = await open({
      multiple: false,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
    if (selected) {
      const path = typeof selected === "string" ? selected : String(selected);
      openTab(path);
      currentView.set("viewer");
      try {
        await invoke("add_recent_file", { path });
      } catch (_) {}
    }
  }

  async function toggleTheme() {
    if (themeSaving) return;
    themeSaving = true;
    try {
      await setThemePreference($isDark ? "light" : "dark");
    } catch {
      // The tooltip displays the persistence error.
    } finally {
      themeSaving = false;
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
  <Tooltip message={t("toolbar.toggleSidebar")}>
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

  <div class="flex-1"></div>

  {#if $currentFileName}
    <span
      class="text-xs text-muted-foreground truncate max-w-[200px] mr-2"
      title={$currentFileName}
    >
      {$currentFileName}
    </span>
  {/if}

  <Tooltip
    message={$isFullscreen ? t("toolbar.exitFullscreen") : t("toolbar.fullscreen")}
  >
    <Button variant="ghost" size="icon" onclick={toggleFullscreen}>
      {#if $isFullscreen}
        <Minimize size={18} />
      {:else}
        <Maximize size={18} />
      {/if}
    </Button>
  </Tooltip>

  <Tooltip
    message={$themeSaveError ?? ($isDark ? t("toolbar.themeLightSaved") : t("toolbar.themeDarkSaved"))}
  >
    <Button variant="ghost" size="icon" onclick={toggleTheme} disabled={themeSaving}>
      {#if $isDark}
        <Sun size={18} />
      {:else}
        <Moon size={18} />
      {/if}
    </Button>
  </Tooltip>
</header>
