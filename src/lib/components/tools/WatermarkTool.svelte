<script lang="ts">
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { invoke } from "@tauri-apps/api/core";
  import { Button, Input, Label } from "@/components/ui";
  import { FileUp as Icon_FileUp, Save as Icon_Save, Loader2 as Icon_Loader2 } from "lucide-svelte";

  let { filePath = $bindable() }: { filePath: string | null } = $props();

  let watermarkText = $state("WATERMARK");
  let watermarkFontSize = $state(48);
  let watermarkOpacity = $state(0.3);
  let watermarkAngle = $state(-45);
  let watermarkColor = $state("#888888");
  let busy = $state(false);
  let resultMsg = $state("");
  let resultOk = $state(false);

  async function openFileForTool() {
    const selected = await open({ filters: [{ name: "PDF", extensions: ["pdf"] }] });
    if (selected) filePath = String(selected);
  }

  async function executeWatermark() {
    if (!filePath) return;
    busy = true;
    resultMsg = "";
    try {
      const out = await save({ filters: [{ name: "PDF", extensions: ["pdf"] }] });
      if (!out) return;
      const outPath = out as string;
      await invoke("add_text_watermark", {
        req: { inputPath: filePath, outputPath: outPath, text: watermarkText, fontSize: watermarkFontSize, opacity: watermarkOpacity, angle: watermarkAngle, color: watermarkColor },
      });
      resultMsg = `Watermark added → ${outPath.split(/[\\/]/).pop()}`;
      resultOk = true;
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
    } finally {
      busy = false;
    }
  }

  export function getWatermarkParams() {
    return { text: watermarkText, fontSize: watermarkFontSize, opacity: watermarkOpacity, angle: watermarkAngle, color: watermarkColor };
  }
</script>

<div class="space-y-3">
  {#if !filePath}
    <p class="text-sm text-muted-foreground">Open a PDF first.</p>
    <div class="flex gap-2">
      <Button variant="outline" size="sm" onclick={openFileForTool}>
        <Icon_FileUp size={14} class="mr-1.5" />Open PDF
      </Button>
    </div>
  {:else}
    <p class="text-sm text-muted-foreground">Add watermark to: <strong>{filePath.split(/[\\/]/).pop()}</strong></p>
    <div class="flex items-center gap-2">
      <Label class="shrink-0">Text</Label>
      <Input value={watermarkText} onchange={(e) => (watermarkText = (e.target as HTMLInputElement).value)} class="flex-1" />
    </div>
    <div class="grid grid-cols-2 gap-3">
      <div class="space-y-1">
        <Label>Font Size</Label>
        <Input type="number" value={watermarkFontSize} onchange={(e) => (watermarkFontSize = parseFloat((e.target as HTMLInputElement).value) || 48)} />
      </div>
      <div class="space-y-1">
        <Label>Angle</Label>
        <Input type="number" value={watermarkAngle} onchange={(e) => (watermarkAngle = parseFloat((e.target as HTMLInputElement).value) || -45)} />
      </div>
      <div class="space-y-1">
        <Label>Opacity (0-1)</Label>
        <Input type="number" step="0.05" min="0" max="1" value={watermarkOpacity} onchange={(e) => (watermarkOpacity = parseFloat((e.target as HTMLInputElement).value) || 0.3)} />
      </div>
      <div class="space-y-1">
        <Label>Color</Label>
        <div class="flex items-center gap-2">
          <input type="color" bind:value={watermarkColor} class="w-8 h-8 rounded cursor-pointer" />
          <Input value={watermarkColor} onchange={(e) => (watermarkColor = (e.target as HTMLInputElement).value)} class="flex-1" />
        </div>
      </div>
    </div>
    <Button onclick={executeWatermark} disabled={busy || !watermarkText.trim()}>
      {#if busy}<Icon_Loader2 size={14} class="animate-spin" />{:else}<Icon_Save size={14} class="mr-1.5" />Add Watermark & Save{/if}
    </Button>
  {/if}
  {#if resultMsg}
    <div class="flex items-center gap-2 p-3 rounded-lg border {resultOk ? 'border-green-500/30 bg-green-500/5' : 'border-destructive/30 bg-destructive/5'}">
      <span class="text-sm {resultOk ? 'text-green-700' : 'text-destructive'}">{resultMsg}</span>
    </div>
  {/if}
</div>
