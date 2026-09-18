<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { invoke } from "@tauri-apps/api/core";
  import { Button, Input } from "@/components/ui";
  import { FileUp as Icon_FileUp, FolderOpen as Icon_FolderOpen, Loader2 as Icon_Loader2 } from "lucide-svelte";

  let { filePath = $bindable() }: { filePath: string | null } = $props();

  let splitMode = $state<"single" | "range">("single");
  let splitRanges = $state("");
  let busy = $state(false);
  let resultMsg = $state("");
  let resultOk = $state(false);

  async function openFileForTool() {
    const selected = await open({ filters: [{ name: "PDF", extensions: ["pdf"] }] });
    if (selected) filePath = String(selected);
  }

  async function executeSplit() {
    if (!filePath) return;
    busy = true;
    resultMsg = "";
    try {
      const selected = await open({ directory: true });
      if (!selected) return;
      const outputDir = String(selected);
      const outPaths = await invoke<string[]>("split_pdf", {
        req: { inputPath: filePath, outputDir, mode: splitMode, ranges: splitMode === "range" ? splitRanges : null },
      });
      resultMsg = `Split into ${outPaths.length} file(s)`;
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
    <p class="text-sm text-muted-foreground">Split: <strong>{filePath.split(/[\\/]/).pop()}</strong></p>
    <div class="flex gap-2">
      <Button variant={splitMode === "single" ? "default" : "outline"} size="sm" onclick={() => (splitMode = "single")}>One file per page</Button>
      <Button variant={splitMode === "range" ? "default" : "outline"} size="sm" onclick={() => (splitMode = "range")}>By ranges</Button>
    </div>
    {#if splitMode === "range"}
      <div class="flex items-center gap-2">
        <span class="text-sm shrink-0">Ranges</span>
        <Input value={splitRanges} onchange={(e) => (splitRanges = (e.target as HTMLInputElement).value)} placeholder="e.g. 1-3,4-6,7-10" class="flex-1" />
      </div>
    {/if}
    <Button onclick={executeSplit} disabled={busy || (splitMode === "range" && !splitRanges.trim())}>
      {#if busy}<Icon_Loader2 size={14} class="animate-spin" />{:else}<Icon_FolderOpen size={14} class="mr-1.5" />Split & Choose Output Folder{/if}
    </Button>
  {/if}
  {#if resultMsg}
    <div class="flex items-center gap-2 p-3 rounded-lg border {resultOk ? 'border-green-500/30 bg-green-500/5' : 'border-destructive/30 bg-destructive/5'}">
      <span class="text-sm {resultOk ? 'text-green-700' : 'text-destructive'}">{resultMsg}</span>
    </div>
  {/if}
</div>
