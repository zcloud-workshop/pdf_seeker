<script lang="ts">
  import { Button } from "@/components/ui";

  let {
    onclose,
    onapply,
  }: {
    onclose: () => void;
    onapply: (params: { text: string; fontSize: number; opacity: number; angle: number; color: string }) => void;
  } = $props();

  let wmText = $state("DRAFT");
  let wmFontSize = $state(48);
  let wmOpacity = $state(0.3);
  let wmAngle = $state(-45);
  let wmColor = $state("#888888");

  function handleApply() {
    if (!wmText.trim()) return;
    onapply({ text: wmText, fontSize: wmFontSize, opacity: wmOpacity, angle: wmAngle, color: wmColor });
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") onclose();
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="fixed inset-0 bg-black/40 flex items-center justify-center z-50"
  onclick={(e) => { if (e.target === e.currentTarget) onclose(); }}
>
  <div class="bg-card border border-border rounded-xl shadow-xl w-[360px] p-5 space-y-4">
    <div class="flex items-center justify-between">
      <h2 class="text-sm font-medium">Add Watermark</h2>
      <button onclick={onclose} class="text-xs text-muted-foreground hover:text-foreground">X</button>
    </div>

    <div class="space-y-3">
      <div class="space-y-1">
        <label class="text-xs text-muted-foreground">Text</label>
        <input
          type="text"
          bind:value={wmText}
          placeholder="DRAFT"
          class="w-full px-2 py-1 text-sm rounded border border-input bg-transparent"
        >
      </div>

      <div class="grid grid-cols-2 gap-3">
        <div class="space-y-1">
          <label class="text-xs text-muted-foreground">Font Size</label>
          <input
            type="number"
            bind:value={wmFontSize}
            min="12" max="200"
            class="w-full px-2 py-1 text-sm rounded border border-input bg-transparent"
          >
        </div>
        <div class="space-y-1">
          <label class="text-xs text-muted-foreground">Opacity: {Math.round(wmOpacity * 100)}%</label>
          <input
            type="range"
            bind:value={wmOpacity}
            min="0.05" max="0.5" step="0.05"
            class="w-full mt-1"
          >
        </div>
      </div>

      <div class="grid grid-cols-2 gap-3">
        <div class="space-y-1">
          <label class="text-xs text-muted-foreground">Angle</label>
          <input
            type="number"
            bind:value={wmAngle}
            class="w-full px-2 py-1 text-sm rounded border border-input bg-transparent"
          >
        </div>
        <div class="space-y-1">
          <label class="text-xs text-muted-foreground">Color</label>
          <input
            type="color"
            bind:value={wmColor}
            class="w-full h-8 rounded cursor-pointer border border-input"
          >
        </div>
      </div>
    </div>

    <div class="flex justify-end gap-2 pt-2">
      <Button variant="outline" size="sm" onclick={onclose}>Cancel</Button>
      <Button size="sm" onclick={handleApply} disabled={!wmText.trim()}>Apply to All Pages</Button>
    </div>
  </div>
</div>
