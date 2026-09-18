<script lang="ts">
  import { t } from "@/i18n/index.svelte.ts";
  import { Button, Input, Label } from "@/components/ui";
  import { Crop, Check, X, RotateCcw, AlertCircle } from "lucide-svelte";
  import type { PdfDocumentProxy } from "@/pdf-engine";

  let {
    doc,
    currentPage = 1,
    totalPages = 1,
    onapplycrop,
    oncancel,
  }: {
    doc: PdfDocumentProxy | null;
    currentPage?: number;
    totalPages?: number;
    onapplycrop?: (params: { pageRange: string; top: number; right: number; bottom: number; left: number }) => void;
    oncancel?: () => void;
  } = $props();

  let previewCanvas = $state<HTMLCanvasElement | null>(null);
  let selectedPage = $state(currentPage);
  let pageRange = $state<"current" | "all">("current");

  // Margin percentages (0 - 45%)
  let cropTop = $state(10);
  let cropRight = $state(10);
  let cropBottom = $state(10);
  let cropLeft = $state(10);

  let canvasW = $state(320);
  let canvasH = $state(450);

  $effect(() => {
    if (doc && previewCanvas) {
      renderPreview(selectedPage);
    }
  });

  async function renderPreview(pageNum: number) {
    if (!doc || !previewCanvas) return;
    try {
      const page = await doc.getPage(pageNum);
      const vp = page.getViewport({ scale: 1.0 });
      const scale = Math.min(320 / vp.width, 460 / vp.height);
      const scaledVp = page.getViewport({ scale });

      previewCanvas.width = scaledVp.width;
      previewCanvas.height = scaledVp.height;
      canvasW = scaledVp.width;
      canvasH = scaledVp.height;

      const ctx = previewCanvas.getContext("2d");
      if (ctx) {
        await page.render({ canvasContext: ctx, viewport: scaledVp }).promise;
      }
    } catch (e) {
      console.error("Failed to render crop preview:", e);
    }
  }

  function handleReset() {
    cropTop = 0;
    cropRight = 0;
    cropBottom = 0;
    cropLeft = 0;
  }

  function handleApply() {
    if (onapplycrop) {
      onapplycrop({
        pageRange: pageRange === "current" ? `${selectedPage}` : "all",
        top: cropTop,
        right: cropRight,
        bottom: cropBottom,
        left: cropLeft,
      });
    }
  }
</script>

<div class="flex flex-col gap-5 p-4 max-w-2xl mx-auto text-foreground select-none">
  <div class="flex items-center justify-between border-b border-border pb-3">
    <div class="flex items-center gap-2">
      <div class="w-8 h-8 rounded-lg bg-primary/15 text-primary flex items-center justify-center">
        <Crop size={16} />
      </div>
      <div>
        <h3 class="text-sm font-semibold">可视化页面裁剪 (Crop Tool)</h3>
        <p class="text-xs text-muted-foreground">调整裁切边距或直接框选，去除多余白边或装订孔留白</p>
      </div>
    </div>
    <span class="text-xs font-mono px-2 py-0.5 rounded bg-muted text-muted-foreground">
      第 {selectedPage} / {totalPages} 页
    </span>
  </div>

  <div class="grid grid-cols-1 md:grid-cols-2 gap-6 items-center">
    <!-- Visual Crop Preview Canvas -->
    <div class="flex flex-col items-center justify-center bg-muted/30 border border-border/80 rounded-xl p-3 min-h-[360px] relative overflow-hidden">
      <div class="relative shadow-md rounded overflow-hidden" style="width: {canvasW}px; height: {canvasH}px;">
        <canvas bind:this={previewCanvas} class="block w-full h-full"></canvas>

        <!-- Visual Crop Overlay Mask -->
        <div
          class="absolute inset-0 pointer-events-none border-2 border-primary border-dashed bg-primary/10 transition-all"
          style="
            top: {cropTop}%;
            right: {cropRight}%;
            bottom: {cropBottom}%;
            left: {cropLeft}%;
          "
        >
          <div class="absolute -top-2 -left-2 w-3.5 h-3.5 bg-primary rounded-full border-2 border-white shadow"></div>
          <div class="absolute -top-2 -right-2 w-3.5 h-3.5 bg-primary rounded-full border-2 border-white shadow"></div>
          <div class="absolute -bottom-2 -left-2 w-3.5 h-3.5 bg-primary rounded-full border-2 border-white shadow"></div>
          <div class="absolute -bottom-2 -right-2 w-3.5 h-3.5 bg-primary rounded-full border-2 border-white shadow"></div>
        </div>
      </div>
      <span class="text-[11px] text-muted-foreground mt-2">虚线框内为最终保留区域</span>
    </div>

    <!-- Numeric Adjustments & Scope -->
    <div class="flex flex-col gap-4">
      <div class="space-y-1.5">
        <Label class="text-xs font-medium">应用范围</Label>
        <div class="flex gap-2">
          <Button
            variant={pageRange === "current" ? "default" : "outline"}
            size="sm"
            class="flex-1 text-xs"
            onclick={() => (pageRange = "current")}
          >
            仅当前页 (P.{selectedPage})
          </Button>
          <Button
            variant={pageRange === "all" ? "default" : "outline"}
            size="sm"
            class="flex-1 text-xs"
            onclick={() => (pageRange = "all")}
          >
            全部页面 ({totalPages} 页)
          </Button>
        </div>
      </div>

      <div class="space-y-2.5 pt-1">
        <Label class="text-xs font-medium">裁切边距 (百分比 %)</Label>
        <div class="grid grid-cols-2 gap-2.5 text-xs">
          <div>
            <span class="text-muted-foreground">上边距 (Top): {cropTop}%</span>
            <input type="range" min="0" max="40" bind:value={cropTop} class="w-full accent-primary mt-1" />
          </div>
          <div>
            <span class="text-muted-foreground">下边距 (Bottom): {cropBottom}%</span>
            <input type="range" min="0" max="40" bind:value={cropBottom} class="w-full accent-primary mt-1" />
          </div>
          <div>
            <span class="text-muted-foreground">左边距 (Left): {cropLeft}%</span>
            <input type="range" min="0" max="40" bind:value={cropLeft} class="w-full accent-primary mt-1" />
          </div>
          <div>
            <span class="text-muted-foreground">右边距 (Right): {cropRight}%</span>
            <input type="range" min="0" max="40" bind:value={cropRight} class="w-full accent-primary mt-1" />
          </div>
        </div>
      </div>

      <div class="flex items-center justify-between pt-3 border-t border-border/60">
        <button
          onclick={handleReset}
          class="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
        >
          <RotateCcw size={13} />
          <span>重置为全幅</span>
        </button>

        <div class="flex items-center gap-2">
          {#if oncancel}
            <Button variant="ghost" size="sm" onclick={oncancel}>
              取消
            </Button>
          {/if}
          <Button variant="default" size="sm" onclick={handleApply} class="gap-1.5">
            <Check size={14} />
            <span>执行裁剪</span>
          </Button>
        </div>
      </div>
    </div>
  </div>
</div>

