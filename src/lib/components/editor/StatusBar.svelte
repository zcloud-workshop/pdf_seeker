<script lang="ts">
  import { currentPage, totalPages, zoomLevel, isFullscreen } from "@/stores";
  import { Button, Tooltip } from "@/components/ui";
  import { ZoomIn, ZoomOut, ChevronLeft, ChevronRight, Minimize, Maximize, Search, BookOpen } from "lucide-svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";

  let {
    onopensearch,
    onopenzen,
  }: {
    onopensearch?: () => void;
    onopenzen?: () => void;
  } = $props();
  let visible = $state(true);

  function prevPage() {
    if ($currentPage > 1) currentPage.set($currentPage - 1);
  }

  function nextPage() {
    if ($currentPage < $totalPages) currentPage.set($currentPage + 1);
  }

  function zoomIn() {
    zoomLevel.set(Math.min(Math.round(($zoomLevel + 0.25) * 20) / 20, 5));
  }

  function zoomOut() {
    zoomLevel.set(Math.max(Math.round(($zoomLevel - 0.25) * 20) / 20, 0.25));
  }

  function resetZoom() {
    zoomLevel.set(1.0);
  }

  async function toggleFullscreen() {
    try {
      const win = getCurrentWindow();
      const current = await win.isFullscreen();
      await win.setFullscreen(!current);
    } catch {}
  }

  export function setVisible(v: boolean) {
    visible = v;
  }
</script>

<div
  class="flex items-center justify-center h-10 px-4 border-t border-border bg-card gap-1 shrink-0 transition-opacity duration-300 {visible
    ? 'opacity-100'
    : 'opacity-0 pointer-events-none'}"
>
  <Tooltip message="Previous Page">
    <Button variant="ghost" size="icon" onclick={prevPage} disabled={$currentPage <= 1}>
      <ChevronLeft size={14} />
    </Button>
  </Tooltip>

  <span class="text-xs text-muted-foreground min-w-[80px] text-center tabular-nums">
    {$currentPage} / {$totalPages}
  </span>

  <Tooltip message="Next Page">
    <Button variant="ghost" size="icon" onclick={nextPage} disabled={$currentPage >= $totalPages}>
      <ChevronRight size={14} />
    </Button>
  </Tooltip>

  <div class="w-px h-4 bg-border mx-1"></div>

  <Tooltip message="Zoom Out">
    <Button variant="ghost" size="icon" onclick={zoomOut}>
      <ZoomOut size={14} />
    </Button>
  </Tooltip>

  <button
    onclick={resetZoom}
    class="text-xs text-muted-foreground hover:text-foreground min-w-[42px] text-center tabular-nums transition-colors"
  >
    {Math.round($zoomLevel * 100)}%
  </button>

  <Tooltip message="Zoom In">
    <Button variant="ghost" size="icon" onclick={zoomIn}>
      <ZoomIn size={14} />
    </Button>
  </Tooltip>

  <div class="flex-1"></div>

  {#if onopensearch}
    <Tooltip message="全文搜索 (⌘F)">
      <Button variant="ghost" size="icon" onclick={onopensearch}>
        <Search size={14} />
      </Button>
    </Tooltip>
  {/if}

  {#if onopenzen}
    <Tooltip message="沉浸阅读模式 (⌘Shift+F)">
      <Button variant="ghost" size="icon" onclick={onopenzen} class="text-primary hover:text-primary" title="沉浸阅读模式">
        <BookOpen size={14} />
      </Button>
    </Tooltip>
  {/if}

  <Tooltip message={$isFullscreen ? "退出窗口全屏" : "切换窗口全屏 (F11)"}>
    <Button variant="ghost" size="icon" onclick={toggleFullscreen}>
      {#if $isFullscreen}
        <Minimize size={14} />
      {:else}
        <Maximize size={14} />
      {/if}
    </Button>
  </Tooltip>
</div>
