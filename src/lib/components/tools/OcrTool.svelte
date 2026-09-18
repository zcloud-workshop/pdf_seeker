<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { readFile, writeTextFile } from "@tauri-apps/plugin-fs";
  import { save } from "@tauri-apps/plugin-dialog";
  import { Button } from "@/components/ui";
  import { FileUp, ScanLine, Save, Loader2, Globe } from "lucide-svelte";
  import { loadPdf } from "@/pdf-engine";
  import { currentView } from "@/stores";
  import ProgressBar from "@/components/ui/ProgressBar.svelte";
  import type { ProgressInfo } from "@/components/ui/ProgressBar.svelte";

  let { filePath = $bindable() }: { filePath: string | null } = $props();

  let ocrLanguage = $state("default");
  let ocrText = $state("");
  let busy = $state(false);
  let resultMsg = $state("");
  let resultOk = $state(false);
  let progress = $state<ProgressInfo>({ current: 0, total: 0, startTime: 0, avgMsPerPage: 0 });
  let availableLanguages: Array<{ code: string; name: string }> = $state([]);
  let showLangPicker = $state(false);
  let engineStatus = $state<string | null>(null);

  async function openFileForTool() {
    const selected = await save({ filters: [{ name: "PDF", extensions: ["pdf"] }] });
    if (selected) filePath = String(selected);
  }

  async function loadLanguages() {
    try {
      const langs = await invoke<Array<{ code: string; name: string }>>("ocr_get_languages");
      availableLanguages = langs;
    } catch (e) {
      console.error("Failed to load OCR languages:", e);
    }
  }

  async function executeOcr() {
    if (!filePath) return;

    try {
      const configured: boolean = await invoke("ocr_check_configured");
      if (!configured) {
        currentView.set("settings");
        return;
      }
    } catch {}

    busy = true;
    resultMsg = "";
    ocrText = "";
    progress = { current: 0, total: 0, startTime: 0, avgMsPerPage: 0 };

    try {
      engineStatus = "initializing";
      await invoke("ocr_init_engine", { language: ocrLanguage, gpuEnabled: true });
      engineStatus = "ready";

      const analysis = await invoke<{
        totalPages: number;
        recommendation: string;
      }>("ocr_analyze_pdf", { filePath });

      const data = await readFile(filePath);
      const doc = await loadPdf(new Uint8Array(data));
      progress.total = doc.numPages;
      progress.startTime = Date.now();

      if (analysis.recommendation === "text") {
        let totalText = "";
        for (let i = 1; i <= doc.numPages; i++) {
          const t0 = Date.now();
          progress.current = i;
          const page = await doc.getPage(i);
          const content = await page.getTextContent();
          const pageText = content.items
            .map((item: any) => item.str)
            .filter((s: string) => s.trim())
            .join(" ");
          if (pageText.trim()) {
            totalText += `\n--- Page ${i} ---\n${pageText}\n`;
          }
          progress.avgMsPerPage = (Date.now() - progress.startTime) / i;
        }
        if (totalText.trim().length > 50) {
          ocrText = totalText.trim();
          resultMsg = `Extracted ${doc.numPages} page(s) (built-in text)`;
          resultOk = true;
          return;
        }
      }

      const allText: string[] = [];
      const tempDir = await invoke<string>("get_temp_dir");

      for (let i = 1; i <= doc.numPages; i++) {
        progress.current = i;
        const page = await doc.getPage(i);
        const canvas = document.createElement("canvas");
        const vp = page.getViewport({ scale: 2 });
        canvas.width = Math.floor(vp.width);
        canvas.height = Math.floor(vp.height);
        const ctx = canvas.getContext("2d")!;
        await page.render({ canvasContext: ctx, viewport: vp }).promise;

        const blob = await new Promise<Blob | null>((r) =>
          canvas.toBlob((b) => r(b), "image/png")
        );
        if (!blob) continue;
        const ab = await blob.arrayBuffer();
        const paddedNum = String(i).padStart(4, "0");
        const imgPath = `${tempDir}/ocr_tool_${paddedNum}.png`;
        await invoke("save_image_file", {
          path: imgPath,
          data: Array.from(new Uint8Array(ab)),
        });

        const boxes = await invoke<Array<{ text: string; confidence: number }>>(
          "ocr_recognize",
          { imagePath: imgPath, language: ocrLanguage }
        );

        const pageText = boxes.map((b) => b.text).join("\n");
        if (pageText.trim()) {
          allText.push(`--- Page ${i} ---\n${pageText}`);
        }
        progress.avgMsPerPage = (Date.now() - progress.startTime) / i;
      }

      if (allText.length === 0) {
        resultMsg = "No text found in this PDF.";
        resultOk = false;
      } else {
        ocrText = allText.join("\n\n");
        resultMsg = `OCR extracted ${allText.length} page(s), ${ocrText.length} chars`;
        resultOk = true;
      }
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
    } finally {
      busy = false;
      progress = { current: 0, total: 0, startTime: 0, avgMsPerPage: 0 };
      engineStatus = null;
    }
  }

  async function saveOcrText() {
    if (!ocrText) return;
    try {
      const out = await save({ filters: [{ name: "Text", extensions: ["txt"] }] });
      if (!out) return;
      await writeTextFile(out as string, ocrText);
      resultMsg = `Saved to ${(out as string).split(/[\\/]/).pop()}`;
      resultOk = true;
    } catch (e) {
      resultMsg = String(e);
      resultOk = false;
    }
  }

  $effect(() => {
    if (filePath) loadLanguages();
  });
