<script lang="ts">
  import { currentView, sidebarCollapsed } from "@/stores";
  import { t } from "@/i18n/index.svelte.ts";
  import {
    Home,
    FileText,
    Wrench,
    HardDrive,
    Settings,
  } from "lucide-svelte";
  import type { ViewName } from "@/stores";
  import { Tooltip } from "@/components/ui";

  const navItems: { view: ViewName; icon: typeof Home; labelKey: string }[] = [
    { view: "home", icon: Home, labelKey: "nav.home" },
    { view: "viewer", icon: FileText, labelKey: "nav.viewer" },
    { view: "tools", icon: Wrench, labelKey: "nav.tools" },
    { view: "storage", icon: HardDrive, labelKey: "nav.storage" },
    { view: "settings", icon: Settings, labelKey: "nav.settings" },
  ];
</script>

{#if !$sidebarCollapsed}
  <aside class="flex flex-col h-full w-52 border-r border-border bg-card transition-all duration-200">
    <div class="flex items-center h-12 px-4 border-b border-border">
      <span class="text-base font-semibold text-foreground tracking-tight">PDF Seeker</span>
    </div>
    <nav class="flex-1 py-2 px-2 space-y-0.5">
      {#each navItems as item}
        {@const Icon = item.icon}
        <Tooltip message={t(item.labelKey)}>
          <button
            onclick={() => currentView.set(item.view)}
            class="w-full flex items-center gap-2.5 px-3 py-2 rounded-lg text-sm transition-colors {$currentView ===
            item.view
              ? 'bg-primary text-primary-foreground'
              : 'text-muted-foreground hover:bg-accent hover:text-accent-foreground'}"
          >
            <Icon size={18} />
            <span>{t(item.labelKey)}</span>
          </button>
        </Tooltip>
      {/each}
    </nav>
  </aside>
{/if}
