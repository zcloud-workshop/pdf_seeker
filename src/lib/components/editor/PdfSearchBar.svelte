<script lang="ts">
  import { t } from "@/i18n/index.svelte.ts";
  import { Search, ChevronUp, ChevronDown, X, CaseSensitive, Loader2 } from "lucide-svelte";
  import type { PdfDocumentProxy } from "@/pdf-engine";
  import { currentPage } from "@/stores";

  export interface SearchMatch {
    pageNum: number;
    text: string;
    index: number;
  }

  let {
    doc,
    open = $bindable(false),
    onmatchchange,
  }: {
    doc: PdfDocumentProxy | null;
    open: boolean;
    onmatchchange?: (matches: SearchMatch[], activeIndex: number) => void;
  } = $props();

  let query = $state("");
  let caseSensitive = $state(false);
  let searching = $state(false);
  let matches = $state<SearchMatch[]>([]);
  let activeIndex = $state(0);
  let inputEl: HTMLInputElement | undefined = $state(undefined);

  let searchTimer: ReturnType<typeof setTimeout> | null = null;

  $effect(() => {
    if (open && inputEl) {
      setTimeout(() => {
        inputEl?.focus();
        inputEl?.select();
      }, 50);
    }
  });

  $effect(() => {
    // Re-run search when query, caseSensitive, or doc changes
    void query;
    void caseSensitive;
    void doc;

    if (searchTimer) clearTimeout(searchTimer);
    searchTimer = setTimeout(() => {
      runSearch();
    }, 200);

    return () => {
      if (searchTimer) clearTimeout(searchTimer);
    };
  });

  async function runSearch() {
    const q = query.trim();
    if (!doc || !q) {
      matches = [];
      activeIndex = 0;
      searching = false;
      if (onmatchchange) onmatchchange([], -1);
      return;
    }

    searching = true;
    const results: SearchMatch[] = [];
    const searchTarget = caseSensitive ? q : q.toLowerCase();

    try {
      for (let i = 1; i <= doc.numPages; i++) {
        const page = await doc.getPage(i);
        const textContent = await page.getTextContent();
        const pageText = textContent.items
          .map((item: any) => item.str || "")
          .join(" ");

        const compareText = caseSensitive ? pageText : pageText.toLowerCase();
        let startPos = 0;
        let foundIdx = 0;

        while ((foundIdx = compareText.indexOf(searchTarget, startPos)) !== -1) {
          results.push({
            pageNum: i,
            text: pageText.slice(foundIdx, foundIdx + q.length),
            index: results.length,
          });
          startPos = foundIdx + Math.max(1, searchTarget.length);
        }
      }

      matches = results;
      if (results.length > 0) {
        // Pick the first match on or after current page, if possible
        const curPage = $currentPage;
        const nearIdx = results.findIndex((m) => m.pageNum >= curPage);
        activeIndex = nearIdx !== -1 ? nearIdx : 0;
        jumpToMatch(activeIndex);
      } else {
        activeIndex = 0;
        if (onmatchchange) onmatchchange([], -1);
      }
    } catch (err) {
      console.error("PDF search error:", err);
    } finally {
      searching = false;
    }
  }

  function jumpToMatch(index: number) {
    if (matches.length === 0) return;
    const target = matches[index];
    if (!target) return;
    currentPage.set(target.pageNum);
    if (onmatchchange) onmatchchange(matches, index);
  }

  function nextMatch() {
    if (matches.length === 0) return;
    activeIndex = (activeIndex + 1) % matches.length;
    jumpToMatch(activeIndex);
  }

  function prevMatch() {
    if (matches.length === 0) return;
    activeIndex = (activeIndex - 1 + matches.length) % matches.length;
    jumpToMatch(activeIndex);
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      if (e.shiftKey) {
        prevMatch();
      } else {
        nextMatch();
      }
    } else if (e.key === "Escape") {
      e.preventDefault();
      close();
    }
  }

  function close() {
    open = false;
    matches = [];
    activeIndex = 0;
    if (onmatchchange) onmatchchange([], -1);
  }
</script>

{#if open}
  <div
    class="absolute top-3 right-5 z-40 flex items-center gap-1.5 px-3 py-1.5 bg-card/95 backdrop-blur border border-border rounded-xl shadow-xl animate-in fade-in slide-in-from-top-2 text-xs"
  >
    <Search size={14} class="text-muted-foreground shrink-0" />
    <input
      bind:this={inputEl}
      bind:value={query}
      onkeydown={handleKeyDown}
      type="text"
      placeholder="搜索关键词 (Enter跳转)..."
      class="bg-transparent border-none outline-none text-foreground placeholder:text-muted-foreground w-44 py-0.5 text-xs"
    />

    {#if searching}
      <Loader2 size={13} class="animate-spin text-muted-foreground shrink-0" />
    {:else if query.trim().length > 0}
      <span class="text-[11px] font-mono text-muted-foreground shrink-0 whitespace-nowrap px-1">
        {#if matches.length > 0}
          {activeIndex + 1}/{matches.length}
        {:else}
          无结果
        {/if}
      </span>
    {/if}

    <div class="h-4 w-px bg-border mx-0.5 shrink-0"></div>

    <!-- Case sensitive toggle -->
    <button
      onclick={() => (caseSensitive = !caseSensitive)}
      class="p-1 rounded hover:bg-accent transition-colors {caseSensitive ? 'text-primary bg-primary/10 font-bold' : 'text-muted-foreground'}"
      title="区分大小写"
    >
      <CaseSensitive size={14} />
    </button>

    <!-- Prev / Next -->
    <button
      onclick={prevMatch}
      disabled={matches.length === 0}
      class="p-1 rounded hover:bg-accent text-muted-foreground hover:text-foreground disabled:opacity-30 transition-colors"
      title="上一个 (Shift+Enter)"
    >
      <ChevronUp size={14} />
    </button>
    <button
      onclick={nextMatch}
      disabled={matches.length === 0}
      class="p-1 rounded hover:bg-accent text-muted-foreground hover:text-foreground disabled:opacity-30 transition-colors"
      title="下一个 (Enter)"
    >
      <ChevronDown size={14} />
    </button>

    <div class="h-4 w-px bg-border mx-0.5 shrink-0"></div>

    <!-- Close button -->
    <button
      onclick={close}
      class="p-1 rounded hover:bg-destructive/10 text-muted-foreground hover:text-destructive transition-colors"
      title="关闭 (Esc)"
    >
      <X size={14} />
    </button>
  </div>
{/if}

