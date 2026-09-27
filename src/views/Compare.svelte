<script lang="ts">
  import { t, getLocale } from "@/i18n/index.svelte.ts";
  import { Button } from "@/components/ui";
  import PdfScroller from "@/components/viewer/PdfScroller.svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { invoke } from "@tauri-apps/api/core";
  import type { Tab } from "@/stores";
  import {
    CompareController,
    initialCompareState,
  } from "@/comparison/analyze-controller";
  import { createDiffService } from "@/comparison/diff-service";
  import type { PageDiff, PageStatus } from "@/comparison/page-diff";
  import { onDestroy } from "svelte";
  import {
    ArrowLeftRight,
    Columns2,
    FileText,
    GitCompare,
    Loader2,
  } from "lucide-svelte";

  // Interim bilingual copy for states without a dictionary key yet (the
  // dictionary is owned by 05). Final translations are handed to 07-E; see
  // docs/remediation/evidence/04-comparison.md for the full list.
  function lt(en: string, zh: string): string {
    return getLocale() === "zh" ? zh : en;
  }

  let pathA = $state<string | null>(null);
  let pathB = $state<string | null>(null);
  let mode = $state<"text" | "visual">("text");
  let onlyChanged = $state(false);
  let visualPage = $state<number | undefined>(undefined);

  let analysis = $state(initialCompareState());

  const controller = new CompareController({
    extract: (path) => invoke<string[]>("extract_page_texts", { path }),
    diff: createDiffService(),
    onState: (state) => (analysis = state),
  });
  onDestroy(() => controller.dispose());

  function nameOf(path: string) {
    return path.split(/[\\/]/).pop() || path;
  }

  const tabA = $derived<Tab | null>(
    pathA ? { id: "compare-a", path: pathA, name: nameOf(pathA) } : null,
  );
  const tabB = $derived<Tab | null>(
    pathB ? { id: "compare-b", path: pathB, name: nameOf(pathB) } : null,
  );

  async function pick(side: "a" | "b") {
    const selected = await open({
      multiple: false,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
    if (selected) {
      const path = typeof selected === "string" ? selected : String(selected);
      if (side === "a") pathA = path;
      else pathB = path;
      visualPage = undefined;
      analyze();
    }
  }

  function swap() {
    const a = pathA;
    pathA = pathB;
    pathB = a;
    visualPage = undefined;
    analyze();
  }

  function analyze() {
    if (!pathA || !pathB) return;
    controller.run({ a: pathA, b: pathB });
  }

  function openVisualAt(page: number) {
    visualPage = page;
    mode = "visual";
  }

  // ─── Derived results (belong to the latest completed request only) ───

  const pagePairs = $derived(analysis.pages);
  const visiblePairs = $derived(
    onlyChanged ? pagePairs.filter((p) => p.status !== "same") : pagePairs,
  );

  const summary = $derived.by(() => {
    let same = 0;
    let changed = 0;
    let onlyA = 0;
    let onlyB = 0;
    for (const p of pagePairs) {
      if (p.status === "same") same++;
      else if (p.status === "changed") changed++;
      else if (p.status === "onlyA") onlyA++;
      else if (p.status === "onlyB") onlyB++;
    }
    return { same, changed, onlyA, onlyB };
  });

  const allSame = $derived(
    pagePairs.length > 0 && pagePairs.every((p) => p.status === "same"),
  );
  const allNoText = $derived(
    pagePairs.length > 0 && pagePairs.every((p) => p.status === "noText"),
  );
  const pageCountMismatch = $derived(
    !analysis.analyzing &&
      !analysis.errorMsg &&
      analysis.textsA.length !== analysis.textsB.length,
  );

  const degradedNote = $derived.by(() => {
    const o = analysis.outcome;
    if (!o) return "";
    const parts: string[] = [];
    if (o.skippedCount > 0) {
      parts.push(
        lt(
          `${o.skippedCount} page(s) not compared (over size/budget/time limit).`,
          `已跳过 ${o.skippedCount} 页(超出规模/预算/时间上限)。`,
        ),
      );
    }
    if (o.engineFailed) {
      parts.push(
        lt(
          "Diff worker unavailable; compared on the main thread with stricter limits.",
          "比较 worker 不可用,已在主线程按更严格预算完成比较。",
        ),
      );
    }
    return parts.join(" ");
  });

  const statusBadge: Record<PageStatus, string> = {
    same: "bg-muted text-muted-foreground",
    changed: "bg-amber-500/15 text-amber-600 dark:text-amber-400",
    onlyA: "bg-red-500/10 text-red-600 dark:text-red-400",
    onlyB: "bg-green-500/10 text-green-600 dark:text-green-400",
    noText: "bg-sky-500/10 text-sky-600 dark:text-sky-400",
    skipped: "bg-orange-500/10 text-orange-600 dark:text-orange-400",
  };

  function statusLabel(status: PageStatus): string {
    if (status === "same") return t("compare.pageSame");
    if (status === "changed") return t("compare.pageChanged");
    if (status === "onlyA") return t("compare.onlyA");
    if (status === "onlyB") return t("compare.onlyB");
    if (status === "noText") return lt("No text layer", "无文字层");
    return lt("Not compared", "未比较");
  }

  function skippedNote(pair: PageDiff): string {
    if (pair.skipReason === "size") {
      return lt(
        "Page text exceeds the size budget — not compared.",
        "本页文本超出规模预算,未比较。",
      );
    }
    return lt(
      "Comparison budget or time limit reached — not compared.",
      "达到比较预算或时间上限,未比较。",
    );
  }

  // ─── Visual mode: unique active panel + leader/follower scroll sync ──

  let scrollerA: PdfScroller | undefined = $state(undefined);
  let scrollerB: PdfScroller | undefined = $state(undefined);
  let focusSide = $state<"a" | "b">("a");
  let syncing = false;

  function syncFrom(source: "a" | "b") {
    if (syncing) return;
    const src = (source === "a" ? scrollerA : scrollerB)?.getScrollContainer();
    const dst = (source === "a" ? scrollerB : scrollerA)?.getScrollContainer();
    if (!src || !dst) return;
    const max = src.scrollHeight - src.clientHeight;
    if (max <= 0) return;
    const ratio = src.scrollTop / max;
    const dstMax = dst.scrollHeight - dst.clientHeight;
    syncing = true;
    dst.scrollTop = ratio * Math.max(dstMax, 0);
    requestAnimationFrame(() => (syncing = false));
  }

  $effect(() => {
    if (mode !== "visual" || !tabA || !tabB) return;
    // Reading the children's scroll containers subscribes this effect to
    // their bind:this + internal state, so listeners attach once ready
    const a = scrollerA?.getScrollContainer();
    const b = scrollerB?.getScrollContainer();
    if (!a || !b) return;
    const ha = () => syncFrom("a");
    const hb = () => syncFrom("b");
    a.addEventListener("scroll", ha, { passive: true });
    b.addEventListener("scroll", hb, { passive: true });
    return () => {
      a.removeEventListener("scroll", ha);
      b.removeEventListener("scroll", hb);
    };
  });
</script>

<div class="flex flex-col h-full overflow-hidden">
  <!-- Header: file pickers + mode toggle -->
  <header class="shrink-0 border-b border-border bg-card px-4 py-3 space-y-3">
    <div class="flex items-center gap-3">
      <GitCompare size={18} class="text-muted-foreground shrink-0" />
      <h1 class="text-base font-semibold text-foreground">{t("compare.title")}</h1>
      <div class="flex-1"></div>
      {#if pathA && pathB}
        <div class="flex items-center rounded-lg border border-border overflow-hidden text-xs">
          <button
            class="px-3 py-1.5 transition-colors {mode === 'text'
              ? 'bg-primary text-primary-foreground'
              : 'text-muted-foreground hover:bg-accent'}"
            onclick={() => (mode = "text")}
          >
            {t("compare.textMode")}
          </button>
          <button
            class="px-3 py-1.5 transition-colors {mode === 'visual'
              ? 'bg-primary text-primary-foreground'
              : 'text-muted-foreground hover:bg-accent'}"
            onclick={() => (mode = "visual")}
          >
            {t("compare.visualMode")}
          </button>
        </div>
      {/if}
    </div>

    <div class="flex items-center gap-2">
      <button
        class="flex items-center gap-2 flex-1 min-w-0 h-9 px-3 rounded-lg border border-border hover:border-primary/50 text-xs text-left transition-colors"
        onclick={() => pick("a")}
      >
        <span class="font-mono text-muted-foreground shrink-0">A</span>
        <span class="truncate {pathA ? 'text-foreground' : 'text-muted-foreground'}">
          {pathA ? nameOf(pathA) : t("compare.pickA")}
        </span>
      </button>
      <Button variant="outline" size="icon" class="shrink-0" onclick={swap} disabled={!pathA || !pathB}>
        <ArrowLeftRight size={14} />
      </Button>
      <button
        class="flex items-center gap-2 flex-1 min-w-0 h-9 px-3 rounded-lg border border-border hover:border-primary/50 text-xs text-left transition-colors"
        onclick={() => pick("b")}
      >
        <span class="font-mono text-muted-foreground shrink-0">B</span>
        <span class="truncate {pathB ? 'text-foreground' : 'text-muted-foreground'}">
          {pathB ? nameOf(pathB) : t("compare.pickB")}
        </span>
      </button>
    </div>
  </header>

  {#if !pathA || !pathB}
    <div class="flex flex-col items-center justify-center flex-1 gap-3">
      <FileText size={40} class="text-muted-foreground/30" />
      <p class="text-sm text-muted-foreground">{t("compare.hint")}</p>
    </div>
  {:else if mode === "text"}
    <!-- Toolbar: summary + filter -->
    <div class="shrink-0 flex items-center gap-4 px-4 py-2 border-b border-border text-xs text-muted-foreground">
      {#if analysis.analyzing}
        <Loader2 size={14} class="animate-spin" />
        <span>{t("app.loading")}</span>
      {:else if !analysis.errorMsg}
        {#if allNoText}
          <span class="text-sky-600 dark:text-sky-400">
            {lt(
              "Neither file has an extractable text layer (scanned PDFs?). Text comparison is unavailable — this does not mean the files look the same.",
              "两个文件都没有可提取的文字层(扫描件?)。无法进行文字比较——这不代表两者视觉相同。",
            )}
          </span>
        {:else}
          <span class="text-green-600 dark:text-green-400">
            {t("compare.sumSame")} {summary.same}
          </span>
          <span class="text-amber-600 dark:text-amber-400">
            {t("compare.sumChanged")} {summary.changed}
          </span>
          <span class="text-red-600 dark:text-red-400">
            {t("compare.sumOnlyA")} {summary.onlyA}
          </span>
          <span class="text-green-700 dark:text-green-500">
            {t("compare.sumOnlyB")} {summary.onlyB}
          </span>
          <div class="flex-1"></div>
          <label class="flex items-center gap-1.5 cursor-pointer">
            <input type="checkbox" bind:checked={onlyChanged} class="rounded" />
            <span>{t("compare.onlyChanged")}</span>
          </label>
        {/if}
      {/if}
    </div>

    {#if !analysis.analyzing && !analysis.errorMsg && (pageCountMismatch || degradedNote)}
      <div class="shrink-0 px-4 py-1.5 border-b border-border text-[11px] text-muted-foreground space-y-0.5">
        {#if pageCountMismatch}
          <p>
            {lt(
              "Page counts differ; pages are paired by page number, so inserted or removed pages shift later pages.",
              "两文件页数不同;按相同页号配对,插入或删除页会使后续页错位。",
            )}
          </p>
        {/if}
        {#if degradedNote}
          <p>{degradedNote}</p>
        {/if}
      </div>
    {/if}

    <div class="flex-1 overflow-auto p-4 space-y-3">
      {#if !analysis.analyzing && analysis.errorMsg}
        <div class="flex items-center justify-center h-full">
          <span class="text-sm text-destructive break-all px-4">{analysis.errorMsg}</span>
        </div>
      {:else if !analysis.analyzing}
        {#each visiblePairs as pair (pair.index)}
          <section class="rounded-lg border border-border bg-card overflow-hidden">
            <header class="flex items-center justify-between px-3 py-2 border-b border-border bg-muted/30">
              <span class="text-xs font-medium text-foreground">
                {t("compare.page")} {pair.index}
              </span>
              <div class="flex items-center gap-3">
                <span class="text-[11px] px-1.5 py-0.5 rounded {statusBadge[pair.status]}">
                  {statusLabel(pair.status)}
                </span>
                {#if pair.status !== "same"}
                  <button
                    class="flex items-center gap-1 text-[11px] text-muted-foreground hover:text-foreground"
                    onclick={() => openVisualAt(pair.index)}
                  >
                    <Columns2 size={12} />
                    {t("compare.openVisual")}
                  </button>
                {/if}
              </div>
            </header>
            <div class="font-mono text-[11px] leading-5 max-h-96 overflow-auto">
              {#if pair.ops}
                {#each pair.ops as op, i (i)}
                  {#if op.type === "same"}
                    <div class="px-3 text-muted-foreground/70 whitespace-pre-wrap">
                      {op.a}
                    </div>
                  {:else if op.type === "del"}
                    <div class="px-3 bg-red-500/10 text-red-700 dark:text-red-400 whitespace-pre-wrap">
                      - {op.a}
                    </div>
                  {:else}
                    <div class="px-3 bg-green-500/10 text-green-700 dark:text-green-400 whitespace-pre-wrap">
                      + {op.b}
                    </div>
                  {/if}
                {/each}
              {:else if pair.status === "onlyA"}
                <div class="px-3 py-1.5 text-muted-foreground whitespace-pre-wrap">
                  {pair.textA}
                </div>
              {:else if pair.status === "onlyB"}
                <div class="px-3 py-1.5 text-muted-foreground whitespace-pre-wrap">
                  {pair.textB}
                </div>
              {:else if pair.status === "noText"}
                <div class="px-3 py-1.5 text-muted-foreground whitespace-pre-wrap">
                  {lt(
                    "No extractable text on either side — visual equality was not assessed.",
                    "两侧均无可提取文字,未评估视觉是否相同。",
                  )}
                </div>
              {:else}
                <div class="px-3 py-1.5 text-muted-foreground whitespace-pre-wrap">
                  {skippedNote(pair)}
                </div>
              {/if}
            </div>
          </section>
        {:else}
          {#if allSame}
            <div class="flex items-center justify-center h-full">
              <span class="text-sm text-green-600">{t("compare.identical")}</span>
            </div>
          {/if}
        {/each}
      {/if}
    </div>
  {:else}
    <!-- Side-by-side visual compare -->
    <div class="shrink-0 px-4 py-1.5 border-b border-border bg-muted/30 text-[11px] text-muted-foreground">
      {lt(
        "Scroll sync is proportional, not content-aligned; shortcuts act on the highlighted panel.",
        "滚动按比例同步,不代表内容对齐;快捷键作用于高亮面板。",
      )}
    </div>
    <div class="flex-1 flex min-h-0">
      {#if tabA && tabB}
        <div
          role="region"
          aria-label="A: {tabA.name}"
          class="flex-1 min-w-0 border-r border-border {focusSide === 'a' ? 'ring-1 ring-inset ring-primary/40' : ''}"
          onpointerenter={() => (focusSide = "a")}
          onfocusin={() => (focusSide = "a")}
        >
          <div class="h-7 px-3 flex items-center text-[11px] text-muted-foreground border-b border-border bg-muted/30 truncate">
            <span class="font-mono mr-2">A</span>{tabA.name}
          </div>
          <div class="h-[calc(100%-1.75rem)]">
            <PdfScroller tab={tabA} active={focusSide === "a"} initialPage={visualPage} bind:this={scrollerA} />
          </div>
        </div>
        <div
          role="region"
          aria-label="B: {tabB.name}"
          class="flex-1 min-w-0 {focusSide === 'b' ? 'ring-1 ring-inset ring-primary/40' : ''}"
          onpointerenter={() => (focusSide = "b")}
          onfocusin={() => (focusSide = "b")}
        >
          <div class="h-7 px-3 flex items-center text-[11px] text-muted-foreground border-b border-border bg-muted/30 truncate">
            <span class="font-mono mr-2">B</span>{tabB.name}
          </div>
          <div class="h-[calc(100%-1.75rem)]">
            <PdfScroller tab={tabB} active={focusSide === "b"} initialPage={visualPage} bind:this={scrollerB} />
          </div>
        </div>
      {/if}
    </div>
  {/if}
</div>
