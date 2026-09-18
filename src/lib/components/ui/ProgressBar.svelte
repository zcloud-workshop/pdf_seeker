<script lang="ts">
  export type ProgressInfo = {
    current: number;
    total: number;
    startTime: number;
    avgMsPerPage: number;
  };

  let { info }: { info: ProgressInfo } = $props();

  let eta = $derived.by(() => {
    if (!info || info.current === 0 || info.total === 0 || !info.avgMsPerPage) return null;
    const remaining = info.total - info.current;
    const ms = Math.round(info.avgMsPerPage * remaining);
    if (ms < 1000) return `${ms}ms`;
    const s = Math.round(ms / 1000);
    if (s < 60) return `${s}s`;
    const m = Math.floor(s / 60);
    const rs = s % 60;
    return `${m}m${rs}s`;
  });

  let pct = $derived(info && info.total > 0 ? (info.current / info.total) * 100 : 0);
</script>

{#if info && info.total > 0}
  <div class="space-y-1">
    <div class="flex justify-between text-xs text-muted-foreground">
      <span>{info.current} / {info.total}</span>
      <span class="flex items-center gap-3">
        {#if eta}
          <span title="Estimated time remaining">{eta}</span>
        {/if}
        <span>{Math.round(pct)}%</span>
      </span>
    </div>
    <div class="w-full bg-muted rounded-full h-1.5 overflow-hidden">
      <div
        class="bg-primary h-1.5 rounded-full transition-all duration-300"
        style="width: {pct}%"
      ></div>
    </div>
  </div>
{/if}
