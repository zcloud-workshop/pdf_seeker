<script lang="ts">
  import "@/i18n/index.svelte.ts";
  import type { Component } from "svelte";
  import { Sidebar, Toolbar, TabBar } from "@/components/layout";
  import { currentView } from "@/stores";
  import Home from "$views/Home.svelte";
  import Viewer from "$views/Viewer.svelte";
  import Tools from "$views/Tools.svelte";
  import Storage from "$views/Storage.svelte";
  import Settings from "$views/Settings.svelte";
  import type { ViewName } from "@/stores";

  const views: Record<ViewName, Component> = {
    home: Home,
    viewer: Viewer,
    tools: Tools,
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
