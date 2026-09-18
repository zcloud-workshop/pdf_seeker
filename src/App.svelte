<script lang="ts">
  import "@/i18n/index.svelte.ts";
  import { Sidebar, Toolbar } from "@/components/layout";
  import { currentView, currentFilePath } from "@/stores";
  import Home from "$views/Home.svelte";
  import Editor from "$views/Editor.svelte";
  import Tools from "$views/Tools.svelte";
  import Storage from "$views/Storage.svelte";
  import Settings from "$views/Settings.svelte";
  import OcrSetupDialog from "@/components/OcrSetupDialog.svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { ViewName } from "@/stores";
  import { onMount } from "svelte";

  const views: Record<ViewName, typeof Home> = {
    home: Home,
    editor: Editor,
    tools: Tools,
    storage: Storage,
    settings: Settings,
  };

  let showOcrSetup = $state(false);

  onMount(async () => {
    try {
      const configured: boolean = await invoke("ocr_check_configured");
      if (!configured) {
        showOcrSetup = true;
      }
    } catch (_) {
      showOcrSetup = true;
    }
  });
</script>

<div class="flex h-screen w-screen overflow-hidden bg-background">
  <Sidebar />
  <div class="flex flex-col flex-1 min-w-0">
    {#if $currentView !== "editor" || !$currentFilePath}
      <Toolbar />
    {/if}
    <main class="flex-1 overflow-hidden">
      {#key $currentView}
        {@const CurrentView = views[$currentView]}
        <CurrentView />
      {/key}
    </main>
  </div>
</div>

{#if showOcrSetup}
  <OcrSetupDialog onDismiss={() => (showOcrSetup = false)} />
{/if}
