<script lang="ts">
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { invoke } from "@tauri-apps/api/core";
  import { Button } from "@/components/ui";
  import { FileUp as Icon_FileUp, Loader2 as Icon_Loader2 } from "lucide-svelte";

  let img2pdfFiles = $state<string[]>([]);
  let busy = $state(false);
  let resultMsg = $state("");
  let resultOk = $state(false);

  async function addImg2PdfFiles() {
    const selected = await open({
      multiple: true,
      filters: [{ name: "Images", extensions: ["jpg", "jpeg", "png", "bmp", "webp"] }],
    });
    if (selected) {
      const paths = Array.isArray(selected) ? selected.map(String) : [String(selected)];
      img2pdfFiles = [...img2pdfFiles, ...paths];
    }
  }

  function removeImg2PdfFile(index: number) {
    img2pdfFiles = img2pdfFiles.filter((_, i) => i !== index);
  }

  async function executeImg2Pdf() {
    if (img2pdfFiles.length === 0) return;
    busy = true;
    resultMsg = "";
    try {
      const out = await save({ filters: [{ name: "PDF", extensions: ["pdf"] }] });
      if (!out) return;
      const outPath = out as string;
      await invoke("images_to_pdf", { req: { imagePaths: img2pdfFiles, outputPath: outPath } });
      resultMsg = `${img2pdfFiles.length} image(s) → ${outPath.split(/[\\/]/).pop()}`;
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
  <p class="text-sm text-muted-foreground">Select images to convert to a single PDF.</p>
  <Button variant="outline" size="sm" onclick={addImg2PdfFiles} class="gap-1.5">
    <Icon_FileUp size={14} />Add Images
  </Button>
  {#if img2pdfFiles.length > 0}
    <div class="space-y-1">
      {#each img2pdfFiles as file, i}
        <div class="flex items-center gap-2 px-3 py-2 rounded-lg bg-muted text-sm">
          <span class="text-muted-foreground w-6">{i + 1}.</span>
          <span class="flex-1 truncate text-foreground">{file.split(/[\\/]/).pop()}</span>
          <button onclick={() => removeImg2PdfFile(i)} class="text-muted-foreground hover:text-destructive text-xs">Remove</button>
        </div>
      {/each}
    </div>
    <Button onclick={executeImg2Pdf} disabled={busy}>
      {#if busy}<Icon_Loader2 size={14} class="animate-spin" />{:else}Convert to PDF ({img2pdfFiles.length} images){/if}
    </Button>
  {/if}
  {#if resultMsg}
    <div class="flex items-center gap-2 p-3 rounded-lg border {resultOk ? 'border-green-500/30 bg-green-500/5' : 'border-destructive/30 bg-destructive/5'}">
      <span class="text-sm {resultOk ? 'text-green-700' : 'text-destructive'}">{resultMsg}</span>
    </div>
  {/if}
</div>
