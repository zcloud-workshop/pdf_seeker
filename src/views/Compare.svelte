<script lang="ts">
  import { t } from "@/i18n/index.svelte.ts";
  import { Button } from "@/components/ui";
  import PdfScroller from "@/components/viewer/PdfScroller.svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { invoke } from "@tauri-apps/api/core";
  import {
    diffLines,
    opsHaveChanges,
    splitLines,
    type DiffOp,
  } from "@/utils/diff";
  import type { Tab } from "@/stores";
  import {
    ArrowLeftRight,
    Columns2,
    FileText,
    GitCompare,
    Loader2,
  } from "lucide-svelte";

  let pathA = $state<string | null>(null);
  let pathB = $state<string | null>(null);
  let mode = $state<"text" | "visual">("text");
  let analyzing = $state(false);
  let errorMsg = $state("");
  let onlyChanged = $state(false);
  let textsA = $state<string[]>([]);
  let textsB = $state<string[]>([]);
  let visualPage = $state<number | undefined>(undefined);

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

  async function analyze() {
    if (!pathA || !pathB) return;
    analyzing = true;
    errorMsg = "";
    try {
      const [ta, tb] = await Promise.all([
        invoke<string[]>("extract_page_texts", { path: pathA }),
        invoke<string[]>("extract_page_texts", { path: pathB }),
      ]);
      textsA = ta;
      textsB = tb;
    } catch (e) {
      errorMsg = String(e);
      textsA = [];
      textsB = [];
    } finally {
      analyzing = false;
    }
  }

  interface PagePair {
    index: number;
    ops: DiffOp[] | null;
    status: "same" | "changed" | "onlyA" | "onlyB";
    textA?: string;
    textB?: string;
  }

  const pagePairs = $derived.by(() => {
    const pairs: PagePair[] = [];
    const max = Math.max(textsA.length, textsB.length);
    for (let i = 0; i < max; i++) {
      const hasA = i < textsA.length;
      const hasB = i < textsB.length;
      if (hasA && hasB) {
        const ops = diffLines(splitLines(textsA[i]), splitLines(textsB[i]));
        pairs.push({
          index: i + 1,
          ops,
          status: opsHaveChanges(ops) ? "changed" : "same",
        });
      } else if (hasA) {
        pairs.push({ index: i + 1, ops: null, status: "onlyA", textA: textsA[i] });
      } else {
        pairs.push({ index: i + 1, ops: null, status: "onlyB", textB: textsB[i] });
      }
    }
    return pairs;
  });

  const summary = $derived.by(() => {
    let same = 0;
    let changed = 0;
    let onlyA = 0;
    let onlyB = 0;
    for (const p of pagePairs) {
      if (p.status === "same") same++;
      else if (p.status === "changed") changed++;
      else if (p.status === "onlyA") onlyA++;
      else onlyB++;
    }
    return { same, changed, onlyA, onlyB };
  });

  const visiblePairs = $derived(
    onlyChanged ? pagePairs.filter((p) => p.status !== "same") : pagePairs,
  );

  const statusBadge: Record<PagePair["status"], string> = {
    same: "bg-muted text-muted-foreground",
    changed: "bg-amber-500/15 text-amber-600 dark:text-amber-400",
    onlyA: "bg-red-500/10 text-red-600 dark:text-red-400",
    onlyB: "bg-green-500/10 text-green-600 dark:text-green-400",
  };

  function statusLabel(status: PagePair["status"]): string {
    if (status === "same") return t("compare.pageSame");
    if (status === "changed") return t("compare.pageChanged");
    if (status === "onlyA") return t("compare.onlyA");
    return t("compare.onlyB");
  }

  function openVisualAt(page: number) {
    visualPage = page;
    mode = "visual";
  }

  // ─── Visual mode: leader/follower scroll sync ──────────────────────

  let scrollerA: PdfScroller | undefined = $state(undefined);
  let scrollerB: PdfScroller | undefined = $state(undefined);
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
      {#if analyzing}
        <Loader2 size={14} class="animate-spin" />
        <span>{t("app.loading")}</span>
      {:else if !errorMsg}
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
    </div>

    <div class="flex-1 overflow-auto p-4 space-y-3">
      {#if !analyzing && errorMsg}
        <div class="flex items-center justify-center h-full">
          <span class="text-sm text-destructive break-all px-4">{errorMsg}</span>
        </div>
      {:else if !analyzing}
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
              {:else}
                <div class="px-3 py-1.5 text-muted-foreground whitespace-pre-wrap">
                  {pair.textB}
                </div>
              {/if}
            </div>
          </section>
        {:else}
          {#if pagePairs.length > 0 && visiblePairs.length === 0}
            <div class="flex items-center justify-center h-full">
              <span class="text-sm text-green-600">{t("compare.identical")}</span>
            </div>
          {/if}
        {/each}
      {/if}
    </div>
  {:else}
    <!-- Side-by-side visual compare -->
    <div class="flex-1 flex min-h-0">
      {#if tabA && tabB}
        <div class="flex-1 min-w-0 border-r border-border">
          <div class="h-7 px-3 flex items-center text-[11px] text-muted-foreground border-b border-border bg-muted/30 truncate">
            <span class="font-mono mr-2">A</span>{tabA.name}
          </div>
          <div class="h-[calc(100%-1.75rem)]">
            <PdfScroller tab={tabA} active={true} initialPage={visualPage} bind:this={scrollerA} />
          </div>
        </div>
        <div class="flex-1 min-w-0">
          <div class="h-7 px-3 flex items-center text-[11px] text-muted-foreground border-b border-border bg-muted/30 truncate">
            <span class="font-mono mr-2">B</span>{tabB.name}
          </div>
          <div class="h-[calc(100%-1.75rem)]">
            <PdfScroller tab={tabB} active={true} initialPage={visualPage} bind:this={scrollerB} />
          </div>
        </div>
      {/if}
    </div>
  {/if}
</div>
