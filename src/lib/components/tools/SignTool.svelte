<script lang="ts">
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { invoke } from "@tauri-apps/api/core";
  import { Button, Input, Label } from "@/components/ui";
  import { FileUp as Icon_FileUp, Save as Icon_Save, Loader2 as Icon_Loader2 } from "lucide-svelte";

  let { filePath = $bindable(), previewPage = $bindable(1) }: { filePath: string | null; previewPage?: number } = $props();

  let signImagePath = $state("");
  let signX = $state(400);
  let signY = $state(50);
  let signWidth = $state(150);
  let signHeight = $state(50);
  let busy = $state(false);
  let resultMsg = $state("");
  let resultOk = $state(false);

  async function openFileForTool() {
    const selected = await open({ filters: [{ name: "PDF", extensions: ["pdf"] }] });
    if (selected) filePath = String(selected);
  }

  async function selectSignImage() {
    const selected = await open({ filters: [{ name: "Images", extensions: ["jpg", "jpeg", "png"] }] });
    if (selected) signImagePath = String(selected);
  }

  async function executeSign() {
    if (!filePath || !signImagePath) return;
    busy = true;
    resultMsg = "";
    try {
      const out = await save({ filters: [{ name: "PDF", extensions: ["pdf"] }] });
      if (!out) return;
      const outPath = out as string;
      await invoke("sign_pdf", {
        req: { inputPath: filePath, outputPath: outPath, signatureImagePath: signImagePath, page: previewPage, x: signX, y: signY, width: signWidth, height: signHeight },
      });
      resultMsg = `Signature added to page ${previewPage} → ${outPath.split(/[\\/]/).pop()}`;
      resultOk = true;
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
    } finally {
      busy = false;
    }
  }

  export function getSignParams() {
    return { imagePath: signImagePath, x: signX, y: signY, width: signWidth, height: signHeight };
  }

  export function updateFromPointer(params: { pdfX: number; pdfY: number; pdfRight: number; pdfTop: number; isClick: boolean }) {
    signX = params.pdfX;
    signY = params.isClick ? params.pdfY - 50 : params.pdfY;
    signWidth = params.isClick ? 150 : Math.max(50, params.pdfRight - params.pdfX);
    signHeight = params.isClick ? 50 : Math.max(20, params.pdfTop - params.pdfY);
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
    <p class="text-sm text-muted-foreground">Sign: <strong>{filePath.split(/[\\/]/).pop()}</strong></p>
    <Button variant="outline" size="sm" onclick={selectSignImage} class="gap-1.5">
      <Icon_FileUp size={14} />
      {signImagePath ? signImagePath.split(/[\\/]/).pop() : "Select Signature Image"}
    </Button>
    <div class="grid grid-cols-2 gap-3">
      <div class="space-y-1">
        <Label>Page</Label>
        <Input type="number" min="1" value={previewPage} onchange={(e) => (previewPage = parseInt((e.target as HTMLInputElement).value) || 1)} />
      </div>
      <div class="space-y-1"></div>
      <div class="space-y-1"><Label>X Position</Label><Input type="number" value={signX} onchange={(e) => (signX = parseFloat((e.target as HTMLInputElement).value) || 0)} /></div>
      <div class="space-y-1"><Label>Y Position</Label><Input type="number" value={signY} onchange={(e) => (signY = parseFloat((e.target as HTMLInputElement).value) || 0)} /></div>
      <div class="space-y-1"><Label>Width</Label><Input type="number" value={signWidth} onchange={(e) => (signWidth = parseFloat((e.target as HTMLInputElement).value) || 150)} /></div>
      <div class="space-y-1"><Label>Height</Label><Input type="number" value={signHeight} onchange={(e) => (signHeight = parseFloat((e.target as HTMLInputElement).value) || 50)} /></div>
    </div>
    <Button onclick={executeSign} disabled={busy || !signImagePath}>
      {#if busy}<Icon_Loader2 size={14} class="animate-spin" />{:else}<Icon_Save size={14} class="mr-1.5" />Sign & Save{/if}
    </Button>
  {/if}
  {#if resultMsg}
    <div class="flex items-center gap-2 p-3 rounded-lg border {resultOk ? 'border-green-500/30 bg-green-500/5' : 'border-destructive/30 bg-destructive/5'}">
      <span class="text-sm {resultOk ? 'text-green-700' : 'text-destructive'}">{resultMsg}</span>
    </div>
  {/if}
</div>
