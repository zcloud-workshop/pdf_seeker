<script lang="ts">
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { invoke } from "@tauri-apps/api/core";
  import { Button } from "@/components/ui";
  import { FileUp as Icon_FileUp, Save as Icon_Save, Loader2 as Icon_Loader2 } from "lucide-svelte";

  let { filePath = $bindable() }: { filePath: string | null } = $props();

  let pageOrder = $state<number[]>([]);
  let busy = $state(false);
  let resultMsg = $state("");
  let resultOk = $state(false);

  $effect(() => {
    if (filePath) loadPageCount();
  });

  async function loadPageCount() {
    try {
      const { readFile } = await import("@tauri-apps/plugin-fs");
      const { loadPdf } = await import("@/pdf-engine");
      const data = await readFile(filePath!);
      const doc = await loadPdf(new Uint8Array(data));
      pageOrder = Array.from({ length: doc.numPages }, (_, i) => i + 1);
    } catch (_) {}
  }

  async function openFileForTool() {
    const selected = await open({ filters: [{ name: "PDF", extensions: ["pdf"] }] });
    if (selected) filePath = String(selected);
  }

  async function executeReorder() {
    if (!filePath || pageOrder.length === 0) return;
    busy = true;
    resultMsg = "";
    try {
      const out = await save({ filters: [{ name: "PDF", extensions: ["pdf"] }] });
      if (!out) return;
      await invoke("reorder_pages", { req: { inputPath: filePath, outputPath: out, newOrder: pageOrder } });
      resultMsg = `Pages reordered → ${(out as string).split(/[\\/]/).pop()}`;
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
    <p class="text-sm text-muted-foreground">Drag and drop pages to reorder: <strong>{filePath.split(/[\\/]/).pop()}</strong></p>
    {#if pageOrder.length > 0}
      <p class="text-xs text-muted-foreground">Current order: {pageOrder.join(", ")}</p>
    {/if}
    <Button onclick={executeReorder} disabled={busy || pageOrder.length === 0}>
      {#if busy}<Icon_Loader2 size={14} class="animate-spin" />{:else}<Icon_Save size={14} class="mr-1.5" />Reorder & Save{/if}
    </Button>
  {/if}
  {#if resultMsg}
    <div class="flex items-center gap-2 p-3 rounded-lg border {resultOk ? 'border-green-500/30 bg-green-500/5' : 'border-destructive/30 bg-destructive/5'}">
      <span class="text-sm {resultOk ? 'text-green-700' : 'text-destructive'}">{resultMsg}</span>
    </div>
  {/if}
</div>
