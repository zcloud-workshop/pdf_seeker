<script lang="ts">
  import { tabsStore, activeTab, tabCount } from "@/stores/tabs";
  import type { TabState } from "@/stores/tabs";
  import { X, Plus, FileText, Globe, Copy, Check, XCircle, ArrowRightToLine } from "lucide-svelte";
  import { Tooltip } from "@/components/ui";

  let {
    onopenfile,
    onopenurl,
    ontabchange,
    ontabclose,
  }: {
    onopenfile?: () => void;
    onopenurl?: () => void;
    ontabchange?: (filePath: string) => void;
    ontabclose?: (filePath: string | null) => void;
  } = $props();

  let contextMenu = $state<{
    visible: boolean;
    x: number;
    y: number;
    tabId: string;
    filePath: string;
  } | null>(null);

  let copied = $state(false);

  function handleClose(tabId: string, e?: MouseEvent | KeyboardEvent) {
    if (e) e.stopPropagation();
    const state = $tabsStore;
    const tab = state.tabs.find((t) => t.id === tabId);
    const wasActive = state.activeTabId === tabId;
    tabsStore.closeTab(tabId);

    // If we closed the active tab, notify parent of the new active tab
    if (wasActive) {
      const newState = $tabsStore;
      const newActive = newState.tabs.find((t) => t.id === newState.activeTabId);
      if (ontabchange) {
        ontabchange(newActive?.filePath ?? "");
      }
      if (ontabclose) {
        ontabclose(newActive?.filePath ?? null);
      }
    }
    contextMenu = null;
  }

  function handleCloseOthers(tabId: string) {
    tabsStore.closeOtherTabs(tabId);
    const state = $tabsStore;
    const active = state.tabs.find((t) => t.id === state.activeTabId);
    if (ontabchange && active) ontabchange(active.filePath);
    contextMenu = null;
  }

  function handleCloseRight(tabId: string) {
    tabsStore.closeTabsToRight(tabId);
    const state = $tabsStore;
    const active = state.tabs.find((t) => t.id === state.activeTabId);
    if (ontabchange && active) ontabchange(active.filePath);
    contextMenu = null;
  }

  function handleCopyPath(filePath: string) {
    navigator.clipboard.writeText(filePath);
    copied = true;
    setTimeout(() => {
      copied = false;
      contextMenu = null;
    }, 1000);
  }

  function handleSelect(tabId: string) {
    tabsStore.activateTab(tabId);
    const tab = $tabsStore.tabs.find((t) => t.id === tabId);
    if (tab && ontabchange) {
      ontabchange(tab.filePath);
    }
  }

  function handleContextMenu(e: MouseEvent, tab: TabState) {
    e.preventDefault();
    e.stopPropagation();
    contextMenu = {
      visible: true,
      x: e.clientX,
      y: e.clientY,
      tabId: tab.id,
      filePath: tab.filePath,
    };
  }

  function handleAuxClick(e: MouseEvent, tabId: string) {
    // Middle click closes tab
    if (e.button === 1) {
      e.preventDefault();
      handleClose(tabId);
    }
  }

  function handleWindowClick() {
    if (contextMenu) contextMenu = null;
  }
</script>

<svelte:window onclick={handleWindowClick} />

