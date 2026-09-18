<script lang="ts">
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { invoke } from "@tauri-apps/api/core";
  import { Button } from "@/components/ui";
  import { FileUp as Icon_FileUp, Save as Icon_Save, Loader2 as Icon_Loader2 } from "lucide-svelte";

  let { filePath = $bindable(), rotateAngle = $bindable(90) }: { filePath: string | null; rotateAngle?: number } = $props();
  let busy = $state(false);
  let resultMsg = $state("");
  let resultOk = $state(false);

  async function openFileForTool() {
    const selected = await open({ filters: [{ name: "PDF", extensions: ["pdf"] }] });
    if (selected) filePath = String(selected);
  }

  async function executeRotate() {
    if (!filePath) return;
    busy = true;
    resultMsg = "";
    try {
      const out = await save({ filters: [{ name: "PDF", extensions: ["pdf"] }] });
      if (!out) return;
      const outPath = out as string;
      await invoke("rotate_pdf", { req: { inputPath: filePath, outputPath: outPath, angle: rotateAngle } });
      resultMsg = `Rotated ${rotateAngle}° → ${outPath.split(/[\\/]/).pop()}`;
      resultOk = true;
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
    } finally {
      busy = false;
    }
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
    <p class="text-sm text-muted-foreground">Rotate: <strong>{filePath.split(/[\\/]/).pop()}</strong></p>
    <div class="flex gap-2">
      {#each [90, 180, 270] as angle}
        <Button variant={rotateAngle === angle ? "default" : "outline"} size="sm" onclick={() => (rotateAngle = angle)}>{angle}°</Button>
      {/each}
    </div>
    <Button onclick={executeRotate} disabled={busy}>
      {#if busy}<Icon_Loader2 size={14} class="animate-spin" />{:else}<Icon_Save size={14} class="mr-1.5" />Rotate & Save{/if}
    </Button>
  {/if}
  {#if resultMsg}
    <div class="flex items-center gap-2 p-3 rounded-lg border {resultOk ? 'border-green-500/30 bg-green-500/5' : 'border-destructive/30 bg-destructive/5'}">
      <span class="text-sm {resultOk ? 'text-green-700' : 'text-destructive'}">{resultMsg}</span>
    </div>
  {/if}
</div>
