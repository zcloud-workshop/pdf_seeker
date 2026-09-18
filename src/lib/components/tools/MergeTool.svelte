<script lang="ts">
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { invoke } from "@tauri-apps/api/core";
  import { Button } from "@/components/ui";
  import { FileUp as Icon_FileUp, Loader2 as Icon_Loader2 } from "lucide-svelte";

  let mergeFiles = $state<string[]>([]);
  let busy = $state(false);
  let resultMsg = $state("");
  let resultOk = $state(false);

  async function addMergeFiles() {
    const selected = await open({
      multiple: true,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
    if (selected) {
      const paths = Array.isArray(selected) ? selected.map(String) : [String(selected)];
      mergeFiles = [...mergeFiles, ...paths];
    }
  }

  function removeMergeFile(index: number) {
    mergeFiles = mergeFiles.filter((_, i) => i !== index);
  }

  async function executeMerge() {
    if (mergeFiles.length < 2) return;
    busy = true;
    resultMsg = "";
    try {
      const out = await save({ filters: [{ name: "PDF", extensions: ["pdf"] }] });
      if (!out) return;
      const outPath = out as string;
      await invoke("merge_pdfs", { paths: mergeFiles, outputPath: outPath });
      resultMsg = `Merged ${mergeFiles.length} files → ${outPath.split(/[\\/]/).pop()}`;
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
  <p class="text-sm text-muted-foreground">Select PDF files to merge in order.</p>
  <Button variant="outline" size="sm" onclick={addMergeFiles} class="gap-1.5">
    <Icon_FileUp size={14} />
    Add Files
  </Button>
  {#if mergeFiles.length > 0}
    <div class="space-y-1">
      {#each mergeFiles as file, i}
        <div class="flex items-center gap-2 px-3 py-2 rounded-lg bg-muted text-sm">
          <span class="text-muted-foreground w-6">{i + 1}.</span>
          <span class="flex-1 truncate text-foreground">{file.split(/[\\/]/).pop()}</span>
          <button onclick={() => removeMergeFile(i)} class="text-muted-foreground hover:text-destructive text-xs">Remove</button>
        </div>
      {/each}
    </div>
    <Button onclick={executeMerge} disabled={busy || mergeFiles.length < 2}>
      {#if busy}<Icon_Loader2 size={14} class="animate-spin" />{:else}Merge ({mergeFiles.length} files){/if}
    </Button>
  {/if}
  {#if resultMsg}
    <div class="flex items-center gap-2 p-3 rounded-lg border {resultOk ? 'border-green-500/30 bg-green-500/5' : 'border-destructive/30 bg-destructive/5'}">
      <span class="text-sm {resultOk ? 'text-green-700' : 'text-destructive'}">{resultMsg}</span>
    </div>
  {/if}
</div>
