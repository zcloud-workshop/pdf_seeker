<script lang="ts">
  import { t } from "@/i18n/index.svelte.ts";
  import {
    tabs,
    activeTabId,
    currentView,
    openTab,
    closeTab,
    activateTab,
  } from "@/stores";
  import { FileText, Plus, X } from "lucide-svelte";
  import { open, ask } from "@tauri-apps/plugin-dialog";
  import { invoke } from "@tauri-apps/api/core";
  import { get } from "svelte/store";

  async function requestClose(id: string) {
    // 07-A: a refused close means the file has a write in flight; surface it
    // instead of silently doing nothing.
    if (!closeTab(id)) {
      await ask(t("tabs.closeBlocked"), {
        title: t("tabs.closeBlockedTitle"),
        kind: "warning",
      });
    }
  }

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

  function activate(id: string) {
    activateTab(id);
    currentView.set("viewer");
  }

  function handleAuxClick(e: MouseEvent, id: string) {
    // Middle click closes the tab
    if (e.button === 1) requestClose(id);
  }

  $effect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "w") {
        const id = get(activeTabId);
        if (id) {
          e.preventDefault();
          requestClose(id);
        }
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  });
</script>

{#if $tabs.length > 0}
  <div
    class="flex items-stretch h-9 border-b border-border bg-muted/40 shrink-0 overflow-x-auto"
  >
    {#each $tabs as tab (tab.id)}
      {@const isActive = tab.id === $activeTabId}
      <div
        class="group flex items-center shrink-0 border-r border-border transition-colors {isActive
          ? 'bg-card text-foreground'
          : 'text-muted-foreground hover:bg-accent/50'}"
      >
        <button
          class="flex items-center gap-1.5 h-9 pl-3 pr-1.5 max-w-[200px]"
          title={tab.path}
          onclick={() => activate(tab.id)}
          onauxclick={(e) => handleAuxClick(e, tab.id)}
        >
          <FileText size={12} class="shrink-0" />
          <span class="truncate text-xs">{tab.name}</span>
        </button>
        <button
          class="flex items-center justify-center w-6 h-6 mr-1.5 rounded hover:bg-muted {isActive
            ? 'opacity-70 hover:opacity-100'
            : 'opacity-0 group-hover:opacity-60 hover:!opacity-100'}"
          title={t("tabs.close")}
          onclick={() => requestClose(tab.id)}
        >
          <X size={12} />
        </button>
      </div>
    {/each}
    <button
      class="flex items-center justify-center w-9 shrink-0 hover:bg-accent transition-colors"
      title={t("toolbar.openFile")}
      onclick={handleOpen}
    >
      <Plus size={14} />
    </button>
  </div>
{/if}
