<script lang="ts">
  import { currentView, currentFilePath, sidebarCollapsed } from "@/stores";
  import { t } from "@/i18n/index.svelte.ts";
  import {
    Home,
    FileText,
    Wrench,
    HardDrive,
    Settings,
    HelpCircle as Icon_HelpCircle,
    ChevronLeft,
    ChevronRight,
  } from "lucide-svelte";
  import type { ViewName } from "@/stores";
  import { Tooltip } from "@/components/ui";
  import ShortcutsModal from "@/components/ShortcutsModal.svelte";

  let shortcutsOpen = $state(false);
  let userPinExpanded = $state(false);

  // Auto-collapse into mini-rail when viewing a document in editor
  const isMini = $derived(
    !userPinExpanded && ($currentView === "editor" && !!$currentFilePath)
  );

  const navItems: { view: ViewName; icon: typeof Home; labelKey: string }[] = [
    { view: "home", icon: Home, labelKey: "nav.home" },
    { view: "editor", icon: FileText, labelKey: "nav.viewer" },
    { view: "tools", icon: Wrench, labelKey: "nav.tools" },
    { view: "storage", icon: HardDrive, labelKey: "nav.storage" },
    { view: "settings", icon: Settings, labelKey: "nav.settings" },
  ];
</script>

{#if !$sidebarCollapsed}
  <aside
    class="flex flex-col h-full border-r border-border bg-card transition-all duration-200 select-none shrink-0 {isMini
      ? 'w-14'
      : 'w-52'}"
  >
    <!-- Header / Brand -->
    <div class="flex items-center h-12 border-b border-border shrink-0 {isMini ? 'justify-center px-1' : 'px-4 justify-between'}">
      {#if isMini}
        <span class="text-sm font-bold text-primary tracking-tight font-mono bg-primary/10 px-1.5 py-0.5 rounded">PS</span>
      {:else}
        <span class="text-base font-semibold text-foreground tracking-tight">PDF Seeker</span>
      {/if}
    </div>

    <!-- Navigation List -->
    <nav class="flex-1 py-3 px-1.5 {isMini ? 'flex flex-col items-center space-y-3' : 'space-y-1'}">
      {#each navItems as item}
        {@const Icon = item.icon}
        {@const isActive = $currentView === item.view}
        <Tooltip message={t(item.labelKey)}>
          <button
            onclick={() => currentView.set(item.view)}
            class="flex items-center rounded-xl transition-all duration-150 {isMini
              ? 'w-10 h-10 justify-center p-0'
              : 'w-full gap-2.5 px-3 py-2 text-sm'} {isActive
              ? 'bg-primary text-primary-foreground font-medium shadow-sm ring-2 ring-primary/20'
              : 'text-muted-foreground hover:bg-accent hover:text-accent-foreground'}"
          >
            <Icon size={19} class="shrink-0" />
            {#if !isMini}
              <span class="truncate">{t(item.labelKey)}</span>
            {/if}
          </button>
        </Tooltip>
      {/each}
    </nav>

    <!-- Bottom Actions -->
    <div class="px-1.5 pb-3 {isMini ? 'flex flex-col items-center space-y-2.5' : 'space-y-1'} shrink-0 border-t border-border/40 pt-2">
      <!-- Keyboard shortcuts -->
      <Tooltip message="快捷键 (Shortcuts)">
        <button
          onclick={() => (shortcutsOpen = true)}
          class="flex items-center rounded-xl text-muted-foreground hover:bg-accent hover:text-accent-foreground transition-colors {isMini
            ? 'w-10 h-10 justify-center p-0'
            : 'w-full gap-2.5 px-3 py-2 text-sm'}"
        >
          <Icon_HelpCircle size={18} class="shrink-0" />
          {#if !isMini}
            <span>快捷键</span>
          {/if}
        </button>
      </Tooltip>

      <!-- Mini-rail toggle -->
      {#if $currentView === "editor" && !!$currentFilePath}
        <Tooltip message={isMini ? "展开侧边栏" : "收起为迷你栏"}>
          <button
            onclick={() => (userPinExpanded = !userPinExpanded)}
            class="flex items-center rounded-xl text-xs text-muted-foreground hover:bg-accent hover:text-accent-foreground transition-colors border border-border/50 {isMini
              ? 'w-10 h-8 justify-center p-0'
              : 'w-full justify-between px-3 py-1.5'}"
          >
            {#if isMini}
              <ChevronRight size={15} />
            {:else}
              <span>收起侧边栏</span>
              <ChevronLeft size={14} />
            {/if}
          </button>
        </Tooltip>
      {/if}
    </div>
  </aside>
{/if}

<ShortcutsModal bind:open={shortcutsOpen} />