</script>

<div class="space-y-3">
  {#if !filePath}
    <p class="text-sm text-muted-foreground">Open a PDF first.</p>
    <div class="flex gap-2">
      <Button variant="outline" size="sm" onclick={openFileForTool}>
        <FileUp size={14} class="mr-1.5" />Open PDF
      </Button>
    </div>
  {:else}
    <p class="text-sm text-muted-foreground">
      OCR text extraction from: <strong>{filePath.split(/[\\/]/).pop()}</strong>
    </p>

    <!-- Language selector -->
    <div class="flex items-center gap-3">
      <div class="relative">
        <button
          class="flex items-center gap-1.5 px-2.5 py-1.5 text-sm border rounded-md hover:bg-muted"
          onclick={() => (showLangPicker = !showLangPicker)}
        >
          <Globe size={14} />
          <span>{availableLanguages.find((l) => l.code === ocrLanguage)?.name ?? ocrLanguage}</span>
          <span class="text-muted-foreground text-xs">&#9662;</span>
        </button>
        {#if showLangPicker}
          <div class="absolute top-full left-0 mt-1 bg-popover border rounded-md shadow-lg z-50 min-w-[200px]">
            {#each availableLanguages as lang (lang.code)}
              <button
                class="block w-full text-left px-3 py-1.5 text-sm hover:bg-muted {ocrLanguage === lang.code ? 'bg-primary/10 font-medium' : ''}"
                onclick={() => {
                  ocrLanguage = lang.code;
                  showLangPicker = false;
                }}
              >
                {lang.name}
              </button>
            {/each}
          </div>
        {/if}
      </div>
    </div>

    <!-- Progress bar -->
    <ProgressBar info={progress} />

    <Button onclick={executeOcr} disabled={busy}>
      {#if busy}
        <Loader2 size={14} class="animate-spin" />
      {:else}
        <ScanLine size={14} class="mr-1.5" />Run OCR
      {/if}
    </Button>

    {#if ocrText}
      <textarea
        readonly
        value={ocrText}
        class="w-full h-64 p-3 rounded-lg border border-input bg-muted text-sm font-mono resize-y"
      ></textarea>
      <div class="flex gap-2">
        <Button variant="outline" size="sm" onclick={saveOcrText}>
          <Save size={14} class="mr-1.5" />Save as .txt
        </Button>
        <Button variant="outline" size="sm" onclick={() => navigator.clipboard.writeText(ocrText)}>
          Copy All
        </Button>
      </div>
    {/if}
  {/if}

  {#if resultMsg}
    <div class="flex items-center gap-2 p-3 rounded-lg border {resultOk ? 'border-green-500/30 bg-green-500/5' : 'border-destructive/30 bg-destructive/5'}">
      <span class="text-sm {resultOk ? 'text-green-700' : 'text-destructive'}">{resultMsg}</span>
    </div>
  {/if}
</div>
