<script lang="ts">
  import { t } from "@/i18n/index.svelte.ts";
  import { tabs, activeTabId, openTab } from "@/stores";
  import { Button } from "@/components/ui";
  import { FileText } from "lucide-svelte";
  import PdfScroller from "@/components/viewer/PdfScroller.svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { invoke } from "@tauri-apps/api/core";

  async function handleOpen() {
    const selected = await open({
      multiple: false,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
    if (selected) {
      const path = typeof selected === "string" ? selected : String(selected);
      openTab(path);
      try {
        await invoke("add_recent_file", { path });
      } catch (_) {}
    }
  }
</script>

<div class="h-full relative">
  {#if $tabs.length === 0}
    <div class="flex flex-col items-center justify-center h-full gap-4">
      <FileText size={48} class="text-muted-foreground/30" />
      <p class="text-muted-foreground">{t("viewer.noFile")}</p>
      <Button variant="outline" onclick={handleOpen}>
        {t("toolbar.openFile")}
      </Button>
    </div>
  {:else}
    {#each $tabs as tab (tab.id)}
      <div
        class="absolute inset-0"
        class:hidden={tab.id !== $activeTabId}
      >
        <PdfScroller {tab} active={tab.id === $activeTabId} />
      </div>
    {/each}
  {/if}
</div>
