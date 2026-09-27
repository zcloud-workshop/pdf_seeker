<script lang="ts">
  import "@/i18n/index.svelte.ts";
  import { onMount } from "svelte";
  import { listenForSystemTheme } from "@/settings";
  import { t } from "@/i18n/index.svelte.ts";
  import { anyWriteBusy } from "@/document/session.svelte.ts";
  import { ask } from "@tauri-apps/plugin-dialog";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import type { Component } from "svelte";
  import { Sidebar, Toolbar, TabBar } from "@/components/layout";
  import { currentView } from "@/stores";
  import Home from "$views/Home.svelte";
  import Viewer from "$views/Viewer.svelte";
  import Tools from "$views/Tools.svelte";
  import Compare from "$views/Compare.svelte";
  import Storage from "$views/Storage.svelte";
  import Settings from "$views/Settings.svelte";
  import type { ViewName } from "@/stores";

  onMount(() => {
    const cleanupTheme = listenForSystemTheme();
    // 07-A: coordinate window close with in-flight PDF writes. The atomic
    // save path keeps files uncorrupted either way; quitting mid-write only
    // abandons the task, so confirm rather than block.
    let unlistenClose: (() => void) | undefined;
    void getCurrentWindow()
      .onCloseRequested(async (event) => {
        if (!anyWriteBusy()) return;
        event.preventDefault();
        const quit = await ask(t("app.exitWhileEditing"), {
          title: t("app.exitWhileEditingTitle"),
          kind: "warning",
        });
        if (quit) await getCurrentWindow().destroy();
      })
      .then((unlisten) => {
        unlistenClose = unlisten;
      });
    return () => {
      cleanupTheme();
      unlistenClose?.();
    };
  });

  const views: Record<ViewName, Component> = {
    home: Home,
    viewer: Viewer,
    tools: Tools,
    compare: Compare,
    storage: Storage,
    settings: Settings,
  };
</script>

<div class="flex h-screen w-screen overflow-hidden bg-background">
  <Sidebar />
  <div class="flex flex-col flex-1 min-w-0">
    <Toolbar />
    <TabBar />
    <main class="flex-1 overflow-hidden">
      {#key $currentView}
        {@const CurrentView = views[$currentView]}
        <CurrentView />
      {/key}
    </main>
  </div>
</div>