{#if $tabCount > 0}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="flex items-center h-9 bg-card border-b border-border shrink-0 overflow-x-auto select-none"
    ondblclick={(e) => {
      // If double clicking outside tabs, open file
      if (e.target === e.currentTarget || (e.target as HTMLElement).classList.contains('tabbar-empty')) {
        if (onopenfile) onopenfile();
      }
    }}
  >
    <div class="flex items-center flex-1 min-w-0 tabbar-empty h-full">
      {#each $tabsStore.tabs as tab (tab.id)}
        {@const isActive = $tabsStore.activeTabId === tab.id}
        <Tooltip message={tab.filePath}>
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            role="tab"
            tabindex="0"
            onclick={() => handleSelect(tab.id)}
            onauxclick={(e) => handleAuxClick(e, tab.id)}
            oncontextmenu={(e) => handleContextMenu(e, tab)}
            onkeydown={(e) => { if (e.key === "Enter") handleSelect(tab.id); }}
            class="group flex items-center gap-1.5 h-full px-3 text-xs font-medium border-r border-border transition-all whitespace-nowrap min-w-0 max-w-[190px] cursor-pointer select-none relative {isActive
              ? 'bg-background text-foreground shadow-sm'
              : 'text-muted-foreground hover:bg-accent/70 hover:text-accent-foreground'}"
          >
            {#if isActive}
              <div class="absolute top-0 left-0 right-0 h-0.5 bg-primary"></div>
            {/if}
            <FileText size={12} class="shrink-0 {isActive ? 'text-primary' : 'opacity-60'}" />
            <span class="truncate">{tab.fileName}</span>
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <span
              role="button"
              tabindex="-1"
              onclick={(e) => handleClose(tab.id, e)}
              onkeydown={(e) => { if (e.key === "Enter") handleClose(tab.id, e); }}
              class="shrink-0 ml-1 w-4 h-4 flex items-center justify-center rounded-sm opacity-0 group-hover:opacity-100 hover:bg-muted transition-all cursor-pointer text-muted-foreground hover:text-foreground"
              title="关闭标签页"
            >
              <X size={11} />
            </span>
          </div>
        </Tooltip>
      {/each}
      <div class="flex-1 h-full tabbar-empty"></div>
    </div>
    <Tooltip message="新建标签 / 打开本地文件">
      <button
        onclick={onopenfile}
        class="flex items-center justify-center w-8 h-8 shrink-0 text-muted-foreground hover:bg-accent hover:text-accent-foreground transition-colors"
      >
        <Plus size={14} />
      </button>
    </Tooltip>
    {#if onopenurl}
      <Tooltip message="从网络网址打开 PDF">
        <button
          onclick={onopenurl}
          class="flex items-center justify-center w-8 h-8 shrink-0 text-muted-foreground hover:bg-accent hover:text-accent-foreground transition-colors"
        >
          <Globe size={14} />
        </button>
      </Tooltip>
    {/if}
  </div>
{/if}

<!-- macOS Style Context Menu -->
{#if contextMenu && contextMenu.visible}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
    class="fixed z-50 min-w-[170px] bg-popover text-popover-foreground border border-border rounded-xl shadow-xl p-1 text-xs select-none animate-in fade-in zoom-in-95 duration-100"
    style="left: {contextMenu.x}px; top: {contextMenu.y}px;"
    onclick={(e) => e.stopPropagation()}
  >
    <button
      class="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg hover:bg-accent hover:text-accent-foreground transition-colors text-left"
      onclick={() => handleClose(contextMenu!.tabId)}
    >
      <X size={13} class="text-muted-foreground" />
      <span>关闭标签页</span>
    </button>
    <button
      class="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg hover:bg-accent hover:text-accent-foreground transition-colors text-left"
      onclick={() => handleCloseOthers(contextMenu!.tabId)}
    >
      <XCircle size={13} class="text-muted-foreground" />
      <span>关闭其他标签页</span>
    </button>
    <button
      class="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg hover:bg-accent hover:text-accent-foreground transition-colors text-left"
      onclick={() => handleCloseRight(contextMenu!.tabId)}
    >
      <ArrowRightToLine size={13} class="text-muted-foreground" />
      <span>关闭右侧标签页</span>
    </button>
    <div class="h-px bg-border my-1"></div>
    <button
      class="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg hover:bg-accent hover:text-accent-foreground transition-colors text-left"
      onclick={() => handleCopyPath(contextMenu!.filePath)}
    >
      {#if copied}
        <Check size={13} class="text-green-500" />
        <span class="text-green-500 font-medium">已复制路径</span>
      {:else}
        <Copy size={13} class="text-muted-foreground" />
        <span>复制完整路径</span>
      {/if}
    </button>
  </div>
{/if}
